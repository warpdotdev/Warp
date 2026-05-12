//! Shared singleton holding which child-agent conversations are currently
//! pinned in the orchestration pill bar. Lives outside the per-view pill
//! bar so a pin in one pane is reflected in every other pane.

use std::collections::HashSet;

use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::blocklist::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};

/// Toggle `id` in `pinned` — insert when absent, remove when present.
/// Extracted so toggle semantics can be unit tested without a singleton.
pub(super) fn toggle_pin_in_set(pinned: &mut HashSet<AIConversationId>, id: AIConversationId) {
    if !pinned.remove(&id) {
        pinned.insert(id);
    }
}

/// Singleton owning the set of currently-pinned child conversation ids.
pub struct OrchestrationPinModel {
    pinned: HashSet<AIConversationId>,
}

impl OrchestrationPinModel {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        // Prune deleted conversations globally so per-pane code never
        // has to (and can't accidentally clobber sibling panes' pins).
        let history_handle = BlocklistAIHistoryModel::handle(ctx);
        ctx.subscribe_to_model(&history_handle, |this, event, ctx| match event {
            BlocklistAIHistoryEvent::RemoveConversation {
                conversation_id, ..
            }
            | BlocklistAIHistoryEvent::DeletedConversation {
                conversation_id, ..
            } => {
                if this.pinned.remove(conversation_id) {
                    ctx.emit(OrchestrationPinEvent::PinSetChanged);
                }
            }
            _ => {}
        });

        Self {
            pinned: HashSet::new(),
        }
    }

    /// Returns `true` if the given conversation id is currently pinned.
    pub fn is_pinned(&self, conversation_id: &AIConversationId) -> bool {
        self.pinned.contains(conversation_id)
    }

    /// Toggle whether `conversation_id` is pinned and notify subscribers.
    pub fn toggle_pin(&mut self, conversation_id: AIConversationId, ctx: &mut ModelContext<Self>) {
        toggle_pin_in_set(&mut self.pinned, conversation_id);
        ctx.emit(OrchestrationPinEvent::PinSetChanged);
    }
}

impl Entity for OrchestrationPinModel {
    type Event = OrchestrationPinEvent;
}

impl SingletonEntity for OrchestrationPinModel {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrchestrationPinEvent {
    /// The pinned set changed (toggle or history-driven prune).
    PinSetChanged,
}

#[cfg(test)]
#[path = "orchestration_pin_model_tests.rs"]
mod tests;
