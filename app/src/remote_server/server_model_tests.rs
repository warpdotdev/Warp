use crate::auth::auth_state::AuthState;
use std::collections::HashMap;
use std::sync::Arc;

use super::super::diff_state_tracker::GlobalDiffStateModel;
use super::super::proto::{Authenticate, Initialize};
use super::super::server_buffer_tracker::ServerBufferTracker;
use super::{PendingFileOps, ServerModel};

fn test_model() -> ServerModel {
    ServerModel {
        connection_senders: HashMap::new(),
        snapshot_sent_roots_by_connection: HashMap::new(),
        grace_timer_cancel: None,
        in_progress: HashMap::new(),
        host_id: "test-host-id".to_string(),
        executors: HashMap::new(),
        pending_file_ops: PendingFileOps::new(),
        auth_state: Arc::new(AuthState::new_logged_out_for_test()),
        buffers: ServerBufferTracker::new(),
        diff_states: GlobalDiffStateModel::new(),
    }
}

#[test]
fn fresh_model_starts_without_auth_token() {
    let model = test_model();

    assert_eq!(model.auth_token().as_deref(), None);
    assert_eq!(model.auth_state.user_id(), None);
    assert_eq!(model.auth_state.user_email(), None);
}

#[test]
fn initialize_with_auth_token_stores_token() {
    let mut model = test_model();

    model.apply_initialize_auth(&Initialize {
        auth_token: "initial-token".to_string(),
        user_id: "test-user-id".to_string(),
        user_email: "test@example.com".to_string(),
        crash_reporting_enabled: true,
    });

    assert_eq!(model.auth_token().as_deref(), Some("initial-token"));
    assert_eq!(
        model.auth_state.user_id().unwrap().as_string(),
        "test-user-id"
    );
    assert_eq!(
        model.auth_state.user_email().as_deref(),
        Some("test@example.com")
    );
}

#[test]
fn empty_initialize_clears_auth_context() {
    let mut model = test_model();
    model.apply_initialize_auth(&Initialize {
        auth_token: "initial-token".to_string(),
        user_id: "test-user-id".to_string(),
        user_email: "test@example.com".to_string(),
        crash_reporting_enabled: true,
    });

    model.apply_initialize_auth(&Initialize {
        auth_token: String::new(),
        user_id: String::new(),
        user_email: String::new(),
        crash_reporting_enabled: true,
    });

    assert_eq!(model.auth_token().as_deref(), None);
    assert_eq!(model.auth_state.user_id(), None);
    assert_eq!(model.auth_state.user_email(), None);
}

#[test]
fn authenticate_with_auth_token_replaces_auth_token() {
    let mut model = test_model();
    model.apply_initialize_auth(&Initialize {
        auth_token: "initial-token".to_string(),
        user_id: String::new(),
        user_email: String::new(),
        crash_reporting_enabled: true,
    });

    model.handle_authenticate(Authenticate {
        auth_token: "rotated-token".to_string(),
    });

    assert_eq!(model.auth_token().as_deref(), Some("rotated-token"));
}

#[test]
fn empty_authenticate_clears_auth_token() {
    let mut model = test_model();
    model.apply_initialize_auth(&Initialize {
        auth_token: "initial-token".to_string(),
        user_id: String::new(),
        user_email: String::new(),
        crash_reporting_enabled: true,
    });

    model.handle_authenticate(Authenticate {
        auth_token: String::new(),
    });

    assert_eq!(model.auth_token().as_deref(), None);
}
