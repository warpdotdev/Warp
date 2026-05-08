use crate::ai::agent::api::RequestParams;
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent::task::TaskId;
use crate::ai::blocklist::SessionContext;
use crate::ai::llms::LLMId;
use crate::ai::llms::LLMProvider;
use warp_core::features::FeatureFlag;
use warp_multi_agent_api as api;

use super::{get_supported_tools, should_use_local_openai_responses_backend};

fn request_params_with_ask_user_question_enabled(ask_user_question_enabled: bool) -> RequestParams {
    let model = LLMId::from("test-model");

    RequestParams {
        conversation_id: AIConversationId::new(),
        input: vec![],
        target_task_id: Some(TaskId::new("task-id".to_string())),
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
        context_window_limit: None,
        mcp_context: None,
        planning_enabled: true,
        should_redact_secrets: false,
        api_keys: None,
        allow_use_of_warp_credits_with_byok: false,
        local_openai_responses_backend_enabled: false,
        local_openai_api_key: None,
        local_openai_base_url: None,
        local_openai_model_override: None,
        model_provider: LLMProvider::Unknown,
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

#[test]
fn local_openai_backend_requires_opt_in_and_openai_provider() {
    let mut params = request_params_with_ask_user_question_enabled(false);
    params.local_openai_responses_backend_enabled = true;
    assert!(!should_use_local_openai_responses_backend(&params));

    params.model_provider = LLMProvider::OpenAI;
    assert!(should_use_local_openai_responses_backend(&params));
}

#[test]
fn local_openai_backend_allows_remote_sessions() {
    let mut params = request_params_with_ask_user_question_enabled(false);
    params.local_openai_responses_backend_enabled = true;
    params.model_provider = LLMProvider::OpenAI;
    params.session_context =
        SessionContext::new_remote_for_test(Some(warp_core::HostId::new("host-1".to_string())));

    assert!(should_use_local_openai_responses_backend(&params));
}
