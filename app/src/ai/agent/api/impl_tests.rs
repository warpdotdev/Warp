use crate::ai::agent::api::RequestParams;
use crate::ai::blocklist::SessionContext;
use crate::ai::llms::LLMId;
use warp_core::features::FeatureFlag;
use warp_multi_agent_api as api;

use super::get_supported_tools;

fn request_params_with_ask_user_question_enabled(ask_user_question_enabled: bool) -> RequestParams {
    let model = LLMId::from("test-model");

    RequestParams {
        input: vec![],
        conversation_token: None,
        forked_from_conversation_token: None,
        ambient_agent_task_id: None,
        tasks: vec![],
        existing_suggestions: None,
        metadata: None,
        session_context: SessionContext::new_for_test(),
        model: model.clone(),
        coding_model: model.clone(),
        cli_agent_model: model.clone(),
        computer_use_model: model,
        is_memory_enabled: false,
        warp_drive_context_enabled: false,
        mcp_context: None,
        planning_enabled: true,
        should_redact_secrets: false,
        api_keys: None,
        allow_use_of_warp_credits_with_byok: false,
        autonomy_level: api::AutonomyLevel::Supervised,
        isolation_level: api::IsolationLevel::None,
        web_search_enabled: false,
        computer_use_enabled: false,
        ask_user_question_enabled,
        research_agent_enabled: false,
        orchestration_enabled: false,
        supported_tools_override: None,
        parent_agent_id: None,
        agent_name: None,
    }
}

#[test]
fn supported_tools_omits_ask_user_question_when_disabled() {
    let params = request_params_with_ask_user_question_enabled(false);
    let supported_tools = get_supported_tools(&params);

    assert!(!supported_tools.contains(&api::ToolType::AskUserQuestion));
}

#[test]
fn supported_tools_includes_ask_user_question_when_enabled_and_feature_flag_is_enabled() {
    if !FeatureFlag::AskUserQuestion.is_enabled() {
        return;
    }

    let params = request_params_with_ask_user_question_enabled(true);
    let supported_tools = get_supported_tools(&params);

    assert!(supported_tools.contains(&api::ToolType::AskUserQuestion));
}

#[test]
fn supported_tools_include_upload_artifact_when_feature_flag_is_enabled() {
    let _flag = FeatureFlag::ArtifactCommand.override_enabled(true);
    let params = request_params_with_ask_user_question_enabled(false);
    let supported_tools = get_supported_tools(&params);

    assert!(supported_tools.contains(&api::ToolType::UploadFileArtifact));
}

#[test]
fn supported_tools_omit_upload_artifact_when_feature_flag_is_disabled() {
    let _flag = FeatureFlag::ArtifactCommand.override_enabled(false);
    let params = request_params_with_ask_user_question_enabled(false);
    let supported_tools = get_supported_tools(&params);

    assert!(!supported_tools.contains(&api::ToolType::UploadFileArtifact));
}

mod normalize_external_tool_call_ids {
    use std::{collections::HashMap, sync::Arc};

    use warp_core::command::ExitCode;
    use warp_multi_agent_api as api;

    use crate::ai::agent::api::RequestParams;
    use crate::ai::agent::task::TaskId;
    use crate::ai::agent::{
        AIAgentActionResult, AIAgentActionResultType, AIAgentInput, RunningCommand,
        TransferShellCommandControlToUserResult, UserQueryMode,
    };
    use crate::terminal::model::block::BlockId;

    use super::super::{
        build_request, normalize_external_tool_call_id, MAX_EXTERNAL_TOOL_CALL_ID_LEN,
    };
    use super::request_params_with_ask_user_question_enabled;

    fn build_request_for_test(params: RequestParams) -> api::Request {
        build_request(params, vec![], vec![]).expect("request should build")
    }

    fn build_request_with_tool_call_id(tool_call_id: &str) -> api::Request {
        let tool_call_id = tool_call_id.to_string();
        let params = request_params_with_inputs_and_tasks(
            vec![
                AIAgentInput::UserQuery {
                    query: "continue".to_string(),
                    context: Arc::new([]),
                    static_query_type: None,
                    referenced_attachments: HashMap::new(),
                    user_query_mode: UserQueryMode::Normal,
                    running_command: Some(RunningCommand {
                        command: "sleep 1".to_string(),
                        block_id: BlockId::default(),
                        grid_contents: "running".to_string(),
                        cursor: String::new(),
                        requested_command_id: Some(tool_call_id.clone().into()),
                        is_alt_screen_active: false,
                    }),
                    intended_agent: None,
                },
                AIAgentInput::ActionResult {
                    result: AIAgentActionResult {
                        id: tool_call_id.clone().into(),
                        task_id: TaskId::new("task".to_string()),
                        result: AIAgentActionResultType::TransferShellCommandControlToUser(
                            TransferShellCommandControlToUserResult::CommandFinished {
                                block_id: BlockId::default(),
                                output: "done".to_string(),
                                exit_code: ExitCode::from(0),
                            },
                        ),
                    },
                    context: Arc::new([]),
                },
            ],
            vec![task_with_tool_call_history(&tool_call_id)],
        );

        build_request_for_test(params)
    }

    fn request_params_with_inputs_and_tasks(
        inputs: Vec<AIAgentInput>,
        tasks: Vec<api::Task>,
    ) -> RequestParams {
        let mut params = request_params_with_ask_user_question_enabled(false);
        params.input = inputs;
        params.tasks = tasks;
        params
    }

    fn tool_call_message(tool_call_id: &str) -> api::Message {
        api::Message {
            id: "tool-call-message".to_string(),
            task_id: "task".to_string(),
            request_id: "request".to_string(),
            message: Some(api::message::Message::ToolCall(api::message::ToolCall {
                tool_call_id: tool_call_id.to_string(),
                tool: Some(api::message::tool_call::Tool::RunShellCommand(
                    api::message::tool_call::RunShellCommand {
                        command: "echo hi".to_string(),
                        ..Default::default()
                    },
                )),
            })),
            ..Default::default()
        }
    }

    fn tool_call_result_message(tool_call_id: &str) -> api::Message {
        api::Message {
            id: "tool-call-result-message".to_string(),
            task_id: "task".to_string(),
            request_id: "request".to_string(),
            message: Some(api::message::Message::ToolCallResult(
                api::message::ToolCallResult {
                    tool_call_id: tool_call_id.to_string(),
                    result: Some(api::message::tool_call_result::Result::RunShellCommand(
                        api::RunShellCommandResult {
                            command: "echo hi".to_string(),
                            output: "done".to_string(),
                            exit_code: 0,
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                },
            )),
            ..Default::default()
        }
    }

    fn task_with_tool_call_history(tool_call_id: &str) -> api::Task {
        api::Task {
            id: "task".to_string(),
            messages: vec![
                tool_call_message(tool_call_id),
                tool_call_result_message(tool_call_id),
            ],
            ..Default::default()
        }
    }

    fn request_tool_call_ids(request: &api::Request) -> (String, String, String, String) {
        let task_context = request
            .task_context
            .as_ref()
            .expect("task context should exist");
        let task = task_context.tasks.first().expect("task should exist");

        let task_tool_call_id = match task
            .messages
            .first()
            .and_then(|message| message.message.as_ref())
        {
            Some(api::message::Message::ToolCall(tool_call)) => tool_call.tool_call_id.clone(),
            other => panic!("expected tool call message, got {other:?}"),
        };

        let task_tool_call_result_id = match task
            .messages
            .get(1)
            .and_then(|message| message.message.as_ref())
        {
            Some(api::message::Message::ToolCallResult(tool_call_result)) => {
                tool_call_result.tool_call_id.clone()
            }
            other => panic!("expected tool call result message, got {other:?}"),
        };

        let user_inputs = match request
            .input
            .as_ref()
            .and_then(|input| input.r#type.as_ref())
        {
            Some(api::request::input::Type::UserInputs(user_inputs)) => user_inputs,
            other => panic!("expected user inputs request, got {other:?}"),
        };

        let cli_agent_tool_call_id = user_inputs
            .inputs
            .iter()
            .find_map(|user_input| match user_input.input.as_ref() {
                Some(api::request::input::user_inputs::user_input::Input::CliAgentUserQuery(
                    cli_agent_query,
                )) => Some(cli_agent_query.run_shell_command_tool_call_id.clone()),
                _ => None,
            })
            .expect("cli agent user query should exist");

        let action_result_tool_call_id = user_inputs
            .inputs
            .iter()
            .find_map(|user_input| match user_input.input.as_ref() {
                Some(api::request::input::user_inputs::user_input::Input::ToolCallResult(
                    tool_call_result,
                )) => Some(tool_call_result.tool_call_id.clone()),
                _ => None,
            })
            .expect("tool call result input should exist");

        (
            task_tool_call_id,
            task_tool_call_result_id,
            cli_agent_tool_call_id,
            action_result_tool_call_id,
        )
    }

    #[test]
    fn build_request_normalizes_over_limit_tool_call_ids_consistently() {
        let over_limit_tool_call_id = "a".repeat(MAX_EXTERNAL_TOOL_CALL_ID_LEN + 1);
        let normalized_tool_call_id = normalize_external_tool_call_id(&over_limit_tool_call_id);
        let request = build_request_with_tool_call_id(&over_limit_tool_call_id);
        let (
            task_tool_call_id,
            task_tool_call_result_id,
            cli_agent_tool_call_id,
            action_result_tool_call_id,
        ) = request_tool_call_ids(&request);

        assert!(normalized_tool_call_id.len() <= MAX_EXTERNAL_TOOL_CALL_ID_LEN);
        assert_ne!(normalized_tool_call_id, over_limit_tool_call_id);
        assert_eq!(task_tool_call_id, normalized_tool_call_id);
        assert_eq!(task_tool_call_result_id, normalized_tool_call_id);
        assert_eq!(cli_agent_tool_call_id, normalized_tool_call_id);
        assert_eq!(action_result_tool_call_id, normalized_tool_call_id);
    }

    #[test]
    fn build_request_preserves_at_limit_tool_call_ids() {
        let at_limit_tool_call_id = "a".repeat(MAX_EXTERNAL_TOOL_CALL_ID_LEN);
        let request = build_request_with_tool_call_id(&at_limit_tool_call_id);
        let (
            task_tool_call_id,
            task_tool_call_result_id,
            cli_agent_tool_call_id,
            action_result_tool_call_id,
        ) = request_tool_call_ids(&request);

        assert_eq!(task_tool_call_id, at_limit_tool_call_id);
        assert_eq!(task_tool_call_result_id, at_limit_tool_call_id);
        assert_eq!(cli_agent_tool_call_id, at_limit_tool_call_id);
        assert_eq!(action_result_tool_call_id, at_limit_tool_call_id);
    }

    #[test]
    fn normalized_tool_call_ids_are_stable_and_distinct_for_over_limit_inputs() {
        let first = "a".repeat(MAX_EXTERNAL_TOOL_CALL_ID_LEN + 1);
        let second = "b".repeat(MAX_EXTERNAL_TOOL_CALL_ID_LEN + 1);

        let first_normalized = normalize_external_tool_call_id(&first);
        let second_normalized = normalize_external_tool_call_id(&second);

        assert_eq!(first_normalized, normalize_external_tool_call_id(&first));
        assert_ne!(first_normalized, second_normalized);
        assert_ne!(first_normalized, first);
        assert_ne!(second_normalized, second);
        assert!(first_normalized.len() <= MAX_EXTERNAL_TOOL_CALL_ID_LEN);
        assert!(second_normalized.len() <= MAX_EXTERNAL_TOOL_CALL_ID_LEN);
    }
}
