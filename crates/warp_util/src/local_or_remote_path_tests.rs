use crate::{
    host_id::HostId, local_or_remote_path::LocalOrRemotePath, remote_path::RemotePath,
    standardized_path::StandardizedPath,
};

fn test_host_id() -> HostId {
    HostId::new("test-host".to_string())
}
#[test]
fn local_or_remote_path_helpers_return_local_path_components() {
    let path = LocalOrRemotePath::Local("/tmp/repo/file.txt".into());

    assert_eq!(path.display_name(), "file.txt");
    assert_eq!(
        path.path_component(),
        StandardizedPath::try_new("/tmp/repo/file.txt").unwrap()
    );
    assert_eq!(path.display_path(), "/tmp/repo/file.txt");
    assert_eq!(
        path.to_local_path(),
        Some(std::path::Path::new("/tmp/repo/file.txt"))
    );
}

#[test]
fn local_or_remote_path_helpers_return_remote_path_components() {
    let path = LocalOrRemotePath::Remote(RemotePath::new(
        test_host_id(),
        StandardizedPath::try_new("/tmp/repo/file.txt").unwrap(),
    ));

    assert_eq!(path.display_name(), "file.txt");
    assert_eq!(
        path.path_component(),
        StandardizedPath::try_new("/tmp/repo/file.txt").unwrap()
    );
    assert_eq!(path.display_path(), "/tmp/repo/file.txt");
    assert_eq!(path.to_local_path(), None);
}

#[test]
fn local_or_remote_path_is_local_and_is_remote_classify_variants() {
    let local = LocalOrRemotePath::Local("/tmp/repo".into());
    let remote = LocalOrRemotePath::Remote(RemotePath::new(
        test_host_id(),
        StandardizedPath::try_new("/tmp/repo").unwrap(),
    ));

    assert!(local.is_local());
    assert!(!local.is_remote());
    assert!(!remote.is_local());
    assert!(remote.is_remote());
}

#[test]
fn local_or_remote_path_join_preserves_host_for_remote() {
    let local = LocalOrRemotePath::Local("/repo".into());
    let remote = LocalOrRemotePath::Remote(RemotePath::new(
        test_host_id(),
        StandardizedPath::try_new("/repo").unwrap(),
    ));

    let local_joined = local.join("src/foo.rs");
    assert_eq!(
        local_joined.path_component(),
        StandardizedPath::try_new("/repo/src/foo.rs").unwrap()
    );
    assert!(local_joined.is_local());

    let remote_joined = remote.join("src/foo.rs");
    assert_eq!(
        remote_joined.path_component(),
        StandardizedPath::try_new("/repo/src/foo.rs").unwrap()
    );
    assert!(remote_joined.is_remote());
    if let LocalOrRemotePath::Remote(remote) = remote_joined {
        assert_eq!(remote.host_id, test_host_id());
    } else {
        panic!("expected remote variant");
    }
}

#[test]
fn local_or_remote_path_join_with_absolute_replaces_prefix() {
    let local = LocalOrRemotePath::Local("/some/repo".into());
    let remote = LocalOrRemotePath::Remote(RemotePath::new(
        test_host_id(),
        StandardizedPath::try_new("/some/repo").unwrap(),
    ));

    let abs = "/server/repo/src/foo.rs";

    // Path::join replacement semantics on absolute argument.
    let local_joined = local.join(abs);
    assert_eq!(
        local_joined.path_component(),
        StandardizedPath::try_new("/server/repo/src/foo.rs").unwrap()
    );

    let remote_joined = remote.join(abs);
    assert_eq!(
        remote_joined.path_component(),
        StandardizedPath::try_new("/server/repo/src/foo.rs").unwrap()
    );
}

#[test]
fn local_or_remote_path_strip_repo_prefix_local_local() {
    let repo = LocalOrRemotePath::Local("/repo".into());
    let inside = LocalOrRemotePath::Local("/repo/src/foo.rs".into());
    let outside = LocalOrRemotePath::Local("/other/foo.rs".into());

    assert_eq!(
        repo.strip_repo_prefix(&inside),
        Some("src/foo.rs".to_string()),
    );
    assert_eq!(repo.strip_repo_prefix(&outside), None);
}

#[test]
fn local_or_remote_path_strip_repo_prefix_requires_same_host() {
    let host_a = HostId::new("host-a".to_string());
    let host_b = HostId::new("host-b".to_string());
    let repo_a = LocalOrRemotePath::Remote(RemotePath::new(
        host_a.clone(),
        StandardizedPath::try_new("/repo").unwrap(),
    ));
    let file_a = LocalOrRemotePath::Remote(RemotePath::new(
        host_a,
        StandardizedPath::try_new("/repo/src/foo.rs").unwrap(),
    ));
    let file_b = LocalOrRemotePath::Remote(RemotePath::new(
        host_b,
        StandardizedPath::try_new("/repo/src/foo.rs").unwrap(),
    ));
    let file_local = LocalOrRemotePath::Local("/repo/src/foo.rs".into());

    assert_eq!(
        repo_a.strip_repo_prefix(&file_a),
        Some("src/foo.rs".to_string()),
    );
    assert_eq!(
        repo_a.strip_repo_prefix(&file_b),
        None,
        "cross-host strip should be rejected"
    );
    assert_eq!(
        repo_a.strip_repo_prefix(&file_local),
        None,
        "local-vs-remote strip should be rejected"
    );
}
