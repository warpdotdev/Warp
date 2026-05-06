use serde::Deserialize;

use crate::terminal::CLIAgent;

use super::{CLIAgentEvent, CLIAgentEventPayload, CLIAgentEventType};

/// Resolves a CLI agent from the `"agent"` string in a CLI agent event.
/// Returns `None` if the string doesn't match any known agent.
fn resolve_agent(agent: &str) -> Option<CLIAgent> {
    enum_iterator::all::<CLIAgent>()
        .find(|a| !matches!(a, CLIAgent::Unknown) && a.command_prefix() == agent)
}

pub(super) fn parse(body: &str) -> Option<CLIAgentEvent> {
    let raw: RawEvent = serde_json::from_str(body).ok()?;

    let event = match raw.event.as_str() {
        "session_start" => CLIAgentEventType::SessionStart,
        "prompt_submit" => CLIAgentEventType::PromptSubmit,
        "tool_complete" => CLIAgentEventType::ToolComplete,
        "stop" => CLIAgentEventType::Stop,
        "permission_request" => CLIAgentEventType::PermissionRequest,
        "permission_replied" => CLIAgentEventType::PermissionReplied,
        "question_asked" => CLIAgentEventType::QuestionAsked,
        "idle_prompt" => CLIAgentEventType::IdlePrompt,
        // Helios converged-panel events
        "metrics_update" => CLIAgentEventType::MetricsUpdate,
        "subagent_update" => CLIAgentEventType::SubagentUpdate,
        "system_message" => CLIAgentEventType::SystemMessage,
        "interview_request" => CLIAgentEventType::InterviewRequest,
        "decision_request" => CLIAgentEventType::DecisionRequest,
        "tool_result" => CLIAgentEventType::ToolResult,
        other => CLIAgentEventType::Unknown(other.to_string()),
    };

    let tool_input_preview = raw.tool_input.as_ref().and_then(|val| {
        val.get("command")
            .or_else(|| val.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });

    let agent = raw
        .agent
        .as_deref()
        .and_then(resolve_agent)
        .unwrap_or(CLIAgent::Unknown);

    Some(CLIAgentEvent {
        v: raw.v.unwrap_or(1),
        agent,
        event,
        session_id: raw.session_id,
        cwd: raw.cwd,
        project: raw.project,
        payload: CLIAgentEventPayload {
            query: raw.query,
            response: raw.response,
            transcript_path: raw.transcript_path,
            summary: raw.summary,
            tool_name: raw.tool_name,
            tool_input_preview,
            plugin_version: raw.plugin_version,
            // Rich tool execution data
            tool_result: raw.tool_result,
            tool_error: raw.tool_error,
            tool_duration_ms: raw.tool_duration_ms,
            // LLM metrics
            input_tokens: raw.input_tokens,
            output_tokens: raw.output_tokens,
            cost_usd: raw.cost_usd,
            model: raw.model,
            // Subagent progress
            subagent_name: raw.subagent_name,
            subagent_role: raw.subagent_role,
            subagent_status: raw.subagent_status,
            // Governance / brain / cortex messages
            source: raw.source,
            severity: raw.severity,
            message: raw.message,
            questions: raw.questions,
            slides: raw.slides,
        },
    })
}

#[derive(Deserialize)]
struct RawEvent {
    v: Option<u32>,
    agent: Option<String>,
    event: String,
    session_id: Option<String>,
    cwd: Option<String>,
    project: Option<String>,
    query: Option<String>,
    response: Option<String>,
    transcript_path: Option<String>,
    summary: Option<String>,
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
    plugin_version: Option<String>,
    // Rich tool execution data
    tool_result: Option<String>,
    tool_error: Option<String>,
    tool_duration_ms: Option<u64>,
    // LLM metrics
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_usd: Option<f64>,
    model: Option<String>,
    // Subagent progress; `status` is an accepted alias for `subagent_status`
    subagent_name: Option<String>,
    subagent_role: Option<String>,
    #[serde(alias = "status")]
    subagent_status: Option<String>,
    // Governance / brain / cortex messages
    source: Option<String>,
    severity: Option<String>,
    message: Option<String>,
    // Interview/Decision request data
    questions: Option<String>,
    slides: Option<String>,
}
