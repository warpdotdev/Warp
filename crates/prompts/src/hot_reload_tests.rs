use std::time::Duration;

use orchestrator::Role;
use tempfile::TempDir;
use tokio::time::timeout;

use super::{HotReloadComposer, PromptFileChanged};
use crate::{ComposerConfig, LayerKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_sync(path: &std::path::Path, content: &str) {
    std::fs::write(path, content).expect("write_sync");
}

/// Build a minimal on-disk skeleton: `base.md` + `roles/planner.md`.
fn make_config(dir: &TempDir) -> ComposerConfig {
    let base = dir.path().join("base.md");
    let roles_dir = dir.path().join("roles");
    std::fs::create_dir_all(&roles_dir).expect("create roles dir");
    write_sync(&base, "# Base\n\nBase content.");
    write_sync(&roles_dir.join("planner.md"), "# Planner\n\nPlanner overlay.");
    ComposerConfig {
        base_path: base,
        role_overlay_dir: roles_dir,
        project_overlay_path: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start_watcher_succeeds() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    HotReloadComposer::start(config).expect("watcher should start");
}

#[tokio::test]
async fn subscribe_returns_independent_receivers() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    let composer = HotReloadComposer::start(config).unwrap();
    let rx1 = composer.subscribe();
    let rx2 = composer.subscribe();
    // Both receivers should report the same starting lag position.
    assert_eq!(rx1.len(), rx2.len());
}

#[tokio::test]
async fn composer_getter_exposes_underlying_composer() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    let hot = HotReloadComposer::start(config).unwrap();
    // The composer should be able to compose without error.
    let prompt = hot
        .composer()
        .compose(Role::Planner)
        .await
        .expect("compose");
    assert_eq!(prompt.layers.len(), 2); // base + role
    assert_eq!(prompt.layers[0].kind, LayerKind::Base);
    assert_eq!(prompt.layers[1].kind, LayerKind::Role);
}

#[tokio::test]
async fn change_event_emitted_when_base_file_modified() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    let base_path = config.base_path.clone();

    let composer = HotReloadComposer::start(config).unwrap();
    let mut rx = composer.subscribe();

    // Give the OS watcher a moment to register the paths before writing.
    tokio::time::sleep(Duration::from_millis(50)).await;

    write_sync(&base_path, "# Base\n\nUpdated base content.");

    let event: PromptFileChanged = timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timed out waiting for change event")
        .expect("broadcast channel closed unexpectedly");

    // The reported path should resolve to the file we modified.
    let reported = event.path.canonicalize().unwrap_or(event.path.clone());
    let expected = base_path.canonicalize().unwrap_or(base_path.clone());
    assert_eq!(
        reported, expected,
        "expected event for base.md, got {reported:?}",
    );
}

#[tokio::test]
async fn change_event_emitted_when_role_overlay_modified() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    let planner_path = config.role_overlay_dir.join("planner.md");

    let composer = HotReloadComposer::start(config).unwrap();
    let mut rx = composer.subscribe();

    tokio::time::sleep(Duration::from_millis(50)).await;

    write_sync(&planner_path, "# Planner\n\nRevised planner overlay.");

    let event: PromptFileChanged = timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timed out waiting for change event")
        .expect("broadcast channel closed");

    let reported = event.path.canonicalize().unwrap_or(event.path.clone());
    let expected = planner_path.canonicalize().unwrap_or(planner_path.clone());
    assert_eq!(reported, expected);
}

#[tokio::test]
async fn recompose_after_base_change_reflects_new_content() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    let base_path = config.base_path.clone();

    let hot = HotReloadComposer::start(config).unwrap();
    let mut rx = hot.subscribe();

    // Confirm initial content.
    let before = hot.composer().compose(Role::Planner).await.unwrap();
    assert!(
        before.system.contains("Base content."),
        "initial system prompt missing expected content",
    );

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Overwrite the base file with new content.
    write_sync(&base_path, "# Base\n\nCompletely rewritten base.");

    // Wait for the watcher event.
    timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timed out waiting for change event")
        .expect("broadcast channel closed");

    // Recompose — the composer always re-reads from disk.
    let after = hot.composer().compose(Role::Planner).await.unwrap();
    assert!(
        after.system.contains("Completely rewritten base."),
        "recomposed prompt missing updated content",
    );
    assert!(
        !after.system.contains("Base content."),
        "recomposed prompt still contains stale content",
    );
}

#[tokio::test]
async fn multiple_subscribers_all_receive_event() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    let base_path = config.base_path.clone();

    let composer = HotReloadComposer::start(config).unwrap();
    let mut rx1 = composer.subscribe();
    let mut rx2 = composer.subscribe();

    tokio::time::sleep(Duration::from_millis(50)).await;

    write_sync(&base_path, "# Base\n\nBroadcast test.");

    let (e1, e2) = tokio::join!(
        timeout(Duration::from_secs(3), rx1.recv()),
        timeout(Duration::from_secs(3), rx2.recv()),
    );

    e1.expect("rx1 timed out").expect("rx1 channel closed");
    e2.expect("rx2 timed out").expect("rx2 channel closed");
}
