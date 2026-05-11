use async_stream::stream;
use chrono::Utc;
use http_client::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
use warp_multi_agent_api::{
    self as api, client_action, message, response_event, response_event::stream_finished,
};

use crate::ai::agent::{AIAgentContext, AIAgentInput, AnyFileContent, UserQueryMode};

use super::{RequestParams, ResponseStream};

const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENROUTER_REFERER: &str = "https://warper.dev";
const DEFAULT_OPENROUTER_MODEL_ID: &str = "openrouter/auto";
const OPENROUTER_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Debug, Serialize)]
struct OpenRouterMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    tools: Vec<OpenRouterTool>,
    tool_choice: &'static str,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OpenRouterTool {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenRouterToolFunction,
}

#[derive(Debug, Serialize)]
struct OpenRouterToolFunction {
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChatResponse {
    #[serde(default)]
    choices: Vec<OpenRouterChoice>,
    #[serde(default)]
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterAssistantMessage,
}

#[derive(Debug, Deserialize)]
struct OpenRouterAssistantMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenRouterToolCall>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterToolCall {
    id: String,
    function: OpenRouterFunctionCall,
}

#[derive(Debug, Deserialize)]
struct OpenRouterFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct RunShellCommandArgs {
    command: String,
    #[serde(default)]
    is_read_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
}

#[derive(Debug)]
enum OpenRouterError {
    MissingApiKey,
    Request(String),
    Status { status: StatusCode, body: String },
    Response(String),
    EmptyResponse,
}

impl OpenRouterError {
    fn is_invalid_api_key(&self) -> bool {
        matches!(
            self,
            Self::MissingApiKey
                | Self::Status {
                    status: StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN,
                    ..
                }
        )
    }

    fn user_message(&self) -> String {
        match self {
            Self::MissingApiKey => {
                "OpenRouter API key is missing. Add it in Settings > OpenRouter, then try again."
                    .to_owned()
            }
            Self::Status { status, body } => {
                let body = body.trim();
                if body.is_empty() {
                    format!("OpenRouter request failed with status {status}.")
                } else {
                    format!("OpenRouter request failed with status {status}:\n{body}")
                }
            }
            Self::Request(error) => format!("OpenRouter request failed: {error}"),
            Self::Response(error) => format!("OpenRouter response could not be read: {error}"),
            Self::EmptyResponse => "OpenRouter returned an empty response.".to_owned(),
        }
    }

    fn finish_reason(&self, model_name: String) -> stream_finished::Reason {
        if self.is_invalid_api_key() {
            stream_finished::Reason::InvalidApiKey(stream_finished::InvalidApiKey {
                provider: api::LlmProvider::Openrouter.into(),
                model_name,
            })
        } else {
            stream_finished::Reason::InternalError(stream_finished::InternalError {
                message: self.user_message(),
            })
        }
    }
}

pub fn generate_openrouter_output(params: RequestParams) -> ResponseStream {
    Box::pin(stream! {
        let task_id = params.primary_task_id.clone();
        let model_id = effective_openrouter_model(&params);
        let request_id = Uuid::new_v4().to_string();
        let conversation_id = params
            .conversation_token
            .as_ref()
            .map(|token| token.as_str().to_owned())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        yield Ok(stream_init_event(
            conversation_id.clone(),
            request_id.clone(),
        ));
        if should_create_openrouter_task(&params, &task_id) {
            yield Ok(create_task_event(&task_id));
        }

        let result = request_openrouter_completion(&params, &model_id).await;
        match result {
            Ok(completion) => {
                yield Ok(add_messages_event(
                    task_id,
                    request_id,
                    model_id.clone(),
                    &params.input,
                    completion.text,
                    completion.tool_calls,
                ));
                yield Ok(done_event(completion.usage, model_id));
            }
            Err(error) => {
                yield Ok(add_messages_event(
                    task_id,
                    request_id,
                    model_id.clone(),
                    &params.input,
                    Some(error.user_message()),
                    Vec::new(),
                ));
                yield Ok(finished_event(error.finish_reason(model_id)));
            }
        }
    })
}

async fn request_openrouter_completion(
    params: &RequestParams,
    model_id: &str,
) -> Result<OpenRouterCompletion, OpenRouterError> {
    let api_key = params
        .api_keys
        .as_ref()
        .map(|keys| keys.open_router.trim())
        .filter(|key| !key.is_empty())
        .ok_or(OpenRouterError::MissingApiKey)?;

    let request = OpenRouterChatRequest {
        model: model_id.to_owned(),
        messages: build_openrouter_messages(params),
        tools: openrouter_tools(),
        tool_choice: "auto",
        stream: false,
    };

    log::info!(
        "OpenRouter request starting: model={model_id}, messages={}, tools={}, task_id={}, timeout_secs={}",
        request.messages.len(),
        request.tools.len(),
        params.primary_task_id,
        OPENROUTER_REQUEST_TIMEOUT.as_secs(),
    );

    let response = http_client::Client::new()
        .post(OPENROUTER_CHAT_COMPLETIONS_URL)
        .bearer_auth(api_key)
        .header("HTTP-Referer", OPENROUTER_REFERER)
        .header("X-OpenRouter-Title", "Warper")
        .json(&request)
        .timeout(OPENROUTER_REQUEST_TIMEOUT)
        .prevent_sleep("OpenRouter agent request in-progress")
        .send()
        .await
        .map_err(|error| {
            log::warn!("OpenRouter request transport failed: model={model_id}, error={error}");
            OpenRouterError::Request(error.to_string())
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|error| {
        log::warn!("OpenRouter response body read failed: model={model_id}, error={error}");
        OpenRouterError::Response(error.to_string())
    })?;

    log::info!(
        "OpenRouter response received: model={model_id}, status={status}, body_bytes={}",
        body.len(),
    );

    if !status.is_success() {
        log::warn!(
            "OpenRouter request failed: model={model_id}, status={status}, body={}",
            truncate_for_log(&body),
        );
        return Err(OpenRouterError::Status { status, body });
    }

    let parsed: OpenRouterChatResponse = serde_json::from_str(&body).map_err(|error| {
        log::warn!(
            "OpenRouter response parse failed: model={model_id}, error={error}, body={}",
            truncate_for_log(&body),
        );
        OpenRouterError::Response(error.to_string())
    })?;
    let mut choices = parsed.choices.into_iter();
    let Some(choice) = choices.next() else {
        return Err(OpenRouterError::EmptyResponse);
    };
    let text = choice
        .message
        .content
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    let tool_calls = choice.message.tool_calls;

    if text.is_none() && tool_calls.is_empty() {
        return Err(OpenRouterError::EmptyResponse);
    }

    Ok(OpenRouterCompletion {
        text,
        tool_calls,
        usage: parsed.usage,
    })
}

fn openrouter_tools() -> Vec<OpenRouterTool> {
    vec![OpenRouterTool {
        kind: "function",
        function: OpenRouterToolFunction {
            name: "run_shell_command",
            description: "Run a shell command in the user's active terminal session. Use this for commands that inspect the project, run tests, or perform an explicit change requested by the user.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The exact shell command to run."
                    },
                    "is_read_only": {
                        "type": "boolean",
                        "description": "True when the command only reads or inspects state."
                    }
                },
                "required": ["command"]
            }),
        },
    }]
}

fn openrouter_tool_call_to_message(
    task_id: &str,
    request_id: &str,
    tool_call: OpenRouterToolCall,
    timestamp: Option<prost_types::Timestamp>,
) -> Option<api::Message> {
    if tool_call.function.name != "run_shell_command" {
        return None;
    }

    let args: RunShellCommandArgs = serde_json::from_str(&tool_call.function.arguments).ok()?;
    let command = args.command.trim().to_owned();
    if command.is_empty() {
        return None;
    }

    let is_read_only = args
        .is_read_only
        .unwrap_or_else(|| command_looks_read_only(&command));
    let is_risky = !is_read_only;

    Some(api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        request_id: request_id.to_owned(),
        timestamp,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call.id,
            tool: Some(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command,
                    is_read_only,
                    uses_pager: false,
                    citations: vec![],
                    is_risky,
                    wait_until_complete_value: None,
                    risk_category: 0,
                },
            )),
        })),
    })
}

fn command_looks_read_only(command: &str) -> bool {
    let command = command.trim_start();
    let first = command
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_');

    matches!(
        first,
        "cat"
            | "cd"
            | "echo"
            | "find"
            | "git"
            | "grep"
            | "head"
            | "ls"
            | "pwd"
            | "rg"
            | "sed"
            | "tail"
            | "tree"
            | "wc"
            | "which"
    )
}

fn effective_openrouter_model(params: &RequestParams) -> String {
    params
        .open_router_model
        .as_ref()
        .map(|model| model.trim())
        .filter(|model| !model.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            let selected = params.model.as_str();
            if selected == "auto" || selected.is_empty() {
                DEFAULT_OPENROUTER_MODEL_ID.to_owned()
            } else {
                selected.to_owned()
            }
        })
}

fn build_openrouter_messages(params: &RequestParams) -> Vec<OpenRouterMessage> {
    let mut system = "You are Warper, an AI agent inside an agentic terminal. Answer directly through OpenRouter. Be concise, practical, and terminal-aware. Use the run_shell_command tool when you need command output or when the user asks you to run something. Prefer read-only inspection commands before making changes. Do not claim that you ran a command unless you used the tool and saw the result.".to_owned();

    if let Some(cwd) = params
        .session_context
        .current_working_directory()
        .as_deref()
    {
        system.push_str("\nCurrent working directory: ");
        system.push_str(cwd);
    }

    let mut user_content = params
        .input
        .iter()
        .map(input_to_prompt_text)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if user_content.trim().is_empty() {
        user_content = "Continue the conversation.".to_owned();
    }

    let mut messages = vec![OpenRouterMessage {
        role: "system",
        content: system,
    }];
    messages.extend(openrouter_messages_from_tasks(&params.tasks));
    messages.push(OpenRouterMessage {
        role: "user",
        content: user_content,
    });
    messages
}

fn input_to_prompt_text(input: &AIAgentInput) -> String {
    match input {
        AIAgentInput::UserQuery { query, context, .. } => {
            prompt_text_with_context(query.clone(), context)
        }
        AIAgentInput::ActionResult { result, context } => {
            prompt_text_with_context(format!("Tool result:\n{result}"), context)
        }
        AIAgentInput::AutoCodeDiffQuery { query, context } => {
            prompt_text_with_context(format!("Code assistance request:\n{query}"), context)
        }
        AIAgentInput::SummarizeConversation { prompt } => {
            format!(
                "Summarize the current conversation. Additional instructions: {}",
                prompt.clone().unwrap_or_default()
            )
        }
        AIAgentInput::PassiveSuggestionResult {
            suggestion,
            context,
            ..
        } => prompt_text_with_context(
            format!("Passive suggestion result:\n{suggestion:?}"),
            context,
        ),
        _ => input.user_query().unwrap_or_else(|| input.to_string()),
    }
}

fn prompt_text_with_context(mut prompt: String, context: &[AIAgentContext]) -> String {
    if let Some(context_text) = context_to_prompt_text(context) {
        if !prompt.trim().is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(&context_text);
    }
    prompt
}

fn context_to_prompt_text(context: &[AIAgentContext]) -> Option<String> {
    let sections = context
        .iter()
        .filter_map(context_item_to_prompt_text)
        .collect::<Vec<_>>();

    (!sections.is_empty()).then(|| {
        format!(
            "Attached context for this request:\n\n{}",
            sections.join("\n\n")
        )
    })
}

fn context_item_to_prompt_text(context: &AIAgentContext) -> Option<String> {
    match context {
        AIAgentContext::Block(block) => {
            let mut text = String::new();
            if block.command.trim().is_empty() {
                text.push_str("Attached terminal output");
            } else {
                text.push_str("Attached terminal command");
                if let Some(pwd) = &block.pwd {
                    text.push_str(&format!(" from `{pwd}`"));
                }
                text.push_str(&format!(
                    ":\nCommand:\n```sh\n{}\n```\nExit code: {}",
                    block.command,
                    block.exit_code.value()
                ));
            }

            if !block.output.is_empty() {
                text.push_str(&format!("\nOutput:\n```text\n{}\n```", block.output));
            }
            Some(text)
        }
        AIAgentContext::SelectedText(text) => {
            Some(format!("Attached selected text:\n```text\n{}\n```", text))
        }
        AIAgentContext::Directory { pwd, home_dir, .. } => {
            let mut parts = Vec::new();
            if let Some(pwd) = pwd {
                parts.push(format!("Current working directory: {pwd}"));
            }
            if let Some(home_dir) = home_dir {
                parts.push(format!("Home directory: {home_dir}"));
            }
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        AIAgentContext::Git { head, branch } => {
            let branch = branch
                .as_ref()
                .map(|branch| format!("\nGit branch: {branch}"))
                .unwrap_or_default();
            Some(format!("Git head: {head}{branch}"))
        }
        AIAgentContext::File(file) => Some(file_context_to_prompt_text("Attached file", file)),
        AIAgentContext::ProjectRules { active_rules, .. } => {
            let rules = active_rules
                .iter()
                .map(|file| file_context_to_prompt_text("Project rule", file))
                .collect::<Vec<_>>();
            (!rules.is_empty()).then(|| rules.join("\n\n"))
        }
        AIAgentContext::ExecutionEnvironment(_)
        | AIAgentContext::CurrentTime { .. }
        | AIAgentContext::Image(_)
        | AIAgentContext::Codebase { .. }
        | AIAgentContext::Skills { .. } => None,
    }
}

fn file_context_to_prompt_text(label: &str, file: &crate::ai::agent::FileContext) -> String {
    let content = match &file.content {
        AnyFileContent::StringContent(content) => content.clone(),
        AnyFileContent::BinaryContent(content) => {
            format!("<binary content: {} bytes>", content.len())
        }
    };

    format!("{label}: {}\n```text\n{}\n```", file.file_name, content)
}

#[allow(deprecated)]
fn input_context_to_api(context: &[AIAgentContext]) -> Option<api::InputContext> {
    let mut api_context = api::InputContext::default();
    let mut has_context = false;

    for context in context {
        match context {
            AIAgentContext::Block(block) => {
                api_context
                    .executed_shell_commands
                    .push(api::ExecutedShellCommand {
                        command: block.command.clone(),
                        output: block.output.clone(),
                        exit_code: block.exit_code.value(),
                        command_id: block.id.to_string(),
                        started_ts: None,
                        finished_ts: None,
                        is_auto_attached: block.is_auto_attached,
                    });
                has_context = true;
            }
            AIAgentContext::SelectedText(text) => {
                api_context
                    .selected_text
                    .push(api::input_context::SelectedText { text: text.clone() });
                has_context = true;
            }
            AIAgentContext::Directory {
                pwd,
                home_dir,
                are_file_symbols_indexed,
            } => {
                api_context.directory = Some(api::input_context::Directory {
                    pwd: pwd.clone().unwrap_or_default(),
                    home: home_dir.clone().unwrap_or_default(),
                    pwd_file_symbols_indexed: *are_file_symbols_indexed,
                });
                has_context = true;
            }
            AIAgentContext::Git { head, branch } => {
                api_context.git = Some(api::input_context::Git {
                    head: head.clone(),
                    branch: branch.clone().unwrap_or_default(),
                });
                has_context = true;
            }
            AIAgentContext::ExecutionEnvironment(_)
            | AIAgentContext::CurrentTime { .. }
            | AIAgentContext::Image(_)
            | AIAgentContext::Codebase { .. }
            | AIAgentContext::ProjectRules { .. }
            | AIAgentContext::File(_)
            | AIAgentContext::Skills { .. } => {}
        }
    }

    has_context.then_some(api_context)
}

fn stream_init_event(conversation_id: String, request_id: String) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(response_event::Type::Init(response_event::StreamInit {
            run_id: conversation_id.clone(),
            conversation_id,
            request_id,
        })),
    }
}

fn should_create_openrouter_task(params: &RequestParams, task_id: &str) -> bool {
    !task_id.is_empty() && !params.tasks.iter().any(|task| task.id == task_id)
}

fn create_task_event(task_id: &str) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(response_event::Type::ClientActions(
            response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(client_action::Action::CreateTask(
                        client_action::CreateTask {
                            task: Some(api::Task {
                                id: task_id.to_owned(),
                                messages: vec![],
                                dependencies: None,
                                description: String::new(),
                                summary: String::new(),
                                server_data: String::new(),
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

fn openrouter_messages_from_tasks(tasks: &[api::Task]) -> Vec<OpenRouterMessage> {
    tasks
        .iter()
        .flat_map(|task| task.messages.iter())
        .filter_map(api_message_to_openrouter_message)
        .collect()
}

fn api_message_to_openrouter_message(message: &api::Message) -> Option<OpenRouterMessage> {
    match message.message.as_ref()? {
        message::Message::UserQuery(user_query) => Some(OpenRouterMessage {
            role: "user",
            content: prompt_text_with_context(
                user_query.query.clone(),
                &super::convert_conversation::convert_input_context(user_query.context.as_ref()),
            ),
        }),
        message::Message::SystemQuery(query) => {
            system_query_to_prompt_text(query).map(|content| OpenRouterMessage {
                role: "user",
                content,
            })
        }
        message::Message::ToolCallResult(result) => Some(OpenRouterMessage {
            role: "user",
            content: format!("Tool result:\n{}", tool_call_result_to_prompt_text(result)),
        }),
        message::Message::AgentOutput(output) => Some(OpenRouterMessage {
            role: "assistant",
            content: output.text.clone(),
        }),
        message::Message::ToolCall(tool_call) => {
            tool_call_to_prompt_text(tool_call).map(|content| OpenRouterMessage {
                role: "assistant",
                content,
            })
        }
        _ => None,
    }
}

fn system_query_to_prompt_text(query: &message::SystemQuery) -> Option<String> {
    match query.r#type.as_ref()? {
        message::system_query::Type::AutoCodeDiff(query) => Some(query.query.clone()),
        message::system_query::Type::CreateNewProject(query) => Some(query.query.clone()),
        message::system_query::Type::CloneRepository(query) => Some(format!("Clone {}", query.url)),
        message::system_query::Type::SummarizeConversation(query) => Some(query.prompt.clone()),
        message::system_query::Type::FetchReviewComments(query) => {
            Some(format!("Fetch review comments for {}", query.repo_path))
        }
        message::system_query::Type::ResumeConversation(_)
        | message::system_query::Type::GeneratePassiveSuggestions(_) => None,
    }
}

fn tool_call_to_prompt_text(tool_call: &message::ToolCall) -> Option<String> {
    match tool_call.tool.as_ref()? {
        message::tool_call::Tool::RunShellCommand(command) => {
            Some(format!("Requested shell command:\n{}", command.command))
        }
        _ => None,
    }
}

fn tool_call_result_to_prompt_text(result: &message::ToolCallResult) -> String {
    match result.result.as_ref() {
        Some(message::tool_call_result::Result::RunShellCommand(result)) => {
            let output = match result.result.as_ref() {
                Some(api::run_shell_command_result::Result::CommandFinished(finished)) => {
                    finished.output.clone()
                }
                Some(api::run_shell_command_result::Result::LongRunningCommandSnapshot(
                    snapshot,
                )) => snapshot.output.clone(),
                Some(api::run_shell_command_result::Result::PermissionDenied(_)) => {
                    "Permission denied.".to_owned()
                }
                None => String::new(),
            };
            format!("Command: {}\n{}", result.command, output)
        }
        Some(message::tool_call_result::Result::Server(server)) => server.serialized_result.clone(),
        Some(message::tool_call_result::Result::Subagent(subagent)) => subagent.payload.clone(),
        Some(message::tool_call_result::Result::Cancel(_)) => "Canceled.".to_owned(),
        Some(other) => format!("{other:?}"),
        None => String::new(),
    }
}

fn add_messages_event(
    task_id: String,
    request_id: String,
    model_id: String,
    input: &[AIAgentInput],
    text: Option<String>,
    tool_calls: Vec<OpenRouterToolCall>,
) -> api::ResponseEvent {
    let now = Utc::now();
    let timestamp = Some(prost_types::Timestamp {
        seconds: now.timestamp(),
        nanos: now.timestamp_subsec_nanos() as i32,
    });

    let model_used = api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.clone(),
        request_id: request_id.clone(),
        timestamp,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::ModelUsed(message::ModelUsed {
            model_id: model_id.clone(),
            model_display_name: model_id,
            is_fallback: false,
        })),
    };

    let mut messages = input_messages_for_task(input, &task_id, &request_id, timestamp);
    messages.push(model_used);
    if let Some(text) = text {
        messages.push(api::Message {
            id: Uuid::new_v4().to_string(),
            task_id: task_id.clone(),
            request_id: request_id.clone(),
            timestamp,
            server_message_data: String::new(),
            citations: vec![],
            message: Some(message::Message::AgentOutput(message::AgentOutput { text })),
        });
    }

    messages.extend(tool_calls.into_iter().filter_map(|tool_call| {
        openrouter_tool_call_to_message(&task_id, &request_id, tool_call, timestamp)
    }));

    api::ResponseEvent {
        r#type: Some(response_event::Type::ClientActions(
            response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(client_action::Action::AddMessagesToTask(
                        client_action::AddMessagesToTask { task_id, messages },
                    )),
                }],
            },
        )),
    }
}

fn input_messages_for_task(
    input: &[AIAgentInput],
    task_id: &str,
    request_id: &str,
    timestamp: Option<prost_types::Timestamp>,
) -> Vec<api::Message> {
    input
        .iter()
        .filter_map(|input| input_to_user_query_message(input, task_id, request_id, timestamp))
        .collect()
}

fn input_to_user_query_message(
    input: &AIAgentInput,
    task_id: &str,
    request_id: &str,
    timestamp: Option<prost_types::Timestamp>,
) -> Option<api::Message> {
    let AIAgentInput::UserQuery {
        query,
        context,
        user_query_mode,
        intended_agent,
        ..
    } = input
    else {
        return None;
    };

    Some(api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        request_id: request_id.to_owned(),
        timestamp,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::UserQuery(message::UserQuery {
            query: query.clone(),
            context: input_context_to_api(context),
            referenced_attachments: Default::default(),
            mode: Some(api_user_query_mode(*user_query_mode)),
            intended_agent: intended_agent.map(|agent| agent as i32).unwrap_or_default(),
        })),
    })
}

fn api_user_query_mode(mode: UserQueryMode) -> api::UserQueryMode {
    let r#type = match mode {
        UserQueryMode::Normal => None,
        UserQueryMode::Plan => Some(api::user_query_mode::Type::Plan(())),
        UserQueryMode::Orchestrate => Some(api::user_query_mode::Type::Orchestrate(())),
    };
    api::UserQueryMode { r#type }
}

fn done_event(usage: Option<OpenRouterUsage>, model_id: String) -> api::ResponseEvent {
    let token_usage = usage
        .map(|usage| stream_finished::TokenUsage {
            model_id,
            total_input: usage.prompt_tokens.unwrap_or_default(),
            output: usage.completion_tokens.unwrap_or_default(),
            ..Default::default()
        })
        .into_iter()
        .collect();

    let mut event = finished_event(stream_finished::Reason::Done(stream_finished::Done {}));
    if let Some(response_event::Type::Finished(finished)) = &mut event.r#type {
        finished.token_usage = token_usage;
    }
    event
}

fn finished_event(reason: stream_finished::Reason) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(response_event::Type::Finished(
            response_event::StreamFinished {
                reason: Some(reason),
                ..Default::default()
            },
        )),
    }
}

struct OpenRouterCompletion {
    text: Option<String>,
    tool_calls: Vec<OpenRouterToolCall>,
    usage: Option<OpenRouterUsage>,
}

fn truncate_for_log(body: &str) -> String {
    const MAX_LOG_CHARS: usize = 2000;
    let body = body.trim();
    if body.chars().count() <= MAX_LOG_CHARS {
        return body.to_owned();
    }

    let mut truncated = body.chars().take(MAX_LOG_CHARS).collect::<String>();
    truncated.push_str("...<truncated>");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::block_context::BlockContext;
    use crate::ai::blocklist::SessionContext;
    use crate::ai::llms::LLMId;
    use crate::terminal::model::{block::BlockId, terminal_model::BlockIndex};
    use futures_lite::{future::block_on, StreamExt};
    use std::sync::Arc;
    use warp_core::command::ExitCode;

    fn request_params_for_test() -> RequestParams {
        let model = LLMId::from("test-model");

        RequestParams {
            input: vec![],
            primary_task_id: "test-task".to_owned(),
            conversation_token: None,
            forked_from_conversation_token: None,
            local_agent_run_id: None,
            tasks: vec![],
            existing_suggestions: None,
            metadata: None,
            session_context: SessionContext::new_for_test(),
            model: model.clone(),
            coding_model: model.clone(),
            cli_agent_model: model.clone(),
            computer_use_model: model,
            is_memory_enabled: false,
            mcp_context: None,
            planning_enabled: true,
            should_redact_secrets: false,
            api_keys: None,
            open_router_model: None,
            allow_use_of_warp_credits_with_byok: false,
            autonomy_level: api::AutonomyLevel::Supervised,
            isolation_level: api::IsolationLevel::None,
            web_search_enabled: false,
            computer_use_enabled: false,
            ask_user_question_enabled: false,
            research_agent_enabled: false,
            orchestration_enabled: false,
            supported_tools_override: None,
            parent_agent_id: None,
            agent_name: None,
        }
    }

    fn user_query_input(query: &str) -> AIAgentInput {
        AIAgentInput::UserQuery {
            query: query.to_owned(),
            context: Default::default(),
            static_query_type: None,
            referenced_attachments: Default::default(),
            user_query_mode: UserQueryMode::Normal,
            running_command: None,
            intended_agent: None,
        }
    }

    fn user_query_input_with_context(query: &str, context: Vec<AIAgentContext>) -> AIAgentInput {
        AIAgentInput::UserQuery {
            query: query.to_owned(),
            context: Arc::from(context),
            static_query_type: None,
            referenced_attachments: Default::default(),
            user_query_mode: UserQueryMode::Normal,
            running_command: None,
            intended_agent: None,
        }
    }

    fn shell_output_context(output: &str) -> AIAgentContext {
        AIAgentContext::Block(Box::new(BlockContext {
            id: BlockId::from("test-block".to_owned()),
            index: BlockIndex::zero(),
            command: String::new(),
            output: output.to_owned(),
            exit_code: ExitCode::from(127),
            is_auto_attached: false,
            started_ts: None,
            finished_ts: None,
            pwd: None,
            shell: None,
            username: None,
            hostname: None,
            git_branch: None,
            os: None,
            session_id: None,
        }))
    }

    fn user_query_message(query: &str, request_id: &str) -> api::Message {
        api::Message {
            id: Uuid::new_v4().to_string(),
            task_id: "test-task".to_owned(),
            request_id: request_id.to_owned(),
            timestamp: None,
            server_message_data: String::new(),
            citations: vec![],
            message: Some(message::Message::UserQuery(message::UserQuery {
                query: query.to_owned(),
                context: None,
                referenced_attachments: Default::default(),
                mode: None,
                intended_agent: Default::default(),
            })),
        }
    }

    #[allow(deprecated)]
    fn user_query_message_with_context(query: &str, request_id: &str) -> api::Message {
        let mut message = user_query_message(query, request_id);
        let Some(message::Message::UserQuery(user_query)) = message.message.as_mut() else {
            panic!("expected user query message");
        };
        user_query.context = Some(api::InputContext {
            executed_shell_commands: vec![api::ExecutedShellCommand {
                command: "asdf".to_owned(),
                output: "zsh: command not found: asdf".to_owned(),
                exit_code: 127,
                command_id: "test-block".to_owned(),
                started_ts: None,
                finished_ts: None,
                is_auto_attached: false,
            }],
            ..Default::default()
        });
        message
    }

    fn agent_output_message(text: &str, request_id: &str) -> api::Message {
        api::Message {
            id: Uuid::new_v4().to_string(),
            task_id: "test-task".to_owned(),
            request_id: request_id.to_owned(),
            timestamp: None,
            server_message_data: String::new(),
            citations: vec![],
            message: Some(message::Message::AgentOutput(message::AgentOutput {
                text: text.to_owned(),
            })),
        }
    }

    #[test]
    fn output_creates_task_before_adding_messages_for_new_conversation() {
        let params = request_params_for_test();
        let events = block_on(generate_openrouter_output(params).collect::<Vec<_>>());

        assert!(matches!(
            events[0].as_ref().unwrap().r#type.as_ref().unwrap(),
            response_event::Type::Init(_)
        ));

        let response_event::Type::ClientActions(create_actions) =
            events[1].as_ref().unwrap().r#type.as_ref().unwrap()
        else {
            panic!("expected CreateTask client action");
        };
        let Some(client_action::Action::CreateTask(create)) =
            create_actions.actions[0].action.as_ref()
        else {
            panic!("expected CreateTask action");
        };
        assert_eq!(create.task.as_ref().unwrap().id, "test-task");

        let response_event::Type::ClientActions(add_actions) =
            events[2].as_ref().unwrap().r#type.as_ref().unwrap()
        else {
            panic!("expected AddMessagesToTask client action");
        };
        let Some(client_action::Action::AddMessagesToTask(add)) =
            add_actions.actions[0].action.as_ref()
        else {
            panic!("expected AddMessagesToTask action");
        };
        assert_eq!(add.task_id, "test-task");
        assert!(add
            .messages
            .iter()
            .any(|message| matches!(message.message, Some(message::Message::AgentOutput(_)))));
    }

    #[test]
    #[allow(deprecated)]
    fn output_persists_current_user_query_message() {
        let mut params = request_params_for_test();
        params.input = vec![user_query_input_with_context(
            "remember this",
            vec![shell_output_context("zsh: command not found: asdf\n")],
        )];

        let events = block_on(generate_openrouter_output(params).collect::<Vec<_>>());

        let response_event::Type::ClientActions(add_actions) =
            events[2].as_ref().unwrap().r#type.as_ref().unwrap()
        else {
            panic!("expected AddMessagesToTask client action");
        };
        let Some(client_action::Action::AddMessagesToTask(add)) =
            add_actions.actions[0].action.as_ref()
        else {
            panic!("expected AddMessagesToTask action");
        };

        let persisted_query = add.messages.iter().find_map(|message| {
            let Some(message::Message::UserQuery(query)) = message.message.as_ref() else {
                return None;
            };
            (query.query == "remember this").then_some(query)
        });
        assert!(persisted_query.is_some());
        assert!(persisted_query
            .and_then(|query| query.context.as_ref())
            .is_some_and(|context| !context.executed_shell_commands.is_empty()));
    }

    #[test]
    fn request_messages_include_current_user_query_context() {
        let mut params = request_params_for_test();
        params.input = vec![user_query_input_with_context(
            "what is the error i'm attaching?",
            vec![shell_output_context("zsh: command not found: asdf\n")],
        )];

        let messages = build_openrouter_messages(&params);

        let user_message = messages.last().expect("expected current user message");
        assert_eq!(user_message.role, "user");
        assert!(user_message
            .content
            .contains("what is the error i'm attaching?"));
        assert!(user_message.content.contains("Attached terminal output"));
        assert!(user_message
            .content
            .contains("zsh: command not found: asdf"));
    }

    #[test]
    fn request_messages_include_prior_task_history() {
        let mut params = request_params_for_test();
        params.tasks.push(api::Task {
            id: "test-task".to_owned(),
            messages: vec![
                user_query_message("first question", "request-1"),
                agent_output_message("first answer", "request-1"),
            ],
            dependencies: None,
            description: String::new(),
            summary: String::new(),
            server_data: String::new(),
        });
        params.input = vec![user_query_input("follow up")];

        let messages = build_openrouter_messages(&params);

        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "first question");
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].content, "first answer");
        assert_eq!(messages[3].role, "user");
        assert_eq!(messages[3].content, "follow up");
    }

    #[test]
    fn request_messages_include_prior_user_query_context() {
        let mut params = request_params_for_test();
        params.tasks.push(api::Task {
            id: "test-task".to_owned(),
            messages: vec![user_query_message_with_context(
                "what is the error i'm attaching?",
                "request-1",
            )],
            dependencies: None,
            description: String::new(),
            summary: String::new(),
            server_data: String::new(),
        });

        let messages = build_openrouter_messages(&params);

        assert_eq!(messages[1].role, "user");
        assert!(messages[1]
            .content
            .contains("what is the error i'm attaching?"));
        assert!(messages[1].content.contains("Attached terminal command"));
        assert!(messages[1].content.contains("asdf"));
        assert!(messages[1].content.contains("zsh: command not found: asdf"));
    }

    #[test]
    fn output_does_not_recreate_existing_task() {
        let mut params = request_params_for_test();
        params.tasks.push(api::Task {
            id: "test-task".to_owned(),
            messages: vec![],
            dependencies: None,
            description: String::new(),
            summary: String::new(),
            server_data: String::new(),
        });

        let events = block_on(generate_openrouter_output(params).collect::<Vec<_>>());

        assert!(matches!(
            events[0].as_ref().unwrap().r#type.as_ref().unwrap(),
            response_event::Type::Init(_)
        ));

        let response_event::Type::ClientActions(add_actions) =
            events[1].as_ref().unwrap().r#type.as_ref().unwrap()
        else {
            panic!("expected AddMessagesToTask client action");
        };
        assert!(matches!(
            add_actions.actions[0].action.as_ref().unwrap(),
            client_action::Action::AddMessagesToTask(_)
        ));
    }
}
