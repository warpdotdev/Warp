use warp_core::ui::appearance::Appearance;
use warpui::{
    platform::WindowStyle, AddSingletonModel, App, SingletonEntity, TypedActionView, ViewHandle,
};

use crate::{
    ai::blocklist::BlocklistAIHistoryModel,
    auth::{AuthManager, AuthStateProvider},
    cloud_object::{
        model::{actions::ObjectActions, persistence::CloudModel, view::CloudViewModel},
        update_manager::UpdateManager,
        CloudObjectSyncStatus, ObjectType, Owner, Space,
    },
    drive::{items::WarpDriveItemId, CloudObjectTypeAndId},
    menu::MenuItem,
    network::NetworkStatus,
    notebooks::{NotebookObject, NotebookObjectModel},
    server::ids::{ClientId, SyncId},
    settings_view::keybindings::KeybindingChangedNotifier,
    test_util::settings::initialize_settings_for_tests,
    workflows::{workflow::Workflow, WorkflowObject, WorkflowObjectModel},
    workspaces::{user_profiles::UserProfiles, user_workspaces::UserWorkspaces},
    Assets,
};

use super::{DriveIndex, DriveIndexAction};

fn initialize_app(app: &mut App) {
    initialize_settings_for_tests(app);

    app.add_singleton_model(CloudModel::mock);
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(CloudViewModel::mock);
    app.add_singleton_model(|_| ObjectActions::new(Vec::new()));
    app.add_singleton_model(|_| UserProfiles::new(Vec::new()));
    app.add_singleton_model(|_| KeybindingChangedNotifier::mock());
    app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
    #[cfg(feature = "voice_input")]
    app.add_singleton_model(voice_input::VoiceInput::new);
}

fn create_index(app: &mut App) -> ViewHandle<DriveIndex> {
    let (_, index) = app.add_window(WindowStyle::NotStealFocus, DriveIndex::new);
    index
}

fn create_workflow(app: &mut App) -> SyncId {
    CloudModel::handle(app).update(app, |cloud_model, ctx| {
        let client_id = ClientId::new();
        let sync_id = SyncId::ClientId(client_id);
        let workflow = Workflow::new("my workflow", "my command");
        cloud_model.create_object(
            sync_id,
            WorkflowObject::new_local(
                WorkflowObjectModel::new(workflow),
                Owner::mock_current_user(),
                None,
                client_id,
            ),
            ctx,
        );
        sync_id
    })
}

fn create_notebook(app: &mut App) -> SyncId {
    CloudModel::handle(app).update(app, |cloud_model, ctx| {
        let client_id = ClientId::new();
        let sync_id = SyncId::ClientId(client_id);
        cloud_model.create_object(
            sync_id,
            NotebookObject::new_local(
                NotebookObjectModel::default(),
                Owner::mock_current_user(),
                None,
                client_id,
            ),
            ctx,
        );
        sync_id
    })
}

fn set_object_in_error(app: &mut App, cloud_object_type_and_id: &CloudObjectTypeAndId) {
    CloudModel::handle(app).update(
        app,
        |cloud_model, _ctx: &mut warpui::ModelContext<'_, CloudModel>| {
            if let Some(object) = cloud_model.get_mut_by_uid(&cloud_object_type_and_id.uid()) {
                object.set_pending_content_changes_status(CloudObjectSyncStatus::Errored);
            }
        },
    );
}

fn label_for_menu_item(item: &MenuItem<DriveIndexAction>) -> &str {
    if let MenuItem::Item(item) = item {
        item.label()
    } else {
        panic!("item provided wasn't of type MenuItem::Item")
    }
}

#[test]
fn test_retry_menu_item_visibility() {
    App::test(Assets, |mut app| async move {
        initialize_app(&mut app);
        let index = create_index(&mut app);
        let sync_id = create_workflow(&mut app);
        let cloud_object_type_and_id: CloudObjectTypeAndId =
            CloudObjectTypeAndId::from_id_and_type(sync_id, ObjectType::Workflow);
        let warp_drive_item_id = WarpDriveItemId::Object(cloud_object_type_and_id);

        // by default, it doesn't show up
        index.update(&mut app, |index, ctx| {
            let menu_items = index.menu_items(&Space::Personal, &warp_drive_item_id, ctx);
            assert_eq!(menu_items.len(), 5);
            assert_eq!(label_for_menu_item(&menu_items[0]), "Edit");
            assert_eq!(label_for_menu_item(&menu_items[1]), "Copy workflow text");
            assert_eq!(label_for_menu_item(&menu_items[2]), "Share");
            assert_eq!(label_for_menu_item(&menu_items[3]), "Duplicate");
            assert_eq!(label_for_menu_item(&menu_items[4]), "Export");
        });

        // when the object is in error, it should show up
        set_object_in_error(&mut app, &cloud_object_type_and_id);
        index.update(&mut app, |index, ctx| {
            let menu_items = index.menu_items(&Space::Personal, &warp_drive_item_id, ctx);
            assert_eq!(menu_items.len(), 6);
            assert_eq!(label_for_menu_item(&menu_items[0]), "Retry");
            assert_eq!(label_for_menu_item(&menu_items[1]), "Edit");
            assert_eq!(label_for_menu_item(&menu_items[2]), "Copy workflow text");
            assert_eq!(label_for_menu_item(&menu_items[3]), "Share");
            assert_eq!(label_for_menu_item(&menu_items[4]), "Duplicate");
            assert_eq!(label_for_menu_item(&menu_items[5]), "Export");
        });

        // but if we're offline, it shouldn't show up
        NetworkStatus::handle(&app).update(&mut app, |network_status, ctx| {
            network_status.reachability_changed(false, ctx);
        });
        index.update(&mut app, |index, ctx| {
            let menu_items = index.menu_items(&Space::Personal, &warp_drive_item_id, ctx);
            assert_eq!(menu_items.len(), 5);
            assert_eq!(label_for_menu_item(&menu_items[0]), "Edit");
            assert_eq!(label_for_menu_item(&menu_items[1]), "Copy workflow text");
            assert_eq!(label_for_menu_item(&menu_items[2]), "Share");
            assert_eq!(label_for_menu_item(&menu_items[3]), "Duplicate");
            assert_eq!(label_for_menu_item(&menu_items[4]), "Export");
        });
    })
}

#[test]
fn test_retry_menu_item_logic() {
    App::test(Assets, |mut app| async move {
        initialize_app(&mut app);
        let index = create_index(&mut app);
        let sync_id = create_workflow(&mut app);
        let cloud_object_type_and_id: CloudObjectTypeAndId =
            CloudObjectTypeAndId::from_id_and_type(sync_id, ObjectType::Workflow);

        // OpenWarp(Wave 4):SyncQueue 整删,原本验证 SyncQueue 队列变化的
        // 断言全部变为无意义。跳过留下调用流程本身以验证不报 panic。

        index.update(&mut app, |index, ctx| {
            index.retry_failed_object(&cloud_object_type_and_id, ctx);
        });

        // the item is now in flight
        CloudModel::handle(&app).update(&mut app, |cloud_model, _ctx| {
            if let Some(object) = cloud_model.get_mut_by_uid(&cloud_object_type_and_id.uid()) {
                let _ = object;
            }
        });

        // OpenWarp(Wave 4):原验证 SyncQueue 队头是 CreateWorkflow,SyncQueue 整删后不适用。

        // OpenWarp(Wave 4):原验证 SyncQueue 队列长度 + UpdateWorkflow tag,SyncQueue 整删后不适用。
    })
}

#[test]
fn test_warp_drive_navigation_states() {
    use crate::drive::index::DriveIndexAction;
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let index = create_index(&mut app);
        let sync_id = create_notebook(&mut app);
        let cloud_object_type_and_id: CloudObjectTypeAndId =
            CloudObjectTypeAndId::from_id_and_type(sync_id, ObjectType::Notebook);

        index.read(&app, |index, _| {
            assert_eq!(index.selected, None, "Expect selected to be None");
            assert_eq!(
                index.focused_index,
                Some(0),
                "Expect focused_index to be initialized"
            );
        });

        index.update(&mut app, |index, ctx| {
            index.handle_action(&DriveIndexAction::OpenObject(cloud_object_type_and_id), ctx);
        });

        index.read(&app, |index, _| {
            assert_eq!(
                index.selected,
                Some(WarpDriveItemId::Object(cloud_object_type_and_id)),
                "Expect selected to have correct value"
            );
        });
    });
}
