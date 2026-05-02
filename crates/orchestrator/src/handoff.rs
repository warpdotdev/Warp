//! Task-state summarization at handoff.
//!
//! When the dispatcher passes a [`Task`] from one [`Agent`] to another at a
//! task boundary, the receiving agent needs a concise summary of what the
//! prior agent attempted, learned, and left pending. Producing that summary
//! is itself an LLM call — and one we explicitly want to route to the
//! cheapest available model rather than the agent the task was using.
//!
//! # Cheapest-model selection
//!
//! Selection is layered on top of [`Router`]: the summarizer constructs a
//! synthetic [`Task`] with [`Role::Summarize`] and asks the router to
//! select an agent for it. The router's existing sort key
//! `(tier, estimated_micros_per_task, agent.id())` does the work — the
//! lowest-cost healthy summarize-capable agent wins.
//!
//! In the production registry, `Provider::FoundationModels` is registered
//! at zero cost (on-device, no billing) and `Provider::ClaudeCode` at the
//! Haiku price point. With both healthy, Foundation Models wins; if the
//! local runtime is unhealthy, the router transparently falls back to
//! Haiku 4.5. This matches the master plan's
//! "Foundation Models or Haiku 4.5" requirement without requiring the
//! summarizer to know about either provider directly.
//!
//! [`Router`]: crate::router::Router
//! [`Task`]: crate::Task
//! [`Agent`]: crate::Agent

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use thiserror::Error;

use crate::{
    AgentError, AgentEvent, AgentId, Role, Router, RouterError, Task, TaskContext, TaskId,
};

/// State captured at a task handoff, fed to the summarizer.
///
/// `task` and `events` describe what happened up to the handoff point;
/// `from_agent` and `to_role` are advisory hints used by
/// [`format_handoff_prompt`] to phrase the request.
#[derive(Debug, Clone)]
pub struct HandoffState {
    /// The task being handed off.
    pub task: Task,
    /// Events emitted by the prior agent during its execution attempt.
    pub events: Vec<AgentEvent>,
    /// Identifier of the agent that was running the task before the handoff.
    pub from_agent: Option<AgentId>,
    /// Role the task is being handed off to (e.g. `Reviewer`).
    pub to_role: Option<Role>,
}

/// Result of a successful summarization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffSummary {
    /// The summary text the model produced.
    pub text: String,
    /// Identifier of the agent (and therefore model) that produced the
    /// summary, for telemetry and provenance.
    pub model: AgentId,
}

/// Errors a [`HandoffSummarizer`] can return.
#[derive(Debug, Error)]
pub enum SummarizerError {
    /// The router could not select a summarize-capable agent.
    #[error("router error: {0}")]
    Routing(#[from] RouterError),
    /// The selected agent failed to start or complete the summarization.
    #[error("summarizer agent {agent} failed: {source}")]
    Execute {
        /// The agent that produced the failure.
        agent: AgentId,
        /// Underlying agent failure.
        #[source]
        source: AgentError,
    },
    /// The summarizer agent terminated without producing any output.
    #[error("summarizer agent {0} produced no summary text")]
    Empty(AgentId),
}

/// Asynchronously produce a [`HandoffSummary`] from a [`HandoffState`].
///
/// Implementations are expected to delegate to whichever model is cheapest
/// at the moment of the call; the canonical implementation is
/// [`RouterHandoffSummarizer`].
#[async_trait]
pub trait HandoffSummarizer: Send + Sync {
    /// Produce a summary of `state`. Errors propagate the underlying
    /// routing or agent failure.
    async fn summarize(&self, state: HandoffState) -> Result<HandoffSummary, SummarizerError>;
}

/// [`HandoffSummarizer`] that delegates to the cheapest healthy
/// [`Role::Summarize`] agent registered with a [`Router`].
///
/// The struct is cheap to clone — it holds an [`Arc<Router>`] and nothing
/// else, so dispatcher code can hand a fresh `Self` to each task without
/// rebuilding routing state.
#[derive(Clone)]
pub struct RouterHandoffSummarizer {
    router: Arc<Router>,
}

impl RouterHandoffSummarizer {
    /// Construct a new summarizer that routes through `router`.
    pub fn new(router: Arc<Router>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl HandoffSummarizer for RouterHandoffSummarizer {
    async fn summarize(&self, state: HandoffState) -> Result<HandoffSummary, SummarizerError> {
        let prompt = format_handoff_prompt(&state);
        // Reuse the original task's context (cwd, env) so the summarizer
        // has the same on-disk view, but generate a fresh `TaskId` so the
        // summary call doesn't collide with the boundary tracker on the
        // original task.
        let task = Task {
            id: TaskId::new(),
            role: Role::Summarize,
            prompt,
            context: TaskContext {
                cwd: state.task.context.cwd.clone(),
                env: state.task.context.env.clone(),
                metadata: state.task.context.metadata.clone(),
            },
            // Conservative budget hint: summaries are short by construction.
            budget_hint: Some(2_000),
        };

        let agent = self.router.select(&task).await?.clone();
        let agent_id = agent.id();
        let mut stream = agent
            .execute(task)
            .await
            .map_err(|source| SummarizerError::Execute {
                agent: agent_id.clone(),
                source,
            })?;

        // Concatenate `OutputChunk`s and prefer the explicit `Completed`
        // summary if the agent supplied one. A `Failed` event is escalated
        // to `SummarizerError::Execute` so callers don't have to inspect
        // events themselves.
        let mut buffer = String::new();
        let mut completed_summary: Option<String> = None;
        while let Some(event) = stream.next().await {
            match event {
                AgentEvent::OutputChunk { text } => buffer.push_str(&text),
                AgentEvent::Completed { summary, .. } => {
                    completed_summary = summary;
                    break;
                }
                AgentEvent::Failed { error, .. } => {
                    return Err(SummarizerError::Execute {
                        agent: agent_id,
                        source: AgentError::Other(error),
                    });
                }
                AgentEvent::Started { .. }
                | AgentEvent::ToolCall { .. }
                | AgentEvent::ToolResult { .. } => {}
            }
        }

        let text = completed_summary.unwrap_or_else(|| buffer.trim().to_string());
        if text.is_empty() {
            return Err(SummarizerError::Empty(agent_id));
        }
        Ok(HandoffSummary {
            text,
            model: agent_id,
        })
    }
}

/// Build the default prompt fed to the summarizer agent.
///
/// Format:
/// 1. One-line goal restatement.
/// 2. Optional `From agent` / `Handing off to` lines.
/// 3. Bullet-list of events (one line per event, truncated to keep prompt
///    bounded).
/// 4. Closing instruction asking for a structured summary.
///
/// The output is deterministic for a given [`HandoffState`] so callers can
/// snapshot-test it.
pub fn format_handoff_prompt(state: &HandoffState) -> String {
    use std::fmt::Write as _;

    let mut s = String::with_capacity(512);
    s.push_str(
        "You are a concise task-handoff summarizer. Summarize the state of the following task in <= 200 words.\n\n",
    );
    let _ = writeln!(s, "Goal: {}", first_line(&state.task.prompt));
    if let Some(from) = &state.from_agent {
        let _ = writeln!(s, "From agent: {}", from);
    }
    if let Some(role) = state.to_role {
        let _ = writeln!(s, "Handing off to role: {:?}", role);
    }
    s.push('\n');
    s.push_str("Events so far:\n");
    if state.events.is_empty() {
        s.push_str("- (none)\n");
    } else {
        for event in &state.events {
            describe_event(&mut s, event);
        }
    }
    s.push('\n');
    s.push_str(
        "Produce a structured summary with sections: Goal, Progress, Pending, Hand-off notes.",
    );
    s
}

fn describe_event(out: &mut String, event: &AgentEvent) {
    use std::fmt::Write as _;
    match event {
        AgentEvent::Started { task_id } => {
            let _ = writeln!(out, "- Started task {task_id}");
        }
        AgentEvent::OutputChunk { text } => {
            let _ = writeln!(out, "- Output: {}", first_line(text));
        }
        AgentEvent::ToolCall { name, .. } => {
            let _ = writeln!(out, "- Tool call: {name}");
        }
        AgentEvent::ToolResult { name, .. } => {
            let _ = writeln!(out, "- Tool result: {name}");
        }
        AgentEvent::Completed { summary, .. } => {
            let _ = writeln!(
                out,
                "- Completed: {}",
                summary.as_deref().unwrap_or("(no agent summary)")
            );
        }
        AgentEvent::Failed { error, .. } => {
            let _ = writeln!(out, "- Failed: {}", first_line(error));
        }
    }
}

/// Return the first non-empty line of `s`, trimmed, or `"(empty)"` if `s`
/// is empty / whitespace-only. Used by the prompt builder to keep events
/// to one line each.
fn first_line(s: &str) -> &str {
    s.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("(empty)")
}
