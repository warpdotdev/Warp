//! Shared singleton holding which child-agent conversations are currently
//! pinned in the orchestration pill bar.
//!
//! Pin state has to be shared across every `OrchestrationPillBar` instance
//! (one per `TerminalView` / pane) so that pinning a child in one pane
//! reflects in every other pane that's rendering the same orchestration
//! tree. Keeping it on the per-view struct caused a visible bug where a
//! pinned pill would appear pinned in the orchestrator pane but unpinned
//! once you focused into the child's own pane.

use std::collections::HashSet;

use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::blocklist::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};

/// Toggles `id` in `pinned`: inserts when absent, removes when present.
/// Extracted as a free function so the toggle semantics can be unit tested
/// without constructing an entire singleton model.
pub(super) fn toggle_pin_in_set(pinned: &mut HashSet<AIConversationId>, id: AIConversationId) {
    if !pinned.remove(&id) {
        pinned.insert(id);
    }
}

/// Singleton owning the set of currently-pinned child conversation ids.
///
/// Each `OrchestrationPillBar` reads from this model when building its
/// pill list and dispatches `TogglePin` actions that mutate this model.
/// All instances subscribe to `PinSetChanged` so a pin/unpin in any pane
/// re-renders the bars in every other pane.
#[derive(Default)]
pub struct OrchestrationPinModel {
    pinned: HashSet<AIConversationId>,
}

impl OrchestrationPinModel {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        // Subscribe to history events so pinned ids are pruned globally
        // when a conversation is removed/deleted. Doing this once in the
        // singleton (rather than per pill bar) keeps the cleanup
        // authoritative and avoids the per-pane `retain` logic that
        // would otherwise clobber pins belonging to orchestrators
        // displayed in other panes.
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

    /// Toggles whether `conversation_id` is pinned and emits
    /// `PinSetChanged` so every subscribed pill bar can re-render.
    pub fn toggle_pin(&mut self, conversation_id: AIConversationId, ctx: &mut ModelContext<Self>) {
        toggle_pin_in_set(&mut self.pinned, conversation_id);
        ctx.emit(OrchestrationPinEvent::PinSetChanged);
    }
}

impl Entity for OrchestrationPinModel {
    type Event = OrchestrationPinEvent;
}

impl SingletonEntity for OrchestrationPinModel {}

/// Events emitted by `OrchestrationPinModel`. Subscribers (each
/// `OrchestrationPillBar`) typically respond by calling `ctx.notify()` to
/// trigger a re-render with the new partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrchestrationPinEvent {
    /// The set of pinned conversation ids changed (a pin or unpin
    /// happened, or a pinned conversation was removed from history).
    PinSetChanged,
}

#[cfg(test)]
#[path = "orchestration_pin_model_tests.rs"]
mod tests;
