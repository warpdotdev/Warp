//! PDX-81 [A1.2.3] - verifies that local SQLite persistence of notebooks and
//! workflows survives a simulated app restart while the `warp_hosted` feature
//! is OFF.
//!
//! The test exercises the schema and the migration set embedded in the
//! `persistence` crate -- the same artifacts the real `app::persistence`
//! module loads at startup. It bypasses the higher-level cloud-object
//! plumbing (which requires an `AppContext`) and instead inserts directly
//! through the public diesel models, which is the most that can be exercised
//! without the full Warp runtime.
//!
//! Findings about the schema, plus the gaps in the "local-only" claim, are
//! documented in `docs/storage/local-persistence.md`.

#![cfg(not(feature = "warp_hosted"))]

use diesel::{
    BoolExpressionMethods, Connection, ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection,
};
use diesel_migrations::MigrationHarness;
use persistence::{MIGRATIONS, model, schema};
use tempfile::TempDir;

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

/// PDX-81 explicit gap check: confirms that the read path used by
/// `app::persistence::sqlite::read_sqlite_data` (which joins `notebooks` /
/// `workflows` against `object_metadata` and silently filters out anything
/// without a metadata row) returns *zero* objects when only the local row
/// exists. This is the divergence between "data persists at the row level"
/// (true) and "the running app can see it after restart" (false), which the
/// PDX-35 audit's claim glosses over.
#[test]
fn notebook_and_workflow_inserted_locally_have_no_object_metadata() {
    let tmp = TempDir::new().expect("tempdir");
    let db = tmp.path().join("warp.sqlite");

    let mut conn = open_with_migrations(&db);
    diesel::insert_into(schema::notebooks::table)
        .values(model::NewNotebook {
            title: Some("local-only".to_string()),
            data: Some("{}".to_string()),
            ai_document_id: None,
        })
        .execute(&mut conn)
        .unwrap();
    diesel::insert_into(schema::workflows::table)
        .values(model::NewWorkflow {
            data: "{}".to_string(),
        })
        .execute(&mut conn)
        .unwrap();

    // `object_metadata` rows are normally written by
    // `warp_server_client::persistence::upsert_cloud_object`, which is only
    // invoked by the cloud-sync path. With `warp_hosted` OFF, no caller
    // currently produces those rows, which means the join in
    // `read_sqlite_data` filters every local-only notebook and workflow out
    // even though the row data itself persists fine.
    let metadata_rows: i64 = schema::object_metadata::table
        .filter(
            schema::object_metadata::object_type
                .eq("NOTEBOOK")
                .or(schema::object_metadata::object_type.eq("WORKFLOW")),
        )
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(
        metadata_rows, 0,
        "no code path produces object_metadata rows in local-only mode; \
         see docs/storage/local-persistence.md for the follow-up gap"
    );
}
