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
//! Only the user-visible text path is implemented here.  Tool calls emitted by
//! the underlying CLI (file edits, shell commands, etc.) are not yet forwarded
//! back to the panel - that requires a richer protocol bridge and is left as
//! follow-up work.

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
    RawText,
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
                OutputMode::RawText,
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
                    OutputMode::RawText,
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
/// - `Claude`: `claude -p --model <name> --output-format text --dangerously-skip-permissions <query>`
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
                "text",
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
            // -p / --print: non-interactive; output-format text: plain text to stdout.
            cmd.args(["-p", "--output-format", "text", "--dangerously-skip-permissions"]);
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
