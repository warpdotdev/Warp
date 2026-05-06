use std::{collections::HashMap, ffi::OsString, sync::Arc, time::Duration};

use futures::channel::oneshot;
use warp_cli::agent::Harness;
use warp_cli::{
    OZ_CLI_ENV, OZ_HARNESS_ENV, OZ_PARENT_RUN_ID_ENV, OZ_RUN_ID_ENV, SERVER_ROOT_URL_OVERRIDE_ENV,
    SESSION_SHARING_SERVER_URL_OVERRIDE_ENV, WS_SERVER_URL_OVERRIDE_ENV,
};
use warp_core::channel::ChannelState;

use super::{
    AgentDriver, build_secret_env_vars, IdleTimeoutSender, SkillRepoLoadMode,
    LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV, LEGACY_OZ_PARENT_STATE_ROOT_ENV,
    OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV, OZ_MESSAGE_LISTENER_STATE_ROOT_ENV,
};
use crate::ai::agent::{
    task::TaskId, AIAgentActionResult, AIAgentActionResultType, AIAgentInput, AIAgentOutput,
    AIAgentOutputMessage, ArtifactCreatedData, MessageId, UploadArtifactResult,
};
use crate::ai::mcp::parsing::normalize_mcp_json;
use crate::ai::{
    agent_sdk::task_env_vars, ambient_agents::AmbientAgentTaskId, cloud_environments::GithubRepo,
};
use warp_managed_secrets::ManagedSecretValue;

#[test]
fn test_normalize_single_cli_server() {
    let input = r#"{"command": "npx", "args": ["-y", "mcp-server"]}"#;
    let result = normalize_mcp_json(input).unwrap();

    // Should wrap with a generated name
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let parsed = parsed.as_object().unwrap();
    assert_eq!(parsed.len(), 1);
    let (_name, server) = parsed.iter().next().unwrap();
    assert_eq!(server["command"].as_str().unwrap(), "npx");
}

#[test]
fn test_normalize_single_sse_server() {
    let input = r#"{"url": "http://localhost:3000/mcp", "headers": {"API_KEY": "value"}}"#;
    let result = normalize_mcp_json(input).unwrap();

    // Should wrap with a generated name
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let parsed = parsed.as_object().unwrap();
    assert_eq!(parsed.len(), 1);
    let (_name, server) = parsed.iter().next().unwrap();
    assert_eq!(server["url"].as_str().unwrap(), "http://localhost:3000/mcp");
}

#[test]
fn test_normalize_already_wrapped_server() {
    let input = r#"{"my-server": {"command": "npx", "args": []}}"#;
    let result = normalize_mcp_json(input).unwrap();

    // Should return as-is (no command/url at top level)
    assert_eq!(result, input);
}

#[test]
fn test_normalize_mcp_servers_wrapper() {
    let input = r#"{"mcpServers": {"server-name": {"command": "npx", "args": []}}}"#;
    let result = normalize_mcp_json(input).unwrap();

    // Should return as-is (no command/url at top level)
    assert_eq!(result, input);
}

#[test]
fn test_normalize_servers_wrapper() {
    let input = r#"{"servers": {"server-name": {"url": "http://example.com"}}}"#;
    let result = normalize_mcp_json(input).unwrap();

    // Should return as-is (no command/url at top level)
    assert_eq!(result, input);
}

#[test]
fn test_normalize_invalid_json() {
    let input = "not valid json";
    let result = normalize_mcp_json(input);

    assert!(result.is_err());
}

#[test]
fn test_normalize_cli_server_with_env() {
    let input = r#"{"command": "npx", "args": ["-y", "mcp-server"], "env": {"API_KEY": "secret"}}"#;
    let result = normalize_mcp_json(input).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let parsed = parsed.as_object().unwrap();
    assert_eq!(parsed.len(), 1);
    let (_name, server) = parsed.iter().next().unwrap();
    assert_eq!(server["env"]["API_KEY"].as_str().unwrap(), "secret");
}

#[test]
fn test_normalize_sse_server_with_headers() {
    let input =
        r#"{"url": "http://localhost:5000/mcp", "headers": {"Authorization": "Bearer token"}}"#;
    let result = normalize_mcp_json(input).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let parsed = parsed.as_object().unwrap();
    assert_eq!(parsed.len(), 1);
    let (_name, server) = parsed.iter().next().unwrap();
    assert_eq!(
        server["headers"]["Authorization"].as_str().unwrap(),
        "Bearer token"
    );
}

// ── IdleTimeoutSender tests ──────────────────────────────────────────────────────

#[test]
fn idle_timeout_sender_send_now_delivers_value() {
    let (tx, mut rx) = oneshot::channel::<i32>();
    let idle_timeout = IdleTimeoutSender::new(tx);
    idle_timeout.end_run_now(42);
    assert_eq!(rx.try_recv().unwrap(), Some(42));
}

#[test]
fn idle_timeout_sender_send_now_only_delivers_once() {
    let (tx, mut rx) = oneshot::channel::<i32>();
    let idle_timeout = IdleTimeoutSender::new(tx);
    idle_timeout.end_run_now(1);
    idle_timeout.end_run_now(2);
    assert_eq!(rx.try_recv().unwrap(), Some(1));
}

#[test]
fn idle_timeout_sender_send_after_delivers_after_timeout() {
    let (tx, mut rx) = oneshot::channel::<i32>();
    let idle_timeout = IdleTimeoutSender::new(tx);
    idle_timeout.end_run_after(Duration::from_millis(50), 99);

    // Not yet delivered.
    assert_eq!(rx.try_recv().unwrap(), None);

    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(rx.try_recv().unwrap(), Some(99));
}

#[test]
fn idle_timeout_sender_cancel_prevents_delivery() {
    let (tx, mut rx) = oneshot::channel::<i32>();
    let idle_timeout = IdleTimeoutSender::new(tx);
    idle_timeout.end_run_after(Duration::from_millis(50), 99);
    idle_timeout.cancel_idle_timeout();

    std::thread::sleep(Duration::from_millis(100));
    // Sender was not consumed, so the channel is still open but empty.
    assert_eq!(rx.try_recv().unwrap(), None);
}

#[test]
fn idle_timeout_sender_cancel_then_send_now_delivers() {
    let (tx, mut rx) = oneshot::channel::<i32>();
    let idle_timeout = IdleTimeoutSender::new(tx);
    idle_timeout.end_run_after(Duration::from_millis(50), 1);
    idle_timeout.cancel_idle_timeout();
    idle_timeout.end_run_now(2);

    assert_eq!(rx.try_recv().unwrap(), Some(2));
}

#[test]
fn idle_timeout_sender_later_send_after_supersedes_earlier() {
    let (tx, mut rx) = oneshot::channel::<i32>();
    let idle_timeout = IdleTimeoutSender::new(tx);
    // First timer: long timeout.
    idle_timeout.end_run_after(Duration::from_secs(10), 1);
    // Second timer: short timeout. The first is implicitly cancelled.
    idle_timeout.end_run_after(Duration::from_millis(50), 2);

    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(rx.try_recv().unwrap(), Some(2));
}

#[test]
fn skill_repo_load_requests_loads_all_environment_repos() {
    let environment_repo = github_repo("warpdotdev", "warp-internal");
    let global_repo = github_repo("warpdotdev", "warp-skills");
    let global_specs = vec![
        skill_spec("warpdotdev/warp-internal:env-skill"),
        skill_spec("warpdotdev/warp-skills:global-skill"),
    ];

    let requests = AgentDriver::skill_repo_load_requests(
        vec![environment_repo.clone()],
        vec![environment_repo.clone(), global_repo.clone()],
        &global_specs,
    );

    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].repo, environment_repo);
    assert!(matches!(&requests[0].load_mode, SkillRepoLoadMode::All));
    assert_eq!(requests[1].repo, global_repo);
    let SkillRepoLoadMode::ExplicitGlobal(specs) = &requests[1].load_mode else {
        panic!("expected explicit global skill load mode");
    };
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].skill_identifier, "global-skill");
}

#[test]
fn skill_repo_load_requests_filters_global_only_repos_to_matching_specs() {
    let first_repo = github_repo("warpdotdev", "warp-skills");
    let second_repo = github_repo("warpdotdev", "warp-server-skills");
    let global_specs = vec![
        skill_spec("warpdotdev/warp-skills:first"),
        skill_spec("warpdotdev/warp-server-skills:second"),
        skill_spec("warpdotdev/warp-skills:third"),
    ];

    let requests = AgentDriver::skill_repo_load_requests(
        Vec::new(),
        vec![first_repo.clone(), second_repo],
        &global_specs,
    );

    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].repo, first_repo);
    let SkillRepoLoadMode::ExplicitGlobal(specs) = &requests[0].load_mode else {
        panic!("expected explicit global skill load mode");
    };
    assert_eq!(
        specs
            .iter()
            .map(|spec| spec.skill_identifier.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "third"]
    );
}

#[test]
fn task_env_vars_include_parent_run_id_when_present() {
    let task_id: AmbientAgentTaskId = "550e8400-e29b-41d4-a716-446655440000".parse().unwrap();
    let env_vars = task_env_vars(Some(&task_id), Some("parent-run-123"), Harness::Claude);
    let overrides_allowed = ChannelState::channel().allows_server_url_overrides();

    assert_eq!(
        env_vars.get(&OsString::from(OZ_RUN_ID_ENV)),
        Some(&OsString::from(task_id.to_string()))
    );
    assert_eq!(
        env_vars.get(&OsString::from(OZ_PARENT_RUN_ID_ENV)),
        Some(&OsString::from("parent-run-123"))
    );
    assert_eq!(
        env_vars.get(&OsString::from(OZ_HARNESS_ENV)),
        Some(&OsString::from("claude"))
    );
    assert_eq!(
        env_vars.get(&OsString::from(OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV)),
        Some(&OsString::from("1"))
    );
    assert_eq!(
        env_vars.get(&OsString::from(
            LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV
        )),
        Some(&OsString::from("1"))
    );
    assert!(env_vars
        .get(&OsString::from(OZ_CLI_ENV))
        .is_some_and(|value| !value.is_empty()));

    let server_root_url = ChannelState::server_root_url().into_owned();
    if overrides_allowed && !server_root_url.is_empty() {
        assert_eq!(
            env_vars.get(&OsString::from(SERVER_ROOT_URL_OVERRIDE_ENV)),
            Some(&OsString::from(server_root_url))
        );
    } else {
        assert!(!env_vars.contains_key(&OsString::from(SERVER_ROOT_URL_OVERRIDE_ENV)));
    }

    let ws_server_url = ChannelState::ws_server_url().into_owned();
    if overrides_allowed && !ws_server_url.is_empty() {
        assert_eq!(
            env_vars.get(&OsString::from(WS_SERVER_URL_OVERRIDE_ENV)),
            Some(&OsString::from(ws_server_url))
        );
    } else {
        assert!(!env_vars.contains_key(&OsString::from(WS_SERVER_URL_OVERRIDE_ENV)));
    }

    if overrides_allowed {
        match ChannelState::session_sharing_server_url() {
            Some(url) if !url.is_empty() => assert_eq!(
                env_vars.get(&OsString::from(SESSION_SHARING_SERVER_URL_OVERRIDE_ENV)),
                Some(&OsString::from(url.into_owned()))
            ),
            _ => {
                assert!(!env_vars
                    .contains_key(&OsString::from(SESSION_SHARING_SERVER_URL_OVERRIDE_ENV)))
            }
        }
    } else {
        assert!(!env_vars.contains_key(&OsString::from(SESSION_SHARING_SERVER_URL_OVERRIDE_ENV)));
    }
}

#[test]
fn task_env_vars_omit_parent_run_id_when_absent() {
    let task_id: AmbientAgentTaskId = "550e8400-e29b-41d4-a716-446655440001".parse().unwrap();
    let env_vars = task_env_vars(Some(&task_id), None, Harness::Oz);
    let overrides_allowed = ChannelState::channel().allows_server_url_overrides();

    assert_eq!(
        env_vars.get(&OsString::from(OZ_RUN_ID_ENV)),
        Some(&OsString::from(task_id.to_string()))
    );
    assert!(!env_vars.contains_key(&OsString::from(OZ_PARENT_RUN_ID_ENV)));
    assert_eq!(
        env_vars.get(&OsString::from(OZ_HARNESS_ENV)),
        Some(&OsString::from("oz"))
    );
    assert!(!env_vars.contains_key(&OsString::from(OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV)));
    assert!(!env_vars.contains_key(&OsString::from(
        LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV
    )));
    assert_eq!(
        env_vars.contains_key(&OsString::from(SERVER_ROOT_URL_OVERRIDE_ENV)),
        overrides_allowed && !ChannelState::server_root_url().is_empty()
    );
    assert_eq!(
        env_vars.contains_key(&OsString::from(WS_SERVER_URL_OVERRIDE_ENV)),
        overrides_allowed && !ChannelState::ws_server_url().is_empty()
    );
}

#[test]
fn task_env_vars_enable_external_parent_listener_for_claude_runs_without_parent_run_id() {
    let task_id: AmbientAgentTaskId = "550e8400-e29b-41d4-a716-446655440002".parse().unwrap();
    let env_vars = task_env_vars(Some(&task_id), None, Harness::Claude);
    assert_eq!(
        env_vars.get(&OsString::from(OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV)),
        Some(&OsString::from("1"))
    );
    assert_eq!(
        env_vars.get(&OsString::from(
            LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV
        )),
        Some(&OsString::from("1"))
    );
}

#[test]
#[serial_test::serial]
fn task_env_vars_propagate_message_listener_state_root_with_legacy_alias() {
    let task_id: AmbientAgentTaskId = "550e8400-e29b-41d4-a716-446655440003".parse().unwrap();
    std::env::set_var(
        OZ_MESSAGE_LISTENER_STATE_ROOT_ENV,
        "/tmp/message-listener-root",
    );
    let env_vars = task_env_vars(Some(&task_id), None, Harness::Claude);
    std::env::remove_var(OZ_MESSAGE_LISTENER_STATE_ROOT_ENV);

    assert_eq!(
        env_vars.get(&OsString::from(OZ_MESSAGE_LISTENER_STATE_ROOT_ENV)),
        Some(&OsString::from("/tmp/message-listener-root"))
    );
    assert_eq!(
        env_vars.get(&OsString::from(LEGACY_OZ_PARENT_STATE_ROOT_ENV)),
        Some(&OsString::from("/tmp/message-listener-root"))
    );
}

#[test]
fn task_env_vars_can_use_opencode_harness() {
    let task_id: AmbientAgentTaskId = "550e8400-e29b-41d4-a716-446655440004".parse().unwrap();
    let env_vars = task_env_vars(Some(&task_id), Some("parent-run-456"), Harness::OpenCode);

    assert_eq!(
        env_vars.get(&OsString::from(OZ_HARNESS_ENV)),
        Some(&OsString::from("opencode"))
    );
}

#[test]
fn json_format_output_includes_filename_for_file_artifact_created_event() {
    let output = AIAgentOutput {
        messages: vec![AIAgentOutputMessage::artifact_created(
            MessageId::new("message-1".to_string()),
            ArtifactCreatedData::File {
                artifact_uid: "artifact-uid".to_string(),
                filepath: "outputs/report.txt".to_string(),
                filename: "report.txt".to_string(),
                mime_type: "text/plain".to_string(),
                description: Some("Build output for the latest run".to_string()),
                size_bytes: 42,
            },
        )],
        ..Default::default()
    };

    let mut bytes = Vec::new();
    super::output::json::format_output(&output, &mut bytes).expect("json formatting should work");

    let value: serde_json::Value =
        serde_json::from_slice(&bytes).expect("output should be valid json");

    assert_eq!(value["type"], "artifact_created");
    assert_eq!(value["artifact_type"], "file");
    assert_eq!(value["artifact_uid"], "artifact-uid");
    assert_eq!(value["filepath"], "outputs/report.txt");
    assert_eq!(value["filename"], "report.txt");
    assert_eq!(value["mime_type"], "text/plain");
    assert_eq!(value["description"], "Build output for the latest run");
    assert_eq!(value["size_bytes"], 42);
}

#[test]
fn json_format_input_omits_filepath_and_description_for_proto_upload_result() {
    let input = AIAgentInput::ActionResult {
        result: AIAgentActionResult {
            id: "tool-call-1".to_string().into(),
            task_id: TaskId::new("task-1".to_string()),
            result: AIAgentActionResultType::UploadArtifact(UploadArtifactResult::Success {
                artifact_uid: "artifact-123".to_string(),
                filepath: None,
                mime_type: "text/plain".to_string(),
                description: None,
                size_bytes: 42,
            }),
        },
        context: Arc::from([]),
    };

    let mut bytes = Vec::new();
    super::output::json::format_input(&input, &mut bytes).expect("json formatting should work");

    let value: serde_json::Value =
        serde_json::from_slice(&bytes).expect("output should be valid json");

    assert_eq!(value["type"], "tool_result");
    assert_eq!(value["tool"], "upload_artifact");
    assert_eq!(value["artifact_uid"], "artifact-123");
    assert_eq!(value["mime_type"], "text/plain");
    assert_eq!(value["size_bytes"], 42);
    assert!(value.get("filepath").is_none());
    assert!(value.get("description").is_none());
}

fn github_repo(owner: &str, repo: &str) -> GithubRepo {
    GithubRepo::new(owner.to_string(), repo.to_string())
}

fn skill_spec(raw: &str) -> warp_cli::skill::SkillSpec {
    raw.parse().unwrap()
}

// ── build_secret_env_vars tests ──────────────────────────────────────────────

#[test]
#[serial_test::serial]
fn raw_value_only_writes_under_secret_name() {
    std::env::remove_var("MY_SECRET");
    let secrets = HashMap::from([(
        "MY_SECRET".to_string(),
        ManagedSecretValue::raw_value("s3cret"),
    )]);
    let env_vars = build_secret_env_vars(&secrets);
    assert_eq!(
        env_vars.get(&OsString::from("MY_SECRET")),
        Some(&OsString::from("s3cret"))
    );
    assert_eq!(env_vars.len(), 1);
}

#[test]
#[serial_test::serial]
fn anthropic_api_key_writes_anthropic_env_var() {
    std::env::remove_var("ANTHROPIC_API_KEY");
    let secrets = HashMap::from([(
        "my-custom-name".to_string(),
        ManagedSecretValue::anthropic_api_key("sk-ant-test-key"),
    )]);
    let env_vars = build_secret_env_vars(&secrets);
    assert_eq!(
        env_vars.get(&OsString::from("ANTHROPIC_API_KEY")),
        Some(&OsString::from("sk-ant-test-key"))
    );
}

#[test]
#[serial_test::serial]
fn typed_secret_overrides_raw_value_with_same_env_name() {
    std::env::remove_var("ANTHROPIC_API_KEY");
    let typed_key = "sk-ant-typed-key-abcdef";
    let raw_key = "sk-ant-raw-key-ghijkl";
    let secrets = HashMap::from([
        (
            "my-auth".to_string(),
            ManagedSecretValue::anthropic_api_key(typed_key),
        ),
        (
            "ANTHROPIC_API_KEY".to_string(),
            ManagedSecretValue::raw_value(raw_key),
        ),
    ]);
    // Run multiple times to defeat HashMap iteration order flakiness.
    for _ in 0..20 {
        let env_vars = build_secret_env_vars(&secrets);
        assert_eq!(
            env_vars.get(&OsString::from("ANTHROPIC_API_KEY")),
            Some(&OsString::from(typed_key)),
            "Typed secret must always override RawValue with the same env name"
        );
    }
}

#[test]
#[serial_test::serial]
fn bedrock_api_key_writes_all_bedrock_env_vars() {
    std::env::remove_var("AWS_BEARER_TOKEN_BEDROCK");
    std::env::remove_var("CLAUDE_CODE_USE_BEDROCK");
    std::env::remove_var("AWS_REGION");
    let secrets = HashMap::from([
        (
            "bedrock-secret".to_string(),
            ManagedSecretValue::anthropic_bedrock_api_key("token-123", "us-west-2"),
        ),
        (
            "AWS_REGION".to_string(),
            ManagedSecretValue::raw_value("eu-west-1"),
        ),
    ]);
    let env_vars = build_secret_env_vars(&secrets);
    assert_eq!(
        env_vars.get(&OsString::from("AWS_BEARER_TOKEN_BEDROCK")),
        Some(&OsString::from("token-123"))
    );
    assert_eq!(
        env_vars.get(&OsString::from("CLAUDE_CODE_USE_BEDROCK")),
        Some(&OsString::from("1"))
    );
    assert_eq!(
        env_vars.get(&OsString::from("AWS_REGION")),
        Some(&OsString::from("us-west-2")),
        "Typed Bedrock secret should win over RawValue for AWS_REGION"
    );
}

#[test]
#[serial_test::serial]
fn bedrock_access_key_writes_all_aws_env_vars() {
    std::env::remove_var("AWS_ACCESS_KEY_ID");
    std::env::remove_var("AWS_SECRET_ACCESS_KEY");
    std::env::remove_var("AWS_SESSION_TOKEN");
    std::env::remove_var("CLAUDE_CODE_USE_BEDROCK");
    std::env::remove_var("AWS_REGION");
    let secrets = HashMap::from([(
        "bedrock-access".to_string(),
        ManagedSecretValue::anthropic_bedrock_access_key(
            "AKID",
            "secret-key",
            Some("session-tok".to_string()),
            "ap-southeast-1",
        ),
    )]);
    let env_vars = build_secret_env_vars(&secrets);
    assert_eq!(
        env_vars.get(&OsString::from("AWS_ACCESS_KEY_ID")),
        Some(&OsString::from("AKID"))
    );
    assert_eq!(
        env_vars.get(&OsString::from("AWS_SECRET_ACCESS_KEY")),
        Some(&OsString::from("secret-key"))
    );
    assert_eq!(
        env_vars.get(&OsString::from("AWS_SESSION_TOKEN")),
        Some(&OsString::from("session-tok"))
    );
    assert_eq!(
        env_vars.get(&OsString::from("CLAUDE_CODE_USE_BEDROCK")),
        Some(&OsString::from("1"))
    );
    assert_eq!(
        env_vars.get(&OsString::from("AWS_REGION")),
        Some(&OsString::from("ap-southeast-1"))
    );
}

#[test]
#[serial_test::serial]
fn raw_value_skipped_when_process_env_already_set() {
    std::env::set_var("WORKER_TOKEN", "injected-value");
    let secrets = HashMap::from([(
        "WORKER_TOKEN".to_string(),
        ManagedSecretValue::raw_value("managed-value"),
    )]);
    let env_vars = build_secret_env_vars(&secrets);
    // The worker-injected env var wins; env_vars should NOT contain it
    // because the child inherits the process env directly.
    assert!(!env_vars.contains_key(&OsString::from("WORKER_TOKEN")));
    std::env::remove_var("WORKER_TOKEN");
}

#[test]
#[serial_test::serial]
fn worker_injected_env_wins_over_typed_secret() {
    std::env::set_var("ANTHROPIC_API_KEY", "worker-key");
    let secrets = HashMap::from([(
        "my-auth".to_string(),
        ManagedSecretValue::anthropic_api_key("managed-key"),
    )]);
    let env_vars = build_secret_env_vars(&secrets);
    // The typed secret should be skipped entirely; the child inherits
    // ANTHROPIC_API_KEY from the process env.
    assert!(!env_vars.contains_key(&OsString::from("ANTHROPIC_API_KEY")));
    std::env::remove_var("ANTHROPIC_API_KEY");
}

#[test]
#[serial_test::serial]
fn worker_injected_env_skips_entire_bedrock_secret() {
    // Only AWS_REGION is worker-injected; the entire Bedrock secret should
    // be atomically skipped — no partial insertion.
    std::env::set_var("AWS_REGION", "us-east-1");
    std::env::remove_var("AWS_BEARER_TOKEN_BEDROCK");
    std::env::remove_var("CLAUDE_CODE_USE_BEDROCK");
    let secrets = HashMap::from([(
        "bedrock-secret".to_string(),
        ManagedSecretValue::anthropic_bedrock_api_key("token-456", "eu-central-1"),
    )]);
    let env_vars = build_secret_env_vars(&secrets);
    assert!(
        !env_vars.contains_key(&OsString::from("AWS_BEARER_TOKEN_BEDROCK")),
        "Entire Bedrock secret must be skipped when any field is worker-injected"
    );
    assert!(!env_vars.contains_key(&OsString::from("CLAUDE_CODE_USE_BEDROCK")));
    assert!(!env_vars.contains_key(&OsString::from("AWS_REGION")));
    std::env::remove_var("AWS_REGION");
}
