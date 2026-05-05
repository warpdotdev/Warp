CREATE TABLE remote_codebase_index_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    repo_identity_key TEXT NOT NULL,
    repo_path TEXT NOT NULL,
    snapshot_version INTEGER NOT NULL,
    snapshot_file_key TEXT NOT NULL,
    root_hash TEXT,
    embedding_config_json TEXT,
    navigated_ts DATETIME,
    modified_ts DATETIME,
    queried_ts DATETIME,
    last_indexed_ts DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(repo_identity_key, repo_path)
);

CREATE TABLE remote_codebase_index_user_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    identity_key TEXT NOT NULL,
    repo_identity_key TEXT NOT NULL,
    repo_path TEXT NOT NULL,
    enablement_state TEXT NOT NULL,
    index_status TEXT NOT NULL,
    failure_reason TEXT,
    backend_association_state TEXT,
    last_ready_root_hash TEXT,
    last_status_updated_at DATETIME NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(identity_key, repo_identity_key, repo_path)
);
