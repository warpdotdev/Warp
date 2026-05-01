//! Tests for `WorkspaceManager`.
//!
//! Hooks are mocked so these tests never touch a real shell.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use symphony::tracker::Issue;
use symphony::workflow::HooksConfig;
use symphony::workspace::{sanitize_identifier, HookRunner, WorkspaceError, WorkspaceManager};
use tempfile::TempDir;

fn issue_with_id(identifier: &str) -> Issue {
    Issue {
        id: format!("uuid-{identifier}"),
        identifier: identifier.into(),
        title: "x".into(),
        description: None,
        priority: None,
        state: "Todo".into(),
        url: None,
        labels: Vec::new(),
        blocked_by: Vec::new(),
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    }
}

#[derive(Default)]
struct RecordingRunner {
    calls: Mutex<Vec<String>>,
    fail_next: Mutex<bool>,
}

impl RecordingRunner {
    fn fail_once(self: &Arc<Self>) {
        *self.fail_next.lock().unwrap() = true;
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl HookRunner for RecordingRunner {
    async fn run(
        &self,
        script: &str,
        _cwd: &Path,
        _timeout: Duration,
    ) -> Result<(), WorkspaceError> {
        self.calls.lock().unwrap().push(script.to_string());
        let mut g = self.fail_next.lock().unwrap();
        if *g {
            *g = false;
            return Err(WorkspaceError::HookFailed {
                hook: "(shell)".into(),
                code: Some(1),
                stderr: "boom".into(),
            });
        }
        Ok(())
    }
}

#[test]
fn sanitizes_identifier_with_special_chars() {
    assert_eq!(sanitize_identifier("PDX-12"), "PDX-12");
    assert_eq!(sanitize_identifier("PDX/12"), "PDX_12");
    assert_eq!(sanitize_identifier("PDX 12"), "PDX_12");
    assert_eq!(sanitize_identifier("PDX:12!"), "PDX_12_");
}

#[tokio::test]
async fn rejects_path_outside_root() {
    // Passing an identifier laden with `..` must NOT escape the root —
    // the sanitizer collapses it and the prefix check passes; verify by
    // confirming the resulting path lives inside the root.
    let dir = TempDir::new().unwrap();
    let runner = Arc::new(RecordingRunner::default());
    let mgr = WorkspaceManager::with_runner(
        dir.path().to_path_buf(),
        HooksConfig::default(),
        runner.clone(),
    );

    let issue = issue_with_id("../../etc/passwd");
    let ws = mgr.ensure_for(&issue).await.unwrap();
    assert!(
        ws.path.starts_with(dir.path()),
        "workspace path {:?} escaped root {:?}",
        ws.path,
        dir.path()
    );
}

#[tokio::test]
async fn created_now_only_true_first_time() {
    let dir = TempDir::new().unwrap();
    let runner = Arc::new(RecordingRunner::default());
    let mgr = WorkspaceManager::with_runner(
        dir.path().to_path_buf(),
        HooksConfig {
            after_create: Some("touch /tmp/x".into()),
            ..Default::default()
        },
        runner.clone(),
    );

    let issue = issue_with_id("PDX-1");
    let ws1 = mgr.ensure_for(&issue).await.unwrap();
    assert!(ws1.created_now);
    let ws2 = mgr.ensure_for(&issue).await.unwrap();
    assert!(!ws2.created_now);
    assert_eq!(runner.calls().len(), 1, "after_create runs only once");
}

#[tokio::test]
async fn after_create_hook_failure_aborts() {
    let dir = TempDir::new().unwrap();
    let runner = Arc::new(RecordingRunner::default());
    runner.fail_once();
    let mgr = WorkspaceManager::with_runner(
        dir.path().to_path_buf(),
        HooksConfig {
            after_create: Some("git clone".into()),
            ..Default::default()
        },
        runner.clone(),
    );
    let issue = issue_with_id("PDX-2");
    let err = mgr.ensure_for(&issue).await.unwrap_err();
    match err {
        WorkspaceError::HookFailed { hook, .. } => assert_eq!(hook, "after_create"),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn after_run_hook_failure_logs_but_continues() {
    let dir = TempDir::new().unwrap();
    let runner = Arc::new(RecordingRunner::default());
    let mgr = WorkspaceManager::with_runner(
        dir.path().to_path_buf(),
        HooksConfig {
            after_run: Some("echo done".into()),
            ..Default::default()
        },
        runner.clone(),
    );
    let issue = issue_with_id("PDX-3");
    let ws = mgr.ensure_for(&issue).await.unwrap();
    runner.fail_once();
    // Should not panic / not return.
    mgr.run_after_run_hook(&ws).await;
    assert_eq!(runner.calls().len(), 1);
}
