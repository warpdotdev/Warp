use prost_types::FieldMask;
use serde::{Deserialize, Serialize};

use warp_multi_agent_api as api;

use crate::ai::agent::api::{Event, RequestParams};
use crate::ai::agent::AIAgentActionResultType;

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<OpenAiTool>,
    pub stream: bool,
    pub task_id: String,
    pub conversation_id: Option<String>,
    has_new_user_facing_content: bool,
    pub user_query: Option<String>,
}

impl OpenAiCompatibleRequest {
    pub fn has_user_facing_content(&self) -> bool {
        self.has_new_user_facing_content
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAiFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiFunctionDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<OpenAiTool>,
    pub stream: bool,
}

impl OpenAiChatRequest {
    pub fn from_request(request: OpenAiCompatibleRequest, model_id: &str) -> Self {
        Self {
            model: model_id.to_string(),
            messages: request.messages,
            tools: request.tools,
            stream: request.stream,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiChatStreamDelta {
    pub choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiStreamChoice {
    pub index: u32,
    pub delta: OpenAiStreamDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiStreamDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<OpenAiStreamToolCall>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiStreamToolCall {
    pub index: u32,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub call_type: Option<String>,
    #[serde(default)]
    pub function: Option<OpenAiStreamFunction>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiStreamFunction {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

const TOOL_RUN_SHELL_COMMAND: &str = "run_shell_command";
const TOOL_READ_FILES: &str = "read_files";
const TOOL_APPLY_FILE_DIFFS: &str = "apply_file_diffs";
const TOOL_SEARCH_CODEBASE: &str = "search_codebase";
const TOOL_GREP: &str = "grep";
const TOOL_FILE_GLOB: &str = "file_glob";

pub fn get_tool_definitions() -> Vec<OpenAiTool> {
    vec![
        OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunctionDef {
                name: TOOL_RUN_SHELL_COMMAND.to_string(),
                description: Some("Run a shell command and return the output. Use this to execute commands, run scripts, install packages, etc.".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to run"
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
        OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunctionDef {
                name: TOOL_READ_FILES.to_string(),
                description: Some("Read the contents of files at the given paths. Returns the file contents.".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of file paths to read"
                        }
                    },
                    "required": ["paths"]
                }),
            },
        },
        OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunctionDef {
                name: TOOL_APPLY_FILE_DIFFS.to_string(),
                description: Some("Apply file edits using search-and-replace diffs. Each diff specifies a file path, the text to search for, and the text to replace it with. Can also create new files.".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "summary": {
                            "type": "string",
                            "description": "A brief summary of the changes"
                        },
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
                            },
                            "description": "List of search-and-replace diffs"
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
                            },
                            "description": "List of new files to create"
                        }
                    },
                    "required": ["summary"]
                }),
            },
        },
        OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunctionDef {
                name: TOOL_SEARCH_CODEBASE.to_string(),
                description: Some("Search the codebase for relevant files based on a query. Returns matching file paths and relevant snippets.".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        }
                    },
                    "required": ["query"]
                }),
            },
        },
        OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunctionDef {
                name: TOOL_GREP.to_string(),
                description: Some("Search for patterns in files using regular expressions. Returns matching lines with context.".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "queries": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "The search patterns"
                        },
                        "path": {
                            "type": "string",
                            "description": "The directory to search in"
                        }
                    },
                    "required": ["queries"]
                }),
            },
        },
        OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunctionDef {
                name: TOOL_FILE_GLOB.to_string(),
                description: Some("Find files matching glob patterns. Returns matching file paths.".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "patterns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Glob patterns to match (e.g. \"**/*.rs\", \"src/**/*.ts\")"
                        },
                        "path": {
                            "type": "string",
                            "description": "The directory to search in (optional)"
                        }
                    },
                    "required": ["patterns"]
                }),
            },
        },
    ]
}

pub struct StreamingState {
    message_id: Option<String>,
    tool_calls_accumulated: Vec<OpenAiToolCall>,
}

impl StreamingState {
    pub fn new() -> Self {
        Self {
            message_id: None,
            tool_calls_accumulated: Vec::new(),
        }
    }

    pub fn take_accumulated_tool_calls(&mut self) -> Vec<OpenAiToolCall> {
        std::mem::take(&mut self.tool_calls_accumulated)
    }
}

pub fn from_request_params(params: &RequestParams) -> OpenAiCompatibleRequest {
    use crate::ai::agent::AIAgentContext;

    let mut messages = Vec::new();
    let mut conversation_id: Option<String> = None;

    if let Some(ref token) = params.conversation_token {
        conversation_id = Some(token.as_str().to_string());
    }

    for task in &params.tasks {
        for msg in &task.messages {
            if let Some(chat_msg) = proto_message_to_chat_message(msg) {
                messages.push(chat_msg);
            }
        }
    }

    let mut context_parts: Vec<String> = Vec::new();
    let mut has_user_query = false;
    let mut has_action_result = false;
    let mut last_user_query: Option<String> = None;

    for input in &params.input {
        if let Some(context_items) = input.context() {
            for ctx in context_items {
                match ctx {
                    AIAgentContext::Directory { pwd, .. } => {
                        if let Some(pwd) = pwd {
                            context_parts.push(format!("Current directory: {}", pwd));
                        }
                    }
                    AIAgentContext::SelectedText(text) => {
                        context_parts.push(format!("Selected text:\n{}", text));
                    }
                    AIAgentContext::CurrentTime { current_time } => {
                        context_parts.push(format!("Current time: {}", current_time.format("%Y-%m-%d %H:%M:%S %Z")));
                    }
                    AIAgentContext::Git { head, branch } => {
                        if let Some(branch) = branch {
                            context_parts.push(format!("Git: branch={}, commit={}", branch, head));
                        } else {
                            context_parts.push(format!("Git: commit={}", head));
                        }
                    }
                    AIAgentContext::ExecutionEnvironment(env) => {
                        context_parts.push(format!("Shell: {}", env.shell_name));
                    }
                    _ => {}
                }
            }
        }

        if let Some(action_result) = input.action_result() {
            has_action_result = true;
            let tool_call_id = action_result.id.to_string();
            let content = format_action_result(&action_result.result);
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: Some(serde_json::Value::String(content)),
                tool_calls: None,
                tool_call_id: Some(tool_call_id),
                name: None,
            });
        }

        if let Some(query) = input.user_query() {
            has_user_query = true;
            last_user_query = Some(query.clone());
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: Some(serde_json::Value::String(query)),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        match input {
            crate::ai::agent::AIAgentInput::MessagesReceivedFromAgents { .. }
            | crate::ai::agent::AIAgentInput::EventsFromAgents { .. } => {
                log::debug!("Custom endpoint: ignoring orchestration event input (not supported by custom endpoints)");
            }
            _ => {}
        }
    }

    let mut system_parts: Vec<String> = Vec::new();
    if !context_parts.is_empty() {
        system_parts.push(format!("Context:\n{}", context_parts.join("\n")));
    }
    if !messages.iter().any(|m| m.role == "assistant") {
        system_parts.push(
            "You have access to tools. Use them to accomplish tasks. \
             When you need to run a command, use run_shell_command. \
             To read files, use read_files. \
             To edit files, use apply_file_diffs. \
             To search the codebase, use search_codebase. \
             To search file contents, use grep. \
             To find files by pattern, use file_glob. \
             Always use tools rather than asking the user to perform actions. \
             If a tool call fails, read the error message carefully and retry with corrected parameters.".to_string()
        );
    }
    if !system_parts.is_empty() {
        let system_content = system_parts.join("\n\n");
        let system_msg = ChatMessage {
            role: "system".to_string(),
            content: Some(serde_json::Value::String(system_content)),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        if messages.first().map_or(false, |m| m.role == "system") {
            messages[0] = system_msg;
        } else {
            messages.insert(0, system_msg);
        }
    }

    let task_id = params.root_task_id.clone();
    if task_id.is_empty() {
        log::warn!("Custom endpoint: root_task_id is empty in RequestParams, messages will not render");
    }

    let has_tool_call_results_in_tasks = params.tasks.iter().any(|t| {
        t.messages.iter().any(|m| {
            matches!(m.message, Some(api::message::Message::ToolCallResult(_)))
        })
    });

    log::info!(
        "Custom endpoint: built request with {} messages, has_user_query={}, has_action_result={}, has_tool_call_results={}, conversation_id={:?}",
        messages.len(),
        has_user_query,
        has_action_result,
        has_tool_call_results_in_tasks,
        conversation_id,
    );

    OpenAiCompatibleRequest {
        messages,
        tools: get_tool_definitions(),
        stream: true,
        task_id,
        conversation_id,
        has_new_user_facing_content: has_user_query || has_action_result || has_tool_call_results_in_tasks,
        user_query: last_user_query,
    }
}

fn proto_message_to_chat_message(msg: &api::Message) -> Option<ChatMessage> {
    match &msg.message {
        Some(api::message::Message::UserQuery(uq)) => {
            Some(ChatMessage {
                role: "user".to_string(),
                content: Some(serde_json::Value::String(uq.query.clone())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })
        }
        Some(api::message::Message::AgentOutput(ao)) => {
            Some(ChatMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::Value::String(ao.text.clone())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })
        }
        Some(api::message::Message::ToolCall(tc)) => {
            let (name, arguments) = proto_tool_call_to_openai(&tc.tool)?;
            Some(ChatMessage {
                role: "assistant".to_string(),
                content: None,
                tool_calls: Some(vec![OpenAiToolCall {
                    id: tc.tool_call_id.clone(),
                    call_type: "function".to_string(),
                    function: OpenAiFunctionCall { name, arguments },
                }]),
                tool_call_id: None,
                name: None,
            })
        }
        Some(api::message::Message::ToolCallResult(tcr)) => {
            let content = format_tool_call_result(tcr);
            Some(ChatMessage {
                role: "tool".to_string(),
                content: Some(serde_json::Value::String(content)),
                tool_calls: None,
                tool_call_id: Some(tcr.tool_call_id.clone()),
                name: None,
            })
        }
        Some(api::message::Message::AgentReasoning(ar)) => {
            Some(ChatMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::Value::String(format!("[thinking]\n{}\n[/thinking]", ar.reasoning))),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })
        }
        _ => None,
    }
}

fn proto_tool_call_to_openai(tool: &Option<api::message::tool_call::Tool>) -> Option<(String, String)> {
    use api::message::tool_call::Tool as T;

    let tool = tool.as_ref()?;
    let (name, args) = match tool {
        T::RunShellCommand(rsc) => (
            TOOL_RUN_SHELL_COMMAND,
            serde_json::json!({ "command": rsc.command }),
        ),
        T::ReadFiles(rf) => (
            TOOL_READ_FILES,
            serde_json::json!({
                "paths": rf.files.iter().map(|f| &f.name).collect::<Vec<_>>()
            }),
        ),
        T::ApplyFileDiffs(afd) => (
            TOOL_APPLY_FILE_DIFFS,
            serde_json::json!({
                "summary": afd.summary,
                "diffs": afd.diffs.iter().map(|d| serde_json::json!({
                    "file_path": d.file_path,
                    "search": d.search,
                    "replace": d.replace,
                })).collect::<Vec<_>>(),
                "new_files": afd.new_files.iter().map(|f| serde_json::json!({
                    "file_path": f.file_path,
                    "content": f.content,
                })).collect::<Vec<_>>(),
            }),
        ),
        T::SearchCodebase(sc) => (
            TOOL_SEARCH_CODEBASE,
            serde_json::json!({ "query": sc.query }),
        ),
        T::Grep(g) => (
            TOOL_GREP,
            serde_json::json!({ "queries": g.queries, "path": g.path }),
        ),
        T::FileGlobV2(fg) => (
            TOOL_FILE_GLOB,
            serde_json::json!({ "patterns": fg.patterns, "path": fg.search_dir }),
        ),
        T::Server(s) => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&s.payload) {
                let name = parsed.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                let args_str = parsed.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                if serde_json::from_str::<serde_json::Value>(args_str).is_err() {
                    log::warn!("Custom endpoint: skipping tool call with invalid JSON arguments in stored message (name={name})");
                    return None;
                }
                let args = args_str.to_string();
                return Some((name, args));
            }
            return None;
        }
        _ => return None,
    };
    Some((name.to_string(), serde_json::to_string(&args).unwrap_or_default()))
}

fn format_tool_call_result(tcr: &api::message::ToolCallResult) -> String {
    use api::message::tool_call_result::Result as R;
    use api::run_shell_command_result::Result as ShellResult;
    use api::read_files_result::Result as ReadResult;
    use api::apply_file_diffs_result::Result as EditResult;
    use api::search_codebase_result::Result as SearchResult;
    use api::grep_result::Result as GrepResult;
    use api::file_glob_v2_result::Result as GlobResult;

    match &tcr.result {
        Some(R::RunShellCommand(rsc)) => match &rsc.result {
            Some(ShellResult::CommandFinished(finished)) => {
                format!("Exit code: {}\nOutput:\n{}", finished.exit_code, finished.output)
            }
            Some(ShellResult::LongRunningCommandSnapshot(snap)) => {
                format!("Command still running. Current output:\n{}", snap.output)
            }
            Some(ShellResult::PermissionDenied(_)) => "Command was denied permission.".to_string(),
            None => format!("Shell command completed (tool_call_id: {})", tcr.tool_call_id),
        },
        Some(R::ReadFiles(rf)) => match &rf.result {
            Some(ReadResult::TextFilesSuccess(s)) => {
                s.files.iter().map(|f| format!("--- {} ---\n{}", f.file_path, f.content)).collect::<Vec<_>>().join("\n\n")
            }
            Some(ReadResult::AnyFilesSuccess(s)) => {
                s.files.iter().map(|f| {
                    match &f.content {
                        Some(api::any_file_content::Content::TextContent(tc)) => format!("--- {} ---\n{}", tc.file_path, tc.content),
                        Some(api::any_file_content::Content::BinaryContent(bc)) => format!("--- {} --- [binary]", bc.file_path),
                        None => "[empty]".to_string(),
                    }
                }).collect::<Vec<_>>().join("\n\n")
            }
            Some(ReadResult::Error(e)) => format!("Error reading files: {}", e.message),
            None => "Read files completed.".to_string(),
        },
        Some(R::ApplyFileDiffs(afd)) => match &afd.result {
            Some(EditResult::Success(s)) => {
                let paths: Vec<&str> = s.updated_files.iter().map(|f| f.file_path.as_str()).collect();
                format!("File edits applied. Files: {}", paths.join(", "))
            }
            Some(EditResult::Error(e)) => format!("Error applying edits: {}", e.message),
            None => "File edits completed.".to_string(),
        },
        Some(R::SearchCodebase(sc)) => match &sc.result {
            Some(SearchResult::Success(s)) => {
                s.files.iter().map(|f| format!("--- {} ---\n{}", f.file_path, f.content)).collect::<Vec<_>>().join("\n\n")
            }
            Some(SearchResult::Error(e)) => format!("Search failed: {}", e.message),
            None => "Search completed.".to_string(),
        },
        Some(R::Grep(g)) => match &g.result {
            Some(GrepResult::Success(s)) => {
                s.matched_files.iter().map(|m| {
                    let lines: Vec<String> = m.matched_lines.iter().map(|l| format!("line {}", l.line_number)).collect();
                    format!("{}: {}", m.file_path, lines.join(", "))
                }).collect::<Vec<_>>().join("\n")
            }
            Some(GrepResult::Error(e)) => format!("Grep error: {}", e.message),
            None => "Grep completed.".to_string(),
        },
        Some(R::FileGlobV2(fg)) => match &fg.result {
            Some(GlobResult::Success(s)) => {
                s.matched_files.iter().map(|m| m.file_path.clone()).collect::<Vec<_>>().join("\n")
            }
            Some(GlobResult::Error(e)) => format!("File glob error: {}", e.message),
            None => "File glob completed.".to_string(),
        },
        Some(R::Server(s)) => s.serialized_result.clone(),
        Some(R::Cancel(_)) => "Action was cancelled.".to_string(),
        _ => format!("Tool result (tool_call_id: {})", tcr.tool_call_id),
    }
}

pub fn make_create_task_event(task_id: &str) -> api::ResponseEvent {
    use api::client_action as api_client_action;
    use api::response_event as api_response_event;

    let task = api::Task {
        id: task_id.to_string(),
        description: String::new(),
        dependencies: None,
        messages: vec![],
        summary: String::new(),
        server_data: String::new(),
    };
    api::ResponseEvent {
        r#type: Some(api_response_event::Type::ClientActions(
            api_response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api_client_action::Action::CreateTask(
                        api_client_action::CreateTask { task: Some(task) },
                    )),
                }],
            },
        )),
    }
}

fn make_add_messages_client_action(message: api::Message, task_id: &str) -> api::response_event::ClientActions {
    api::response_event::ClientActions {
        actions: vec![api::ClientAction {
            action: Some(api::client_action::Action::AddMessagesToTask(
                api::client_action::AddMessagesToTask {
                    task_id: task_id.to_string(),
                    messages: vec![message],
                },
            )),
        }],
    }
}

fn make_text_client_action(text: String, message_id: String, task_id: &str) -> api::response_event::ClientActions {
    let message = api::Message {
        id: message_id,
        task_id: task_id.to_string(),
        request_id: String::new(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentOutput(api::message::AgentOutput {
            text,
        })),
    };
    make_add_messages_client_action(message, task_id)
}

pub fn make_user_query_client_action(query: String, task_id: &str) -> api::response_event::ClientActions {
    let message = api::Message {
        id: uuid::Uuid::new_v4().to_string(),
        task_id: task_id.to_string(),
        request_id: String::new(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::UserQuery(api::message::UserQuery {
            query,
            context: None,
            referenced_attachments: Default::default(),
            mode: None,
            intended_agent: 0,
        })),
    };
    make_add_messages_client_action(message, task_id)
}

fn make_append_text_client_action(text: String, message_id: String, task_id: &str) -> api::response_event::ClientActions {
    let message = api::Message {
        id: message_id.clone(),
        task_id: task_id.to_string(),
        request_id: String::new(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentOutput(api::message::AgentOutput {
            text,
        })),
    };
    api::response_event::ClientActions {
        actions: vec![api::ClientAction {
            action: Some(api::client_action::Action::AppendToMessageContent(
                api::client_action::AppendToMessageContent {
                    task_id: task_id.to_string(),
                    message: Some(message),
                    mask: Some(FieldMask {
                        paths: vec!["agent_output.text".to_string()],
                    }),
                },
            )),
        }],
    }
}

pub fn delta_to_response_events(
    delta: OpenAiChatStreamDelta,
    task_id: &str,
    state: &mut StreamingState,
) -> Vec<Event> {
    let mut events = Vec::new();

    for (i, choice) in delta.choices.into_iter().enumerate() {
        let d = choice.delta;

        if let Some(content) = d.content {
            if !content.is_empty() {
                let is_first = state.message_id.is_none();
                let message_id = if let Some(ref mid) = state.message_id {
                    mid.clone()
                } else {
                    let mid = if let Some(ref chunk_id) = delta.id {
                        format!("{}_{}", chunk_id, i)
                    } else {
                        format!("custom_stream_{}", uuid::Uuid::new_v4())
                    };
                    state.message_id = Some(mid.clone());
                    mid
                };

                let client_actions = if is_first {
                    make_text_client_action(content, message_id, task_id)
                } else {
                    make_append_text_client_action(content, message_id, task_id)
                };

                events.push(Ok(api::ResponseEvent {
                    r#type: Some(api::response_event::Type::ClientActions(client_actions)),
                }));
            }
        } else {
            if let Some(_role) = d.role {
                log::debug!("Custom endpoint: skipping role-only chunk (role={})", _role);
            }
        }

        if let Some(tool_calls) = d.tool_calls {
            for tc in tool_calls {
                let idx = tc.index as usize;
                while state.tool_calls_accumulated.len() <= idx {
                    state.tool_calls_accumulated.push(OpenAiToolCall {
                        id: String::new(),
                        call_type: "function".to_string(),
                        function: OpenAiFunctionCall {
                            name: String::new(),
                            arguments: String::new(),
                        },
                    });
                }
                let accumulated = &mut state.tool_calls_accumulated[idx];
                if let Some(id) = tc.id {
                    accumulated.id = id;
                }
                if let Some(func) = tc.function {
                    if let Some(name) = func.name {
                        accumulated.function.name = name;
                    }
                    if let Some(args) = func.arguments {
                        accumulated.function.arguments.push_str(&args);
                    }
                }
            }
        }
    }

    if events.is_empty() {
        log::debug!("Custom endpoint delta produced 0 events (likely role-only or empty content chunk)");
    }

    events
}

pub fn finalize_tool_call_events(
    tool_calls: Vec<OpenAiToolCall>,
    task_id: &str,
) -> Vec<Event> {
    if tool_calls.is_empty() {
        return vec![];
    }

    let mut actions = Vec::new();
    for tc in &tool_calls {
        let action_type = match openai_tool_call_to_action(tc) {
            Ok(a) => a,
            Err(e) => {
                log::warn!("Failed to convert tool call '{}' to action: {}", tc.function.name, e);
                let available: String = get_tool_definitions()
                    .iter()
                    .map(|t| t.function.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let model_facing_msg = format!(
                    "Error: tool call '{}' failed: {}. Available tools: {}. \
                     Please retry with a valid tool name and correct JSON arguments.",
                    tc.function.name, e, available
                );
                let sanitized_args = serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                    .ok()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| serde_json::json!({}).to_string());
                let payload = serde_json::json!({
                    "name": tc.function.name,
                    "arguments": sanitized_args,
                    "error": e,
                }).to_string();
                let visible_id = format!("{}_visible", tc.id);
                actions.push(api::ClientAction {
                    action: Some(api::client_action::Action::AddMessagesToTask(
                        api::client_action::AddMessagesToTask {
                            task_id: task_id.to_string(),
                            messages: vec![
                                api::Message {
                                    id: tc.id.clone(),
                                    task_id: task_id.to_string(),
                                    request_id: String::new(),
                                    timestamp: None,
                                    server_message_data: String::new(),
                                    citations: vec![],
                                    message: Some(api::message::Message::ToolCall(
                                        api::message::ToolCall {
                                            tool_call_id: tc.id.clone(),
                                            tool: Some(api::message::tool_call::Tool::Server(
                                                api::message::tool_call::Server {
                                                    payload,
                                                },
                                            )),
                                        },
                                    )),
                                },
                                api::Message {
                                    id: format!("{}_error", tc.id),
                                    task_id: task_id.to_string(),
                                    request_id: String::new(),
                                    timestamp: None,
                                    server_message_data: String::new(),
                                    citations: vec![],
                                    message: Some(api::message::Message::ToolCallResult(
                                        api::message::ToolCallResult {
                                            tool_call_id: tc.id.clone(),
                                            result: Some(
                                                api::message::tool_call_result::Result::Server(
                                                    api::message::tool_call_result::ServerResult {
                                                        serialized_result: model_facing_msg,
                                                    },
                                                ),
                                            ),
                                            context: None,
                                        },
                                    )),
                                },
                                api::Message {
                                    id: visible_id,
                                    task_id: task_id.to_string(),
                                    request_id: String::new(),
                                    timestamp: None,
                                    server_message_data: String::new(),
                                    citations: vec![],
                                    message: Some(api::message::Message::AgentOutput(
                                        api::message::AgentOutput {
                                            text: format!("AI made an unknown tool call: '{}'", tc.function.name),
                                        },
                                    )),
                                },
                            ],
                        },
                    )),
                });
                continue;
            }
        };
        actions.push(api::ClientAction {
            action: Some(api::client_action::Action::AddMessagesToTask(
                api::client_action::AddMessagesToTask {
                    task_id: task_id.to_string(),
                    messages: vec![api::Message {
                        id: tc.id.clone(),
                        task_id: task_id.to_string(),
                        request_id: String::new(),
                        timestamp: None,
                        server_message_data: String::new(),
                        citations: vec![],
                        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
                            tool_call_id: tc.id.clone(),
                            tool: Some(action_type),
                        })),
                    }],
                },
            )),
        });
    }

    actions.into_iter().map(|action| {
        Ok(api::ResponseEvent {
            r#type: Some(api::response_event::Type::ClientActions(
                api::response_event::ClientActions { actions: vec![action] },
            )),
        })
    }).collect()
}

fn openai_tool_call_to_action(tc: &OpenAiToolCall) -> Result<api::message::tool_call::Tool, String> {
    let args: serde_json::Value = if tc.function.arguments.is_empty() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(&tc.function.arguments).map_err(|e| format!("Invalid JSON arguments: {e}"))?
    };

    match tc.function.name.as_str() {
        TOOL_RUN_SHELL_COMMAND => {
            let command = args.get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command,
                    is_read_only: false,
                    uses_pager: false,
                    citations: vec![],
                    is_risky: true,
                    risk_category: 0,
                    wait_until_complete_value: Some(
                        api::message::tool_call::run_shell_command::WaitUntilCompleteValue::WaitUntilComplete(true)
                    ),
                },
            ))
        }
        TOOL_READ_FILES => {
            let paths: Vec<String> = args.get("paths")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
                .unwrap_or_default();
            Ok(api::message::tool_call::Tool::ReadFiles(
                api::message::tool_call::ReadFiles {
                    files: paths.into_iter().map(|p| api::message::tool_call::read_files::File {
                        name: p,
                        line_ranges: vec![],
                    }).collect(),
                },
            ))
        }
        TOOL_APPLY_FILE_DIFFS => {
            let summary = args.get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let diffs = args.get("diffs")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|d| {
                    let file_path = d.get("file_path")?.as_str()?.to_string();
                    let search = d.get("search")?.as_str()?.to_string();
                    let replace = d.get("replace")?.as_str()?.to_string();
                    Some(api::message::tool_call::apply_file_diffs::FileDiff {
                        file_path, search, replace,
                    })
                }).collect())
                .unwrap_or_default();
            let new_files = args.get("new_files")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|d| {
                    let file_path = d.get("file_path")?.as_str()?.to_string();
                    let content = d.get("content")?.as_str()?.to_string();
                    Some(api::message::tool_call::apply_file_diffs::NewFile {
                        file_path, content,
                    })
                }).collect())
                .unwrap_or_default();
            Ok(api::message::tool_call::Tool::ApplyFileDiffs(
                api::message::tool_call::ApplyFileDiffs {
                    summary,
                    diffs,
                    new_files,
                    deleted_files: vec![],
                    v4a_updates: vec![],
                },
            ))
        }
        TOOL_SEARCH_CODEBASE => {
            let query = args.get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(api::message::tool_call::Tool::SearchCodebase(
                api::message::tool_call::SearchCodebase {
                    query,
                    path_filters: vec![],
                    codebase_path: String::new(),
                },
            ))
        }
        TOOL_GREP => {
            let queries = args.get("queries")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let path = args.get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(api::message::tool_call::Tool::Grep(
                api::message::tool_call::Grep { queries, path },
            ))
        }
        TOOL_FILE_GLOB => {
            let patterns = args.get("patterns")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let path = args.get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(api::message::tool_call::Tool::FileGlobV2(
                api::message::tool_call::FileGlobV2 {
                    patterns,
                    search_dir: path,
                    max_matches: 50,
                    max_depth: 0,
                    min_depth: 0,
                },
            ))
        }
        _ => Err(format!(
            "Unknown tool: '{}'. Available tools: run_shell_command, read_files, apply_file_diffs, search_codebase, grep, file_glob",
            tc.function.name
        )),
    }
}

pub fn format_action_result(result: &AIAgentActionResultType) -> String {
    match result {
        AIAgentActionResultType::RequestCommandOutput(cmd_result) => {
            match cmd_result {
                ai::agent::action_result::RequestCommandOutputResult::Completed { output, exit_code, .. } => {
                    format!("Exit code: {}\nOutput:\n{}", exit_code, output)
                }
                ai::agent::action_result::RequestCommandOutputResult::LongRunningCommandSnapshot { grid_contents, .. } => {
                    format!("Command is still running. Current output:\n{}", grid_contents)
                }
                ai::agent::action_result::RequestCommandOutputResult::CancelledBeforeExecution => {
                    "Command was cancelled before execution.".to_string()
                }
                ai::agent::action_result::RequestCommandOutputResult::Denylisted { command } => {
                    format!("Command '{}' was denied.", command)
                }
            }
        }
        AIAgentActionResultType::ReadFiles(read_result) => {
            match read_result {
                ai::agent::action_result::ReadFilesResult::Success { files } => {
                    files.iter().map(|f| {
                        let content = match &f.content {
                            ai::agent::action_result::AnyFileContent::StringContent(s) => s.clone(),
                            ai::agent::action_result::AnyFileContent::BinaryContent(_) => "[binary content]".to_string(),
                        };
                        format!("--- {} ---\n{}", f.file_name, content)
                    }).collect::<Vec<_>>().join("\n\n")
                }
                ai::agent::action_result::ReadFilesResult::Error(e) => format!("Error reading files: {}", e),
                ai::agent::action_result::ReadFilesResult::Cancelled => "Read files was cancelled.".to_string(),
            }
        }
        AIAgentActionResultType::RequestFileEdits(edit_result) => {
            match edit_result {
                ai::agent::action_result::RequestFileEditsResult::Success { diff, updated_files, .. } => {
                    let paths: Vec<String> = updated_files.iter().map(|u| u.file_context.file_name.clone()).collect();
                    format!("File edits applied successfully. Files: {}. Diff:\n{}", paths.join(", "), diff)
                }
                ai::agent::action_result::RequestFileEditsResult::Cancelled => "File edits were cancelled.".to_string(),
                ai::agent::action_result::RequestFileEditsResult::DiffApplicationFailed { error } => format!("Error applying file edits: {}", error),
            }
        }
        AIAgentActionResultType::SearchCodebase(search_result) => {
            match search_result {
                ai::agent::action_result::SearchCodebaseResult::Success { files } => {
                    files.iter().map(|f| {
                        let content = match &f.content {
                            ai::agent::action_result::AnyFileContent::StringContent(s) => s.clone(),
                            ai::agent::action_result::AnyFileContent::BinaryContent(_) => "[binary content]".to_string(),
                        };
                        format!("--- {} ---\n{}", f.file_name, content)
                    }).collect::<Vec<_>>().join("\n\n")
                }
                ai::agent::action_result::SearchCodebaseResult::Failed { message, .. } => format!("Search failed: {}", message),
                ai::agent::action_result::SearchCodebaseResult::Cancelled => "Search was cancelled.".to_string(),
            }
        }
        AIAgentActionResultType::Grep(grep_result) => {
            match grep_result {
                ai::agent::action_result::GrepResult::Success { matched_files } => {
                    matched_files.iter().map(|m| {
                        let lines: Vec<String> = m.matched_lines.iter().map(|l| format!("line {}", l.line_number)).collect();
                        format!("{}: {}", m.file_path, lines.join(", "))
                    }).collect::<Vec<_>>().join("\n")
                }
                ai::agent::action_result::GrepResult::Error(e) => format!("Grep error: {}", e),
                ai::agent::action_result::GrepResult::Cancelled => "Grep was cancelled.".to_string(),
            }
        }
        AIAgentActionResultType::FileGlob(glob_result) => {
            match glob_result {
                ai::agent::action_result::FileGlobResult::Success { matched_files } => matched_files.clone(),
                ai::agent::action_result::FileGlobResult::Error(e) => format!("File glob error: {}", e),
                ai::agent::action_result::FileGlobResult::Cancelled => "File glob was cancelled.".to_string(),
            }
        }
        AIAgentActionResultType::FileGlobV2(glob_result) => {
            match glob_result {
                ai::agent::action_result::FileGlobV2Result::Success { matched_files, .. } => {
                    matched_files.iter().map(|m| m.file_path.clone()).collect::<Vec<_>>().join("\n")
                }
                ai::agent::action_result::FileGlobV2Result::Error(e) => format!("File glob error: {}", e),
                ai::agent::action_result::FileGlobV2Result::Cancelled => "File glob was cancelled.".to_string(),
            }
        }
        _ => format!("{}", result),
    }
}
