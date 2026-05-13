//! Unit tests for [`super::util`].
//!
//! These tests focus on the small, pure pieces of [`WorkspaceState`] that
//! don't need a workspace view or app context — currently the
//! tab/pane/conversation rename state machine. Heavier scenarios that need a
//! live `Workspace` view live in `view_test.rs` / integration tests.
use super::{PaneViewLocator, WorkspaceState};
use crate::ai::agent::conversation::AIConversationId;
use crate::pane_group::TerminalPaneId;
use warpui::EntityId;

fn pane_locator() -> PaneViewLocator {
    PaneViewLocator {
        pane_group_id: EntityId::new(),
        pane_id: TerminalPaneId::dummy_terminal_pane_id().into(),
    }
}

#[test]
fn workspace_state_starts_with_no_rename_in_progress() {
    let state = WorkspaceState::default();
    assert!(!state.is_tab_being_renamed());
    assert!(!state.is_any_pane_being_renamed());
    assert!(!state.is_any_conversation_being_renamed());
    assert_eq!(state.tab_being_renamed(), None);
    assert_eq!(state.pane_being_renamed(), None);
    assert_eq!(state.conversation_being_renamed(), None);
}

/// GH8642 invariant: only one rename surface (tab / pane / conversation) can
/// be active at a time. Setting any one of the three must clear the others
/// so the workspace-level inline editor is always owned by exactly one
/// rename source.
#[test]
fn workspace_state_rename_modes_are_mutually_exclusive() {
    let mut state = WorkspaceState::default();
    let conversation_id = AIConversationId::new();
    let pane = pane_locator();

    // Start with a tab rename, then enter pane rename: tab clears.
    state.set_tab_being_renamed(7);
    assert_eq!(state.tab_being_renamed(), Some(7));
    state.set_pane_being_renamed(pane);
    assert_eq!(state.tab_being_renamed(), None);
    assert!(state.is_pane_being_renamed(pane));

    // Now enter conversation rename: pane clears.
    state.set_conversation_being_renamed(conversation_id);
    assert!(!state.is_any_pane_being_renamed());
    assert!(state.is_any_conversation_being_renamed());
    assert_eq!(state.conversation_being_renamed(), Some(conversation_id));
    assert!(state.is_conversation_being_renamed(conversation_id));

    // Round-trip back to tab: conversation clears.
    state.set_tab_being_renamed(2);
    assert_eq!(state.tab_being_renamed(), Some(2));
    assert!(!state.is_any_conversation_being_renamed());
    assert_eq!(state.conversation_being_renamed(), None);
}

#[test]
fn workspace_state_clear_conversation_being_renamed_idempotent() {
    let mut state = WorkspaceState::default();
    let conversation_id = AIConversationId::new();

    state.set_conversation_being_renamed(conversation_id);
    assert!(state.is_any_conversation_being_renamed());
    state.clear_conversation_being_renamed();
    assert!(!state.is_any_conversation_being_renamed());
    // Calling clear again is a no-op (regression: don't panic on double-clear).
    state.clear_conversation_being_renamed();
    assert_eq!(state.conversation_being_renamed(), None);
}

#[test]
fn workspace_state_is_conversation_being_renamed_only_matches_active_id() {
    let mut state = WorkspaceState::default();
    let active = AIConversationId::new();
    let other = AIConversationId::new();
    assert_ne!(active, other);

    state.set_conversation_being_renamed(active);
    assert!(state.is_conversation_being_renamed(active));
    assert!(!state.is_conversation_being_renamed(other));
}
