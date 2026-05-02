//! Task→agent boundary tracking: enforce that an agent cannot be swapped
//! mid-task.
//!
//! The orchestrator's invariant is "switches happen only at task boundaries":
//! once a [`Task`] has been dispatched to an [`Agent`], no other agent may
//! claim the same task until the current execution terminates. This module
//! provides a small synchronous tracker — [`TaskBoundary`] — that enforces
//! that rule independently of the [`Router`] selection logic.
//!
//! # Design
//!
//! The tracker is a thin wrapper around a `HashMap<TaskId, AgentId>` behind
//! a [`std::sync::Mutex`]. A successful [`TaskBoundary::begin`] call inserts
//! the binding and returns a [`BoundaryGuard`] whose [`Drop`] impl removes
//! it. Subsequent attempts to bind the same [`TaskId`] — to the same agent
//! or a different one — fail with [`BoundaryError::AlreadyBound`] until the
//! original guard drops.
//!
//! Synchronous locking is deliberate: bind and release are O(1) and never
//! need to `.await`, and synchronous locking is the only option inside
//! [`Drop`]. Lock poisoning is recovered transparently so a panic in an
//! unrelated code path cannot strand a binding.
//!
//! [`Router`]: crate::router::Router
//! [`Task`]: crate::Task
//! [`Agent`]: crate::Agent

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::{AgentId, TaskId};

/// Tracker that enforces "no mid-task agent switching" by recording the
/// agent currently bound to each in-flight [`TaskId`].
///
/// Cloning a [`TaskBoundary`] yields a new handle to the same underlying
/// state — share clones across the dispatcher and any component that needs
/// to inspect bindings.
#[derive(Debug, Clone, Default)]
pub struct TaskBoundary {
    inner: Arc<Mutex<HashMap<TaskId, AgentId>>>,
}

/// Errors returned by [`TaskBoundary::begin`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    /// The task is already bound to an agent. The existing binding is
    /// preserved unchanged.
    ///
    /// This single variant covers two cases the orchestrator forbids:
    /// 1. A different agent attempting to claim a task already in flight
    ///    (`current != requested`) — the canonical "mid-task switch".
    /// 2. The same agent attempting to re-bind a task it already holds
    ///    (`current == requested`) — almost always a caller bug
    ///    (double-`begin`), worth surfacing rather than silently succeeding.
    #[error(
        "task {task_id} is already bound to agent {current}; cannot bind to {requested} mid-task"
    )]
    AlreadyBound {
        /// The task that is already in flight.
        task_id: TaskId,
        /// The agent currently holding the task.
        current: AgentId,
        /// The agent the caller tried to (re)bind to.
        requested: AgentId,
    },
}

/// RAII guard returned by [`TaskBoundary::begin`].
///
/// While alive, the guard owns the (task, agent) binding inside the parent
/// [`TaskBoundary`]. Dropping the guard removes the binding, freeing the
/// task so a subsequent dispatch can target a different agent — the
/// "switch at task boundary" semantic.
#[must_use = "the binding is released as soon as the guard is dropped"]
pub struct BoundaryGuard {
    inner: Arc<Mutex<HashMap<TaskId, AgentId>>>,
    task_id: TaskId,
    agent_id: AgentId,
}

impl BoundaryGuard {
    /// The task this guard is holding.
    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    /// The agent the guard's task is bound to.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }
}

impl std::fmt::Debug for BoundaryGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundaryGuard")
            .field("task_id", &self.task_id)
            .field("agent_id", &self.agent_id)
            .finish()
    }
}

impl Drop for BoundaryGuard {
    fn drop(&mut self) {
        // Best-effort release. Recover from a poisoned mutex so a panic in
        // some unrelated code path does not strand the binding forever.
        let mut map = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        map.remove(&self.task_id);
    }
}

impl TaskBoundary {
    /// Construct a new, empty [`TaskBoundary`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a new (task, agent) binding.
    ///
    /// Returns a [`BoundaryGuard`] that releases the binding on drop. If the
    /// task is already bound to any agent — including the same one — this
    /// returns [`BoundaryError::AlreadyBound`] without modifying state.
    pub fn begin(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
    ) -> Result<BoundaryGuard, BoundaryError> {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(current) = map.get(&task_id) {
            return Err(BoundaryError::AlreadyBound {
                task_id,
                current: current.clone(),
                requested: agent_id,
            });
        }
        map.insert(task_id, agent_id.clone());
        drop(map);
        Ok(BoundaryGuard {
            inner: self.inner.clone(),
            task_id,
            agent_id,
        })
    }

    /// Look up the agent currently bound to `task_id`, if any.
    pub fn bound_agent(&self, task_id: TaskId) -> Option<AgentId> {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.get(&task_id).cloned()
    }

    /// Number of currently in-flight task bindings.
    pub fn in_flight(&self) -> usize {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}
