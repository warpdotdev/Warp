use std::{collections::HashMap, sync::Arc};

use crate::{ai::agent::redaction, terminal::model::session::SessionType};
use ::ai::local_models::{LocalModelClient, LocalModelProvider};
use ::ai::local_models::provider::ProviderFactory;
use anyhow::anyhow;
use futures_util::StreamExt;
use uuid::Uuid;
use warp_core::features::FeatureFlag;
use warp_multi_agent_api as api;

use crate::server::server_api::{AIApiError, ServerApi};

use super::{
    convert_to::convert_input, ConvertToAPITypeError, LocalModelRequestConfig, RequestParams,
    ResponseStream,
};

pub async fn generate_multi_agent_output(
    server_api: Arc<ServerApi>,
    mut params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    if let Some(local_model) = params.local_model.take() {
        return generate_local_model_output(local_model, params, cancellation_rx).await;
    }

    let supported_tools = params
        .supported_tools_override
        .take()
        .unwrap_or_else(|| get_supported_tools(&params));
    let supported_cli_agent_tools = get_supported_cli_agent_tools(&params);
    let mut logging_metadata = HashMap::new();
    if let Some(metadata) = params.metadata {
        logging_metadata.insert(
            "is_autodetected_user_query".to_owned(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::BoolValue(
                    metadata.is_autodetected_user_query,
                )),
            },
        );
        logging_metadata.insert(
            "entrypoint".to_owned(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    metadata.entrypoint.entrypoint(),
                )),
            },
        );
        logging_metadata.insert(
            "is_auto_resume_after_error".to_owned(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::BoolValue(
                    metadata.is_auto_resume_after_error,
                )),
            },
        );
    }

    if params.should_redact_secrets {
        redaction::redact_inputs(&mut params.input);
    }

    let mut api_keys = params.api_keys;
    if let Some(api_keys) = &mut api_keys {
        api_keys.allow_use_of_warp_credits = params.allow_use_of_warp_credits_with_byok;
    }

    let request = api::Request {
        task_context: Some(api::request::TaskContext {
            tasks: params.tasks,
        }),
        input: Some(convert_input(params.input)?),
        settings: Some(api::request::Settings {
            model_config: Some(api::request::settings::ModelConfig {
                base: params.model.into(),
                cli_agent: params.cli_agent_model.into(),
                computer_use_agent: params.computer_use_model.into(),
                base_model_context_window_limit: if FeatureFlag::ConfigurableContextWindow
                    .is_enabled()
                {
                    params.context_window_limit.unwrap_or(0)
                } else {
                    0
                },
                ..Default::default()
            }),
            rules_enabled: params.is_memory_enabled,
            warp_drive_context_enabled: params.warp_drive_context_enabled,
            web_context_retrieval_enabled: true,
            supports_parallel_tool_calls: true,
            use_anthropic_text_editor_tools: false,
            planning_enabled: params.planning_enabled,
            supports_create_files: true,
            supported_tools: supported_tools.into_iter().map(Into::into).collect(),
            supports_long_running_commands: true,
            should_preserve_file_content_in_history: true,
            supports_todos_ui: true,
            supports_linked_code_blocks: FeatureFlag::LinkedCodeBlocks.is_enabled(),
            supports_started_child_task_message: true,
            supports_suggest_prompt: true,
            supports_read_image_files: FeatureFlag::ReadImageFiles.is_enabled(),
            supports_reasoning_message: true,
            api_keys,
            autonomy_level: params.autonomy_level.into(),
            isolation_level: params.isolation_level.into(),
            web_search_enabled: params.web_search_enabled,
            supported_cli_agent_tools: supported_cli_agent_tools
                .into_iter()
                .map(Into::into)
                .collect(),
            supports_v4a_file_diffs: FeatureFlag::V4AFileDiffs.is_enabled(),
            supports_summarization_via_message_replacement:
                FeatureFlag::SummarizationViaMessageReplacement.is_enabled(),
            supports_bundled_skills: FeatureFlag::BundledSkills.is_enabled(),
            supports_research_agent: params.research_agent_enabled,
            supports_orchestration_v2: FeatureFlag::OrchestrationV2.is_enabled(),
        }),
        metadata: Some(api::request::Metadata {
            logging: logging_metadata,
            conversation_id: params
                .conversation_token
                .as_ref()
                .map(|token| token.as_str().to_string())
                .unwrap_or_default(),
            ambient_agent_task_id: params
                .ambient_agent_task_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            forked_from_conversation_id: if params.conversation_token.is_none() {
                params
                    .forked_from_conversation_token
                    .map(|token| token.as_str().to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            },
            parent_agent_id: params.parent_agent_id.unwrap_or_default(),
            agent_name: params.agent_name.unwrap_or_default(),
        }),
        existing_suggestions: params
            .existing_suggestions
            .map(|suggestions| suggestions.into()),
        mcp_context: params.mcp_context.map(Into::into),
    };

    let response_stream = server_api.generate_multi_agent_output(&request).await;
    match response_stream {
        Ok(stream) => {
            let output_stream = stream.take_until(cancellation_rx);
            Ok(Box::pin(output_stream))
        }
        Err(e) => {
            let (tx, rx) = async_channel::unbounded();
            let _ = tx.send(Err(e)).await;
            Ok(Box::pin(rx))
        }
    }
}

async fn generate_local_model_output(
    local_model: LocalModelRequestConfig,
    mut params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    if params.should_redact_secrets {
        redaction::redact_inputs(&mut params.input);
    }

    let (tx, rx) = async_channel::unbounded();
    let result = generate_local_model_events(local_model, params).await;
    match result {
        Ok(events) => {
            for event in events {
                let _ = tx.send(Ok(event)).await;
            }
        }
        Err(err) => {
            let _ = tx.send(Err(Arc::new(AIApiError::Other(err)))).await;
        }
    }

    Ok(Box::pin(rx.take_until(cancellation_rx)))
}

async fn generate_local_model_events(
    local_model: LocalModelRequestConfig,
    params: RequestParams,
) -> Result<Vec<api::ResponseEvent>, anyhow::Error> {
    let prompt = local_prompt_from_inputs(&params.input)?;
    let client: Box<dyn LocalModelClient> = match local_model.provider {
        LocalModelProvider::None => {
            return Err(anyhow!("No local model provider configured"));
        }
        LocalModelProvider::Ollama => Box::new(
            ProviderFactory::create_ollama_client(&local_model.base_url, None)
                .map_err(|e: Box<dyn std::error::Error>| anyhow!(e.to_string()))?,
        ),
        LocalModelProvider::LMStudio => Box::new(
            ProviderFactory::create_lmstudio_client(&local_model.base_url, None)
                .map_err(|e: Box<dyn std::error::Error>| anyhow!(e.to_string()))?,
        ),
        LocalModelProvider::CustomOpenAICompatible => {
            return Err(anyhow!("CustomOpenAICompatible provider is not yet supported"));
        }
    };

    let completion = client
        .generate_completion(&prompt, &local_model.model)
        .await
        .map_err(|e: Box<dyn std::error::Error>| anyhow!(e.to_string()))?;

    let request_id = format!("local-request-{}", Uuid::new_v4());
    let conversation_id = params
        .conversation_token
        .as_ref()
        .map(|token| token.as_str().to_string())
        .unwrap_or_else(|| format!("local-conversation-{}", Uuid::new_v4()));
    let task_id = params
        .tasks
        .first()
        .map(|task| task.id.clone())
        .unwrap_or_else(|| "root-task".to_string());

    Ok(vec![
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Init(
                api::response_event::StreamInit {
                    request_id: request_id.clone(),
                    conversation_id,
                    run_id: String::new(),
                },
            )),
        },
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::ClientActions(
                api::response_event::ClientActions {
                    actions: vec![api::ClientAction {
                        action: Some(api::client_action::Action::AddMessagesToTask(
                            api::client_action::AddMessagesToTask {
                                task_id: task_id.clone(),
                                messages: vec![api::Message {
                                    id: format!("local-message-{}", Uuid::new_v4()),
                                    task_id,
                                    request_id,
                                    timestamp: None,
                                    server_message_data: String::new(),
                                    citations: vec![],
                                    message: Some(api::message::Message::AgentOutput(
                                        api::message::AgentOutput { text: completion },
                                    )),
                                }],
                            },
                        )),
                    }],
                },
            )),
        },
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    token_usage: vec![],
                    should_refresh_model_config: false,
                    request_cost: None,
                    conversation_usage_metadata: None,
                    reason: Some(api::response_event::stream_finished::Reason::Done(
                        api::response_event::stream_finished::Done {},
                    )),
                },
            )),
        },
    ])
}

fn local_prompt_from_inputs(
    inputs: &[crate::ai::agent::AIAgentInput],
) -> Result<String, anyhow::Error> {
    let prompt = inputs
        .iter()
        .filter_map(crate::ai::agent::AIAgentInput::user_query)
        .map(|query| query.trim().to_string())
        .filter(|query| !query.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if prompt.is_empty() {
        Err(anyhow!(
            "Local models currently support user prompts only. This request has no prompt text."
        ))
    } else {
        Ok(prompt)
    }
}

fn get_supported_tools(params: &RequestParams) -> Vec<api::ToolType> {
    let mut supported_tools = vec![
        api::ToolType::Grep,
        api::ToolType::FileGlob,
        api::ToolType::FileGlobV2,
        api::ToolType::ReadMcpResource,
        api::ToolType::CallMcpTool,
        api::ToolType::InitProject,
        api::ToolType::OpenCodeReview,
        api::ToolType::RunShellCommand,
        api::ToolType::SuggestNewConversation,
        api::ToolType::Subagent,
        api::ToolType::WriteToLongRunningShellCommand,
        api::ToolType::ReadShellCommandOutput,
        api::ToolType::ReadDocuments,
        api::ToolType::CreateDocuments,
        api::ToolType::EditDocuments,
        api::ToolType::SuggestPrompt,
    ];

    if FeatureFlag::ConversationsAsContext.is_enabled() {
        supported_tools.push(api::ToolType::FetchConversation);
    }

    match params.session_context.session_type() {
        None | Some(SessionType::Local) => {
            supported_tools.extend(&[
                api::ToolType::ReadFiles,
                api::ToolType::ApplyFileDiffs,
                api::ToolType::SearchCodebase,
            ]);

            if FeatureFlag::ArtifactCommand.is_enabled() {
                supported_tools.push(api::ToolType::UploadFileArtifact);
            }
        }
        Some(SessionType::WarpifiedRemote { host_id: Some(_) }) => {
            supported_tools.extend(&[api::ToolType::ReadFiles, api::ToolType::ApplyFileDiffs]);
        }
        Some(SessionType::WarpifiedRemote { host_id: None }) => {
            // Feature flag off or not yet connected — no remote tools.
        }
    }

    if FeatureFlag::AgentModeComputerUse.is_enabled() && params.computer_use_enabled {
        supported_tools.extend(&[api::ToolType::UseComputer]);
        supported_tools.extend(&[api::ToolType::RequestComputerUse])
    }

    if FeatureFlag::PRCommentsSlashCommand.is_enabled() {
        supported_tools.push(api::ToolType::InsertReviewComments);
    }

    if FeatureFlag::ListSkills.is_enabled() {
        supported_tools.push(api::ToolType::ReadSkill);
    }

    if params.orchestration_enabled {
        supported_tools.push(if FeatureFlag::OrchestrationV2.is_enabled() {
            api::ToolType::StartAgentV2
        } else {
            api::ToolType::StartAgent
        });
        if FeatureFlag::RunAgentsTool.is_enabled() && FeatureFlag::OrchestrationV2.is_enabled() {
            supported_tools.push(api::ToolType::RunAgents);
        }
        supported_tools.push(api::ToolType::SendMessageToAgent);
    }

    if FeatureFlag::AskUserQuestion.is_enabled() && params.ask_user_question_enabled {
        supported_tools.push(api::ToolType::AskUserQuestion);
    }

    supported_tools
}

fn get_supported_cli_agent_tools(params: &RequestParams) -> Vec<api::ToolType> {
    let mut supported_cli_agent_tools = vec![
        api::ToolType::WriteToLongRunningShellCommand,
        api::ToolType::ReadShellCommandOutput,
        api::ToolType::Grep,
        api::ToolType::FileGlob,
        api::ToolType::FileGlobV2,
    ];

    if FeatureFlag::TransferControlTool.is_enabled() {
        supported_cli_agent_tools.push(api::ToolType::TransferShellCommandControlToUser);
    }

    match params.session_context.session_type() {
        None | Some(SessionType::Local) => {
            supported_cli_agent_tools
                .extend(&[api::ToolType::ReadFiles, api::ToolType::SearchCodebase]);
        }
        Some(SessionType::WarpifiedRemote { host_id: Some(_) }) => {
            supported_cli_agent_tools.push(api::ToolType::ReadFiles);
        }
        Some(SessionType::WarpifiedRemote { host_id: None }) => {}
    }

    supported_cli_agent_tools
}

#[cfg(test)]
#[path = "impl_tests.rs"]
mod tests;
