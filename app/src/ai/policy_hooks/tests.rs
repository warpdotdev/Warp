use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use serde_json::json;

use super::{
    config::AgentPolicyHookConfig,
    decision::{
        compose_policy_decisions, AgentPolicyDecisionKind, AgentPolicyHookErrorKind,
        AgentPolicyHookEvaluation, AgentPolicyHookResponse, AgentPolicyUnavailableDecision,
        WarpPermissionSnapshot,
    },
    event::{
        AgentPolicyAction, AgentPolicyEvent, PolicyCallMcpToolAction, PolicyExecuteCommandAction,
        PolicyReadFilesAction, AGENT_POLICY_SCHEMA_VERSION,
    },
    redaction::redact_command_for_policy,
};

#[cfg(not(target_family = "wasm"))]
fn existing_secret_env_var() -> (&'static str, String) {
    let name = "PATH";
    let value = std::env::var(name).expect("PATH must be set for policy hook tests");
    assert!(!value.is_empty());
    (name, value)
}

#[cfg(not(target_family = "wasm"))]
use super::audit::audit_record_json_line;
#[cfg(not(target_family = "wasm"))]
use super::engine::AgentPolicyHookEngine;

#[test]
fn config_defaults_to_disabled_and_ask_on_unavailable() {
    let config = AgentPolicyHookConfig::default();

    assert!(!config.enabled);
    assert!(!config.is_active());
    assert_eq!(config.on_unavailable, AgentPolicyUnavailableDecision::Ask);
    assert_eq!(config.timeout_ms, 5_000);
    assert!(config.validate().is_ok());
}

#[test]
fn config_enabled_without_hooks_is_active_but_invalid() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": []
    }))
    .unwrap();

    assert!(config.is_active());
    assert!(config.validate().is_err());
}

#[test]
fn config_deserializes_stdio_hook_shape() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "company-agent-guard",
            "transport": "stdio",
            "command": "company-agent-guard",
            "args": ["warp", "before-action"],
            "timeout_ms": 2500,
            "on_unavailable": "deny"
        }]
    }))
    .unwrap();

    assert!(config.is_active());
    assert_eq!(config.before_action[0].name, "company-agent-guard");
    assert_eq!(config.hook_timeout_ms(&config.before_action[0]), 2_500);
    assert_eq!(
        config.hook_unavailable_decision(&config.before_action[0]),
        AgentPolicyUnavailableDecision::Deny
    );
    assert!(config.validate().is_ok());
}

#[test]
fn config_rejects_non_https_remote_http_hooks() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "remote-guard",
            "transport": "http",
            "url": "http://example.com/policy"
        }]
    }))
    .unwrap();

    assert!(config.validate().is_err());

    let localhost_config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "local-guard",
            "transport": "http",
            "url": "http://localhost:3030/policy"
        }]
    }))
    .unwrap();
    assert!(localhost_config.validate().is_ok());
}

#[test]
fn config_rejects_inline_hook_secret_values() {
    let config = serde_json::from_value::<AgentPolicyHookConfig>(json!({
        "enabled": true,
        "before_action": [
            {
                "name": "stdio-guard",
                "transport": "stdio",
                "command": "guard",
                "env": { "API_TOKEN": "super-secret-token" }
            },
            {
                "name": "http-guard",
                "transport": "http",
                "url": "https://example.com/policy",
                "headers": { "authorization": "Bearer super-secret-token" }
            }
        ]
    }));

    assert!(config.is_err());
}

#[test]
fn config_serialization_preserves_secret_environment_references() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [
            {
                "name": "stdio-guard",
                "transport": "stdio",
                "command": "guard",
                "env": { "API_TOKEN": { "env": "WARP_POLICY_HOOK_TOKEN" } }
            },
            {
                "name": "http-guard",
                "transport": "http",
                "url": "https://example.com/policy",
                "headers": { "authorization": { "env": "WARP_POLICY_HOOK_AUTH_HEADER" } }
            }
        ]
    }))
    .unwrap();

    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(
        value["before_action"][0]["env"]["API_TOKEN"]["env"],
        "WARP_POLICY_HOOK_TOKEN"
    );
    assert_eq!(
        value["before_action"][1]["headers"]["authorization"]["env"],
        "WARP_POLICY_HOOK_AUTH_HEADER"
    );

    let round_trip: AgentPolicyHookConfig = serde_json::from_value(value).unwrap();
    assert_eq!(round_trip, config);
}

#[test]
fn event_serializes_redacted_command_shape() {
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        Some(PathBuf::from("/repo")),
        true,
        Some("profile_default".to_string()),
        WarpPermissionSnapshot::allow(Some("RunToCompletion".to_string())),
        PolicyExecuteCommandAction::new(
            "OPENAI_API_KEY=sk-secretsecretsecret curl -H 'Authorization: Bearer token123' https://example.com",
            "OPENAI_API_KEY=sk-secretsecretsecret curl https://example.com",
            Some(false),
            Some(true),
        ),
    );

    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["schema_version"], AGENT_POLICY_SCHEMA_VERSION);
    assert_eq!(value["action_kind"], "execute_command");
    assert_eq!(value["run_until_completion"], true);
    assert_eq!(value["warp_permission"]["decision"], "allow");

    let command = value["action"]["command"].as_str().unwrap();
    assert!(command.contains("OPENAI_API_KEY=<redacted>"));
    assert!(command.contains("Authorization: Bearer <redacted>"));
    assert!(!command.contains("sk-secretsecretsecret"));
    assert_eq!(value["action"]["is_risky"], true);
}

#[test]
fn command_redaction_handles_quoted_secret_assignments() {
    let command = concat!(
        "OPENAI_API_KEY=\"sk-secret value\" ",
        "GITHUB_TOKEN='ghp_secret value' ",
        "ACCESS_KEY=\"escaped \\\" secret\" curl https://example.com",
    );
    let unterminated = "PASSWORD=\"unterminated secret curl https://example.com";

    let redacted = redact_command_for_policy(command);
    let redacted_unterminated = redact_command_for_policy(unterminated);

    assert!(redacted.contains("OPENAI_API_KEY=<redacted>"));
    assert!(redacted.contains("GITHUB_TOKEN=<redacted>"));
    assert!(redacted.contains("ACCESS_KEY=<redacted>"));
    assert!(!redacted.contains("sk-secret"));
    assert!(!redacted.contains("ghp_secret"));
    assert!(!redacted.contains("escaped"));
    assert!(redacted_unterminated.contains("PASSWORD=<redacted>"));
    assert!(!redacted_unterminated.contains("unterminated"));
}

#[test]
fn mcp_tool_action_preserves_only_argument_keys() {
    let action = PolicyCallMcpToolAction::new(
        None,
        "dangerous_tool",
        &json!({
            "token": "secret",
            "path": "/repo",
            "count": 3
        }),
    );

    assert_eq!(action.argument_keys, vec!["count", "path", "token"]);
}

#[test]
fn policy_decision_composition_is_conservative() {
    let hook_allow = AgentPolicyHookEvaluation {
        hook_name: "guard".to_string(),
        decision: AgentPolicyDecisionKind::Allow,
        reason: Some("trusted".to_string()),
        external_audit_id: None,
        error: None,
    };

    let needs_confirmation = compose_policy_decisions(
        WarpPermissionSnapshot::ask(Some("AlwaysAsk".to_string())),
        vec![hook_allow.clone()],
        false,
    );
    assert_eq!(needs_confirmation.decision, AgentPolicyDecisionKind::Ask);
    assert_eq!(needs_confirmation.reason.as_deref(), Some("AlwaysAsk"));

    let autoapproved = compose_policy_decisions(
        WarpPermissionSnapshot::ask(Some("AlwaysAsk".to_string())),
        vec![hook_allow],
        true,
    );
    assert_eq!(autoapproved.decision, AgentPolicyDecisionKind::Allow);
    assert_eq!(autoapproved.reason.as_deref(), Some("trusted"));
}

#[test]
fn policy_decision_composition_keeps_denials_terminal() {
    let hook_deny = AgentPolicyHookEvaluation {
        hook_name: "guard".to_string(),
        decision: AgentPolicyDecisionKind::Deny,
        reason: Some("blocked".to_string()),
        external_audit_id: Some("audit_1".to_string()),
        error: None,
    };

    let denied_by_hook =
        compose_policy_decisions(WarpPermissionSnapshot::allow(None), vec![hook_deny], false);
    assert_eq!(denied_by_hook.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(denied_by_hook.reason.as_deref(), Some("blocked"));

    let warp_denied = compose_policy_decisions(
        WarpPermissionSnapshot::deny(Some("protected path".to_string())),
        vec![AgentPolicyHookEvaluation {
            hook_name: "guard".to_string(),
            decision: AgentPolicyDecisionKind::Allow,
            reason: Some("external allow".to_string()),
            external_audit_id: None,
            error: None,
        }],
        true,
    );
    assert_eq!(warp_denied.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(warp_denied.reason.as_deref(), Some("protected path"));
}

#[test]
fn hook_response_strings_are_redacted_and_capped() {
    let evaluation = AgentPolicyHookEvaluation::from_response(
        "guard",
        AgentPolicyHookResponse {
            schema_version: AGENT_POLICY_SCHEMA_VERSION.to_string(),
            decision: AgentPolicyDecisionKind::Deny,
            reason: Some(format!(
                "OPENAI_API_KEY=sk-secretsecretsecret {}",
                "x".repeat(10_000)
            )),
            external_audit_id: Some("audit-ghp_secretsecretsecret".to_string()),
        },
    );

    let reason = evaluation.reason.as_deref().unwrap();
    assert!(reason.contains("OPENAI_API_KEY=<redacted>"));
    assert!(!reason.contains("sk-secretsecretsecret"));
    assert!(reason.len() < 8_300);
    assert_eq!(
        evaluation.external_audit_id.as_deref(),
        Some("audit-<redacted>")
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn audit_record_uses_redacted_policy_event_payload() {
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        Some(PathBuf::from("/repo")),
        false,
        Some("profile_default".to_string()),
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new(
            "GITHUB_TOKEN=ghp_secretsecretsecret curl -H 'Authorization: Bearer token123' https://example.com",
            "GITHUB_TOKEN=ghp_secretsecretsecret curl https://example.com",
            Some(false),
            Some(true),
        ),
    );
    let decision = compose_policy_decisions(
        WarpPermissionSnapshot::allow(None),
        vec![AgentPolicyHookEvaluation {
            hook_name: "guard".to_string(),
            decision: AgentPolicyDecisionKind::Deny,
            reason: Some("blocked".to_string()),
            external_audit_id: Some("audit_1".to_string()),
            error: None,
        }],
        false,
    );

    let line = audit_record_json_line(&event, &decision).unwrap();
    let value: serde_json::Value = serde_json::from_str(&line).unwrap();

    assert_eq!(value["action_kind"], "execute_command");
    assert_eq!(value["effective_decision"]["decision"], "deny");
    assert_eq!(value["redaction"]["command_secrets_redacted"], true);
    assert!(line.contains("GITHUB_TOKEN=<redacted>"));
    assert!(line.contains("Authorization: Bearer <redacted>"));
    assert!(!line.contains("ghp_secretsecretsecret"));
    assert!(!line.contains("token123"));
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_can_deny_before_action() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "local-guard",
            "transport": "stdio",
            "command": "sh",
            "args": [
                "-c",
                "cat >/dev/null; printf '%s\\n' '{\"schema_version\":\"warp.agent_policy_hook.v1\",\"decision\":\"deny\",\"reason\":\"blocked by test\",\"external_audit_id\":\"audit_789\"}'"
            ],
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("rm -rf .", "rm -rf .", Some(false), Some(true)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(decision.reason.as_deref(), Some("blocked by test"));
    assert_eq!(decision.hook_results[0].hook_name, "local-guard");
    assert_eq!(
        decision.hook_results[0].external_audit_id.as_deref(),
        Some("audit_789")
    );
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_maps_malformed_response_to_unavailable_policy() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "bad-guard",
            "transport": "stdio",
            "command": "sh",
            "args": ["-c", "cat >/dev/null; printf nope"],
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::MalformedResponse)
    );
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_rejects_oversized_stdout() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "noisy-guard",
            "transport": "stdio",
            "command": "sh",
            "args": ["-c", "cat >/dev/null; dd if=/dev/zero bs=70000 count=1 2>/dev/null"],
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::MalformedResponse)
    );
    assert!(decision.hook_results[0]
        .reason
        .as_deref()
        .unwrap()
        .contains("stdout exceeded"));
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_times_out_blocked_stdin_write() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "blocked-stdin-guard",
            "transport": "stdio",
            "command": "sleep",
            "args": ["5"],
            "timeout_ms": 100
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let suffix = "x".repeat(160);
    let paths = (0..15_000)
        .map(|index| PathBuf::from(format!("/tmp/policy-hook-large-event-{index}-{suffix}")))
        .collect();
    let event = AgentPolicyEvent::new(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        AgentPolicyAction::ReadFiles(PolicyReadFilesAction { paths }),
    );

    let started = Instant::now();
    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::Timeout)
    );
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_redacts_configured_secret_stderr() {
    let (secret_env, secret_value) = existing_secret_env_var();
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "failing-guard",
            "transport": "stdio",
            "command": "sh",
            "args": ["-c", "cat >/dev/null; printf '%s\\n' \"$API_TOKEN\" >&2; exit 42"],
            "env": { "API_TOKEN": { "env": secret_env } },
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    let reason = decision.hook_results[0].reason.as_deref().unwrap();
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::NonZeroExit)
    );
    assert!(reason.contains("<redacted>"));
    assert!(!reason.contains(&secret_value));
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_redacts_configured_secret_hook_reason() {
    let (secret_env, secret_value) = existing_secret_env_var();
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "secret-reason-guard",
            "transport": "stdio",
            "command": "sh",
            "args": [
                "-c",
                "cat >/dev/null; printf '{\"schema_version\":\"warp.agent_policy_hook.v1\",\"decision\":\"deny\",\"reason\":\"token: %s\",\"external_audit_id\":\"audit-%s\"}\\n' \"$API_TOKEN\" \"$API_TOKEN\""
            ],
            "env": { "API_TOKEN": { "env": secret_env } },
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    let reason = decision.hook_results[0].reason.as_deref().unwrap();
    assert!(reason.contains("<redacted>"));
    assert!(!reason.contains(&secret_value));
    assert_eq!(
        decision.hook_results[0].external_audit_id.as_deref(),
        Some("audit-<redacted>")
    );
}

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_can_deny_before_action() {
    let mut server = mockito::Server::new_async().await;
    let hook_response = json!({
        "schema_version": AGENT_POLICY_SCHEMA_VERSION,
        "decision": "deny",
        "reason": "blocked by HTTP test",
        "external_audit_id": "audit_http_1"
    })
    .to_string();
    let mock = server
        .mock("POST", "/policy")
        .match_header("content-type", "application/json")
        .match_header("x-warp-agent-policy-event-id", mockito::Matcher::Any)
        .with_status(200)
        .with_body(hook_response)
        .create_async()
        .await;
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "http-guard",
            "transport": "http",
            "url": format!("{}/policy", server.url()),
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("rm -rf .", "rm -rf .", Some(false), Some(true)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    mock.assert_async().await;
    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(decision.reason.as_deref(), Some("blocked by HTTP test"));
    assert_eq!(decision.hook_results[0].hook_name, "http-guard");
    assert_eq!(
        decision.hook_results[0].external_audit_id.as_deref(),
        Some("audit_http_1")
    );
}

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_rejects_oversized_response_body() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/policy")
        .with_status(200)
        .with_body(vec![b'x'; 70_000])
        .create_async()
        .await;
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "http-guard",
            "transport": "http",
            "url": format!("{}/policy", server.url()),
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("rm -rf .", "rm -rf .", Some(false), Some(true)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    mock.assert_async().await;
    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::MalformedResponse)
    );
    assert!(decision.hook_results[0]
        .reason
        .as_deref()
        .unwrap()
        .contains("response exceeded"));
}

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_uses_single_timeout_for_request_and_response_body() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/policy", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut request = [0_u8; 2048];
        let _ = socket.read(&mut request).await;

        tokio::time::sleep(Duration::from_millis(80)).await;
        let body = br#"{"schema_version":"warp.agent_policy_hook.v1","decision":"allow"}"#;
        let headers = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
            body.len()
        );
        let _ = socket.write_all(headers.as_bytes()).await;
        let _ = socket.flush().await;

        tokio::time::sleep(Duration::from_millis(80)).await;
        let _ = socket.write_all(body).await;
    });

    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "slow-http-guard",
            "transport": "http",
            "url": url,
            "timeout_ms": 120
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("rm -rf .", "rm -rf .", Some(false), Some(true)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::Timeout)
    );
}

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_does_not_follow_redirects() {
    let mut server = mockito::Server::new_async().await;
    let (secret_env, _) = existing_secret_env_var();
    let redirect_location = format!("{}/redirected", server.url());
    let mock = server
        .mock("POST", "/policy")
        .with_status(307)
        .with_header("location", redirect_location.as_str())
        .create_async()
        .await;
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "http-guard",
            "transport": "http",
            "url": format!("{}/policy", server.url()),
            "headers": { "authorization": { "env": secret_env } },
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("rm -rf .", "rm -rf .", Some(false), Some(true)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    mock.assert_async().await;
    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::HttpStatus)
    );
    assert!(decision.hook_results[0]
        .reason
        .as_deref()
        .unwrap()
        .contains("307"));
}

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_redacts_configured_header_secret_hook_reason() {
    let mut server = mockito::Server::new_async().await;
    let (secret_env, secret_value) = existing_secret_env_var();
    let hook_response = json!({
        "schema_version": AGENT_POLICY_SCHEMA_VERSION,
        "decision": "deny",
        "reason": format!("raw token {secret_value}"),
        "external_audit_id": format!("audit-{secret_value}")
    })
    .to_string();
    let mock = server
        .mock("POST", "/policy")
        .match_header("authorization", secret_value.as_str())
        .with_status(200)
        .with_body(hook_response)
        .create_async()
        .await;
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "http-guard",
            "transport": "http",
            "url": format!("{}/policy", server.url()),
            "headers": { "authorization": { "env": secret_env } },
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("rm -rf .", "rm -rf .", Some(false), Some(true)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    mock.assert_async().await;
    let reason = decision.hook_results[0].reason.as_deref().unwrap();
    assert!(reason.contains("<redacted>"));
    assert!(!reason.contains(&secret_value));
    assert_eq!(
        decision.hook_results[0].external_audit_id.as_deref(),
        Some("audit-<redacted>")
    );
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn engine_maps_enabled_empty_config_to_unavailable_policy() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": []
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::InvalidConfiguration)
    );
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn engine_maps_invalid_enabled_config_to_unavailable_policy() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "missing-command",
            "transport": "stdio",
            "command": ""
        }]
    }))
    .unwrap();
    let engine = AgentPolicyHookEngine::new(config);
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::InvalidConfiguration)
    );
}
