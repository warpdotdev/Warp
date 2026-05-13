use chrono::Utc;
use lazy_static::lazy_static;
use settings::{RespectUserSyncSetting, SyncToCloud};
use warpui::{App, ModelHandle};

use crate::auth::AuthManager;
use crate::auth::AuthStateProvider;
use crate::auth::UserUid;
use crate::auth::TEST_USER_UID;
use crate::cloud_object::model::actions::ObjectActions;
use crate::cloud_object::model::generic_string_model::GenericStringModel;
use crate::cloud_object::model::view::EditorState;
use crate::cloud_object::model::view::ObjectStoreViewModel;
use crate::cloud_object::model::view::UpdateTimestamp;
use crate::cloud_object::model::view::EDITOR_TIMEOUT_DURATION_MINUTES;
use crate::cloud_object::CloudObjectMetadata;
use crate::cloud_object::CloudObjectPermissions;
use crate::cloud_object::CloudObjectStatuses;
use crate::cloud_object::CloudObjectSyncStatus;
use crate::cloud_object::Owner;
use crate::drive::folders::FolderObjectModel;
use crate::drive::DriveIndexVariant;
use crate::features::FeatureFlag;
use crate::notebooks::NotebookObjectModel;
use crate::server::ids::ClientId;
use crate::server::ids::ServerId;
use crate::settings::init_and_register_user_preferences;
use crate::settings::Preference;
use crate::system::SystemStats;
use crate::workspaces::team::Team;
use crate::workspaces::user_profiles::UserProfiles;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::Workspace;

use crate::workspaces::workspace::WorkspaceUid;
use crate::NetworkStatus;
use crate::UpdateManager;

use super::*;

fn create_cloud_model(
    app: &mut App,
    objects: Vec<Box<dyn CloudObject>>,
) -> ModelHandle<ObjectStoreModel> {
    // Make sure to register the ObjectStoreModel singleton - some CloudObject methods
    // find it and other dependencies via the AppContext.
    app.add_singleton_model(|_ctx| ObjectStoreModel::new(None, objects, None))
}

lazy_static! {
    /// Mock the user being on _a_ team in tests, so that the team drive is available.
    /// Otherwise, any team objects will appear shared.
    static ref TEST_TEAM: Team = Team::from_local_cache(
        ServerId::from(1),
        "Test Team".to_string(),
        None,
        None,
        None,
    );

    static ref TEST_WORKSPACE: Workspace = Workspace::from_local_cache(
        WorkspaceUid::from(ServerId::from(1)),
        "Test Workspace".to_string(),
        Some(vec![TEST_TEAM.clone()]),
    );
}

fn initialize_app(app: &mut App, cached_objects: Vec<Box<dyn CloudObject>>) {
    // Add the necessary singleton models to the App
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(|ctx| UserWorkspaces::mock(vec![TEST_WORKSPACE.clone()], ctx));
    app.add_singleton_model(|_ctx| ObjectStoreModel::new(None, cached_objects, None));
    app.add_singleton_model(|ctx| UpdateManager::new(None, ctx));
    app.add_singleton_model(|_| UserProfiles::new(Vec::new()));
    app.add_singleton_model(ObjectStoreViewModel::new);
    app.add_singleton_model(|_| ObjectActions::new(Vec::new()));
}

fn mock_stored_metadata() -> CloudObjectMetadata {
    let mut metadata = CloudObjectMetadata::mock();
    metadata.revision = Some(Revision::now());
    metadata.metadata_last_updated_ts = Some(Utc::now().into());
    metadata
}

fn mock_permissions() -> CloudObjectPermissions {
    CloudObjectPermissions {
        owner: Owner::mock_current_user(),
        guests: Vec::new(),
        permissions_last_updated_ts: None,
        anyone_with_link: None,
    }
}

fn mock_cloud_folder(id: SyncId, name: String, folder_id: Option<SyncId>) -> FolderObject {
    FolderObject::new(
        id,
        FolderObjectModel {
            name,
            is_open: true,
            is_warp_pack: false,
        },
        CloudObjectMetadata {
            pending_changes_statuses: CloudObjectStatuses {
                content_sync_status: CloudObjectSyncStatus::NoLocalChanges,
                has_pending_metadata_change: false,
                has_pending_permissions_change: false,
                pending_untrash: false,
                pending_delete: false,
            },
            folder_id,
            revision: Default::default(),
            metadata_last_updated_ts: Default::default(),
            current_editor_uid: Default::default(),
            trashed_ts: Default::default(),
            is_welcome_object: false,
            creator_uid: None,
            last_editor_uid: None,
            last_task_run_ts: None,
        },
        mock_permissions(),
    )
}

fn mock_cloud_notebook(id: SyncId, title: String, folder_id: Option<SyncId>) -> NotebookObject {
    NotebookObject::new(
        id,
        NotebookObjectModel {
            title,
            data: "test".into(),
            ai_document_id: None,
            conversation_id: None,
        },
        CloudObjectMetadata {
            pending_changes_statuses: CloudObjectStatuses {
                content_sync_status: CloudObjectSyncStatus::NoLocalChanges,
                has_pending_metadata_change: false,
                has_pending_permissions_change: false,
                pending_untrash: false,
                pending_delete: false,
            },
            folder_id,
            revision: Default::default(),
            metadata_last_updated_ts: Default::default(),
            current_editor_uid: Default::default(),
            trashed_ts: Default::default(),
            is_welcome_object: false,
            creator_uid: None,
            last_editor_uid: None,
            last_task_run_ts: None,
        },
        mock_permissions(),
    )
}

fn mock_trashed_cloud_folder(id: SyncId, name: String, folder_id: Option<SyncId>) -> FolderObject {
    let mut folder = mock_cloud_folder(id, name, folder_id);
    folder.metadata.trashed_ts = Some(ServerTimestamp::from_unix_timestamp_micros(10).unwrap());
    folder
}

fn folder_from_cloud_model(model: &ObjectStoreModel, id: SyncId) -> &FolderObject {
    model.get_folder_by_uid(&id.uid()).expect("is a folder")
}

/// Mock receiving an RTC update. These tests update objects by mocking RTC messages so that they
/// don't need to mock the server API for updates. The unit tests for [`UpdateManager`] ensure that
/// updates from both RTC and client actions emit the same events.
// OpenWarp(本地化,Phase 2d-4a-1):RTC 入口 `received_message_from_server` 随 `Listener`
// 一并物理删除,以下依赖 `receive_rtc_update` / `move_object` helper 的 4 个
// folder 排序时间戳测试(test_update_folder_timestamp_from_*)与 helper 同一删除,
// 本地写入路径下的 metadata 更新由 `ObjectStoreModel` 直接接手,无需 RTC 路径。

#[test]
fn test_create_json_object() {
    let client_id = ClientId::default();
    let id = SyncId::ClientId(client_id);
    let json_object: Box<dyn CloudObject> = Box::new(CloudPreference::new(
        id,
        GenericStringModel::new(
            Preference::new(
                "test_storage_key".to_owned(),
                "{\"test_key\": \"test_value\"}",
                SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            )
            .expect("error creating preference"),
        ),
        CloudObjectMetadata {
            pending_changes_statuses: CloudObjectStatuses {
                content_sync_status: CloudObjectSyncStatus::NoLocalChanges,
                has_pending_metadata_change: false,
                has_pending_permissions_change: false,
                pending_untrash: false,
                pending_delete: false,
            },
            folder_id: Default::default(),
            revision: Default::default(),
            metadata_last_updated_ts: Default::default(),
            current_editor_uid: Default::default(),
            trashed_ts: Default::default(),
            is_welcome_object: false,
            creator_uid: None,
            last_editor_uid: None,
            last_task_run_ts: None,
        },
        mock_permissions(),
    ));

    App::test((), |mut app| async move {
        let cloud_model = create_cloud_model(&mut app, vec![json_object]);
        cloud_model.read(&app, |model, _| {
            let json_object: &CloudPreference =
                model.get_object_of_type(&id).expect("model should exist");
            assert_eq!(
                json_object.model().string_model.storage_key,
                "test_storage_key".to_owned()
            );
        });
    })
}

fn check_cloud_folders(app: &mut App, number_of_folders: usize) {
    ObjectStoreModel::handle(app).read(app, |model, _| {
        assert_eq!(
            number_of_folders,
            model.get_all_active_and_inactive_folders().count(),
            "we expected {} folders, and received {}",
            number_of_folders,
            model.get_all_active_and_inactive_folders().count()
        );
    });
}

fn check_cloud_workflows(app: &mut App, number_of_workflows: usize) {
    ObjectStoreModel::handle(app).read(app, |model, _| {
        assert_eq!(
            number_of_workflows,
            model.get_all_active_and_inactive_workflows().count(),
            "we expected {} workflows, and received {}",
            number_of_workflows,
            model.get_all_active_and_inactive_workflows().count()
        );
    });
}

fn check_cloud_notebooks(app: &mut App, number_of_notebooks: usize) {
    ObjectStoreModel::handle(app).read(app, |model, _| {
        assert_eq!(
            number_of_notebooks,
            model.get_all_active_and_inactive_notebooks().count(),
            "we expected {} notebooks, and received {}",
            number_of_notebooks,
            model.get_all_active_and_inactive_notebooks().count()
        );
    });
}

#[test]
fn test_collapse_all_in_location() {
    /*
       the folder structure looks like:

       test1
        ↳ test 4
         ↳ test 5
       test 2
        ↳ test 6
         ↳ test 7
       test 3

    */
    let folder_1_id: SyncId = SyncId::ServerId(1.into());
    let folder_2_id: SyncId = SyncId::ServerId(2.into());
    let folder_3_id: SyncId = SyncId::ServerId(3.into());
    let folder_4_id: SyncId = SyncId::ServerId(4.into());
    let folder_5_id: SyncId = SyncId::ServerId(5.into());
    let folder_6_id: SyncId = SyncId::ServerId(6.into());
    let folder_7_id: SyncId = SyncId::ServerId(7.into());

    let folders = vec![
        mock_cloud_folder(folder_1_id, "test1".to_string(), None),
        mock_cloud_folder(folder_2_id, "test2".to_string(), None),
        mock_cloud_folder(folder_3_id, "test3".to_string(), None),
        mock_cloud_folder(folder_4_id, "test4".to_string(), Some(folder_1_id)),
        mock_cloud_folder(folder_5_id, "test5".to_string(), Some(folder_4_id)),
        mock_cloud_folder(folder_6_id, "test6".to_string(), Some(folder_2_id)),
        mock_cloud_folder(folder_7_id, "test7".to_string(), Some(folder_6_id)),
    ]
    .into_iter()
    .map(|o| Box::new(o) as Box<dyn CloudObject>)
    .collect();

    App::test((), |mut app| async move {
        app.add_singleton_model(UserWorkspaces::default_mock);
        let cloud_model = create_cloud_model(&mut app, folders);

        cloud_model.update(&mut app, |model, ctx| {
            // first, collapse all folders in folder 1
            model.collapse_all_in_location(
                CloudObjectLocation::Folder(folder_1_id),
                DriveIndexVariant::MainIndex,
                ctx,
            );

            // folders 1, 4, and 5 should be collapsed
            let folder_1 = folder_from_cloud_model(model, folder_1_id);
            let folder_4 = folder_from_cloud_model(model, folder_4_id);
            let folder_5 = folder_from_cloud_model(model, folder_5_id);
            assert!(!folder_1.model.is_open);
            assert!(!folder_4.model.is_open);
            assert!(!folder_5.model.is_open);
            // but the others are still open
            let folder_2 = folder_from_cloud_model(model, folder_2_id);
            let folder_3 = folder_from_cloud_model(model, folder_3_id);
            let folder_6 = folder_from_cloud_model(model, folder_6_id);
            let folder_7 = folder_from_cloud_model(model, folder_7_id);
            assert!(folder_2.model.is_open);
            assert!(folder_3.model.is_open);
            assert!(folder_6.model.is_open);
            assert!(folder_7.model.is_open);

            model.collapse_all_in_location(
                CloudObjectLocation::Space(Default::default()),
                DriveIndexVariant::MainIndex,
                ctx,
            );
            // now all folders in this space are collapsed
            let folder_1 = folder_from_cloud_model(model, folder_1_id);
            let folder_2 = folder_from_cloud_model(model, folder_2_id);
            let folder_3 = folder_from_cloud_model(model, folder_3_id);
            let folder_4 = folder_from_cloud_model(model, folder_4_id);
            let folder_5 = folder_from_cloud_model(model, folder_5_id);
            let folder_6 = folder_from_cloud_model(model, folder_6_id);
            let folder_7 = folder_from_cloud_model(model, folder_7_id);
            assert!(!folder_1.model.is_open);
            assert!(!folder_2.model.is_open);
            assert!(!folder_3.model.is_open);
            assert!(!folder_4.model.is_open);
            assert!(!folder_5.model.is_open);
            assert!(!folder_6.model.is_open);
            assert!(!folder_7.model.is_open);
        });
    })
}

#[test]
fn test_collapse_all_in_trash() {
    /*
       the folder structure looks like:

       test1 -- trashed by user
        ↳ test 4
         ↳ test 5 -- trashed by user
       test 2 -- trashed by user
        ↳ test 6
         ↳ test 7
       test 3 -- trashed by user

       the structure in the trash index looks like:

       test1 -- trashed by user
        ↳ test 4
       test 5 -- trashed by user
       test 2 -- trashed by user
        ↳ test 6
         ↳ test 7
       test 3 -- trashed by user

    */
    let folder_1_id: SyncId = SyncId::ServerId(1.into());
    let folder_2_id: SyncId = SyncId::ServerId(2.into());
    let folder_3_id: SyncId = SyncId::ServerId(3.into());
    let folder_4_id: SyncId = SyncId::ServerId(4.into());
    let folder_5_id: SyncId = SyncId::ServerId(5.into());
    let folder_6_id: SyncId = SyncId::ServerId(6.into());
    let folder_7_id: SyncId = SyncId::ServerId(7.into());

    let folders = vec![
        mock_trashed_cloud_folder(folder_1_id, "test1".to_string(), None),
        mock_trashed_cloud_folder(folder_2_id, "test2".to_string(), None),
        mock_trashed_cloud_folder(folder_3_id, "test3".to_string(), None),
        mock_cloud_folder(folder_4_id, "test4".to_string(), Some(folder_1_id)),
        mock_trashed_cloud_folder(folder_5_id, "test5".to_string(), Some(folder_4_id)),
        mock_cloud_folder(folder_6_id, "test6".to_string(), Some(folder_2_id)),
        mock_cloud_folder(folder_7_id, "test7".to_string(), Some(folder_6_id)),
    ]
    .into_iter()
    .map(|o| Box::new(o) as Box<dyn CloudObject>)
    .collect();

    App::test((), |mut app| async move {
        app.add_singleton_model(UserWorkspaces::default_mock);
        let cloud_model = create_cloud_model(&mut app, folders);

        cloud_model.update(&mut app, |model, ctx| {
            // first, collapse all folders in folder 1
            model.collapse_all_in_location(
                CloudObjectLocation::Folder(folder_1_id),
                DriveIndexVariant::Trash,
                ctx,
            );

            // folders 1, 4 should be collapsed
            let folder_1 = folder_from_cloud_model(model, folder_1_id);
            let folder_4 = folder_from_cloud_model(model, folder_4_id);
            assert!(!folder_1.model.is_open);
            assert!(!folder_4.model.is_open);
            // but the others, including folder 5, are still open
            let folder_2 = folder_from_cloud_model(model, folder_2_id);
            let folder_3 = folder_from_cloud_model(model, folder_3_id);
            let folder_5 = folder_from_cloud_model(model, folder_5_id);
            let folder_6 = folder_from_cloud_model(model, folder_6_id);
            let folder_7 = folder_from_cloud_model(model, folder_7_id);
            assert!(folder_2.model.is_open);
            assert!(folder_3.model.is_open);
            assert!(folder_5.model.is_open);
            assert!(folder_6.model.is_open);
            assert!(folder_7.model.is_open);

            model.collapse_all_in_location(
                CloudObjectLocation::Space(Default::default()),
                DriveIndexVariant::Trash,
                ctx,
            );
            // now all folders in this space are collapsed
            let folder_1 = folder_from_cloud_model(model, folder_1_id);
            let folder_2 = folder_from_cloud_model(model, folder_2_id);
            let folder_3 = folder_from_cloud_model(model, folder_3_id);
            let folder_4 = folder_from_cloud_model(model, folder_4_id);
            let folder_5 = folder_from_cloud_model(model, folder_5_id);
            let folder_6 = folder_from_cloud_model(model, folder_6_id);
            let folder_7 = folder_from_cloud_model(model, folder_7_id);
            assert!(!folder_1.model.is_open);
            assert!(!folder_2.model.is_open);
            assert!(!folder_3.model.is_open);
            assert!(!folder_4.model.is_open);
            assert!(!folder_5.model.is_open);
            assert!(!folder_6.model.is_open);
            assert!(!folder_7.model.is_open);
        });
    })
}

#[test]
fn test_object_editor_timeout() {
    App::test((), |mut app| async move {
        initialize_app(&mut app, Vec::new());
        let notebook_id: SyncId = SyncId::ServerId(1.into());
        let cloud_notebook = mock_cloud_notebook(notebook_id, "test1".into(), None);

        ObjectStoreModel::handle(&app).update(&mut app, |model, _ctx| {
            // Add a notebook to ObjectStoreModel
            model.add_object(notebook_id, cloud_notebook.clone());

            let notebook = model
                .get_notebook_mut(&notebook_id)
                .expect("notebook should exist");

            // Set the editor to be somebody else.
            notebook.metadata.current_editor_uid = Some("ian@warp.dev".to_string());
        });

        let current_editor = ObjectStoreViewModel::handle(&app).read(&app, |view_model, ctx| {
            view_model
                .object_current_editor(&notebook_id.uid(), ctx)
                .expect("expect editor to be set")
        });
        // Assert that the current editor is an active other user
        assert_eq!(current_editor.state, EditorState::OtherUserActive);

        ObjectStoreModel::handle(&app).update(&mut app, |model, _ctx| {
            let notebook = model
                .get_notebook_mut(&notebook_id)
                .expect("notebook should exist");

            // Set the notebook timesteps to be more than the timeout
            let timeout_timestamp = Utc::now()
                - chrono::Duration::minutes(EDITOR_TIMEOUT_DURATION_MINUTES)
                - chrono::Duration::seconds(1);
            notebook.metadata.revision = Some(Revision::from(timeout_timestamp));
            notebook.metadata.metadata_last_updated_ts = Some(timeout_timestamp.into());
        });

        let current_editor = ObjectStoreViewModel::handle(&app).read(&app, |view_model, ctx| {
            view_model
                .object_current_editor(&notebook_id.uid(), ctx)
                .expect("expect editor to be set")
        });
        // Assert that the current editor is an idle other user
        assert_eq!(current_editor.state, EditorState::OtherUserIdle);
    });
}

#[test]
fn test_breadcrumbs() {
    let folder_1_id: SyncId = SyncId::ServerId(1.into());
    let folder_2_id: SyncId = SyncId::ServerId(2.into());
    let folder_3_id: SyncId = SyncId::ServerId(3.into());

    let folders = vec![
        mock_cloud_folder(folder_1_id, "test1".to_string(), None),
        mock_cloud_folder(folder_2_id, "test2".to_string(), Some(folder_1_id)),
        mock_cloud_folder(folder_3_id, "test3".to_string(), Some(folder_2_id)),
    ]
    .into_iter()
    .map(|f| Box::new(f) as Box<dyn CloudObject>)
    .collect::<Vec<_>>();

    App::test((), |mut app| async move {
        initialize_app(&mut app, folders.clone());

        ObjectStoreModel::handle(&app).read(&app, |_, ctx| {
            assert_eq!("Personal".to_string(), folders[0].breadcrumbs(ctx));
            assert_eq!("Personal / test1".to_string(), folders[1].breadcrumbs(ctx));
            assert_eq!(
                "Personal / test1 / test2".to_string(),
                folders[2].breadcrumbs(ctx)
            );
        });
    });
}

/// Asserts that the object with the given ID has the expected sorting timestamp.
#[track_caller]
fn assert_sorting_timestamp(id: ServerId, expected_ts: impl Into<ServerTimestamp>, app: &App) {
    let sorting_timestamp = app.read(|ctx| {
        let object = ObjectStoreModel::as_ref(ctx).get_by_uid(&id.uid())?;
        ObjectStoreViewModel::as_ref(ctx).object_sorting_timestamp(
            object,
            UpdateTimestamp::Revision,
            ctx,
        )
    });
    assert_eq!(
        sorting_timestamp,
        Some(expected_ts.into()),
        "Unexpected timestamp for {}",
        id.uid()
    );
}

#[test]
fn test_shared_personal_object() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app, Vec::new());

        let other_user = UserUid::new("other_user");
        let shared_notebook_id = SyncId::ServerId(123.into());
        let shared_notebook = NotebookObject::new(
            shared_notebook_id,
            NotebookObjectModel {
                title: "Shared Notebook".to_string(),
                data: "Hello".to_string(),
                ai_document_id: None,
                conversation_id: None,
            },
            mock_stored_metadata(),
            CloudObjectPermissions {
                owner: Owner::User {
                    user_uid: other_user,
                },
                guests: Vec::new(),
                permissions_last_updated_ts: None,
                anyone_with_link: None,
            },
        );

        ObjectStoreModel::handle(&app).update(&mut app, |cloud_model, ctx| {
            cloud_model.add_object(shared_notebook_id, shared_notebook);

            let space = cloud_model
                .get_notebook(&shared_notebook_id)
                .expect("Notebook is in ObjectStoreModel")
                .space(ctx);
            assert_eq!(space, Space::Shared);
        });
    });
}

#[test]
fn test_unshared_personal_object() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app, Vec::new());

        let shared_notebook_id = SyncId::ServerId(123.into());
        let shared_notebook = NotebookObject::new(
            shared_notebook_id,
            NotebookObjectModel {
                title: "Shared Notebook".to_string(),
                data: "Hello".to_string(),
                ai_document_id: None,
                conversation_id: None,
            },
            mock_stored_metadata(),
            CloudObjectPermissions {
                owner: Owner::User {
                    user_uid: UserUid::new(TEST_USER_UID),
                },
                guests: Vec::new(),
                permissions_last_updated_ts: None,
                anyone_with_link: None,
            },
        );

        ObjectStoreModel::handle(&app).update(&mut app, |cloud_model, ctx| {
            cloud_model.add_object(shared_notebook_id, shared_notebook);

            let space = cloud_model
                .get_notebook(&shared_notebook_id)
                .expect("Notebook is in ObjectStoreModel")
                .space(ctx);
            assert_eq!(space, Space::Personal);
        });
    });
}

#[test]
fn test_shared_team_object() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app, Vec::new());

        // The user is not on this team.
        let team_uid = ServerId::from(456);

        let shared_notebook_id = SyncId::ServerId(123.into());
        let shared_notebook = NotebookObject::new(
            shared_notebook_id,
            NotebookObjectModel {
                title: "Shared Notebook".to_string(),
                data: "Hello".to_string(),
                ai_document_id: None,
                conversation_id: None,
            },
            mock_stored_metadata(),
            CloudObjectPermissions {
                owner: Owner::Team { team_uid },
                guests: Vec::new(),
                permissions_last_updated_ts: None,
                anyone_with_link: None,
            },
        );

        ObjectStoreModel::handle(&app).update(&mut app, |cloud_model, ctx| {
            cloud_model.add_object(shared_notebook_id, shared_notebook);

            let space = cloud_model
                .get_notebook(&shared_notebook_id)
                .expect("Notebook is in ObjectStoreModel")
                .space(ctx);
            assert_eq!(space, Space::Shared);
        });
    });
}

#[test]
fn test_unshared_team_object() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);
    App::test((), |mut app| async move {
        app.update(init_and_register_user_preferences);
        initialize_app(&mut app, Vec::new());

        // Use the current user's team.
        let team_uid = TEST_TEAM.uid;
        let shared_notebook_id = SyncId::ServerId(123.into());
        let shared_notebook = NotebookObject::new(
            shared_notebook_id,
            NotebookObjectModel {
                title: "Shared Notebook".to_string(),
                data: "Hello".to_string(),
                ai_document_id: None,
                conversation_id: None,
            },
            mock_stored_metadata(),
            CloudObjectPermissions {
                owner: Owner::Team { team_uid },
                guests: Vec::new(),
                permissions_last_updated_ts: None,
                anyone_with_link: None,
            },
        );

        ObjectStoreModel::handle(&app).update(&mut app, |cloud_model, ctx| {
            cloud_model.add_object(shared_notebook_id, shared_notebook);

            let space = cloud_model
                .get_notebook(&shared_notebook_id)
                .expect("Notebook is in ObjectStoreModel")
                .space(ctx);
            assert_eq!(space, Space::Team { team_uid });
        });
    });
}

#[test]
fn test_shared_object_in_unshared_folder() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);
    App::test((), |mut app| async move {
        app.update(init_and_register_user_preferences);
        initialize_app(&mut app, Vec::new());

        let other_user = UserUid::new("other_user");
        let unshared_folder_id = SyncId::ServerId(567.into());
        let shared_notebook_id = SyncId::ServerId(123.into());
        let mut shared_notebook = NotebookObject::new(
            shared_notebook_id,
            NotebookObjectModel {
                title: "Shared Notebook".to_string(),
                data: "Hello".to_string(),
                ai_document_id: None,
                conversation_id: None,
            },
            mock_stored_metadata(),
            CloudObjectPermissions {
                owner: Owner::User {
                    user_uid: other_user,
                },
                guests: Vec::new(),
                permissions_last_updated_ts: None,
                anyone_with_link: None,
            },
        );
        shared_notebook.metadata_mut().folder_id = Some(unshared_folder_id);

        ObjectStoreModel::handle(&app).update(&mut app, |cloud_model, ctx| {
            cloud_model.add_object(shared_notebook_id, shared_notebook);
            let notebook = cloud_model
                .get_notebook(&shared_notebook_id)
                .expect("Notebook is in ObjectStoreModel");

            // Check space-based APIs.
            assert_eq!(notebook.space(ctx), Space::Shared);
            assert!(notebook.is_in_space(Space::Shared, ctx));

            // Check location-based APIs.
            assert_eq!(
                notebook.location(cloud_model, ctx),
                CloudObjectLocation::Space(Space::Shared)
            );
            assert!(notebook.metadata.folder_id.is_some());

            // Despite the missing parent folder, the notebook is not trashed.
            assert!(!notebook.is_trashed(cloud_model));

            // Check that iteration APIs include the notebook where it's expected.
            assert!(cloud_model
                .active_cloud_objects_in_space(Space::Shared, ctx)
                .any(|obj| obj.uid() == notebook.uid()));
            assert!(cloud_model
                .active_cloud_objects_in_location_without_descendents(
                    CloudObjectLocation::Space(Space::Shared),
                    ctx
                )
                .any(|obj| obj.uid() == notebook.uid()));
            assert_eq!(
                cloud_model
                    .trashed_cloud_objects_in_space(Space::Shared, ctx)
                    .count(),
                0
            );
            assert_eq!(
                cloud_model
                    .trashed_cloud_objects_in_location_without_descendents(
                        CloudObjectLocation::Space(Space::Shared),
                        ctx
                    )
                    .count(),
                0
            );

            let folder_location = CloudObjectLocation::Folder(unshared_folder_id);
            assert_eq!(
                cloud_model
                    .active_cloud_objects_in_location_without_descendents(folder_location, ctx)
                    .count(),
                0
            );
            assert_eq!(
                cloud_model
                    .trashed_cloud_objects_in_location_without_descendents(folder_location, ctx)
                    .count(),
                0
            );
        });
    });
}

/// Helper: compute active UIDs using the naive (non-memoized) is_trashed approach.
fn naive_active_object_uids(model: &ObjectStoreModel) -> HashSet<String> {
    model
        .as_cloud_objects()
        .filter(|obj| !obj.is_trashed(model))
        .map(|obj| obj.uid())
        .collect()
}

#[test]
fn active_object_uids_matches_naive_with_no_trashed_objects() {
    let folder_id = SyncId::ServerId(1.into());
    let objects: Vec<Box<dyn CloudObject>> = vec![
        Box::new(mock_cloud_folder(folder_id, "Folder".into(), None)),
        Box::new(mock_cloud_notebook(
            SyncId::ServerId(2.into()),
            "Notebook".into(),
            Some(folder_id),
        )),
    ];

    App::test((), |mut app| async move {
        let cloud_model = create_cloud_model(&mut app, objects);
        cloud_model.read(&app, |model, _| {
            assert_eq!(model.active_object_uids(), naive_active_object_uids(model));
            assert_eq!(model.active_object_uids().len(), 2);
        });
    });
}

#[test]
fn active_object_uids_matches_naive_with_directly_trashed_object() {
    let trashed_folder_id = SyncId::ServerId(1.into());
    let active_notebook_id = SyncId::ServerId(2.into());
    let objects: Vec<Box<dyn CloudObject>> = vec![
        Box::new(mock_trashed_cloud_folder(
            trashed_folder_id,
            "Trashed Folder".into(),
            None,
        )),
        Box::new(mock_cloud_notebook(
            active_notebook_id,
            "Active Notebook".into(),
            None,
        )),
    ];

    App::test((), |mut app| async move {
        let cloud_model = create_cloud_model(&mut app, objects);
        cloud_model.read(&app, |model, _| {
            let active = model.active_object_uids();
            assert_eq!(active, naive_active_object_uids(model));
            assert_eq!(active.len(), 1);
            assert!(active.contains(&active_notebook_id.uid()));
            assert!(!active.contains(&trashed_folder_id.uid()));
        });
    });
}

#[test]
fn active_object_uids_matches_naive_with_indirectly_trashed_children() {
    // A trashed folder with a non-trashed notebook inside it.
    // The notebook should be considered trashed (indirectly) by both approaches.
    let trashed_folder_id = SyncId::ServerId(1.into());
    let child_notebook_id = SyncId::ServerId(2.into());
    let active_notebook_id = SyncId::ServerId(3.into());
    let objects: Vec<Box<dyn CloudObject>> = vec![
        Box::new(mock_trashed_cloud_folder(
            trashed_folder_id,
            "Trashed Folder".into(),
            None,
        )),
        Box::new(mock_cloud_notebook(
            child_notebook_id,
            "Child in Trashed Folder".into(),
            Some(trashed_folder_id),
        )),
        Box::new(mock_cloud_notebook(
            active_notebook_id,
            "Top-level Notebook".into(),
            None,
        )),
    ];

    App::test((), |mut app| async move {
        let cloud_model = create_cloud_model(&mut app, objects);
        cloud_model.read(&app, |model, _| {
            let active = model.active_object_uids();
            assert_eq!(active, naive_active_object_uids(model));
            assert_eq!(active.len(), 1);
            assert!(active.contains(&active_notebook_id.uid()));
        });
    });
}

#[test]
fn active_object_uids_matches_naive_with_nested_trashed_folder() {
    // folder_a (trashed) -> folder_b (not trashed) -> notebook (not trashed)
    // Both folder_b and notebook should be indirectly trashed.
    let folder_a_id = SyncId::ServerId(1.into());
    let folder_b_id = SyncId::ServerId(2.into());
    let notebook_id = SyncId::ServerId(3.into());
    let active_notebook_id = SyncId::ServerId(4.into());
    let objects: Vec<Box<dyn CloudObject>> = vec![
        Box::new(mock_trashed_cloud_folder(
            folder_a_id,
            "Folder A (trashed)".into(),
            None,
        )),
        Box::new(mock_cloud_folder(
            folder_b_id,
            "Folder B".into(),
            Some(folder_a_id),
        )),
        Box::new(mock_cloud_notebook(
            notebook_id,
            "Deeply nested".into(),
            Some(folder_b_id),
        )),
        Box::new(mock_cloud_notebook(
            active_notebook_id,
            "Active".into(),
            None,
        )),
    ];

    App::test((), |mut app| async move {
        let cloud_model = create_cloud_model(&mut app, objects);
        cloud_model.read(&app, |model, _| {
            let active = model.active_object_uids();
            assert_eq!(active, naive_active_object_uids(model));
            assert_eq!(active.len(), 1);
            assert!(active.contains(&active_notebook_id.uid()));
        });
    });
}

#[test]
fn active_object_uids_matches_naive_with_empty_model() {
    App::test((), |mut app| async move {
        let cloud_model = create_cloud_model(&mut app, vec![]);
        cloud_model.read(&app, |model, _| {
            let active = model.active_object_uids();
            assert_eq!(active, naive_active_object_uids(model));
            assert!(active.is_empty());
        });
    });
}
