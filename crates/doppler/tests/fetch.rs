// SPDX-License-Identifier: AGPL-3.0-only

use std::io;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt as _;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt as _;
use std::process::{ExitStatus, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use doppler::{CommandRunner, DopplerClient, DopplerError};

/// A scriptable mock runner. Each `run` consumes the next queued response,
/// increments a call counter, and records the (args, cwd) pair so tests can
/// assert on cache behaviour and cwd forwarding.
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

// ── Existing behaviour tests ──────────────────────────────────────────────────

#[tokio::test]
async fn cache_hit_does_not_respawn() {
    let runner = MockRunner::new(vec![Ok(ok_output("super-secret\n"))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);

    let v1 = client.get("API_KEY", None).await.expect("first fetch");
    assert_eq!(v1.expose(), "super-secret");
    assert_eq!(runner.call_count(), 1);

    // Second call should hit the cache — no new mock response queued, and
    // call_count must remain at 1.
    let v2 = client.get("API_KEY", None).await.expect("cache hit");
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

    let v1 = client.get("API_KEY", None).await.unwrap();
    assert_eq!(v1.expose(), "v1");
    assert_eq!(runner.call_count(), 1);

    // Wait past TTL.
    tokio::time::sleep(Duration::from_millis(80)).await;

    let v2 = client.get("API_KEY", None).await.unwrap();
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

    let err = client.get("API_KEY", None).await.expect_err("should be error");
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

    let err = client.get("API_KEY", None).await.expect_err("err");
    assert!(matches!(err, DopplerError::NotAuthenticated));
}

#[tokio::test]
async fn no_project_bound_parsed_from_stderr() {
    let runner = MockRunner::new(vec![Ok(err_output(1, "doppler: no config selected\n"))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner as Arc<dyn CommandRunner>);

    let err = client.get("API_KEY", None).await.expect_err("err");
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

    client.get("K", None).await.unwrap();
    assert_eq!(runner.call_count(), 1);

    client.invalidate("K", None);

    let v = client.get("K", None).await.unwrap();
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

    client.get("A", None).await.unwrap();
    client.get("B", None).await.unwrap();
    assert_eq!(runner.call_count(), 2);

    client.clear();

    let a = client.get("A", None).await.unwrap();
    assert_eq!(a.expose(), "a2");
    assert_eq!(runner.call_count(), 3);
}

// ── cwd-scoping tests (PDX-56) ────────────────────────────────────────────────

#[tokio::test]
async fn different_cwds_produce_separate_cache_entries() {
    // Two repos, same secret name — each should trigger its own CLI call
    // because the (cwd, name) cache key differs.
    let runner = MockRunner::new(vec![
        Ok(ok_output("secret-for-repo-a\n")),
        Ok(ok_output("secret-for-repo-b\n")),
    ]);
    let client = DopplerClient::with_runner(
        Duration::from_secs(60),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    let dir_a = PathBuf::from("/repos/service-a");
    let dir_b = PathBuf::from("/repos/service-b");

    let va = client.get("DATABASE_URL", Some(&dir_a)).await.unwrap();
    assert_eq!(va.expose(), "secret-for-repo-a");
    assert_eq!(runner.call_count(), 1);

    // Different cwd → must not hit the cache for dir_a's entry.
    let vb = client.get("DATABASE_URL", Some(&dir_b)).await.unwrap();
    assert_eq!(vb.expose(), "secret-for-repo-b");
    assert_eq!(runner.call_count(), 2, "different cwd must not share cache");
}

#[tokio::test]
async fn same_cwd_is_a_cache_hit() {
    let runner = MockRunner::new(vec![Ok(ok_output("cached-value\n"))]);
    let client = DopplerClient::with_runner(
        Duration::from_secs(60),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    let dir = PathBuf::from("/repos/service-a");

    client.get("API_KEY", Some(&dir)).await.unwrap();
    client.get("API_KEY", Some(&dir)).await.unwrap();

    assert_eq!(runner.call_count(), 1, "same (cwd, name) must be a cache hit");
}

#[tokio::test]
async fn none_cwd_and_explicit_cwd_are_separate_entries() {
    // A call with cwd=None (inherit process dir) and a call with an explicit
    // path must be treated as different contexts.
    let runner = MockRunner::new(vec![
        Ok(ok_output("process-dir-secret\n")),
        Ok(ok_output("explicit-dir-secret\n")),
    ]);
    let client = DopplerClient::with_runner(
        Duration::from_secs(60),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    let dir = PathBuf::from("/repos/myservice");

    let v_none = client.get("TOKEN", None).await.unwrap();
    assert_eq!(v_none.expose(), "process-dir-secret");

    let v_explicit = client.get("TOKEN", Some(&dir)).await.unwrap();
    assert_eq!(v_explicit.expose(), "explicit-dir-secret");

    assert_eq!(runner.call_count(), 2, "None and Some(path) are distinct cache keys");
}

#[tokio::test]
async fn invalidate_is_scoped_to_cwd() {
    // Invalidating a key under dir_a must not evict the cache entry for dir_b.
    let runner = MockRunner::new(vec![
        Ok(ok_output("a1\n")),
        Ok(ok_output("b1\n")),
        Ok(ok_output("a2\n")),
    ]);
    let client = DopplerClient::with_runner(
        Duration::from_secs(60),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    let dir_a = PathBuf::from("/repos/service-a");
    let dir_b = PathBuf::from("/repos/service-b");

    client.get("KEY", Some(&dir_a)).await.unwrap();
    client.get("KEY", Some(&dir_b)).await.unwrap();
    assert_eq!(runner.call_count(), 2);

    // Invalidate only dir_a's entry.
    client.invalidate("KEY", Some(&dir_a));

    // dir_b must still be cached — no new CLI call.
    let vb = client.get("KEY", Some(&dir_b)).await.unwrap();
    assert_eq!(vb.expose(), "b1");
    assert_eq!(runner.call_count(), 2, "dir_b cache must be untouched");

    // dir_a must be refetched.
    let va2 = client.get("KEY", Some(&dir_a)).await.unwrap();
    assert_eq!(va2.expose(), "a2");
    assert_eq!(runner.call_count(), 3, "dir_a must have been refetched");
}

#[tokio::test]
async fn cwd_is_forwarded_to_runner() {
    // The client must pass the caller-supplied cwd to the runner so Doppler
    // picks up the right `.doppler.yaml` for that directory.
    let runner = MockRunner::new(vec![Ok(ok_output("val\n"))]);
    let client = DopplerClient::with_runner(
        Duration::from_secs(60),
        runner.clone() as Arc<dyn CommandRunner>,
    );

    let dir = PathBuf::from("/repos/myservice");
    client.get("KEY", Some(&dir)).await.unwrap();

    let calls = runner.last_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].1.as_deref(),
        Some(dir.as_path()),
        "cwd must be forwarded to the runner"
    );
}
