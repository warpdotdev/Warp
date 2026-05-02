// SPDX-License-Identifier: AGPL-3.0-only
//
// Integration tests for MetadataClient.
//
// These tests use a scriptable MockRunner (same pattern as the doppler crate's
// own tests) so no real `doppler` binary is required.  They verify:
//   - that the right CLI arguments are forwarded
//   - that JSON is correctly parsed
//   - that error cases map to the expected DopplerError variant
//   - that has_secret is a pure name-list membership check (no value fetch)

use std::io;
use std::process::Output;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

use async_trait::async_trait;
use doppler::{CommandRunner, DopplerError};
use doppler_mcp::MetadataClient;

// ── MockRunner ─────────────────────────────────────────────────────────────

struct MockRunner {
    calls: AtomicUsize,
    responses: Mutex<Vec<io::Result<Output>>>,
    recorded_args: Mutex<Vec<Vec<String>>>,
}

impl MockRunner {
    fn new(responses: Vec<io::Result<Output>>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            responses: Mutex::new(responses),
            recorded_args: Mutex::new(Vec::new()),
        })
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn nth_args(&self, n: usize) -> Vec<String> {
        self.recorded_args.lock().unwrap()[n].clone()
    }
}

#[async_trait]
impl CommandRunner for MockRunner {
    async fn run(&self, args: &[&str]) -> io::Result<Output> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.recorded_args
            .lock()
            .unwrap()
            .push(args.iter().map(|s| s.to_string()).collect());
        let mut guard = self.responses.lock().unwrap();
        if guard.is_empty() {
            return Err(io::Error::other("no more mock responses"));
        }
        Ok(guard.remove(0)?)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn ok_output(stdout: &str) -> io::Result<Output> {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt as _;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt as _;
    Ok(Output {
        #[cfg(unix)]
        status: std::process::ExitStatus::from_raw(0),
        #[cfg(windows)]
        status: std::process::ExitStatus::from_raw(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

fn err_output(code: i32, stderr: &str) -> io::Result<Output> {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt as _;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt as _;
    Ok(Output {
        #[cfg(unix)]
        status: std::process::ExitStatus::from_raw(code << 8),
        #[cfg(windows)]
        status: std::process::ExitStatus::from_raw(code as u32),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    })
}

fn client_with(responses: Vec<io::Result<Output>>) -> (Arc<MockRunner>, MetadataClient) {
    let runner = MockRunner::new(responses);
    let client = MetadataClient::with_runner(runner.clone() as Arc<dyn CommandRunner>);
    (runner, client)
}

// ── list_projects ──────────────────────────────────────────────────────────

#[tokio::test]
async fn list_projects_passes_correct_args() {
    let payload = r#"[{"name":"Backend","slug":"backend","description":""}]"#;
    let (runner, client) = client_with(vec![ok_output(payload)]);
    client.list_projects().await.unwrap();
    let args = runner.nth_args(0);
    assert_eq!(args, vec!["projects", "list", "--json"]);
}

#[tokio::test]
async fn list_projects_parses_multiple_projects() {
    let payload = r#"[
        {"name":"Backend","slug":"backend","description":"API"},
        {"name":"Frontend","slug":"frontend","description":""}
    ]"#;
    let (_, client) = client_with(vec![ok_output(payload)]);
    let projects = client.list_projects().await.unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].slug, "backend");
    assert_eq!(projects[1].name, "Frontend");
}

#[tokio::test]
async fn list_projects_ignores_extra_fields() {
    let payload = r#"[{"name":"X","slug":"x","description":"","created_at":"2024","unknown":99}]"#;
    let (_, client) = client_with(vec![ok_output(payload)]);
    let projects = client.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].slug, "x");
}

#[tokio::test]
async fn list_projects_not_authenticated() {
    let (_, client) = client_with(vec![err_output(1, "not authenticated")]);
    let err = client.list_projects().await.unwrap_err();
    assert!(matches!(err, DopplerError::NotAuthenticated));
}

#[tokio::test]
async fn list_projects_network_error() {
    let (_, client) = client_with(vec![err_output(1, "could not reach api.doppler.com")]);
    let err = client.list_projects().await.unwrap_err();
    assert!(matches!(err, DopplerError::Unreachable));
}

// ── list_configs ───────────────────────────────────────────────────────────

#[tokio::test]
async fn list_configs_passes_project_arg() {
    let payload = r#"{"page":1,"configs":[{"name":"dev","environment":"dev","locked":false}]}"#;
    let (runner, client) = client_with(vec![ok_output(payload)]);
    client.list_configs("my-project").await.unwrap();
    let args = runner.nth_args(0);
    assert_eq!(args, vec!["configs", "--project", "my-project", "--json"]);
}

#[tokio::test]
async fn list_configs_parses_wrapped_format() {
    let payload = r#"{"page":1,"configs":[
        {"name":"dev","environment":"dev","locked":false},
        {"name":"prd","environment":"prd","locked":true}
    ]}"#;
    let (_, client) = client_with(vec![ok_output(payload)]);
    let configs = client.list_configs("proj").await.unwrap();
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0].name, "dev");
    assert!(configs[1].locked);
}

#[tokio::test]
async fn list_configs_parses_bare_array_format() {
    let payload = r#"[{"name":"dev","environment":"dev","locked":false}]"#;
    let (_, client) = client_with(vec![ok_output(payload)]);
    let configs = client.list_configs("proj").await.unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].environment, "dev");
}

// ── list_secret_names ──────────────────────────────────────────────────────

#[tokio::test]
async fn list_secret_names_passes_correct_args() {
    let (runner, client) = client_with(vec![ok_output("API_KEY\n")]);
    client.list_secret_names("my-project", "dev").await.unwrap();
    let args = runner.nth_args(0);
    assert_eq!(
        args,
        vec!["secrets", "names", "--project", "my-project", "--config", "dev"]
    );
}

#[tokio::test]
async fn list_secret_names_newline_format() {
    let (_, client) = client_with(vec![ok_output("API_KEY\nDB_HOST\nDB_PORT\n")]);
    let names = client.list_secret_names("p", "dev").await.unwrap();
    assert_eq!(names, vec!["API_KEY", "DB_HOST", "DB_PORT"]);
}

#[tokio::test]
async fn list_secret_names_json_array_format() {
    let (_, client) = client_with(vec![ok_output(r#"["API_KEY","DB_HOST"]"#)]);
    let names = client.list_secret_names("p", "dev").await.unwrap();
    assert_eq!(names, vec!["API_KEY", "DB_HOST"]);
}

#[tokio::test]
async fn list_secret_names_empty_config_ok() {
    let (_, client) = client_with(vec![ok_output("")]);
    let names = client.list_secret_names("p", "dev").await.unwrap();
    assert!(names.is_empty());
}

#[tokio::test]
async fn list_secret_names_strips_whitespace() {
    let (_, client) = client_with(vec![ok_output("  API_KEY  \n  DB_HOST  \n")]);
    let names = client.list_secret_names("p", "dev").await.unwrap();
    assert_eq!(names, vec!["API_KEY", "DB_HOST"]);
}

// ── has_secret ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn has_secret_returns_true_when_present() {
    let (runner, client) = client_with(vec![ok_output("API_KEY\nDB_HOST\n")]);
    let exists = client.has_secret("p", "dev", "API_KEY").await.unwrap();
    assert!(exists);
    // Only one CLI call should have been made (list names, no value fetch).
    assert_eq!(runner.call_count(), 1);
}

#[tokio::test]
async fn has_secret_returns_false_when_absent() {
    let (_, client) = client_with(vec![ok_output("API_KEY\n")]);
    let exists = client.has_secret("p", "dev", "MISSING_KEY").await.unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn has_secret_is_case_sensitive() {
    let (_, client) = client_with(vec![ok_output("API_KEY\n")]);
    // "api_key" != "API_KEY" — Doppler keys are conventionally uppercase.
    let exists = client.has_secret("p", "dev", "api_key").await.unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn has_secret_never_fetches_value() {
    // The mock only has one response: the name list.
    // If has_secret tried to call `doppler secrets get` afterwards it would
    // panic with "no more mock responses".
    let (runner, client) = client_with(vec![ok_output("API_KEY\n")]);
    client.has_secret("p", "dev", "API_KEY").await.unwrap();
    assert_eq!(runner.call_count(), 1, "exactly one CLI call — names only");
    let args = runner.nth_args(0);
    // Verify it called `secrets names`, not `secrets get`.
    assert_eq!(args[0], "secrets");
    assert_eq!(args[1], "names");
}
