use std::time::Duration;

use warpui::{
    async_assert, async_assert_eq,
    integration::{AssertionCallback, AssertionOutcome, TestStep},
    SingletonEntity,
};

use crate::{
    integration_testing::view_getters::single_terminal_view_for_tab,
    remote_server::manager::{RemoteServerManager, RemoteSessionState},
    terminal::model::session::command_executor::remote_server_executor::RemoteServerCommandExecutor,
};

/// Returns a `TestStep` that polls until the remote server setup state for
/// the active session reaches `Ready`. Times out after 60 seconds to allow
/// for the full check → install → connect → handshake flow.
pub fn wait_for_remote_server_ready(tab_idx: usize) -> TestStep {
    TestStep::new("Wait for remote server setup to be Ready")
        .set_timeout(Duration::from_secs(60))
        .add_assertion(assert_remote_server_setup_ready(tab_idx))
}

/// Asserts that `Sessions::remote_server_setup_state` for the active session
/// is `RemoteServerSetupState::Ready`.
fn assert_remote_server_setup_ready(tab_idx: usize) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, tab_idx);
        terminal_view.read(app, |view, ctx| {
            let Some(session_id) = view.active_block_session_id() else {
                return AssertionOutcome::PreconditionFailed("No active session ID".into());
            };
            let sessions = view.sessions(ctx);
            let Some(state) = sessions.remote_server_setup_state(session_id) else {
                return AssertionOutcome::PreconditionFailed(
                    "No remote server setup state for session".into(),
                );
            };
            async_assert!(
                state.is_ready(),
                "Expected RemoteServerSetupState::Ready, got {state:?}"
            )
        })
    })
}

/// Asserts that `RemoteServerManager` has the active session in `Connected`
/// state (i.e., the initialize handshake succeeded and a `HostId` is present).
pub fn assert_remote_server_connected(tab_idx: usize) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, tab_idx);
        terminal_view.read(app, |view, ctx| {
            let Some(session_id) = view.active_block_session_id() else {
                return AssertionOutcome::PreconditionFailed("No active session ID".into());
            };
            let mgr = RemoteServerManager::as_ref(ctx);
            let Some(session_state) = mgr.session(session_id) else {
                return AssertionOutcome::failure(format!(
                    "RemoteServerManager has no session for {session_id:?}"
                ));
            };
            async_assert!(
                matches!(session_state, RemoteSessionState::Connected { .. }),
                "Expected RemoteSessionState::Connected, got {session_state:?}"
            )
        })
    })
}

/// Asserts that the active session's `CommandExecutor` is a
/// `RemoteServerCommandExecutor` (not the fallback ControlMaster-based
/// `RemoteCommandExecutor`).
pub fn assert_command_executor_is_remote_server(tab_idx: usize) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, tab_idx);
        terminal_view.read(app, |view, ctx| {
            let Some(session_id) = view.active_block_session_id() else {
                return AssertionOutcome::PreconditionFailed("No active session ID".into());
            };
            let Some(session) = view.sessions(ctx).get(session_id) else {
                return AssertionOutcome::PreconditionFailed("Session not found".into());
            };
            let executor = session.command_executor();
            let is_remote_server = executor
                .as_any()
                .downcast_ref::<RemoteServerCommandExecutor>()
                .is_some();
            async_assert!(
                is_remote_server,
                "Expected RemoteServerCommandExecutor, got {:?}",
                std::any::type_name_of_val(&*executor)
            )
        })
    })
}

/// Returns a `TestStep` action that writes a file on the remote host via
/// the `RemoteServerClient::write_file` proto API. The write is dispatched
/// on a background thread using `tokio::runtime::Runtime::block_on` since
/// the action callback is synchronous.
pub fn write_file_via_remote_server(
    tab_idx: usize,
    path: String,
    content: String,
) -> Box<dyn Fn(&mut warpui::App, warpui::WindowId, &mut warpui::integration::StepDataMap) + 'static>
{
    Box::new(move |app, window_id, _| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, tab_idx);
        let maybe_client = terminal_view.read(app, |view, ctx| {
            let session_id = view.active_block_session_id()?;
            RemoteServerManager::as_ref(ctx)
                .client_for_session(session_id)
                .cloned()
        });

        if let Some(client) = maybe_client {
            let path = path.clone();
            let content = content.clone();
            // Spawn on a background thread because the action callback is sync
            // but write_file is async.
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                let result = rt.block_on(client.write_file(path.clone(), content));
                if let Err(e) = &result {
                    log::error!("write_file_via_remote_server failed for {path}: {e}");
                }
            });
        } else {
            log::error!("write_file_via_remote_server: no connected client");
        }
    })
}

/// Returns a `TestStep` action that calls
/// `RemoteServerManager::load_remote_repo_metadata_directory` for the active
/// session. This triggers the lazy-loading proto request.
pub fn load_repo_metadata_directory_via_remote_server(
    tab_idx: usize,
    repo_path: String,
    dir_path: String,
) -> Box<dyn Fn(&mut warpui::App, warpui::WindowId, &mut warpui::integration::StepDataMap) + 'static>
{
    Box::new(move |app, window_id, _| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, tab_idx);
        let maybe_session_id = terminal_view.read(app, |view, _ctx| view.active_block_session_id());

        let Some(session_id) = maybe_session_id else {
            log::error!("load_repo_metadata_directory_via_remote_server: no active session");
            return;
        };

        let repo_path = repo_path.clone();
        let dir_path = dir_path.clone();
        RemoteServerManager::handle(app).update(app, |mgr, ctx| {
            mgr.load_remote_repo_metadata_directory(session_id, repo_path, dir_path, ctx);
        });
    })
}

/// Asserts that `RemoteServerManager` has a successful navigation response
/// recorded for the active session and that it matches `expected_path`.
pub fn assert_remote_server_has_navigated(
    tab_idx: usize,
    expected_path: impl Into<String>,
) -> AssertionCallback {
    let expected_path = expected_path.into();
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, tab_idx);
        terminal_view.read(app, |view, ctx| {
            let Some(session_id) = view.active_block_session_id() else {
                return AssertionOutcome::PreconditionFailed("No active session ID".into());
            };
            let mgr = RemoteServerManager::as_ref(ctx);
            if !matches!(
                mgr.session(session_id),
                Some(RemoteSessionState::Connected { .. })
            ) {
                return AssertionOutcome::PreconditionFailed(
                    "Session not in Connected state".into(),
                );
            }
            let Some(navigated_path) = mgr.last_successful_navigation_path_for_session(session_id)
            else {
                return AssertionOutcome::PreconditionFailed(
                    "No successful navigation path recorded for session".into(),
                );
            };
            async_assert_eq!(
                navigated_path,
                expected_path.as_str(),
                "Expected remote server to navigate session {session_id:?} to {expected_path}"
            )
        })
    })
}
