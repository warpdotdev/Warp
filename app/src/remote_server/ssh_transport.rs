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
use remote_server::transport::{Connection, Error, RemoteTransport};

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
            "{}/server.sock",
            remote_server_daemon_dir(&self.auth_context.remote_server_identity_key())
        )
    }

    pub fn remote_daemon_pid_path(&self) -> String {
        format!(
            "{}/server.pid",
            remote_server_daemon_dir(&self.auth_context.remote_server_identity_key())
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
            let cmd = format!("test -x {}", remote_server::setup::remote_server_binary());
            let output = remote_server::ssh::run_ssh_command(
                &socket_path,
                &cmd,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await?;
            // `test -x` exits 0 when present+executable, 1 when missing.
            // Anything else (e.g. SSH exit 255 for a dead connection, or
            // signal termination) is a transport-level failure.
            match output.status.code() {
                Some(0) => Ok(true),
                Some(1) => Ok(false),
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

    fn install_binary(&self) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let script = remote_server::setup::install_script(None);
            log::info!(
                "Installing remote server binary to {}",
                remote_server::setup::remote_server_binary()
            );
            match remote_server::ssh::run_ssh_script(
                &socket_path,
                &script,
                remote_server::setup::INSTALL_TIMEOUT,
            )
            .await
            {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output)
                    if output.status.code()
                        == Some(remote_server::setup::NO_HTTP_CLIENT_EXIT_CODE) =>
                {
                    log::info!("Remote has no curl/wget, falling back to SCP upload");
                    scp_install_fallback(&socket_path)
                        .await
                        .map_err(Error::Other)
                }
                Ok(output)
                    if output.status.code()
                        == Some(remote_server::setup::DOWNLOAD_FAILED_EXIT_CODE) =>
                {
                    log::info!(
                        "Remote download failed (both HTTP clients tried), \
                         falling back to SCP upload"
                    );
                    scp_install_fallback(&socket_path)
                        .await
                        .map_err(Error::Other)
                }
                Ok(output)
                    if output.status.code() == Some(remote_server::setup::NO_TAR_EXIT_CODE) =>
                {
                    log::info!("Remote has no tar, falling back to direct binary upload");
                    scp_install_binary_direct(&socket_path)
                        .await
                        .map_err(Error::Other)
                }
                Ok(output) => {
                    let exit_code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    // Do not attempt fallback for host-level failures that no
                    // alternate download strategy can fix.
                    if remote_server::setup::is_non_retryable_host_error(&stderr) {
                        return Err(Error::ScriptFailed { exit_code, stderr });
                    }
                    // SSH exit 255 means the connection itself is dead —
                    // no point attempting an SCP fallback.
                    if exit_code == 255 {
                        return Err(Error::ScriptFailed { exit_code, stderr });
                    }
                    Err(Error::ScriptFailed { exit_code, stderr })
                }
                // Timeout: the install script did not complete in time.
                // If the timeout is likely from the download phase, the SCP
                // fallback may succeed. Host-filesystem timeouts are rare;
                // if the SCP fallback also times out, the error propagates.
                Err(SshCommandError::TimedOut { .. }) => {
                    log::info!("Install script timed out, attempting SCP fallback");
                    scp_install_fallback(&socket_path)
                        .await
                        .map_err(|fallback_err| {
                            log::warn!("SCP fallback also failed after timeout: {fallback_err:#}");
                            Error::TimedOut
                        })
                }
                Err(e) => Err(Error::Other(e.into())),
            }
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

            let (client, event_rx) =
                RemoteServerClient::from_child_streams(stdin, stdout, stderr, &executor);
            Ok(Connection {
                client,
                event_rx,
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

/// SCP install fallback: downloads the tarball locally, uploads it to
/// the remote via SCP, then re-invokes the install script with the
/// staging path baked in so the shared extraction tail runs.
async fn scp_install_fallback(socket_path: &Path) -> anyhow::Result<()> {
    let (_platform, tmp_dir) = download_tarball_locally(socket_path).await?;
    let temp_client_tarball_path = tmp_dir.path().join("oz.tar.gz");

    let remote_tarball_path = format!(
        "{}/oz-upload.tar.gz",
        remote_server::setup::remote_server_dir()
    );
    let timeout = remote_server::setup::SCP_INSTALL_TIMEOUT;

    // Upload to the remote via SCP.
    log::info!("Uploading tarball to remote at {remote_tarball_path}");
    remote_server::ssh::scp_upload(
        socket_path,
        &temp_client_tarball_path,
        &remote_tarball_path,
        timeout,
    )
    .await?;

    // Run the install script with the staging path baked in.
    // The script's `staging_tarball_path` variable is non-empty, so it
    // skips the download and extracts from the uploaded tarball.
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

/// Direct binary upload fallback: downloads the tarball locally, extracts
/// it locally, and uploads only the resolved binary via SCP. This avoids
/// requiring `tar` on the remote host.
///
/// The remote-side steps are:
///   1. `mkdir -p <install_dir>` (ensures the directory exists)
///   2. SCP the binary to a staging path
///   3. `chmod +x && mv` to the final install path
async fn scp_install_binary_direct(socket_path: &Path) -> anyhow::Result<()> {
    let (_platform, tmp_dir) = download_tarball_locally(socket_path).await?;
    let temp_client_tarball_path = tmp_dir.path().join("oz.tar.gz");
    let timeout = remote_server::setup::SCP_INSTALL_TIMEOUT;

    // Extract locally using the client machine's tar.
    log::info!("Extracting tarball locally for direct binary upload");
    let extract_dir = tmp_dir.path().join("extracted");
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create local extraction dir: {e}"))?;

    let tar_output = command::r#async::Command::new("tar")
        .arg("-xzf")
        .arg(&temp_client_tarball_path)
        .arg("-C")
        .arg(&extract_dir)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn local tar: {e}"))?;
    if !tar_output.status.success() {
        let stderr = String::from_utf8_lossy(&tar_output.stderr);
        return Err(anyhow::anyhow!(
            "Local tar extraction failed (exit {:?}): {stderr}",
            tar_output.status.code()
        ));
    }

    // Find the binary in the extraction directory.
    let binary_path = find_oz_binary_in_dir(&extract_dir)?;

    // Upload the binary directly to a staging path on the remote.
    let remote_binary = remote_server::setup::remote_server_binary();
    let remote_staging = format!("{remote_binary}.staging");

    log::info!("Uploading binary directly to remote at {remote_staging}");
    remote_server::ssh::scp_upload(socket_path, &binary_path, &remote_staging, timeout).await?;

    // chmod +x and move to final location on the remote.
    let finalize_cmd =
        format!("chmod +x '{remote_staging}' && mv '{remote_staging}' '{remote_binary}'");
    log::info!("Finalizing remote binary: {finalize_cmd}");
    let output = remote_server::ssh::run_ssh_command(socket_path, &finalize_cmd, timeout).await?;
    if output.status.success() {
        Ok(())
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "Remote finalize failed (exit {code}): {stderr}"
        ))
    }
}

/// Downloads the remote-server tarball to a local temp directory.
/// Returns the detected remote platform and the temp directory handle.
///
/// Shared by [`scp_install_fallback`] and [`scp_install_binary_direct`]
/// to avoid duplicating the platform-detection + local-download logic.
async fn download_tarball_locally(
    socket_path: &Path,
) -> anyhow::Result<(remote_server::setup::RemotePlatform, tempfile::TempDir)> {
    use std::process::Stdio;

    // Detect the remote platform so we can construct the correct download URL.
    // This is a redundant uname call (the manager already ran detect_platform
    // earlier), but it only happens on the rare SCP fallback path and avoids
    // threading the platform through the trait.
    let platform = detect_remote_platform(socket_path)
        .await
        .map_err(|e| anyhow::anyhow!("SCP fallback: {e:#}"))?;

    let url = remote_server::setup::download_tarball_url(&platform);

    let tmp_dir =
        tempfile::tempdir().map_err(|e| anyhow::anyhow!("Failed to create local temp dir: {e}"))?;
    let temp_client_tarball_path = tmp_dir.path().join("oz.tar.gz");

    log::info!("Downloading tarball locally from {url}");
    let output = command::r#async::Command::new("curl")
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

    Ok((platform, tmp_dir))
}

/// Walks `dir` for the first file whose name starts with `oz` and is not
/// a `.tar.gz`, matching the install script's `find` invocation.
fn find_oz_binary_in_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name.starts_with("oz") && !name.ends_with(".tar.gz") {
            return Ok(entry.into_path());
        }
    }
    Err(anyhow::anyhow!("no binary found in extracted tarball"))
}

#[cfg(test)]
#[path = "ssh_transport_tests.rs"]
mod tests;
