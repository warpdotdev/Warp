use crate::ai::agent::conversation::{AIConversation, ConversationStatus};
use crate::ai::agent::{
    AIAgentOutputStatus, CancellationReason, FinishedAIAgentOutput, RenderableAIError,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;
use uuid::{NonNilUuid, Uuid};

pub mod github_auth_notifier;
pub mod spawn;
pub mod task;

pub use task::{
    AgentConfigSnapshot, AgentSource, AmbientAgentTask, AmbientAgentTaskState, AttachmentInput,
    TaskStatusMessage,
};
pub const OUT_OF_CREDITS_TASK_FAILURE_MESSAGE: &str =
    "Agent usage limit reached. Please try again later.";
pub const SERVER_OVERLOADED_TASK_FAILURE_MESSAGE: &str =
    "Warp is temporarily overloaded. Please try again shortly.";

/// JSON payload for starting an agent run. In OpenWarp this is only used by local UI/CLI
/// plumbing; no remote run endpoint is contacted.
#[derive(Debug, Clone, Serialize)]
pub struct SpawnAgentRequest {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<AgentConfigSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<bool>,
    /// Use a Claude-compatible skill as the base prompt.
    /// Format: "repo:skill_name" or just "skill_name".
    pub skill: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AttachmentInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
    /// Populated when an agent spawns a child run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    /// Base64-encoded `warp.multi_agent.v1.Skill` payloads to restore as runtime skills.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub runtime_skills: Vec<String>,
    /// Base64-encoded `warp.multi_agent.v1.Attachment` payloads to restore as referenced attachments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub referenced_attachments: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid task ID: {0}")]
pub struct ParseAmbientAgentTaskIdError(#[from] uuid::Error);

/// A globally unique ID for an ambient agent task.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AmbientAgentTaskId(NonNilUuid);

impl Display for AmbientAgentTaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for AmbientAgentTaskId {
    type Err = ParseAmbientAgentTaskIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::try_parse(s)?;
        Ok(Self(NonNilUuid::try_from(uuid)?))
    }
}

impl AmbientAgentTaskId {
    /// OpenWarp(本地化,Phase 3b-4):本地生成一个 UUID v4 作为 task_id,避免本地
    /// harness 启动子 task 时依赖远端预创建任务接口。
    pub fn new_local() -> Self {
        let uuid = Uuid::new_v4();
        // UUID v4 几乎不可能产生 nil(概率 ~ 1/2^122),采用 expect 表示逻辑不可达。
        let non_nil =
            NonNilUuid::try_from(uuid).expect("freshly generated UUID v4 must be non-nil");
        Self(non_nil)
    }
}

/// High-level outcome of an ambient agent conversation.
#[derive(Clone, Debug)]
pub enum AmbientConversationStatus {
    Success,
    Error {
        error: RenderableAIError,
    },
    #[allow(dead_code)]
    Cancelled {
        reason: CancellationReason,
    },
    #[allow(dead_code)]
    Blocked {
        blocked_action: String,
    },
}

/// Derive an [`AmbientConversationStatus`] from the given conversation, if it has
/// reached a terminal state that we care about for ambient agents.
pub fn conversation_output_status_from_conversation(
    conversation: &AIConversation,
) -> Option<AmbientConversationStatus> {
    if let ConversationStatus::Blocked { blocked_action } = conversation.status() {
        return Some(AmbientConversationStatus::Blocked {
            blocked_action: blocked_action.clone(),
        });
    }

    let last_exchange = conversation.root_task_exchanges().last()?;
    if let AIAgentOutputStatus::Finished { finished_output } = &last_exchange.output_status {
        let status = match finished_output {
            FinishedAIAgentOutput::Cancelled { output: _, reason } => {
                AmbientConversationStatus::Cancelled { reason: *reason }
            }
            FinishedAIAgentOutput::Error { output: _, error } => AmbientConversationStatus::Error {
                error: error.clone(),
            },
            FinishedAIAgentOutput::Success { output: _ } => AmbientConversationStatus::Success,
        };
        return Some(status);
    }

    None
}
