use ai::LLMId;
use onboarding::slides::{AgentAutonomy, AgentDevelopmentSettings, ProjectOnboardingSettings};
use onboarding::SelectedSettings;
use warpui::{App, SingletonEntity};

use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::execution_profiles::{
    AIExecutionProfile, AIExecutionProfileObject, AIExecutionProfileObjectModel, ActionPermission,
};
use crate::ai::mcp::TemplatableMCPServerManager;
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::persistence::{ObjectStoreEvent, ObjectStoreModel};
use crate::cloud_object::update_manager::UpdateManager;
use crate::cloud_object::{StoredObjectMetadata, StoredObjectPermissions};
use crate::network::NetworkStatus;
use crate::server::ids::{ServerId, SyncId};
use crate::settings::{apply_onboarding_settings, PrivacySettings};
use crate::test_util::settings::initialize_settings_for_tests;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::LaunchMode;

/// Regression test for: "Logging in to an existing user at the end of
/// onboarding should preserve the user's cloud-stored default execution
/// profile rather than overwriting it with the onboarding-selected
/// base_model and autonomy."
///
/// Simulates the full post-login flow:
///   1. User starts unauthenticated; `AIExecutionProfilesModel` begins in
///      `Unsynced`.
///   2. User logs into an existing account; their cloud default profile
///      arrives via initial bulk load and `InitialLoadCompleted` fires.
///   3. The reconciliation handler promotes the local state to `Synced`
///      with the cloud profile's `sync_id`.
///   4. `apply_onboarding_settings` runs (as it would from
///      `handle_cloud_preferences_syncer_event`) with onboarding-selected
///      values that differ from what's on the stored profile object.
///   5. The stored profile object's values must be preserved.
#[test]
fn apply_onboarding_settings_preserves_existing_profile_object_on_existing_user_login() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(|_| NetworkStatus::new());
        app.add_singleton_model(UpdateManager::mock);
        app.add_singleton_model(ObjectStoreModel::mock);
        app.add_singleton_model(|_| TemplatableMCPServerManager::default());
        app.add_singleton_model(PrivacySettings::mock);
        app.add_singleton_model(UserWorkspaces::default_mock);
        let profile_model = app.add_singleton_model(|ctx| {
            AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
        });

        // The existing user's stored default profile. Values are
        // deliberately chosen to differ from both `AIExecutionProfile`'s
        // defaults and from the onboarding values we'll pass below, so any
        // accidental overwrite is detectable.
        let cloud_uid = ServerId::from(7);
        let cloud_sync_id = SyncId::ServerId(cloud_uid);
        let cloud_stored_model = LLMId::from("claude-existing-cloud-model");
        let local_profile = AIExecutionProfile {
            name: "Default".to_string(),
            is_default_profile: true,
            base_model: Some(cloud_stored_model.clone()),
            apply_code_diffs: ActionPermission::AlwaysAllow,
            read_files: ActionPermission::AlwaysAllow,
            execute_commands: ActionPermission::AlwaysAllow,
            mcp_permissions: ActionPermission::AlwaysAllow,
            ..Default::default()
        };
        let profile_object = AIExecutionProfileObject::new(
            cloud_sync_id,
            AIExecutionProfileObjectModel::new(local_profile),
            StoredObjectMetadata::mock(),
            StoredObjectPermissions::mock_personal(),
        );

        // Insert the existing user's stored profile without per-object events
        // and emit `InitialLoadCompleted` so `AIExecutionProfilesModel`
        // reconciles to `Synced`.
        ObjectStoreModel::handle(&app).update(&mut app, move |cloud_model, ctx| {
            cloud_model.add_object(cloud_sync_id, profile_object);
            ctx.emit(ObjectStoreEvent::InitialLoadCompleted);
        });

        // Sanity: reconciliation occurred and the model now reads the
        // stored profile object.
        profile_model.read(&app, |model, ctx| {
            let info = model.default_profile(ctx);
            assert_eq!(info.sync_id(), Some(cloud_sync_id));
            assert_eq!(info.data().base_model, Some(cloud_stored_model.clone()));
        });

        // Simulate the onboarding handoff: the user picked a different
        // base_model and "None" autonomy on the agent slide, which would
        // map to every `ActionPermission` being `AlwaysAsk`.
        let onboarding_settings = SelectedSettings::AgentDrivenDevelopment {
            agent_settings: AgentDevelopmentSettings {
                selected_model_id: LLMId::from("onboarding-chosen-model"),
                autonomy: Some(AgentAutonomy::None),
                cli_agent_toolbar_enabled: true,
                session_default: onboarding::SessionDefault::Agent,
                disable_oz: false,
                show_agent_notifications: true,
            },
            project_settings: ProjectOnboardingSettings::default(),
            ui_customization: None,
        };

        app.update(|ctx| {
            apply_onboarding_settings(&onboarding_settings, ctx);
        });

        // Post-condition: the cloud profile retains its stored values.
        // Every field touched by `apply_agent_settings` should be
        // unchanged.
        profile_model.read(&app, |model, ctx| {
            let info = model.default_profile(ctx);
            assert_eq!(
                info.sync_id(),
                Some(cloud_sync_id),
                "still pointing at the existing profile object"
            );
            assert_eq!(
                info.data().base_model,
                Some(cloud_stored_model.clone()),
                "base_model should not be overwritten by onboarding for existing users"
            );
            assert_eq!(
                info.data().apply_code_diffs,
                ActionPermission::AlwaysAllow,
                "apply_code_diffs should not be overwritten by onboarding for existing users"
            );
            assert_eq!(
                info.data().read_files,
                ActionPermission::AlwaysAllow,
                "read_files should not be overwritten by onboarding for existing users"
            );
            assert_eq!(
                info.data().execute_commands,
                ActionPermission::AlwaysAllow,
                "execute_commands should not be overwritten by onboarding for existing users"
            );
            assert_eq!(
                info.data().mcp_permissions,
                ActionPermission::AlwaysAllow,
                "mcp_permissions should not be overwritten by onboarding for existing users"
            );
        });
    })
}
