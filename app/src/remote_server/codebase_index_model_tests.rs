use super::*;

fn host() -> HostId {
    HostId::new("host".to_string())
}

fn remote_path(repo_path: &str) -> RemotePath {
    remote_path_from_repo_path(&host(), repo_path).unwrap()
}

fn ready_status(repo_path: &str) -> RemoteCodebaseIndexStatus {
    RemoteCodebaseIndexStatus {
        repo_path: repo_path.to_string(),
        state: RemoteCodebaseIndexState::Ready,
        last_updated_epoch_millis: Some(1),
        progress_completed: None,
        progress_total: None,
        failure_message: None,
        root_hash: Some(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        ),
    }
}

fn status_with_state(
    repo_path: &str,
    state: RemoteCodebaseIndexState,
) -> RemoteCodebaseIndexStatus {
    let mut status = ready_status(repo_path);
    status.state = state;
    status
}
fn status_with_path(repo_path: &str) -> RemoteCodebaseIndexStatusWithPath {
    RemoteCodebaseIndexStatusWithPath {
        remote_path: remote_path(repo_path),
        status: ready_status(repo_path),
    }
}

#[test]
fn snapshot_replaces_statuses_for_host() {
    let mut model = RemoteCodebaseIndexModel::default();
    let host = host();
    model.apply_status_update(remote_path("/old"), ready_status("/old"));
    model.apply_statuses_snapshot(&host, &[status_with_path("/new")]);

    assert!(model.status_for_repo(&remote_path("/old")).is_none());
    assert!(model.status_for_repo(&remote_path("/new")).is_some());
}

#[test]
fn availability_uses_active_navigated_repo() {
    let mut model = RemoteCodebaseIndexModel::default();
    let host = host();
    model.record_navigated_directory(&remote_path("/repo"));
    model.apply_status_update(remote_path("/repo"), ready_status("/repo"));

    let availability = model.availability_for_remote(&host, Some("/repo/src"), None);

    assert!(availability.is_ready());
    assert_eq!(availability.repo_path(), Some("/repo"));
}

#[test]
fn availability_uses_active_navigated_non_git_directory() {
    let mut model = RemoteCodebaseIndexModel::default();
    let host = host();
    model.record_navigated_directory(&remote_path("/directory"));
    model.apply_status_update(remote_path("/directory"), ready_status("/directory"));

    let availability = model.availability_for_remote(&host, Some("/repo/src"), None);

    assert!(availability.is_ready());
    assert_eq!(availability.repo_path(), Some("/directory"));
}

#[test]
fn availability_falls_back_to_longest_status_prefix() {
    let mut model = RemoteCodebaseIndexModel::default();
    let host = host();
    model.apply_status_update(remote_path("/repo"), ready_status("/repo"));
    model.apply_status_update(remote_path("/repo/nested"), ready_status("/repo/nested"));

    let availability = model.availability_for_remote(&host, Some("/repo/nested/src"), None);

    assert!(availability.is_ready());
    assert_eq!(availability.repo_path(), Some("/repo/nested"));
}

#[test]
fn availability_rejects_unmatched_requested_local_path() {
    let mut model = RemoteCodebaseIndexModel::default();
    let host = host();
    model.record_navigated_directory(&remote_path("/workspaces/warp"));
    model.apply_status_update(
        remote_path("/workspaces/warp"),
        ready_status("/workspaces/warp"),
    );

    let availability = model.availability_for_remote(
        &host,
        Some("/workspaces/warp"),
        Some("/Users/moirahuang/code/warp"),
    );

    assert!(matches!(
        availability,
        RemoteCodebaseSearchAvailability::NoActiveRepo
    ));
    assert_eq!(availability.repo_path(), None);
}

#[test]
fn availability_rejects_unknown_requested_remote_path() {
    let mut model = RemoteCodebaseIndexModel::default();
    let host = host();
    model.record_navigated_directory(&remote_path("/workspaces/active"));
    model.apply_status_update(
        remote_path("/workspaces/active"),
        ready_status("/workspaces/active"),
    );

    let availability =
        model.availability_for_remote(&host, Some("/workspaces/active"), Some("/workspaces/other"));

    assert!(matches!(
        availability,
        RemoteCodebaseSearchAvailability::NoActiveRepo
    ));
    assert_eq!(availability.repo_path(), None);
}

#[test]
fn availability_uses_requested_path_when_it_matches_known_remote_repo() {
    let mut model = RemoteCodebaseIndexModel::default();
    let host = host();
    model.record_navigated_directory(&remote_path("/workspaces/other"));
    model.apply_status_update(
        remote_path("/workspaces/other"),
        ready_status("/workspaces/other"),
    );
    model.apply_status_update(
        remote_path("/workspaces/warp"),
        ready_status("/workspaces/warp"),
    );

    let availability = model.availability_for_remote(
        &host,
        Some("/workspaces/other"),
        Some("/workspaces/warp/app"),
    );

    assert!(availability.is_ready());
    assert_eq!(availability.repo_path(), Some("/workspaces/warp"));
}
#[test]
fn codebases_for_agent_context_includes_searchable_remote_paths() {
    let mut model = RemoteCodebaseIndexModel::default();
    model.apply_status_update(
        remote_path("/workspaces/warp"),
        ready_status("/workspaces/warp"),
    );
    model.apply_status_update(
        remote_path("/workspaces/stale"),
        status_with_state("/workspaces/stale", RemoteCodebaseIndexState::Stale),
    );

    let entries = model.codebases_for_agent_context();

    assert_eq!(
        entries,
        vec![
            RemoteCodebaseContextEntry {
                name: "stale".to_string(),
                path: "/workspaces/stale".to_string(),
            },
            RemoteCodebaseContextEntry {
                name: "warp".to_string(),
                path: "/workspaces/warp".to_string(),
            },
        ]
    );
}

#[test]
fn codebases_for_agent_context_skips_unsearchable_remote_paths() {
    let mut model = RemoteCodebaseIndexModel::default();
    let mut missing_root_hash = ready_status("/workspaces/missing-root-hash");
    missing_root_hash.root_hash = None;
    model.apply_status_update(
        remote_path("/workspaces/missing-root-hash"),
        missing_root_hash,
    );
    model.apply_status_update(
        remote_path("/workspaces/indexing"),
        status_with_state("/workspaces/indexing", RemoteCodebaseIndexState::Indexing),
    );
    model.apply_status_update(
        remote_path("/workspaces/failed"),
        status_with_state("/workspaces/failed", RemoteCodebaseIndexState::Failed),
    );

    assert!(model.codebases_for_agent_context().is_empty());
}

#[test]
fn resolve_remote_repo_path_falls_back_to_current_remote_cwd_when_no_repo_is_known() {
    let model = RemoteCodebaseIndexModel::default();
    let host = host();

    let remote_path = model.resolve_remote_repo_path(&host, Some("/workspaces/new"), None);

    assert_eq!(
        remote_path.map(|remote_path| remote_path.path.as_str().to_string()),
        Some("/workspaces/new".to_string())
    );
}

#[test]
fn indexing_state_is_not_ready() {
    let mut status = ready_status("/repo");
    status.state = RemoteCodebaseIndexState::Indexing;

    let availability = search_availability_for_status(&status, remote_path("/repo"));

    assert!(matches!(
        availability,
        RemoteCodebaseSearchAvailability::Indexing { .. }
    ));
}

#[test]
fn missing_root_hash_is_unavailable() {
    let mut status = ready_status("/repo");
    status.root_hash = None;

    let availability = search_availability_for_status(&status, remote_path("/repo"));

    assert!(matches!(
        availability,
        RemoteCodebaseSearchAvailability::Unavailable { .. }
    ));
}

#[test]
fn auto_index_navigated_git_repo_when_status_is_missing() {
    let model = RemoteCodebaseIndexModel::default();

    assert!(model.should_request_auto_index_for_navigated_git_repo(&remote_path("/repo")));
}

#[test]
fn auto_index_navigated_git_repo_skips_existing_searchable_index() {
    let mut model = RemoteCodebaseIndexModel::default();
    model.apply_status_update(remote_path("/ready"), ready_status("/ready"));
    model.apply_status_update(
        remote_path("/stale"),
        status_with_state("/stale", RemoteCodebaseIndexState::Stale),
    );

    assert!(!model.should_request_auto_index_for_navigated_git_repo(&remote_path("/ready")));
    assert!(!model.should_request_auto_index_for_navigated_git_repo(&remote_path("/stale")));
}

#[test]
fn auto_index_navigated_git_repo_skips_index_already_in_progress() {
    let mut model = RemoteCodebaseIndexModel::default();
    model.apply_status_update(
        remote_path("/queued"),
        status_with_state("/queued", RemoteCodebaseIndexState::Queued),
    );
    model.apply_status_update(
        remote_path("/indexing"),
        status_with_state("/indexing", RemoteCodebaseIndexState::Indexing),
    );

    assert!(!model.should_request_auto_index_for_navigated_git_repo(&remote_path("/queued")));
    assert!(!model.should_request_auto_index_for_navigated_git_repo(&remote_path("/indexing")));
}

#[test]
fn auto_index_navigated_git_repo_when_existing_index_is_unusable() {
    let mut model = RemoteCodebaseIndexModel::default();
    let mut missing_root_hash = ready_status("/missing-root-hash");
    missing_root_hash.root_hash = None;
    model.apply_status_update(remote_path("/missing-root-hash"), missing_root_hash);
    model.apply_status_update(
        remote_path("/failed"),
        status_with_state("/failed", RemoteCodebaseIndexState::Failed),
    );

    assert!(
        model.should_request_auto_index_for_navigated_git_repo(&remote_path("/missing-root-hash"))
    );
    assert!(model.should_request_auto_index_for_navigated_git_repo(&remote_path("/failed")));
}

#[test]
fn remote_auto_indexing_requires_feature_codebase_context_and_auto_indexing() {
    {
        let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
        let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(false);
        assert!(!remote_auto_indexing_enabled(true, true));
    }
    {
        let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
        let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
        assert!(remote_auto_indexing_enabled(true, true));
        assert!(!remote_auto_indexing_enabled(false, true));
        assert!(!remote_auto_indexing_enabled(true, false));
        assert!(!remote_auto_indexing_enabled(false, false));
    }
    {
        let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(false);
        let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
        assert!(!remote_auto_indexing_enabled(true, true));
    }
}
