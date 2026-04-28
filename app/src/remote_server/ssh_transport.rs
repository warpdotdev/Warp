//! SSH-specific implementation of [`RemoteTransport`].
//!
//! [`SshTransport`] uses an existing SSH ControlMaster socket to check/install
//! the remote server binary and to launch the `remote-server-proxy` process
//! whose stdin/stdout become the protocol channel.
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::Result;
use warpui::r#async::executor;

use remote_server::client::RemoteServerClient;
use remote_server::setup::{self, RemotePlatform, CHECK_TIMEOUT, INSTALL_TIMEOUT};
use remote_server::ssh::{run_ssh_command, run_ssh_script, ssh_args};
use remote_server::transport::{Connection, RemoteTransport};

/// SSH transport: connects via a ControlMaster socket.
///
/// `socket_path` is the local Unix socket created by the ControlMaster
/// process (`ssh -N -o ControlMaster=yes -o ControlPath=<path>`). All SSH
/// commands (binary check, install, proxy launch) are multiplexed through
/// this socket without re-authenticating.
#[derive(Clone, Debug)]
pub struct SshTransport {
    socket_path: PathBuf,
}

impl SshTransport {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }
}
impl RemoteTransport for SshTransport {
    fn detect_platform(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<RemotePlatform, String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            match run_ssh_command(&socket_path, "uname -sm", CHECK_TIMEOUT).await {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    setup::parse_uname_output(&stdout).map_err(|e| format!("{e:#}"))
                }
                Ok(output) => {
                    let code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("uname -sm exited with code {code}: {stderr}"))
                }
                Err(e) => Err(format!("{e:#}")),
            }
        })
    }

    fn check_binary(&self) -> Pin<Box<dyn Future<Output = Result<bool, String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let bin_path = setup::remote_server_binary();
            log::info!("Checking for remote server binary at {bin_path}");
            match run_ssh_command(&socket_path, &setup::binary_check_command(), CHECK_TIMEOUT).await
            {
                Ok(output) => match output.status.code() {
                    Some(0) => Ok(true),
                    Some(1) => Ok(false),
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

    fn install_binary(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let script = setup::install_script();
            log::info!(
                "Installing remote server binary to {}",
                setup::remote_server_binary()
            );
            match run_ssh_script(&socket_path, &script, INSTALL_TIMEOUT).await {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => {
                    let code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("install script failed (exit {code}): {stderr}"))
                }
                Err(e) => Err(format!("{e:#}")),
            }
        })
    }

    fn connect(
        &self,
        executor: Arc<executor::Background>,
    ) -> Pin<Box<dyn Future<Output = Result<Connection>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let binary = setup::remote_server_binary();
            let mut args = ssh_args(&socket_path);
            args.push(format!("{binary} remote-server-proxy"));

            // `kill_on_drop(true)` pairs with ownership of the `Child` being
            // returned in the [`Connection`] below: the
            // [`RemoteServerManager`] holds the `Child` on its per-session
            // state, and dropping that state (on explicit teardown or
            // spontaneous disconnect) sends SIGKILL to this ssh process.
            let mut child = command::r#async::Command::new("ssh")
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
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
}
