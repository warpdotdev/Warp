// SPDX-License-Identifier: AGPL-3.0-only

use std::io;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt as _;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use doppler::{parse_configure_all, read_status, CommandRunner, DopplerError, DopplerStatus};

// ---------------------------------------------------------------------------
// MockRunner — identical pattern to tests/fetch.rs
// ---------------------------------------------------------------------------

struct MockRunner {
    calls: AtomicUsize,
    responses: Mutex<Vec<io::Result<Output>>>,
    last_args: Mutex<Vec<Vec<String>>>,
}

impl MockRunner {
    fn new(responses: Vec<io::Result<Output>>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            responses: Mutex::new(responses),
            last_args: Mutex::new(Vec::new()),
        })
    }

    #[allow(dead_code)]
    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn last_args(&self) -> Vec<Vec<String>> {
        self.last_args.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl CommandRunner for MockRunner {
    async fn run(&self, args: &[&str], _cwd: Option<&Path>) -> io::Result<Output> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.last_args
            .lock()
            .unwrap()
            .push(args.iter().map(|s| s.to_string()).collect());
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

// ---------------------------------------------------------------------------
// Shared fixture strings
// ---------------------------------------------------------------------------

/// Realistic `doppler configure --all` output: authenticated, one binding,
/// default scope has project + config as well.
const FULL_TABLE: &str = "\
┌─────────────────────────┬─────────┬───────────────────────────────┐
│ Scope                   │ Name    │ Value                         │
├─────────────────────────┼─────────┼───────────────────────────────┤
│ /home/user/myapp        │ project │ my-project                    │
│                         │ config  │ dev                           │
├─────────────────────────┼─────────┼───────────────────────────────┤
│ (default)               │ token   │ dp.pt.abc123                  │
│                         │ project │ fallback-project              │
│                         │ config  │ staging                       │
└─────────────────────────┴─────────┴───────────────────────────────┘
";

// ---------------------------------------------------------------------------
// parse_configure_all unit tests
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_returns_zero_state() {
    let s = parse_configure_all("");
    assert_eq!(
        s,
        DopplerStatus {
            authenticated: false,
            scoped_bindings: vec![],
            default_project: None,
            default_config: None,
        }
    );
}

#[test]
fn parse_full_table_authenticated() {
    let s = parse_configure_all(FULL_TABLE);
    assert!(s.authenticated, "token in default scope → authenticated");
}

#[test]
fn parse_full_table_scoped_binding() {
    let s = parse_configure_all(FULL_TABLE);
    assert_eq!(s.scoped_bindings.len(), 1);
    assert_eq!(s.scoped_bindings[0].scope, PathBuf::from("/home/user/myapp"));
    assert_eq!(s.scoped_bindings[0].project, Some("my-project".into()));
    assert_eq!(s.scoped_bindings[0].config, Some("dev".into()));
}

#[test]
fn parse_full_table_default_scope() {
    let s = parse_configure_all(FULL_TABLE);
    assert_eq!(s.default_project, Some("fallback-project".into()));
    assert_eq!(s.default_config, Some("staging".into()));
}

#[test]
fn parse_multiple_bindings_order_preserved() {
    let table = "\
┌──────────────────┬─────────┬─────────────┐
│ Scope            │ Name    │ Value       │
├──────────────────┼─────────┼─────────────┤
│ /projects/alpha  │ project │ alpha       │
│                  │ config  │ dev         │
├──────────────────┼─────────┼─────────────┤
│ /projects/beta   │ project │ beta        │
│                  │ config  │ prd         │
├──────────────────┼─────────┼─────────────┤
│ /projects/gamma  │ project │ gamma       │
│                  │ config  │ stg         │
└──────────────────┴─────────┴─────────────┘
";
    let s = parse_configure_all(table);
    assert_eq!(s.scoped_bindings.len(), 3);
    assert_eq!(s.scoped_bindings[0].scope, PathBuf::from("/projects/alpha"));
    assert_eq!(s.scoped_bindings[1].scope, PathBuf::from("/projects/beta"));
    assert_eq!(s.scoped_bindings[2].scope, PathBuf::from("/projects/gamma"));
    assert_eq!(s.scoped_bindings[2].config, Some("stg".into()));
}

#[test]
fn parse_scope_repeated_every_row() {
    let table = "\
┌──────────────┬─────────┬─────────────┐
│ Scope        │ Name    │ Value       │
├──────────────┼─────────┼─────────────┤
│ /home/u/app  │ project │ my-app      │
│ /home/u/app  │ config  │ dev         │
└──────────────┴─────────┴─────────────┘
";
    let s = parse_configure_all(table);
    assert_eq!(s.scoped_bindings.len(), 1);
    assert_eq!(s.scoped_bindings[0].project, Some("my-app".into()));
    assert_eq!(s.scoped_bindings[0].config, Some("dev".into()));
}

#[test]
fn parse_legacy_enclave_prefix() {
    let table = "\
┌──────────────┬──────────────────┬─────────────┐
│ Scope        │ Name             │ Value       │
├──────────────┼──────────────────┼─────────────┤
│ /home/u/app  │ enclave.project  │ old-app     │
│              │ enclave.config   │ dev         │
└──────────────┴──────────────────┴─────────────┘
";
    let s = parse_configure_all(table);
    assert_eq!(s.scoped_bindings[0].project, Some("old-app".into()));
    assert_eq!(s.scoped_bindings[0].config, Some("dev".into()));
}

#[test]
fn parse_no_token_means_unauthenticated() {
    let table = "\
┌──────────────┬─────────┬─────────────┐
│ Scope        │ Name    │ Value       │
├──────────────┼─────────┼─────────────┤
│ /home/u/app  │ project │ my-app      │
│              │ config  │ dev         │
└──────────────┴─────────┴─────────────┘
";
    assert!(!parse_configure_all(table).authenticated);
}

// ---------------------------------------------------------------------------
// read_status integration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn read_status_passes_correct_args() {
    let runner = MockRunner::new(vec![Ok(ok_output(FULL_TABLE))]);
    read_status(runner.clone() as Arc<dyn CommandRunner>)
        .await
        .expect("should succeed");

    let args = runner.last_args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0], vec!["configure", "--all"]);
}

#[tokio::test]
async fn read_status_happy_path_returns_parsed_status() {
    let runner = MockRunner::new(vec![Ok(ok_output(FULL_TABLE))]);
    let status = read_status(runner as Arc<dyn CommandRunner>)
        .await
        .expect("should succeed");

    assert!(status.authenticated);
    assert_eq!(status.scoped_bindings.len(), 1);
}

#[tokio::test]
async fn read_status_non_zero_exit_returns_error() {
    let runner = MockRunner::new(vec![Ok(err_output(1, "something went wrong\n"))]);
    let err = read_status(runner as Arc<dyn CommandRunner>)
        .await
        .expect_err("non-zero exit should error");

    match err {
        DopplerError::NonZeroExit { code, .. } => assert_eq!(code, 1),
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

#[tokio::test]
async fn read_status_not_authenticated_error_mapped() {
    let runner = MockRunner::new(vec![Ok(err_output(
        1,
        "doppler: not authenticated. run `doppler login`\n",
    ))]);
    let err = read_status(runner as Arc<dyn CommandRunner>)
        .await
        .expect_err("should error");

    assert!(matches!(err, DopplerError::NotAuthenticated));
}

#[tokio::test]
async fn read_status_spawn_failure_propagated() {
    let runner = MockRunner::new(vec![Err(io::Error::other("spawn failed"))]);
    let err = read_status(runner as Arc<dyn CommandRunner>)
        .await
        .expect_err("spawn error should propagate");

    assert!(matches!(err, DopplerError::Spawn(_)));
}

#[tokio::test]
async fn read_status_empty_output_returns_zero_state() {
    let runner = MockRunner::new(vec![Ok(ok_output(""))]);
    let status = read_status(runner as Arc<dyn CommandRunner>)
        .await
        .expect("empty output is still success");

    assert_eq!(
        status,
        DopplerStatus {
            authenticated: false,
            scoped_bindings: vec![],
            default_project: None,
            default_config: None,
        }
    );
}
