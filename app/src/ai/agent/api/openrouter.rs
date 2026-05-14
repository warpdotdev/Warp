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
use crate::search::slash_command_menu::static_commands::commands;

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
    #[serde(default)]
    uses_pager: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ReadFilesArgs {
    files: Vec<ReadFileArg>,
}

#[derive(Debug, Deserialize)]
struct ReadFileArg {
    path: String,
    #[serde(default)]
    line_ranges: Vec<LineRangeArg>,
}

#[derive(Debug, Deserialize)]
struct LineRangeArg {
    start: u32,
    end: u32,
}

#[derive(Debug, Deserialize)]
struct SearchCodebaseArgs {
    query: String,
    #[serde(default)]
    path_filters: Vec<String>,
    #[serde(default)]
    codebase_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GrepArgs {
    queries: Vec<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileGlobV2Args {
    patterns: Vec<String>,
    #[serde(default)]
    search_dir: Option<String>,
    #[serde(default)]
    max_matches: Option<i32>,
    #[serde(default)]
    max_depth: Option<i32>,
    #[serde(default)]
    min_depth: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct ApplyFileDiffsArgs {
    summary: String,
    #[serde(default)]
    diffs: Vec<FileDiffArg>,
    #[serde(default)]
    new_files: Vec<NewFileArg>,
    #[serde(default)]
    deleted_files: Vec<DeletedFileArg>,
}

#[derive(Debug, Deserialize)]
struct FileDiffArg {
    file_path: String,
    search: String,
    replace: String,
}

#[derive(Debug, Deserialize)]
struct NewFileArg {
    file_path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct DeletedFileArg {
    file_path: String,
}

#[derive(Debug, Deserialize)]
struct ReadSkillArgs {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    skill_path: Option<String>,
    #[serde(default)]
    bundled_skill_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AskUserQuestionArgs {
    questions: Vec<AskQuestionArg>,
}

#[derive(Debug, Deserialize)]
struct AskQuestionArg {
    #[serde(default)]
    question_id: Option<String>,
    question: String,
    options: Vec<AskOptionArg>,
    #[serde(default)]
    recommended_option_index: Option<i32>,
    #[serde(default)]
    is_multiselect: Option<bool>,
    #[serde(default)]
    supports_other: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AskOptionArg {
    label: String,
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
    vec![
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "run_shell_command",
                description: "Run a finite, non-interactive shell command in the user's active terminal session. Use this for terminal inspection, tests, or explicit command changes requested by the user. Warper runs agent commands with a controlled non-paging environment.",
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
        },
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "read_files",
                description: "Read one or more local files. Prefer this over shell commands when the task needs file contents.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "files": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "path": { "type": "string" },
                                    "line_ranges": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "start": { "type": "integer" },
                                                "end": { "type": "integer" }
                                            },
                                            "required": ["start", "end"]
                                        }
                                    }
                                },
                                "required": ["path"]
                            }
                        }
                    },
                    "required": ["files"]
                }),
            },
        },
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "grep",
                description: "Search local file contents for text or regex patterns.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "queries": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional relative file or directory path to search."
                        }
                    },
                    "required": ["queries"]
                }),
            },
        },
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "file_glob_v2",
                description: "Find local files by file-name patterns.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "patterns": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "search_dir": {
                            "type": "string",
                            "description": "Optional relative directory to search."
                        },
                        "max_matches": { "type": "integer" },
                        "max_depth": { "type": "integer" },
                        "min_depth": { "type": "integer" }
                    },
                    "required": ["patterns"]
                }),
            },
        },
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "search_codebase",
                description: "Search the locally indexed codebase for relevant files.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "path_filters": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "codebase_path": {
                            "type": "string",
                            "description": "Optional absolute path to the codebase."
                        }
                    },
                    "required": ["query"]
                }),
            },
        },
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "apply_file_diffs",
                description: "Propose local file edits through Warper's normal diff review and permission flow.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "summary": { "type": "string" },
                        "diffs": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "file_path": { "type": "string" },
                                    "search": { "type": "string" },
                                    "replace": { "type": "string" }
                                },
                                "required": ["file_path", "search", "replace"]
                            }
                        },
                        "new_files": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "file_path": { "type": "string" },
                                    "content": { "type": "string" }
                                },
                                "required": ["file_path", "content"]
                            }
                        },
                        "deleted_files": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "file_path": { "type": "string" }
                                },
                                "required": ["file_path"]
                            }
                        }
                    },
                    "required": ["summary"]
                }),
            },
        },
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "read_skill",
                description: "Read a known local or bundled skill file for detailed instructions.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "skill_path": { "type": "string" },
                        "bundled_skill_id": { "type": "string" }
                    }
                }),
            },
        },
        OpenRouterTool {
            kind: "function",
            function: OpenRouterToolFunction {
                name: "ask_user_question",
                description: "Ask the user a short clarifying multiple-choice question before proceeding.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "questions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "question_id": { "type": "string" },
                                    "question": { "type": "string" },
                                    "options": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": { "type": "string" }
                                            },
                                            "required": ["label"]
                                        }
                                    },
                                    "recommended_option_index": { "type": "integer" },
                                    "is_multiselect": { "type": "boolean" },
                                    "supports_other": { "type": "boolean" }
                                },
                                "required": ["question", "options"]
                            }
                        }
                    },
                    "required": ["questions"]
                }),
            },
        },
    ]
}

fn openrouter_tool_call_to_message(
    task_id: &str,
    request_id: &str,
    tool_call: OpenRouterToolCall,
    timestamp: Option<prost_types::Timestamp>,
) -> Option<api::Message> {
    let Some(tool) = openrouter_tool_call_to_api_tool(&tool_call) else {
        log::warn!(
            "Ignoring unsupported or invalid OpenRouter tool call: name={}, id={}",
            tool_call.function.name,
            tool_call.id
        );
        return None;
    };

    Some(api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        request_id: request_id.to_owned(),
        timestamp,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call.id,
            tool: Some(tool),
        })),
    })
}

fn openrouter_tool_call_to_api_tool(
    tool_call: &OpenRouterToolCall,
) -> Option<api::message::tool_call::Tool> {
    match tool_call.function.name.as_str() {
        "run_shell_command" => {
            let args: RunShellCommandArgs =
                serde_json::from_str(&tool_call.function.arguments).ok()?;
            let command = args.command.trim().to_owned();
            if command.is_empty() {
                return None;
            }

            let is_read_only = args
                .is_read_only
                .unwrap_or_else(|| command_looks_read_only(&command));
            let is_risky = !is_read_only;

            Some(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command,
                    is_read_only,
                    uses_pager: args.uses_pager.unwrap_or(false),
                    citations: vec![],
                    is_risky,
                    wait_until_complete_value: None,
                    risk_category: 0,
                },
            ))
        }
        "read_files" => {
            let args: ReadFilesArgs = serde_json::from_str(&tool_call.function.arguments).ok()?;
            let files = args
                .files
                .into_iter()
                .filter(|file| !file.path.trim().is_empty())
                .map(|file| api::message::tool_call::read_files::File {
                    name: file.path,
                    line_ranges: file
                        .line_ranges
                        .into_iter()
                        .map(|range| api::FileContentLineRange {
                            start: range.start,
                            end: range.end,
                        })
                        .collect(),
                })
                .collect::<Vec<_>>();
            (!files.is_empty()).then_some(api::message::tool_call::Tool::ReadFiles(
                api::message::tool_call::ReadFiles { files },
            ))
        }
        "search_codebase" => {
            let args: SearchCodebaseArgs =
                serde_json::from_str(&tool_call.function.arguments).ok()?;
            let query = args.query.trim().to_owned();
            (!query.is_empty()).then_some(api::message::tool_call::Tool::SearchCodebase(
                api::message::tool_call::SearchCodebase {
                    query,
                    path_filters: args.path_filters,
                    codebase_path: args.codebase_path.unwrap_or_default(),
                },
            ))
        }
        "grep" => {
            let args: GrepArgs = serde_json::from_str(&tool_call.function.arguments).ok()?;
            let queries = args
                .queries
                .into_iter()
                .map(|query| query.trim().to_owned())
                .filter(|query| !query.is_empty())
                .collect::<Vec<_>>();
            (!queries.is_empty()).then_some(api::message::tool_call::Tool::Grep(
                api::message::tool_call::Grep {
                    queries,
                    path: args.path.unwrap_or_default(),
                },
            ))
        }
        "file_glob_v2" => {
            let args: FileGlobV2Args = serde_json::from_str(&tool_call.function.arguments).ok()?;
            let patterns = args
                .patterns
                .into_iter()
                .map(|pattern| pattern.trim().to_owned())
                .filter(|pattern| !pattern.is_empty())
                .collect::<Vec<_>>();
            (!patterns.is_empty()).then_some(api::message::tool_call::Tool::FileGlobV2(
                api::message::tool_call::FileGlobV2 {
                    patterns,
                    search_dir: args.search_dir.unwrap_or_default(),
                    max_matches: args.max_matches.unwrap_or_default(),
                    max_depth: args.max_depth.unwrap_or_default(),
                    min_depth: args.min_depth.unwrap_or_default(),
                },
            ))
        }
        "apply_file_diffs" => {
            let args: ApplyFileDiffsArgs =
                serde_json::from_str(&tool_call.function.arguments).ok()?;
            if args.diffs.is_empty() && args.new_files.is_empty() && args.deleted_files.is_empty() {
                return None;
            }

            Some(api::message::tool_call::Tool::ApplyFileDiffs(
                api::message::tool_call::ApplyFileDiffs {
                    summary: args.summary,
                    diffs: args
                        .diffs
                        .into_iter()
                        .map(|diff| api::message::tool_call::apply_file_diffs::FileDiff {
                            file_path: diff.file_path,
                            search: diff.search,
                            replace: diff.replace,
                        })
                        .collect(),
                    new_files: args
                        .new_files
                        .into_iter()
                        .map(|file| api::message::tool_call::apply_file_diffs::NewFile {
                            file_path: file.file_path,
                            content: file.content,
                        })
                        .collect(),
                    deleted_files: args
                        .deleted_files
                        .into_iter()
                        .map(
                            |file| api::message::tool_call::apply_file_diffs::DeleteFile {
                                file_path: file.file_path,
                            },
                        )
                        .collect(),
                    v4a_updates: vec![],
                },
            ))
        }
        "read_skill" => {
            let args: ReadSkillArgs = serde_json::from_str(&tool_call.function.arguments).ok()?;
            let name = args.name.unwrap_or_default();
            let skill_reference = match (args.skill_path, args.bundled_skill_id) {
                (Some(path), _) if !path.trim().is_empty() => {
                    Some(api::message::tool_call::read_skill::SkillReference::SkillPath(path))
                }
                (_, Some(id)) if !id.trim().is_empty() => {
                    Some(api::message::tool_call::read_skill::SkillReference::BundledSkillId(id))
                }
                _ if !name.trim().is_empty() => Some(
                    api::message::tool_call::read_skill::SkillReference::BundledSkillId(
                        name.clone(),
                    ),
                ),
                _ => None,
            }?;

            Some(api::message::tool_call::Tool::ReadSkill(
                api::message::tool_call::ReadSkill {
                    name,
                    skill_reference: Some(skill_reference),
                },
            ))
        }
        "ask_user_question" => {
            let args: AskUserQuestionArgs =
                serde_json::from_str(&tool_call.function.arguments).ok()?;
            let questions = args
                .questions
                .into_iter()
                .enumerate()
                .filter(|(_, question)| {
                    !question.question.trim().is_empty() && !question.options.is_empty()
                })
                .map(|(index, question)| api::ask_user_question::Question {
                    question_id: question
                        .question_id
                        .filter(|id| !id.trim().is_empty())
                        .unwrap_or_else(|| format!("question_{}", index + 1)),
                    question: question.question,
                    question_type: Some(
                        api::ask_user_question::question::QuestionType::MultipleChoice(
                            api::ask_user_question::MultipleChoice {
                                options: question
                                    .options
                                    .into_iter()
                                    .map(|option| api::ask_user_question::Option {
                                        label: option.label,
                                    })
                                    .collect(),
                                recommended_option_index: question
                                    .recommended_option_index
                                    .unwrap_or(-1),
                                is_multiselect: question.is_multiselect.unwrap_or(false),
                                supports_other: question.supports_other.unwrap_or(false),
                            },
                        ),
                    ),
                })
                .collect::<Vec<_>>();
            (!questions.is_empty()).then_some(api::message::tool_call::Tool::AskUserQuestion(
                api::AskUserQuestion { questions },
            ))
        }
        _ => None,
    }
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
        AIAgentInput::UserQuery {
            query,
            context,
            user_query_mode,
            ..
        } => user_query_prompt_text(query, *user_query_mode, context),
        AIAgentInput::ActionResult { result, context } => {
            prompt_text_with_context(format!("Tool result:\n{result}"), context)
        }
        AIAgentInput::AutoCodeDiffQuery { query, context } => {
            prompt_text_with_context(format!("Code assistance request:\n{query}"), context)
        }
        AIAgentInput::InitProjectRules {
            context,
            display_query,
        } => {
            let command = display_query.as_deref().unwrap_or(commands::INIT.name);
            prompt_text_with_context(
                format!(
                    "Original user message: {command}\n\nInitialize project rules for the current repository. Inspect the project and create or update the local AGENTS.md guidance as appropriate."
                ),
                context,
            )
        }
        AIAgentInput::CreateNewProject { query, context } => {
            prompt_text_with_context(create_new_project_prompt_text(query), context)
        }
        AIAgentInput::CloneRepository {
            clone_repo_url,
            context,
        } => prompt_text_with_context(clone_repository_prompt_text(clone_repo_url.url()), context),
        AIAgentInput::CodeReview {
            context,
            review_comments,
        } => prompt_text_with_context(
            crate::terminal::cli_agent::build_review_prompt(review_comments),
            context,
        ),
        AIAgentInput::FetchReviewComments { repo_path, context } => {
            prompt_text_with_context(fetch_review_comments_prompt_text(repo_path), context)
        }
        AIAgentInput::SummarizeConversation { prompt } => {
            summarize_conversation_prompt_text(prompt.as_deref())
        }
        AIAgentInput::PassiveSuggestionResult {
            suggestion,
            context,
            ..
        } => prompt_text_with_context(
            format!("Passive suggestion result:\n{suggestion:?}"),
            context,
        ),
        AIAgentInput::InvokeSkill {
            skill,
            user_query,
            context,
        } => prompt_text_with_context(
            invoke_skill_prompt_text(skill, user_query.as_ref()),
            context,
        ),
        _ => input.user_query().unwrap_or_else(|| input.to_string()),
    }
}

fn create_new_project_prompt_text(query: &str) -> String {
    format!(
        "Original user message: {} {query}\n\nCreate a new local coding project from the user's request.",
        commands::CREATE_NEW_PROJECT.name
    )
}

fn clone_repository_prompt_text(url: &str) -> String {
    format!("Clone repository:\n{url}")
}

fn fetch_review_comments_prompt_text(repo_path: &str) -> String {
    format!(
        "Original user message: {}\n\nFetch and address GitHub PR review comments for the repository at:\n{repo_path}",
        commands::PR_COMMENTS.name
    )
}

fn summarize_conversation_prompt_text(prompt: Option<&str>) -> String {
    match prompt.filter(|prompt| !prompt.trim().is_empty()) {
        Some(prompt) => format!(
            "Original user message: {} {prompt}\n\nSummarize the current conversation. Additional instructions:\n{prompt}",
            commands::COMPACT.name
        ),
        None => format!(
            "Original user message: {}\n\nSummarize the current conversation.",
            commands::COMPACT.name
        ),
    }
}

fn invoke_skill_prompt_text(
    skill: &ai::skills::ParsedSkill,
    user_query: Option<&crate::ai::agent::InvokeSkillUserQuery>,
) -> String {
    let mut prompt = format!(
        "Invoked skill: /{}\n\nUser request:\n{}\n\nSkill instructions:\n```markdown\n{}\n```",
        skill.name,
        user_query
            .map(|query| query.query.as_str())
            .filter(|query| !query.trim().is_empty())
            .unwrap_or("(no additional user request)"),
        skill.content
    );

    if let Some(dependencies) = resolved_skill_dependency_text(skill) {
        prompt.push_str("\n\nResolved dependent skill context:\n");
        prompt.push_str(&dependencies);
    }

    prompt
}

fn user_query_prompt_text(query: &str, mode: UserQueryMode, context: &[AIAgentContext]) -> String {
    prompt_text_with_context(user_query_text(query, mode), context)
}

fn user_query_text(query: &str, mode: UserQueryMode) -> String {
    match mode {
        UserQueryMode::Normal => query.to_owned(),
        UserQueryMode::Plan => format!(
            "Original user message: {} {query}\n\nPlan mode instructions: produce a concrete plan before taking action. Do not modify files, run mutating commands, or create commits for this turn unless the user explicitly approves proceeding after the plan.",
            commands::PLAN.name
        ),
        UserQueryMode::Orchestrate => format!(
            "Original user message: {} {query}\n\nOrchestrate mode instructions: coordinate the requested work explicitly and preserve the user's orchestration intent. Do not assume hosted orchestration services are available.",
            commands::ORCHESTRATE.name
        ),
    }
}

fn resolved_skill_dependency_text(skill: &ai::skills::ParsedSkill) -> Option<String> {
    let dependency_names = match skill.name.as_str() {
        "update-tab-config" | "create-tab-config" => &["tab-configs"][..],
        _ => &[][..],
    };
    if dependency_names.is_empty() {
        return None;
    }

    let bundled_root = skill.path.parent()?.parent()?;
    let dependencies = dependency_names
        .iter()
        .filter_map(|name| {
            let path = bundled_root.join(name).join("SKILL.md");
            let content = std::fs::read_to_string(&path).ok()?;
            Some(format!(
                "Dependent skill: /{name}\n```markdown\n{content}\n```"
            ))
        })
        .collect::<Vec<_>>();

    (!dependencies.is_empty()).then(|| dependencies.join("\n\n"))
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
        AIAgentContext::Skills { skills } => {
            let skills = skills
                .iter()
                .map(|skill| {
                    format!(
                        "- /{}: {} ({})",
                        skill.name, skill.description, skill.reference
                    )
                })
                .collect::<Vec<_>>();
            (!skills.is_empty()).then(|| format!("Available skills:\n{}", skills.join("\n")))
        }
        AIAgentContext::ExecutionEnvironment(_)
        | AIAgentContext::CurrentTime { .. }
        | AIAgentContext::Image(_)
        | AIAgentContext::Codebase { .. } => None,
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
    let messages = tasks
        .iter()
        .flat_map(|task| task.messages.iter())
        .collect::<Vec<_>>();
    openrouter_messages_from_task_messages(&messages)
}

fn openrouter_messages_from_task_messages(messages: &[&api::Message]) -> Vec<OpenRouterMessage> {
    let latest_summary_index = messages
        .iter()
        .rposition(|message| conversation_summary_text(message).is_some());

    let mut openrouter_messages = Vec::new();
    let replay_start_index = if let Some(summary_index) = latest_summary_index {
        if let Some(summary) = conversation_summary_text(messages[summary_index]) {
            openrouter_messages.push(OpenRouterMessage {
                role: "user",
                content: format!("Compacted conversation summary:\n{summary}"),
            });
        }
        summary_index + 1
    } else {
        0
    };

    openrouter_messages.extend(
        messages[replay_start_index..]
            .iter()
            .filter_map(|message| api_message_to_openrouter_message(message)),
    );
    openrouter_messages
}

fn api_message_to_openrouter_message(message: &api::Message) -> Option<OpenRouterMessage> {
    match message.message.as_ref()? {
        message::Message::UserQuery(user_query) => Some(OpenRouterMessage {
            role: "user",
            content: user_query_prompt_text(
                &user_query.query,
                api_user_query_mode_to_local(user_query.mode.as_ref()),
                &super::convert_conversation::convert_input_context(user_query.context.as_ref()),
            ),
        }),
        message::Message::InvokeSkill(invoke_skill) => Some(OpenRouterMessage {
            role: "user",
            content: historical_invoke_skill_prompt_text(invoke_skill),
        }),
        message::Message::SystemQuery(query) => {
            system_query_to_prompt_text(query).map(|content| OpenRouterMessage {
                role: "user",
                content,
            })
        }
        message::Message::CodeReview(code_review) => Some(OpenRouterMessage {
            role: "user",
            content: code_review_to_prompt_text(code_review),
        }),
        message::Message::ToolCallResult(result) => Some(OpenRouterMessage {
            role: "user",
            content: format!("Tool result:\n{}", tool_call_result_to_prompt_text(result)),
        }),
        message::Message::AgentOutput(output) => Some(OpenRouterMessage {
            role: "assistant",
            content: output.text.clone(),
        }),
        message::Message::Summarization(summarization) => {
            conversation_summary_text_from_summarization(summarization).map(|summary| {
                OpenRouterMessage {
                    role: "user",
                    content: format!("Compacted conversation summary:\n{summary}"),
                }
            })
        }
        message::Message::ToolCall(tool_call) => {
            tool_call_to_prompt_text(tool_call).map(|content| OpenRouterMessage {
                role: "assistant",
                content,
            })
        }
        _ => None,
    }
}

fn conversation_summary_text(message: &api::Message) -> Option<&str> {
    let message::Message::Summarization(summarization) = message.message.as_ref()? else {
        return None;
    };
    conversation_summary_text_from_summarization(summarization)
}

fn conversation_summary_text_from_summarization(
    summarization: &message::Summarization,
) -> Option<&str> {
    let Some(message::summarization::SummaryType::ConversationSummary(summary)) =
        summarization.summary_type.as_ref()
    else {
        return None;
    };
    (!summary.summary.is_empty()).then_some(summary.summary.as_str())
}

fn code_review_to_prompt_text(code_review: &message::CodeReview) -> String {
    let mut text = String::from("Please address the following code review comments.");
    let Some(comments) = &code_review.comments else {
        return text;
    };

    for comment in &comments.pending_comments {
        let target = match comment.comment_target.as_ref() {
            Some(api::review_comment::CommentTarget::CommentedLine(line)) => {
                format!(
                    "{} {}",
                    line.file_path,
                    line_range_to_prompt_text(&line.line_range)
                )
            }
            Some(api::review_comment::CommentTarget::CommentedFile(file)) => file.file_path.clone(),
            Some(api::review_comment::CommentTarget::CommentedDiffset(_)) | None => {
                "General".to_owned()
            }
        };
        text.push_str(&format!("\n- {target}: {}", comment.comment));
    }

    text
}

fn line_range_to_prompt_text(range: &Option<api::FileContentLineRange>) -> String {
    match range {
        Some(range) if range.end > range.start + 1 => {
            format!("L{}-L{}", range.start + 1, range.end)
        }
        Some(range) => format!("L{}", range.start + 1),
        None => String::new(),
    }
}

fn api_user_query_mode_to_local(mode: Option<&api::UserQueryMode>) -> UserQueryMode {
    match mode.and_then(|mode| mode.r#type.as_ref()) {
        Some(api::user_query_mode::Type::Plan(())) => UserQueryMode::Plan,
        Some(api::user_query_mode::Type::Orchestrate(())) => UserQueryMode::Orchestrate,
        None => UserQueryMode::Normal,
    }
}

fn historical_invoke_skill_prompt_text(invoke_skill: &message::InvokeSkill) -> String {
    let skill_name = invoke_skill
        .skill
        .as_ref()
        .and_then(|skill| skill.descriptor.as_ref())
        .map(|descriptor| descriptor.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("unknown-skill");
    let user_query = invoke_skill
        .user_query
        .as_ref()
        .map(|query| query.query.as_str())
        .filter(|query| !query.trim().is_empty());

    let mut prompt = match user_query {
        Some(query) => format!("Invoked skill: /{skill_name}\n\nUser request:\n{query}"),
        None => format!("Invoked skill: /{skill_name}"),
    };

    if let Some(content) = invoke_skill
        .skill
        .as_ref()
        .and_then(|skill| skill.content.as_ref())
        .map(|content| content.content.as_str())
        .filter(|content| !content.trim().is_empty())
    {
        prompt.push_str("\n\nSkill instructions:\n```markdown\n");
        prompt.push_str(content);
        prompt.push_str("\n```");
    }

    prompt
}

fn system_query_to_prompt_text(query: &message::SystemQuery) -> Option<String> {
    let context = super::convert_conversation::convert_input_context(query.context.as_ref());
    match query.r#type.as_ref()? {
        message::system_query::Type::AutoCodeDiff(query) => Some(prompt_text_with_context(
            format!("Code assistance request:\n{}", query.query),
            &context,
        )),
        message::system_query::Type::CreateNewProject(query) => Some(prompt_text_with_context(
            create_new_project_prompt_text(&query.query),
            &context,
        )),
        message::system_query::Type::CloneRepository(query) => Some(prompt_text_with_context(
            clone_repository_prompt_text(&query.url),
            &context,
        )),
        message::system_query::Type::SummarizeConversation(query) => {
            Some(summarize_conversation_prompt_text(Some(&query.prompt)))
        }
        message::system_query::Type::FetchReviewComments(query) => Some(prompt_text_with_context(
            fetch_review_comments_prompt_text(&query.repo_path),
            &context,
        )),
        message::system_query::Type::ResumeConversation(_)
        | message::system_query::Type::GeneratePassiveSuggestions(_) => None,
    }
}

fn tool_call_to_prompt_text(tool_call: &message::ToolCall) -> Option<String> {
    match tool_call.tool.as_ref()? {
        message::tool_call::Tool::RunShellCommand(command) => {
            Some(format!("Requested shell command:\n{}", command.command))
        }
        message::tool_call::Tool::ReadFiles(read_files) => Some(format!(
            "Requested file read:\n{}",
            read_files
                .files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        )),
        message::tool_call::Tool::SearchCodebase(search) => Some(format!(
            "Requested codebase search:\nQuery: {}\nPath filters: {}",
            search.query,
            search.path_filters.join(", ")
        )),
        message::tool_call::Tool::Grep(grep) => Some(format!(
            "Requested grep search:\nQueries: {}\nPath: {}",
            grep.queries.join(", "),
            grep.path
        )),
        message::tool_call::Tool::FileGlobV2(glob) => Some(format!(
            "Requested file glob:\nPatterns: {}\nSearch dir: {}",
            glob.patterns.join(", "),
            glob.search_dir
        )),
        message::tool_call::Tool::ApplyFileDiffs(diff) => Some(format!(
            "Requested file edits:\nSummary: {}\n{}",
            diff.summary,
            apply_file_diffs_summary(diff)
        )),
        message::tool_call::Tool::ReadSkill(read_skill) => Some(format!(
            "Requested skill read:\nName: {}\nReference: {}",
            read_skill.name,
            read_skill_reference_to_text(read_skill.skill_reference.as_ref())
        )),
        message::tool_call::Tool::AskUserQuestion(ask) => Some(format!(
            "Requested user clarification:\n{}",
            ask.questions
                .iter()
                .map(|question| question.question.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        )),
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
        Some(message::tool_call_result::Result::ReadFiles(result)) => {
            read_files_result_to_prompt_text(result)
        }
        Some(message::tool_call_result::Result::SearchCodebase(result)) => {
            search_codebase_result_to_prompt_text(result)
        }
        Some(message::tool_call_result::Result::Grep(result)) => grep_result_to_prompt_text(result),
        Some(message::tool_call_result::Result::FileGlobV2(result)) => {
            file_glob_v2_result_to_prompt_text(result)
        }
        Some(message::tool_call_result::Result::ApplyFileDiffs(result)) => {
            apply_file_diffs_result_to_prompt_text(result)
        }
        Some(message::tool_call_result::Result::ReadSkill(result)) => {
            read_skill_result_to_prompt_text(result)
        }
        Some(message::tool_call_result::Result::AskUserQuestion(result)) => {
            ask_user_question_result_to_prompt_text(result)
        }
        Some(message::tool_call_result::Result::Server(server)) => server.serialized_result.clone(),
        Some(message::tool_call_result::Result::Subagent(subagent)) => subagent.payload.clone(),
        Some(message::tool_call_result::Result::Cancel(_)) => "Canceled.".to_owned(),
        Some(other) => format!("{other:?}"),
        None => String::new(),
    }
}

fn apply_file_diffs_summary(diff: &message::tool_call::ApplyFileDiffs) -> String {
    let mut parts = Vec::new();
    if !diff.diffs.is_empty() {
        parts.push(format!(
            "Updated files: {}",
            diff.diffs
                .iter()
                .map(|diff| diff.file_path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !diff.new_files.is_empty() {
        parts.push(format!(
            "New files: {}",
            diff.new_files
                .iter()
                .map(|file| file.file_path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !diff.deleted_files.is_empty() {
        parts.push(format!(
            "Deleted files: {}",
            diff.deleted_files
                .iter()
                .map(|file| file.file_path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    parts.join("\n")
}

fn read_skill_reference_to_text(
    reference: Option<&api::message::tool_call::read_skill::SkillReference>,
) -> String {
    match reference {
        Some(api::message::tool_call::read_skill::SkillReference::SkillPath(path)) => {
            format!("path:{path}")
        }
        Some(api::message::tool_call::read_skill::SkillReference::BundledSkillId(id)) => {
            format!("bundled:{id}")
        }
        None => "none".to_owned(),
    }
}

fn read_files_result_to_prompt_text(result: &api::ReadFilesResult) -> String {
    match result.result.as_ref() {
        Some(api::read_files_result::Result::TextFilesSuccess(success)) => success
            .files
            .iter()
            .map(file_content_to_prompt_text)
            .collect::<Vec<_>>()
            .join("\n\n"),
        Some(api::read_files_result::Result::AnyFilesSuccess(success)) => success
            .files
            .iter()
            .map(any_file_content_to_prompt_text)
            .collect::<Vec<_>>()
            .join("\n\n"),
        Some(api::read_files_result::Result::Error(error)) => {
            format!("Read files error: {}", error.message)
        }
        None => "Read files cancelled.".to_owned(),
    }
}

fn search_codebase_result_to_prompt_text(result: &api::SearchCodebaseResult) -> String {
    match result.result.as_ref() {
        Some(api::search_codebase_result::Result::Success(success)) => success
            .files
            .iter()
            .map(file_content_to_prompt_text)
            .collect::<Vec<_>>()
            .join("\n\n"),
        Some(api::search_codebase_result::Result::Error(error)) => {
            format!("Codebase search error: {}", error.message)
        }
        None => "Codebase search cancelled.".to_owned(),
    }
}

fn grep_result_to_prompt_text(result: &api::GrepResult) -> String {
    match result.result.as_ref() {
        Some(api::grep_result::Result::Success(success)) => success
            .matched_files
            .iter()
            .map(|file| {
                let lines = file
                    .matched_lines
                    .iter()
                    .map(|line| line.line_number.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}: lines [{}]", file.file_path, lines)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(api::grep_result::Result::Error(error)) => format!("Grep error: {}", error.message),
        None => "Grep cancelled.".to_owned(),
    }
}

fn file_glob_v2_result_to_prompt_text(result: &api::FileGlobV2Result) -> String {
    match result.result.as_ref() {
        Some(api::file_glob_v2_result::Result::Success(success)) => {
            let files = success
                .matched_files
                .iter()
                .map(|file| file.file_path.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if success.warnings.is_empty() {
                files
            } else {
                format!("{files}\nWarnings:\n{}", success.warnings)
            }
        }
        Some(api::file_glob_v2_result::Result::Error(error)) => {
            format!("File glob error: {}", error.message)
        }
        None => "File glob cancelled.".to_owned(),
    }
}

fn apply_file_diffs_result_to_prompt_text(result: &api::ApplyFileDiffsResult) -> String {
    match result.result.as_ref() {
        Some(api::apply_file_diffs_result::Result::Success(success)) => {
            let updated = success
                .updated_files_v2
                .iter()
                .filter_map(|file| file.file.as_ref())
                .map(|file| file.file_path.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let deleted = success
                .deleted_files
                .iter()
                .map(|file| file.file_path.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("File edits applied.\nUpdated: {updated}\nDeleted: {deleted}")
        }
        Some(api::apply_file_diffs_result::Result::Error(error)) => {
            format!("File edit error: {}", error.message)
        }
        None => "File edits cancelled.".to_owned(),
    }
}

fn read_skill_result_to_prompt_text(result: &api::ReadSkillResult) -> String {
    match result.result.as_ref() {
        Some(api::read_skill_result::Result::Success(success)) => success
            .content
            .as_ref()
            .map(file_content_to_prompt_text)
            .unwrap_or_else(|| "Skill read succeeded with no content.".to_owned()),
        Some(api::read_skill_result::Result::Error(error)) => {
            format!("Read skill error: {}", error.message)
        }
        None => "Read skill cancelled.".to_owned(),
    }
}

fn ask_user_question_result_to_prompt_text(result: &api::AskUserQuestionResult) -> String {
    match result.result.as_ref() {
        Some(api::ask_user_question_result::Result::Success(success)) => success
            .answers
            .iter()
            .map(|answer| {
                let value = match answer.answer.as_ref() {
                    Some(api::ask_user_question_result::answer_item::Answer::MultipleChoice(
                        choice,
                    )) => {
                        let mut selected = choice.selected_options.join(", ");
                        if !choice.other_text.is_empty() {
                            if !selected.is_empty() {
                                selected.push_str(", ");
                            }
                            selected.push_str(&choice.other_text);
                        }
                        selected
                    }
                    Some(api::ask_user_question_result::answer_item::Answer::Skipped(_)) => {
                        "skipped".to_owned()
                    }
                    None => "no answer".to_owned(),
                };
                format!("{}: {}", answer.question_id, value)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(api::ask_user_question_result::Result::Error(error)) => {
            format!("User question error: {}", error.message)
        }
        None => "User question cancelled.".to_owned(),
    }
}

fn file_content_to_prompt_text(file: &api::FileContent) -> String {
    format!("File: {}\n```text\n{}\n```", file.file_path, file.content)
}

fn any_file_content_to_prompt_text(file: &api::AnyFileContent) -> String {
    match file.content.as_ref() {
        Some(api::any_file_content::Content::TextContent(file)) => {
            file_content_to_prompt_text(file)
        }
        Some(api::any_file_content::Content::BinaryContent(file)) => {
            format!(
                "File: {}\n<binary content: {} bytes>",
                file.file_path,
                file.data.len()
            )
        }
        None => "<empty file content>".to_owned(),
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
        let message = if is_summarize_conversation_input(input) {
            message::Message::Summarization(message::Summarization {
                finished_duration: None,
                summary_type: Some(message::summarization::SummaryType::ConversationSummary(
                    message::summarization::ConversationSummary {
                        summary: text,
                        token_count: 0,
                    },
                )),
            })
        } else {
            message::Message::AgentOutput(message::AgentOutput { text })
        };

        messages.push(api::Message {
            id: Uuid::new_v4().to_string(),
            task_id: task_id.clone(),
            request_id: request_id.clone(),
            timestamp,
            server_message_data: String::new(),
            citations: vec![],
            message: Some(message),
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

fn is_summarize_conversation_input(input: &[AIAgentInput]) -> bool {
    input
        .iter()
        .any(|input| matches!(input, AIAgentInput::SummarizeConversation { .. }))
}

fn input_messages_for_task(
    input: &[AIAgentInput],
    task_id: &str,
    request_id: &str,
    timestamp: Option<prost_types::Timestamp>,
) -> Vec<api::Message> {
    input
        .iter()
        .filter_map(|input| input_to_task_message(input, task_id, request_id, timestamp))
        .collect()
}

fn input_to_task_message(
    input: &AIAgentInput,
    task_id: &str,
    request_id: &str,
    timestamp: Option<prost_types::Timestamp>,
) -> Option<api::Message> {
    let message = match input {
        AIAgentInput::UserQuery {
            query,
            context,
            user_query_mode,
            intended_agent,
            ..
        } => message::Message::UserQuery(message::UserQuery {
            query: query.clone(),
            context: input_context_to_api(context),
            referenced_attachments: Default::default(),
            mode: Some(api_user_query_mode(*user_query_mode)),
            intended_agent: intended_agent.map(|agent| agent as i32).unwrap_or_default(),
        }),
        AIAgentInput::InvokeSkill {
            skill,
            user_query,
            context,
        } => message::Message::InvokeSkill(message::InvokeSkill {
            skill: Some(skill.clone().into()),
            user_query: Some(message::UserQuery {
                query: user_query
                    .as_ref()
                    .map(|query| query.query.clone())
                    .unwrap_or_default(),
                context: input_context_to_api(context),
                referenced_attachments: Default::default(),
                mode: Some(api_user_query_mode(UserQueryMode::Normal)),
                intended_agent: Default::default(),
            }),
        }),
        AIAgentInput::InitProjectRules {
            context,
            display_query,
        } => message::Message::UserQuery(message::UserQuery {
            query: display_query
                .clone()
                .unwrap_or_else(|| commands::INIT.name.to_owned()),
            context: input_context_to_api(context),
            referenced_attachments: Default::default(),
            mode: Some(api_user_query_mode(UserQueryMode::Normal)),
            intended_agent: Default::default(),
        }),
        AIAgentInput::AutoCodeDiffQuery { query, context } => system_query_message(
            input_context_to_api(context),
            api::message::system_query::Type::AutoCodeDiff(message::AutoCodeDiff {
                query: query.clone(),
            }),
        ),
        AIAgentInput::CreateNewProject { query, context } => system_query_message(
            input_context_to_api(context),
            api::message::system_query::Type::CreateNewProject(message::CreateNewProject {
                query: query.clone(),
            }),
        ),
        AIAgentInput::CloneRepository {
            clone_repo_url,
            context,
        } => system_query_message(
            input_context_to_api(context),
            api::message::system_query::Type::CloneRepository(message::CloneRepository {
                url: clone_repo_url.url().to_owned(),
            }),
        ),
        AIAgentInput::CodeReview {
            review_comments, ..
        } => message::Message::CodeReview(message::CodeReview {
            comments: Some(api::ReviewComments {
                pending_comments: review_comments
                    .comments
                    .iter()
                    .filter(|comment| !comment.outdated)
                    .cloned()
                    .map(Into::into)
                    .collect(),
                completed_comments: vec![],
                diff_set: None,
            }),
        }),
        AIAgentInput::FetchReviewComments { repo_path, context } => system_query_message(
            input_context_to_api(context),
            api::message::system_query::Type::FetchReviewComments(message::FetchReviewComments {
                repo_path: repo_path.clone(),
            }),
        ),
        AIAgentInput::SummarizeConversation { prompt } => system_query_message(
            None,
            api::message::system_query::Type::SummarizeConversation(
                message::SummarizeConversation {
                    prompt: prompt.clone().unwrap_or_default(),
                },
            ),
        ),
        _ => return None,
    };

    Some(api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        request_id: request_id.to_owned(),
        timestamp,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message),
    })
}

fn system_query_message(
    context: Option<api::InputContext>,
    r#type: api::message::system_query::Type,
) -> message::Message {
    message::Message::SystemQuery(message::SystemQuery {
        context,
        r#type: Some(r#type),
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
#[path = "openrouter_tests.rs"]
mod tests;
