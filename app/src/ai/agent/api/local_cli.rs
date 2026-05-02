//! Local-CLI bypass for the interactive agent panel.
//!
//! When `WARP_LOCAL_AI=claude` or `WARP_LOCAL_AI=codex` is set, the panel
//! ordinarily fails with a 401 because `generate_multi_agent_output` routes to
//! Warp's authenticated GraphQL server.  This module short-circuits that path:
//! instead of hitting the server, it shells out to the local CLI and synthesises
//! a `ResponseEvent` stream that Warp's controller can consume normally.
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

    // Determine CLI command and output mode.
    let (cli, output_mode) = match crate::local_ai::current() {
        crate::local_ai::LocalAiMode::Claude => ("claude", OutputMode::RawText),
        crate::local_ai::LocalAiMode::Codex => ("codex", OutputMode::CodexJson),
        _ => {
            let _ =
                send_internal_error(&tx, "WARP_LOCAL_AI mode not supported in panel bypass").await;
            return;
        }
    };

    // Build the subprocess.
    let mut cmd = build_command(cli, &query, cwd.as_deref());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to launch `{cli}`: {e}");
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

/// Build the `tokio::process::Command` for the local CLI.
fn build_command(cli: &str, query: &str, cwd: Option<&str>) -> Command {
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
