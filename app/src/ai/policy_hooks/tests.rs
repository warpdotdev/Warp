use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::ai::execution_profiles::AIExecutionProfile;
use serde_json::json;

use super::{
    config::{AgentPolicyHook, AgentPolicyHookConfig, AgentPolicyHookTransport},
    decision::{
        compose_policy_decisions, AgentPolicyDecisionKind, AgentPolicyHookErrorKind,
        AgentPolicyHookEvaluation, AgentPolicyHookResponse, AgentPolicyUnavailableDecision,
        WarpPermissionSnapshot,
    },
    event::{
        AgentPolicyAction, AgentPolicyEvent, PolicyCallMcpToolAction, PolicyExecuteCommandAction,
        PolicyReadFilesAction, PolicyReadMcpResourceAction, PolicyWriteFilesAction,
        AGENT_POLICY_SCHEMA_VERSION,
    },
    redaction::{redact_command_for_policy, MAX_POLICY_COLLECTION_ITEMS},
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
fn config_empty_hook_list_is_not_autoapproval_capable() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "allow_hook_autoapproval": true,
        "before_action": []
    }))
    .unwrap();

    assert!(!config.allow_autoapproval_for_all_hooks());
}

#[test]
fn config_nonempty_hook_list_can_be_autoapproval_capable() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "allow_hook_autoapproval": true,
        "before_action": [{
            "name": "company-agent-guard",
            "transport": "stdio",
            "command": "company-agent-guard",
            "allow_autoapproval": true
        }]
    }))
    .unwrap();

    assert!(config.allow_autoapproval_for_all_hooks());
}

#[test]
fn config_per_hook_autoapproval_does_not_bypass_global_opt_in() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "company-agent-guard",
            "transport": "stdio",
            "command": "company-agent-guard",
            "allow_autoapproval": true
        }]
    }))
    .unwrap();

    assert!(!config.allow_autoapproval_for_all_hooks());
}

#[test]
fn config_global_autoapproval_does_not_bypass_per_hook_opt_in() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "allow_hook_autoapproval": true,
        "before_action": [{
            "name": "company-agent-guard",
            "transport": "stdio",
            "command": "company-agent-guard",
            "allow_autoapproval": false
        }]
    }))
    .unwrap();

    assert!(!config.allow_autoapproval_for_all_hooks());
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
fn config_global_unavailable_deny_cannot_be_relaxed_by_hook_allow() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "company-agent-guard",
            "transport": "stdio",
            "command": "company-agent-guard",
            "on_unavailable": "allow"
        }]
    }))
    .unwrap();

    assert_eq!(
        config.hook_unavailable_decision(&config.before_action[0]),
        AgentPolicyUnavailableDecision::Deny
    );
}

#[test]
fn config_rejects_stdio_hook_credential_args() {
    for args in [
        json!(["--token=secret"]),
        json!(["--token", "secret"]),
        json!(["--token", "%s"]),
        json!(["--token", "%raw-secret"]),
        json!(["--token", "prefix$API_TOKEN"]),
        json!(["--api-key", "secret"]),
        json!(["--client-secret", "secret"]),
        json!(["--refresh-token", "secret"]),
        json!(["--accessToken", "secret"]),
        json!(["--authorization", "Bearer secret"]),
        json!(["--authorization", "Bearer token$with-dollar"]),
        json!(["API_KEY=secret"]),
        json!(["clientSecret=secret"]),
        json!(["API_KEY=secret$with-dollar"]),
        json!(["X-API-Key:", "secret"]),
        json!(["Authorization: Bearer secret"]),
        json!(["Authorization: Bearer token$with-dollar"]),
        json!(["Authorization:", "Bearer token$with-dollar"]),
        json!(["-u", "user:pass"]),
        json!(["--user", "alice:secret"]),
        json!(["--proxy-user=proxy:secret"]),
        json!(["--client-secret-key", "actual-client-secret"]),
        json!(["--token-value", "actual-token"]),
        json!(["--authorization-header", "Bearer raw-token"]),
        json!(["-H", "X-Api-Key: abc123def456"]),
        json!(["--header=X-Api-Key: abc123def456"]),
        json!(["-c", "guard --token raw-secret"]),
        json!(["-xc", "guard --token raw-secret"]),
        json!([
            "-lc",
            "curl -H 'X-Api-Key: abc123def456' https://example.com"
        ]),
        json!(["https://user:pass@example.com/policy"]),
        json!(["https://example.com/policy?token=secret"]),
    ] {
        let config: AgentPolicyHookConfig = serde_json::from_value(json!({
            "enabled": true,
            "before_action": [{
                "name": "stdio-guard",
                "transport": "stdio",
                "command": "guard",
                "args": args
            }]
        }))
        .unwrap();

        assert!(matches!(
            config.validate(),
            Err(super::config::AgentPolicyHookConfigError::StdioArgContainsCredentials)
        ));

        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["enabled"], false);
        assert!(!value.to_string().contains("raw-secret"));
        assert!(!value.to_string().contains("abc123def456"));
    }
}

#[test]
fn config_rejects_stdio_hook_credential_command() {
    for command in [
        "guard --token secret",
        "guard --token %s",
        "guard --token %raw-secret",
        "guard --authorization 'Bearer raw-token'",
        "API_KEY=secret guard",
        "guard sk-secretsecretsecret",
        "guard ghp_secretsecretsecret",
        "curl -u user:pass https://example.com",
        "curl --user alice:secret https://example.com",
        "curl --proxy-user=proxy:secret https://example.com",
        "curl -H 'X-Api-Key: abc123def456' https://example.com",
        "curl --header='X-Api-Key: abc123def456' https://example.com",
        "sh -c 'guard --token raw-secret'",
        "sh -xc 'guard --token raw-secret'",
        "bash -euc \"guard --token raw-secret\"",
        "guard --client-secret-key actual-client-secret",
        "guard --token-value actual-token",
        "guard --authorization-header Bearer raw-token",
        "bash -lc \"curl -H 'X-Api-Key: abc123def456' https://example.com\"",
        "curl https://user:pass@example.com/policy",
        "curl 'https://example.com/policy?token=secret'",
    ] {
        let config: AgentPolicyHookConfig = serde_json::from_value(json!({
            "enabled": true,
            "before_action": [{
                "name": "stdio-guard",
                "transport": "stdio",
                "command": command
            }]
        }))
        .unwrap();

        assert!(matches!(
            config.validate(),
            Err(super::config::AgentPolicyHookConfigError::StdioCommandContainsCredentials)
        ));

        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["enabled"], false);
        assert!(!value.to_string().contains("secret"));
        assert!(!value.to_string().contains("raw-token"));
        assert!(!value.to_string().contains("raw-secret"));
        assert!(!value.to_string().contains("abc123def456"));
    }
}

#[test]
fn config_allows_stdio_hook_secret_env_reference_args() {
    for args in [
        json!(["--token", "$API_TOKEN"]),
        json!(["--api-key=${POLICY_API_KEY}"]),
        json!(["--authorization", "Bearer $POLICY_TOKEN"]),
        json!(["--auth", "Basic ${POLICY_AUTH}"]),
        json!(["Authorization: BEARER $HEADER_TOKEN"]),
        json!(["X-API-Key:", "$HEADER_API_KEY"]),
        json!(["Authorization:", "Bearer $HEADER_TOKEN"]),
        json!(["-H", "X-Api-Key: $HEADER_API_KEY"]),
        json!(["--header=Authorization: Bearer $HEADER_TOKEN"]),
        json!(["-c", "guard --token $API_TOKEN"]),
        json!(["-xc", "guard --token $API_TOKEN"]),
        json!([
            "-lc",
            "curl -H 'X-Api-Key: $HEADER_API_KEY' https://example.com"
        ]),
        json!(["https://example.com/policy?state=public-value"]),
    ] {
        let args_debug = args.clone();
        let config: AgentPolicyHookConfig = serde_json::from_value(json!({
            "enabled": true,
            "before_action": [{
                "name": "stdio-guard",
                "transport": "stdio",
                "command": "guard",
                "args": args
            }]
        }))
        .unwrap();

        let validation = config.validate();
        assert!(
            validation.is_ok(),
            "expected args {args_debug:?} to validate, got {validation:?}"
        );
    }
}

#[test]
fn config_allows_stdio_hook_secret_env_reference_command() {
    for command in [
        "guard --token $API_TOKEN",
        "curl -H 'X-Api-Key: $HEADER_API_KEY' https://example.com",
        "curl --header='Authorization: Bearer $HEADER_TOKEN' https://example.com",
        "sh -c 'guard --token $API_TOKEN'",
        "sh -xc 'guard --token $API_TOKEN'",
        "bash -euc \"guard --token $API_TOKEN\"",
        "bash -lc \"curl -H 'X-Api-Key: $HEADER_API_KEY' https://example.com\"",
        "curl 'https://example.com/policy?state=public-value'",
    ] {
        let config: AgentPolicyHookConfig = serde_json::from_value(json!({
            "enabled": true,
            "before_action": [{
                "name": "stdio-guard",
                "transport": "stdio",
                "command": command
            }]
        }))
        .unwrap();

        let validation = config.validate();
        assert!(
            validation.is_ok(),
            "expected command {command:?} to validate, got {validation:?}"
        );
    }
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
fn config_rejects_http_hook_url_embedded_credentials() {
    for url in [
        "https://token@example.com/policy",
        "https://user:pass@example.com/policy",
        "https://token@example .com/policy",
        "https:user:pass@example.com/policy",
        "https://example.com/policy?token=secret",
        "https://example.com/policy?api_key=secret",
        "https://example.com/policy?clientSecret=abc123",
        "https://example.com/policy?clientSecretKey=abc123def4567890",
        "https://example.com/policy?accessToken=abc123",
        "https://example.com/policy?accessTokenValue=abc123def4567890",
        "https://example.com/policy?refreshTokenId=abc123def4567890",
        "https://example.com/policy?refresh-token=abc123",
        "https://example.com/policy?q=sk-secretsecretsecret",
        "https://example.com/policy?state=ghp_secretsecretsecret",
        "https://example.com/policy?state=gho_secretsecretsecret",
        "https://example.com/policy?state=ghu_secretsecretsecret",
        "https://example.com/policy?state=ghs_secretsecretsecret",
        "https://example.com/policy?state=ghr_secretsecretsecret",
        "https://example.com/policy#access_token=secret",
        "https://example.com/policy#accessTokenValue=abc123def4567890",
        "https://example.com/policy#access_token%3Dsecret",
        "https://example.com/policy#state=sk-secretsecretsecret",
        "https://example.com/policy#state%3Dsk-secretsecretsecret",
        "https://example.com/policy#Authorization%3A%20Bearer%20secret",
        "https://example.com/policy?authorization=Bearer%20secret",
        "https://example.com/hooks/sk-secretsecretsecret",
        "https://example.com/hooks/Authorization%3A%20Bearer%20secret",
        "https://example.com/policy?api%255Fkey=abc123def456",
        "https://example.com/policy?api%252Dkey=abc123def456",
        "https://example.com/hooks/%2525252574oken/abc123def4567890",
        "ftp://user:pass@example.com/policy",
        "custom://example.com/policy?token=secret",
    ] {
        let config: AgentPolicyHookConfig = serde_json::from_value(json!({
            "enabled": true,
            "before_action": [{
                "name": "remote-guard",
                "transport": "http",
                "url": url
            }]
        }))
        .unwrap();

        assert!(matches!(
            config.validate(),
            Err(super::config::AgentPolicyHookConfigError::HttpUrlContainsCredentials)
        ));
    }
}

#[test]
fn config_allows_http_hook_url_non_credential_query_values() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "remote-guard",
            "transport": "http",
            "url": "https://example.com/policy?q=skeleton&state=public-value#section"
        }]
    }))
    .unwrap();

    assert!(config.validate().is_ok());
}

#[test]
fn config_rejects_hook_name_and_stdio_working_directory_credentials() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "guard-sk-secretsecretsecret",
            "transport": "stdio",
            "command": "guard"
        }]
    }))
    .unwrap();

    assert!(matches!(
        config.validate(),
        Err(super::config::AgentPolicyHookConfigError::HookNameContainsCredentials)
    ));
    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(value["enabled"], false);
    assert!(!value.to_string().contains("sk-secretsecretsecret"));

    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "stdio-guard",
            "transport": "stdio",
            "command": "guard",
            "working_directory": "/tmp/API_KEY=raw-secret-value"
        }]
    }))
    .unwrap();

    assert!(matches!(
        config.validate(),
        Err(super::config::AgentPolicyHookConfigError::StdioWorkingDirectoryContainsCredentials)
    ));
    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(value["enabled"], false);
    assert!(!value.to_string().contains("raw-secret-value"));
}

#[test]
fn config_rejects_disabled_http_hook_url_embedded_credentials() {
    for url in [
        "https://token@example.com/policy",
        "https://token@example .com/policy",
        "https:user:pass@example.com/policy",
        "https://example.com/policy?q=sk-secretsecretsecret",
        "https://example .com/policy?q=sk-secretsecretsecret",
        "https://example.com/hooks/sk-secretsecretsecret",
        "https://example .com/policy?api%255Fkey=abc123def456",
        "ftp://user:pass@example.com/policy",
        "custom://example.com/policy?token=secret",
    ] {
        let config: AgentPolicyHookConfig = serde_json::from_value(json!({
            "enabled": false,
            "before_action": [{
                "name": "remote-guard",
                "transport": "http",
                "url": url
            }]
        }))
        .unwrap();

        assert!(matches!(
            config.validate(),
            Err(super::config::AgentPolicyHookConfigError::HttpUrlContainsCredentials)
        ));
    }
}

#[test]
fn profile_serialization_sanitizes_disabled_http_hook_url_embedded_credentials() {
    for url in [
        "https:user:pass@example.com/policy",
        "https://example .com/policy?q=sk-secretsecretsecret",
        "https://example.com/hooks/sk-secretsecretsecret",
        "ftp://user:pass@example.com/policy",
    ] {
        let agent_policy_hooks = AgentPolicyHookConfig {
            enabled: false,
            before_action: vec![AgentPolicyHook {
                name: "remote-guard".to_string(),
                transport: AgentPolicyHookTransport::Http {
                    url: url.to_string(),
                    headers: Default::default(),
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let profile = AIExecutionProfile {
            agent_policy_hooks,
            ..Default::default()
        };

        let value = serde_json::to_value(&profile).unwrap();
        assert_eq!(value["agent_policy_hooks"]["enabled"], false);
        assert!(value["agent_policy_hooks"]["before_action"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(!value.to_string().contains('@'));
        assert!(!value.to_string().contains("sk-secretsecretsecret"));
    }
}

#[test]
fn profile_serialization_sanitizes_invalid_http_hook_urls() {
    for url in [
        "ssh://internal-host/policy",
        "http://example.com/policy",
        "https://exa mple.com/policy",
    ] {
        let agent_policy_hooks = AgentPolicyHookConfig {
            enabled: true,
            before_action: vec![AgentPolicyHook {
                name: "remote-guard".to_string(),
                transport: AgentPolicyHookTransport::Http {
                    url: url.to_string(),
                    headers: Default::default(),
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let profile = AIExecutionProfile {
            agent_policy_hooks,
            ..Default::default()
        };

        let value = serde_json::to_value(&profile).unwrap();
        assert_eq!(value["agent_policy_hooks"]["enabled"], false);
        assert!(value["agent_policy_hooks"]["before_action"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(!value.to_string().contains(url));
    }
}

#[test]
fn config_allows_disabled_incomplete_hook_without_persisted_credentials() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": false,
        "before_action": [{
            "transport": "stdio",
            "command": ""
        }]
    }))
    .unwrap();

    assert!(config.validate().is_ok());
    assert!(serde_json::to_value(&config).is_ok());
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
fn config_rejects_object_shaped_hook_secret_literals() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [
            {
                "name": "stdio-guard",
                "transport": "stdio",
                "command": "guard",
                "env": { "API_TOKEN": { "env": "sk-secretsecretsecret" } }
            },
            {
                "name": "http-guard",
                "transport": "http",
                "url": "https://example.com/policy",
                "headers": { "authorization": { "env": "Bearer raw-secret" } }
            }
        ]
    }))
    .unwrap();

    assert!(matches!(
        config.validate(),
        Err(super::config::AgentPolicyHookConfigError::InvalidSecretEnvironmentVariableName)
    ));
    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(value["enabled"], false);
    assert!(!value.to_string().contains("sk-secretsecretsecret"));
    assert!(!value.to_string().contains("Bearer raw-secret"));
}

#[test]
fn config_rejects_hook_secret_map_literal_keys() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [
            {
                "name": "stdio-guard",
                "transport": "stdio",
                "command": "guard",
                "env": { "ghp_secretsecretsecret": { "env": "POLICY_TOKEN" } }
            },
            {
                "name": "http-guard",
                "transport": "http",
                "url": "https://example.com/policy",
                "headers": { "sk-secretsecretsecret": { "env": "POLICY_HEADER" } }
            }
        ]
    }))
    .unwrap();

    assert!(matches!(
        config.validate(),
        Err(super::config::AgentPolicyHookConfigError::InvalidSecretEnvironmentVariableName)
    ));
    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(value["enabled"], false);
    assert!(!value.to_string().contains("ghp_secretsecretsecret"));
    assert!(!value.to_string().contains("sk-secretsecretsecret"));
}

#[test]
fn config_rejects_http_hook_secret_header_literal_key() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "http-guard",
            "transport": "http",
            "url": "https://example.com/policy",
            "headers": { "sk-secretsecretsecret": { "env": "POLICY_HEADER" } }
        }]
    }))
    .unwrap();

    assert!(matches!(
        config.validate(),
        Err(super::config::AgentPolicyHookConfigError::InvalidHttpHeaderName(_))
    ));
    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(value["enabled"], false);
    assert!(!value.to_string().contains("sk-secretsecretsecret"));
}

#[test]
fn config_rejects_whitespace_padded_hook_secret_refs() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "stdio-guard",
            "transport": "stdio",
            "command": "guard",
            "env": { "API_TOKEN": { "env": " POLICY_TOKEN " } }
        }]
    }))
    .unwrap();

    assert!(matches!(
        config.validate(),
        Err(super::config::AgentPolicyHookConfigError::InvalidSecretEnvironmentVariableName)
    ));
    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(value["enabled"], false);
    assert!(!value.to_string().contains(" POLICY_TOKEN "));
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
    assert_eq!(value["hook_autoapproval_enabled"], false);
    assert_eq!(value["warp_permission"]["decision"], "allow");

    let command = value["action"]["command"].as_str().unwrap();
    assert!(command.contains("OPENAI_API_KEY=<redacted>"));
    assert!(command.contains("Authorization: Bearer <redacted>"));
    assert!(!command.contains("sk-secretsecretsecret"));
    assert_eq!(value["action"]["is_risky"], true);
}

#[test]
fn event_serializes_hook_autoapproval_state() {
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        Some(PathBuf::from(
            "/repo/sk-secretsecretsecret/clientSecret=raw-secret-value",
        )),
        false,
        Some("profile_default".to_string()),
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    )
    .with_hook_autoapproval_enabled(true);

    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["hook_autoapproval_enabled"], true);
}

#[test]
fn policy_event_redacts_working_directory_before_serialization() {
    let raw_path = PathBuf::from("/tmp/sk-secretsecretsecret/clientSecret=raw-secret-value");
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        Some(raw_path.clone()),
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    assert_eq!(event.working_directory.as_deref(), Some(raw_path.as_path()));
    let value = serde_json::to_string(&event).unwrap();
    assert!(value.contains("<redacted>"));
    assert!(!value.contains("sk-secretsecretsecret"));
    assert!(!value.contains("raw-secret-value"));
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
fn command_redaction_handles_url_userinfo_and_basic_auth() {
    let command = concat!(
        "curl -u user:pass https://user:pass@example.com/api ",
        "-H 'Authorization: Basic dXNlcjpwYXNz' && ",
        "curl --user='alice:secret value' https://token@example.org"
    );

    let redacted = redact_command_for_policy(command);

    assert!(redacted.contains("curl -u <redacted>"));
    assert!(redacted.contains("Authorization: Basic <redacted>"));
    assert!(redacted.contains("https://<redacted>@example.com/api"));
    assert!(redacted.contains("https://<redacted>@example.org"));
    assert!(!redacted.contains("user:pass"));
    assert!(!redacted.contains("alice:secret"));
    assert!(!redacted.contains("dXNlcjpwYXNz"));
    assert!(!redacted.contains("token@example"));
}

#[test]
fn command_redaction_handles_generic_credential_headers() {
    let command = concat!(
        "curl -H 'X-API-Key: abc123def456' ",
        "-H 'Client-Secret: client-secret-value' ",
        "-H 'X-Access-Token: Bearer raw-access-token' ",
        "-H 'Authorization: raw-auth-token' ",
        "-H 'X-Auth: raw-auth-secret' ",
        "https://example.com"
    );

    let redacted = redact_command_for_policy(command);

    assert!(redacted.contains("X-API-Key: <redacted>"));
    assert!(redacted.contains("Client-Secret: <redacted>"));
    assert!(redacted.contains("X-Access-Token: <redacted>"));
    assert!(redacted.contains("Authorization: <redacted>"));
    assert!(redacted.contains("X-Auth: <redacted>"));
    assert!(!redacted.contains("abc123def456"));
    assert!(!redacted.contains("client-secret-value"));
    assert!(!redacted.contains("raw-access-token"));
    assert!(!redacted.contains("raw-auth-token"));
    assert!(!redacted.contains("raw-auth-secret"));
}

#[test]
fn command_redaction_handles_split_secret_args() {
    let command = concat!(
        "guard --token token-secret --password 'quoted secret' ",
        "--api-key sk-secretsecretsecret --authorization Bearer split-secret ",
        "--authorization=Bearer eq-secret --auth Basic basic-secret ",
        "--client-secret client-secret-value --refresh-token refresh-secret ",
        "--access-token access-secret --clientSecret=camel-secret ",
        "--client-secret-key client-key-secret --token-value token-value-secret ",
        "--authorization-header Bearer header-secret ",
        "--safe visible"
    );

    let redacted = redact_command_for_policy(command);

    assert!(redacted.contains("--token <redacted>"));
    assert!(redacted.contains("--password <redacted>"));
    assert!(redacted.contains("--api-key <redacted>"));
    assert!(redacted.contains("--authorization <redacted>"));
    assert!(redacted.contains("--authorization=<redacted>"));
    assert!(redacted.contains("--auth <redacted>"));
    assert!(redacted.contains("--client-secret <redacted>"));
    assert!(redacted.contains("--refresh-token <redacted>"));
    assert!(redacted.contains("--access-token <redacted>"));
    assert!(redacted.contains("--clientSecret=<redacted>"));
    assert!(redacted.contains("--client-secret-key <redacted>"));
    assert!(redacted.contains("--token-value <redacted>"));
    assert!(redacted.contains("--authorization-header <redacted>"));
    assert!(redacted.contains("--safe visible"));
    assert!(!redacted.contains("token-secret"));
    assert!(!redacted.contains("quoted secret"));
    assert!(!redacted.contains("sk-secretsecretsecret"));
    assert!(!redacted.contains("split-secret"));
    assert!(!redacted.contains("eq-secret"));
    assert!(!redacted.contains("basic-secret"));
    assert!(!redacted.contains("client-secret-value"));
    assert!(!redacted.contains("refresh-secret"));
    assert!(!redacted.contains("access-secret"));
    assert!(!redacted.contains("camel-secret"));
    assert!(!redacted.contains("client-key-secret"));
    assert!(!redacted.contains("token-value-secret"));
    assert!(!redacted.contains("header-secret"));
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
    assert_eq!(action.omitted_argument_key_count, None);
}

#[test]
fn mcp_tool_action_redacts_secret_shaped_argument_keys() {
    let action = PolicyCallMcpToolAction::new(
        None,
        "tool-sk-secretsecretsecret",
        &json!({
            "Authorization: Bearer rawbearer": "omitted",
            "sk-secretsecretsecret": "omitted",
            "path": "/repo"
        }),
    );

    assert_eq!(action.tool_name, "tool-<redacted>");
    assert_eq!(
        action.argument_keys,
        vec!["<redacted>", "Authorization: Bearer <redacted>", "path"]
    );
}

#[test]
fn mcp_resource_action_redacts_secret_shaped_uri() {
    let action = PolicyReadMcpResourceAction::new(
        None,
        "resource-ghp_secretsecretsecret",
        Some(
            "mcp://user:secret@example/resource?api_key=raw-key&state=sk-secretsecretsecret"
                .to_string(),
        ),
    );

    assert_eq!(action.name, "resource-<redacted>");
    let uri = action.uri.as_deref().unwrap();
    assert!(uri.contains("mcp://<redacted>@example/resource"));
    assert!(uri.contains("api_key=<redacted>"));
    assert!(!uri.contains("raw-key"));
    assert!(!uri.contains("sk-secretsecretsecret"));
}

#[test]
fn policy_action_collections_are_capped() {
    let paths = (0..MAX_POLICY_COLLECTION_ITEMS + 3)
        .map(|index| PathBuf::from(format!("/tmp/policy-path-{index}")));
    let action = PolicyReadFilesAction::new(paths);

    assert_eq!(action.paths.len(), MAX_POLICY_COLLECTION_ITEMS);
    assert_eq!(action.omitted_path_count, Some(3));

    let mut arguments = serde_json::Map::new();
    for index in 0..MAX_POLICY_COLLECTION_ITEMS + 2 {
        arguments.insert(format!("key_{index:03}"), json!(index));
    }
    let action = PolicyCallMcpToolAction::new(None, "tool", &serde_json::Value::Object(arguments));

    assert_eq!(action.argument_keys.len(), MAX_POLICY_COLLECTION_ITEMS);
    assert_eq!(action.omitted_argument_key_count, Some(2));
}

#[test]
fn policy_file_paths_are_redacted_before_serialization() {
    let read = PolicyReadFilesAction::new([
        PathBuf::from("/tmp/sk-secretsecretsecret/report.md"),
        PathBuf::from("/tmp/clientSecret=raw-secret-value/config.md"),
        PathBuf::from("/tmp/X-API-Key: abc123def456/config.md"),
    ]);
    let write = PolicyWriteFilesAction::new(
        [PathBuf::from(
            "/tmp/Authorization: Bearer raw-path-token/output.md",
        )],
        None,
    );

    let value = serde_json::to_string(&json!({
        "read": read,
        "write": write,
    }))
    .unwrap();

    assert!(value.contains("<redacted>"));
    assert!(!value.contains("sk-secretsecretsecret"));
    assert!(!value.contains("raw-secret-value"));
    assert!(!value.contains("abc123def456"));
    assert!(!value.contains("raw-path-token"));
}

#[test]
fn mcp_resource_uri_redacts_generic_and_percent_encoded_credentials() {
    let resource = PolicyReadMcpResourceAction::new(
        None,
        "resource",
        Some(
            "mcp://srv/resource/X-API-Key: abc123def456?api_key%3Draw-key#Authorization%3A%20Bearer%20raw-token"
                .to_string(),
        ),
    );

    let value = serde_json::to_string(&resource).unwrap();
    assert!(value.contains("<redacted>"));
    assert!(!value.contains("abc123def456"));
    assert!(!value.contains("raw-key"));
    assert!(!value.contains("raw-token"));
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
fn policy_decision_composition_does_not_autoapprove_unavailable_allow() {
    let unavailable_allow = AgentPolicyHookEvaluation::unavailable(
        "guard",
        AgentPolicyDecisionKind::Allow,
        AgentPolicyHookErrorKind::Timeout,
        "hook timed out",
    );

    let decision = compose_policy_decisions(
        WarpPermissionSnapshot::ask(Some("AlwaysAsk".to_string())),
        vec![unavailable_allow],
        true,
    );

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Ask);
    assert_eq!(decision.reason.as_deref(), Some("AlwaysAsk"));
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
                "OPENAI_API_KEY=sk-secretsecretsecret X-API-Key: abc123def456 {}",
                "x".repeat(10_000)
            )),
            external_audit_id: Some("audit-ghp_secretsecretsecret".to_string()),
        },
    );

    let reason = evaluation.reason.as_deref().unwrap();
    assert!(reason.contains("OPENAI_API_KEY=<redacted>"));
    assert!(reason.contains("X-API-Key: <redacted>"));
    assert!(!reason.contains("sk-secretsecretsecret"));
    assert!(!reason.contains("abc123def456"));
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
    assert!(!line.contains("sk-secretsecretsecret"));
    assert!(!line.contains("raw-secret-value"));
    assert!(!line.contains("token123"));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn policy_event_new_redacts_execute_command_payload() {
    let event = AgentPolicyEvent::new(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        AgentPolicyAction::ExecuteCommand(PolicyExecuteCommandAction::new(
            "OPENAI_API_KEY=sk-secretsecretsecret curl https://example.com",
            "OPENAI_API_KEY=sk-secretsecretsecret curl https://example.com",
            Some(false),
            Some(true),
        )),
    );
    let line = audit_record_json_line(
        &event,
        &compose_policy_decisions(WarpPermissionSnapshot::allow(None), Vec::new(), false),
    )
    .unwrap();

    assert!(line.contains("OPENAI_API_KEY=<redacted>"));
    assert!(!line.contains("sk-secretsecretsecret"));
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
async fn stdio_engine_requires_global_and_hook_autoapproval_for_warp_ask() {
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "local-guard",
            "transport": "stdio",
            "command": "sh",
            "args": [
                "-c",
                "cat >/dev/null; printf '%s\n' '{\"schema_version\":\"warp.agent_policy_hook.v1\",\"decision\":\"allow\",\"reason\":\"approved by test\"}'"
            ],
            "allow_autoapproval": true,
            "timeout_ms": 1000
        }]
    }))
    .unwrap();
    let event = AgentPolicyEvent::execute_command(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::ask(Some("AlwaysAsk".to_string())),
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let global_disabled = AgentPolicyHookEngine::new(config.clone())
        .preflight(
            event.clone(),
            WarpPermissionSnapshot::ask(Some("AlwaysAsk".to_string())),
        )
        .await;
    assert_eq!(global_disabled.decision, AgentPolicyDecisionKind::Ask);
    assert_eq!(global_disabled.reason.as_deref(), Some("AlwaysAsk"));

    let mut global_enabled_config = config;
    global_enabled_config.allow_hook_autoapproval = true;
    let global_enabled = AgentPolicyHookEngine::new(global_enabled_config)
        .preflight(
            event,
            WarpPermissionSnapshot::ask(Some("AlwaysAsk".to_string())),
        )
        .await;
    assert_eq!(global_enabled.decision, AgentPolicyDecisionKind::Allow);
    assert_eq!(global_enabled.reason.as_deref(), Some("approved by test"));
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
    let suffix = "x".repeat(120);
    let paths = (0..650)
        .map(|index| PathBuf::from(format!("/tmp/policy-hook-large-event-{index}-{suffix}")))
        .collect();
    let event = AgentPolicyEvent::new(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        AgentPolicyAction::ReadFiles(PolicyReadFilesAction {
            paths,
            omitted_path_count: None,
        }),
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

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_rejects_oversized_policy_event_before_request() {
    let server = mockito::Server::new_async().await;
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
    let event = AgentPolicyEvent::new(
        "conv_123",
        "action_456",
        None,
        false,
        None,
        WarpPermissionSnapshot::allow(None),
        AgentPolicyAction::ReadFiles(PolicyReadFilesAction {
            paths: vec![PathBuf::from(format!("/tmp/{}", "x".repeat(200_000)))],
            omitted_path_count: None,
        }),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::PayloadTooLarge)
    );
}

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_does_not_inherit_parent_environment() {
    const PARENT_ONLY_ENV: &str = "WARP_POLICY_HOOK_TEST_PARENT_ENV_SENTINEL";
    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    let _env_guard = EnvGuard {
        key: PARENT_ONLY_ENV,
        previous: std::env::var_os(PARENT_ONLY_ENV),
    };
    std::env::set_var(PARENT_ONLY_ENV, "parent-only");
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "before_action": [{
            "name": "env-isolated-guard",
            "transport": "stdio",
            "command": "/bin/sh",
            "args": [
                "-c",
                "cat >/dev/null; if [ \"${WARP_POLICY_HOOK_TEST_PARENT_ENV_SENTINEL+x}\" = x ]; then printf '%s\\n' '{\"schema_version\":\"warp.agent_policy_hook.v1\",\"decision\":\"deny\",\"reason\":\"inherited parent sentinel\"}'; else printf '%s\\n' '{\"schema_version\":\"warp.agent_policy_hook.v1\",\"decision\":\"allow\"}'; fi"
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
        PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
    );

    let decision = engine
        .preflight(event, WarpPermissionSnapshot::allow(None))
        .await;

    assert_eq!(decision.decision, AgentPolicyDecisionKind::Allow);
    assert_eq!(
        decision.hook_results[0].decision,
        AgentPolicyDecisionKind::Allow
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

#[cfg(all(unix, not(target_family = "wasm")))]
#[tokio::test]
async fn stdio_engine_redacts_configured_secret_malformed_response_error() {
    const SECRET_ENV: &str = "WARP_POLICY_HOOK_TEST_MALFORMED_STDIO_SECRET";
    const SECRET_VALUE: &str = "sk-malformedstdiosecret";

    struct EnvGuard;
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(SECRET_ENV);
        }
    }

    let _guard = EnvGuard;
    std::env::set_var(SECRET_ENV, SECRET_VALUE);
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "secret-malformed-guard",
            "transport": "stdio",
            "command": "sh",
            "args": [
                "-c",
                "cat >/dev/null; printf '{\"schema_version\":\"%s\",\"decision\":\"allow\"}\\n' \"$API_TOKEN\""
            ],
            "env": { "API_TOKEN": { "env": SECRET_ENV } },
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
    let audit_line = audit_record_json_line(
        &AgentPolicyEvent::execute_command(
            "conv_123",
            "action_456",
            None,
            false,
            None,
            WarpPermissionSnapshot::allow(None),
            PolicyExecuteCommandAction::new("ls", "ls", Some(true), Some(false)),
        ),
        &decision,
    )
    .unwrap();

    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::MalformedResponse)
    );
    assert!(!reason.contains(SECRET_VALUE));
    assert!(!audit_line.contains(SECRET_VALUE));
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

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_redacts_configured_header_secret_malformed_response_error() {
    let mut server = mockito::Server::new_async().await;
    const SECRET_ENV: &str = "WARP_POLICY_HOOK_TEST_MALFORMED_HTTP_SECRET";
    const SECRET_VALUE: &str = "Bearer malformedhttpsecret";

    struct EnvGuard;
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(SECRET_ENV);
        }
    }

    let _guard = EnvGuard;
    std::env::set_var(SECRET_ENV, SECRET_VALUE);
    let hook_response = json!({
        "schema_version": SECRET_VALUE,
        "decision": "allow"
    })
    .to_string();
    let mock = server
        .mock("POST", "/policy")
        .match_header("authorization", SECRET_VALUE)
        .with_status(200)
        .with_body(hook_response)
        .create_async()
        .await;
    let config: AgentPolicyHookConfig = serde_json::from_value(json!({
        "enabled": true,
        "on_unavailable": "deny",
        "before_action": [{
            "name": "http-guard",
            "transport": "http",
            "url": format!("{}/policy", server.url()),
            "headers": { "authorization": { "env": SECRET_ENV } },
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
    let reason = decision.hook_results[0].reason.as_deref().unwrap();
    let audit_line = audit_record_json_line(
        &AgentPolicyEvent::execute_command(
            "conv_123",
            "action_456",
            None,
            false,
            None,
            WarpPermissionSnapshot::allow(None),
            PolicyExecuteCommandAction::new("rm -rf .", "rm -rf .", Some(false), Some(true)),
        ),
        &decision,
    )
    .unwrap();

    mock.assert_async().await;
    assert_eq!(
        decision.hook_results[0].error,
        Some(AgentPolicyHookErrorKind::MalformedResponse)
    );
    assert!(!reason.contains(SECRET_VALUE));
    assert!(!audit_line.contains(SECRET_VALUE));
}

#[cfg(not(target_family = "wasm"))]
#[tokio::test]
async fn http_engine_redacts_basic_header_credential_fragment_hook_reason() {
    let mut server = mockito::Server::new_async().await;
    let secret_env = "WARP_POLICY_HOOK_TEST_BASIC_AUTH";
    let credential = "dXNlcjpwYXNz";
    let secret_value = format!("Basic {credential}");
    std::env::set_var(secret_env, &secret_value);
    let hook_response = json!({
        "schema_version": AGENT_POLICY_SCHEMA_VERSION,
        "decision": "deny",
        "reason": format!("credential fragment {credential}"),
        "external_audit_id": format!("audit-{credential}")
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
    assert_eq!(reason, "credential fragment <redacted>");
    assert!(!reason.contains(credential));
    assert_eq!(
        decision.hook_results[0].external_audit_id.as_deref(),
        Some("audit-<redacted>")
    );
    std::env::remove_var(secret_env);
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
