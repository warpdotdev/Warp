//! Integration tests for the dispatcher.
//!
//! Uses a `MockProvisioner` (records provision/release calls in memory)
//! and a `MockAgent` (controllable hold-then-emit behavior) so concurrency
//! caps and worktree lifecycle can be verified deterministically.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

use async_stream::stream;
use async_trait::async_trait;
use chrono::Utc;
use futures_util::StreamExt;
use orchestrator::{
    Agent, AgentEvent, AgentEventStream, AgentId, AgentRegistration, Budget, Cap, Capabilities,
    DispatchError, DispatcherBuilder, DispatcherConfig, Health, Provider, Role, Router, Task,
    TaskContext, TaskId, Worktree, WorktreeError, WorktreeProvisioner,
};
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Default)]
struct ProvisionerLog {
    provisioned: Vec<TaskId>,
    released: Vec<PathBuf>,
}

struct MockProvisioner {
    log: Arc<Mutex<ProvisionerLog>>,
    counter: AtomicU32,
    base: PathBuf,
}

impl MockProvisioner {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            log: Arc::new(Mutex::new(ProvisionerLog::default())),
            counter: AtomicU32::new(0),
            base: PathBuf::from("/tmp/dispatcher-test-worktrees"),
        })
    }

    fn log(&self) -> Arc<Mutex<ProvisionerLog>> {
        self.log.clone()
    }
}

#[async_trait]
impl WorktreeProvisioner for MockProvisioner {
    async fn provision(&self, task_id: TaskId) -> Result<Worktree, WorktreeError> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut log = self.log.lock().await;
        log.provisioned.push(task_id);
        Ok(Worktree {
            path: self.base.join(format!("wt-{n}")),
            branch: format!("wt-branch-{n}"),
        })
    }
    fn release(&self, worktree: Worktree) {
        // Avoid blocking inside Drop: try_lock first; if contended, push
        // best-effort via a parking_lot-style spin would over-engineer the
        // test. Tests await on empty lock before asserting.
        if let Ok(mut log) = self.log.try_lock() {
            log.released.push(worktree.path);
        } else {
            // Fall back: spawn a tiny task on the current runtime that
            // performs the release async-ly. Tests use `flush_releases`
            // below to wait for cleanup.
            let log = self.log.clone();
            tokio::spawn(async move {
                let mut g = log.lock().await;
                g.released.push(worktree.path);
            });
        }
    }
}

/// Wait up to ~500 ms for the provisioner to record `expected` releases.
async fn wait_for_releases(log: &Arc<Mutex<ProvisionerLog>>, expected: usize) {
    for _ in 0..50 {
        {
            let g = log.lock().await;
            if g.released.len() >= expected {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let g = log.lock().await;
    panic!(
        "expected {} releases, only saw {} ({:?})",
        expected,
        g.released.len(),
        g.released
    );
}

/// Agent that tracks how many concurrent executions are in flight,
/// and waits on a [`Notify`] before emitting Completed. Tests use this to
/// verify concurrency caps.
struct GatedAgent {
    id: AgentId,
    capabilities: Capabilities,
    in_flight: Arc<AtomicU32>,
    max_observed: Arc<AtomicU32>,
    release: Arc<Notify>,
    started: Arc<Notify>,
}

impl GatedAgent {
    fn new(
        id: &str,
        in_flight: Arc<AtomicU32>,
        max_observed: Arc<AtomicU32>,
        release: Arc<Notify>,
        started: Arc<Notify>,
    ) -> Self {
        Self {
            id: AgentId(id.to_string()),
            capabilities: Capabilities {
                roles: [Role::Worker].into_iter().collect::<HashSet<_>>(),
                max_context_tokens: 8_192,
                supports_tools: false,
                supports_vision: false,
            },
            in_flight,
            max_observed,
            release,
            started,
        }
    }
}

#[async_trait]
impl Agent for GatedAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
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
    async fn execute(
        &self,
        task: Task,
    ) -> Result<AgentEventStream, orchestrator::AgentError> {
        let in_flight = self.in_flight.clone();
        let max_observed = self.max_observed.clone();
        let release = self.release.clone();
        let started = self.started.clone();
        let task_id = task.id;
        let s = stream! {
            // Track max concurrency.
            let n = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            max_observed.fetch_max(n, Ordering::SeqCst);
            yield AgentEvent::Started { task_id };
            started.notify_one();
            // Hold the slot until the test releases.
            release.notified().await;
            yield AgentEvent::Completed { task_id, summary: None };
            in_flight.fetch_sub(1, Ordering::SeqCst);
        };
        Ok(Box::pin(s))
    }
}

/// Always-completes agent for the simpler happy-path tests.
struct ImmediateAgent {
    id: AgentId,
    capabilities: Capabilities,
}

impl ImmediateAgent {
    fn new(id: &str) -> Self {
        Self {
            id: AgentId(id.to_string()),
            capabilities: Capabilities {
                roles: [Role::Worker].into_iter().collect::<HashSet<_>>(),
                max_context_tokens: 8_192,
                supports_tools: false,
                supports_vision: false,
            },
        }
    }
}

#[async_trait]
impl Agent for ImmediateAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
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
    async fn execute(
        &self,
        task: Task,
    ) -> Result<AgentEventStream, orchestrator::AgentError> {
        let task_id = task.id;
        let cwd = task.context.cwd.clone();
        let s = stream! {
            yield AgentEvent::Started { task_id };
            yield AgentEvent::OutputChunk { text: cwd.display().to_string() };
            yield AgentEvent::Completed { task_id, summary: None };
        };
        Ok(Box::pin(s))
    }
}

fn ample_budget(providers: &[Provider]) -> Arc<Budget> {
    let mut caps = HashMap::new();
    for p in providers {
        caps.insert(
            *p,
            Cap {
                monthly_micro_dollars: 1_000_000_000,
                session_micro_dollars: 1_000_000_000,
            },
        );
    }
    Arc::new(Budget::new(caps))
}

fn worker_task() -> Task {
    Task {
        id: TaskId::new(),
        role: Role::Worker,
        prompt: "do work".to_string(),
        context: TaskContext {
            cwd: PathBuf::from("/should/be/overridden"),
            env: HashMap::new(),
            metadata: HashMap::new(),
        },
        budget_hint: None,
    }
}

#[tokio::test]
async fn dispatch_provisions_worktree_and_overrides_cwd() {
    let provisioner = MockProvisioner::new();
    let log = provisioner.log();

    let dispatcher = DispatcherBuilder::new(
        Router::new(ample_budget(&[Provider::ClaudeCode])),
        provisioner.clone(),
    )
    .register(AgentRegistration {
        agent: Arc::new(ImmediateAgent::new("immediate")),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    })
    .build();

    let outcome = dispatcher
        .dispatch(worker_task())
        .await
        .expect("dispatch succeeds");

    assert_eq!(outcome.agent.0, "immediate");
    assert!(outcome
        .worktree_path
        .starts_with("/tmp/dispatcher-test-worktrees"));

    // Drain stream and assert the agent saw the patched cwd.
    let events: Vec<_> = outcome.events.collect().await;
    let chunk = events.iter().find_map(|e| match e {
        AgentEvent::OutputChunk { text } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(
        chunk.as_deref(),
        Some(outcome.worktree_path.display().to_string().as_str())
    );

    wait_for_releases(&log, 1).await;
    let g = log.lock().await;
    assert_eq!(g.provisioned.len(), 1);
    assert_eq!(g.released.len(), 1);
    assert_eq!(g.released[0], outcome.worktree_path);
}

#[tokio::test]
async fn global_concurrency_cap_blocks_extra_dispatches() {
    let provisioner = MockProvisioner::new();
    let in_flight = Arc::new(AtomicU32::new(0));
    let max_observed = Arc::new(AtomicU32::new(0));
    let release = Arc::new(Notify::new());
    let started = Arc::new(Notify::new());

    let dispatcher = DispatcherBuilder::new(
        Router::new(ample_budget(&[Provider::ClaudeCode])),
        provisioner.clone(),
    )
    .with_config(DispatcherConfig {
        global_concurrency: 2,
        per_provider_concurrency: HashMap::new(),
        default_per_provider: 8,
    })
    .register(AgentRegistration {
        agent: Arc::new(GatedAgent::new(
            "gated",
            in_flight.clone(),
            max_observed.clone(),
            release.clone(),
            started.clone(),
        )),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    })
    .build();

    // Spawn 5 dispatches; cap is 2, so only 2 should be active at any time.
    let dispatcher = Arc::new(dispatcher);
    let mut handles = vec![];
    for _ in 0..5 {
        let d = dispatcher.clone();
        handles.push(tokio::spawn(async move {
            let outcome = d.dispatch(worker_task()).await.expect("dispatch");
            // Drain events; this releases the worktree on stream end.
            let _: Vec<_> = outcome.events.collect().await;
        }));
    }

    // Wait for first batch of starts then release sequentially.
    started.notified().await;
    started.notified().await;
    // Give the dispatcher a moment to (correctly) NOT start a third before
    // we release one slot.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        max_observed.load(Ordering::SeqCst) <= 2,
        "max_observed={}, expected <=2",
        max_observed.load(Ordering::SeqCst)
    );

    // Release them all.
    for _ in 0..5 {
        release.notify_one();
        // Each release lets one task complete and frees a slot.
        // The next task will start; await its start signal.
        let _ = tokio::time::timeout(Duration::from_secs(1), started.notified()).await;
    }

    for h in handles {
        h.await.unwrap();
    }
    assert!(max_observed.load(Ordering::SeqCst) <= 2);
    assert_eq!(in_flight.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn per_provider_cap_independently_throttles() {
    let provisioner = MockProvisioner::new();
    let in_flight_a = Arc::new(AtomicU32::new(0));
    let in_flight_b = Arc::new(AtomicU32::new(0));
    let max_a = Arc::new(AtomicU32::new(0));
    let max_b = Arc::new(AtomicU32::new(0));
    let release = Arc::new(Notify::new());
    let started = Arc::new(Notify::new());

    let mut per = HashMap::new();
    per.insert(Provider::ClaudeCode, 1);
    per.insert(Provider::Codex, 2);

    let dispatcher = DispatcherBuilder::new(
        Router::new(ample_budget(&[Provider::ClaudeCode, Provider::Codex])),
        provisioner.clone(),
    )
    .with_config(DispatcherConfig {
        global_concurrency: 16,
        per_provider_concurrency: per,
        default_per_provider: 4,
    })
    .register(AgentRegistration {
        // Cheap so the router prefers this for ClaudeCode-routed tasks.
        agent: Arc::new(GatedAgent::new(
            "claude-agent",
            in_flight_a.clone(),
            max_a.clone(),
            release.clone(),
            started.clone(),
        )),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    })
    .register(AgentRegistration {
        agent: Arc::new(GatedAgent::new(
            "codex-agent",
            in_flight_b.clone(),
            max_b.clone(),
            release.clone(),
            started.clone(),
        )),
        provider: Provider::Codex,
        // Higher cost so router never picks this for the same role —
        // separate test would otherwise use a different role per agent.
        estimated_micros_per_task: 1_000_000,
    })
    .build();

    // The router will pick claude-agent for every Worker task because it's
    // cheaper. So we exercise the per-provider cap on ClaudeCode.
    let dispatcher = Arc::new(dispatcher);
    let mut handles = vec![];
    for _ in 0..3 {
        let d = dispatcher.clone();
        handles.push(tokio::spawn(async move {
            let outcome = d.dispatch(worker_task()).await.expect("dispatch");
            let _: Vec<_> = outcome.events.collect().await;
        }));
    }

    // Cap on ClaudeCode is 1; only one should be active.
    let _ = tokio::time::timeout(Duration::from_secs(1), started.notified()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        max_a.load(Ordering::SeqCst) <= 1,
        "ClaudeCode cap of 1 violated: max_a={}",
        max_a.load(Ordering::SeqCst)
    );

    // Release them all sequentially.
    for _ in 0..3 {
        release.notify_one();
        let _ = tokio::time::timeout(Duration::from_secs(1), started.notified()).await;
    }
    for h in handles {
        h.await.unwrap();
    }
    assert!(max_a.load(Ordering::SeqCst) <= 1);
    assert_eq!(in_flight_a.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dispatch_returns_routing_error_when_no_capable_agent() {
    let provisioner = MockProvisioner::new();
    let log = provisioner.log();
    let dispatcher = DispatcherBuilder::new(
        Router::new(ample_budget(&[Provider::ClaudeCode])),
        provisioner.clone(),
    )
    .build();

    let result = dispatcher.dispatch(worker_task()).await;
    let err = match result {
        Ok(_) => panic!("expected routing error, got Ok"),
        Err(e) => e,
    };
    assert!(matches!(err, DispatchError::Routing(_)));
    // No worktree should have been provisioned.
    let g = log.lock().await;
    assert_eq!(g.provisioned.len(), 0);
    assert_eq!(g.released.len(), 0);
}

#[tokio::test]
async fn early_drop_of_stream_releases_worktree() {
    let provisioner = MockProvisioner::new();
    let log = provisioner.log();
    let dispatcher = DispatcherBuilder::new(
        Router::new(ample_budget(&[Provider::ClaudeCode])),
        provisioner.clone(),
    )
    .register(AgentRegistration {
        agent: Arc::new(ImmediateAgent::new("immediate")),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    })
    .build();

    // Dispatch and immediately drop the outcome (specifically the events
    // stream) without consuming events. Resources should still release.
    {
        let outcome = dispatcher.dispatch(worker_task()).await.expect("dispatch");
        // Take the first event then drop.
        let mut events = outcome.events;
        let _first = events.next().await;
        drop(events);
    }

    wait_for_releases(&log, 1).await;
    assert_eq!(dispatcher.available_global_slots(), 8); // default config
}
