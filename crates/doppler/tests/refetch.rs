// SPDX-License-Identifier: AGPL-3.0-only
//
// Integration tests for the 401-triggered refetch logic (PDX-54).

use std::io;
use std::path::Path;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt as _;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt as _;
use std::process::{ExitStatus, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use doppler::{
    with_refetch_on_unauthorized, CommandRunner, DopplerClient, RefetchError, SecretValue,
};

struct MockRunner {
    calls: AtomicUsize,
    responses: Mutex<Vec<io::Result<Output>>>,
}

impl MockRunner {
    fn new(responses: Vec<io::Result<Output>>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            responses: Mutex::new(responses),
        })
    }
    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl CommandRunner for MockRunner {
    async fn run(&self, _args: &[&str], _cwd: Option<&Path>) -> io::Result<Output> {
        self.calls.fetch_add(1, Ordering::SeqCst);
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

#[derive(Debug, thiserror::Error, Clone)]
enum FakeProviderError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("server error: {0}")]
    Server(String),
}

fn is_unauthorized(e: &FakeProviderError) -> bool {
    matches!(e, FakeProviderError::Unauthorized)
}

/// Counts how many times `op` was called and what secrets it saw.
struct OpRecorder {
    calls: AtomicUsize,
    secrets_seen: Mutex<Vec<String>>,
}

impl OpRecorder {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            secrets_seen: Mutex::new(vec![]),
        })
    }
    fn record(&self, secret: &SecretValue) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.secrets_seen
            .lock()
            .unwrap()
            .push(secret.expose().to_string());
    }
    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    fn secrets(&self) -> Vec<String> {
        self.secrets_seen.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn happy_path_calls_op_once_no_refetch() {
    let runner = MockRunner::new(vec![Ok(ok_output("token-v1\n"))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);
    let recorder = OpRecorder::new();

    let result: Result<u32, _> = with_refetch_on_unauthorized(
        &client,
        "API_KEY",
        None,
        |secret| {
            let recorder = recorder.clone();
            async move {
                recorder.record(&secret);
                Ok::<u32, FakeProviderError>(42)
            }
        },
        is_unauthorized,
    )
    .await;

    assert_eq!(result.unwrap(), 42);
    assert_eq!(recorder.count(), 1);
    assert_eq!(recorder.secrets(), vec!["token-v1"]);
    assert_eq!(runner.call_count(), 1, "no refetch should occur on success");
}

#[tokio::test]
async fn unauthorized_then_success_invalidates_and_refetches_once() {
    // First Doppler fetch returns "stale-token"; after 401, refetch returns
    // "fresh-token" and the second op call succeeds.
    let runner = MockRunner::new(vec![
        Ok(ok_output("stale-token\n")),
        Ok(ok_output("fresh-token\n")),
    ]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);
    let recorder = OpRecorder::new();

    let result: Result<u32, _> = with_refetch_on_unauthorized(
        &client,
        "API_KEY",
        None,
        |secret| {
            let recorder = recorder.clone();
            async move {
                recorder.record(&secret);
                if secret.expose() == "stale-token" {
                    Err(FakeProviderError::Unauthorized)
                } else {
                    Ok(7)
                }
            }
        },
        is_unauthorized,
    )
    .await;

    assert_eq!(result.unwrap(), 7);
    assert_eq!(recorder.count(), 2);
    assert_eq!(recorder.secrets(), vec!["stale-token", "fresh-token"]);
    assert_eq!(
        runner.call_count(),
        2,
        "should have spawned doppler twice: initial + refetch"
    );

    // The cache must now hold the fresh token, not the stale one. A
    // subsequent direct `get` should return fresh-token without spawning
    // doppler again.
    let cached = client.get("API_KEY", None).await.expect("cache hit");
    assert_eq!(cached.expose(), "fresh-token");
    assert_eq!(runner.call_count(), 2, "cache hit must not respawn");
}

#[tokio::test]
async fn unauthorized_twice_returns_provider_error_from_second_attempt() {
    let runner = MockRunner::new(vec![
        Ok(ok_output("token-1\n")),
        Ok(ok_output("token-2\n")),
    ]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);
    let recorder = OpRecorder::new();

    let result: Result<u32, _> = with_refetch_on_unauthorized(
        &client,
        "API_KEY",
        None,
        |secret| {
            let recorder = recorder.clone();
            async move {
                recorder.record(&secret);
                Err::<u32, _>(FakeProviderError::Unauthorized)
            }
        },
        is_unauthorized,
    )
    .await;

    match result {
        Err(RefetchError::Provider(FakeProviderError::Unauthorized)) => {}
        other => panic!("expected Provider(Unauthorized), got {other:?}"),
    }
    assert_eq!(recorder.count(), 2);
    assert_eq!(runner.call_count(), 2, "exactly one refetch (no infinite loop)");
}

#[tokio::test]
async fn non_unauthorized_error_short_circuits_no_refetch() {
    let runner = MockRunner::new(vec![Ok(ok_output("ok-token\n"))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);
    let recorder = OpRecorder::new();

    let result: Result<u32, _> = with_refetch_on_unauthorized(
        &client,
        "API_KEY",
        None,
        |secret| {
            let recorder = recorder.clone();
            async move {
                recorder.record(&secret);
                Err::<u32, _>(FakeProviderError::Server("500".to_string()))
            }
        },
        is_unauthorized,
    )
    .await;

    match result {
        Err(RefetchError::Provider(FakeProviderError::Server(msg))) => assert_eq!(msg, "500"),
        other => panic!("expected Provider(Server), got {other:?}"),
    }
    assert_eq!(recorder.count(), 1, "non-401 must not retry");
    assert_eq!(runner.call_count(), 1, "no refetch on non-401 error");
}

#[tokio::test]
async fn doppler_failure_during_initial_get_propagates_as_doppler_error() {
    // No mock responses queued → MockRunner returns io::Error → DopplerError::Spawn.
    let runner = MockRunner::new(vec![]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);
    let recorder = OpRecorder::new();

    let result: Result<u32, _> = with_refetch_on_unauthorized(
        &client,
        "API_KEY",
        None,
        |secret| {
            let recorder = recorder.clone();
            async move {
                recorder.record(&secret);
                Ok::<u32, FakeProviderError>(0)
            }
        },
        is_unauthorized,
    )
    .await;

    assert!(matches!(result, Err(RefetchError::Doppler(_))));
    assert_eq!(recorder.count(), 0, "op must not run when initial doppler fetch fails");
}

#[tokio::test]
async fn doppler_failure_during_refetch_propagates_as_doppler_error() {
    // Initial fetch succeeds; refetch fails (no second response queued).
    let runner = MockRunner::new(vec![Ok(ok_output("first-token\n"))]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);
    let recorder = OpRecorder::new();

    let result: Result<u32, _> = with_refetch_on_unauthorized(
        &client,
        "API_KEY",
        None,
        |secret| {
            let recorder = recorder.clone();
            async move {
                recorder.record(&secret);
                Err::<u32, _>(FakeProviderError::Unauthorized)
            }
        },
        is_unauthorized,
    )
    .await;

    assert!(matches!(result, Err(RefetchError::Doppler(_))));
    // Op called exactly once (initial); refetch step never makes it back to op.
    assert_eq!(recorder.count(), 1);
    assert_eq!(runner.call_count(), 2, "refetch attempt does spawn doppler once");
}

#[tokio::test]
async fn refetch_method_invalidates_cache_and_returns_fresh_value() {
    let runner = MockRunner::new(vec![
        Ok(ok_output("v1\n")),
        Ok(ok_output("v2\n")),
    ]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);

    let v1 = client.get("API_KEY", None).await.unwrap();
    assert_eq!(v1.expose(), "v1");
    assert_eq!(runner.call_count(), 1);

    // Refetch should return v2 even though cache had v1.
    let v2 = client.refetch("API_KEY", None).await.unwrap();
    assert_eq!(v2.expose(), "v2");
    assert_eq!(runner.call_count(), 2);

    // Subsequent get must read v2 from cache.
    let cached = client.get("API_KEY", None).await.unwrap();
    assert_eq!(cached.expose(), "v2");
    assert_eq!(runner.call_count(), 2, "cache hit, no respawn");
}

#[tokio::test]
async fn refetch_path_in_helper_uses_fresh_secret() {
    // Pre-populate the cache via a normal `get`. Then exercise the helper —
    // the first op call should see the cached value, and after the 401 the
    // second op call should see the freshly fetched value.
    let runner = MockRunner::new(vec![
        Ok(ok_output("cached\n")),
        Ok(ok_output("post-401\n")),
    ]);
    let client =
        DopplerClient::with_runner(Duration::from_secs(60), runner.clone() as Arc<dyn CommandRunner>);

    // Warm the cache.
    let warm = client.get("API_KEY", None).await.unwrap();
    assert_eq!(warm.expose(), "cached");
    assert_eq!(runner.call_count(), 1);

    let recorder = OpRecorder::new();
    let result: Result<u32, _> = with_refetch_on_unauthorized(
        &client,
        "API_KEY",
        None,
        |secret| {
            let recorder = recorder.clone();
            async move {
                recorder.record(&secret);
                if secret.expose() == "cached" {
                    Err(FakeProviderError::Unauthorized)
                } else {
                    Ok(99)
                }
            }
        },
        is_unauthorized,
    )
    .await;

    assert_eq!(result.unwrap(), 99);
    assert_eq!(recorder.secrets(), vec!["cached", "post-401"]);
    assert_eq!(runner.call_count(), 2, "1 warm + 1 refetch");
}
