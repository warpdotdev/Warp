//! Shared singleton holding which child-agent conversations are currently
//! pinned in the orchestration pill bar. Backed by `AgentConversationData.pinned`
//! so pins persist across app restarts and stay consistent across panes.

use std::collections::HashSet;

use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::blocklist::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};

/// Toggle `id` in `pinned` — insert when absent, remove when present.
/// Returns the new membership state (`true` = now pinned).
/// Extracted so toggle semantics can be unit tested without a singleton.
pub(super) fn toggle_pin_in_set(
    pinned: &mut HashSet<AIConversationId>,
    id: AIConversationId,
) -> bool {
    if pinned.remove(&id) {
        false
    } else {
        pinned.insert(id);
        true
    }
}

/// Singleton owning the in-memory cache of pinned child conversation ids.
/// The source of truth lives on each `AIConversation` (persisted via
/// `AgentConversationData.pinned`); this set mirrors it for fast lookups
/// and for cross-pane event notification.
pub struct OrchestrationPinModel {
    pinned: HashSet<AIConversationId>,
}

impl OrchestrationPinModel {
    /// Construct the singleton seeded with the set of conversation ids that
    /// were already pinned in persisted storage at startup.
    pub fn new(initial_pinned: HashSet<AIConversationId>, ctx: &mut ModelContext<Self>) -> Self {
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
            pinned: initial_pinned,
        }
    }

    /// Returns `true` if the given conversation id is currently pinned.
    pub fn is_pinned(&self, conversation_id: &AIConversationId) -> bool {
        self.pinned.contains(conversation_id)
    }

    /// Toggle whether `conversation_id` is pinned, persist the new state to
    /// the underlying conversation, and notify subscribers.
    pub fn toggle_pin(&mut self, conversation_id: AIConversationId, ctx: &mut ModelContext<Self>) {
        let now_pinned = toggle_pin_in_set(&mut self.pinned, conversation_id);
        // Push the change down to the conversation so the next session
        // restore reflects it. Doing this even if the conversation isn't
        // yet loaded is fine — the in-memory set is the active source
        // until that conversation rehydrates.
        BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
            history.set_conversation_pinned(conversation_id, now_pinned, ctx);
        });
        ctx.emit(OrchestrationPinEvent::PinSetChanged);
    }

    /// Clears the in-memory pinned set. Invoked from `log_out` so the next
    /// user does not inherit the previous account's pins. The persisted
    /// per-conversation `pinned` flags are wiped by the sqlite reset that
    /// runs alongside logout.
    pub fn reset(&mut self) {
        self.pinned.clear();
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
