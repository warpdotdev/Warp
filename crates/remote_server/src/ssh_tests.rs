use std::path::PathBuf;
use std::time::Duration;

use super::{classify_scp_failure, ScpFailureKind, ScpUploadFailure};

fn make_failure(kind: ScpFailureKind, stderr: &str, stdout: &str) -> ScpUploadFailure {
    ScpUploadFailure {
        kind,
        exit_code: Some(1),
        stderr: stderr.to_string(),
        stdout: stdout.to_string(),
        local_path: PathBuf::from("/tmp/oz-tarball.tar.gz"),
        remote_path: "/home/user/.oz/oz-upload.tar.gz".to_string(),
        timeout: Duration::from_secs(240),
    }
}

#[test]
fn classify_protocol_contamination_received_message_too_long() {
    let kind = classify_scp_failure("Received message too long 1936287860\n", "", Some(1));
    assert_eq!(kind, ScpFailureKind::ProtocolContaminated);
}

#[test]
fn classify_protocol_contamination_unexpected_protocol_error() {
    let kind = classify_scp_failure("protocol error: bad mode\n", "", Some(1));
    assert_eq!(kind, ScpFailureKind::ProtocolContaminated);
}

#[test]
fn classify_protocol_contamination_garbage_packet() {
    let kind = classify_scp_failure("Garbage packet received for SSH protocol\n", "", Some(1));
    assert_eq!(kind, ScpFailureKind::ProtocolContaminated);
}

#[test]
fn classify_auth_failure_pubkey() {
    let kind = classify_scp_failure(
        "Permission denied (publickey,password).\n",
        "",
        Some(255),
    );
    assert_eq!(kind, ScpFailureKind::AuthFailure);
}

#[test]
fn classify_auth_failure_host_key() {
    let kind = classify_scp_failure("Host key verification failed.\n", "", Some(255));
    assert_eq!(kind, ScpFailureKind::AuthFailure);
}

#[test]
fn classify_auth_failure_password_expired() {
    let kind = classify_scp_failure("Your password has expired\n", "", Some(1));
    assert_eq!(kind, ScpFailureKind::AuthFailure);
}

#[test]
fn classify_permission_denied_distinct_from_auth() {
    let kind = classify_scp_failure(
        "scp: /opt/oz/oz.tar.gz: Permission denied\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::PermissionDenied);
}

#[test]
fn classify_no_space() {
    let kind = classify_scp_failure(
        "scp: /opt/oz/oz.tar.gz: No space left on device\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::NoSpace);
}

#[test]
fn classify_disk_quota_exceeded() {
    let kind = classify_scp_failure(
        "scp: /home/user/oz.tar.gz: Disk quota exceeded\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::NoSpace);
}

#[test]
fn classify_read_only_fs() {
    let kind = classify_scp_failure(
        "scp: /opt/oz/oz.tar.gz: Read-only file system\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::ReadOnlyFs);
}

#[test]
fn classify_destination_missing() {
    let kind = classify_scp_failure(
        "scp: /missing/dir/oz.tar.gz: No such file or directory\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::DestinationMissing);
}

#[test]
fn classify_destination_not_a_directory() {
    let kind = classify_scp_failure(
        "scp: /opt/oz: Not a directory\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::DestinationMissing);
}

#[test]
fn classify_lost_connection_reset() {
    let kind = classify_scp_failure(
        "scp: Connection reset by peer\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::LostConnection);
}

#[test]
fn classify_lost_connection_broken_pipe() {
    let kind = classify_scp_failure("scp: write: Broken pipe\n", "", Some(1));
    assert_eq!(kind, ScpFailureKind::LostConnection);
}

#[test]
fn classify_lost_connection_closed_by_remote() {
    let kind = classify_scp_failure(
        "Connection closed by 10.0.0.1 port 22\nlost connection\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::LostConnection);
}

#[test]
fn classify_scp_not_found() {
    let kind = classify_scp_failure("bash: scp: command not found\n", "", Some(127));
    assert_eq!(kind, ScpFailureKind::ScpNotFound);
}

#[test]
fn classify_unknown_falls_back_to_other() {
    let kind = classify_scp_failure("totally unexpected error wording\n", "", Some(42));
    assert_eq!(kind, ScpFailureKind::Other);
}

#[test]
fn classify_protocol_signal_takes_precedence_over_permission_denied() {
    // The contaminating shell output may legitimately contain `permission
    // denied` text from an unrelated profile script. The protocol signature
    // must dominate so we report the actionable cause.
    let kind = classify_scp_failure(
        "Received message too long 1701209960\n... Permission denied ...\n",
        "",
        Some(1),
    );
    assert_eq!(kind, ScpFailureKind::ProtocolContaminated);
}

#[test]
fn classify_uses_stdout_when_stderr_empty() {
    let kind = classify_scp_failure("", "Received message too long 12345\n", Some(1));
    assert_eq!(kind, ScpFailureKind::ProtocolContaminated);
}

#[test]
fn retriable_only_for_transient_failure_kinds() {
    // Retriable (transient transport/network).
    assert!(ScpFailureKind::Timeout.is_retriable());
    assert!(ScpFailureKind::LostConnection.is_retriable());

    // Non-retriable (deterministic host state / config).
    assert!(!ScpFailureKind::ProtocolContaminated.is_retriable());
    assert!(!ScpFailureKind::AuthFailure.is_retriable());
    assert!(!ScpFailureKind::PermissionDenied.is_retriable());
    assert!(!ScpFailureKind::NoSpace.is_retriable());
    assert!(!ScpFailureKind::ReadOnlyFs.is_retriable());
    assert!(!ScpFailureKind::DestinationMissing.is_retriable());
    assert!(!ScpFailureKind::ScpNotFound.is_retriable());
    assert!(!ScpFailureKind::SpawnFailed.is_retriable());
    assert!(!ScpFailureKind::Other.is_retriable());
}

#[test]
fn render_includes_kind_paths_timeout_and_stderr() {
    let failure = make_failure(
        ScpFailureKind::PermissionDenied,
        "scp: /opt/oz: Permission denied",
        "",
    );
    let rendered = failure.render();
    assert!(rendered.contains("permission denied"));
    assert!(rendered.contains("/tmp/oz-tarball.tar.gz"));
    assert!(rendered.contains("/home/user/.oz/oz-upload.tar.gz"));
    assert!(rendered.contains("exit 1"));
    assert!(rendered.contains("scp: /opt/oz: Permission denied"));
    assert!(rendered.contains("240"));
}

#[test]
fn render_falls_back_to_stdout_when_stderr_empty() {
    let failure = make_failure(
        ScpFailureKind::ProtocolContaminated,
        "",
        "warning: profile printed banner",
    );
    let rendered = failure.render();
    assert!(rendered.contains("empty stderr"));
    assert!(rendered.contains("warning: profile printed banner"));
}

#[test]
fn render_reports_no_output_when_both_streams_empty() {
    let failure = ScpUploadFailure {
        kind: ScpFailureKind::Timeout,
        exit_code: None,
        stderr: String::new(),
        stdout: String::new(),
        local_path: PathBuf::from("/tmp/x"),
        remote_path: "/dest".to_string(),
        timeout: Duration::from_secs(10),
    };
    let rendered = failure.render();
    assert!(rendered.contains("timeout"));
    assert!(rendered.contains("(no output captured)"));
    assert!(rendered.contains("signal/unknown"));
}
