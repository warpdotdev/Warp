use super::validate_cli_installed;
use crate::ai::agent_sdk::driver::AgentDriverError;

fn assert_harness_setup_failed(err: &AgentDriverError) -> (&str, &str) {
    match err {
        AgentDriverError::HarnessSetupFailed { harness, reason } => (harness, reason),
        other => panic!("expected HarnessSetupFailed, got: {other}"),
    }
}

#[cfg(not(windows))]
#[test]
fn validate_cli_installed_succeeds_for_known_binary() {
    assert!(validate_cli_installed("ls", None).is_ok());
}

#[test]
fn validate_cli_installed_fails_for_missing_binary() {
    let err = validate_cli_installed("__nonexistent_cli_abc123__", None).unwrap_err();
    let (harness, reason) = assert_harness_setup_failed(&err);
    assert_eq!(harness, "__nonexistent_cli_abc123__");
    assert!(reason.contains("not found"));
    assert!(!reason.contains("Install it first"));
}

#[test]
fn validate_cli_installed_includes_docs_url_in_error() {
    let url = "https://example.com/install";
    let err = validate_cli_installed("__nonexistent_cli_abc123__", Some(url)).unwrap_err();
    let (_, reason) = assert_harness_setup_failed(&err);
    assert!(reason.contains(url));
    assert!(reason.contains("Install it first"));
}

// --- Preflight command tests ---

#[test]
fn claude_returns_auth_check_command() {
    use super::claude_code::ClaudeHarness;
    use super::ThirdPartyHarness;
    let harness = ClaudeHarness;
    let cmd = harness.auth_check_command().expect("should return Some");
    assert!(cmd.contains("auth status --json"));
}

#[test]
fn claude_returns_billing_check_command() {
    use super::claude_code::ClaudeHarness;
    use super::ThirdPartyHarness;
    let harness = ClaudeHarness;
    let cmd = harness.billing_check_command().expect("should return Some");
    assert!(cmd.contains("-p hello"));
}

#[test]
fn codex_returns_auth_check_command() {
    use super::codex::CodexHarness;
    use super::ThirdPartyHarness;
    let harness = CodexHarness;
    let cmd = harness.auth_check_command().expect("should return Some");
    assert!(cmd.contains("login status"));
}

#[test]
fn codex_returns_billing_check_command() {
    use super::codex::CodexHarness;
    use super::ThirdPartyHarness;
    let harness = CodexHarness;
    let cmd = harness.billing_check_command().expect("should return Some");
    assert!(cmd.contains("exec hello"));
}

#[test]
fn gemini_returns_no_preflight_commands() {
    use super::gemini::GeminiHarness;
    use super::ThirdPartyHarness;
    let harness = GeminiHarness;
    assert!(harness.auth_check_command().is_none());
    assert!(harness.billing_check_command().is_none());
}
