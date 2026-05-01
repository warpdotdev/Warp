//! Capability-, health-, and budget-aware [`Agent`] selection.
//!
//! The [`Router`] is the orchestrator's policy gate: every dispatch goes
//! through [`Router::select`], which returns the highest-ranked [`Agent`]
//! that can fulfil a [`Task`]'s [`Role`], reports itself healthy, and is not
//! blocked by the current per-provider [`BudgetTier`].
//!
//! Selection is intentionally pure and deterministic — calling [`Router::select`]
//! repeatedly with the same registered agents and the same task returns the
//! same [`AgentId`] every time. This makes routing decisions trivial to log,
//! replay, and reason about, and lets higher layers cache choices safely.
//!
//! # Selection algorithm
//!
//! For a given [`Task`]:
//!
//! 1. Filter to agents whose advertised capabilities include the task's
//!    [`Role`]. If none match, return [`RouterError::NoCapableAgent`].
//! 2. Filter to agents whose [`Health::healthy`] flag is `true`. If none
//!    survive, return [`RouterError::AllUnhealthy`].
//! 3. Look up each surviving agent's current [`BudgetTier`] via
//!    [`Budget::current_tier`].
//! 4. Apply the tier-aware filter:
//!    * [`BudgetTier::Halted`]   — exclude (never dispatch to a halted provider).
//!    * [`BudgetTier::Critical`] — only [`Role::Planner`] and [`Role::Reviewer`]
//!      tasks are allowed; everything else is excluded.
//!    * [`BudgetTier::Warning`]  — allowed, but the sort key biases towards
//!      cheaper agents.
//!    * [`BudgetTier::Healthy`]  — allowed unconditionally.
//!
//!    If the filter empties the set, distinguish two cases for clearer
//!    diagnostics:
//!    * a candidate was killed because its tier was [`BudgetTier::Critical`]
//!      and the task role was not Planner/Reviewer
//!      → [`RouterError::NoFallbackForTier`];
//!    * otherwise → [`RouterError::BudgetHalted`].
//! 5. Sort surviving agents by `(tier, estimated_micros_per_task, agent.id())`
//!    ascending. The lexicographic [`AgentId`] tiebreaker guarantees
//!    determinism.
//! 6. Return the first surviving agent.

use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;

use crate::{Agent, AgentId, Budget, BudgetError, BudgetTier, Provider, Role, Task};

/// Registration record for a single [`Agent`] instance.
///
/// The router keeps these by [`AgentId`] in an internal registry; the
/// `provider` field tells the [`Budget`] which billing bucket the agent
/// charges against, and `estimated_micros_per_task` gives the sort key used
/// to break ties when multiple capable agents are eligible.
pub struct AgentRegistration {
    /// Shared handle to the agent implementation.
    pub agent: Arc<dyn Agent>,
    /// Billing bucket this agent's spend lands in.
    pub provider: Provider,
    /// Caller-supplied cost estimate, in micro-dollars per task. Used as a
    /// secondary sort key — lower is preferred under [`BudgetTier::Warning`].
    pub estimated_micros_per_task: u64,
}

/// Selects an [`Agent`] for a [`Task`] based on capability, health, and
/// per-provider [`BudgetTier`].
///
/// See the module-level docs for the full selection algorithm. The router
/// owns no mutable state of its own beyond the registry; the [`Budget`] it
/// holds is shared via [`Arc`] with the rest of the orchestrator.
pub struct Router {
    agents: HashMap<AgentId, AgentRegistration>,
    budget: Arc<Budget>,
}

/// Errors returned by [`Router::select`].
#[derive(Debug, Error)]
pub enum RouterError {
    /// No registered agent advertises the task's [`Role`].
    #[error("no Agent supports role {0:?}")]
    NoCapableAgent(Role),
    /// At least one capable agent exists, but all are reporting unhealthy.
    #[error("all capable agents are unhealthy")]
    AllUnhealthy,
    /// All capable, healthy agents are routed to providers that are halted
    /// for budget reasons.
    #[error("budget halted for all capable providers")]
    BudgetHalted,
    /// All capable, healthy agents are on providers in
    /// [`BudgetTier::Critical`], and the task's role is not one of the
    /// allow-listed Planner/Reviewer roles.
    #[error("budget tier requires fallback agent for role {0:?}; none available")]
    NoFallbackForTier(Role),
    /// A budget lookup failed (typically because a registered agent uses an
    /// unknown [`Provider`]).
    #[error("budget error: {0}")]
    Budget(#[from] BudgetError),
}

impl Router {
    /// Construct a new [`Router`] with an empty agent registry, sharing the
    /// supplied [`Budget`] with the rest of the orchestrator.
    pub fn new(budget: Arc<Budget>) -> Self {
        Self {
            agents: HashMap::new(),
            budget,
        }
    }

    /// Add an agent to the registry. If an agent with the same [`AgentId`]
    /// is already registered it is replaced.
    pub fn register(&mut self, registration: AgentRegistration) {
        let id = registration.agent.id();
        self.agents.insert(id, registration);
    }

    /// Choose the best [`Agent`] for `task` per the algorithm in the module
    /// docs.
    ///
    /// Returns a borrowed [`Arc`] to the selected agent. The borrow is tied
    /// to `&self` so callers can clone the [`Arc`] cheaply if they need to
    /// hand it off across awaits.
    pub async fn select(&self, task: &Task) -> Result<&Arc<dyn Agent>, RouterError> {
        // Step 1: capability filter.
        let capable: Vec<&AgentRegistration> = self
            .agents
            .values()
            .filter(|reg| reg.agent.capabilities().roles.contains(&task.role))
            .collect();
        if capable.is_empty() {
            return Err(RouterError::NoCapableAgent(task.role));
        }

        // Step 2: health filter.
        let healthy: Vec<&AgentRegistration> = capable
            .into_iter()
            .filter(|reg| reg.agent.health().healthy)
            .collect();
        if healthy.is_empty() {
            return Err(RouterError::AllUnhealthy);
        }

        // Step 3 + 4: tier lookup + tier-aware filter.
        //
        // We track *why* each candidate was dropped so we can produce a
        // useful error if the filter empties the set: a Critical-tier kill
        // for a non-allow-listed role is qualitatively different from a
        // Halted-tier kill, and the dispatcher reacts differently to each.
        let tier_allows_role = |tier: BudgetTier, role: Role| -> bool {
            match tier {
                BudgetTier::Halted => false,
                BudgetTier::Critical => matches!(role, Role::Planner | Role::Reviewer),
                BudgetTier::Warning | BudgetTier::Healthy => true,
            }
        };

        let mut survivors: Vec<(BudgetTier, &AgentRegistration)> = Vec::with_capacity(healthy.len());
        let mut saw_critical_kill = false;
        for reg in healthy {
            let tier = self.budget.current_tier(reg.provider).await?;
            if tier_allows_role(tier, task.role) {
                survivors.push((tier, reg));
            } else if matches!(tier, BudgetTier::Critical) {
                // Critical tier excluded a non-Planner/Reviewer task — the
                // caller may want to fall back to a different agent class
                // entirely, so surface that distinctly.
                saw_critical_kill = true;
            }
        }

        if survivors.is_empty() {
            return Err(if saw_critical_kill {
                RouterError::NoFallbackForTier(task.role)
            } else {
                RouterError::BudgetHalted
            });
        }

        // Step 5: deterministic sort. Tier is ranked Healthy < Warning <
        // Critical < Halted (Halted never appears here, but the ordering is
        // total). Cost ascending and AgentId lexicographic round out the key.
        survivors.sort_by(|a, b| {
            tier_rank(a.0)
                .cmp(&tier_rank(b.0))
                .then_with(|| a.1.estimated_micros_per_task.cmp(&b.1.estimated_micros_per_task))
                .then_with(|| a.1.agent.id().0.cmp(&b.1.agent.id().0))
        });

        // Step 6: return the head. `survivors` is non-empty by the check
        // above, so `unwrap` is sound; using indexing keeps the lifetime tied
        // to `&self` rather than to the local `Vec`.
        let chosen_id = survivors[0].1.agent.id();
        Ok(&self.agents.get(&chosen_id).expect("registered above").agent)
    }
}

/// Total order over [`BudgetTier`] used for sorting candidate agents.
///
/// Lower is preferred. Halted is included for completeness even though the
/// router never sorts a Halted-tier agent (they are dropped in step 4).
fn tier_rank(tier: BudgetTier) -> u8 {
    match tier {
        BudgetTier::Healthy => 0,
        BudgetTier::Warning => 1,
        BudgetTier::Critical => 2,
        BudgetTier::Halted => 3,
    }
}
