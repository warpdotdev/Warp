use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use diesel::sqlite::SqliteConnection;
use diesel::RunQueryDsl;
use persistence::model::{RemoteCodebaseIndexCache, RemoteCodebaseIndexUserState};
use warpui::{Entity, SingletonEntity};

use super::schema;

const REMOTE_CODEBASE_INDEXING_SQLITE_FILE_NAME: &str = "index.sqlite";
const REMOTE_CODEBASE_INDEXING_DIR_NAME: &str = "codebase-indexes";
const SHARED_CACHE_DIR_NAME: &str = "shared";
const SNAPSHOTS_DIR_NAME: &str = "snapshots";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteCodebaseIndexingData {
    pub shared_caches: Vec<RemoteCodebaseIndexCache>,
    pub user_states: Vec<RemoteCodebaseIndexUserState>,
}

pub struct RemoteCodebaseIndexingPersistence {
    bootstrap_data: RemoteCodebaseIndexingData,
    database_path: PathBuf,
    shared_snapshots_dir: PathBuf,
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
                }
            }
        }
    }

    pub fn bootstrap_data(&self) -> &RemoteCodebaseIndexingData {
        &self.bootstrap_data
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn shared_snapshots_dir(&self) -> &Path {
        &self.shared_snapshots_dir
    }

    fn initialize_at_paths(database_path: PathBuf, shared_snapshots_dir: PathBuf) -> Result<Self> {
        ensure_remote_codebase_indexing_paths(&database_path, &shared_snapshots_dir)?;
        let mut conn = super::sqlite::setup_database(&database_path)?;
        ensure_owner_only_file(&database_path)?;
        let bootstrap_data = read_remote_codebase_indexing_data(&mut conn)
            .context("reading remote codebase indexing bootstrap data")?;

        Ok(Self {
            bootstrap_data,
            database_path,
            shared_snapshots_dir,
        })
    }
}

impl Entity for RemoteCodebaseIndexingPersistence {
    type Event = ();
}

impl SingletonEntity for RemoteCodebaseIndexingPersistence {}

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

// Remote-server daemon persistence is currently Unix-only, so match the
// existing daemon socket/cache privacy model there. Keep non-Unix builds
// compiling without trying to emulate chmod-style modes through std APIs.
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

// See the directory permission helper above for why this is a Unix-only
// permission tightening step.
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use diesel::Connection;
    use persistence::model::{NewRemoteCodebaseIndexCache, NewRemoteCodebaseIndexUserState};

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

    fn user_state_row(
        identity_key_value: &str,
        index_status_value: &str,
    ) -> NewRemoteCodebaseIndexUserState {
        let now = Utc::now().naive_utc();
        NewRemoteCodebaseIndexUserState {
            identity_key: identity_key_value.to_string(),
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

    fn test_paths() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("codebase-indexes").join("index.sqlite");
        let shared_snapshots_dir = tempdir
            .path()
            .join("codebase-indexes")
            .join("shared")
            .join("snapshots");
        (tempdir, database_path, shared_snapshots_dir)
    }

    fn seed_remote_indexing_rows(database_path: &Path) {
        std::fs::create_dir_all(
            database_path
                .parent()
                .expect("database path should have parent"),
        )
        .expect("database parent should be created");
        let mut conn = super::super::sqlite::setup_database(database_path)
            .expect("database should initialize");
        diesel::insert_into(schema::remote_codebase_index_cache::table)
            .values(cache_row(Some("root-hash")))
            .execute(&mut conn)
            .expect("cache row should insert");
        diesel::insert_into(schema::remote_codebase_index_user_state::table)
            .values(user_state_row("identity-key", "ready"))
            .execute(&mut conn)
            .expect("user state row should insert");
        diesel::insert_into(schema::remote_codebase_index_user_state::table)
            .values(user_state_row("other-identity-key", "disabled"))
            .execute(&mut conn)
            .expect("user state row should insert");
    }

    #[test]
    fn initialize_reads_empty_remote_indexing_rows() {
        let (_tempdir, database_path, shared_snapshots_dir) = test_paths();

        let persistence = RemoteCodebaseIndexingPersistence::initialize_at_paths(
            database_path,
            shared_snapshots_dir,
        )
        .expect("persistence should initialize");

        assert_eq!(persistence.bootstrap_data().shared_caches, Vec::new());
        assert_eq!(persistence.bootstrap_data().user_states, Vec::new());
    }

    #[test]
    fn initialize_reads_seeded_shared_and_identity_scoped_remote_indexing_rows() {
        let (_tempdir, database_path, shared_snapshots_dir) = test_paths();
        seed_remote_indexing_rows(&database_path);

        let persistence = RemoteCodebaseIndexingPersistence::initialize_at_paths(
            database_path.clone(),
            shared_snapshots_dir.clone(),
        )
        .expect("persistence should initialize");

        assert_eq!(persistence.database_path(), database_path);
        assert_eq!(persistence.shared_snapshots_dir(), shared_snapshots_dir);
        assert_eq!(persistence.bootstrap_data().shared_caches.len(), 1);
        assert_eq!(
            persistence.bootstrap_data().shared_caches[0]
                .root_hash
                .as_deref(),
            Some("root-hash")
        );
        let user_states = &persistence.bootstrap_data().user_states;
        assert_eq!(user_states.len(), 2);
        assert!(user_states.iter().any(|state| {
            state.identity_key == "identity-key" && state.index_status == "ready"
        }));
        assert!(user_states.iter().any(|state| {
            state.identity_key == "other-identity-key" && state.index_status == "disabled"
        }));
    }

    #[cfg(unix)]
    #[test]
    fn initialize_creates_owner_only_paths() {
        use std::os::unix::fs::PermissionsExt;

        let (_tempdir, database_path, shared_snapshots_dir) = test_paths();

        RemoteCodebaseIndexingPersistence::initialize_at_paths(
            database_path.clone(),
            shared_snapshots_dir.clone(),
        )
        .expect("persistence should initialize");

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
            diesel::insert_into(schema::remote_codebase_index_cache::table)
                .values(cache_row(Some("root-hash")))
                .execute(conn)?;
            diesel::insert_into(schema::remote_codebase_index_user_state::table)
                .values(user_state_row("identity-key", "ready"))
                .execute(conn)?;
            Ok(())
        });
    }

    #[test]
    fn setup_database_migration_enforces_remote_indexing_unique_keys() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("index.sqlite");
        let mut conn = super::super::sqlite::setup_database(&database_path)
            .expect("database should initialize");
        let cache = cache_row(Some("root-hash"));
        let user_state = user_state_row("identity-key", "ready");

        diesel::insert_into(schema::remote_codebase_index_cache::table)
            .values(&cache)
            .execute(&mut conn)
            .expect("cache row should insert");
        assert!(
            diesel::insert_into(schema::remote_codebase_index_cache::table)
                .values(&cache)
                .execute(&mut conn)
                .is_err()
        );

        diesel::insert_into(schema::remote_codebase_index_user_state::table)
            .values(&user_state)
            .execute(&mut conn)
            .expect("user state row should insert");
        assert!(
            diesel::insert_into(schema::remote_codebase_index_user_state::table)
                .values(&user_state)
                .execute(&mut conn)
                .is_err()
        );
    }
}
