use super::*;
use clap::Parser;

use crate::agent::{AgentCommand, Harness};
use crate::artifact::ArtifactCommand;
// OpenWarp Wave 7-2:`environment` CLI 随 cloud ambient agent 主体物理删。
use crate::harness_support::{HarnessSupportCommand, TaskStatus};
use crate::integration::IntegrationCommand;

#[test]
fn agent_run_accepts_model() {
    let args = Args::try_parse_from([
        "warp", "agent", "run", "--prompt", "hello", "--model", "gpt-4o",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(run_args.model.model.as_deref(), Some("gpt-4o"));
}

#[test]
fn agent_run_accepts_hidden_bedrock_inference_role_flag() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--bedrock-inference-role",
        "arn:aws:iam::123456789012:role/test",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(
        run_args.bedrock_inference_role.as_deref(),
        Some("arn:aws:iam::123456789012:role/test")
    );
}

#[test]
fn model_list_parses() {
    let args = Args::try_parse_from(["warp", "model", "list"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp model list` command");
    };
    let CliCommand::Model(model_cmd) = boxed_cmd.as_ref() else {
        panic!("Expected `warp model` command");
    };

    assert!(matches!(model_cmd, crate::model::ModelCommand::List));
}

#[test]
fn login_parses() {
    let args = Args::try_parse_from(["warp", "login"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp login` command");
    };

    assert!(matches!(boxed_cmd.as_ref(), CliCommand::Login));
}

#[test]
fn agent_run_accepts_file() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--file",
        "config.yaml",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(
        run_args.config_file.file.as_ref().and_then(|p| p.to_str()),
        Some("config.yaml")
    );
}

#[test]
fn agent_run_accepts_idle_on_complete_flag() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--idle-on-complete",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(
        run_args.idle_on_complete,
        Some(humantime::Duration::from(std::time::Duration::from_secs(
            45 * 60
        )))
    );
}

#[test]
fn agent_run_accepts_idle_on_complete_duration() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--idle-on-complete",
        "10m",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(
        run_args.idle_on_complete,
        Some(humantime::Duration::from(std::time::Duration::from_secs(
            10 * 60
        )))
    );
}

#[test]
fn agent_run_rejects_prompt_and_task_id() {
    let result = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--task-id",
        "d1b9b002-a8e1-422a-9016-e62490cb6a59",
    ]);
    assert!(result.is_err());
}

#[test]
fn agent_run_rejects_without_prompt_or_task_id() {
    let result = Args::try_parse_from(["warp", "agent", "run", "--model", "gpt-4o"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("prompt_group") || err_str.contains("required"));
}

#[test]
fn agent_run_accepts_prompt_only() {
    let args = Args::try_parse_from(["warp", "agent", "run", "--prompt", "hello"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(run_args.prompt_arg.prompt.as_deref(), Some("hello"));
    assert!(run_args.prompt_arg.saved_prompt.is_none());
    assert!(run_args.skill.is_none());
    assert!(run_args.task_id.is_none());
}

#[test]
fn agent_run_accepts_saved_prompt_only() {
    let args = Args::try_parse_from(["warp", "agent", "run", "--saved-prompt", "sp-123"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert!(run_args.prompt_arg.prompt.is_none());
    assert_eq!(run_args.prompt_arg.saved_prompt.as_deref(), Some("sp-123"));
    assert!(run_args.skill.is_none());
    assert!(run_args.task_id.is_none());
}

#[test]
fn agent_run_accepts_skill_only() {
    let args = Args::try_parse_from(["warp", "agent", "run", "--skill", "my-skill"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert!(run_args.prompt_arg.prompt.is_none());
    assert!(run_args.skill.is_some());
    assert!(run_args.task_id.is_none());
}

#[test]
fn agent_run_accepts_task_id_only() {
    let args = Args::try_parse_from(["warp", "agent", "run", "--task-id", "tid-456"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert!(run_args.prompt_arg.prompt.is_none());
    assert!(run_args.skill.is_none());
    assert_eq!(run_args.task_id.as_deref(), Some("tid-456"));
}

#[test]
fn agent_run_accepts_prompt_and_skill() {
    let args = Args::try_parse_from([
        "warp", "agent", "run", "--prompt", "do stuff", "--skill", "my-skill",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(run_args.prompt_arg.prompt.as_deref(), Some("do stuff"));
    assert!(run_args.skill.is_some());
}

#[test]
fn agent_run_accepts_saved_prompt_and_skill() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--saved-prompt",
        "sp-1",
        "--skill",
        "my-skill",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert_eq!(run_args.prompt_arg.saved_prompt.as_deref(), Some("sp-1"));
    assert!(run_args.skill.is_some());
}

#[test]
fn agent_run_rejects_saved_prompt_and_task_id() {
    let result = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--saved-prompt",
        "sp-1",
        "--task-id",
        "tid-1",
    ]);
    assert!(result.is_err());
}

#[test]
fn agent_run_rejects_file_and_task_id() {
    let result = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--task-id",
        "tid-1",
        "--file",
        "config.yaml",
    ]);
    assert!(result.is_err());
}

#[test]
fn agent_run_accepts_skill_and_task_id() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--skill",
        "my-skill",
        "--task-id",
        "tid-1",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert!(run_args.prompt_arg.prompt.is_none());
    assert!(run_args.skill.is_some());
    assert_eq!(run_args.task_id.as_deref(), Some("tid-1"));
}

#[test]
fn agent_run_rejects_prompt_and_saved_prompt() {
    let result = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--saved-prompt",
        "sp-1",
    ]);
    assert!(result.is_err());
}

#[test]
fn artifact_upload_accepts_run_id() {
    let args = Args::try_parse_from([
        "warp",
        "artifact",
        "upload",
        "path/to/file.json",
        "--run-id",
        "run-123",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp artifact upload` command");
    };
    let CliCommand::Artifact(ArtifactCommand::Upload(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp artifact upload` command");
    };

    assert_eq!(args.path.to_str(), Some("path/to/file.json"));
    assert_eq!(args.run_id.as_deref(), Some("run-123"));
    assert_eq!(args.conversation_id, None);
}

#[test]
fn artifact_help_hides_upload_but_keeps_download_visible() {
    warp_core::features::mark_initialized();

    let mut command = Args::clap_command();
    command.build();

    let artifact = command
        .find_subcommand("artifact")
        .expect("artifact subcommand should exist");
    let upload = artifact
        .find_subcommand("upload")
        .expect("upload subcommand should exist");
    let download = artifact
        .find_subcommand("download")
        .expect("download subcommand should exist");
    let get = artifact
        .find_subcommand("get")
        .expect("get subcommand should exist");

    assert!(upload.is_hide_set());
    assert!(!get.is_hide_set());
    assert!(!download.is_hide_set());

    let visible_subcommands: Vec<_> = artifact
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .map(|subcommand| subcommand.get_name())
        .collect();
    assert!(visible_subcommands.contains(&"get"));

    assert!(visible_subcommands.contains(&"download"));
    assert!(!visible_subcommands.contains(&"upload"));
}

#[test]
fn run_help_hides_message_when_orchestration_v2_disabled() {
    warp_core::features::mark_initialized();

    let mut command = Args::clap_command();
    command.build();

    let run = command
        .find_subcommand("run")
        .expect("run subcommand should exist");
    let message = run
        .find_subcommand("message")
        .expect("message subcommand should exist");

    assert!(message.is_hide_set());

    let visible_subcommands: Vec<_> = run
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .map(|subcommand| subcommand.get_name())
        .collect();

    assert!(!visible_subcommands.contains(&"message"));
}

#[test]
fn raw_command_keeps_message_visible_before_runtime_help_customization() {
    let mut command = <Args as clap::CommandFactory>::command();
    command.build();

    let run = command
        .find_subcommand("run")
        .expect("run subcommand should exist");
    let message = run
        .find_subcommand("message")
        .expect("message subcommand should exist");

    assert!(!message.is_hide_set());

    let visible_subcommands: Vec<_> = run
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .map(|subcommand| subcommand.get_name())
        .collect();

    assert!(visible_subcommands.contains(&"message"));
}

#[test]
fn artifact_upload_accepts_run_id_and_description() {
    let args = Args::try_parse_from([
        "warp",
        "artifact",
        "upload",
        "path/to/file.json",
        "--run-id",
        "run-123",
        "--description",
        "Test artifact",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp artifact upload` command");
    };
    let CliCommand::Artifact(ArtifactCommand::Upload(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp artifact upload` command");
    };

    assert_eq!(args.run_id.as_deref(), Some("run-123"));
    assert_eq!(args.conversation_id, None);
    assert_eq!(args.description.as_deref(), Some("Test artifact"));
}

#[test]
fn artifact_upload_accepts_conversation_id_and_description() {
    let args = Args::try_parse_from([
        "warp",
        "artifact",
        "upload",
        "path/to/file.json",
        "--conversation-id",
        "conversation-123",
        "--description",
        "Test artifact",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp artifact upload` command");
    };
    let CliCommand::Artifact(ArtifactCommand::Upload(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp artifact upload` command");
    };

    assert_eq!(args.path.to_str(), Some("path/to/file.json"));
    assert_eq!(args.run_id, None);
    assert_eq!(args.conversation_id.as_deref(), Some("conversation-123"));
    assert_eq!(args.description.as_deref(), Some("Test artifact"));
}

#[test]
fn artifact_upload_accepts_missing_association_target_for_env_fallback() {
    let args = Args::try_parse_from(["warp", "artifact", "upload", "path/to/file.json"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp artifact upload` command");
    };
    let CliCommand::Artifact(ArtifactCommand::Upload(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp artifact upload` command");
    };

    assert_eq!(args.path.to_str(), Some("path/to/file.json"));
    assert_eq!(args.run_id, None);
    assert_eq!(args.conversation_id, None);
}

#[test]
fn artifact_upload_rejects_both_association_targets() {
    let err = Args::try_parse_from([
        "warp",
        "artifact",
        "upload",
        "path/to/file.json",
        "--run-id",
        "run-123",
        "--conversation-id",
        "conversation-123",
    ])
    .unwrap_err();
    let err = err.to_string();

    assert!(err.contains("--run-id"));
    assert!(err.contains("--conversation-id"));
}

#[test]
fn artifact_download_parses_artifact_id_and_out() {
    let args = Args::try_parse_from([
        "warp",
        "artifact",
        "download",
        "artifact-123",
        "--out",
        "downloads/file.json",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp artifact download` command");
    };
    let CliCommand::Artifact(ArtifactCommand::Download(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp artifact download` command");
    };

    assert_eq!(args.artifact_uid, "artifact-123");
    assert_eq!(
        args.out.as_ref().and_then(|path| path.to_str()),
        Some("downloads/file.json")
    );
}
#[test]
fn artifact_get_parses_artifact_uid() {
    let args = Args::try_parse_from(["warp", "artifact", "get", "artifact-123"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp artifact get` command");
    };
    let CliCommand::Artifact(ArtifactCommand::Get(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp artifact get` command");
    };

    assert_eq!(args.artifact_uid, "artifact-123");
}

#[test]
fn integration_create_accepts_file() {
    let args = Args::try_parse_from([
        "warp",
        "integration",
        "create",
        "slack",
        "--file",
        "integration.yml",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp integration create` command");
    };
    let CliCommand::Integration(IntegrationCommand::Create(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp integration create` command");
    };

    assert_eq!(
        args.config_file.file.as_ref().and_then(|p| p.to_str()),
        Some("integration.yml")
    );
}

#[test]
fn integration_create_accepts_model() {
    let args = Args::try_parse_from([
        "warp",
        "integration",
        "create",
        "slack",
        "--model",
        "gpt-4o",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp integration create` command");
    };
    let CliCommand::Integration(IntegrationCommand::Create(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp integration create` command");
    };

    assert_eq!(args.model.model.as_deref(), Some("gpt-4o"));
}

#[test]
fn integration_update_accepts_file() {
    let args = Args::try_parse_from([
        "warp",
        "integration",
        "update",
        "slack",
        "--file",
        "integration.json",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp integration update` command");
    };
    let CliCommand::Integration(IntegrationCommand::Update(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp integration update` command");
    };

    assert_eq!(
        args.config_file.file.as_ref().and_then(|p| p.to_str()),
        Some("integration.json")
    );
}

#[test]
fn integration_update_accepts_model() {
    let args = Args::try_parse_from([
        "warp",
        "integration",
        "update",
        "slack",
        "--model",
        "gpt-4o",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp integration update` command");
    };
    let CliCommand::Integration(IntegrationCommand::Update(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp integration update` command");
    };

    assert_eq!(args.model.model.as_deref(), Some("gpt-4o"));
}

#[test]
fn integration_create_accepts_mcp_json() {
    let json = r#"{"my-server":{"command":"echo"}}"#;

    let args =
        Args::try_parse_from(["warp", "integration", "create", "slack", "--mcp", json]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp integration create` command");
    };
    let CliCommand::Integration(IntegrationCommand::Create(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp integration create` command");
    };

    assert!(matches!(
        args.mcp_specs.as_slice(),
        [crate::mcp::MCPSpec::Json(parsed_json)] if parsed_json == json
    ));
}

#[test]
fn integration_update_accepts_mcp_json_and_remove_mcp() {
    let json = r#"{"my-server":{"command":"echo"}}"#;

    let args = Args::try_parse_from([
        "warp",
        "integration",
        "update",
        "slack",
        "--mcp",
        json,
        "--remove-mcp",
        "existing",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp integration update` command");
    };
    let CliCommand::Integration(IntegrationCommand::Update(args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp integration update` command");
    };

    assert!(matches!(
        args.mcp_specs.as_slice(),
        [crate::mcp::MCPSpec::Json(parsed_json)] if parsed_json == json
    ));
    assert_eq!(args.remove_mcp, vec!["existing".to_string()]);
}

// OpenWarp Wave 7-2:environment_image_list_parses / environment_create_accepts_description /
// environment_create_description_max_length / environment_update_accepts_description /
// environment_update_accepts_remove_description 随 cloud ambient agent 主体子系统物理删。

#[test]
fn agent_run_accepts_computer_use_flag() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--computer-use",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert!(run_args.computer_use.computer_use);
    assert!(!run_args.computer_use.no_computer_use);
    assert_eq!(run_args.computer_use.computer_use_override(), Some(true));
}

#[test]
fn agent_run_accepts_no_computer_use_flag() {
    let args = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--no-computer-use",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert!(!run_args.computer_use.computer_use);
    assert!(run_args.computer_use.no_computer_use);
    assert_eq!(run_args.computer_use.computer_use_override(), Some(false));
}

#[test]
fn agent_run_rejects_both_computer_use_flags() {
    let result = Args::try_parse_from([
        "warp",
        "agent",
        "run",
        "--prompt",
        "hello",
        "--computer-use",
        "--no-computer-use",
    ]);

    assert!(result.is_err());
}

#[test]
fn agent_run_defaults_to_no_computer_use_override() {
    let args = Args::try_parse_from(["warp", "agent", "run", "--prompt", "hello"]).unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected `warp agent run` command");
    };
    let CliCommand::Agent(AgentCommand::Run(run_args)) = boxed_cmd.as_ref() else {
        panic!("Expected `warp agent run` command");
    };

    assert!(!run_args.computer_use.computer_use);
    assert!(!run_args.computer_use.no_computer_use);
    assert_eq!(run_args.computer_use.computer_use_override(), None);
}
#[test]
fn harness_parse_orchestration_harness_accepts_aliases() {
    assert_eq!(
        Harness::parse_orchestration_harness("claude-code"),
        Some(Harness::Claude)
    );
    assert_eq!(
        Harness::parse_orchestration_harness("open_code"),
        Some(Harness::OpenCode)
    );
}

#[test]
fn harness_parse_local_child_harness_rejects_oz() {
    assert_eq!(Harness::parse_local_child_harness("oz"), None);
    assert_eq!(
        Harness::parse_local_child_harness("opencode"),
        Some(Harness::OpenCode)
    );
}

#[test]
fn finish_task_accepts_status_success() {
    let args = Args::try_parse_from([
        "warp",
        "harness-support",
        "--run-id",
        "run-1",
        "finish-task",
        "--status",
        "success",
        "--summary",
        "all good",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected harness-support command");
    };
    let CliCommand::HarnessSupport(hs_args) = boxed_cmd.as_ref() else {
        panic!("Expected harness-support command");
    };
    let HarnessSupportCommand::FinishTask(finish_args) = &hs_args.command else {
        panic!("Expected finish-task subcommand");
    };

    assert_eq!(finish_args.status, TaskStatus::Success);
    assert_eq!(finish_args.summary, "all good");
}

#[test]
fn finish_task_accepts_status_failure() {
    let args = Args::try_parse_from([
        "warp",
        "harness-support",
        "--run-id",
        "run-1",
        "finish-task",
        "--status",
        "failure",
        "--summary",
        "something broke",
    ])
    .unwrap();

    let Some(Command::CommandLine(boxed_cmd)) = args.command else {
        panic!("Expected harness-support command");
    };
    let CliCommand::HarnessSupport(hs_args) = boxed_cmd.as_ref() else {
        panic!("Expected harness-support command");
    };
    let HarnessSupportCommand::FinishTask(finish_args) = &hs_args.command else {
        panic!("Expected finish-task subcommand");
    };

    assert_eq!(finish_args.status, TaskStatus::Failure);
    assert_eq!(finish_args.summary, "something broke");
}

#[test]
fn finish_task_rejects_invalid_status() {
    let result = Args::try_parse_from([
        "warp",
        "harness-support",
        "--run-id",
        "run-1",
        "finish-task",
        "--status",
        "maybe",
        "--summary",
        "who knows",
    ]);
    assert!(result.is_err());
}

#[test]
fn finish_task_rejects_missing_status() {
    let result = Args::try_parse_from([
        "warp",
        "harness-support",
        "--run-id",
        "run-1",
        "finish-task",
        "--summary",
        "no status",
    ]);
    assert!(result.is_err());
}
