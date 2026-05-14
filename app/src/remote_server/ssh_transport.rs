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

use anyhow::{anyhow, Result};
use warpui::r#async::{executor, FutureExt as _};

use remote_server::auth::RemoteServerAuthContext;
use remote_server::client::RemoteServerClient;
use remote_server::setup::{
    parse_uname_output, remote_server_daemon_dir, PreinstallCheckResult, RemotePlatform,
};
use remote_server::ssh::ssh_args;
use remote_server::transport::{Connection, RemoteTransport};

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

#[derive(Debug)]
enum InstallError {
    ScriptFailed { exit_code: i32, stderr: String },
    Other(anyhow::Error),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScriptFailed { exit_code, stderr } => {
                write!(f, "install script failed (exit {exit_code}): {stderr}")
            }
            Self::Other(error) => write!(f, "{error:#}"),
        }
    }
}

impl From<anyhow::Error> for InstallError {
    fn from(error: anyhow::Error) -> Self {
        Self::Other(error)
    }
}

async fn detect_remote_platform(socket_path: &Path) -> Result<RemotePlatform> {
    let output = remote_server::ssh::run_ssh_command(
        socket_path,
        "uname -sm",
        remote_server::setup::CHECK_TIMEOUT,
    )
    .await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return parse_uname_output(&stdout);
    }

    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(anyhow!("uname -sm exited with code {code}: {stderr}"))
}

async fn verify_installed_binary(socket_path: &Path) -> Result<()> {
    let output = remote_server::ssh::run_ssh_command(
        socket_path,
        &remote_server::setup::binary_check_command(),
        remote_server::setup::CHECK_TIMEOUT,
    )
    .await?;

    if output.status.success() {
        return Ok(());
    }

    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(anyhow!(
        "installed binary check failed with code {code}: {stderr}"
    ))
}

async fn run_install_script(
    socket_path: &Path,
    staging_tarball_path: Option<&str>,
    timeout: std::time::Duration,
) -> core::result::Result<(), InstallError> {
    let script = remote_server::setup::install_script(staging_tarball_path);
    match remote_server::ssh::run_ssh_script(socket_path, &script, timeout).await {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(InstallError::ScriptFailed { exit_code, stderr })
        }
        Err(error) => Err(InstallError::Other(error)),
    }
}

fn should_skip_scp_fallback(error: &InstallError) -> bool {
    matches!(error, InstallError::ScriptFailed { exit_code: 2, .. })
}

async fn download_remote_server_tarball(download_url: &str, tarball_path: &Path) -> Result<()> {
    let output = async {
        command::r#async::Command::new("curl")
            .arg("-fSL")
            .arg("--connect-timeout")
            .arg("15")
            .arg(download_url)
            .arg("-o")
            .arg(tarball_path.as_os_str())
            .kill_on_drop(true)
            .output()
            .await
    }
    .with_timeout(remote_server::setup::SCP_INSTALL_TIMEOUT)
    .await
    .map_err(|_| {
        anyhow!(
            "local tarball download timed out after {:?}",
            remote_server::setup::SCP_INSTALL_TIMEOUT
        )
    })?
    .map_err(|e| anyhow!("local curl failed to execute: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(anyhow!(
        "local tarball download failed with code {code}: {stderr}"
    ))
}

async fn scp_install_fallback(socket_path: &Path) -> Result<()> {
    let platform = detect_remote_platform(socket_path).await?;
    let download_url = remote_server::setup::download_tarball_url(&platform);
    let remote_server_dir = remote_server::setup::remote_server_dir();
    let mkdir_cmd = format!("mkdir -p {remote_server_dir}");
    let mkdir_output = remote_server::ssh::run_ssh_command(
        socket_path,
        &mkdir_cmd,
        remote_server::setup::CHECK_TIMEOUT,
    )
    .await?;

    if !mkdir_output.status.success() {
        let code = mkdir_output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&mkdir_output.stderr);
        return Err(anyhow!(
            "remote-server dir creation failed with code {code}: {stderr}"
        ));
    }

    let tempdir = tempfile::tempdir()?;
    let tarball_path = tempdir.path().join("openwarp.tar.gz");
    download_remote_server_tarball(&download_url, &tarball_path).await?;

    let remote_tarball_path = format!("{remote_server_dir}/openwarp-upload.tar.gz");
    remote_server::ssh::scp_upload(
        socket_path,
        &tarball_path,
        &remote_tarball_path,
        remote_server::setup::SCP_INSTALL_TIMEOUT,
    )
    .await?;

    run_install_script(
        socket_path,
        Some(&remote_tarball_path),
        remote_server::setup::SCP_INSTALL_TIMEOUT,
    )
    .await
    .map_err(|error| anyhow!("staged install failed: {error}"))?;

    verify_installed_binary(socket_path).await
}

impl RemoteTransport for SshTransport {
    fn detect_platform(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<RemotePlatform, String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            detect_remote_platform(&socket_path)
                .await
                .map_err(|e| format!("{e:#}"))
        })
    }

    fn run_preinstall_check(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<PreinstallCheckResult, String>> + Send>> {
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
                    let code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!(
                        "Preinstall check exited with code {code}: {stderr}"
                    ))
                }
                Err(e) => Err(format!("{e:#}")),
            }
        })
    }

    fn check_binary(&self) -> Pin<Box<dyn Future<Output = Result<bool, String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let bin_path = remote_server::setup::remote_server_binary();
            log::info!("Checking for remote server binary at {bin_path}");
            match remote_server::ssh::run_ssh_command(
                &socket_path,
                &remote_server::setup::binary_check_command(),
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await
            {
                // `{binary} --version` 退出 0 表示存在且可运行。
                // 126/127 表示缺失或不可执行;其他非 0 退出视为真实检查失败。
                Ok(output) => match output.status.code() {
                    Some(0) => Ok(true),
                    Some(126) | Some(127) => Ok(false),
                    Some(code) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Err(format!("binary check exited with code {code}: {stderr}"))
                    }
                    None => Err("binary check terminated by signal".into()),
                },
                Err(e) => Err(format!("{e:#}")),
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

    fn install_binary(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            log::info!(
                "Installing remote server binary to {}",
                remote_server::setup::remote_server_binary()
            );
            match run_install_script(&socket_path, None, remote_server::setup::INSTALL_TIMEOUT)
                .await
            {
                Ok(()) => verify_installed_binary(&socket_path)
                    .await
                    .map_err(|error| format!("{error:#}")),
                Err(error) if should_skip_scp_fallback(&error) => Err(error.to_string()),
                Err(error) => {
                    log::warn!("remote-server install failed, trying SCP fallback: {error}");
                    match scp_install_fallback(&socket_path).await {
                        Ok(()) => Ok(()),
                        Err(fallback_error) => {
                            Err(format!("{error}; SCP fallback failed: {fallback_error:#}"))
                        }
                    }
                }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use warpui::r#async::BoxFuture;
    fn static_auth_context() -> Arc<RemoteServerAuthContext> {
        Arc::new(RemoteServerAuthContext::new(
            || -> BoxFuture<'static, Option<String>> { Box::pin(async { None }) },
            || "user id/with spaces".to_string(),
        ))
    }

    #[test]
    fn remote_proxy_command_quotes_identity_key() {
        let transport = SshTransport::new(
            PathBuf::from("/tmp/control-master.sock"),
            static_auth_context(),
        );

        let command = transport.remote_proxy_command();

        assert!(command.contains("remote-server-proxy --identity-key"));
        assert!(command.contains("'user id/with spaces'"));
    }
}
