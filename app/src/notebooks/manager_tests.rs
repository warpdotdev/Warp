use std::sync::mpsc;

use warp_core::ui::appearance::Appearance;
use warpui::{
    platform::WindowStyle, AddSingletonModel, App, EntityId, ModelHandle, ViewContext, ViewHandle,
};

use crate::{
    ai::blocklist::BlocklistAIHistoryModel,
    auth::{AuthManager, AuthStateProvider},
    cloud_object::update_manager::UpdateManager,
    cloud_object::{
        model::{
            actions::ObjectActions, persistence::ObjectStoreModel, view::ObjectStoreViewModel,
        },
        Owner,
    },
    network::NetworkStatus,
    notebooks::{editor::keys::NotebookKeybindings, notebook::NotebookView},
    pane_group::NotebookPane,
    persistence::ModelEvent,
    search::files::model::FileSearchModel,
    settings::PrivacySettings,
    settings_view::keybindings::KeybindingChangedNotifier,
    terminal::keys::TerminalKeybindings,
    test_util::settings::initialize_settings_for_tests,
    workspace::ActiveSession,
    workspaces::{user_profiles::UserProfiles, user_workspaces::UserWorkspaces},
    GlobalResourceHandles, GlobalResourceHandlesProvider,
};

use super::NotebookManager;

struct TestState {
    manager: ModelHandle<NotebookManager>,
    model_events: mpsc::Receiver<ModelEvent>,
}

impl TestState {
    /// Add a notebook view, configured by `init`, and register it with the [`NotebookManager`].
    fn add_notebook<F>(&self, app: &mut App, init: F) -> ViewHandle<NotebookView>
    where
        F: FnOnce(&mut NotebookView, &mut ViewContext<NotebookView>),
    {
        let (window, notebook) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
            let mut view = NotebookView::new(ctx);
            init(&mut view, ctx);
            view
        });

        self.manager.update(app, |manager, ctx| {
            let pane = NotebookPane::new(notebook.clone(), ctx);
            manager.register_pane(&pane, EntityId::new(), window, ctx)
        });

        notebook
    }

    /// All model events not yet received.
    fn model_events(&self) -> Vec<ModelEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.model_events.try_recv() {
            events.push(event);
        }
        events
    }
}

fn initialize_app(app: &mut App) -> TestState {
    initialize_settings_for_tests(app);

    let global_resources = GlobalResourceHandles::mock(app);
    app.add_singleton_model(|_| GlobalResourceHandlesProvider::new(global_resources));
    app.add_singleton_model(ObjectStoreModel::mock);
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(|_| UserProfiles::new(vec![]));
    app.add_singleton_model(|_| ActiveSession::default());
    app.add_singleton_model(|_| ObjectActions::new(Vec::new()));
    app.add_singleton_model(|_| KeybindingChangedNotifier::new());
    app.add_singleton_model(|_| repo_metadata::repositories::DetectedRepositories::default());
    #[cfg(feature = "local_fs")]
    app.add_singleton_model(repo_metadata::RepoMetadataModel::new);
    app.add_singleton_model(FileSearchModel::new);
    app.add_singleton_model(NotebookKeybindings::new);
    app.add_singleton_model(TerminalKeybindings::new);
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
    #[cfg(feature = "voice_input")]
    app.add_singleton_model(voice_input::VoiceInput::new);

    let (sender, receiver) = mpsc::sync_channel(10);
    app.add_singleton_model(|ctx| UpdateManager::new(Some(sender), ctx));
    // OpenWarp(Wave 4):SyncQueue 整删,原 `sync_queue.start_dequeueing(ctx)` 已不适用。

    app.add_singleton_model(ObjectStoreViewModel::mock);
    let manager = app.add_singleton_model(NotebookManager::mock);
    TestState {
        manager,
        model_events: receiver,
    }
}

#[test]
fn test_save_on_close() {
    App::test((), |mut app| async move {
        let state = initialize_app(&mut app);
        let notebook = state.add_notebook(&mut app, |view, ctx| {
            view.open_new_notebook(
                Some("Test Notebook".to_string()),
                Owner::mock_current_user(),
                None,
                ctx,
            );
        });

        // Ensure the notebook has a pending edit.
        notebook.update(&mut app, |notebook, ctx| {
            notebook.input_editor().update(ctx, |editor, ctx| {
                editor.user_typed("Hello", ctx);
            });
        });

        // There will be an initial model event to save the notebook.
        let events = state.model_events();
        assert_eq!(events.len(), 1, "Expected 1 event, got {events:?}");

        // Closing the notebook manager should trigger a save.
        state
            .manager
            .update(&mut app, |manager, ctx| manager.close_notebooks(ctx));

        // There should now be a pending model event to save the notebook.
        let events = state.model_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ModelEvent::UpsertNotebook { notebook } => {
                assert_eq!(notebook.model().title, "Test Notebook");
                assert_eq!(notebook.model().data, "Hello");
            }
            other => panic!("Expected an UpsertNotebook event, got {other:?}"),
        }
    });
}
