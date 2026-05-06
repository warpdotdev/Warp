use std::sync::atomic::{AtomicU64, Ordering};

use crate::ai::blocklist::PendingAttachment;
use crate::server::ids::SyncId;
use crate::server::server_api::ai::AttachmentInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CloudLaunchRequestId(u64);

impl CloudLaunchRequestId {
    pub(crate) fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CloudLaunchAttachments {
    pub(crate) request_attachments: Vec<AttachmentInput>,
    pub(crate) display_attachments: Vec<PendingAttachment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloudLaunchSubmitMode {
    Compose,
    AutoSubmit,
}

#[derive(Debug, Clone)]
pub struct CloudLaunchRequest {
    id: CloudLaunchRequestId,
    pub(crate) initial_prompt: Option<String>,
    pub(crate) attachments: CloudLaunchAttachments,
    pub(crate) explicit_environment_id: Option<SyncId>,
    pub(crate) submit_mode: CloudLaunchSubmitMode,
}

impl CloudLaunchRequest {
    pub(crate) fn compose() -> Self {
        Self::new(
            None,
            CloudLaunchAttachments::default(),
            None,
            CloudLaunchSubmitMode::Compose,
        )
    }

    pub(crate) fn auto_submit(
        initial_prompt: String,
        attachments: CloudLaunchAttachments,
        explicit_environment_id: Option<SyncId>,
    ) -> Self {
        Self::new(
            Some(initial_prompt),
            attachments,
            explicit_environment_id,
            CloudLaunchSubmitMode::AutoSubmit,
        )
    }

    pub(crate) fn with_attachments(mut self, attachments: CloudLaunchAttachments) -> Self {
        self.attachments = attachments;
        self
    }

    fn new(
        initial_prompt: Option<String>,
        attachments: CloudLaunchAttachments,
        explicit_environment_id: Option<SyncId>,
        submit_mode: CloudLaunchSubmitMode,
    ) -> Self {
        Self {
            id: CloudLaunchRequestId::new(),
            initial_prompt,
            attachments,
            explicit_environment_id,
            submit_mode,
        }
    }

    pub(crate) fn id(&self) -> CloudLaunchRequestId {
        self.id
    }

    pub(crate) fn prompt(&self) -> Option<&str> {
        self.initial_prompt.as_deref()
    }
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
