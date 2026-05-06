use crate::ai::blocklist::PendingAttachment;
use crate::server::ids::SyncId;
use crate::server::server_api::ai::AttachmentInput;
use crate::terminal::input::handoff_compose::HandoffLaunchRequestId;

#[derive(Debug, Clone, Default)]
pub(crate) struct HandoffLaunchAttachments {
    pub(crate) request_attachments: Vec<AttachmentInput>,
    pub(crate) display_attachments: Vec<PendingAttachment>,
}

#[derive(Debug, Clone)]
pub(crate) struct HandoffLaunchRequest {
    id: HandoffLaunchRequestId,
    pub(crate) initial_prompt: Option<String>,
    pub(crate) attachments: HandoffLaunchAttachments,
    pub(crate) explicit_environment_id: Option<SyncId>,
}

impl HandoffLaunchRequest {
    pub(crate) fn auto_submit(
        initial_prompt: String,
        attachments: HandoffLaunchAttachments,
        explicit_environment_id: Option<SyncId>,
    ) -> Self {
        Self {
            id: HandoffLaunchRequestId::new(),
            initial_prompt: Some(initial_prompt),
            attachments,
            explicit_environment_id,
        }
    }

    pub(crate) fn id(&self) -> HandoffLaunchRequestId {
        self.id
    }
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
