use std::collections::HashMap;

use super::super::proto::{Authenticate, Initialize};
use super::super::protocol::RequestId;
use super::{DaemonAuthContext, PendingFileOps, ServerModel};

fn test_model() -> ServerModel {
    ServerModel {
        connection_senders: HashMap::new(),
        snapshot_sent_roots_by_connection: HashMap::new(),
        grace_timer_cancel: None,
        in_progress: HashMap::new(),
        host_id: "test-host-id".to_string(),
        executors: HashMap::new(),
        pending_file_ops: PendingFileOps::new(),
        auth: DaemonAuthContext::new(),
    }
}

fn request_id() -> RequestId {
    RequestId::from("test-request".to_string())
}

#[test]
fn fresh_model_starts_without_auth_token() {
    let model = test_model();

    assert_eq!(model.auth_token(), None);
}

#[test]
fn initialize_with_auth_token_stores_token() {
    let mut model = test_model();

    model.apply_initialize_auth(&Initialize {
        auth_token: "initial-token".to_string(),
        user_id: String::new(),
        user_email: String::new(),
        crash_reporting_enabled: true,
    });

    assert_eq!(model.auth_token(), Some("initial-token"));
}

#[test]
fn empty_initialize_preserves_existing_auth_token() {
    let mut model = test_model();
    model.apply_initialize_auth(&Initialize {
        auth_token: "initial-token".to_string(),
        user_id: String::new(),
        user_email: String::new(),
        crash_reporting_enabled: true,
    });

    model.apply_initialize_auth(&Initialize {
        auth_token: String::new(),
        user_id: String::new(),
        user_email: String::new(),
        crash_reporting_enabled: true,
    });

    assert_eq!(model.auth_token(), Some("initial-token"));
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

    assert_eq!(model.auth_token(), Some("rotated-token"));
}

#[test]
fn empty_authenticate_preserves_existing_auth_token() {
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

    assert_eq!(model.auth_token(), Some("initial-token"));
}
