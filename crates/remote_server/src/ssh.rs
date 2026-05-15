use std::path::Path;
use std::process::Output;
use std::time::Duration;

use anyhow::anyhow;
use command::r#async::Command;
use warpui::r#async::FutureExt as _;
use warpui::r#async::Timer;

/// Transport-level error from [`run_ssh_command`] or [`run_ssh_script`].
///
/// Distinguishes timeouts from other I/O failures so callers can promote
/// timeouts to a per-method `TimedOut` variant on the trait error types.
#[derive(Debug, thiserror::Error)]
pub enum SshCommandError {
    /// The SSH command or script did not complete within the timeout.
    #[error("Timed out after {timeout:?}")]
    TimedOut { timeout: Duration },
    /// The `ssh` process could not be spawned.
    #[error("Failed to spawn ssh: {0}")]
    SpawnFailed(std::io::Error),
    /// Writing to the SSH process's stdin failed.
    #[error("Failed to write to ssh stdin: {0}")]
    StdinWriteFailed(std::io::Error),
    /// The SSH process was spawned but `output()` returned an I/O error.
    #[error("SSH I/O error: {0}")]
    IoError(std::io::Error),
}

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
) -> Result<Output, SshCommandError> {
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
    .map_err(|_| SshCommandError::TimedOut { timeout })?
    .map_err(SshCommandError::IoError)
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
pub async fn run_ssh_script(
    socket_path: &Path,
    script: &str,
    timeout: Duration,
) -> Result<Output, SshCommandError> {
    use std::process::Stdio;

    let mut child = Command::new("ssh")
        .args(ssh_args(socket_path))
        .arg("bash -s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(SshCommandError::SpawnFailed)?;

    // Write the script to stdin.
    if let Some(mut stdin) = child.stdin.take() {
        use futures_lite::io::AsyncWriteExt;
        stdin
            .write_all(script.as_bytes())
            .await
            .map_err(SshCommandError::StdinWriteFailed)?;
        // Close stdin so the remote bash exits after reading the script.
        drop(stdin);
    }

    child
        .output()
        .with_timeout(timeout)
        .await
        .map_err(|_| SshCommandError::TimedOut { timeout })?
        .map_err(SshCommandError::IoError)
}

impl From<SshCommandError> for crate::transport::Error {
    fn from(err: SshCommandError) -> Self {
        match err {
            SshCommandError::TimedOut { .. } => Self::TimedOut,
            other => Self::Other(other.into()),
        }
    }
}

/// Maximum number of SCP upload attempts. Only retriable failure kinds
/// (timeouts and transient connection drops) consume retries; deterministic
/// failures fail on the first attempt.
const SCP_UPLOAD_MAX_ATTEMPTS: usize = 3;

/// Delay between SCP upload retries. Short, since retries only cover
/// transient transport hiccups — long sleeps would only delay surfacing a
/// genuine failure.
const SCP_UPLOAD_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Classification of an SCP upload failure. Used to decide whether the
/// caller should retry and to produce actionable error messages that
/// distinguish protocol contamination, deterministic host state, and
/// transient transport blips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScpFailureKind {
    /// scp did not complete within the configured timeout.
    Timeout,
    /// Remote shell startup printed non-protocol output (e.g. a profile
    /// printing a banner), corrupting the SCP1 byte stream. The canonical
    /// signature is OpenSSH's `Received message too long`. Not retriable —
    /// the same shell will print the same banner on the next attempt.
    ProtocolContaminated,
    /// SSH transport dropped mid-upload (broken pipe, connection reset,
    /// lost connection). Retriable for a small number of attempts to cover
    /// transient network blips.
    LostConnection,
    /// SCP authentication or host-key verification failed. Not retriable —
    /// the credentials/known_hosts state is identical on retry.
    AuthFailure,
    /// Remote path is not writable for the SSH user.
    PermissionDenied,
    /// Remote disk full or user quota exhausted.
    NoSpace,
    /// Remote filesystem is read-only.
    ReadOnlyFs,
    /// Remote destination directory does not exist.
    DestinationMissing,
    /// `scp` binary not present on the remote (or PATH issue).
    ScpNotFound,
    /// `scp` could not be spawned locally.
    SpawnFailed,
    /// Anything we did not recognise. Not retried.
    Other,
}

impl ScpFailureKind {
    /// Whether retrying the upload could plausibly succeed without operator
    /// intervention. Mirrors the install-classification invariant: never
    /// retry on permission/quota/read-only/auth/protocol-contamination/
    /// destination-missing — those are deterministic host state.
    pub fn is_retriable(self) -> bool {
        match self {
            Self::Timeout | Self::LostConnection => true,
            Self::ProtocolContaminated
            | Self::AuthFailure
            | Self::PermissionDenied
            | Self::NoSpace
            | Self::ReadOnlyFs
            | Self::DestinationMissing
            | Self::ScpNotFound
            | Self::SpawnFailed
            | Self::Other => false,
        }
    }

    /// Short, human-readable label used in error messages.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::ProtocolContaminated => "protocol contaminated by shell startup output",
            Self::LostConnection => "lost connection",
            Self::AuthFailure => "authentication failed",
            Self::PermissionDenied => "permission denied",
            Self::NoSpace => "no space / quota exhausted",
            Self::ReadOnlyFs => "read-only filesystem",
            Self::DestinationMissing => "destination directory missing",
            Self::ScpNotFound => "scp not found on remote",
            Self::SpawnFailed => "failed to spawn scp",
            Self::Other => "unknown scp failure",
        }
    }
}

/// Classify a finished SCP failure from its captured streams and exit code.
///
/// Pattern-matches the most common OpenSSH/scp stderr signatures observed in
/// the install-failure CSV. The matcher is intentionally substring-based
/// because `scp` emits free-form text, not structured output, and small
/// version-to-version wording changes should not break classification.
///
/// `stdout` is consulted as a fallback when stderr is empty — SCP1 protocol
/// contamination in particular routes the contaminating bytes to stdout.
pub fn classify_scp_failure(stderr: &str, stdout: &str, exit_code: Option<i32>) -> ScpFailureKind {
    let haystack = format!("{stderr}\n{stdout}").to_ascii_lowercase();

    // Shell startup output corrupting the SCP1 protocol stream. Match these
    // signatures first: the same stderr may also mention "permission denied"
    // in the corrupted bytes and we must not misclassify it.
    if haystack.contains("received message too long")
        || haystack.contains("protocol error: bad mode")
        || haystack.contains("protocol error: unexpected")
        || haystack.contains("garbage packet received")
    {
        return ScpFailureKind::ProtocolContaminated;
    }

    // SSH-level auth failures. Check before generic "permission denied" so a
    // pubkey/password failure doesn't get tagged as a remote ACL denial.
    if haystack.contains("permission denied (publickey")
        || haystack.contains("permission denied (password")
        || haystack.contains("permission denied (gssapi")
        || haystack.contains("permission denied, please try again")
        || haystack.contains("host key verification failed")
        || haystack.contains("password expired")
        || haystack.contains("password has expired")
        || haystack.contains("your account has expired")
    {
        return ScpFailureKind::AuthFailure;
    }

    if haystack.contains("read-only file system") {
        return ScpFailureKind::ReadOnlyFs;
    }
    if haystack.contains("no space left on device")
        || haystack.contains("disk quota exceeded")
        || haystack.contains("quota exceeded")
    {
        return ScpFailureKind::NoSpace;
    }
    if haystack.contains("no such file or directory")
        || haystack.contains("not a directory")
        || haystack.contains("failed to open")
        || haystack.contains("scp: dest open")
    {
        return ScpFailureKind::DestinationMissing;
    }
    if haystack.contains("permission denied") {
        return ScpFailureKind::PermissionDenied;
    }

    if haystack.contains("scp: command not found")
        || haystack.contains("scp: not found")
        || haystack.contains("bash: scp:")
    {
        return ScpFailureKind::ScpNotFound;
    }

    if haystack.contains("connection reset by peer")
        || haystack.contains("connection closed by")
        || haystack.contains("broken pipe")
        || haystack.contains("lost connection")
        || haystack.contains("connection timed out")
        || haystack.contains("connection refused")
        || haystack.contains("network is unreachable")
        || haystack.contains("no route to host")
    {
        return ScpFailureKind::LostConnection;
    }

    let _ = exit_code;
    ScpFailureKind::Other
}

/// A classified SCP upload failure carrying the context needed to render a
/// useful error: the kind, the captured streams (with stdout fallback when
/// stderr is empty), the local/remote paths, the exit status, and the
/// timeout under which the attempt ran.
#[derive(Debug)]
pub struct ScpUploadFailure {
    pub kind: ScpFailureKind,
    pub exit_code: Option<i32>,
    pub stderr: String,
    pub stdout: String,
    pub local_path: std::path::PathBuf,
    pub remote_path: String,
    pub timeout: Duration,
}

impl ScpUploadFailure {
    /// Format the failure for inclusion in an `anyhow::Error` or log line.
    /// Always includes the kind label, both paths, the timeout budget, and
    /// the exit status. When stderr is empty, falls back to stdout — empty
    /// stderr is the dominant CSV signature for SCP1 protocol corruption,
    /// where the diagnostic bytes land on stdout instead.
    pub fn render(&self) -> String {
        let exit_display = match self.exit_code {
            Some(code) => format!("{code}"),
            None => "signal/unknown".to_string(),
        };
        let body = if !self.stderr.trim().is_empty() {
            self.stderr.trim().to_string()
        } else if !self.stdout.trim().is_empty() {
            format!("(empty stderr; stdout: {})", self.stdout.trim())
        } else {
            "(no output captured)".to_string()
        };
        format!(
            "scp upload failed [{kind}] (exit {exit_display}, timeout {timeout:?}, \
             local {local}, remote {remote}): {body}",
            kind = self.kind.as_label(),
            timeout = self.timeout,
            local = self.local_path.display(),
            remote = self.remote_path,
        )
    }
}

impl std::fmt::Display for ScpUploadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

impl std::error::Error for ScpUploadFailure {}

/// Upload a local file to the remote host via `scp`, reusing the
/// ControlMaster socket for authentication.
///
/// Retries transient failures (timeouts, lost connections) up to
/// [`SCP_UPLOAD_MAX_ATTEMPTS`]; deterministic failures (auth, permission,
/// no-space, read-only filesystem, destination missing, protocol
/// contamination) fail on the first attempt with a classified, actionable
/// error message that includes the local and remote paths, the exit status,
/// and the timeout budget under which the attempt ran.
pub async fn scp_upload(
    socket_path: &Path,
    local_path: &Path,
    remote_path: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let mut last_failure: Option<ScpUploadFailure> = None;
    for attempt in 1..=SCP_UPLOAD_MAX_ATTEMPTS {
        match scp_upload_once(socket_path, local_path, remote_path, timeout).await {
            Ok(()) => return Ok(()),
            Err(failure) => {
                let kind = failure.kind;
                if kind.is_retriable() && attempt < SCP_UPLOAD_MAX_ATTEMPTS {
                    log::warn!(
                        "scp upload attempt {attempt}/{SCP_UPLOAD_MAX_ATTEMPTS} failed: {failure}; retrying"
                    );
                    last_failure = Some(failure);
                    Timer::after(SCP_UPLOAD_RETRY_DELAY).await;
                    continue;
                }
                return Err(anyhow!(failure.render()));
            }
        }
    }

    // Loop exits only when the final attempt was retriable but exhausted.
    Err(anyhow!(last_failure
        .map(|f| f.render())
        .unwrap_or_else(|| "scp upload failed without a captured error".to_string())))
}

async fn scp_upload_once(
    socket_path: &Path,
    local_path: &Path,
    remote_path: &str,
    timeout: Duration,
) -> Result<(), ScpUploadFailure> {
    let result = async {
        Command::new("scp")
            .arg("-o")
            .arg(format!("ControlPath={}", socket_path.display()))
            .arg("-o")
            .arg("ControlMaster=no")
            .arg("-o")
            .arg("ConnectTimeout=15")
            .arg(local_path.as_os_str())
            .arg(format!("placeholder@placeholder:{remote_path}"))
            .kill_on_drop(true)
            .output()
            .await
    }
    .with_timeout(timeout)
    .await;

    let output = match result {
        Err(_) => {
            return Err(ScpUploadFailure {
                kind: ScpFailureKind::Timeout,
                exit_code: None,
                stderr: String::new(),
                stdout: String::new(),
                local_path: local_path.to_path_buf(),
                remote_path: remote_path.to_string(),
                timeout,
            });
        }
        Ok(Err(e)) => {
            return Err(ScpUploadFailure {
                kind: ScpFailureKind::SpawnFailed,
                exit_code: None,
                stderr: e.to_string(),
                stdout: String::new(),
                local_path: local_path.to_path_buf(),
                remote_path: remote_path.to_string(),
                timeout,
            });
        }
        Ok(Ok(output)) => output,
    };

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let exit_code = output.status.code();
    let kind = classify_scp_failure(&stderr, &stdout, exit_code);
    Err(ScpUploadFailure {
        kind,
        exit_code,
        stderr,
        stdout,
        local_path: local_path.to_path_buf(),
        remote_path: remote_path.to_string(),
        timeout,
    })
}

#[cfg(test)]
#[path = "ssh_tests.rs"]
mod tests;
