use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use diesel::sqlite::SqliteConnection;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use persistence::model::{
    NewRemoteCodebaseIndexCache, NewRemoteCodebaseIndexUserState, RemoteCodebaseIndexCache,
    RemoteCodebaseIndexUserState,
};
use warpui::{Entity, SingletonEntity};

use super::schema;

const CHANNEL_SIZE: usize = 1024;
const REMOTE_CODEBASE_INDEXING_SQLITE_FILE_NAME: &str = "index.sqlite";
const REMOTE_CODEBASE_INDEXING_DIR_NAME: &str = "codebase-indexes";
const SHARED_CACHE_DIR_NAME: &str = "shared";
const SNAPSHOTS_DIR_NAME: &str = "snapshots";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteCodebaseIndexingData {
    pub shared_caches: Vec<RemoteCodebaseIndexCache>,
    pub user_states: Vec<RemoteCodebaseIndexUserState>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum RemoteCodebaseIndexingPersistenceEvent {
    UpsertRemoteCodebaseIndexCache {
        cache: NewRemoteCodebaseIndexCache,
    },
    DeleteRemoteCodebaseIndexCache {
        repo_identity_key: String,
        repo_path: String,
    },
    UpsertRemoteCodebaseIndexUserState {
        state: NewRemoteCodebaseIndexUserState,
    },
    DeleteRemoteCodebaseIndexUserState {
        identity_key: String,
        repo_identity_key: String,
        repo_path: String,
    },
    Terminate,
}

pub struct RemoteCodebaseIndexingPersistence {
    #[allow(dead_code)]
    bootstrap_data: RemoteCodebaseIndexingData,
    #[allow(dead_code)]
    database_path: PathBuf,
    #[allow(dead_code)]
    shared_snapshots_dir: PathBuf,
    thread_handle: Option<JoinHandle<()>>,
    event_sender: Option<SyncSender<RemoteCodebaseIndexingPersistenceEvent>>,
}

impl RemoteCodebaseIndexingPersistence {
    pub fn initialize() -> Self {
        let database_path = remote_codebase_indexing_database_path();
        let shared_snapshots_dir = remote_codebase_indexing_shared_snapshots_dir();
        match Self::initialize_at_paths(database_path.clone(), shared_snapshots_dir.clone()) {
            Ok(persistence) => persistence,
            Err(err) => {
                log::error!("Failed to initialize remote codebase indexing persistence: {err:#}");
                Self {
                    bootstrap_data: RemoteCodebaseIndexingData::default(),
                    database_path,
                    shared_snapshots_dir,
                    thread_handle: None,
                    event_sender: None,
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn bootstrap_data(&self) -> &RemoteCodebaseIndexingData {
        &self.bootstrap_data
    }
    #[allow(dead_code)]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    #[allow(dead_code)]
    pub fn shared_snapshots_dir(&self) -> &Path {
        &self.shared_snapshots_dir
    }

    #[allow(dead_code)]
    pub fn sender(&self) -> Option<SyncSender<RemoteCodebaseIndexingPersistenceEvent>> {
        self.event_sender.clone()
    }

    pub fn terminate(&mut self) {
        if let Some(handle) = self.thread_handle.take() {
            let Some(sender) = self.event_sender.take() else {
                log::error!(
                    "Remote codebase indexing persistence sender missing while thread handle is set"
                );
                return;
            };
            if let Err(err) = sender.send(RemoteCodebaseIndexingPersistenceEvent::Terminate) {
                log::error!(
                    "Could not terminate remote codebase indexing SQLite writer thread: {err}"
                );
            }
            if handle.join().is_err() {
                log::error!("Remote codebase indexing SQLite writer thread panicked");
            }
        }
    }

    fn initialize_at_paths(database_path: PathBuf, shared_snapshots_dir: PathBuf) -> Result<Self> {
        super::sqlite::initialize_sqlite_logging();
        ensure_remote_codebase_indexing_paths(&database_path, &shared_snapshots_dir)?;
        let mut conn = super::sqlite::setup_database(&database_path)?;
        ensure_owner_only_file(&database_path)?;
        let bootstrap_data = read_remote_codebase_indexing_data(&mut conn)
            .context("reading remote codebase indexing bootstrap data")?;
        let writer_handles = start_writer(conn, database_path.clone())?;

        Ok(Self {
            bootstrap_data,
            database_path,
            shared_snapshots_dir,
            thread_handle: Some(writer_handles.handle),
            event_sender: Some(writer_handles.sender),
        })
    }
}

impl Drop for RemoteCodebaseIndexingPersistence {
    fn drop(&mut self) {
        self.terminate();
    }
}

impl Entity for RemoteCodebaseIndexingPersistence {
    type Event = ();
}

impl SingletonEntity for RemoteCodebaseIndexingPersistence {}

struct RemoteCodebaseIndexingWriterHandles {
    handle: JoinHandle<()>,
    sender: SyncSender<RemoteCodebaseIndexingPersistenceEvent>,
}

fn remote_codebase_indexing_database_path() -> PathBuf {
    remote_codebase_indexing_dir().join(REMOTE_CODEBASE_INDEXING_SQLITE_FILE_NAME)
}

fn remote_codebase_indexing_shared_snapshots_dir() -> PathBuf {
    remote_codebase_indexing_dir()
        .join(SHARED_CACHE_DIR_NAME)
        .join(SNAPSHOTS_DIR_NAME)
}

fn remote_codebase_indexing_dir() -> PathBuf {
    let expanded_remote_server_dir =
        shellexpand::tilde(&remote_server::setup::remote_server_dir()).into_owned();
    PathBuf::from(expanded_remote_server_dir).join(REMOTE_CODEBASE_INDEXING_DIR_NAME)
}

fn ensure_remote_codebase_indexing_paths(
    database_path: &Path,
    shared_snapshots_dir: &Path,
) -> Result<()> {
    if let Some(parent) = database_path.parent() {
        ensure_owner_only_dir(parent)?;
    }
    if let Some(shared_dir) = shared_snapshots_dir.parent() {
        ensure_owner_only_dir(shared_dir)?;
    }
    ensure_owner_only_dir(shared_snapshots_dir)?;
    Ok(())
}

fn ensure_owner_only_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| {
        format!(
            "creating remote codebase indexing directory {}",
            path.display()
        )
    })?;
    set_owner_only_dir_permissions(path)
}

#[cfg(unix)]
fn set_owner_only_dir_permissions(path: &Path) -> Result<()> {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, Permissions::from_mode(0o700))
        .with_context(|| format!("setting permissions on directory {}", path.display()))
}

#[cfg(not(unix))]
fn set_owner_only_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn ensure_owner_only_file(path: &Path) -> Result<()> {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    if path.exists() {
        std::fs::set_permissions(path, Permissions::from_mode(0o600))
            .with_context(|| format!("setting permissions on file {}", path.display()))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_owner_only_file(_path: &Path) -> Result<()> {
    Ok(())
}

fn read_remote_codebase_indexing_data(
    conn: &mut SqliteConnection,
) -> Result<RemoteCodebaseIndexingData, diesel::result::Error> {
    Ok(RemoteCodebaseIndexingData {
        shared_caches: schema::remote_codebase_index_cache::dsl::remote_codebase_index_cache
            .load::<RemoteCodebaseIndexCache>(conn)?,
        user_states:
            schema::remote_codebase_index_user_state::dsl::remote_codebase_index_user_state
                .load::<RemoteCodebaseIndexUserState>(conn)?,
    })
}

fn start_writer(
    conn: SqliteConnection,
    database_path: PathBuf,
) -> Result<RemoteCodebaseIndexingWriterHandles> {
    let (tx, rx) = sync_channel(CHANNEL_SIZE);
    let mut conn = conn;
    let handle = thread::Builder::new()
        .name("Remote Codebase Index SQLite Writer".into())
        .spawn(move || {
            loop {
                let events = match rx.recv() {
                    Ok(event) => {
                        let mut events = vec![event];
                        events.extend(rx.try_iter());
                        events
                    }
                    Err(_) => {
                        log::warn!(
                            "Remote codebase indexing SQLite event sender closed; terminating writer thread"
                        );
                        break;
                    }
                };

                for event in events {
                    if matches!(event, RemoteCodebaseIndexingPersistenceEvent::Terminate) {
                        log::info!("Shutting down remote codebase indexing SQLite writer thread");
                        return;
                    }
                    if let Err(err) = handle_event(event, &mut conn) {
                        log::error!(
                            "Remote codebase indexing SQLite write error for {}: {err:#}",
                            database_path.display()
                        );
                    }
                }
            }
        })?;
    Ok(RemoteCodebaseIndexingWriterHandles { handle, sender: tx })
}

fn handle_event(
    event: RemoteCodebaseIndexingPersistenceEvent,
    conn: &mut SqliteConnection,
) -> Result<()> {
    match event {
        RemoteCodebaseIndexingPersistenceEvent::UpsertRemoteCodebaseIndexCache { cache } => {
            upsert_remote_codebase_index_cache(conn, cache)
                .context("upserting remote codebase index cache")
        }
        RemoteCodebaseIndexingPersistenceEvent::DeleteRemoteCodebaseIndexCache {
            repo_identity_key,
            repo_path,
        } => delete_remote_codebase_index_cache(conn, &repo_identity_key, &repo_path)
            .context("deleting remote codebase index cache"),
        RemoteCodebaseIndexingPersistenceEvent::UpsertRemoteCodebaseIndexUserState { state } => {
            upsert_remote_codebase_index_user_state(conn, state)
                .context("upserting remote codebase index user state")
        }
        RemoteCodebaseIndexingPersistenceEvent::DeleteRemoteCodebaseIndexUserState {
            identity_key,
            repo_identity_key,
            repo_path,
        } => delete_remote_codebase_index_user_state(
            conn,
            &identity_key,
            &repo_identity_key,
            &repo_path,
        )
        .context("deleting remote codebase index user state"),
        RemoteCodebaseIndexingPersistenceEvent::Terminate => {
            panic!("Unhandled remote codebase indexing writer terminate event");
        }
    }
}

fn upsert_remote_codebase_index_cache(
    conn: &mut SqliteConnection,
    cache: NewRemoteCodebaseIndexCache,
) -> Result<(), diesel::result::Error> {
    use schema::remote_codebase_index_cache::dsl::*;

    diesel::insert_into(remote_codebase_index_cache)
        .values(&cache)
        .on_conflict((repo_identity_key, repo_path))
        .do_update()
        .set(&cache)
        .execute(conn)?;

    Ok(())
}

fn delete_remote_codebase_index_cache(
    conn: &mut SqliteConnection,
    target_repo_identity_key: &str,
    target_repo_path: &str,
) -> Result<(), diesel::result::Error> {
    use schema::remote_codebase_index_cache::dsl::*;

    diesel::delete(
        remote_codebase_index_cache
            .filter(repo_identity_key.eq(target_repo_identity_key))
            .filter(repo_path.eq(target_repo_path)),
    )
    .execute(conn)?;

    Ok(())
}

fn upsert_remote_codebase_index_user_state(
    conn: &mut SqliteConnection,
    state: NewRemoteCodebaseIndexUserState,
) -> Result<(), diesel::result::Error> {
    use schema::remote_codebase_index_user_state::dsl::*;

    diesel::insert_into(remote_codebase_index_user_state)
        .values(&state)
        .on_conflict((identity_key, repo_identity_key, repo_path))
        .do_update()
        .set(&state)
        .execute(conn)?;

    Ok(())
}

fn delete_remote_codebase_index_user_state(
    conn: &mut SqliteConnection,
    target_identity_key: &str,
    target_repo_identity_key: &str,
    target_repo_path: &str,
) -> Result<(), diesel::result::Error> {
    use schema::remote_codebase_index_user_state::dsl::*;

    diesel::delete(
        remote_codebase_index_user_state
            .filter(identity_key.eq(target_identity_key))
            .filter(repo_identity_key.eq(target_repo_identity_key))
            .filter(repo_path.eq(target_repo_path)),
    )
    .execute(conn)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use diesel::Connection;

    use super::*;

    fn cache_row(repo_root_hash: Option<&str>) -> NewRemoteCodebaseIndexCache {
        let now = Utc::now().naive_utc();
        NewRemoteCodebaseIndexCache {
            repo_identity_key: "repo-key".to_string(),
            repo_path: "/repo".to_string(),
            snapshot_version: 1,
            snapshot_file_key: "snapshot-key".to_string(),
            root_hash: repo_root_hash.map(str::to_string),
            embedding_config_json: Some("{\"model\":\"test\"}".to_string()),
            navigated_ts: Some(now),
            modified_ts: Some(now),
            queried_ts: None,
            last_indexed_ts: Some(now),
            updated_at: now,
        }
    }

    fn user_state_row(index_status_value: &str) -> NewRemoteCodebaseIndexUserState {
        let now = Utc::now().naive_utc();
        NewRemoteCodebaseIndexUserState {
            identity_key: "identity-key".to_string(),
            repo_identity_key: "repo-key".to_string(),
            repo_path: "/repo".to_string(),
            enablement_state: "enabled".to_string(),
            index_status: index_status_value.to_string(),
            failure_reason: None,
            backend_association_state: Some("associated".to_string()),
            last_ready_root_hash: Some("root-hash".to_string()),
            last_status_updated_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn initialize_reads_empty_remote_indexing_rows() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("codebase-indexes").join("index.sqlite");
        let shared_snapshots_dir = tempdir
            .path()
            .join("codebase-indexes")
            .join("shared")
            .join("snapshots");

        let mut persistence = RemoteCodebaseIndexingPersistence::initialize_at_paths(
            database_path,
            shared_snapshots_dir,
        )
        .expect("persistence should initialize");

        assert!(persistence.bootstrap_data().shared_caches.is_empty());
        assert!(persistence.bootstrap_data().user_states.is_empty());
        persistence.terminate();
    }

    #[test]
    fn writer_upserts_and_deletes_remote_indexing_rows() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("codebase-indexes").join("index.sqlite");
        let shared_snapshots_dir = tempdir
            .path()
            .join("codebase-indexes")
            .join("shared")
            .join("snapshots");

        let mut persistence = RemoteCodebaseIndexingPersistence::initialize_at_paths(
            database_path.clone(),
            shared_snapshots_dir.clone(),
        )
        .expect("persistence should initialize");
        let sender = persistence.sender().expect("writer sender should exist");
        sender
            .send(
                RemoteCodebaseIndexingPersistenceEvent::UpsertRemoteCodebaseIndexCache {
                    cache: cache_row(Some("root-hash")),
                },
            )
            .expect("cache event should send");
        sender
            .send(
                RemoteCodebaseIndexingPersistenceEvent::UpsertRemoteCodebaseIndexUserState {
                    state: user_state_row("ready"),
                },
            )
            .expect("user state event should send");
        persistence.terminate();

        let mut conn =
            super::super::sqlite::setup_database(&database_path).expect("database should reopen");
        let data = read_remote_codebase_indexing_data(&mut conn).expect("rows should read");
        assert_eq!(data.shared_caches.len(), 1);
        assert_eq!(
            data.shared_caches[0].root_hash.as_deref(),
            Some("root-hash")
        );
        assert_eq!(data.user_states.len(), 1);
        assert_eq!(data.user_states[0].index_status, "ready");

        let mut persistence = RemoteCodebaseIndexingPersistence::initialize_at_paths(
            database_path,
            shared_snapshots_dir,
        )
        .expect("persistence should reinitialize");
        assert_eq!(persistence.bootstrap_data().shared_caches.len(), 1);
        assert_eq!(persistence.bootstrap_data().user_states.len(), 1);
        let sender = persistence.sender().expect("writer sender should exist");
        sender
            .send(
                RemoteCodebaseIndexingPersistenceEvent::DeleteRemoteCodebaseIndexCache {
                    repo_identity_key: "repo-key".to_string(),
                    repo_path: "/repo".to_string(),
                },
            )
            .expect("cache delete event should send");
        sender
            .send(
                RemoteCodebaseIndexingPersistenceEvent::DeleteRemoteCodebaseIndexUserState {
                    identity_key: "identity-key".to_string(),
                    repo_identity_key: "repo-key".to_string(),
                    repo_path: "/repo".to_string(),
                },
            )
            .expect("user state delete event should send");
        persistence.terminate();

        let mut conn = super::super::sqlite::setup_database(persistence.database_path())
            .expect("database should reopen");
        let data = read_remote_codebase_indexing_data(&mut conn).expect("rows should read");
        assert!(data.shared_caches.is_empty());
        assert!(data.user_states.is_empty());
    }

    #[test]
    fn direct_upsert_updates_existing_remote_indexing_rows() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("index.sqlite");
        let mut conn = super::super::sqlite::setup_database(&database_path)
            .expect("database should initialize");

        upsert_remote_codebase_index_cache(&mut conn, cache_row(Some("old-root")))
            .expect("cache insert should succeed");
        upsert_remote_codebase_index_cache(&mut conn, cache_row(Some("new-root")))
            .expect("cache update should succeed");
        upsert_remote_codebase_index_user_state(&mut conn, user_state_row("indexing"))
            .expect("user state insert should succeed");
        upsert_remote_codebase_index_user_state(&mut conn, user_state_row("ready"))
            .expect("user state update should succeed");

        let data = read_remote_codebase_indexing_data(&mut conn).expect("rows should read");
        assert_eq!(data.shared_caches.len(), 1);
        assert_eq!(data.shared_caches[0].root_hash.as_deref(), Some("new-root"));
        assert_eq!(data.user_states.len(), 1);
        assert_eq!(data.user_states[0].index_status, "ready");
    }

    #[cfg(unix)]
    #[test]
    fn initialize_creates_owner_only_paths() {
        use std::os::unix::fs::PermissionsExt;

        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("codebase-indexes").join("index.sqlite");
        let shared_snapshots_dir = tempdir
            .path()
            .join("codebase-indexes")
            .join("shared")
            .join("snapshots");

        let mut persistence = RemoteCodebaseIndexingPersistence::initialize_at_paths(
            database_path.clone(),
            shared_snapshots_dir.clone(),
        )
        .expect("persistence should initialize");
        persistence.terminate();

        let codebase_index_dir = database_path.parent().expect("database should have parent");
        assert_eq!(
            std::fs::metadata(codebase_index_dir)
                .expect("codebase index directory should exist")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(shared_snapshots_dir)
                .expect("shared snapshots directory should exist")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(database_path)
                .expect("database file should exist")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn setup_database_migration_creates_remote_indexing_tables() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("index.sqlite");
        let mut conn = super::super::sqlite::setup_database(&database_path)
            .expect("database should initialize");

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            upsert_remote_codebase_index_cache(conn, cache_row(Some("root-hash")))?;
            upsert_remote_codebase_index_user_state(conn, user_state_row("ready"))?;
            Ok(())
        });
    }
}
