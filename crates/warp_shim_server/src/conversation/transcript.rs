use warp_multi_agent_api as api;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TranscriptRole {
    User,
    Assistant,
    Tool,
}

impl TranscriptRole {
    pub(crate) fn as_openai_role(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptMessage {
    pub(crate) role: TranscriptRole,
    pub(crate) content: String,
    pub(crate) tool_call_id: Option<String>,
    pub(crate) tool_calls: Vec<TranscriptToolCall>,
}

impl TranscriptMessage {
    pub(crate) fn user(content: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::User,
            content: content.into(),
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    pub(crate) fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    pub(crate) fn assistant_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<TranscriptToolCall>,
    ) -> Self {
        Self {
            role: TranscriptRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            tool_calls,
        }
    }

    pub(crate) fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WarpToolKind {
    Builtin(api::ToolType),
    Mcp {
        server_id: String,
        server_name: String,
        tool_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingToolCall {
    pub(crate) warp_tool_call_id: String,
    pub(crate) openai_tool_call_id: String,
    pub(crate) openai_tool_name: String,
    pub(crate) kind: WarpToolKind,
}

pub(crate) fn history_from_request(request: &api::Request) -> Vec<TranscriptMessage> {
    request
        .task_context
        .as_ref()
        .into_iter()
        .flat_map(|context| &context.tasks)
        .flat_map(|task| &task.messages)
        .filter_map(transcript_message_from_api_message)
        .collect()
}

fn transcript_message_from_api_message(message: &api::Message) -> Option<TranscriptMessage> {
    match message.message.as_ref()? {
        api::message::Message::UserQuery(user_query) => non_empty(&user_query.query).map(|query| {
            TranscriptMessage::user(format_with_context(query, user_query.context.as_ref()))
        }),
        api::message::Message::AgentOutput(output) => {
            non_empty(&output.text).map(TranscriptMessage::assistant)
        }
        api::message::Message::SystemQuery(system_query) => system_query_to_text(system_query)
            .map(|query| format_with_context(query, system_query.context.as_ref()))
            .map(TranscriptMessage::user),
        _ => None,
    }
}

fn system_query_to_text(system_query: &api::message::SystemQuery) -> Option<&str> {
    match system_query.r#type.as_ref()? {
        api::message::system_query::Type::AutoCodeDiff(query) => non_empty(&query.query),
        api::message::system_query::Type::CreateNewProject(query) => non_empty(&query.query),
        api::message::system_query::Type::CloneRepository(query) => non_empty(&query.url),
        api::message::system_query::Type::SummarizeConversation(query) => non_empty(&query.prompt),
        api::message::system_query::Type::FetchReviewComments(query) => non_empty(&query.repo_path),
        api::message::system_query::Type::ResumeConversation(_) => {
            Some("Continue the conversation.")
        }
        api::message::system_query::Type::GeneratePassiveSuggestions(_) => None,
    }
}

fn format_with_context(query: &str, context: Option<&api::InputContext>) -> String {
    let Some(context) = context else {
        return query.to_string();
    };

    let context = super::super::upstream::prompt::context_prefix(context);
    if context.is_empty() {
        query.to_string()
    } else {
        format!("{context}\n\nUser request:\n{query}")
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}
