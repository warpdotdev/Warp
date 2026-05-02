//! Bridges the local [`AgentDriver`] conversation model to the [`orchestrator`]
//! crate's [`Agent`] / [`Router`] abstraction, replacing the deprecated hosted
//! Oz execution path.
//!
//! [`LocalOrchestratorAgent`] wraps the driver's existing [`execute_run`]
//! mechanism behind the `orchestrator::Agent` trait so that routing, health
//! checks, and budget accounting can all go through the canonical orchestrator
//! stack without any changes to the underlying conversation machinery.
//!
//! [`build_local_router`] constructs a single-agent [`Router`] pre-populated
//! with a [`LocalOrchestratorAgent`] backed by [`Provider::FoundationModels`]
//! with unlimited caps — local runs are never budget-halted.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use orchestrator::{
    Agent, AgentError, AgentEvent, AgentEventStream, AgentId, AgentRegistration, Budget, Cap,
    Capabilities, Health, Provider, Role, Router, Task, TaskId,
};
use warpui::ModelSpawner;

use super::{AgentDriver, AgentRunPrompt, SDKConversationOutputStatus};

/// Stable identifier used for the local Warp agent in the orchestrator registry.
const LOCAL_AGENT_ID: &str = "local-warp-oz";

/// An [`orchestrator::Agent`] implementation that runs tasks through the local
/// Warp conversation model, replacing the hosted Oz cloud execution path.
///
/// The agent is constructed per-task with the `AgentRunPrompt` already baked in.
/// The orchestrator's [`Router`] uses this struct purely as a routing and
/// health/budget gate; the prompt data travels outside the
/// `orchestrator::Task::prompt` field (which is left empty) so we never need
/// to convert back from `String` to `AgentRunPrompt`.
pub(crate) struct LocalOrchestratorAgent {
    foreground: ModelSpawner<AgentDriver>,
    prompt: AgentRunPrompt,
    capabilities: Capabilities,
}

impl LocalOrchestratorAgent {
    pub(crate) fn new(foreground: ModelSpawner<AgentDriver>, prompt: AgentRunPrompt) -> Self {
        Self {
            foreground,
            prompt,
            capabilities: Capabilities {
                roles: HashSet::from([Role::Worker, Role::Planner]),
                max_context_tokens: 200_000,
                supports_tools: true,
                supports_vision: false,
            },
        }
    }
}

#[async_trait]
impl Agent for LocalOrchestratorAgent {
    fn id(&self) -> AgentId {
        AgentId(LOCAL_AGENT_ID.to_string())
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn health(&self) -> Health {
        Health {
            healthy: true,
            last_check: Utc::now(),
            error_rate: 0.0,
        }
    }

    /// Bridge `orchestrator::Agent::execute` to `AgentDriver::execute_run`.
    ///
    /// Returns an [`AgentEventStream`] that emits:
    /// - [`AgentEvent::Started`] immediately on entry.
    /// - [`AgentEvent::Completed`] when the conversation model reports success.
    /// - [`AgentEvent::Failed`] for all error / cancelled / blocked outcomes.
    async fn execute(&self, task: Task) -> Result<AgentEventStream, AgentError> {
        let foreground = self.foreground.clone();
        let prompt = self.prompt.clone();
        let task_id = task.id;

        let stream = async_stream::stream! {
            yield AgentEvent::Started { task_id };

            // Hand off execution to the driver's existing conversation model.
            // `execute_run` is private to the `driver` module; child modules
            // may access it per Rust's privacy rules.
            let status_rx = match foreground
                .spawn(move |me, ctx| me.execute_run(prompt, ctx))
                .await
            {
                Ok(rx) => rx,
                Err(_) => {
                    yield AgentEvent::Failed {
                        task_id,
                        error: "local orchestrator: driver model unavailable".into(),
                    };
                    return;
                }
            };

            match status_rx.await {
                Ok(SDKConversationOutputStatus::Success) => {
                    yield AgentEvent::Completed { task_id, summary: None };
                }
                Ok(SDKConversationOutputStatus::Error { error }) => {
                    yield AgentEvent::Failed {
                        task_id,
                        error: error.to_string(),
                    };
                }
                Ok(SDKConversationOutputStatus::Cancelled { reason }) => {
                    yield AgentEvent::Failed {
                        task_id,
                        error: format!("cancelled: {reason:?}"),
                    };
                }
                Ok(SDKConversationOutputStatus::Blocked { blocked_action }) => {
                    yield AgentEvent::Failed {
                        task_id,
                        error: format!("blocked: {blocked_action}"),
                    };
                }
                Err(_) => {
                    yield AgentEvent::Failed {
                        task_id,
                        error: "local orchestrator: driver dropped before conversation finished"
                            .into(),
                    };
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

/// Construct a single-agent [`Router`] backed by a [`LocalOrchestratorAgent`].
///
/// Uses [`Provider::FoundationModels`] as the billing bucket with `u64::MAX`
/// caps so that local runs are never budget-halted. The returned router can
/// immediately select the local agent for any [`Role::Worker`] or
/// [`Role::Planner`] task.
pub(crate) fn build_local_router(
    foreground: ModelSpawner<AgentDriver>,
    prompt: AgentRunPrompt,
) -> Router {
    let mut caps = HashMap::new();
    caps.insert(
        Provider::FoundationModels,
        Cap {
            monthly_micro_dollars: u64::MAX,
            session_micro_dollars: u64::MAX,
        },
    );
    let budget = Arc::new(Budget::new(caps));
    let mut router = Router::new(budget);
    router.register(AgentRegistration {
        agent: Arc::new(LocalOrchestratorAgent::new(foreground, prompt)),
        provider: Provider::FoundationModels,
        estimated_micros_per_task: 0,
    });
    router
}

/// Generates a fresh [`TaskId`] for use when constructing an
/// `orchestrator::Task` in the Oz dispatch arm.
///
/// Exposed as a thin convenience wrapper so the call-site in `driver.rs`
/// does not need to import `TaskId` and call `TaskId::new()` directly,
/// keeping the orchestrator API surface minimal at the call site.
pub(crate) fn new_task_id() -> TaskId {
    TaskId::new()
}
