use crate::ai::agent::conversation::AIConversationId;
use serde::Serialize;
use serde_json::{json, Value};
use strum_macros::{EnumDiscriminants, EnumIter};
use warp_core::telemetry::{EnablementState, TelemetryEvent, TelemetryEventDesc};

#[derive(Debug, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter))]
pub(crate) enum BlocklistOrchestrationTelemetryEvent {
    TeamAgentCommunicationFailed(TeamAgentCommunicationFailedEvent),
    PlanConfigApprovalToggled(PlanConfigApprovalToggledEvent),
    RunAgentsCardDecision(RunAgentsCardDecisionEvent),
    PillBarInteraction(PillBarInteractionEvent),
    OrchestrationEntered(OrchestrationEnteredEvent),
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TeamAgentCommunicationKind {
    Message,
    LifecycleEvent,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TeamAgentCommunicationTransport {
    Local,
    ServerApi,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TeamAgentOrchestrationVersion {
    V1,
    V2,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TeamAgentCommunicationFailureReason {
    InvalidLifecycleEventType,
    MissingSourceConversation,
    MissingSourceIdentifier,
    UnknownAgent,
    NoTargets,
    RequestFailed,
}

#[derive(Debug, Serialize)]
pub(crate) struct TeamAgentCommunicationFailedEvent {
    pub communication_kind: TeamAgentCommunicationKind,
    pub transport: TeamAgentCommunicationTransport,
    pub orchestration_version: TeamAgentOrchestrationVersion,
    pub failure_reason: TeamAgentCommunicationFailureReason,
    pub source_conversation_id: AIConversationId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_event_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Coarse approval transition for the plan card's
/// `Use orchestration` toggle.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationApprovalStatus {
    Approved,
    Disapproved,
}

/// Run-wide execution mode reported on telemetry payloads. Mirrors
/// `RunAgentsExecutionMode` but is a flat enum so the payload stays
/// metadata-only and never carries an environment id or worker host.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationExecutionModeKind {
    Local,
    Remote,
}

impl OrchestrationExecutionModeKind {
    pub(crate) fn from_run_agents(mode: &ai::agent::action::RunAgentsExecutionMode) -> Self {
        if mode.is_remote() {
            Self::Remote
        } else {
            Self::Local
        }
    }
}

/// Stable bucket for the run-wide harness selection. Maps a raw harness
/// string (which is server-/catalog-controlled but may include strings
/// we don't yet recognize on the client) onto a closed set so the
/// analytics column stays low-cardinality.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationHarnessKind {
    Oz,
    ClaudeCode,
    Codex,
    OpenCode,
    Gemini,
    Unknown,
}

impl OrchestrationHarnessKind {
    pub(crate) fn from_str(harness_type: &str) -> Self {
        // Match the canonical strings used in `OrchestrationEditState`
        // and the orchestration config proto. Anything else collapses
        // to `Unknown` so the analytics column stays bounded even if
        // the server adds a new harness before the client catalogs it.
        match harness_type {
            "oz" | "" => Self::Oz,
            "claude" | "claude-code" | "claude_code" => Self::ClaudeCode,
            "codex" => Self::Codex,
            "opencode" | "open-code" | "open_code" => Self::OpenCode,
            "gemini" => Self::Gemini,
            _ => Self::Unknown,
        }
    }
}

/// Stable names for the run-wide config fields that can diverge between
/// the dispatched orchestration request and either the original tool
/// call payload or an active approved orchestration config. Mirrors the
/// server's `RunAgentsModifiedField*` constants so the two telemetry
/// streams can be joined on field name.
pub(crate) mod orchestration_modified_field {
    pub const MODEL_ID: &str = "model_id";
    pub const HARNESS: &str = "harness";
    pub const EXECUTION_MODE: &str = "execution_mode";
    pub const ENVIRONMENT_ID: &str = "environment_id";
    pub const WORKER_HOST: &str = "worker_host";
    pub const AUTH_SECRET: &str = "auth_secret";
}

#[derive(Debug, Serialize)]
pub(crate) struct PlanConfigApprovalToggledEvent {
    pub conversation_id: AIConversationId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    pub previous_status: OrchestrationApprovalStatus,
    pub new_status: OrchestrationApprovalStatus,
    pub execution_mode: OrchestrationExecutionModeKind,
    pub harness: OrchestrationHarnessKind,
    pub has_model: bool,
    pub has_environment: bool,
    pub has_worker_host: bool,
    pub has_auth_secret: bool,
}

/// Decision a user took on the run_agents confirmation card.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunAgentsCardDecision {
    Accept,
    AcceptWithoutOrchestration,
    Reject,
}

#[derive(Debug, Serialize)]
pub(crate) struct RunAgentsCardDecisionEvent {
    pub conversation_id: AIConversationId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    pub decision: RunAgentsCardDecision,
    pub agent_count: usize,
    pub harness: OrchestrationHarnessKind,
    pub execution_mode: OrchestrationExecutionModeKind,
    /// Names of run-wide config fields where the dispatched request
    /// diverged from the original `RunAgentsRequest` emitted by the LLM.
    /// Values are drawn from `orchestration_modified_field` so this can
    /// be joined against the server-side `RunAgentsOutcome.modified_fields`.
    /// Empty when the user accepted the tool call without edits.
    pub modified_fields_from_tool_call: Vec<&'static str>,
    /// Same shape as `modified_fields_from_tool_call`, but compared
    /// against the approved orchestration config snapshot (when one
    /// exists). Empty when no active approved config exists or when
    /// the dispatched request matches it exactly.
    pub modified_fields_from_active_config: Vec<&'static str>,
    pub had_active_config: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_config_status: Option<OrchestrationApprovalStatus>,
}

/// Surface that first introduced orchestration into a conversation.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationEntrySource {
    /// User typed `/orchestrate` as the active query mode.
    SlashCommandOrchestrate,
    /// User toggled the `Use orchestration` switch on the plan card to
    /// the approved state.
    PlanCardApproved,
    /// The LLM emitted a `run_agents` tool call that surfaced the
    /// confirmation card (i.e. did not auto-launch).
    RunAgentsCardShown,
}

#[derive(Debug, Serialize)]
pub(crate) struct OrchestrationEnteredEvent {
    pub conversation_id: AIConversationId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    pub entry_source: OrchestrationEntrySource,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PillBarPillKind {
    Orchestrator,
    Child,
}

/// Concrete user actions against an orchestration pill bar entry.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PillBarActionKind {
    Switch,
    OpenInNewPane,
    OpenInNewTab,
    FocusOpenedConversation,
    Stop,
    Kill,
    TogglePinOn,
    TogglePinOff,
    OpenMenu,
}

#[derive(Debug, Serialize)]
pub(crate) struct PillBarInteractionEvent {
    pub action: PillBarActionKind,
    pub pill_kind: PillBarPillKind,
    pub total_pills: usize,
    pub total_pinned: usize,
    /// The conversation that hosts the pill bar (the orchestrator's
    /// conversation in the active pane).
    pub source_conversation_id: AIConversationId,
    /// The conversation that the action targets (the pill the user clicked).
    pub target_conversation_id: AIConversationId,
}

impl TelemetryEvent for BlocklistOrchestrationTelemetryEvent {
    fn name(&self) -> &'static str {
        BlocklistOrchestrationTelemetryEventDiscriminants::from(self).name()
    }

    fn payload(&self) -> Option<Value> {
        match self {
            Self::TeamAgentCommunicationFailed(event) => Some(json!(event)),
            Self::PlanConfigApprovalToggled(event) => Some(json!(event)),
            Self::RunAgentsCardDecision(event) => Some(json!(event)),
            Self::PillBarInteraction(event) => Some(json!(event)),
            Self::OrchestrationEntered(event) => Some(json!(event)),
        }
    }

    fn description(&self) -> &'static str {
        BlocklistOrchestrationTelemetryEventDiscriminants::from(self).description()
    }

    fn enablement_state(&self) -> EnablementState {
        BlocklistOrchestrationTelemetryEventDiscriminants::from(self).enablement_state()
    }

    fn contains_ugc(&self) -> bool {
        false
    }

    fn event_descs() -> impl Iterator<Item = Box<dyn TelemetryEventDesc>> {
        warp_core::telemetry::enum_events::<Self>()
    }
}

impl TelemetryEventDesc for BlocklistOrchestrationTelemetryEventDiscriminants {
    fn name(&self) -> &'static str {
        match self {
            Self::TeamAgentCommunicationFailed => {
                "AgentMode.Orchestration.TeamAgentCommunicationFailed"
            }
            Self::PlanConfigApprovalToggled => "AgentMode.Orchestration.PlanConfigApprovalToggled",
            Self::RunAgentsCardDecision => "AgentMode.Orchestration.RunAgentsCardDecision",
            Self::PillBarInteraction => "AgentMode.Orchestration.PillBarInteraction",
            Self::OrchestrationEntered => "AgentMode.Orchestration.Entered",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::TeamAgentCommunicationFailed => {
                "Failed to send an orchestration message or lifecycle event for a TeamAgent"
            }
            Self::PlanConfigApprovalToggled => {
                "User toggled the Use orchestration switch on a plan card"
            }
            Self::RunAgentsCardDecision => {
                "User accepted, accepted-without-orchestration, or rejected a run_agents confirmation card. Reports which config fields diverged from the original tool call and/or the active approved config."
            }
            Self::PillBarInteraction => {
                "User interacted with the orchestration pill bar (switch, pin, open in pane/tab, stop, kill, etc.)"
            }
            Self::OrchestrationEntered => {
                "Orchestration was activated in a conversation via /orchestrate, a plan-card approval toggle, or a run_agents confirmation card surfacing"
            }
        }
    }

    fn enablement_state(&self) -> EnablementState {
        EnablementState::Always
    }
}

warp_core::register_telemetry_event!(BlocklistOrchestrationTelemetryEvent);
