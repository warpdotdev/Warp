use chrono::Utc;
use settings::manager::SettingsManager;
use warpui::{App, SingletonEntity};

use super::*;
use crate::auth::AuthStateProvider;
use crate::cloud_object::{Owner, StoredObjectMetadata, StoredObjectPermissions};
use crate::notebooks::manager::NotebookManager;
use crate::notebooks::{NotebookObject, NotebookObjectModel};
use crate::server::ids::SyncId::{self};
use crate::settings::AISettings;
use crate::workflows::workflow::Workflow;
use crate::workflows::{WorkflowObject, WorkflowObjectModel};
use crate::{
    cloud_object::update_manager::UpdateManager,
    cloud_object::{
        model::{persistence::ObjectStoreModel, view::ObjectStoreViewModel},
        Revision,
    },
    network::NetworkStatus,
    notebooks::NotebookId,
    search::data_source::Query,
    system::SystemStats,
    workflows::WorkflowId,
    workspaces::{user_profiles::UserProfiles, user_workspaces::UserWorkspaces},
};

fn mock_metadata() -> StoredObjectMetadata {
    let mut metadata = StoredObjectMetadata::mock();
    metadata.revision = Some(Revision::now());
    metadata
}

fn mock_permissions(owner: Owner) -> StoredObjectPermissions {
    StoredObjectPermissions {
        owner,
        guests: Vec::new(),
        permissions_last_updated_ts: Some(Utc::now().into()),
        anyone_with_link: None,
    }
}

fn mock_workflow(id: WorkflowId, owner: Owner) -> WorkflowObject {
    let sync_id = SyncId::ServerId(id.into());
    WorkflowObject::new(
        sync_id,
        WorkflowObjectModel::new(Workflow::new(format!("foo{id}"), format!("bar{id}"))),
        mock_metadata(),
        mock_permissions(owner),
    )
}

fn mock_notebook(id: NotebookId, owner: Owner) -> NotebookObject {
    let sync_id = SyncId::ServerId(id.into());
    NotebookObject::new(
        sync_id,
        NotebookObjectModel {
            title: format!("foo{id}"),
            data: format!("bar{id}"),
            ai_document_id: None,
            conversation_id: None,
        },
        mock_metadata(),
        mock_permissions(owner),
    )
}

fn initialize_app(app: &mut App) {
    // Add the necessary singleton models to the App
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(|ctx| UserWorkspaces::mock(vec![], ctx));
    app.add_singleton_model(ObjectStoreModel::mock);
    app.add_singleton_model(|ctx| UpdateManager::new(None, ctx));
    app.add_singleton_model(|_| UserProfiles::new(Vec::new()));
    app.add_singleton_model(ObjectStoreViewModel::new);
    app.add_singleton_model(NotebookManager::mock);
    app.add_singleton_model(|_| SettingsManager::default());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.update(crate::settings::init_and_register_user_preferences);
    app.update(AISettings::register_and_subscribe_to_events);
}

#[test]
fn test_drive_data_source_correctly_filters_drive_filter() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        // Initialize ObjectStoreModel
        ObjectStoreModel::handle(&app).update(&mut app, |model, _| {
            let notebook = mock_notebook(1.into(), Owner::mock_current_user());
            model.add_object(notebook.id, notebook);
            let workflow = mock_workflow(2.into(), Owner::mock_current_user());
            model.add_object(workflow.id, workflow);
        });

        let mixer = app.add_model(|_| CommandPaletteMixer::new());
        let data_source_handle = app.add_model(warp_drive::DataSource::new);
        mixer.update(&mut app, |mixer, ctx| {
            // Add the drive data source with the relevant filters
            mixer.add_sync_source(
                data_source_handle,
                [
                    QueryFilter::Drive,
                    QueryFilter::Notebooks,
                    QueryFilter::Workflows,
                ],
            );

            // Run the query with the drive filter
            mixer.run_query(
                Query {
                    filters: HashSet::from([QueryFilter::Drive]),
                    text: "foo".into(),
                },
                ctx,
            );
        });

        app.read(|app| {
            let results = mixer.as_ref(app).results();

            // Expect both of the results to be included
            assert_eq!(results.len(), 2);
        });
    })
}

#[test]
fn test_drive_data_source_correctly_filters_no_filter() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        // Initialize ObjectStoreModel
        ObjectStoreModel::handle(&app).update(&mut app, |model, _| {
            let notebook = mock_notebook(1.into(), Owner::mock_current_user());
            model.add_object(notebook.id, notebook);
            let workflow = mock_workflow(2.into(), Owner::mock_current_user());
            model.add_object(workflow.id, workflow);
        });
        let mixer = app.add_model(|_| CommandPaletteMixer::new());
        let data_source_handle = app.add_model(warp_drive::DataSource::new);
        mixer.update(&mut app, |mixer, ctx| {
            // Add the drive data source with the relevant filters
            mixer.add_sync_source(
                data_source_handle,
                [
                    QueryFilter::Drive,
                    QueryFilter::Notebooks,
                    QueryFilter::Workflows,
                ],
            );

            // Run the query with no filter
            mixer.run_query(
                Query {
                    filters: HashSet::new(),
                    text: "foo".into(),
                },
                ctx,
            );
        });

        app.read(|app| {
            let results = mixer.as_ref(app).results();

            // Expect both of the results to be included
            assert_eq!(results.len(), 2);
        });
    })
}

#[test]
fn test_drive_data_source_correctly_filters_workflow_filter() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        // Initialize ObjectStoreModel
        ObjectStoreModel::handle(&app).update(&mut app, |model, _| {
            let notebook = mock_notebook(1.into(), Owner::mock_current_user());
            model.add_object(notebook.id, notebook);
            let workflow = mock_workflow(2.into(), Owner::mock_current_user());
            model.add_object(workflow.id, workflow);
        });
        let mixer = app.add_model(|_| CommandPaletteMixer::new());
        let data_source_handle = app.add_model(warp_drive::DataSource::new);
        mixer.update(&mut app, |mixer, ctx| {
            // Add the drive data source with the relevant filters
            mixer.add_sync_source(
                data_source_handle,
                [
                    QueryFilter::Drive,
                    QueryFilter::Notebooks,
                    QueryFilter::Workflows,
                ],
            );

            // Run the query with no filter
            mixer.run_query(
                Query {
                    filters: HashSet::from([QueryFilter::Workflows]),
                    text: "foo".into(),
                },
                ctx,
            );
        });

        app.read(|app| {
            let results = mixer.as_ref(app).results();

            // Expect only the workflow result to be included
            assert_eq!(results.len(), 1);

            assert!(results[0].accessibility_label().starts_with("Workflow:"));
        });
    })
}

#[test]
fn test_drive_data_source_correctly_filters_notebook_filter() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        // Initialize ObjectStoreModel
        ObjectStoreModel::handle(&app).update(&mut app, |model, _| {
            let notebook = mock_notebook(1.into(), Owner::mock_current_user());
            model.add_object(notebook.id, notebook);
            let workflow = mock_workflow(2.into(), Owner::mock_current_user());
            model.add_object(workflow.id, workflow);
        });
        let mixer = app.add_model(|_| CommandPaletteMixer::new());
        let data_source_handle = app.add_model(warp_drive::DataSource::new);
        mixer.update(&mut app, |mixer, ctx| {
            // Add the drive data source with the relevant filters
            mixer.add_sync_source(
                data_source_handle,
                [
                    QueryFilter::Drive,
                    QueryFilter::Notebooks,
                    QueryFilter::Workflows,
                ],
            );

            // Run the query with no filter
            mixer.run_query(
                Query {
                    filters: HashSet::from([QueryFilter::Notebooks]),
                    text: "foo".into(),
                },
                ctx,
            );
        });

        app.read(|app| {
            let results = mixer.as_ref(app).results();

            // Expect only the workflow result to be included
            assert_eq!(results.len(), 1);

            assert!(results[0].accessibility_label().starts_with("Notebook:"));
        });
    })
}
