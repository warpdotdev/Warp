//! [`Agent`] implementation backed by the `claude` CLI (Claude Code).
//!
//! The agent shells out to a locally installed `claude` binary
//! (`npm i -g @anthropic-ai/claude-code`) running in non-interactive
//! `--print` mode with `--output-format stream-json`, then translates the
//! line-delimited JSON event stream into [`AgentEvent`]s.
//!
//! No direct Anthropic SDK calls happen here — `claude` is the LLM driver,
//! we are merely a process supervisor.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use chrono::Utc;
use orchestrator::{
    Agent, AgentError, AgentEvent, AgentEventStream, AgentId, Capabilities, Health, Role, Task,
};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

/// Concrete Claude Code model variants this agent can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeModel {
    /// Opus 4.7 — the heavy reasoning model.
    Opus47,
    /// Sonnet 4.6 — balanced workhorse.
    Sonnet46,
    /// Haiku 4.5 — cheap/fast for short summaries and inline completions.
    Haiku45,
}

impl ClaudeModel {
    /// String passed to `claude --model …` to select this variant.
    pub fn cli_arg(self) -> &'static str {
        match self {
            ClaudeModel::Opus47 => "claude-opus-4-7",
            ClaudeModel::Sonnet46 => "claude-sonnet-4-6",
            ClaudeModel::Haiku45 => "claude-haiku-4-5",
        }
    }

    fn capabilities(self) -> Capabilities {
        use std::collections::HashSet;
        let (roles, supports_vision): (HashSet<Role>, bool) = match self {
            ClaudeModel::Opus47 => (
                [
                    Role::Planner,
                    Role::Reviewer,
                    Role::Worker,
                    Role::BulkRefactor,
                ]
                .into_iter()
                .collect(),
                true,
            ),
            ClaudeModel::Sonnet46 => (
                [Role::Worker, Role::BulkRefactor, Role::Reviewer]
                    .into_iter()
                    .collect(),
                true,
            ),
            ClaudeModel::Haiku45 => (
                [Role::Summarize, Role::Inline, Role::Worker]
                    .into_iter()
                    .collect(),
                false,
            ),
        };
        Capabilities {
            roles,
            max_context_tokens: 200_000,
            supports_tools: true,
            supports_vision,
        }
    }
}

/// An [`Agent`] that drives a local `claude` CLI binary.
pub struct ClaudeCodeAgent {
    id: AgentId,
    binary: PathBuf,
    model: ClaudeModel,
    capabilities: Capabilities,
    health: Arc<Mutex<Health>>,
}

impl ClaudeCodeAgent {
    /// Construct a new [`ClaudeCodeAgent`].
    ///
    /// Resolves the `claude` binary on `PATH` via [`which::which`]. If it is
    /// not found, returns [`AgentError::Other`] with an install hint pointing
    /// at the npm package.
    pub fn new(id: AgentId, model: ClaudeModel) -> Result<Self, AgentError> {
        let binary = which::which("claude").map_err(|_| {
            AgentError::Other(
                "claude CLI not on PATH; install via npm i -g @anthropic-ai/claude-code"
                    .to_string(),
            )
        })?;
        let capabilities = model.capabilities();
        let health = Arc::new(Mutex::new(Health {
            healthy: true,
            last_check: Utc::now(),
            error_rate: 0.0,
        }));
        Ok(Self {
            id,
            binary,
            model,
            capabilities,
            health,
        })
    }

    /// Path to the resolved `claude` binary. Exposed for diagnostics / tests.
    pub fn binary_path(&self) -> &PathBuf {
        &self.binary
    }

    /// Model variant this agent is configured to drive.
    pub fn model(&self) -> ClaudeModel {
        self.model
    }
}

/// Convert one stream-json event line emitted by `claude` into zero or more
/// [`AgentEvent`]s for downstream consumers.
///
/// `claude --output-format stream-json` emits a sequence of JSON objects, one
/// per line, with a `"type"` discriminator. The exact schema is internal to
/// Claude Code, so we deliberately keep this mapping conservative: anything
/// we don't recognise is dropped (with a `tracing::trace!`) rather than
/// surfaced as a bogus event.
fn translate_stream_event(line: &str) -> Vec<AgentEvent> {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(err) => {
            tracing::trace!(?err, line, "non-JSON line from claude stdout; ignoring");
            return Vec::new();
        }
    };

    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match kind {
        // Top-level assistant turn carrying a content array.
        "assistant" => {
            let content = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Value::as_array);
            let mut events = Vec::new();
            if let Some(items) = content {
                for item in items {
                    let item_kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
                    match item_kind {
                        "text" => {
                            if let Some(text) = item.get("text").and_then(Value::as_str) {
                                events.push(AgentEvent::OutputChunk {
                                    text: text.to_string(),
                                });
                            }
                        }
                        "tool_use" => {
                            let name = item
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let args = item.get("input").cloned().unwrap_or(Value::Null);
                            events.push(AgentEvent::ToolCall { name, args });
                        }
                        _ => {}
                    }
                }
            }
            events
        }
        "user" => {
            // The CLI echoes tool results back via a synthetic user turn.
            let content = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Value::as_array);
            let mut events = Vec::new();
            if let Some(items) = content {
                for item in items {
                    if item.get("type").and_then(Value::as_str) == Some("tool_result") {
                        let name = item
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let result = item.get("content").cloned().unwrap_or(Value::Null);
                        events.push(AgentEvent::ToolResult { name, result });
                    }
                }
            }
            events
        }
        // Plain text deltas (older / partial-message format).
        "text" | "output" => value
            .get("text")
            .and_then(Value::as_str)
            .map(|t| {
                vec![AgentEvent::OutputChunk {
                    text: t.to_string(),
                }]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[async_trait]
impl Agent for ClaudeCodeAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn health(&self) -> Health {
        // `try_lock` lets us stay synchronous; if another task is currently
        // mutating health we fall back to a fresh "healthy" snapshot rather
        // than blocking the dispatcher.
        if let Ok(guard) = self.health.try_lock() {
            guard.clone()
        } else {
            Health {
                healthy: true,
                last_check: Utc::now(),
                error_rate: 0.0,
            }
        }
    }

    async fn execute(&self, task: Task) -> Result<AgentEventStream, AgentError> {
        let task_id = task.id;
        let mut cmd = Command::new(&self.binary);
        cmd.arg("--print")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--model")
            .arg(self.model.cli_arg())
            .arg(&task.prompt);

        if !task.context.cwd.as_os_str().is_empty() {
            cmd.current_dir(&task.context.cwd);
        }
        for (k, v) in &task.context.env {
            cmd.env(k, v);
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|err| {
            AgentError::Other(format!(
                "failed to spawn `{}`: {err}",
                self.binary.display()
            ))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Other("claude child missing stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::Other("claude child missing stderr".to_string()))?;

        let health = Arc::clone(&self.health);

        let stream = stream! {
            yield AgentEvent::Started { task_id };

            // Drain stderr concurrently into a buffer so it's available if we
            // need to surface a failure summary, without deadlocking on a
            // full pipe.
            let stderr_handle = tokio::spawn(async move {
                let mut buf = String::new();
                let mut reader = BufReader::new(stderr);
                let _ = reader.read_to_string(&mut buf).await;
                buf
            });

            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        for event in translate_stream_event(&line) {
                            yield event;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        tracing::warn!(?err, "claude stdout read error");
                        break;
                    }
                }
            }

            let stderr_text = stderr_handle.await.unwrap_or_default();
            match child.wait().await {
                Ok(status) if status.success() => {
                    yield AgentEvent::Completed { task_id, summary: None };
                }
                Ok(status) => {
                    let mut h = health.lock().await;
                    h.healthy = false;
                    h.last_check = Utc::now();
                    h.error_rate = (h.error_rate + 1.0).min(1.0);
                    drop(h);
                    let error = if stderr_text.trim().is_empty() {
                        format!("claude exited with status {status}")
                    } else {
                        format!("claude exited with status {status}: {}", stderr_text.trim())
                    };
                    yield AgentEvent::Failed { task_id, error };
                }
                Err(err) => {
                    let mut h = health.lock().await;
                    h.healthy = false;
                    h.last_check = Utc::now();
                    h.error_rate = (h.error_rate + 1.0).min(1.0);
                    drop(h);
                    yield AgentEvent::Failed {
                        task_id,
                        error: format!("waiting on claude failed: {err}"),
                    };
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator::Role;

    #[test]
    fn cli_arg_strings_match_model_release_names() {
        assert_eq!(ClaudeModel::Opus47.cli_arg(), "claude-opus-4-7");
        assert_eq!(ClaudeModel::Sonnet46.cli_arg(), "claude-sonnet-4-6");
        assert_eq!(ClaudeModel::Haiku45.cli_arg(), "claude-haiku-4-5");
    }

    #[test]
    fn opus_advertises_planner_and_reviewer_roles() {
        let caps = ClaudeModel::Opus47.capabilities();
        assert!(caps.roles.contains(&Role::Planner));
        assert!(caps.roles.contains(&Role::Reviewer));
        assert!(caps.roles.contains(&Role::Worker));
        assert!(caps.roles.contains(&Role::BulkRefactor));
        assert!(caps.supports_tools);
        assert!(caps.supports_vision);
    }

    #[test]
    fn haiku_does_not_advertise_vision() {
        let caps = ClaudeModel::Haiku45.capabilities();
        assert!(!caps.supports_vision);
        assert!(caps.roles.contains(&Role::Summarize));
        assert!(caps.roles.contains(&Role::Inline));
    }

    #[test]
    fn translate_assistant_text_chunk() {
        let line = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "hello world"}
                ]
            }
        })
        .to_string();
        let events = translate_stream_event(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::OutputChunk { text } => assert_eq!(text, "hello world"),
            other => panic!("expected OutputChunk, got {other:?}"),
        }
    }

    #[test]
    fn translate_tool_use_call() {
        let line = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "tool_use", "name": "Read", "input": {"path": "/tmp/x"}}
                ]
            }
        })
        .to_string();
        let events = translate_stream_event(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolCall { name, args } => {
                assert_eq!(name, "Read");
                assert_eq!(args.get("path").and_then(Value::as_str), Some("/tmp/x"));
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
    }

    #[test]
    fn translate_unknown_line_yields_no_events() {
        assert!(translate_stream_event("not json").is_empty());
        assert!(translate_stream_event(r#"{"type":"weird"}"#).is_empty());
    }
}
