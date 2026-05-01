//! Integration tests for the three-layer prompt composer.

use std::path::PathBuf;

use orchestrator::Role;
use prompts::{Composer, ComposerConfig, LayerKind, PromptError};
use tempfile::TempDir;
use tokio::fs;

/// Build a temporary skeleton with a base file and (optionally) a planner role
/// overlay. Returns `(tempdir, config)`. Holding the `TempDir` keeps the
/// directory alive for the duration of the test.
async fn skeleton(with_planner: bool) -> (TempDir, ComposerConfig) {
    let dir = TempDir::new().expect("tempdir");
    let base_path = dir.path().join("base.md");
    let role_dir = dir.path().join("roles");
    fs::create_dir_all(&role_dir).await.unwrap();
    fs::write(&base_path, "BASE CONTENT\n").await.unwrap();
    if with_planner {
        fs::write(role_dir.join("planner.md"), "PLANNER OVERLAY\n")
            .await
            .unwrap();
    }
    let config = ComposerConfig {
        base_path,
        role_overlay_dir: role_dir,
        project_overlay_path: None,
    };
    (dir, config)
}

#[tokio::test]
async fn compose_returns_three_layers_when_all_present() {
    let (dir, mut config) = skeleton(true).await;
    let project_path = dir.path().join("WARP.md");
    fs::write(&project_path, "PROJECT OVERLAY\n").await.unwrap();
    config.project_overlay_path = Some(project_path);

    let composer = Composer::new(config);
    let prompt = composer.compose(Role::Planner).await.expect("compose ok");

    assert_eq!(prompt.layers.len(), 3);
    assert_eq!(prompt.layers[0].kind, LayerKind::Base);
    assert_eq!(prompt.layers[1].kind, LayerKind::Role);
    assert_eq!(prompt.layers[2].kind, LayerKind::Project);
    assert!(prompt.system.contains("BASE CONTENT"));
    assert!(prompt.system.contains("PLANNER OVERLAY"));
    assert!(prompt.system.contains("PROJECT OVERLAY"));
}

#[tokio::test]
async fn compose_skips_project_layer_when_path_is_none() {
    let (_dir, config) = skeleton(true).await;
    let composer = Composer::new(config);
    let prompt = composer.compose(Role::Planner).await.expect("compose ok");

    assert_eq!(prompt.layers.len(), 2);
    assert!(prompt
        .layers
        .iter()
        .all(|l| l.kind != LayerKind::Project));
}

#[tokio::test]
async fn compose_skips_project_layer_when_file_does_not_exist() {
    let (dir, mut config) = skeleton(true).await;
    config.project_overlay_path = Some(dir.path().join("does-not-exist.md"));

    let composer = Composer::new(config);
    let prompt = composer.compose(Role::Planner).await.expect("compose ok");

    assert_eq!(prompt.layers.len(), 2);
    assert!(prompt
        .layers
        .iter()
        .all(|l| l.kind != LayerKind::Project));
}

#[tokio::test]
async fn compose_errors_when_base_missing() {
    let (dir, mut config) = skeleton(true).await;
    config.base_path = dir.path().join("missing-base.md");

    let composer = Composer::new(config);
    let err = composer
        .compose(Role::Planner)
        .await
        .expect_err("expected base missing error");
    match err {
        PromptError::BaseMissing(_) => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn compose_errors_when_role_overlay_missing() {
    // Skeleton without the planner overlay.
    let (_dir, config) = skeleton(false).await;
    let composer = Composer::new(config);
    let err = composer
        .compose(Role::Planner)
        .await
        .expect_err("expected role missing error");
    match err {
        PromptError::RoleMissing(Role::Planner) => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn compose_includes_section_headers() {
    let (dir, mut config) = skeleton(true).await;
    let project_path = dir.path().join("WARP.md");
    fs::write(&project_path, "PROJECT OVERLAY\n").await.unwrap();
    config.project_overlay_path = Some(project_path);

    let composer = Composer::new(config);
    let prompt = composer.compose(Role::Planner).await.expect("compose ok");

    assert!(prompt.system.contains("## Layer: Base"));
    assert!(prompt.system.contains("## Layer: Role"));
    assert!(prompt.system.contains("## Layer: Project"));
}

#[tokio::test]
async fn compose_with_real_templates_smoke() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let templates = crate_dir.join("templates");
    let config = ComposerConfig {
        base_path: templates.join("base.md"),
        role_overlay_dir: templates.join("roles"),
        project_overlay_path: None,
    };
    let composer = Composer::new(config);

    for role in [
        Role::Planner,
        Role::Reviewer,
        Role::Worker,
        Role::BulkRefactor,
        Role::Summarize,
        Role::ToolRouter,
        Role::Inline,
    ] {
        let prompt = composer
            .compose(role)
            .await
            .unwrap_or_else(|e| panic!("compose {role:?} failed: {e}"));
        assert!(
            !prompt.system.is_empty(),
            "composed prompt for {role:?} was empty",
        );
        // Locked stack rule: the AI Gateway URL must appear in every
        // composed prompt because it's part of the base layer.
        assert!(
            prompt.system.contains("gateway.ai.cloudflare.com"),
            "composed prompt for {role:?} is missing the AI Gateway URL",
        );
    }
}
