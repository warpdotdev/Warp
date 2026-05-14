//! Unit tests for [`OrchestrationViewerModel`].
//!
//! The status-mapping coverage here is pure (no `App::test` needed); the
//! interactive child-discovery / polling paths are exercised via the public
//! `pub(crate)` accessors so callers can write higher-level integration tests
//! without reaching into private fields.

use super::*;

#[test]
fn maps_working_states_to_in_progress() {
    for state in [
        AmbientAgentTaskState::Queued,
        AmbientAgentTaskState::Pending,
        AmbientAgentTaskState::Claimed,
        AmbientAgentTaskState::InProgress,
    ] {
        assert!(
            matches!(
                conversation_status_from_state(&state),
                ConversationStatus::InProgress
            ),
            "expected InProgress for {state:?}",
        );
    }
}

#[test]
fn maps_succeeded_to_success() {
    assert!(matches!(
        conversation_status_from_state(&AmbientAgentTaskState::Succeeded),
        ConversationStatus::Success
    ));
}

#[test]
fn maps_failed_and_error_to_error() {
    assert!(matches!(
        conversation_status_from_state(&AmbientAgentTaskState::Failed),
        ConversationStatus::Error
    ));
    assert!(matches!(
        conversation_status_from_state(&AmbientAgentTaskState::Error),
        ConversationStatus::Error
    ));
}

#[test]
fn maps_blocked_to_blocked() {
    let status = conversation_status_from_state(&AmbientAgentTaskState::Blocked);
    assert!(matches!(status, ConversationStatus::Blocked { .. }));
}

#[test]
fn maps_cancelled_to_cancelled() {
    assert!(matches!(
        conversation_status_from_state(&AmbientAgentTaskState::Cancelled),
        ConversationStatus::Cancelled
    ));
}

#[test]
fn unknown_state_maps_to_in_progress() {
    // Forward-compat: we don't want a yet-unseen server state to commit the
    // pill badge to a final outcome.
    assert!(matches!(
        conversation_status_from_state(&AmbientAgentTaskState::Unknown),
        ConversationStatus::InProgress
    ));
}
