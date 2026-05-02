use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::{
    build_tera_context, build_warp_tpl_env, eval_condition, resolve_working_dir,
    strip_tera_delimiters, FailStrategy, HookContext, HookError, HookEvent, HookProgress,
    PostInitHook, SilentProgress, Variables,
};

// ---------------------------------------------------------------------------
// Helper: collect events
// ---------------------------------------------------------------------------

struct EventLog(Mutex<Vec<HookEvent>>);

impl EventLog {
    fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    fn events(&self) -> Vec<HookEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl HookProgress for EventLog {
    fn on_event(&self, event: HookEvent) {
        self.0.lock().unwrap().push(event);
    }
}

fn vars(pairs: &[(&str, serde_json::Value)]) -> Variables {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

// ---------------------------------------------------------------------------
// strip_tera_delimiters
// ---------------------------------------------------------------------------

#[test]
fn strip_delimiters_with_braces() {
    assert_eq!(strip_tera_delimiters("{{ install_deps }}"), "install_deps");
}

#[test]
fn strip_delimiters_without_braces() {
    assert_eq!(strip_tera_delimiters("install_deps"), "install_deps");
}

// ---------------------------------------------------------------------------
// eval_condition
// ---------------------------------------------------------------------------

#[test]
fn condition_bool_true() {
    let ctx = build_tera_context(&vars(&[("install_deps", serde_json::Value::Bool(true))]));
    assert!(eval_condition("{{ install_deps }}", &ctx).unwrap());
}

#[test]
fn condition_bool_false() {
    let ctx = build_tera_context(&vars(&[("install_deps", serde_json::Value::Bool(false))]));
    assert!(!eval_condition("{{ install_deps }}", &ctx).unwrap());
}

#[test]
fn condition_string_comparison() {
    let ctx = build_tera_context(&vars(&[("framework", serde_json::json!("react"))]));
    assert!(eval_condition("{{ framework != \"none\" }}", &ctx).unwrap());

    let ctx = build_tera_context(&vars(&[("framework", serde_json::json!("none"))]));
    assert!(!eval_condition("{{ framework != \"none\" }}", &ctx).unwrap());
}

// ---------------------------------------------------------------------------
// build_warp_tpl_env
// ---------------------------------------------------------------------------

#[test]
fn tpl_env_converts_values() {
    let v = vars(&[
        ("project_slug", serde_json::json!("my-app")),
        ("install_deps", serde_json::Value::Bool(true)),
        ("optional", serde_json::Value::Null),
    ]);
    let env = build_warp_tpl_env(&v);
    assert_eq!(env["WARP_TPL_PROJECT_SLUG"], "my-app");
    assert_eq!(env["WARP_TPL_INSTALL_DEPS"], "true");
    assert_eq!(env["WARP_TPL_OPTIONAL"], "");
}

// ---------------------------------------------------------------------------
// resolve_working_dir
// ---------------------------------------------------------------------------

#[test]
fn working_dir_default_dot() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(resolve_working_dir(tmp.path(), None).is_ok());
}

#[test]
fn working_dir_relative_subdir() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    assert!(resolve_working_dir(tmp.path(), Some("src")).is_ok());
}

#[test]
fn working_dir_parent_escape_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(resolve_working_dir(tmp.path(), Some("../etc")).is_err());
}

#[test]
fn working_dir_absolute_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(resolve_working_dir(tmp.path(), Some("/etc")).is_err());
}

// ---------------------------------------------------------------------------
// run_hooks integration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_hooks_success() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = vec![PostInitHook {
        name: "touch sentinel".into(),
        command: "touch sentinel.txt".into(),
        working_dir: None,
        env: HashMap::new(),
        condition: None,
        fail_strategy: FailStrategy::Abort,
    }];
    let ctx = HookContext::new(tmp.path().to_path_buf(), HashMap::new());
    let log = EventLog::new();
    crate::run_hooks(&hooks, &ctx, &log).await.unwrap();

    assert!(tmp.path().join("sentinel.txt").exists());
    let ev = log.events();
    assert!(matches!(ev[0], HookEvent::Starting { index: 0, .. }));
    assert!(matches!(ev[1], HookEvent::Finished { index: 0, .. }));
}

#[tokio::test]
async fn run_hooks_skipped_when_condition_false() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = vec![PostInitHook {
        name: "should skip".into(),
        command: "touch should_not_exist.txt".into(),
        working_dir: None,
        env: HashMap::new(),
        condition: Some("{{ run_me }}".into()),
        fail_strategy: FailStrategy::Abort,
    }];
    let ctx = HookContext::new(
        tmp.path().to_path_buf(),
        vars(&[("run_me", serde_json::Value::Bool(false))]),
    );
    let log = EventLog::new();
    crate::run_hooks(&hooks, &ctx, &log).await.unwrap();

    assert!(!tmp.path().join("should_not_exist.txt").exists());
    assert!(matches!(log.events()[0], HookEvent::Skipped { index: 0, .. }));
}

#[tokio::test]
async fn run_hooks_abort_stops_subsequent_hooks() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = vec![
        PostInitHook {
            name: "failing".into(),
            command: "exit 1".into(),
            working_dir: None,
            env: HashMap::new(),
            condition: None,
            fail_strategy: FailStrategy::Abort,
        },
        PostInitHook {
            name: "unreachable".into(),
            command: "touch after.txt".into(),
            working_dir: None,
            env: HashMap::new(),
            condition: None,
            fail_strategy: FailStrategy::Abort,
        },
    ];
    let ctx = HookContext::new(tmp.path().to_path_buf(), HashMap::new());
    let result = crate::run_hooks(&hooks, &ctx, &SilentProgress).await;

    assert!(matches!(result, Err(HookError::HookFailed { .. })));
    assert!(!tmp.path().join("after.txt").exists());
}

#[tokio::test]
async fn run_hooks_warn_continues() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = vec![
        PostInitHook {
            name: "warn hook".into(),
            command: "exit 2".into(),
            working_dir: None,
            env: HashMap::new(),
            condition: None,
            fail_strategy: FailStrategy::Warn,
        },
        PostInitHook {
            name: "after warn".into(),
            command: "touch after_warn.txt".into(),
            working_dir: None,
            env: HashMap::new(),
            condition: None,
            fail_strategy: FailStrategy::Abort,
        },
    ];
    let ctx = HookContext::new(tmp.path().to_path_buf(), HashMap::new());
    let log = EventLog::new();
    crate::run_hooks(&hooks, &ctx, &log).await.unwrap();

    assert!(tmp.path().join("after_warn.txt").exists());
    assert!(matches!(
        log.events()[1],
        HookEvent::Failed { strategy: FailStrategy::Warn, .. }
    ));
}

#[tokio::test]
async fn run_hooks_ignore_continues() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = vec![
        PostInitHook {
            name: "ignore hook".into(),
            command: "exit 3".into(),
            working_dir: None,
            env: HashMap::new(),
            condition: None,
            fail_strategy: FailStrategy::Ignore,
        },
        PostInitHook {
            name: "after ignore".into(),
            command: "touch after_ignore.txt".into(),
            working_dir: None,
            env: HashMap::new(),
            condition: None,
            fail_strategy: FailStrategy::Abort,
        },
    ];
    let ctx = HookContext::new(tmp.path().to_path_buf(), HashMap::new());
    crate::run_hooks(&hooks, &ctx, &SilentProgress).await.unwrap();
    assert!(tmp.path().join("after_ignore.txt").exists());
}

#[tokio::test]
async fn run_hooks_warp_tpl_env_is_set() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = vec![PostInitHook {
        name: "check env".into(),
        command: r#"printf '%s' "$WARP_TPL_PROJECT_SLUG" > slug.txt"#.into(),
        working_dir: None,
        env: HashMap::new(),
        condition: None,
        fail_strategy: FailStrategy::Abort,
    }];
    let ctx = HookContext::new(
        tmp.path().to_path_buf(),
        vars(&[("project_slug", serde_json::json!("my-app"))]),
    );
    crate::run_hooks(&hooks, &ctx, &SilentProgress).await.unwrap();

    let content = std::fs::read_to_string(tmp.path().join("slug.txt")).unwrap();
    assert_eq!(content, "my-app");
}

#[tokio::test]
async fn run_hooks_timeout_abort() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = vec![PostInitHook {
        name: "slow hook".into(),
        command: "sleep 60".into(),
        working_dir: None,
        env: HashMap::new(),
        condition: None,
        fail_strategy: FailStrategy::Abort,
    }];
    let mut ctx = HookContext::new(tmp.path().to_path_buf(), HashMap::new());
    ctx.timeout = Duration::from_millis(100);

    let log = EventLog::new();
    let result = crate::run_hooks(&hooks, &ctx, &log).await;

    assert!(matches!(result, Err(HookError::HookTimedOut { .. })));
    assert!(matches!(log.events().last(), Some(HookEvent::TimedOut { .. })));
}
