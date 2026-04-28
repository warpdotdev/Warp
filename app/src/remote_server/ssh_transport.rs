//! SSH-specific implementation of [`RemoteTransport`].
//!
//! [`SshTransport`] uses an existing SSH ControlMaster socket to check/install
//! the remote server binary and to launch the `remote-server-proxy` process
//! whose stdin/stdout become the protocol channel.
use std::path::PathBuf;

use anyhow::Result;
use warpui::r#async::executor;

use remote_server::client::RemoteServerClient;
use remote_server::setup::RemotePlatform;
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
}

impl SshTransport {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }
}

impl RemoteTransport for SshTransport {
    async fn detect_platform(&self) -> Result<RemotePlatform, String> {
        match remote_server::ssh::run_ssh_command(
            &self.socket_path,
            "uname -sm",
            remote_server::setup::CHECK_TIMEOUT,
        )
        .await
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                remote_server::setup::parse_uname_output(&stdout).map_err(|e| format!("{e:#}"))
            }
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("uname -sm exited with code {code}: {stderr}"))
            }
            Err(e) => Err(format!("{e:#}")),
        }
    }

    async fn check_binary(&self) -> Result<bool, String> {
        let bin_path = remote_server::setup::remote_server_binary();
        log::info!("Checking for remote server binary at {bin_path}");
        match remote_server::ssh::run_ssh_command(
            &self.socket_path,
            &remote_server::setup::binary_check_command(),
            remote_server::setup::CHECK_TIMEOUT,
        )
        .await
        {
            // `test -x` exits 0 when present, 1 when missing.
            // Any other exit code (or None / signal) is treated as a check failure.
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
    }

    async fn install_binary(&self) -> Result<(), String> {
        let script = remote_server::setup::install_script();
        log::info!(
            "Installing remote server binary to {}",
            remote_server::setup::remote_server_binary()
        );
        match remote_server::ssh::run_ssh_script(
            &self.socket_path,
            &script,
            remote_server::setup::INSTALL_TIMEOUT,
        )
        .await
        {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("install script failed (exit {code}): {stderr}"))
            }
            Err(e) => Err(format!("{e:#}")),
        }
    }

    async fn connect(&self, executor: &executor::Background) -> Result<Connection> {
        let binary = remote_server::setup::remote_server_binary();
        let mut args = remote_server::ssh::ssh_args(&self.socket_path);
        args.push(format!("{binary} remote-server-proxy"));

        // `kill_on_drop(true)` pairs with ownership of the `Child` being
        // returned in the [`Connection`] below: the
        // [`RemoteServerManager`] holds the `Child` on its per-session
        // state, and dropping that state (on explicit teardown or
        // spontaneous disconnect) sends SIGKILL to this ssh process.
        // Without this the ssh child is orphaned and keeps a channel
        // open on the ControlMaster socket, blocking the master from
        // exiting cleanly when the user logs out.
        //
        // Note that the child's lifetime is decoupled from any
        // `Arc<RemoteServerClient>` clones: other owners (e.g. the
        // per-session command executor) can keep the client alive for
        // their own purposes without pinning the subprocess.
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
            RemoteServerClient::from_child_streams(stdin, stdout, stderr, executor);
        Ok(Connection {
            client,
            event_rx,
            child,
            control_path: Some(self.socket_path.clone()),
        })
    }
}
