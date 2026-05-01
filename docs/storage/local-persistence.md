# Local SQLite persistence (notebooks + workflows)

> **PDX-82 [A1.2.4] -- RESOLVED 2026-04-30.** All four PDX-81 gaps are now
> closed. The application-layer hydration path no longer requires a cloud
> account: a new `Owner::Local { local_id: Uuid }` variant lets
> `upsert_cloud_object` synthesize the `object_metadata` + `object_permissions`
> sidecar rows that `read_sqlite_data` joins against, and
> `to_cloud_object_permissions` knows how to reconstitute `Owner::Local` from
> a `subject_type = 'LOCAL'` row without consulting any cloud user. See
> [Resolution](#pdx-82-resolution) at the end of this document for code
> references.

PDX-81 [A1.2.3]: verifies the PDX-35 audit assertion *"Local-only storage via
SQLite (already exists in codebase)."* This document is the result of the
verification pass plus the gaps discovered while writing
`app/tests/local_notebook_persistence.rs`.

The conclusion up front: **the SQLite database itself works fine without
`warp_hosted`** -- the schema, migrations, and on-disk file are all reachable,
and a notebook/workflow row inserted via the diesel models survives a process
restart. **However**, the application code path that hydrates notebooks and
workflows on startup hard-requires a sibling row in `object_metadata`, and that
sibling row is only ever produced by the cloud-sync code (`upsert_cloud_object`
in `crates/warp_server_client`). With `warp_hosted` OFF, no code path currently
populates that table, so the running app cannot list any locally-stored
notebook/workflow even though the underlying SQLite row is intact. See
[Gaps](#gaps-vs-the-pdx-35-audit-claim) below.

## Where the database lives

Schema, models, and migrations are owned by the workspace crate
[`crates/persistence/`](../../crates/persistence). The crate auto-enables its
`local_fs` feature on every non-wasm target via
[`crates/persistence/build.rs`](../../crates/persistence/build.rs), so the
embedded migration set is always compiled in on macOS / Linux / Windows.

The connection lifecycle (open, migrate, write, read, restart) is implemented
in [`app/src/persistence/sqlite.rs`](../../app/src/persistence/sqlite.rs):

| Concern | Function | Notes |
| --- | --- | --- |
| Resolve DB path | `database_file_path()` | Joins `warp.sqlite` onto `secure_state_dir()` if available, falling back to `state_dir()`. |
| Open + migrate | `init_db()` -> `setup_database(...)` | Runs `persistence::MIGRATIONS` via `diesel_migrations`. Also migrates an old DB outside the secure container into it on first run. |
| Read on startup | `read_sqlite_data(...)` | Builds `Vec<Box<dyn CloudObject>>` from `object_metadata` joined with the per-type tables. |
| Write loop | `start_writer(...)` -> `handle_model_event(...)` | A dedicated `SQLite Writer` thread receives `ModelEvent`s over an `mpsc::sync_channel(1024)`; events are deduplicated to keep up. |
| Upsert notebook | `upsert_notebooks(conn, ...)` | Always wraps the row insert in `warp_server_client::persistence::upsert_cloud_object`, which writes both the per-type row and an `object_metadata` row. |
| Upsert workflow | `upsert_workflows(conn, ...)` | Same wrapping as notebooks. |
| Logout reset | `remove(...)` / `reconstruct(...)` | Used by Logout v0; deletes the file and re-runs migrations. |

### Concrete file locations

`warp_core::paths::project_dirs()` runs the channel-aware app id (e.g.
`dev.warp.WarpOss`) through the `directories` crate, then
`secure_state_dir()` overrides that on macOS to point inside the App Group
container when one is available. The per-platform path of `warp.sqlite` is
therefore:

- **macOS (App Group container)**:
  `~/Library/Group Containers/<group-id>/Library/Application Support/dev.warp.WarpOss/warp.sqlite`
- **macOS (no group container)**:
  `~/Library/Application Support/dev.warp.WarpOss/warp.sqlite`
- **Linux**:
  `${XDG_STATE_HOME:-$HOME/.local/state}/Warp-Oss/warp.sqlite`
  (falls back to `${XDG_DATA_HOME:-$HOME/.local/share}/Warp-Oss/warp.sqlite` on
  systems without an XDG state dir)
- **Windows**:
  `%LOCALAPPDATA%\Warp\WarpOss\data\warp.sqlite`
- **Integration tests**: `Channel::Integration` deliberately disables
  `secure_state_dir()` (see `crates/warp_core/src/paths.rs:163`) so tests stay
  inside `state_dir()` under the temp HOME they spin up.

WAL mode is on (`PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=500`), so
expect three sibling files: `warp.sqlite`, `warp.sqlite-wal`, `warp.sqlite-shm`.

## Schema overview

Final schema after all 134 embedded migrations (only the notebook / workflow
slice is shown here -- see `crates/persistence/src/schema.rs` for the rest).

```text
notebooks
+----------------------+--------------------+------------------------------+
| column               | type               | notes                        |
+----------------------+--------------------+------------------------------+
| id                   | INTEGER PK         | auto-assigned local sqlite id |
| title                | TEXT NULL          |                              |
| data                 | TEXT NULL          | serialized notebook JSON      |
| ai_document_id       | TEXT NULL          | added 2025-10-31              |
+----------------------+--------------------+------------------------------+

notebook_panes
+----------------------+--------------------+------------------------------+
| id                   | INTEGER PK         | matches pane_leaves.pane_node_id |
| kind                 | TEXT 'notebook'    | CHECK constraint              |
| notebook_id          | TEXT NULL          | sync id of the notebook       |
| local_path           | BLOB NULL          | non-utf-8-safe file path      |
+----------------------+--------------------+------------------------------+

workflows
+----------------------+--------------------+------------------------------+
| id                   | INTEGER PK         |                              |
| data                 | TEXT NOT NULL      | serialized workflow JSON      |
| user_can_delete      | BOOLEAN NOT NULL=0 | not modeled in Rust schema    |
+----------------------+--------------------+------------------------------+

workflow_panes
+----------------------+--------------------+------------------------------+
| id                   | INTEGER PK         | matches pane_leaves.pane_node_id |
| kind                 | TEXT 'workflow'    | CHECK constraint              |
| workflow_id          | TEXT NULL          | sync id of the workflow       |
+----------------------+--------------------+------------------------------+

object_metadata                                       <-- the cloud sidecar
+----------------------+--------------------+------------------------------+
| id                   | INTEGER PK         |                              |
| is_pending           | BOOLEAN NOT NULL   |                              |
| object_type          | TEXT NOT NULL      | 'NOTEBOOK' / 'WORKFLOW' / ... |
| revision_ts          | BIGINT NULL        | server revision timestamp     |
| server_id            | TEXT NULL          | hashed sync id (server side)  |
| client_id            | TEXT NULL          | hashed sync id (client side)  |
| shareable_object_id  | INTEGER NOT NULL   | -> notebooks.id / workflows.id|
| author_id            | INTEGER NULL       |                              |
| retry_count          | INTEGER NOT NULL   |                              |
| metadata_last_updated_ts | BIGINT NULL    |                              |
| trashed_ts           | BIGINT NULL        |                              |
| folder_id            | TEXT NULL          |                              |
| is_welcome_object    | BOOLEAN NOT NULL   |                              |
| creator_uid          | TEXT NULL          |                              |
| last_editor_uid      | TEXT NULL          |                              |
| current_editor       | TEXT NULL          |                              |
+----------------------+--------------------+------------------------------+
```

The Rust types backing those rows live in `crates/persistence/src/model.rs`:

- `model::Notebook` / `model::NewNotebook` -- maps the four columns above.
- `model::Workflow` / `model::NewWorkflow` -- omits `user_can_delete`, which
  the SQL keeps with `DEFAULT FALSE`.
- `model::NotebookPane`, `model::WorkflowPane`, `model::NewNotebookPane`,
  `model::NewWorkflowPane`.
- `model::ObjectMetadata` (and `NewObjectMetadata`) -- the cloud sidecar
  row that `read_sqlite_data` joins against.

`CloudObjectMetadata` (the in-memory type, in
`app/src/cloud_object/mod.rs`) carries `revision`, `last_editor_uid`,
`creator_uid`, `metadata_last_updated_ts`, etc. Those fields are sourced from
GraphQL responses in the cloud-sync code path. They are *not* synthesized
locally.

## Call paths exercised on a local-only run

1. **Notebook create (`warp_hosted` OFF, no signed-in user)**
   - UI invokes `create_notebook(...)` in `app/src/notebooks/`.
   - The notebook is wrapped in a `CloudNotebook` whose
     `CloudObjectMetadata` is constructed with `revision = None`,
     `creator_uid = None`, etc. (the "pending" defaults).
   - `ModelEvent::UpsertNotebook { notebook }` is sent to the SQLite writer
     thread.
   - The writer calls `upsert_notebooks` -> `upsert_cloud_object`. The latter
     **also** writes a row into `object_metadata` (with empty server/client
     ids on the local-only path) and a row into `object_permissions`.
2. **Notebook edit** -- same flow; `is_pending` flips to `true` until a sync
   succeeds (which never happens in `warp_hosted` OFF).
3. **App restart** -- `app::persistence::initialize` calls `init_db`, which
   re-runs migrations and then `read_sqlite_data` to rebuild
   `Vec<Box<dyn CloudObject>>`. The hydration only succeeds if the matching
   `object_metadata` row exists.
4. Workflows follow the identical pattern, with `WorkflowId` instead of
   `NotebookId`.

## Verification test

`app/tests/local_notebook_persistence.rs` is gated with
`#![cfg(not(feature = "warp_hosted"))]` and contains three tests:

1. `notebook_round_trips_across_restart` -- inserts a notebook via
   `model::NewNotebook`, drops the connection, reopens the same path, and
   asserts that title + data come back.
2. `workflow_round_trips_across_restart` -- same flow for workflows.
3. `notebook_and_workflow_inserted_locally_have_no_object_metadata` -- pins
   the gap described below: a row inserted only into `notebooks` /
   `workflows` (with no cloud sidecar) leaves `object_metadata` empty, which
   is exactly what a strict `warp_hosted=off` install would do today if no
   one short-circuits the cloud writer.

To run only the local-fs path:

```sh
cargo test -p warp --no-default-features --test local_notebook_persistence
```

## Gaps vs. the PDX-35 audit claim

The audit said local-only storage *exists*. That is true at the SQLite layer
and false at the application layer. Concrete gaps:

### Gap 1 -- `read_sqlite_data` filters out objects without an `object_metadata` row

`app/src/persistence/sqlite.rs` line ~2832 onward:

```rust
schema::workflows::dsl::workflows
    .load::<model::Workflow>(conn)?
    .iter()
    .filter_map(|workflow| {
        metadata_by_id
            .get(&(workflow.id, ObjectType::Workflow.sqlite_object_type_as_str().to_string()))
            ...
    })
```

If no `object_metadata` row exists for a notebook or workflow, the row is
silently dropped on startup. With `warp_hosted` OFF and **no** code path
that writes a local-only `object_metadata` row, every locally-created
notebook/workflow becomes invisible after a restart even though its data is
intact on disk.

The third test in the new integration test pins this behavior. Suggested
follow-up Linear issue: *"PDX-XX: emit a local-only `object_metadata` row
when `warp_hosted` is OFF, OR teach `read_sqlite_data` to fall back to the
per-type table without requiring a sidecar."*

### Gap 2 -- write path requires a `CloudObjectPermissions { owner: User|Team }`

`upsert_cloud_object` derives `subject_*` columns from
`cloud_object_permissions.owner`. Both variants
(`Owner::User { user_uid }`, `Owner::Team { team_uid }`) require a UID that
in upstream Warp comes from the auth state. With `warp_hosted` OFF there is
no signed-in user; if the upstream code passes a synthetic UID the row will
still be inserted, but it cements the assumption that "local" notebooks
have an owner concept derived from cloud auth.

This is the PDX-79 audit ambiguity #2 (CloudObject metadata coupling) showing
up in practice. Suggested follow-up: *"PDX-XX: allow `Owner::Local` and have
`upsert_cloud_object` skip permissions writes when in local-only mode."*

### Gap 3 -- the read side joins `object_permissions` too

`read_sqlite_data` calls `to_cloud_object_permissions(permissions, current_user_id)`
inside the `filter_map`. If `current_user_id` is `None` (no signed-in
user), the helper currently returns `None` for the
`Owner::User` case, which would also cause the notebook/workflow to be
skipped on startup. So even if Gap 1 is patched by writing a synthetic
`object_metadata` row, Gap 3 will still hide the data unless the helper is
taught about a local-only mode.

### Gap 4 -- `notebook_panes.local_path` exists but session restoration goes through `notebook_id`

The `local_path` column was added by
`2023-09-28-165749_local_notebook_path/up.sql` to support local file-backed
notebooks during session restoration, but the main hydration code path
keys off the cloud `notebook_id`. A user who creates and saves a local
notebook with `warp_hosted` OFF will have a `notebook_panes.local_path`
row, but the actual notebook content will not load until Gaps 1-3 are
fixed.

## Audit verdict

| Audit claim | Verdict |
| --- | --- |
| "Local-only storage via SQLite already exists in the codebase" | **Partially true.** The SQLite file, schema, and migrations all work without `warp_hosted`. The diesel models persist round-trip cleanly (proven by tests #1 and #2). |
| "...and is sufficient for an offline workflow." | **False as shipped.** The hydration path filters out anything without a cloud `object_metadata` sidecar. A locally-created notebook persists at the row level but does not reappear in the running app after a restart. (Test #3 pins this.) |

In short: the **store** is local-ready; the **load** is not. Fixing the four
gaps above is the work that PDX-81's audit was implicitly punting to a
follow-up.

## PDX-82 resolution

PDX-82 [A1.2.4] closed all four gaps above by adding an `Owner::Local`
variant and teaching the existing sidecar-write/sidecar-read code paths how
to handle it. No new tables, no schema changes -- only a new owner kind that
flows through the existing `object_metadata` + `object_permissions` plumbing.

The variant itself is feature-gated:

```rust
// crates/warp_server_client/src/cloud_object/mod.rs
pub enum Owner {
    User { user_uid: UserUid },
    Team { team_uid: ServerId },
    #[cfg(not(feature = "warp_hosted"))]
    Local { local_id: Uuid },
}
```

That keeps the cloud-side build (`warp_hosted` ON) unchanged: the existing
exhaustive matches against `Owner::User` / `Owner::Team` stay exhaustive,
and `Owner::Local` literally does not exist as a variant the cloud-sync code
can construct or pattern-match. With `warp_hosted` OFF, the variant becomes
visible and a small set of cascade arms in cloud-side files (sharing,
telemetry) become reachable as `unreachable!()` or sensible defaults.

| Gap | File:line where the fix landed | Notes |
| --- | --- | --- |
| **Gap 1** -- `read_sqlite_data` filters out objects without an `object_metadata` row | `crates/warp_server_client/src/persistence/mod.rs:33` | `upsert_cloud_object` now writes a synthetic `object_metadata` row when `Owner::Local`, with `revision_ts >= 1` and `metadata_last_updated_ts = now`. The existing JOIN in `app/src/persistence/sqlite.rs:2832` therefore matches on local rows without modification. |
| **Gap 2** -- write path requires `Owner::User` / `Owner::Team` | `crates/warp_server_client/src/persistence/mod.rs:55` | Added an `Owner::Local { local_id }` arm that maps to `("LOCAL", None, local_id.to_string())` for the `subject_type` / `subject_id` / `subject_uid` columns. |
| **Gap 3** -- `to_cloud_object_permissions` returns `None` for `Owner::User` without `current_user_id` | `app/src/persistence/sqlite.rs:3354` (`owner_for_permissions`) | Added a `"LOCAL" =>` arm that recovers the synthetic UUID from `subject_uid` and returns `Owner::Local { local_id }`. No cloud user lookup needed. |
| **Gap 4** -- `notebook_panes.local_path` is populated but hydration keys off cloud `notebook_id` | Subsumed by Gaps 1-3 | Once the notebook itself rehydrates with `Owner::Local`, the existing `notebook_panes` -> `notebook_id` join in `read_sqlite_data` resolves correctly because the synthetic `object_metadata.client_id` (a hashed `Client-<uuid>`) matches what the pane row was given at create time. The `local_path` column remains a session-restoration optimization for file-backed notebooks; it is not the primary key the hydration follows. |

### Cascade boundary decisions

Adding `Owner::Local` cascades into a small number of exhaustive `match`
sites in cloud-side files. Per the PDX-79 inline-feature-gate playbook, each
cascade was closed with a single `#[cfg(not(feature = "warp_hosted"))]` arm
that is either an `unreachable!()` (for cloud-only code that genuinely can
never see a local owner -- e.g. GraphQL conversion, Drive sharing) or a
sensible local-default mapping (for code that does run offline -- e.g.
telemetry classifies local notebooks as `PersonalCloud`, the CLI displays
`Owner::Local` as "Personal", `is_embed_accessible` rejects local objects
from team spaces).

### Verification

`app/tests/local_notebook_persistence.rs` test #3 -- previously named
`notebook_and_workflow_inserted_locally_have_no_object_metadata` and used
to *pin the gap* -- was renamed to
`local_notebook_and_workflow_get_synthetic_object_metadata` and now
*verifies the fix*: it calls `upsert_cloud_object` with `Owner::Local` for
both a notebook and a workflow, simulates a process restart, and asserts
that the sidecar rows survive with the right shape (`subject_type='LOCAL'`,
`subject_uid` round-trips the UUID, `revision_ts >= 1`,
`metadata_last_updated_ts` is non-null).

Run with:

```sh
cargo test -p warp --no-default-features --test local_notebook_persistence
```
