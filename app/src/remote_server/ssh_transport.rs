//! SSH-specific implementation of [`RemoteTransport`].
//!
//! [`SshTransport`] uses an existing SSH ControlMaster socket to check/install
//! the remote server binary and to launch the `remote-server-proxy` process
//! whose stdin/stdout become the protocol channel.
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use warpui::r#async::executor;

use remote_server::auth::RemoteServerAuthContext;
use remote_server::client::RemoteServerClient;
use remote_server::manager::RemoteServerExitStatus;
use remote_server::setup::{
    parse_uname_output, remote_server_daemon_dir, PreinstallCheckResult, RemotePlatform,
};
use remote_server::ssh::{ssh_args, SshCommandError};
use remote_server::transport::{Connection, Error, InstallOutcome, InstallSource, RemoteTransport};

/// SSH transport: connects via a ControlMaster socket.
///
/// `socket_path` is the local Unix socket created by the ControlMaster
/// process (`ssh -N -o ControlMaster=yes -o ControlPath=<path>`). All SSH
/// commands (binary check, install, proxy launch) are multiplexed through
/// this socket without re-authenticating.
#[derive(Clone)]
pub struct SshTransport {
    socket_path: PathBuf,
    auth_context: Arc<RemoteServerAuthContext>,
}

impl fmt::Debug for SshTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SshTransport")
            .field("socket_path", &self.socket_path)
            .finish_non_exhaustive()
    }
}

impl SshTransport {
    pub fn new(socket_path: PathBuf, auth_context: Arc<RemoteServerAuthContext>) -> Self {
        Self {
            socket_path,
            auth_context,
        }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    pub fn remote_daemon_socket_path(&self) -> String {
        format!(
            "{}/{}",
            remote_server_daemon_dir(&self.auth_context.remote_server_identity_key()),
            remote_server::setup::daemon_socket_name(),
        )
    }

    pub fn remote_daemon_pid_path(&self) -> String {
        format!(
            "{}/{}",
            remote_server_daemon_dir(&self.auth_context.remote_server_identity_key()),
            remote_server::setup::daemon_pid_name(),
        )
    }

    fn remote_proxy_command(&self) -> String {
        let binary = remote_server::setup::remote_server_binary();
        let identity_key = self.auth_context.remote_server_identity_key();
        let quoted_identity_key = shell_words::quote(&identity_key);
        format!("{binary} remote-server-proxy --identity-key {quoted_identity_key}")
    }
}

/// Runs `uname -sm` on the remote host via the ControlMaster socket and
/// parses the output into a [`RemotePlatform`].
async fn detect_remote_platform(socket_path: &Path) -> Result<RemotePlatform, Error> {
    let output = remote_server::ssh::run_ssh_command(
        socket_path,
        "uname -sm",
        remote_server::setup::CHECK_TIMEOUT,
    )
    .await?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_uname_output(&stdout)
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::Other(anyhow::anyhow!(
            "uname -sm exited with code {code}: {stderr}"
        )))
    }
}

impl RemoteTransport for SshTransport {
    fn detect_platform(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<RemotePlatform, Error>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move { detect_remote_platform(&socket_path).await })
    }

    fn run_preinstall_check(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<PreinstallCheckResult, Error>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            match remote_server::ssh::run_ssh_script(
                &socket_path,
                remote_server::setup::PREINSTALL_CHECK_SCRIPT,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await
            {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    Ok(PreinstallCheckResult::parse(&stdout))
                }
                Ok(output) => {
                    let exit_code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    Err(Error::ScriptFailed { exit_code, stderr })
                }
                Err(e) => Err(e.into()),
            }
        })
    }

    fn check_binary(&self) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let cmd = remote_server::setup::binary_check_command();
            log::info!("Running binary check: {cmd}");
            let output = remote_server::ssh::run_ssh_command(
                &socket_path,
                &cmd,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await?;
            // `<binary> --version` exits 0 when present, executable, and
            // functional. Exit 127 means the binary was not found, and 126
            // means it exists but is not executable. Any other non-zero
            // exit (e.g. SSH exit 255 for a dead connection, or signal
            // termination) is treated as a transport-level failure.
            let code = output.status.code();
            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("Binary check result: exit={code:?} stdout={stdout}");
            match code {
                Some(0) => Ok(true),
                Some(126) | Some(127) => Ok(false),
                Some(code) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(Error::Other(anyhow::anyhow!(
                        "binary check exited with code {code}: {stderr}"
                    )))
                }
                None => Err(Error::Other(anyhow::anyhow!(
                    "binary check terminated by signal"
                ))),
            }
        })
    }

    fn check_has_old_binary(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            // Treat the existence of the remote-server install directory
            // itself as evidence of a prior install. If `~/.warp-XX/remote-server`
            // exists, something was installed there before, so any mismatch
            // with the client's expected binary path should be auto-updated
            // rather than surfaced as a first-time install prompt.
            let cmd = format!("test -d {}", remote_server::setup::remote_server_dir());
            let output = remote_server::ssh::run_ssh_command(
                &socket_path,
                &cmd,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await?;
            // `test -d` exits 0 when present, 1 when missing.
            // Anything else is treated as a check failure.
            match output.status.code() {
                Some(0) => Ok(true),
                Some(1) => Ok(false),
                Some(code) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(anyhow::anyhow!(
                        "remote-server dir check exited with code {code}: {stderr}"
                    ))
                }
                None => Err(anyhow::anyhow!(
                    "remote-server dir check terminated by signal"
                )),
            }
        })
    }

    fn install_binary(&self) -> Pin<Box<dyn Future<Output = InstallOutcome> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let binary_path = remote_server::setup::remote_server_binary();
            log::info!("Installing remote server binary to {binary_path}");
            let mut outcome = match install_on_server(&socket_path).await {
                Ok(()) => InstallOutcome {
                    source: Some(InstallSource::Server),
                    result: Ok(()),
                },
                Err(server_err) => {
                    let should_try_scp = !should_skip_scp_fallback(&server_err);

                    if should_try_scp {
                        log::info!("Remote server has no curl/wget, falling back to SCP upload");
                        match scp_install_fallback(&socket_path).await {
                            Ok(()) => InstallOutcome {
                                source: Some(InstallSource::Client),
                                result: Ok(()),
                            },
                            Err(e) => InstallOutcome {
                                source: Some(InstallSource::Client),
                                result: Err(Error::Other(e)),
                            },
                        }
                    } else {
                        InstallOutcome {
                            source: Some(InstallSource::Server),
                            result: Err(server_err),
                        }
                    }
                }
            };

            // Post-install verification: confirm the binary actually
            // landed at the expected path and is functional. This catches
            // silent install failures (e.g. tilde-expansion bugs) that
            // would otherwise surface as a cryptic "Response channel
            // closed" error during the IPC handshake.
            if outcome.result.is_ok() {
                log::info!("Running post-install verification for {binary_path}");
                let check_cmd = remote_server::setup::binary_check_command();
                let verify = remote_server::ssh::run_ssh_command(
                    &socket_path,
                    &check_cmd,
                    remote_server::setup::CHECK_TIMEOUT,
                )
                .await;
                match verify {
                    Ok(output) if output.status.success() => {}
                    Ok(output) => {
                        let code = output.status.code().unwrap_or(-1);
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        outcome.result = Err(Error::Other(anyhow::anyhow!(
                            "Post-install verification failed: binary not found or not \
                             executable at {binary_path} (exit {code}): {stderr}"
                        )));
                    }
                    Err(e) => {
                        outcome.result = Err(Error::Other(anyhow::anyhow!(
                            "Post-install verification failed: {e}"
                        )));
                    }
                }
            }

            outcome
        })
    }

    fn connect(
        &self,
        executor: Arc<executor::Background>,
    ) -> Pin<Box<dyn Future<Output = Result<Connection>> + Send>> {
        let socket_path = self.socket_path.clone();
        let remote_proxy_command = self.remote_proxy_command();
        Box::pin(async move {
            let mut args = ssh_args(&socket_path);
            args.push(remote_proxy_command);

            // `kill_on_drop(true)` pairs with ownership of the `Child` being
            // returned in the [`Connection`] below: the
            // [`RemoteServerManager`] holds the `Child` on its per-session
            // state, and dropping that state (on explicit teardown or
            // spontaneous disconnect) sends SIGKILL to this ssh process.
            let mut child = command::r#async::Command::new("ssh")
                .args(&args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()?;

            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stdin"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stdout"))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stderr"))?;

            let (client, event_rx, failure_rx) =
                RemoteServerClient::from_child_streams(stdin, stdout, stderr, &executor);
            Ok(Connection {
                client,
                event_rx,
                failure_rx,
                child,
                control_path: Some(socket_path),
            })
        })
    }

    fn remove_remote_server_binary(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let cmd = format!("rm -f {}", remote_server::setup::remote_server_binary());
            log::info!("Removing stale remote server binary: {cmd}");
            let output = remote_server::ssh::run_ssh_command(
                &socket_path,
                &cmd,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await?;
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow::anyhow!("Failed to remove binary: {stderr}"))
            }
        })
    }

    /// SSH exit code 255 indicates a connection-level error (broken pipe,
    /// connection reset, host unreachable) — the ControlMaster's TCP
    /// connection is dead. A signal kill also suggests the transport was
    /// torn down. In either case, reconnecting through the same
    /// ControlMaster is futile.
    fn is_reconnectable(&self, exit_status: Option<&RemoteServerExitStatus>) -> bool {
        match exit_status {
            Some(s) => s.code != Some(255) && !s.signal_killed,
            // No exit status available — optimistically allow reconnect.
            None => true,
        }
    }
}

/// Exit codes where SCP fallback would not help because the failure
/// is on the remote host itself (not a network/download issue).
fn should_skip_scp_fallback(error: &Error) -> bool {
    // Unsupported arch/OS — SCP won't change the architecture
    matches!(error, Error::ScriptFailed { exit_code , .. } if *exit_code == 2)
}

/// Runs the install script on the remote host to download and install
/// the binary directly from the CDN.
async fn install_on_server(socket_path: &Path) -> Result<(), Error> {
    let script = remote_server::setup::install_script(None);
    match remote_server::ssh::run_ssh_script(
        socket_path,
        &script,
        remote_server::setup::INSTALL_TIMEOUT,
    )
    .await
    {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(Error::ScriptFailed { exit_code, stderr })
        }
        Err(SshCommandError::TimedOut { .. }) => Err(Error::TimedOut),
        Err(e) => Err(Error::Other(e.into())),
    }
}

/// SCP install fallback: downloads the tarball locally, uploads it to
/// the remote via SCP, then re-invokes the install script with the
/// staging path baked in so the shared extraction tail runs.
async fn scp_install_fallback(socket_path: &Path) -> anyhow::Result<()> {
    use std::process::Stdio;

    // Detect the remote platform so we can construct the correct download URL.
    // This is a redundant uname call (the manager already ran detect_platform
    // earlier), but it only happens on the rare SCP fallback path and avoids
    // threading the platform through the trait.
    let platform = detect_remote_platform(socket_path)
        .await
        .map_err(|e| anyhow::anyhow!("SCP fallback: {e:#}"))?;

    let url = remote_server::setup::download_tarball_url(&platform);
    let remote_tarball_path = format!(
        "{}/oz-upload.tar.gz",
        remote_server::setup::remote_server_dir()
    );
    let timeout = remote_server::setup::SCP_INSTALL_TIMEOUT;

    // 1. Download the tarball locally into a temp directory.
    let tmp_dir =
        tempfile::tempdir().map_err(|e| anyhow::anyhow!("Failed to create local temp dir: {e}"))?;
    let temp_client_tarball_path = tmp_dir.path().join("oz.tar.gz");

    log::info!("Downloading tarball locally from {url}");
    let output = command::r#async::Command::new("curl")
        // -f: fail silently on HTTP errors (non-zero exit instead of HTML error page)
        // -S: show errors even when -f is used
        // -L: follow redirects (the CDN may 302 to a regional edge)
        .arg("-fSL")
        .arg("--connect-timeout")
        .arg("15")
        .arg(&url)
        .arg("-o")
        .arg(&temp_client_tarball_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn local curl: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Local curl failed (exit {:?}): {stderr}",
            output.status.code()
        ));
    }

    // 2. Upload to the remote via SCP.
    log::info!("Uploading tarball to remote at {remote_tarball_path}");
    remote_server::ssh::scp_upload(
        socket_path,
        &temp_client_tarball_path,
        &remote_tarball_path,
        timeout,
    )
    .await?;

    // 3. Run the install script with the staging path baked in.
    //    The script's `staging_tarball_path` variable is non-empty, so it
    //    skips the download and extracts from the uploaded tarball.
    log::info!("Running extraction via install script with tarball at {remote_tarball_path}");

    let script = remote_server::setup::install_script(Some(&remote_tarball_path));

    let output = remote_server::ssh::run_ssh_script(socket_path, &script, timeout).await?;
    if output.status.success() {
        Ok(())
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "Extraction script failed (exit {code}): {stderr}"
        ))
    }
}

#[cfg(test)]
#[path = "ssh_transport_tests.rs"]
mod tests;
