//! Concurrency-capped, worktree-per-worker task dispatcher.
//!
//! The [`Dispatcher`] composes the [`Router`] with a pluggable
//! [`WorktreeProvisioner`] and a pair of [`Semaphore`]s — one global, one
//! per [`Provider`] — so that callers can fire-and-forget [`Task`]s and
//! trust the orchestrator to cap parallelism and isolate every worker in
//! its own working directory.
//!
//! # What this module does
//!
//! [`Dispatcher::dispatch`] runs the following pipeline for each task:
//!
//! 1. Acquire a slot from the global concurrency [`Semaphore`]; if all
//!    slots are taken the call awaits.
//! 2. Ask the [`Router`] to select an [`Agent`].
//! 3. Look up the chosen agent's [`Provider`] in the dispatcher's
//!    registration map and acquire a slot from that provider's
//!    [`Semaphore`]; per-provider caps are independent.
//! 4. Provision a fresh worktree via the [`WorktreeProvisioner`]; rewrite
//!    `task.context.cwd` to point at that worktree so the worker sees its
//!    own checkout.
//! 5. Hand the task to [`Agent::execute`] and wrap the resulting event
//!    stream in a guard that releases all four resources (global permit,
//!    per-provider permit, worktree, and any future boundary guard) when
//!    the wrapping stream is dropped — i.e. when the caller stops consuming
//!    events.
//!
//! # What this module does *not* do
//!
//! - It does not enforce mid-task agent stability — that's
//!   [`crate::boundary::TaskBoundary`], wired in once both PDX-40 and
//!   PDX-42 land on master.
//! - It does not invoke `git worktree` itself. The [`WorktreeProvisioner`]
//!   trait is the seam where the real implementation (or a test stub)
//!   plugs in.
//!
//! [`Agent`]: crate::Agent
//! [`Agent::execute`]: crate::Agent::execute
//! [`Router`]: crate::router::Router

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use futures_util::StreamExt;
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    AgentEventStream, AgentId, AgentRegistration, Provider, Router, RouterError, Task, TaskId,
};

/// Information about a freshly provisioned worktree.
///
/// The dispatcher owns this value for the lifetime of the dispatched task
/// and hands it back to the [`WorktreeProvisioner`] for cleanup once the
/// task's event stream is dropped.
#[derive(Debug, Clone)]
pub struct Worktree {
    /// Absolute path to the worktree's working directory.
    pub path: PathBuf,
    /// Branch checked out in the worktree.
    pub branch: String,
}

/// Errors a [`WorktreeProvisioner`] may surface.
#[derive(Debug, Error)]
pub enum WorktreeError {
    /// Provisioning failed — the underlying error message is opaque to the
    /// dispatcher.
    #[error("worktree provisioning failed: {0}")]
    Provision(String),
}

/// Pluggable strategy for creating and tearing down per-task worktrees.
///
/// Real implementations shell out to `git worktree add` / `git worktree
/// remove`. Tests use an in-memory stub that records calls. The trait is
/// async on the create path and synchronous on release because the release
/// happens inside [`Drop`] where awaiting is impossible.
#[async_trait]
pub trait WorktreeProvisioner: Send + Sync {
    /// Provision a worktree for `task_id`. Implementations should produce a
    /// unique path per call so concurrent dispatches do not collide.
    async fn provision(&self, task_id: TaskId) -> Result<Worktree, WorktreeError>;

    /// Best-effort synchronous release. Implementations that need async
    /// cleanup should spawn a detached task here and return immediately —
    /// the dispatcher does not wait.
    fn release(&self, worktree: Worktree);
}

/// Concurrency configuration for a [`Dispatcher`].
#[derive(Debug, Clone)]
pub struct DispatcherConfig {
    /// Maximum number of in-flight tasks across all providers.
    pub global_concurrency: usize,
    /// Per-provider maximum. Providers absent from the map default to
    /// [`DispatcherConfig::default_per_provider`].
    pub per_provider_concurrency: HashMap<Provider, usize>,
    /// Default per-provider cap when a provider has no explicit entry.
    pub default_per_provider: usize,
}

impl DispatcherConfig {
    /// Convenience constructor with sensible defaults: 8 global slots,
    /// 4 per provider.
    pub fn with_defaults() -> Self {
        Self {
            global_concurrency: 8,
            per_provider_concurrency: HashMap::new(),
            default_per_provider: 4,
        }
    }
}

/// Errors returned by [`Dispatcher::dispatch`].
#[derive(Debug, Error)]
pub enum DispatchError {
    /// The router failed to select an agent for this task.
    #[error("router error: {0}")]
    Routing(#[from] RouterError),
    /// The dispatcher's registration map has no provider entry for the
    /// agent the router selected. Indicates a misconfiguration: every
    /// agent registered with the router should also be registered with
    /// the dispatcher.
    #[error("no provider mapping registered for agent {0}")]
    UnknownAgent(AgentId),
    /// Provisioning the worktree failed.
    #[error("worktree error: {0}")]
    Worktree(#[from] WorktreeError),
    /// The agent itself failed before producing a stream.
    #[error("agent execution failed: {0}")]
    Agent(#[from] crate::AgentError),
}

/// Concurrency-capped, worktree-aware task dispatcher.
///
/// Construct via [`DispatcherBuilder`] in production and via [`Dispatcher::new`]
/// in tests. The dispatcher is cheap to clone (everything inside is `Arc`-shared).
#[derive(Clone)]
pub struct Dispatcher {
    router: Arc<Router>,
    provisioner: Arc<dyn WorktreeProvisioner>,
    agent_provider: Arc<HashMap<AgentId, Provider>>,
    global_sema: Arc<Semaphore>,
    provider_sema: Arc<HashMap<Provider, Arc<Semaphore>>>,
    default_per_provider_sema: Arc<Semaphore>,
}

/// Outcome of a successful [`Dispatcher::dispatch`] call.
///
/// The `events` stream owns the dispatched task's resources (semaphore
/// permits, worktree, etc.) — dropping the stream releases everything.
/// `agent` and `worktree_path` are exposed for telemetry; they are clones
/// of the underlying values and do not affect resource lifetime.
pub struct DispatchOutcome {
    /// The agent the router selected.
    pub agent: AgentId,
    /// Path of the worktree provisioned for this task.
    pub worktree_path: PathBuf,
    /// Stream of [`crate::AgentEvent`]s emitted by the agent. Holds all
    /// per-task resources internally.
    pub events: AgentEventStream,
}

impl Dispatcher {
    /// Build a [`Dispatcher`] from a pre-populated [`Router`] plus the
    /// agent → provider mapping that mirrors the router's registrations.
    ///
    /// Prefer [`DispatcherBuilder`] for the common case where you control
    /// both the router and the agent registrations.
    pub fn new(
        router: Arc<Router>,
        provisioner: Arc<dyn WorktreeProvisioner>,
        agent_provider: HashMap<AgentId, Provider>,
        config: DispatcherConfig,
    ) -> Self {
        let global_sema = Arc::new(Semaphore::new(config.global_concurrency.max(1)));
        let mut provider_sema = HashMap::with_capacity(config.per_provider_concurrency.len());
        for (p, n) in &config.per_provider_concurrency {
            provider_sema.insert(*p, Arc::new(Semaphore::new((*n).max(1))));
        }
        let default_per_provider_sema =
            Arc::new(Semaphore::new(config.default_per_provider.max(1)));
        Self {
            router,
            provisioner,
            agent_provider: Arc::new(agent_provider),
            global_sema,
            provider_sema: Arc::new(provider_sema),
            default_per_provider_sema,
        }
    }

    /// Available global slots, primarily for tests and observability.
    pub fn available_global_slots(&self) -> usize {
        self.global_sema.available_permits()
    }

    /// Available slots for `provider`. Returns the default-pool count when
    /// the provider has no explicit entry.
    pub fn available_provider_slots(&self, provider: Provider) -> usize {
        match self.provider_sema.get(&provider) {
            Some(s) => s.available_permits(),
            None => self.default_per_provider_sema.available_permits(),
        }
    }

    /// Dispatch `task`. Awaits a global slot, selects an agent, awaits the
    /// chosen provider's slot, provisions a worktree, and hands the task
    /// to the agent. The returned [`DispatchOutcome::events`] stream owns
    /// all per-task resources; drop it to release them.
    pub async fn dispatch(&self, mut task: Task) -> Result<DispatchOutcome, DispatchError> {
        let task_id = task.id;

        // 1. Global slot.
        let global_permit = self
            .global_sema
            .clone()
            .acquire_owned()
            .await
            .expect("global semaphore is never closed");

        // 2. Router selects an agent.
        let agent_arc = self.router.select(&task).await?.clone();
        let agent_id = agent_arc.id();

        // 3. Look up the provider for that agent and grab its slot.
        let provider = self
            .agent_provider
            .get(&agent_id)
            .copied()
            .ok_or_else(|| DispatchError::UnknownAgent(agent_id.clone()))?;
        let provider_sema = match self.provider_sema.get(&provider) {
            Some(s) => s.clone(),
            None => self.default_per_provider_sema.clone(),
        };
        let provider_permit = provider_sema
            .acquire_owned()
            .await
            .expect("provider semaphore is never closed");

        // 4. Worktree.
        let worktree = self.provisioner.provision(task_id).await?;
        task.context.cwd = worktree.path.clone();
        let worktree_path = worktree.path.clone();
        let release_guard = WorktreeReleaseGuard::new(self.provisioner.clone(), worktree);

        // 5. Hand off to the agent.
        let agent_stream = agent_arc.execute(task).await?;

        // 6. Wrap the stream so all resources release when the consumer
        //    drops it (early termination) or when the stream completes.
        let outcome_stream = stream! {
            // Capture every per-task resource. They drop when this async
            // block is dropped, which happens when the caller drops the
            // returned stream.
            let _global = global_permit;
            let _provider = provider_permit;
            let _release = release_guard;
            let mut inner = agent_stream;
            while let Some(event) = inner.next().await {
                yield event;
            }
        };

        Ok(DispatchOutcome {
            agent: agent_id,
            worktree_path,
            events: Box::pin(outcome_stream),
        })
    }
}

/// Builder that wires a [`Router`] and a [`Dispatcher`] from the same set
/// of [`AgentRegistration`]s, so the agent → provider mapping cannot drift.
pub struct DispatcherBuilder {
    router: Router,
    agent_provider: HashMap<AgentId, Provider>,
    provisioner: Arc<dyn WorktreeProvisioner>,
    config: DispatcherConfig,
}

impl DispatcherBuilder {
    /// Start a new builder seeded with the given [`Router`] and worktree
    /// provisioner. Use [`DispatcherBuilder::register`] to add agents; the
    /// builder mirrors each registration into both the router and the
    /// dispatcher's provider map.
    pub fn new(router: Router, provisioner: Arc<dyn WorktreeProvisioner>) -> Self {
        Self {
            router,
            agent_provider: HashMap::new(),
            provisioner,
            config: DispatcherConfig::with_defaults(),
        }
    }

    /// Override the default [`DispatcherConfig`].
    pub fn with_config(mut self, config: DispatcherConfig) -> Self {
        self.config = config;
        self
    }

    /// Register an agent with both the router and the dispatcher's
    /// agent → provider map. Mirrors [`Router::register`]'s
    /// "last-registration-wins" semantic for the same [`AgentId`].
    pub fn register(mut self, registration: AgentRegistration) -> Self {
        let id = registration.agent.id();
        let provider = registration.provider;
        self.router.register(registration);
        self.agent_provider.insert(id, provider);
        self
    }

    /// Finalize the dispatcher.
    pub fn build(self) -> Dispatcher {
        Dispatcher::new(
            Arc::new(self.router),
            self.provisioner,
            self.agent_provider,
            self.config,
        )
    }
}

/// RAII guard that releases a worktree on drop unless explicitly disarmed.
struct WorktreeReleaseGuard {
    provisioner: Arc<dyn WorktreeProvisioner>,
    worktree: Option<Worktree>,
}

impl WorktreeReleaseGuard {
    fn new(provisioner: Arc<dyn WorktreeProvisioner>, worktree: Worktree) -> Self {
        Self {
            provisioner,
            worktree: Some(worktree),
        }
    }
}

impl Drop for WorktreeReleaseGuard {
    fn drop(&mut self) {
        if let Some(wt) = self.worktree.take() {
            self.provisioner.release(wt);
        }
    }
}

// `OwnedSemaphorePermit` already implements `Send`, but tying it explicitly
// to this module's public surface lets `cargo doc` show that the permits
// flow out via the wrapped stream.
#[allow(dead_code)]
const _ASSERT_PERMITS_SEND: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<OwnedSemaphorePermit>();
};
