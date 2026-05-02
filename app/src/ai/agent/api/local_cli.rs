//! Local-CLI bypass for the interactive agent panel.
//!
//! When `WARP_BYPASS_AUTH=1` is set (with or without `WARP_LOCAL_AI`), the
//! panel ordinarily fails with a 401 because `generate_multi_agent_output`
//! routes to Warp's authenticated GraphQL server.  This module short-circuits
//! that path: instead of hitting the server, it shells out to the local CLI
//! indicated by the user's model-selector choice, and synthesises a
//! `ResponseEvent` stream that Warp's controller can consume normally.
//!
//! # Model routing
//!
//! The model is taken from `params.model`.  Local model IDs are structured
//! strings like `"local:claude:claude-sonnet-4-7"` (see `local_ai` module).
//! If the ID is not a local model ID, the legacy `WARP_LOCAL_AI` env var is
//! used as a fallback so existing configurations keep working.
//!
//! # Tool-call visibility (Slice B)
//!
//! Claude is invoked with `--output-format stream-json --verbose`, which emits
//! one JSON object per line.  This module parses those events and renders them
//! as inline text annotations in the panel:
//!
//! - `assistant` content blocks of type `text` are forwarded verbatim.
//! - `assistant` content blocks of type `tool_use` produce a `[tool: Name]`
//!   line so the user sees which tool the agent is calling.
//! - `user` events carrying `tool_result` produce a `[result]` line with a
//!   short stdout/stderr excerpt so the user sees what the tool returned.
//! - `result`/`system` events are silently dropped.
//!
//! This is Slice B of the full tool-loop bridge.  Warp native tool-block UI
//! (Slice C) is deferred - that requires mapping each CLI tool event to a
//! proper `api::message::ToolCall` protobuf message.
//!
//! Codex `--json` events are unchanged from the previous implementation: only
//! `item.completed / agent_message` lines are surfaced as text.

use std::process::Stdio;

use async_channel::{Sender, unbounded};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;
use warp_multi_agent_api as api;

use crate::ai::agent::api::RequestParams;
use crate::ai::agent::{AIAgentContext, AIAgentInput};
use crate::local_ai::{LocalAiMode, LocalModelSpec};

// Re-use the type aliases from the parent api module.
type Event = crate::ai::agent::api::Event;
type ResponseStream = crate::ai::agent::api::ResponseStream;

/// Which output format the CLI subprocess uses.
enum OutputMode {
    /// Raw text: each stdout line is displayed verbatim.
    ///
    /// Kept for future harnesses (e.g. Ollama) that don't emit structured JSON.
    #[allow(dead_code)]
    RawText,
    /// Claude `--output-format stream-json --verbose` events.
    ///
    /// Each line is a JSON object with a `type` discriminator.  Text content
    /// from `assistant` events is forwarded; `tool_use` and `tool_result`
    /// events are rendered as inline annotations.
    ClaudeStreamJson,
    /// Codex `--json` events: each stdout line is a JSON object; only
    /// `{"type":"item.completed","item":{"type":"agent_message","text":"..."}}` is shown.
    CodexJson,
}

/// Build and return a local-CLI `ResponseStream` for the given `params`.
///
/// On success the stream emits:
///   1. `StreamInit`
///   2. One or more `ClientActions(AddMessagesToTask / AppendToMessageContent)`
///      carrying the CLI stdout as agent-output text
///   3. `StreamFinished(Done)` or `StreamFinished(InternalError)` on failure
///
/// The `cancellation_rx` channel is monitored: when it fires the stream ends
/// early with a `Done` finish event (consistent with user-cancel behaviour in
/// the non-bypass path).
pub(super) async fn generate_local_output(
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> ResponseStream {
    let (tx, rx) = unbounded::<Event>();
    tokio::spawn(run_local_cli(params, tx, cancellation_rx));
    Box::pin(rx)
}

async fn run_local_cli(
    params: RequestParams,
    tx: Sender<Event>,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) {
    let conversation_id = params
        .conversation_token
        .as_ref()
        .map(|t| t.as_str().to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let request_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();

    // Emit StreamInit.
    let _ = tx
        .send(Ok(api::ResponseEvent {
            r#type: Some(api::response_event::Type::Init(
                api::response_event::StreamInit {
                    conversation_id: conversation_id.clone(),
                    request_id: request_id.clone(),
                    run_id: run_id.clone(),
                },
            )),
        }))
        .await;

    // Extract user query and working directory from params.
    let query = extract_query(&params.input);
    let cwd = extract_cwd(&params.input);

    // Resolve task id.
    let task_id = params
        .tasks
        .first()
        .and_then(|t| {
            let id = t.id.trim();
            if id.is_empty() {
                None
            } else {
                Some(id.to_string())
            }
        })
        .unwrap_or_else(|| format!("local-task-{}", Uuid::new_v4()));

    let message_id = format!("local-msg-{}", Uuid::new_v4());

    // Determine which local CLI to invoke.
    //
    // Priority:
    //   1. If the selected model is a local-model ID (e.g. "local:claude:…"),
    //      parse it and use its provider + parameters.
    //   2. Fall back to the legacy WARP_LOCAL_AI env var for backward compat.
    let model_id_str = params.model.to_string();
    let maybe_spec = LocalModelSpec::parse(&model_id_str);

    let (output_mode, cli_label, mut cmd) = match maybe_spec {
        Some(ref spec) => match spec {
            LocalModelSpec::Claude { .. } => (
                OutputMode::ClaudeStreamJson,
                "claude",
                build_command_from_spec(spec, &query, cwd.as_deref()),
            ),
            LocalModelSpec::Codex { .. } => (
                OutputMode::CodexJson,
                "codex",
                build_command_from_spec(spec, &query, cwd.as_deref()),
            ),
        },
        None => {
            // Legacy WARP_LOCAL_AI env var path.
            match crate::local_ai::current() {
                LocalAiMode::Claude => (
                    OutputMode::ClaudeStreamJson,
                    "claude",
                    build_legacy_command("claude", &query, cwd.as_deref()),
                ),
                LocalAiMode::Codex => (
                    OutputMode::CodexJson,
                    "codex",
                    build_legacy_command("codex", &query, cwd.as_deref()),
                ),
                _ => {
                    let _ = send_internal_error(
                        &tx,
                        "No local model selected; set WARP_LOCAL_AI or select a model from the menu",
                    )
                    .await;
                    return;
                }
            }
        }
    };

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to launch `{cli_label}`: {e}");
            log::error!("{msg}");
            let _ = send_internal_error(&tx, msg).await;
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = send_internal_error(&tx, "CLI subprocess has no stdout").await;
            return;
        }
    };

    let mut lines = BufReader::new(stdout).lines();
    let mut first_chunk = true;
    let mut cancellation_rx = cancellation_rx;

    loop {
        tokio::select! {
            // Cancellation: user pressed stop.
            _ = &mut cancellation_rx => {
                let _ = child.kill().await;
                let _ = send_done(&tx).await;
                return;
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(raw_line)) => {
                        // Extract text to display (may be empty for non-text lines).
                        let text = match output_mode {
                            OutputMode::RawText => {
                                if raw_line.is_empty() {
                                    // Preserve blank lines.
                                    "\n".to_string()
                                } else {
                                    format!("{raw_line}\n")
                                }
                            }
                            OutputMode::ClaudeStreamJson => {
                                match extract_claude_stream_json_text(&raw_line) {
                                    Some(t) => t,
                                    None => continue,
                                }
                            }
                            OutputMode::CodexJson => {
                                match extract_codex_json_text(&raw_line) {
                                    Some(t) => t,
                                    None => continue,
                                }
                            }
                        };

                        let event = if first_chunk {
                            first_chunk = false;
                            add_agent_output_event(&request_id, &task_id, &message_id, &text)
                        } else {
                            append_agent_output_event(&request_id, &task_id, &message_id, &text)
                        };
                        if tx.send(Ok(event)).await.is_err() {
                            // Receiver dropped; clean up.
                            let _ = child.kill().await;
                            return;
                        }
                    }
                    Ok(None) => {
                        // EOF: stream finished.
                        break;
                    }
                    Err(e) => {
                        log::warn!("Error reading CLI stdout: {e}");
                        break;
                    }
                }
            }
        }
    }

    // Wait for exit and check status.
    match child.wait().await {
        Ok(status) if status.success() => {
            let _ = send_done(&tx).await;
        }
        Ok(status) => {
            log::warn!("CLI exited with non-zero status: {status}");
            // Finish with Done so the panel renders whatever text was already emitted.
            let _ = send_done(&tx).await;
        }
        Err(e) => {
            let _ = send_internal_error(&tx, format!("CLI wait failed: {e}")).await;
        }
    }
}

/// Build a `tokio::process::Command` from a parsed [`LocalModelSpec`].
///
/// - `Claude`: `claude -p --model <name> --output-format stream-json --verbose --dangerously-skip-permissions <query>`
/// - `Codex`: `codex exec -m <name> [-c reasoning_effort=<effort>] --dangerously-bypass-approvals-and-sandbox --json --color never <query>`
fn build_command_from_spec(spec: &LocalModelSpec, query: &str, cwd: Option<&str>) -> Command {
    let mut cmd = match spec {
        LocalModelSpec::Claude { model_name } => {
            let mut c = Command::new("claude");
            c.args([
                "-p",
                "--model",
                model_name,
                "--output-format",
                "stream-json",
                "--verbose",
                "--dangerously-skip-permissions",
            ]);
            c.arg(query);
            c
        }
        LocalModelSpec::Codex {
            model_name,
            reasoning_effort,
        } => {
            let mut c = Command::new("codex");
            c.args(["exec", "-m", model_name]);
            if let Some(effort) = reasoning_effort {
                c.args(["-c", &format!("reasoning_effort={effort}")]);
            }
            c.args([
                "--dangerously-bypass-approvals-and-sandbox",
                "--json",
                "--color",
                "never",
            ]);
            c.arg(query);
            c
        }
    };
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd
}

/// Build a `tokio::process::Command` using the legacy provider name string
/// (no model flag, uses the CLI's own default model).
fn build_legacy_command(cli: &str, query: &str, cwd: Option<&str>) -> Command {
    let mut cmd = Command::new(cli);

    match cli {
        "claude" => {
            // -p / --print: non-interactive; stream-json for tool-call visibility.
            cmd.args([
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--dangerously-skip-permissions",
            ]);
            cmd.arg(query);
        }
        "codex" => {
            // exec subcommand: non-interactive run; --json: structured JSON events on stdout.
            cmd.args([
                "exec",
                "--dangerously-bypass-approvals-and-sandbox",
                "--json",
                "--color",
                "never",
            ]);
            cmd.arg(query);
        }
        _ => {
            cmd.arg(query);
        }
    }

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd
}

/// Parse one line from `claude -p --output-format stream-json --verbose` and
/// return the text that should appear in the panel, or `None` to skip the line.
///
/// Handled event types:
/// - `assistant` with `content[].type == "text"` -> forward the text verbatim.
/// - `assistant` with `content[].type == "tool_use"` -> emit `[tool: Name] summary`.
/// - `user` with `content[].type == "tool_result"` (tool finished) -> emit `[result] excerpt`.
/// - Everything else (`system`, `result`, `rate_limit_event`, …) -> `None`.
fn extract_claude_stream_json_text(line: &str) -> Option<String> {
    // Fast-path: skip lines that can't carry user-visible content.
    if line.is_empty() {
        return None;
    }

    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let event_type = v.get("type")?.as_str()?;

    match event_type {
        "assistant" => {
            let message = v.get("message")?;
            let content = message.get("content")?.as_array()?;
            let mut out = String::new();
            for block in content {
                match block.get("type")?.as_str()? {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            if !text.is_empty() {
                                out.push_str(text);
                                // Ensure the text ends with a newline so subsequent
                                // chunks stay visually separated.
                                if !out.ends_with('\n') {
                                    out.push('\n');
                                }
                            }
                        }
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown");
                        // Build a short human-readable summary of the input.
                        let summary = tool_use_summary(name, block.get("input"));
                        out.push_str(&format!("[tool: {name}] {summary}\n"));
                    }
                    _ => {}
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        "user" => {
            // Tool results come back as `user` events with tool_result content blocks.
            let message = v.get("message")?;
            let content = message.get("content")?.as_array()?;
            let mut out = String::new();
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                    // Prefer the top-level `tool_use_result` field (richer).
                    let excerpt = if let Some(tur) = v.get("tool_use_result") {
                        tool_result_excerpt(tur)
                    } else {
                        // Fall back to content string payload.
                        block
                            .get("content")
                            .and_then(|c| c.as_str())
                            .map(|s| truncate(s, 120).to_string())
                            .unwrap_or_default()
                    };
                    if !excerpt.is_empty() {
                        out.push_str(&format!("[result] {excerpt}\n"));
                    }
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        // `system`, `result`, `rate_limit_event`, and any future event types
        // are intentionally dropped - they carry no user-visible content for
        // the panel.
        _ => None,
    }
}

/// Build a short summary string for a `tool_use` content block.
///
/// For well-known tool names (Bash, Read, Write, Edit, Grep, Glob) we surface
/// the most relevant field.  For everything else we fall back to a compact JSON
/// representation of the input (truncated to 120 chars).
fn tool_use_summary(name: &str, input: Option<&serde_json::Value>) -> String {
    let Some(inp) = input else {
        return String::new();
    };

    // Helper: get a string field from the input object.
    let field = |key: &str| -> Option<&str> { inp.get(key)?.as_str() };

    match name {
        "Bash" => field("command")
            .map(|s| truncate(s, 120).to_string())
            .unwrap_or_default(),
        "Read" => field("file_path")
            .or_else(|| field("path"))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        "Write" | "Edit" => field("file_path")
            .or_else(|| field("path"))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        "Grep" => {
            let pattern = field("pattern").unwrap_or("");
            let path = field("path").unwrap_or("");
            if path.is_empty() {
                pattern.to_string()
            } else {
                format!("{pattern} in {path}")
            }
        }
        "Glob" => field("pattern").unwrap_or("").to_string(),
        _ => {
            // Generic fallback: compact JSON, truncated.
            let compact = inp.to_string();
            truncate(&compact, 120).to_string()
        }
    }
}

/// Extract a short excerpt from a `tool_use_result` JSON value.
fn tool_result_excerpt(tur: &serde_json::Value) -> String {
    // `stdout` is the primary output; fall back to `output` for older schemas.
    let raw = tur
        .get("stdout")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            tur.get("output")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("");
    truncate(raw, 120).to_string()
}

/// Return a string slice of at most `max_chars` UTF-8 characters.
fn truncate(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }
    // Find the byte offset of the `max_chars`-th character boundary.
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// For codex `--json` output, extract the agent reply text from an
/// `item.completed` / `agent_message` line.  Returns `None` for all other
/// event types.
fn extract_codex_json_text(line: &str) -> Option<String> {
    // Fast path: skip lines that definitely aren't the right event.
    if !line.contains("item.completed") {
        return None;
    }
    // Parse just enough JSON to get item.type and item.text.
    // Example: {"type":"item.completed","item":{"id":"…","type":"agent_message","text":"Hello."}}
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "item.completed" {
        return None;
    }
    let item = v.get("item")?;
    if item.get("type")?.as_str()? != "agent_message" {
        return None;
    }
    let text = item.get("text")?.as_str()?;
    Some(text.to_string())
}

/// Extract the human-readable query text from the input list.
fn extract_query(inputs: &[AIAgentInput]) -> String {
    for input in inputs.iter().rev() {
        let text = match input {
            AIAgentInput::UserQuery { query, .. } => query.as_str(),
            AIAgentInput::AutoCodeDiffQuery { query, .. } => query.as_str(),
            AIAgentInput::CreateNewProject { query, .. } => query.as_str(),
            _ => continue,
        };
        if !text.is_empty() {
            return text.to_string();
        }
    }
    String::new()
}

/// Extract the working directory from the input list.
fn extract_cwd(inputs: &[AIAgentInput]) -> Option<String> {
    for input in inputs {
        if let Some(context) = input.context() {
            for ctx in context {
                if let AIAgentContext::Directory {
                    pwd: Some(pwd), ..
                } = ctx
                {
                    if !pwd.is_empty() {
                        return Some(pwd.clone());
                    }
                }
            }
        }
    }
    None
}

// --- ResponseEvent helpers ---

fn add_agent_output_event(
    request_id: &str,
    task_id: &str,
    message_id: &str,
    text: &str,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::AddMessagesToTask(
                        api::client_action::AddMessagesToTask {
                            task_id: task_id.to_string(),
                            messages: vec![api::Message {
                                id: message_id.to_string(),
                                task_id: task_id.to_string(),
                                request_id: request_id.to_string(),
                                message: Some(api::message::Message::AgentOutput(
                                    api::message::AgentOutput {
                                        text: text.to_string(),
                                    },
                                )),
                                ..Default::default()
                            }],
                        },
                    )),
                }],
            },
        )),
    }
}

fn append_agent_output_event(
    request_id: &str,
    task_id: &str,
    message_id: &str,
    text: &str,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::AppendToMessageContent(
                        api::client_action::AppendToMessageContent {
                            task_id: task_id.to_string(),
                            message: Some(api::Message {
                                id: message_id.to_string(),
                                task_id: task_id.to_string(),
                                request_id: request_id.to_string(),
                                message: Some(api::message::Message::AgentOutput(
                                    api::message::AgentOutput {
                                        text: text.to_string(),
                                    },
                                )),
                                ..Default::default()
                            }),
                            mask: Some(prost_types::FieldMask {
                                paths: vec!["agent_output.text".to_string()],
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

async fn send_done(tx: &Sender<Event>) -> Result<(), async_channel::SendError<Event>> {
    tx.send(Ok(api::ResponseEvent {
        r#type: Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                reason: Some(api::response_event::stream_finished::Reason::Done(
                    api::response_event::stream_finished::Done {},
                )),
                ..Default::default()
            },
        )),
    }))
    .await
}

async fn send_internal_error(
    tx: &Sender<Event>,
    msg: impl Into<String>,
) -> Result<(), async_channel::SendError<Event>> {
    tx.send(Ok(api::ResponseEvent {
        r#type: Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                reason: Some(api::response_event::stream_finished::Reason::InternalError(
                    api::response_event::stream_finished::InternalError {
                        message: msg.into(),
                    },
                )),
                ..Default::default()
            },
        )),
    }))
    .await
}
