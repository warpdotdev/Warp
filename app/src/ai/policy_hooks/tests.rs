use std::path::PathBuf;

use serde_json::json;

use super::{
    config::AgentPolicyHookConfig,
    decision::{
        compose_policy_decisions, AgentPolicyDecisionKind, AgentPolicyHookErrorKind,
        AgentPolicyHookEvaluation, AgentPolicyUnavailableDecision, WarpPermissionSnapshot,
    },
    event::{
        AgentPolicyEvent, PolicyCallMcpToolAction, PolicyExecuteCommandAction,
        AGENT_POLICY_SCHEMA_VERSION,
    },
};

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
