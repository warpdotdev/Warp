//! Stub [`Agent`] for the remote / cloud backend.
//!
//! The real cloud client is tracked by PDX-C2 (`crates/cloud_client/`). Until
//! that crate lands this stub stands in so the agent registry stays
//! enumerable: it advertises a "kitchen sink" capability set (the cloud may
//! ultimately back any role) and reports itself healthy iff a `cloud_url`
//! was configured. `execute()` always returns a synthetic
//! [`AgentEvent::Failed`] explaining that the backend isn't wired yet.

use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use chrono::Utc;
use orchestrator::{
    Agent, AgentError, AgentEvent, AgentEventStream, AgentId, Capabilities, Health, Role, Task,
};
use tokio::sync::Mutex;

/// Context window the production cloud tier is expected to expose. Used by
/// the stub's advertised [`Capabilities`].
pub const REMOTE_CONTEXT_TOKENS: u32 = 200_000;

/// Stub remote agent. Healthy if and only if `cloud_url` is `Some`, but
/// `execute` always fails until PDX-C2 wires the actual transport.
pub struct RemoteAgent {
    id: AgentId,
    cloud_url: Option<String>,
    capabilities: Capabilities,
    health: Arc<Mutex<Health>>,
}

impl RemoteAgent {
    /// Construct a new [`RemoteAgent`]. Infallible — the stub does not
    /// actually open a connection. When `cloud_url` is `None` the agent is
    /// permanently unhealthy.
    pub fn new(id: AgentId, cloud_url: Option<String>) -> Self {
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
            max_context_tokens: REMOTE_CONTEXT_TOKENS,
            supports_tools: true,
            supports_vision: true,
        };
        let healthy = cloud_url.is_some();
        let health = Arc::new(Mutex::new(Health {
            healthy,
            last_check: Utc::now(),
            error_rate: 0.0,
        }));
        Self {
            id,
            cloud_url,
            capabilities,
            health,
        }
    }

    /// Configured cloud URL, if any. Exposed for diagnostics / tests.
    pub fn cloud_url(&self) -> Option<&str> {
        self.cloud_url.as_deref()
    }
}

#[async_trait]
impl Agent for RemoteAgent {
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
                healthy: self.cloud_url.is_some(),
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
                error: "RemoteAgent stub: cloud backend not yet wired (PDX-C2 territory)"
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
    fn capabilities_cover_all_roles() {
        let agent = RemoteAgent::new(AgentId("r".into()), None);
        let caps = agent.capabilities();
        for role in [
            Role::Planner,
            Role::Reviewer,
            Role::Worker,
            Role::BulkRefactor,
            Role::Summarize,
            Role::ToolRouter,
            Role::Inline,
        ] {
            assert!(caps.roles.contains(&role), "missing role: {role:?}");
        }
        assert!(caps.supports_tools);
        assert!(caps.supports_vision);
        assert_eq!(caps.max_context_tokens, REMOTE_CONTEXT_TOKENS);
    }

    #[test]
    fn health_reflects_cloud_url_presence() {
        let no_url = RemoteAgent::new(AgentId("r".into()), None);
        assert!(!no_url.health().healthy);

        let with_url =
            RemoteAgent::new(AgentId("r".into()), Some("wss://cloud.example".into()));
        assert!(with_url.health().healthy);
    }
}
