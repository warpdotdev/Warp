use crate::auth::auth_state::AuthState;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use warp_util::standardized_path::StandardizedPath;
use warpui::App;

use super::super::diff_state_tracker::RemoteDiffStateManager;

use super::super::proto::{Authenticate, Initialize};
use super::super::server_buffer_tracker::ServerBufferTracker;
use super::{PendingFileOps, ServerModel};

fn test_model(app: &mut App) -> ServerModel {
    ServerModel {
        connection_senders: HashMap::new(),
        snapshot_sent_roots_by_connection: HashMap::new(),
        authorized_codebase_index_roots_by_connection: HashMap::new(),
        grace_timer_cancel: None,
        in_progress: HashMap::new(),
        host_id: "test-host-id".to_string(),
        executors: HashMap::new(),
        pending_file_ops: PendingFileOps::new(),
        auth_state: Arc::new(AuthState::new_logged_out_for_test()),
        buffers: ServerBufferTracker::new(),
        diff_states: app.add_model(|_| RemoteDiffStateManager::new()),
    }
}

#[test]
fn codebase_index_root_authorization_is_connection_scoped() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        let authorized_conn = uuid::Uuid::new_v4();
        let unauthorized_conn = uuid::Uuid::new_v4();
        let repo_path = StandardizedPath::from_local_canonicalized(Path::new("/")).unwrap();

        model
            .authorized_codebase_index_roots_by_connection
            .insert(authorized_conn, HashSet::new());
        model
            .authorized_codebase_index_roots_by_connection
            .insert(unauthorized_conn, HashSet::new());
        model.authorize_codebase_index_root_for_connection(authorized_conn, repo_path.clone());

        assert!(model.codebase_index_root_authorized_for_connection(authorized_conn, &repo_path));
        assert!(!model.codebase_index_root_authorized_for_connection(
            unauthorized_conn,
            &repo_path
        ));
    });
}

#[test]
fn removing_authorized_codebase_index_root_clears_all_connections() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        let first_conn = uuid::Uuid::new_v4();
        let second_conn = uuid::Uuid::new_v4();
        let repo_path = StandardizedPath::from_local_canonicalized(Path::new("/")).unwrap();

        model.authorize_codebase_index_root_for_connection(first_conn, repo_path.clone());
        model.authorize_codebase_index_root_for_connection(second_conn, repo_path.clone());
        model.remove_authorized_codebase_index_root(&repo_path);

        assert!(!model.codebase_index_root_authorized_for_connection(first_conn, &repo_path));
        assert!(!model.codebase_index_root_authorized_for_connection(second_conn, &repo_path));
    });
}

#[test]
fn fresh_model_starts_without_auth_token() {
    App::test((), |mut app| async move {
        let model = test_model(&mut app);

        assert_eq!(model.auth_token().as_deref(), None);
        assert_eq!(model.auth_state.user_id(), None);
        assert_eq!(model.auth_state.user_email(), None);
    });
}

#[test]
fn initialize_with_auth_token_stores_token() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);

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
    });
}

#[test]
fn empty_initialize_clears_auth_context() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
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
    });
}

#[test]
fn authenticate_with_auth_token_replaces_auth_token() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
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
    });
}

#[test]
fn empty_authenticate_clears_auth_token() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
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
    });
}
