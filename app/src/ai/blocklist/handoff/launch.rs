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
pub(crate) enum CloudLaunchEntrypoint {
    Ampersand,
    SlashCommand,
}

#[derive(Debug, Clone)]
pub(crate) struct CloudLaunchRequest {
    id: CloudLaunchRequestId,
    pub(crate) initial_prompt: Option<String>,
    pub(crate) attachments: CloudLaunchAttachments,
    pub(crate) explicit_environment_id: Option<SyncId>,
    #[allow(dead_code)]
    pub(crate) entrypoint: CloudLaunchEntrypoint,
}

impl CloudLaunchRequest {
    pub(crate) fn auto_submit(
        initial_prompt: String,
        attachments: CloudLaunchAttachments,
        explicit_environment_id: Option<SyncId>,
        entrypoint: CloudLaunchEntrypoint,
    ) -> Self {
        Self::new(
            Some(initial_prompt),
            attachments,
            explicit_environment_id,
            entrypoint,
        )
    }

    fn new(
        initial_prompt: Option<String>,
        attachments: CloudLaunchAttachments,
        explicit_environment_id: Option<SyncId>,
        entrypoint: CloudLaunchEntrypoint,
    ) -> Self {
        Self {
            id: CloudLaunchRequestId::new(),
            initial_prompt,
            attachments,
            explicit_environment_id,
            entrypoint,
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
