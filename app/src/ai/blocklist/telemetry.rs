use crate::ai::agent::conversation::AIConversationId;
use serde::Serialize;
use serde_json::{json, Value};
use strum_macros::{EnumDiscriminants, EnumIter};
use warp_core::telemetry::{EnablementState, TelemetryEvent, TelemetryEventDesc};

#[derive(Debug, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter))]
pub(crate) enum BlocklistOrchestrationTelemetryEvent {
    TeamAgentCommunicationFailed(TeamAgentCommunicationFailedEvent),
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

impl TelemetryEvent for BlocklistOrchestrationTelemetryEvent {
    fn name(&self) -> &'static str {
        BlocklistOrchestrationTelemetryEventDiscriminants::from(self).name()
    }

    fn payload(&self) -> Option<Value> {
        match self {
            Self::TeamAgentCommunicationFailed(event) => Some(json!(event)),
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
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::TeamAgentCommunicationFailed => {
                "Failed to send an orchestration message or lifecycle event for a TeamAgent"
            }
        }
    }

    fn enablement_state(&self) -> EnablementState {
        EnablementState::Always
    }
}

warp_core::register_telemetry_event!(BlocklistOrchestrationTelemetryEvent);
