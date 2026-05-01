//! Stub [`Agent`] for Apple's on-device Foundation Models runtime.
//!
//! The real implementation lives behind a Swift bridge tracked by PDX-13;
//! until that lands, this stub stands in to keep the agent registry
//! enumerable and the router happy. It always reports itself unhealthy so
//! the dispatcher skips it, and `execute()` returns a synthetic
//! [`AgentEvent::Failed`] explaining the situation.
//!
//! The stub does, however, advertise the capabilities Foundation Models is
//! expected to fulfil per the master plan (`Inline`, `ToolRouter`,
//! `Summarize`) so router code can be exercised end-to-end without waiting
//! on the bridge.

use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use chrono::Utc;
use orchestrator::{
    Agent, AgentError, AgentEvent, AgentEventStream, AgentId, Capabilities, Health, Role, Task,
};
use tokio::sync::Mutex;

/// Context window the production Foundation Models tier is expected to
/// expose. Used by the stub's advertised [`Capabilities`] so router unit
/// tests get realistic numbers.
pub const FOUNDATION_MODELS_CONTEXT_TOKENS: u32 = 4_096;

/// Stub Foundation Models agent. Always unhealthy; `execute` always fails.
pub struct FoundationModelsAgent {
    id: AgentId,
    capabilities: Capabilities,
    health: Arc<Mutex<Health>>,
}

impl FoundationModelsAgent {
    /// Construct a new [`FoundationModelsAgent`]. Infallible — the stub does
    /// not probe the host Swift runtime; that's PDX-13's job.
    pub fn new(id: AgentId) -> Self {
        use std::collections::HashSet;
        let roles: HashSet<Role> = [Role::Inline, Role::ToolRouter, Role::Summarize]
            .into_iter()
            .collect();
        let capabilities = Capabilities {
            roles,
            max_context_tokens: FOUNDATION_MODELS_CONTEXT_TOKENS,
            supports_tools: false,
            supports_vision: false,
        };
        let health = Arc::new(Mutex::new(Health {
            // Permanently unhealthy until PDX-13 wires the Swift bridge.
            healthy: false,
            last_check: Utc::now(),
            error_rate: 0.0,
        }));
        Self {
            id,
            capabilities,
            health,
        }
    }
}

#[async_trait]
impl Agent for FoundationModelsAgent {
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
                healthy: false,
                last_check: Utc::now(),
                error_rate: 0.0,
            }
        }
    }

    async fn execute(&self, task: Task) -> Result<AgentEventStream, AgentError> {
        let task_id = task.id;
        let stream = stream! {
            yield AgentEvent::Failed {
                task_id,
                error: "FoundationModelsAgent is a stub; PDX-13 implements the Swift bridge"
                    .to_string(),
            };
        };
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_advertise_inline_toolrouter_summarize() {
        let agent = FoundationModelsAgent::new(AgentId("fm".into()));
        let caps = agent.capabilities();
        assert!(caps.roles.contains(&Role::Inline));
        assert!(caps.roles.contains(&Role::ToolRouter));
        assert!(caps.roles.contains(&Role::Summarize));
        assert!(!caps.supports_tools);
        assert!(!caps.supports_vision);
        assert_eq!(caps.max_context_tokens, FOUNDATION_MODELS_CONTEXT_TOKENS);
    }

    #[test]
    fn health_is_unhealthy() {
        let agent = FoundationModelsAgent::new(AgentId("fm".into()));
        assert!(!agent.health().healthy);
    }
}
