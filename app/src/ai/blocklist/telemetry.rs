use crate::ai::agent::conversation::AIConversationId;
use serde::Serialize;

#[derive(Debug)]
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
