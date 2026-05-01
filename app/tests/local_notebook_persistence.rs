//! PDX-82 [A1.2.4] - verifies that local SQLite persistence of notebooks and
//! workflows survives a simulated app restart while the `warp_hosted` feature
//! is OFF, AND that the application-layer hydration path
//! (`object_metadata` + `object_permissions` sidecars) is wired up correctly
//! through `upsert_cloud_object` with the new `Owner::Local` variant.
//!
//! Test #3 used to be a *gap-documenting* assertion (it asserted that
//! `object_metadata` was empty for local-only rows); PDX-82 flipped it to a
//! *fix-verifying* assertion: the sidecar rows are now populated by the
//! application-layer write path even with no cloud account.
//!
//! Findings about the schema, plus the fixes for the four PDX-81 gaps, are
//! documented in `docs/storage/local-persistence.md`.

#![cfg(not(feature = "warp_hosted"))]

use diesel::{
    Connection, ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection,
};
use diesel_migrations::MigrationHarness;
use persistence::{MIGRATIONS, model, schema};
use tempfile::TempDir;
use uuid::Uuid;

use warp_server_client::cloud_object::{
    CloudObjectMetadata, CloudObjectPermissions, CloudObjectStatuses, CloudObjectSyncStatus,
    ObjectType, Owner,
};
use warp_server_client::ids::{ClientId, SyncId};
use warp_server_client::persistence::upsert_cloud_object;

/// Opens a SQLite connection at `path`, applies WAL + FK pragmas, and runs all
/// embedded migrations. Mirrors what `app::persistence::sqlite::setup_database`
/// does, but stripped of the production telemetry/log integration.
fn open_with_migrations(path: &std::path::Path) -> SqliteConnection {
    let url = path.to_str().expect("temp path should be utf-8");
    let mut conn =
        SqliteConnection::establish(url).expect("should establish a sqlite connection");
    diesel::connection::SimpleConnection::batch_execute(
        &mut conn,
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;",
    )
    .expect("pragmas should apply");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("embedded migrations should apply cleanly");
    conn
}

/// Builds a `CloudObjectMetadata` with all-`None` cloud fields, mirroring how
/// a brand-new local notebook would look at create time (no revision, no
/// editor, no folder).
fn local_only_metadata() -> CloudObjectMetadata {
    CloudObjectMetadata {
        revision: None,
        current_editor_uid: None,
        metadata_last_updated_ts: None,
        pending_changes_statuses: CloudObjectStatuses {
            content_sync_status: CloudObjectSyncStatus::NoLocalChanges,
            has_pending_permissions_change: false,
            has_pending_metadata_change: false,
            pending_untrash: false,
            pending_delete: false,
        },
        trashed_ts: None,
        folder_id: None,
        is_welcome_object: false,
        last_editor_uid: None,
        creator_uid: None,
        last_task_run_ts: None,
    }
}

/// Builds a `CloudObjectPermissions` whose owner is `Owner::Local` --
/// the new PDX-82 variant that lets a local notebook/workflow round-trip
/// through `upsert_cloud_object` without a cloud user/team.
fn local_only_permissions(local_id: Uuid) -> CloudObjectPermissions {
    CloudObjectPermissions {
        owner: Owner::Local { local_id },
        permissions_last_updated_ts: None,
        guests: Vec::new(),
        anyone_with_link: None,
    }
}

#[test]
fn notebook_round_trips_across_restart() {
    let tmp = TempDir::new().expect("tempdir");
    let db = tmp.path().join("warp.sqlite");

    // First "session": insert one notebook directly through the diesel model.
    {
        let mut conn = open_with_migrations(&db);
        let new_notebook = model::NewNotebook {
            title: Some("PDX-81 test".to_string()),
            data: Some(
                r#"{"blocks":[{"id":"b1","kind":"text","content":"hello local sqlite"}]}"#
                    .to_string(),
            ),
            ai_document_id: None,
        };
        diesel::insert_into(schema::notebooks::table)
            .values(&new_notebook)
            .execute(&mut conn)
            .expect("notebook insert succeeds with no warp-server columns");
        // `conn` drops here, simulating app shutdown. SQLite WAL is checkpointed
        // implicitly when the last connection closes.
    }

    // Second "session": reopen and verify what survived.
    {
        let mut conn = open_with_migrations(&db);
        let rows = schema::notebooks::table
            .load::<model::Notebook>(&mut conn)
            .expect("re-reading notebooks should succeed");
        assert_eq!(rows.len(), 1, "exactly one notebook should persist");
        let n = &rows[0];
        assert_eq!(n.title.as_deref(), Some("PDX-81 test"));
        assert_eq!(
            n.data.as_deref(),
            Some(
                r#"{"blocks":[{"id":"b1","kind":"text","content":"hello local sqlite"}]}"#
            )
        );
        assert!(n.ai_document_id.is_none());
    }
}

#[test]
fn workflow_round_trips_across_restart() {
    let tmp = TempDir::new().expect("tempdir");
    let db = tmp.path().join("warp.sqlite");

    let workflow_payload = r#"{"name":"PDX-81 wf","command":"echo hello"}"#;

    {
        let mut conn = open_with_migrations(&db);
        let new_workflow = model::NewWorkflow {
            data: workflow_payload.to_string(),
        };
        diesel::insert_into(schema::workflows::table)
            .values(&new_workflow)
            .execute(&mut conn)
            .expect("workflow insert succeeds with no warp-server columns");
    }

    {
        let mut conn = open_with_migrations(&db);
        let rows = schema::workflows::table
            .load::<model::Workflow>(&mut conn)
            .expect("re-reading workflows should succeed");
        assert_eq!(rows.len(), 1, "exactly one workflow should persist");
        assert_eq!(rows[0].data, workflow_payload);
    }
}

/// PDX-82 fix-verification: the application-layer write path
/// (`upsert_cloud_object`) MUST populate a synthetic `object_metadata` row
/// + `object_permissions` row when called with `Owner::Local`. Before PDX-82
/// this was the gap that left local-only notebooks invisible after restart;
/// after PDX-82 it is the mechanism that makes them visible.
#[test]
fn local_notebook_and_workflow_get_synthetic_object_metadata() {
    let tmp = TempDir::new().expect("tempdir");
    let db = tmp.path().join("warp.sqlite");

    let local_owner_id = Uuid::new_v4();
    let notebook_sync_id = SyncId::ClientId(ClientId::new());
    let workflow_sync_id = SyncId::ClientId(ClientId::new());

    // First "session": create a notebook and a workflow via the same
    // application-layer entry point (`upsert_cloud_object`) the real app uses,
    // both owned by `Owner::Local`.
    {
        let mut conn = open_with_migrations(&db);

        // Notebook.
        upsert_cloud_object(
            &mut conn,
            ObjectType::Notebook,
            notebook_sync_id,
            local_only_metadata(),
            local_only_permissions(local_owner_id),
            Box::new(|c| {
                diesel::insert_into(schema::notebooks::table)
                    .values(model::NewNotebook {
                        title: Some("local-only".to_string()),
                        data: Some("{}".to_string()),
                        ai_document_id: None,
                    })
                    .execute(c)?;
                schema::notebooks::table
                    .select(schema::notebooks::id)
                    .order(schema::notebooks::id.desc())
                    .first(c)
            }),
            Box::new(|_, _| Ok(())),
        )
        .expect("upsert_cloud_object for a local notebook should succeed");

        // Workflow.
        upsert_cloud_object(
            &mut conn,
            ObjectType::Workflow,
            workflow_sync_id,
            local_only_metadata(),
            local_only_permissions(local_owner_id),
            Box::new(|c| {
                diesel::insert_into(schema::workflows::table)
                    .values(model::NewWorkflow {
                        data: r#"{"name":"local-wf"}"#.to_string(),
                    })
                    .execute(c)?;
                schema::workflows::table
                    .select(schema::workflows::id)
                    .order(schema::workflows::id.desc())
                    .first(c)
            }),
            Box::new(|_, _| Ok(())),
        )
        .expect("upsert_cloud_object for a local workflow should succeed");
    }

    // Second "session": reopen and verify the sidecar rows survive the
    // simulated restart with the right shape.
    {
        let mut conn = open_with_migrations(&db);

        // Sidecar rows exist for both objects.
        let metadata_rows: Vec<model::ObjectMetadata> = schema::object_metadata::table
            .load::<model::ObjectMetadata>(&mut conn)
            .expect("re-reading object_metadata should succeed");
        assert_eq!(
            metadata_rows.len(),
            2,
            "PDX-82 fix: upsert_cloud_object must write object_metadata sidecars \
             for both Owner::Local notebook and workflow"
        );

        // Both rows carry a non-null synthetic revision (>= 1) so the
        // hydration filter in `read_sqlite_data` doesn't drop them.
        for m in &metadata_rows {
            assert!(
                m.revision_ts.is_some_and(|ts| ts >= 1),
                "expected synthetic revision >= 1, got {:?} for {}",
                m.revision_ts,
                m.object_type
            );
            assert!(
                m.metadata_last_updated_ts.is_some(),
                "expected synthetic metadata_last_updated_ts, got None for {}",
                m.object_type
            );
            assert!(
                !m.is_pending,
                "no in-flight content changes were declared, so is_pending must be false"
            );
        }

        // Permissions rows exist for both sidecars and carry the LOCAL owner
        // tag plus the original UUID we passed in -- this is what
        // `to_cloud_object_permissions` reads to reconstruct `Owner::Local`
        // on hydration without consulting any cloud user.
        let permission_rows: Vec<model::ObjectPermissions> = schema::object_permissions::table
            .load::<model::ObjectPermissions>(&mut conn)
            .expect("re-reading object_permissions should succeed");
        assert_eq!(permission_rows.len(), 2);
        for p in &permission_rows {
            assert_eq!(
                p.subject_type, "LOCAL",
                "PDX-82: Owner::Local must serialize as subject_type='LOCAL'"
            );
            assert_eq!(
                p.subject_uid,
                local_owner_id.to_string(),
                "subject_uid round-trips the Owner::Local UUID"
            );
            assert!(
                p.subject_id.is_none(),
                "Owner::Local has no cloud user_uid, so subject_id stays NULL"
            );
        }

        // Notebook + workflow data themselves still round-trip through their
        // per-type tables -- this is the original PDX-81 invariant.
        let notebook_rows = schema::notebooks::table
            .load::<model::Notebook>(&mut conn)
            .expect("re-reading notebooks should succeed");
        assert_eq!(notebook_rows.len(), 1);
        assert_eq!(notebook_rows[0].title.as_deref(), Some("local-only"));

        let workflow_rows = schema::workflows::table
            .load::<model::Workflow>(&mut conn)
            .expect("re-reading workflows should succeed");
        assert_eq!(workflow_rows.len(), 1);
        assert_eq!(workflow_rows[0].data, r#"{"name":"local-wf"}"#);
    }
}
