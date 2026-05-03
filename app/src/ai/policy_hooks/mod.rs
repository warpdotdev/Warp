#[cfg(not(target_family = "wasm"))]
mod audit;
pub(crate) mod config;
pub(crate) mod decision;
#[cfg(not(target_family = "wasm"))]
pub(crate) mod engine;
pub(crate) mod event;
mod redaction;

pub(crate) use config::AgentPolicyHookConfig;
pub(crate) use decision::{
    AgentPolicyDecisionKind, AgentPolicyEffectiveDecision, WarpPermissionSnapshot,
};
#[cfg(not(target_family = "wasm"))]
pub(crate) use engine::AgentPolicyHookEngine;
pub(crate) use event::{
    AgentPolicyAction, AgentPolicyEvent, PolicyCallMcpToolAction, PolicyDiffStats,
    PolicyExecuteCommandAction, PolicyReadFilesAction, PolicyReadMcpResourceAction,
    PolicyWriteFilesAction,
};

#[cfg(test)]
mod tests;
