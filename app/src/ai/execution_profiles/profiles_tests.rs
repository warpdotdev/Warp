use chrono::{DateTime, Utc};
use warpui::{App, SingletonEntity};

use crate::ai::execution_profiles::profiles::{AIExecutionProfilesModel, ProfileLookupError};
use crate::ai::execution_profiles::{
    AIExecutionProfile, ActionPermission, CloudAIExecutionProfileModel,
};
use crate::ai::mcp::TemplatableMCPServerManager;
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::persistence::{CloudModel, CloudModelEvent};
use crate::cloud_object::{Revision, ServerAIExecutionProfile, ServerMetadata, ServerPermissions};
use crate::network::NetworkStatus;
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::ids::{ServerId, SyncId};
use crate::server::sync_queue::SyncQueue;
use crate::settings::PrivacySettings;
use crate::test_util::settings::initialize_settings_for_tests;
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::LaunchMode;

fn mock_server_metadata(uid: ServerId) -> ServerMetadata {
    ServerMetadata {
        uid,
        revision: Revision::now(),
        metadata_last_updated_ts: DateTime::<Utc>::default().into(),
        trashed_ts: None,
        folder_id: None,
        is_welcome_object: false,
        creator_uid: None,
        last_editor_uid: None,
        current_editor_uid: None,
    }
}

/// Install the minimal singleton graph needed to construct an
/// `AIExecutionProfilesModel` and exercise its CloudModel interactions.
fn install_singletons(app: &mut App, auth_state: AuthStateProvider) {
    initialize_settings_for_tests(app);
    app.add_singleton_model(|_| auth_state);
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(CloudModel::mock);
    app.add_singleton_model(|_| TemplatableMCPServerManager::default());
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(UserWorkspaces::default_mock);
}

/// Regression test for the onboarding autonomy bug where
/// `edit_profile_internal` would silently drop edits made to an `Unsynced`
/// default profile whenever `personal_drive` returned `None` (logged-out
/// users). `apply_agent_settings` calls `set_*` on the default profile the
/// moment onboarding completes, which can happen before the user logs in
/// (e.g. `LoginSlideEvent::LoginLaterConfirmed`), so those edits must
/// persist on the local `Unsynced` state rather than being dropped.
#[test]
fn edits_persist_on_unsynced_default_profile_when_logged_out() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_logged_out_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        let default_profile_id = profile_model.read(&app, |model, _ctx| model.default_profile_id());

        // Sanity-check the precondition: the baseline `apply_code_diffs`
        // on a fresh default profile is the enum default (`AgentDecides`).
        profile_model.read(&app, |model, ctx| {
            assert!(
                matches!(
                    model.default_profile(ctx).data().apply_code_diffs,
                    ActionPermission::AgentDecides
                ),
                "unexpected baseline apply_code_diffs"
            );
        });

        // Apply the edit that onboarding would make for the Full autonomy
        // preset. Before the fix, this call no-ops because
        // `personal_drive` is `None` while the profile is `Unsynced` — the
        // `set_apply_code_diffs` value was cloned, mutated, then dropped
        // without being written back to `default_profile_state`.
        profile_model.update(&mut app, |model, ctx| {
            model.set_apply_code_diffs(default_profile_id, &ActionPermission::AlwaysAllow, ctx);
        });

        profile_model.read(&app, |model, ctx| {
            assert_eq!(
                model.default_profile(ctx).data().apply_code_diffs,
                ActionPermission::AlwaysAllow,
                "edit was dropped: default profile still has the baseline \
                 apply_code_diffs value after an edit made while logged out",
            );
        });
    })
}

/// Regression test for the "log in to an existing user after onboarding"
/// bug. Cloud objects arriving via the initial bulk load are inserted into
/// `CloudModel` *without* firing per-object `ObjectCreated` events —
/// `update_objects_from_initial_load` passes `emit_events: false` and emits
/// a single `CloudModelEvent::InitialLoadCompleted` afterward instead.
/// Without the reconciliation handler for `InitialLoadCompleted`, the
/// existing user's default profile sits in `CloudModel` but
/// `AIExecutionProfilesModel` stays in `Unsynced`, so a subsequent
/// onboarding edit creates a duplicate cloud default profile instead of
/// editing the existing one. This test drives that sequence and asserts
/// the model adopts the cloud profile's sync id.
#[test]
fn reconciles_unsynced_default_profile_with_cloud_after_initial_load() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        // Baseline: CloudModel is empty, so the model starts Unsynced and
        // `sync_id` is `None`.
        profile_model.read(&app, |model, ctx| {
            assert!(
                model.default_profile(ctx).sync_id().is_none(),
                "default profile should be Unsynced at startup"
            );
        });

        // Simulate the user's existing cloud default profile arriving via
        // initial bulk load. We construct the existing profile with
        // `apply_code_diffs = AlwaysAllow` so we can verify the model is
        // reading that cloud object after reconciliation.
        let cloud_uid = ServerId::from(42);
        let cloud_sync_id = SyncId::ServerId(cloud_uid);
        let cloud_profile = AIExecutionProfile {
            name: "Default".to_string(),
            is_default_profile: true,
            apply_code_diffs: ActionPermission::AlwaysAllow,
            ..Default::default()
        };
        let server_object = ServerAIExecutionProfile {
            id: cloud_sync_id,
            model: CloudAIExecutionProfileModel::new(cloud_profile),
            metadata: mock_server_metadata(cloud_uid),
            permissions: ServerPermissions::mock_personal(),
        };

        // Insert the object into CloudModel via the initial-load path
        // (`emit_events=false`) and then emit `InitialLoadCompleted` so the
        // reconciliation handler fires.
        CloudModel::handle(&app).update(&mut app, move |cloud_model, ctx| {
            let server_objects: Vec<ServerAIExecutionProfile> = vec![server_object];
            cloud_model.update_objects_from_initial_load(server_objects, false, false, ctx);
            ctx.emit(CloudModelEvent::InitialLoadCompleted);
        });

        // The model should now be Synced with the cloud profile's sync_id,
        // and `default_profile` should read values from the existing cloud
        // object (proving we're not backed by a fresh client-side default).
        profile_model.read(&app, |model, ctx| {
            let info = model.default_profile(ctx);
            assert_eq!(
                info.sync_id(),
                Some(cloud_sync_id),
                "model did not adopt the existing cloud default profile's sync_id"
            );
            assert_eq!(
                info.data().apply_code_diffs,
                ActionPermission::AlwaysAllow,
                "default profile should now surface the existing cloud value"
            );
        });

        // Further edits should now target the existing cloud profile in
        // place, rather than falling through the `Unsynced` branch and
        // creating a duplicate.
        let default_profile_id = profile_model.read(&app, |model, _ctx| model.default_profile_id());
        profile_model.update(&mut app, |model, ctx| {
            model.set_apply_code_diffs(default_profile_id, &ActionPermission::AlwaysAsk, ctx);
        });
        profile_model.read(&app, |model, ctx| {
            let info = model.default_profile(ctx);
            assert_eq!(
                info.sync_id(),
                Some(cloud_sync_id),
                "edit should target the same cloud sync_id, not create a duplicate"
            );
            assert_eq!(
                info.data().apply_code_diffs,
                ActionPermission::AlwaysAsk,
                "edit should be reflected on the existing cloud profile"
            );
        });
    })
}

// ── find_profile_by_display_name tests ──────────────────────────────
//
// These tests cover the lookup helper used by tab-config agent panes.
// They install a model that already has the default profile populated
// from the in-memory `Unsynced` state, then push named profiles into
// `CloudModel` via the initial-load path so the model reconciles them
// and they become discoverable via `find_profile_by_display_name`.

/// Insert a custom (non-default) AI execution profile into `CloudModel`
/// and reconcile so the profile model picks up the new sync_id.
fn install_custom_profile(
    app: &mut App,
    profile_model: warpui::ModelHandle<AIExecutionProfilesModel>,
    server_id_raw: i64,
    name: &str,
) {
    let cloud_uid = ServerId::from(server_id_raw);
    let cloud_sync_id = SyncId::ServerId(cloud_uid);
    let cloud_profile = AIExecutionProfile {
        name: name.to_string(),
        is_default_profile: false,
        ..Default::default()
    };
    let server_object = ServerAIExecutionProfile {
        id: cloud_sync_id,
        model: CloudAIExecutionProfileModel::new(cloud_profile),
        metadata: mock_server_metadata(cloud_uid),
        permissions: ServerPermissions::mock_personal(),
    };

    CloudModel::handle(app).update(app, move |cloud_model, ctx| {
        let server_objects: Vec<ServerAIExecutionProfile> = vec![server_object];
        cloud_model.update_objects_from_initial_load(server_objects, false, false, ctx);
        ctx.emit(CloudModelEvent::InitialLoadCompleted);
    });

    // Sanity: the profile model has registered a new sync id.
    profile_model.read(app, |model, _ctx| {
        assert!(
            model
                .get_all_profile_ids()
                .iter()
                .any(|id| model.default_profile_id() != *id),
            "expected a non-default profile to be registered after initial load",
        );
    });
}

#[test]
fn find_profile_by_display_name_unique_match() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        install_custom_profile(&mut app, profile_model.clone(), 100, "Coder");

        profile_model.read(&app, |model, ctx| {
            let result = model.find_profile_by_display_name("Coder", ctx);
            assert!(
                result.is_ok(),
                "expected unique match for 'Coder', got {result:?}",
            );
        });
    })
}

#[test]
fn find_profile_by_display_name_not_found() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        profile_model.read(&app, |model, ctx| {
            let result = model.find_profile_by_display_name("Nonexistent", ctx);
            assert!(
                matches!(result, Err(ProfileLookupError::NotFound(ref n)) if n == "Nonexistent"),
                "expected NotFound for 'Nonexistent', got {result:?}",
            );
        });
    })
}

#[test]
fn find_profile_by_display_name_ambiguous_two_custom_same_name() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        install_custom_profile(&mut app, profile_model.clone(), 200, "Work");
        install_custom_profile(&mut app, profile_model.clone(), 201, "Work");

        profile_model.read(&app, |model, ctx| {
            let result = model.find_profile_by_display_name("Work", ctx);
            match result {
                Err(ProfileLookupError::Ambiguous { ref name, count }) => {
                    assert_eq!(name, "Work");
                    assert_eq!(count, 2);
                }
                other => panic!("expected Ambiguous for 'Work', got {other:?}"),
            }
        });
    })
}

#[test]
fn find_profile_by_display_name_default_resolves() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        // The default profile's `display_name()` is hard-coded to "Default"
        // regardless of the stored name field (see `mod.rs:444-452`).
        profile_model.read(&app, |model, ctx| {
            let result = model.find_profile_by_display_name("Default", ctx);
            let default_id = model.default_profile_id();
            match result {
                Ok(id) => assert_eq!(id, default_id, "expected default profile id, got {id:?}",),
                other => panic!("expected default profile to resolve, got {other:?}"),
            }
        });
    })
}

#[test]
fn find_profile_by_display_name_two_untitled_collide() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        // Profiles with empty `name` display as "Untitled" — two such
        // profiles collide into Ambiguous when looked up by "Untitled".
        install_custom_profile(&mut app, profile_model.clone(), 300, "");
        install_custom_profile(&mut app, profile_model.clone(), 301, "");

        profile_model.read(&app, |model, ctx| {
            let result = model.find_profile_by_display_name("Untitled", ctx);
            match result {
                Err(ProfileLookupError::Ambiguous { ref name, count }) => {
                    assert_eq!(name, "Untitled");
                    assert_eq!(count, 2);
                }
                other => panic!("expected Ambiguous for 'Untitled', got {other:?}"),
            }
        });
    })
}

#[test]
fn find_profile_by_display_name_trims_input() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        install_custom_profile(&mut app, profile_model.clone(), 400, "Coder");

        profile_model.read(&app, |model, ctx| {
            let result = model.find_profile_by_display_name("  Coder  ", ctx);
            assert!(
                result.is_ok(),
                "expected trimmed lookup to succeed, got {result:?}",
            );
        });
    })
}

#[test]
fn find_profile_by_display_name_empty_input_returns_not_found() {
    App::test((), |mut app| async move {
        install_singletons(&mut app, AuthStateProvider::new_for_test());
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        profile_model.read(&app, |model, ctx| {
            let result = model.find_profile_by_display_name("   ", ctx);
            assert!(
                matches!(result, Err(ProfileLookupError::NotFound(_))),
                "expected NotFound for whitespace-only input, got {result:?}",
            );
        });
    })
}
