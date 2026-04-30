//! [`Agent`] implementation backed by the `codex` CLI (OpenAI Codex).
//!
//! The agent shells out to a locally installed `codex` binary
//! (`npm i -g @openai/codex`) running in non-interactive `exec --json` mode
//! and translates its JSONL event stream into [`AgentEvent`]s.
//!
//! No direct OpenAI SDK calls happen here — `codex` is the LLM driver and we
//! are merely a process supervisor. Configuration of the underlying model,
//! reasoning effort and service tier is forwarded via `-c key=value`
//! overrides into codex's TOML config layer:
//!
//! - `model_reasoning_effort` — `"minimal" | "low" | "medium" | "high"`
//! - `model_service_tier`     — `"priority" | "default"` (Fast vs Standard)
//!
//! The `codex` CLI does not (as of 0.128.x) expose dedicated `--reasoning` or
//! `--service-tier` flags; both knobs are passed through `-c …` overrides.
//! See `codex --help` and `~/.codex/config.toml` for the full surface.

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

/// Service tier requested from the upstream provider.
///
/// Codex forwards this to OpenAI's responses API as `service_tier`. `Fast`
/// maps to the priority queue (lower latency, higher cost); `Standard` is the
/// default queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceTier {
    /// Priority queue — lower latency at higher cost.
    Fast,
    /// Default queue — standard latency and cost.
    Standard,
}

impl ServiceTier {
    /// String passed to codex as the `model_service_tier` config override.
    pub fn cli_value(self) -> &'static str {
        match self {
            ServiceTier::Fast => "priority",
            ServiceTier::Standard => "default",
        }
    }
}

/// Reasoning effort knob applied to GPT-5.5-class reasoning models.
///
/// Maps to codex's `model_reasoning_effort` config key. The CLI accepts
/// `"minimal" | "low" | "medium" | "high"`; we expose `None` as the public
/// equivalent of `"minimal"` (the lowest setting the API supports).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    /// Maximum reasoning depth — slowest, highest cost.
    High,
    /// Balanced reasoning depth.
    Medium,
    /// Light reasoning — fast, cheap.
    Low,
    /// Effectively disable reasoning (mapped to codex's `"minimal"`).
    None,
}

impl ReasoningEffort {
    /// String passed to codex as the `model_reasoning_effort` config override.
    pub fn cli_value(self) -> &'static str {
        match self {
            ReasoningEffort::High => "high",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::Low => "low",
            ReasoningEffort::None => "minimal",
        }
    }
}

/// An [`Agent`] that drives a local `codex` CLI binary.
pub struct CodexAgent {
    id: AgentId,
    binary: PathBuf,
    service_tier: ServiceTier,
    reasoning_effort: ReasoningEffort,
    capabilities: Capabilities,
    health: Arc<Mutex<Health>>,
}

impl CodexAgent {
    fn locate_binary() -> Result<PathBuf, AgentError> {
        which::which("codex").map_err(|_| {
            AgentError::Other(
                "codex CLI not on PATH; install via npm i -g @openai/codex".to_string(),
            )
        })
    }

    fn build(
        id: AgentId,
        service_tier: ServiceTier,
        reasoning_effort: ReasoningEffort,
        capabilities: Capabilities,
    ) -> Result<Self, AgentError> {
        let binary = Self::locate_binary()?;
        let health = Arc::new(Mutex::new(Health {
            healthy: true,
            last_check: Utc::now(),
            error_rate: 0.0,
        }));
        Ok(Self {
            id,
            binary,
            service_tier,
            reasoning_effort,
            capabilities,
            health,
        })
    }

    /// Profile suited to planning and review work: `Fast` tier with `High`
    /// reasoning effort. Advertises [`Role::Planner`] and [`Role::Reviewer`].
    pub fn planner(id: AgentId) -> Result<Self, AgentError> {
        use std::collections::HashSet;
        let roles: HashSet<Role> = [Role::Planner, Role::Reviewer].into_iter().collect();
        let capabilities = Capabilities {
            roles,
            max_context_tokens: 1_000_000,
            supports_tools: true,
            supports_vision: false,
        };
        Self::build(id, ServiceTier::Fast, ReasoningEffort::High, capabilities)
    }

    /// Profile suited to general workers: `Standard` tier with `Medium`
    /// reasoning effort. Advertises [`Role::Worker`].
    pub fn worker(id: AgentId) -> Result<Self, AgentError> {
        use std::collections::HashSet;
        let roles: HashSet<Role> = [Role::Worker].into_iter().collect();
        let capabilities = Capabilities {
            roles,
            max_context_tokens: 1_000_000,
            supports_tools: true,
            supports_vision: false,
        };
        Self::build(
            id,
            ServiceTier::Standard,
            ReasoningEffort::Medium,
            capabilities,
        )
    }

    /// Profile suited to large mechanical refactors: `Standard` tier with
    /// `Low` reasoning effort. Advertises [`Role::BulkRefactor`].
    pub fn bulk_refactor(id: AgentId) -> Result<Self, AgentError> {
        use std::collections::HashSet;
        let roles: HashSet<Role> = [Role::BulkRefactor].into_iter().collect();
        let capabilities = Capabilities {
            roles,
            max_context_tokens: 1_000_000,
            supports_tools: true,
            supports_vision: false,
        };
        Self::build(
            id,
            ServiceTier::Standard,
            ReasoningEffort::Low,
            capabilities,
        )
    }

    /// Profile suited to summarization and inline completions: `Standard`
    /// tier with reasoning disabled (`None` → codex `"minimal"`). Advertises
    /// [`Role::Summarize`] and [`Role::Inline`]. Reports
    /// `supports_tools=false` because codex's reasoning-minimal mode rejects
    /// the built-in tool surface.
    pub fn summarizer(id: AgentId) -> Result<Self, AgentError> {
        use std::collections::HashSet;
        let roles: HashSet<Role> = [Role::Summarize, Role::Inline].into_iter().collect();
        let capabilities = Capabilities {
            roles,
            max_context_tokens: 1_000_000,
            supports_tools: false,
            supports_vision: false,
        };
        Self::build(
            id,
            ServiceTier::Standard,
            ReasoningEffort::None,
            capabilities,
        )
    }

    /// Caller-specified tier and effort. Advertises every [`Role`] variant so
    /// the router treats this as a fully general-purpose backend; emits a
    /// `tracing::warn!` if the (tier, effort) pair is not one of the four
    /// canonical profiles, since unusual combinations have not been
    /// benchmarked and may interact poorly with codex's tool surface.
    pub fn custom(
        id: AgentId,
        service_tier: ServiceTier,
        reasoning_effort: ReasoningEffort,
    ) -> Result<Self, AgentError> {
        use std::collections::HashSet;
        let roles: HashSet<Role> = [
            Role::Planner,
            Role::Reviewer,
            Role::Worker,
            Role::BulkRefactor,
            Role::Summarize,
            Role::ToolRouter,
            Role::Inline,
        ]
        .into_iter()
        .collect();
        let capabilities = Capabilities {
            roles,
            max_context_tokens: 1_000_000,
            supports_tools: !matches!(reasoning_effort, ReasoningEffort::None),
            supports_vision: false,
        };

        let is_canonical = matches!(
            (service_tier, reasoning_effort),
            (ServiceTier::Fast, ReasoningEffort::High)
                | (ServiceTier::Standard, ReasoningEffort::Medium)
                | (ServiceTier::Standard, ReasoningEffort::Low)
                | (ServiceTier::Standard, ReasoningEffort::None)
        );
        if !is_canonical {
            tracing::warn!(
                ?service_tier,
                ?reasoning_effort,
                "CodexAgent::custom: (tier, effort) combination is not in the standard profile map"
            );
        }

        Self::build(id, service_tier, reasoning_effort, capabilities)
    }

    /// Path to the resolved `codex` binary. Exposed for diagnostics / tests.
    pub fn binary_path(&self) -> &PathBuf {
        &self.binary
    }

    /// Service tier this agent is configured to request.
    pub fn service_tier(&self) -> ServiceTier {
        self.service_tier
    }

    /// Reasoning effort this agent is configured to request.
    pub fn reasoning_effort(&self) -> ReasoningEffort {
        self.reasoning_effort
    }
}

/// Convert one JSONL event line emitted by `codex exec --json` into zero or
/// more [`AgentEvent`]s for downstream consumers.
///
/// `codex` emits a sequence of JSON objects, one per line, each tagged with a
/// `"type"` discriminator. Observed types include `thread.started`,
/// `turn.started`, `item.completed` (with `item.type` of `agent_message`,
/// `command_execution`, etc.), `turn.completed`, `turn.failed`, and `error`.
/// Anything we don't recognise is dropped (with a `tracing::trace!`) rather
/// than surfaced as a bogus event. Terminal events are emitted by
/// [`CodexAgent::execute`] based on the child's exit status, not from these
/// in-stream events, so a failing `turn.failed` does not short-circuit the
/// stream.
fn translate_stream_event(line: &str) -> Vec<AgentEvent> {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(err) => {
            tracing::trace!(?err, line, "non-JSON line from codex stdout; ignoring");
            return Vec::new();
        }
    };

    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match kind {
        "item.completed" | "item.started" => {
            let item = match value.get("item") {
                Some(i) => i,
                None => return Vec::new(),
            };
            let item_kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
            match item_kind {
                "agent_message" => item
                    .get("text")
                    .and_then(Value::as_str)
                    .map(|t| {
                        vec![AgentEvent::OutputChunk {
                            text: t.to_string(),
                        }]
                    })
                    .unwrap_or_default(),
                "command_execution" if kind == "item.started" => {
                    // Codex about to run a shell command — surface as a
                    // ToolCall so downstream consumers can audit it.
                    let cmd = item.get("command").cloned().unwrap_or(Value::Null);
                    vec![AgentEvent::ToolCall {
                        name: "shell".to_string(),
                        args: cmd,
                    }]
                }
                "command_execution" if kind == "item.completed" => {
                    let result = item
                        .get("output")
                        .or_else(|| item.get("result"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    vec![AgentEvent::ToolResult {
                        name: "shell".to_string(),
                        result,
                    }]
                }
                _ => Vec::new(),
            }
        }
        // Plain text deltas (some codex builds emit incremental text).
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
impl Agent for CodexAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn health(&self) -> Health {
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
        cmd.arg("exec")
            .arg("--json")
            .arg("--skip-git-repo-check")
            .arg("-c")
            .arg(format!(
                "model_reasoning_effort=\"{}\"",
                self.reasoning_effort.cli_value()
            ))
            .arg("-c")
            .arg(format!(
                "model_service_tier=\"{}\"",
                self.service_tier.cli_value()
            ))
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
            .ok_or_else(|| AgentError::Other("codex child missing stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::Other("codex child missing stderr".to_string()))?;

        let health = Arc::clone(&self.health);

        let stream = stream! {
            yield AgentEvent::Started { task_id };

            // Drain stderr concurrently so a full pipe doesn't deadlock the
            // child while we're iterating stdout.
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
                        tracing::warn!(?err, "codex stdout read error");
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
                        format!("codex exited with status {status}")
                    } else {
                        format!("codex exited with status {status}: {}", stderr_text.trim())
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
                        error: format!("waiting on codex failed: {err}"),
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

    #[test]
    fn service_tier_cli_values() {
        assert_eq!(ServiceTier::Fast.cli_value(), "priority");
        assert_eq!(ServiceTier::Standard.cli_value(), "default");
    }

    #[test]
    fn reasoning_effort_cli_values() {
        assert_eq!(ReasoningEffort::High.cli_value(), "high");
        assert_eq!(ReasoningEffort::Medium.cli_value(), "medium");
        assert_eq!(ReasoningEffort::Low.cli_value(), "low");
        assert_eq!(ReasoningEffort::None.cli_value(), "minimal");
    }

    #[test]
    fn translate_agent_message_chunk() {
        let line = serde_json::json!({
            "type": "item.completed",
            "item": {"id": "item_0", "type": "agent_message", "text": "pong"}
        })
        .to_string();
        let events = translate_stream_event(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::OutputChunk { text } => assert_eq!(text, "pong"),
            other => panic!("expected OutputChunk, got {other:?}"),
        }
    }

    #[test]
    fn translate_command_execution_started_emits_tool_call() {
        let line = serde_json::json!({
            "type": "item.started",
            "item": {
                "id": "item_1",
                "type": "command_execution",
                "command": ["ls", "-la"],
            }
        })
        .to_string();
        let events = translate_stream_event(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolCall { name, args } => {
                assert_eq!(name, "shell");
                assert_eq!(args.as_array().map(Vec::len), Some(2));
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
    }

    #[test]
    fn translate_unknown_lines_yield_no_events() {
        assert!(translate_stream_event("not json").is_empty());
        assert!(translate_stream_event(r#"{"type":"thread.started"}"#).is_empty());
        assert!(translate_stream_event(r#"{"type":"turn.completed"}"#).is_empty());
        assert!(translate_stream_event(r#"{"type":"weird"}"#).is_empty());
    }
}
