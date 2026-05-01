// SPDX-License-Identifier: AGPL-3.0-only

use std::io;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt as _;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt as _;
use std::process::{ExitStatus, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use doppler::{CommandRunner, DopplerClient, DopplerError};

/// A scriptable mock runner. Each `run` consumes the next queued response
/// and increments a call counter so tests can assert on cache behaviour.
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

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl CommandRunner for MockRunner {
    async fn run(&self, args: &[&str]) -> io::Result<Output> {
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
    // On unix, `from_raw(code << 8)` encodes a normal exit with `code`.
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

#[tokio::test]
async fn cache_hit_does_not_respawn() {
    let runner = MockRunner::new(vec![Ok(ok_output("super-secret\n"))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);

    let v1 = client.get("API_KEY").await.expect("first fetch");
    assert_eq!(v1.expose(), "super-secret");
    assert_eq!(runner.call_count(), 1);

    // Second call should hit the cache — no new mock response queued, and
    // call_count must remain at 1.
    let v2 = client.get("API_KEY").await.expect("cache hit");
    assert_eq!(v2.expose(), "super-secret");
    assert_eq!(runner.call_count(), 1, "cache hit should not respawn");
}

#[tokio::test]
async fn ttl_expiry_triggers_refetch() {
    let runner = MockRunner::new(vec![
        Ok(ok_output("v1\n")),
        Ok(ok_output("v2\n")),
    ]);
    let client = DopplerClient::with_runner(
        Duration::from_millis(50),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    let v1 = client.get("API_KEY").await.unwrap();
    assert_eq!(v1.expose(), "v1");
    assert_eq!(runner.call_count(), 1);

    // Wait past TTL.
    tokio::time::sleep(Duration::from_millis(80)).await;

    let v2 = client.get("API_KEY").await.unwrap();
    assert_eq!(v2.expose(), "v2", "expired entry should be refetched");
    assert_eq!(runner.call_count(), 2);
}

#[tokio::test]
async fn key_missing_parsed_from_stderr() {
    let runner = MockRunner::new(vec![Ok(err_output(
        1,
        "doppler: secret not found: API_KEY\n",
    ))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner as Arc<dyn CommandRunner>);

    let err = client.get("API_KEY").await.expect_err("should be error");
    match err {
        DopplerError::KeyMissing(name) => assert_eq!(name, "API_KEY"),
        other => panic!("expected KeyMissing, got {other:?}"),
    }
}

#[tokio::test]
async fn not_authenticated_parsed_from_stderr() {
    let runner = MockRunner::new(vec![Ok(err_output(
        1,
        "doppler: not authenticated. run `doppler login`\n",
    ))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner as Arc<dyn CommandRunner>);

    let err = client.get("API_KEY").await.expect_err("err");
    assert!(matches!(err, DopplerError::NotAuthenticated));
}

#[tokio::test]
async fn no_project_bound_parsed_from_stderr() {
    let runner = MockRunner::new(vec![Ok(err_output(1, "doppler: no config selected\n"))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner as Arc<dyn CommandRunner>);

    let err = client.get("API_KEY").await.expect_err("err");
    assert!(matches!(err, DopplerError::NoProjectBound));
}

#[tokio::test]
async fn invalidate_drops_entry() {
    let runner = MockRunner::new(vec![
        Ok(ok_output("v1\n")),
        Ok(ok_output("v2\n")),
    ]);
    let client = DopplerClient::with_runner(
        Duration::from_secs(60),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    client.get("K").await.unwrap();
    assert_eq!(runner.call_count(), 1);

    client.invalidate("K");

    let v = client.get("K").await.unwrap();
    assert_eq!(v.expose(), "v2");
    assert_eq!(runner.call_count(), 2);
}

#[tokio::test]
async fn clear_drops_all_entries() {
    let runner = MockRunner::new(vec![
        Ok(ok_output("a1\n")),
        Ok(ok_output("b1\n")),
        Ok(ok_output("a2\n")),
    ]);
    let client = DopplerClient::with_runner(
        Duration::from_secs(60),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    client.get("A").await.unwrap();
    client.get("B").await.unwrap();
    assert_eq!(runner.call_count(), 2);

    client.clear();

    let a = client.get("A").await.unwrap();
    assert_eq!(a.expose(), "a2");
    assert_eq!(runner.call_count(), 3);
}
