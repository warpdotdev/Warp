//! Symphony's poll-loop state machine and dispatcher (spec §7, §8).
//!
//! Owns the runtime state, the registered agents, the workspace manager,
//! and the audit log. On every tick:
//!
//!   1. Pull candidate issues from the tracker.
//!   2. Filter them per spec §8.2 (active state, not running/claimed,
//!      required label, concurrency cap respected).
//!   3. Sort by `(priority asc, created_at oldest, identifier lex)`.
//!   4. Dispatch the first eligible issue, streaming events into the audit
//!      log as the agent runs.
//!
//! Stall detection, retries, and reconciliation are deliberately omitted
//! from the MVP.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;
use orchestrator::{
    Agent, AgentEvent, AgentEventStream, AgentId, Role, Router, RouterError, Task, TaskContext,
    TaskId,
};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::audit::{AuditEvent, AuditEventKind, AuditLog};
use crate::diff_guard::{DiffGuard, DiffGuardError};
use crate::tracker::{Issue, TrackerError};
use crate::workflow::WorkflowDefinition;
use crate::workspace::{Workspace, WorkspaceError, WorkspaceManager};

/// Trait abstracting the tracker so tests can inject a mock.
#[async_trait]
pub trait IssueSource: Send + Sync {
    /// Fetch all issues whose state is in `active_states`.
    async fn fetch_candidate_issues(
        &self,
        active_states: &[String],
    ) -> Result<Vec<Issue>, TrackerError>;
}

#[async_trait]
impl IssueSource for crate::tracker::LinearClient {
    async fn fetch_candidate_issues(
        &self,
        active_states: &[String],
    ) -> Result<Vec<Issue>, TrackerError> {
        crate::tracker::LinearClient::fetch_candidate_issues(self, active_states).await
    }
}

/// Top-level orchestrator errors.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// Tracker call failed.
    #[error("tracker: {0}")]
    Tracker(#[from] TrackerError),
    /// Workspace setup failed.
    #[error("workspace: {0}")]
    Workspace(#[from] WorkspaceError),
    /// Router refused to assign an agent.
    #[error("router: {0}")]
    Router(String),
    /// Diff guard rejected the run.
    #[error("diff guard: {0}")]
    DiffGuard(#[from] DiffGuardError),
    /// Catch-all for unexpected failures.
    #[error("{0}")]
    Other(String),
}

impl From<RouterError> for OrchestratorError {
    fn from(value: RouterError) -> Self {
        OrchestratorError::Router(value.to_string())
    }
}

/// Bookkeeping for a currently-running issue dispatch.
#[derive(Debug, Clone)]
pub struct RunningEntry {
    /// Linear issue id.
    pub issue_id: String,
    /// Linear identifier (`PDX-12`).
    pub identifier: String,
    /// On-disk workspace path.
    pub workspace_path: PathBuf,
    /// Wall-clock instant the dispatch started.
    pub started_at: Instant,
    /// Selected agent id.
    pub agent_id: AgentId,
}

/// Mutable runtime state, guarded by an `RwLock` so that the tick task and
/// the spawned agent tasks can both mutate it.
#[derive(Default, Debug)]
pub struct RuntimeState {
    /// Issues actively being worked on, keyed by issue id.
    pub running: HashMap<String, RunningEntry>,
    /// Issues that have been claimed in this tick but haven't reached
    /// `running` state yet — kept as a defensive guard against double
    /// dispatch within a single tick.
    pub claimed: HashSet<String>,
    /// Issues we've completed in this process lifetime.
    pub completed: HashSet<String>,
}

/// Orchestrator core.
pub struct Orchestrator {
    workflow: WorkflowDefinition,
    tracker: Arc<dyn IssueSource>,
    workspaces: Arc<WorkspaceManager>,
    router: Arc<Router>,
    state: Arc<RwLock<RuntimeState>>,
    audit: Arc<AuditLog>,
    diff_guard: DiffGuard,
}

impl Orchestrator {
    /// Construct a new orchestrator wiring all collaborators.
    pub fn new(
        workflow: WorkflowDefinition,
        tracker: Arc<dyn IssueSource>,
        workspaces: Arc<WorkspaceManager>,
        router: Arc<Router>,
        audit: Arc<AuditLog>,
    ) -> Self {
        let max_diff = workflow.config.agent.max_diff_lines;
        Self {
            workflow,
            tracker,
            workspaces,
            router,
            state: Arc::new(RwLock::new(RuntimeState::default())),
            audit,
            diff_guard: DiffGuard::new(max_diff),
        }
    }

    /// Snapshot of current runtime state. Useful for tests.
    pub async fn state_snapshot(&self) -> (HashMap<String, RunningEntry>, HashSet<String>) {
        let s = self.state.read().await;
        (s.running.clone(), s.completed.clone())
    }

    /// Main loop: tick on `polling.interval_ms` until `shutdown` resolves.
    pub async fn run(self: Arc<Self>, mut shutdown: tokio::sync::oneshot::Receiver<()>) {
        let interval = Duration::from_millis(self.workflow.config.polling.interval_ms);
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = self.tick().await {
                        tracing::warn!(error = %e, "tick failed");
                    }
                }
                _ = &mut shutdown => {
                    tracing::info!("shutdown requested; exiting orchestrator loop");
                    break;
                }
            }
        }
    }

    /// One iteration of the poll loop.
    pub async fn tick(self: &Arc<Self>) -> Result<(), OrchestratorError> {
        self.audit.record(AuditEvent::new(AuditEventKind::Tick));

        let active = &self.workflow.config.tracker.active_states;
        let candidates = self.tracker.fetch_candidate_issues(active).await?;

        // Snapshot state to filter without holding the lock during the
        // filter pass — we only re-acquire it to mutate `claimed`/`running`.
        let (running, claimed, completed) = {
            let s = self.state.read().await;
            (
                s.running.keys().cloned().collect::<HashSet<_>>(),
                s.claimed.clone(),
                s.completed.clone(),
            )
        };

        let max = self.workflow.config.agent.max_concurrent_agents;
        if running.len() >= max {
            tracing::debug!(
                running = running.len(),
                cap = max,
                "concurrency cap reached; skipping tick"
            );
            return Ok(());
        }

        let label = &self.workflow.config.agent.agent_label_required;
        let label_lc = label.to_lowercase();
        let active_set: HashSet<&str> = active.iter().map(|s| s.as_str()).collect();
        let mut eligible: Vec<Issue> = candidates
            .into_iter()
            .filter(|i| active_set.contains(i.state.as_str()))
            .filter(|i| !running.contains(&i.id))
            .filter(|i| !claimed.contains(&i.id))
            .filter(|i| !completed.contains(&i.id))
            .filter(|i| i.labels.iter().any(|l| l == &label_lc))
            .collect();

        // §8.2 sort: priority asc (lowest number = most urgent), then
        // created_at oldest, then identifier lex. Treat absent priority as
        // "very low" (after every numbered priority). Linear convention is
        // 0=No priority, 1=Urgent, 2=High, 3=Med, 4=Low — we still sort
        // ascending so callers should normalize 0 to a sentinel if desired.
        eligible.sort_by(|a, b| {
            a.priority
                .unwrap_or(i32::MAX)
                .cmp(&b.priority.unwrap_or(i32::MAX))
                .then_with(|| match (a.created_at, b.created_at) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                })
                .then_with(|| a.identifier.cmp(&b.identifier))
        });

        if let Some(issue) = eligible.into_iter().next() {
            self.dispatch(issue).await?;
        }

        Ok(())
    }

    /// Dispatch a single issue: claim, materialize workspace, render prompt,
    /// route to an agent, spawn the streaming task. The streaming half runs
    /// asynchronously; this function returns once the agent task has been
    /// spawned and the issue has been promoted from `claimed` to `running`.
    pub async fn dispatch(self: &Arc<Self>, issue: Issue) -> Result<(), OrchestratorError> {
        // Atomic claim.
        {
            let mut s = self.state.write().await;
            if s.running.contains_key(&issue.id) || s.claimed.contains(&issue.id) {
                return Ok(());
            }
            s.claimed.insert(issue.id.clone());
        }
        self.audit.record(
            AuditEvent::new(AuditEventKind::Claimed)
                .with_issue(issue.id.clone(), issue.identifier.clone()),
        );

        let workspace = match self.workspaces.ensure_for(&issue).await {
            Ok(ws) => ws,
            Err(e) => {
                let mut s = self.state.write().await;
                s.claimed.remove(&issue.id);
                return Err(e.into());
            }
        };

        let prompt = self
            .workflow
            .render_prompt(&issue, None)
            .map_err(|e| OrchestratorError::Other(e.to_string()))?;

        let task = Task {
            id: TaskId::new(),
            role: Role::Worker,
            prompt,
            context: TaskContext {
                cwd: workspace.path.clone(),
                env: HashMap::new(),
                metadata: HashMap::new(),
            },
            budget_hint: None,
        };

        let agent: Arc<dyn Agent> = {
            let agent = self.router.select(&task).await?;
            agent.clone()
        };
        let agent_id = agent.id();

        // Promote claimed → running.
        {
            let mut s = self.state.write().await;
            s.claimed.remove(&issue.id);
            s.running.insert(
                issue.id.clone(),
                RunningEntry {
                    issue_id: issue.id.clone(),
                    identifier: issue.identifier.clone(),
                    workspace_path: workspace.path.clone(),
                    started_at: Instant::now(),
                    agent_id: agent_id.clone(),
                },
            );
        }

        self.audit.record(
            AuditEvent::new(AuditEventKind::Dispatched)
                .with_issue(issue.id.clone(), issue.identifier.clone())
                .with_provider(agent_id.0.clone()),
        );

        // Run before_run hook (fatal on failure).
        if let Err(e) = self.workspaces.run_before_run_hook(&workspace).await {
            self.cleanup_running(&issue.id).await;
            return Err(e.into());
        }

        let this = Arc::clone(self);
        tokio::spawn(async move {
            this.run_agent(issue, workspace, agent, task).await;
        });

        Ok(())
    }

    async fn run_agent(
        self: Arc<Self>,
        issue: Issue,
        workspace: Workspace,
        agent: Arc<dyn Agent>,
        task: Task,
    ) {
        let provider = agent.id().0.clone();
        let stream = match agent.execute(task).await {
            Ok(s) => s,
            Err(e) => {
                self.audit.record(
                    AuditEvent::new(AuditEventKind::Failed)
                        .with_issue(issue.id.clone(), issue.identifier.clone())
                        .with_provider(provider.clone())
                        .with_error(e.to_string()),
                );
                self.workspaces.run_after_run_hook(&workspace).await;
                self.cleanup_running(&issue.id).await;
                return;
            }
        };

        self.consume_stream(stream, &issue, &provider).await;
        self.run_post_steps(&issue, &workspace).await;
    }

    async fn consume_stream(
        &self,
        mut stream: AgentEventStream,
        issue: &Issue,
        provider: &str,
    ) {
        while let Some(ev) = stream.next().await {
            match ev {
                AgentEvent::Started { task_id } => {
                    tracing::info!(?task_id, "agent started");
                }
                AgentEvent::OutputChunk { text } => {
                    self.audit.record(
                        AuditEvent::new(AuditEventKind::Chunk)
                            .with_issue(issue.id.clone(), issue.identifier.clone())
                            .with_provider(provider.to_string())
                            .with_message(truncate(&text, 256)),
                    );
                }
                AgentEvent::ToolCall { name, .. } => {
                    self.audit.record(
                        AuditEvent::new(AuditEventKind::ToolCall)
                            .with_issue(issue.id.clone(), issue.identifier.clone())
                            .with_provider(provider.to_string())
                            .with_message(name),
                    );
                }
                AgentEvent::ToolResult { name, .. } => {
                    self.audit.record(
                        AuditEvent::new(AuditEventKind::ToolResult)
                            .with_issue(issue.id.clone(), issue.identifier.clone())
                            .with_provider(provider.to_string())
                            .with_message(name),
                    );
                }
                AgentEvent::Completed { task_id, summary } => {
                    self.audit.record(
                        AuditEvent::new(AuditEventKind::Completed)
                            .with_issue(issue.id.clone(), issue.identifier.clone())
                            .with_provider(provider.to_string())
                            .with_message(
                                summary.unwrap_or_else(|| format!("task {}", task_id)),
                            ),
                    );
                }
                AgentEvent::Failed { task_id, error } => {
                    self.audit.record(
                        AuditEvent::new(AuditEventKind::Failed)
                            .with_issue(issue.id.clone(), issue.identifier.clone())
                            .with_provider(provider.to_string())
                            .with_error(format!("{} (task {})", error, task_id)),
                    );
                }
            }
        }
    }

    async fn run_post_steps(&self, issue: &Issue, workspace: &Workspace) {
        // Diff guard. TODO(PDX-28): post a Linear comment when exceeded;
        // out of scope for the MVP — for now we just audit and continue.
        match self.diff_guard.check(&workspace.path).await {
            Ok(stat) => {
                tracing::info!(
                    insertions = stat.insertions,
                    deletions = stat.deletions,
                    "diff guard ok"
                );
            }
            Err(e) => {
                self.audit.record(
                    AuditEvent::new(AuditEventKind::DiffGuardExceeded)
                        .with_issue(issue.id.clone(), issue.identifier.clone())
                        .with_error(e.to_string()),
                );
            }
        }

        self.workspaces.run_after_run_hook(workspace).await;

        let mut s = self.state.write().await;
        s.running.remove(&issue.id);
        s.completed.insert(issue.id.clone());
    }

    async fn cleanup_running(&self, issue_id: &str) {
        let mut s = self.state.write().await;
        s.running.remove(issue_id);
        s.claimed.remove(issue_id);
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut t = s[..n].to_string();
        t.push('…');
        t
    }
}
