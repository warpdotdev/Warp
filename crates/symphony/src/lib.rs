//! Symphony — Linear-driven coding agent orchestrator (Helm MVP).
//!
//! Polls a Linear-compatible tracker, claims active issues, materializes
//! per-issue workspaces, dispatches the work to a registered
//! `orchestrator::Agent`, and streams events back into an audit log.
//!
//! This MVP is intentionally minimal. Stall detection, retry/backoff,
//! reconciliation, live workflow reload, and the dynamic `linear_graphql`
//! tool are out of scope; they live as follow-up tickets in the Helm
//! Symphony epic. See `docs/symphony/README.md` for the divergences from
//! the upstream Symphony spec.

#![deny(missing_docs)]

pub mod audit;
pub mod diff_guard;
pub mod orchestrator;
pub mod tracker;
pub mod workflow;
pub mod workspace;

pub use audit::{AuditEvent, AuditLog};
pub use diff_guard::{DiffGuard, DiffGuardError, DiffStat};
pub use orchestrator::{Orchestrator, OrchestratorError};
pub use tracker::{BlockerRef, Issue, LinearClient, TrackerError};
pub use workflow::{
    AgentConfig, HooksConfig, PollingConfig, TrackerConfig, WorkflowConfig, WorkflowDefinition,
    WorkflowError, WorkspaceConfig,
};
pub use workspace::{Workspace, WorkspaceError, WorkspaceManager};
