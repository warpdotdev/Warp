use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentPolicyDecisionKind {
    Allow,
    Deny,
    Ask,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentPolicyUnavailableDecision {
    Allow,
    Deny,
    #[default]
    Ask,
    #[serde(other)]
    Unknown,
}

impl AgentPolicyUnavailableDecision {
    pub(crate) fn decision_kind(self) -> AgentPolicyDecisionKind {
        match self {
            Self::Allow => AgentPolicyDecisionKind::Allow,
            Self::Deny => AgentPolicyDecisionKind::Deny,
            Self::Ask | Self::Unknown => AgentPolicyDecisionKind::Ask,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentPolicyHookResponse {
    pub schema_version: String,
    pub decision: AgentPolicyDecisionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_audit_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WarpPermissionDecisionKind {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WarpPermissionSnapshot {
    pub decision: WarpPermissionDecisionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl WarpPermissionSnapshot {
    pub(crate) fn allow(reason: Option<String>) -> Self {
        Self {
            decision: WarpPermissionDecisionKind::Allow,
            reason,
        }
    }

    pub(crate) fn ask(reason: Option<String>) -> Self {
        Self {
            decision: WarpPermissionDecisionKind::Ask,
            reason,
        }
    }

    pub(crate) fn deny(reason: Option<String>) -> Self {
        Self {
            decision: WarpPermissionDecisionKind::Deny,
            reason,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentPolicyHookErrorKind {
    InvalidConfiguration,
    Timeout,
    SpawnFailed,
    StdinWriteFailed,
    NonZeroExit,
    MalformedResponse,
    UnsupportedTransport,
    HttpRequestFailed,
    HttpStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentPolicyHookEvaluation {
    pub hook_name: String,
    pub decision: AgentPolicyDecisionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_audit_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AgentPolicyHookErrorKind>,
}

impl AgentPolicyHookEvaluation {
    pub(crate) fn from_response(
        hook_name: impl Into<String>,
        response: AgentPolicyHookResponse,
    ) -> Self {
        Self {
            hook_name: hook_name.into(),
            decision: response.decision,
            reason: response.reason,
            external_audit_id: response.external_audit_id,
            error: None,
        }
    }

    pub(crate) fn unavailable(
        hook_name: impl Into<String>,
        decision: AgentPolicyDecisionKind,
        error: AgentPolicyHookErrorKind,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            hook_name: hook_name.into(),
            decision,
            reason: Some(reason.into()),
            external_audit_id: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentPolicyEffectiveDecision {
    pub decision: AgentPolicyDecisionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub warp_permission: WarpPermissionSnapshot,
    #[serde(default)]
    pub hook_results: Vec<AgentPolicyHookEvaluation>,
}

pub(crate) fn compose_policy_decisions(
    warp_permission: WarpPermissionSnapshot,
    hook_results: Vec<AgentPolicyHookEvaluation>,
    allow_hook_autoapproval: bool,
) -> AgentPolicyEffectiveDecision {
    let first_denial = hook_results
        .iter()
        .find(|result| result.decision == AgentPolicyDecisionKind::Deny);
    if let Some(denial) = first_denial {
        return AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Deny,
            reason: denial.reason.clone(),
            warp_permission,
            hook_results,
        };
    }

    if warp_permission.decision == WarpPermissionDecisionKind::Deny {
        return AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Deny,
            reason: warp_permission.reason.clone(),
            warp_permission,
            hook_results,
        };
    }

    let first_ask = hook_results
        .iter()
        .find(|result| result.decision == AgentPolicyDecisionKind::Ask);
    if let Some(ask) = first_ask {
        return AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Ask,
            reason: ask.reason.clone(),
            warp_permission,
            hook_results,
        };
    }

    match warp_permission.decision {
        WarpPermissionDecisionKind::Allow => AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Allow,
            reason: warp_permission.reason.clone(),
            warp_permission,
            hook_results,
        },
        WarpPermissionDecisionKind::Ask if allow_hook_autoapproval && !hook_results.is_empty() => {
            AgentPolicyEffectiveDecision {
                decision: AgentPolicyDecisionKind::Allow,
                reason: hook_results.iter().find_map(|result| result.reason.clone()),
                warp_permission,
                hook_results,
            }
        }
        WarpPermissionDecisionKind::Ask => AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Ask,
            reason: warp_permission.reason.clone(),
            warp_permission,
            hook_results,
        },
        WarpPermissionDecisionKind::Deny => unreachable!("warp deny is handled before this match"),
    }
}
