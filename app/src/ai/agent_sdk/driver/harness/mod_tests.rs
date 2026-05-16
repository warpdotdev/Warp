use super::{preflight_commands_for, validate_cli_installed};
use crate::ai::agent_sdk::driver::AgentDriverError;
use warp_cli::agent::Harness;

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

#[test]
fn preflight_commands_for_claude_returns_auth_and_billing() {
    use super::claude_code::ClaudeHarness;
    use super::ThirdPartyHarness;
    let commands = preflight_commands_for(Harness::Claude);
    assert_eq!(commands.len(), 2);
    // The helper's output is the single source of truth for the viewer's
    // preflight detection, so pin the strings to the trait impls directly.
    assert_eq!(
        commands[0],
        ClaudeHarness.auth_check_command().expect("auth check")
    );
    assert_eq!(
        commands[1],
        ClaudeHarness
            .billing_check_command()
            .expect("billing check")
    );
}

#[test]
fn preflight_commands_for_codex_returns_auth_and_billing() {
    use super::codex::CodexHarness;
    use super::ThirdPartyHarness;
    let commands = preflight_commands_for(Harness::Codex);
    assert_eq!(commands.len(), 2);
    assert_eq!(
        commands[0],
        CodexHarness.auth_check_command().expect("auth check")
    );
    assert_eq!(
        commands[1],
        CodexHarness.billing_check_command().expect("billing check")
    );
}

#[test]
fn preflight_commands_for_gemini_is_empty() {
    assert!(preflight_commands_for(Harness::Gemini).is_empty());
}

#[test]
fn preflight_commands_for_oz_is_empty() {
    assert!(preflight_commands_for(Harness::Oz).is_empty());
}

#[test]
fn preflight_commands_for_unsupported_is_empty() {
    // OpenCode is mapped to HarnessKind::Unsupported and therefore has no
    // preflight commands of its own.
    assert!(preflight_commands_for(Harness::OpenCode).is_empty());
}

#[test]
fn preflight_commands_for_unknown_is_empty() {
    // Harness::Unknown causes harness_kind to return Err; the helper still
    // returns an empty Vec instead of panicking.
    assert!(preflight_commands_for(Harness::Unknown).is_empty());
}
