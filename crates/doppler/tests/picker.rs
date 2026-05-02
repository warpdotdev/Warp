// SPDX-License-Identifier: AGPL-3.0-only
//
// Integration tests for the project/config picker (PDX-51).

use std::io;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt as _;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt as _;
use std::process::{ExitStatus, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use doppler::{
    bind_project, list_configs, list_projects, CommandRunner, DopplerConfig, DopplerError,
    DopplerProject,
};

/// Same scriptable mock as `tests/fetch.rs`, copied here to keep this test
/// file standalone (Rust integration tests cannot share modules).
struct MockRunner {
    calls: AtomicUsize,
    responses: Mutex<Vec<io::Result<Output>>>,
    last_calls: Mutex<Vec<(Vec<String>, Option<PathBuf>)>>,
}

impl MockRunner {
    fn new(responses: Vec<io::Result<Output>>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            responses: Mutex::new(responses),
            last_calls: Mutex::new(Vec::new()),
        })
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn last_args(&self) -> Vec<String> {
        let calls = self.last_calls.lock().unwrap();
        calls.last().map(|(a, _)| a.clone()).unwrap_or_default()
    }

    fn last_cwd(&self) -> Option<PathBuf> {
        let calls = self.last_calls.lock().unwrap();
        calls.last().and_then(|(_, c)| c.clone())
    }
}

#[async_trait::async_trait]
impl CommandRunner for MockRunner {
    async fn run(&self, args: &[&str], cwd: Option<&Path>) -> io::Result<Output> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.last_calls.lock().unwrap().push((
            args.iter().map(|s| s.to_string()).collect(),
            cwd.map(|p| p.to_path_buf()),
        ));
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(io::Error::other("no more mock responses"));
        }
        responses.remove(0)
    }
}

fn ok_output(stdout: &str) -> Output {
    #[cfg(unix)]
    let status = ExitStatus::from_raw(0);
    #[cfg(windows)]
    let status = ExitStatus::from_raw(0);
    Output {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

fn err_output(code: i32, stderr: &str) -> Output {
    #[cfg(unix)]
    let status = ExitStatus::from_raw(code << 8);
    #[cfg(windows)]
    let status = ExitStatus::from_raw(code as u32);
    Output {
        status,
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

const PROJECTS_JSON: &str = r#"[
  {"slug":"my-app","name":"my-app","description":"Production app"},
  {"slug":"infra","name":"infra","description":""},
  {"slug":"playground","name":"Playground"}
]"#;

const CONFIGS_JSON: &str = r#"[
  {"name":"dev","root":true,"environment":"dev"},
  {"name":"dev_alice","root":false,"environment":"dev"},
  {"name":"prd","root":true,"environment":"prd"}
]"#;

#[tokio::test]
async fn list_projects_parses_json_and_invokes_correct_command() {
    let runner = MockRunner::new(vec![Ok(ok_output(PROJECTS_JSON))]);
    let projects = list_projects(runner.clone() as Arc<dyn CommandRunner>)
        .await
        .expect("list_projects ok");

    assert_eq!(runner.call_count(), 1);
    assert_eq!(runner.last_args(), vec!["projects", "--json"]);
    assert_eq!(runner.last_cwd(), None);

    assert_eq!(
        projects,
        vec![
            DopplerProject {
                slug: "my-app".into(),
                name: "my-app".into(),
                description: "Production app".into(),
            },
            DopplerProject {
                slug: "infra".into(),
                name: "infra".into(),
                description: String::new(),
            },
            DopplerProject {
                slug: "playground".into(),
                name: "Playground".into(),
                description: String::new(),
            },
        ]
    );
}

#[tokio::test]
async fn list_configs_passes_project_arg() {
    let runner = MockRunner::new(vec![Ok(ok_output(CONFIGS_JSON))]);
    let configs = list_configs(runner.clone() as Arc<dyn CommandRunner>, "my-app")
        .await
        .expect("list_configs ok");

    assert_eq!(
        runner.last_args(),
        vec!["configs", "--project", "my-app", "--json"]
    );
    assert_eq!(
        configs,
        vec![
            DopplerConfig {
                name: "dev".into(),
                root: true,
                environment: "dev".into(),
            },
            DopplerConfig {
                name: "dev_alice".into(),
                root: false,
                environment: "dev".into(),
            },
            DopplerConfig {
                name: "prd".into(),
                root: true,
                environment: "prd".into(),
            },
        ]
    );
}

#[tokio::test]
async fn list_projects_maps_unauthenticated_stderr() {
    let runner = MockRunner::new(vec![Ok(err_output(1, "Doppler Error: not authenticated"))]);
    let err = list_projects(runner as Arc<dyn CommandRunner>)
        .await
        .expect_err("expected NotAuthenticated");
    assert!(matches!(err, DopplerError::NotAuthenticated));
}

#[tokio::test]
async fn list_projects_maps_network_error() {
    let runner = MockRunner::new(vec![Ok(err_output(
        1,
        "could not reach api.doppler.com: dial tcp",
    ))]);
    let err = list_projects(runner as Arc<dyn CommandRunner>)
        .await
        .expect_err("expected Unreachable");
    assert!(matches!(err, DopplerError::Unreachable));
}

#[tokio::test]
async fn list_projects_passes_through_other_nonzero_exit() {
    let runner = MockRunner::new(vec![Ok(err_output(7, "weird unexpected failure"))]);
    let err = list_projects(runner as Arc<dyn CommandRunner>)
        .await
        .expect_err("expected NonZeroExit");
    match err {
        DopplerError::NonZeroExit { code, stderr } => {
            assert_eq!(code, 7);
            assert!(stderr.contains("weird"));
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

#[tokio::test]
async fn list_projects_surfaces_parse_error_on_garbage_stdout() {
    let runner = MockRunner::new(vec![Ok(ok_output("<<not json>>"))]);
    let err = list_projects(runner as Arc<dyn CommandRunner>)
        .await
        .expect_err("expected parse error");
    match err {
        DopplerError::NonZeroExit { stderr, .. } => {
            assert!(stderr.contains("failed to parse"));
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn bind_project_invokes_doppler_setup_with_no_prompt_and_scope() {
    let runner = MockRunner::new(vec![Ok(ok_output(""))]);
    let scope = PathBuf::from("/home/u/repo");
    bind_project(
        runner.clone() as Arc<dyn CommandRunner>,
        "my-app",
        "dev",
        &scope,
    )
    .await
    .expect("bind ok");

    assert_eq!(runner.call_count(), 1);
    let args = runner.last_args();
    // Order matters: --no-prompt must come before any positional arg pattern,
    // and --scope must point at the requested directory.
    assert_eq!(args[0], "setup");
    assert!(args.contains(&"--no-prompt".to_string()));
    let project_idx = args.iter().position(|s| s == "--project").unwrap();
    assert_eq!(args[project_idx + 1], "my-app");
    let config_idx = args.iter().position(|s| s == "--config").unwrap();
    assert_eq!(args[config_idx + 1], "dev");
    let scope_idx = args.iter().position(|s| s == "--scope").unwrap();
    assert_eq!(args[scope_idx + 1], "/home/u/repo");
    // `cwd` is also passed so the CLI writes `.doppler.yaml` in the right
    // place even on older CLI versions that ignore --scope.
    assert_eq!(runner.last_cwd().as_deref(), Some(scope.as_path()));
}

#[tokio::test]
async fn bind_project_propagates_unauthenticated() {
    let runner = MockRunner::new(vec![Ok(err_output(1, "Doppler Error: not authenticated"))]);
    let err = bind_project(
        runner as Arc<dyn CommandRunner>,
        "my-app",
        "dev",
        Path::new("/tmp"),
    )
    .await
    .expect_err("expected NotAuthenticated");
    assert!(matches!(err, DopplerError::NotAuthenticated));
}
