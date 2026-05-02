//! Foundation types for the multi-agent orchestrator.
//!
//! This crate defines the core trait, role enum, task and event types that
//! every concrete agent implementation in the workspace must speak. It is
//! intentionally minimal: routing, budget tracking, and dispatch logic live
//! in sibling crates and are *not* part of this foundation layer. The goal
//! here is to give those higher-level components a stable, serde-friendly
//! contract to plan against.
//!
//! # Example
//!
//! ```no_run
//! use orchestrator::{Agent, AgentError, AgentEventStream, AgentId, Capabilities, Health, Task};
//! use async_trait::async_trait;
//!
//! struct NoopAgent;
//!
//! #[async_trait]
//! impl Agent for NoopAgent {
//!     fn id(&self) -> AgentId {
//!         AgentId("noop".to_string())
//!     }
//!     fn capabilities(&self) -> &Capabilities {
//!         // In real code you'd return a borrowed field on `self`.
//!         unimplemented!()
//!     }
//!     async fn execute(&self, _task: Task) -> Result<AgentEventStream, AgentError> {
//!         Err(AgentError::Other("noop".into()))
//!     }
//!     fn health(&self) -> Health {
//!         unimplemented!()
//!     }
//! }
//! ```

#![deny(missing_docs)]

pub mod budget;
pub mod mcp_forwarder;
pub mod router;

pub use budget::{
    evaluate_charge, Budget, BudgetError, BudgetSnapshot, BudgetTier, Cap, CustomProviderId,
    Provider,
};
pub use mcp_forwarder::{ForwardingTarget, McpForwarder};
pub use router::{AgentRegistration, Router, RouterError};

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Identifier for a concrete [`Agent`] instance.
///
/// Wraps a free-form string so callers can pick whatever scheme suits their
/// runtime (uuids, slugs, hostnames). Implements [`fmt::Display`] and
/// [`FromStr`] so it round-trips trivially through configuration and logs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AgentId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(AgentId(s.to_string()))
    }
}

/// High-level role an agent (or task) is playing in the orchestration graph.
///
/// The router uses this enum to match tasks to agents that advertise the
/// corresponding capability. Variants are intentionally coarse — finer
/// specialization belongs on [`Capabilities`] or task metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Decomposes a goal into an ordered set of sub-tasks.
    Planner,
    /// Reviews work produced by other agents.
    Reviewer,
    /// General-purpose worker that executes a planned task.
    Worker,
    /// Performs large-scale, mechanical refactors across many files.
    BulkRefactor,
    /// Produces concise summaries of long inputs or histories.
    Summarize,
    /// Selects and dispatches the appropriate tool for a request.
    ToolRouter,
    /// Inline assistant (low-latency, short-context completions).
    Inline,
}

/// Strongly-typed wrapper around a [`Uuid`] identifying a single [`Task`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Generate a fresh random [`TaskId`].
    pub fn new() -> Self {
        TaskId(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Ambient context passed alongside a task prompt.
///
/// Holds the working directory, environment variables and any free-form
/// metadata the caller wants to thread through to the agent. The metadata
/// map intentionally uses [`serde_json::Value`] so that callers can attach
/// arbitrary structured data without coupling this crate to their schemas.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskContext {
    /// Working directory the task should execute in.
    pub cwd: PathBuf,
    /// Environment variables visible to the task.
    pub env: HashMap<String, String>,
    /// Caller-defined metadata, opaque to the orchestrator.
    pub metadata: HashMap<String, Value>,
}

/// A unit of work dispatched to an [`Agent`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier for this task.
    pub id: TaskId,
    /// Role the task expects the executing agent to fulfil.
    pub role: Role,
    /// User-facing prompt or instruction describing the work.
    pub prompt: String,
    /// Ambient execution context.
    pub context: TaskContext,
    /// Optional caller-supplied budget hint (interpretation is up to the
    /// budget layer — typically tokens or milliseconds).
    pub budget_hint: Option<u64>,
}

/// Streaming event emitted by an [`Agent`] while executing a [`Task`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    /// The agent has accepted the task and started work.
    Started {
        /// Identifier of the task that started.
        task_id: TaskId,
    },
    /// A chunk of textual output from the agent.
    OutputChunk {
        /// The output text fragment.
        text: String,
    },
    /// The agent invoked a tool.
    ToolCall {
        /// Name of the tool being invoked.
        name: String,
        /// JSON-encoded arguments passed to the tool.
        args: Value,
    },
    /// The agent received the result of a previously-issued tool call.
    ToolResult {
        /// Name of the tool that produced the result.
        name: String,
        /// JSON-encoded tool result.
        result: Value,
    },
    /// The agent finished the task successfully.
    Completed {
        /// Identifier of the task that completed.
        task_id: TaskId,
        /// Optional human-readable summary of the work performed.
        summary: Option<String>,
    },
    /// The agent failed the task.
    Failed {
        /// Identifier of the task that failed.
        task_id: TaskId,
        /// Description of the failure.
        error: String,
    },
}

/// Boxed, pinned async stream of [`AgentEvent`]s returned by
/// [`Agent::execute`].
pub type AgentEventStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;

/// Capabilities advertised by an agent to the router.
#[derive(Debug, Clone)]
pub struct Capabilities {
    /// Roles this agent can fulfil.
    pub roles: HashSet<Role>,
    /// Maximum context window the agent can accept, in tokens.
    pub max_context_tokens: u32,
    /// Whether the agent supports tool calling.
    pub supports_tools: bool,
    /// Whether the agent supports image / vision inputs.
    pub supports_vision: bool,
}

/// Snapshot of an agent's health.
#[derive(Debug, Clone)]
pub struct Health {
    /// Whether the agent is currently considered healthy.
    pub healthy: bool,
    /// Time at which this health snapshot was taken.
    pub last_check: DateTime<Utc>,
    /// Recent error rate, in `[0.0, 1.0]`.
    pub error_rate: f32,
}

/// Errors an [`Agent`] may surface while executing a [`Task`].
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The configured budget for the task was exceeded.
    #[error("budget exceeded")]
    BudgetExceeded,
    /// The task's required capability is not advertised by this agent.
    #[error("capability mismatch")]
    CapabilityMismatch,
    /// The agent reports itself unhealthy and refuses the task.
    #[error("agent unhealthy")]
    Unhealthy,
    /// The task was cancelled by the caller or the orchestrator.
    #[error("cancelled")]
    Cancelled,
    /// An I/O error occurred while executing the task.
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),
    /// A catch-all for agent-specific failure modes.
    #[error("{0}")]
    Other(String),
}

/// Core trait every agent implementation must satisfy.
///
/// Implementors are expected to be cheap to clone or share behind an [`Arc`]
/// since the dispatcher will hold them for the lifetime of the orchestrator.
///
/// [`Arc`]: std::sync::Arc
#[async_trait]
pub trait Agent: Send + Sync {
    /// Stable identifier for this agent instance.
    fn id(&self) -> AgentId;

    /// Capabilities advertised to the router.
    fn capabilities(&self) -> &Capabilities;

    /// Begin executing `task`, returning a stream of [`AgentEvent`]s that
    /// terminates with either [`AgentEvent::Completed`] or
    /// [`AgentEvent::Failed`].
    async fn execute(&self, task: Task) -> Result<AgentEventStream, AgentError>;

    /// Snapshot of the agent's current [`Health`].
    fn health(&self) -> Health;
}
