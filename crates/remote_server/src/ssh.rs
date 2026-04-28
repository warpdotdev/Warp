use std::path::Path;
use std::process::Output;
use std::time::Duration;

use anyhow::{anyhow, Result};
use command::r#async::Command;
use warpui::r#async::FutureExt as _;

/// Timeout for `ssh -O exit`. The command only talks to the local
/// ControlMaster over a Unix socket, so it should return almost
/// immediately; if it doesn't, we'd rather give up than block
/// teardown.
const STOP_CONTROL_MASTER_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds the common SSH argument list for multiplexed connections through
/// an existing ControlMaster socket.
pub fn ssh_args(socket_path: &Path) -> Vec<String> {
    vec![
        "-q".to_string(),
        "-o".to_string(),
        "PasswordAuthentication=no".to_string(),
        "-o".to_string(),
        "ForwardX11=no".to_string(),
        "-o".to_string(),
        format!("ControlPath={}", socket_path.display()),
        "placeholder@placeholder".to_string(),
    ]
}

/// Runs `ssh -O exit -o ControlPath=<socket_path>` to force the local
/// SSH `ControlMaster` managing `socket_path` to exit immediately,
/// without waiting for multiplexed channels to finish draining.
///
/// The user's interactive ssh is spawned with `-o ControlMaster=yes` by
/// `warp_ssh_helper`, so it is both the interactive session and the
/// multiplex master. When the user's remote shell exits, that ssh can
/// hang waiting for half-closed slave channels (e.g. from
/// `ssh ... remote-server-proxy`) to finish cleanup on the remote
/// side. Sending `-O exit` bypasses that wait.
///
/// **Only safe to call once the user's shell has already exited** --
/// this tears down the interactive ssh outright. In practice it is
/// invoked from the `ExitShell` teardown path on the client.
///
/// Fire-and-forget. Errors are logged but not propagated: at teardown
/// time there is nothing useful to do with them.
pub async fn stop_control_master(socket_path: &Path) {
    let args = ssh_args(socket_path);
    let result = async {
        Command::new("ssh")
            .arg("-O")
            .arg("exit")
            .args(&args)
            .kill_on_drop(true)
            .output()
            .await
    }
    .with_timeout(STOP_CONTROL_MASTER_TIMEOUT)
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            log::info!(
                "stop_control_master: `ssh -O exit` succeeded for {}",
                socket_path.display()
            );
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::info!(
                "stop_control_master: `ssh -O exit` for {} exited with {:?}: {stderr}",
                socket_path.display(),
                output.status.code(),
            );
        }
        Ok(Err(e)) => {
            log::info!(
                "stop_control_master: failed to spawn `ssh -O exit` for {}: {e}",
                socket_path.display()
            );
        }
        Err(_) => {
            log::warn!(
                "stop_control_master: `ssh -O exit` for {} timed out after {:?}",
                socket_path.display(),
                STOP_CONTROL_MASTER_TIMEOUT,
            );
        }
    }
}

/// Run a single SSH command through the ControlMaster socket and return a result where:
/// - `Err` for transport-level failures (e.g. couldn't spawn `ssh`, or timeout).
/// - `Ok(output)` callers should check `output.status` to distinguish a successful remote command from a non-zero remote exit.
pub async fn run_ssh_command(
    socket_path: &Path,
    remote_command: &str,
    timeout: Duration,
) -> Result<Output> {
    async {
        Command::new("ssh")
            .args(ssh_args(socket_path))
            .arg(remote_command)
            .kill_on_drop(true)
            .output()
            .await
    }
    .with_timeout(timeout)
    .await
    .map_err(|_| anyhow!("SSH command timed out after {timeout:?}"))?
    .map_err(|e| anyhow!("SSH command failed to execute: {e}"))
}

/// Pipe a script into `bash -s` on the remote host via the ControlMaster
/// socket. Returns a result where:
/// - `Err` for transport-level failures (e.g. couldn't spawn `ssh`, or timeout).
/// - `Ok(output)` callers should check `output.status` to distinguish a successful remote script from a non-zero remote exit.
///
/// We pipe via stdin rather than passing the script as an SSH command-line
/// argument because the install script is multi-line and contains shell
/// constructs (case statements, variable expansions, single/double quotes)
/// that would require complex, fragile escaping if passed as an argument.
/// The `bash -s` + stdin approach avoids all escaping issues and has no
/// argument length limits.
pub async fn run_ssh_script(socket_path: &Path, script: &str, timeout: Duration) -> Result<Output> {
    use std::process::Stdio;

    let mut child = Command::new("ssh")
        .args(ssh_args(socket_path))
        .arg("bash -s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn SSH for script: {e}"))?;

    // Write the script to stdin.
    if let Some(mut stdin) = child.stdin.take() {
        use futures_lite::io::AsyncWriteExt;
        stdin
            .write_all(script.as_bytes())
            .await
            .map_err(|e| anyhow!("Failed to write script to stdin: {e}"))?;
        // Close stdin so the remote bash exits after reading the script.
        drop(stdin);
    }

    child
        .output()
        .with_timeout(timeout)
        .await
        .map_err(|_| anyhow!("Script timed out after {timeout:?}"))?
        .map_err(|e| anyhow!("Script failed: {e}"))
}
