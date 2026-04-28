//! Shared retry primitives for HTTP-backed operations in the agent SDK.
//!
//! Both the end-of-run snapshot upload pipeline and the handoff snapshot download pipeline
//! need to retry transient HTTP failures on a bounded, predictable schedule. This module
//! centralizes the backoff policy, transient-vs-permanent classification, and retry loop so
//! the two call sites share a single source of truth.

use std::future::Future;
use std::time::Duration;

use anyhow::{anyhow, Result};
use warpui::duration_with_jitter;
use warpui::r#async::Timer;

pub(crate) use crate::server::retry_strategies::is_transient_http_error;

/// Maximum total attempts per operation (initial attempt plus retries on transient errors).
pub(crate) const MAX_ATTEMPTS: usize = 3;

/// Base backoff between retry attempts; each subsequent attempt multiplies by [`BACKOFF_FACTOR`].
pub(crate) const INITIAL_BACKOFF: Duration = Duration::from_millis(500);

/// Exponential growth factor for retry backoff.
pub(crate) const BACKOFF_FACTOR: f32 = 2.0;

/// Maximum jitter as a fraction of the backoff interval.
pub(crate) const BACKOFF_JITTER: f32 = 0.3;

/// Run `attempt_fn` with bounded exponential-backoff retries on transient failures.
///
/// `operation` is included in retry logs so concurrent callers can be distinguished.
///
/// `attempt_fn` is called repeatedly with a fresh `Future` per attempt, so callers that need
/// per-attempt state (e.g. cloning a request body) own that inside their closure.
///
/// Transient errors are retried up to [`MAX_ATTEMPTS`] total. Permanent errors return
/// immediately. A warning is logged between attempts so retries are visible in logs.
pub(crate) async fn with_bounded_retry<T, F, Fut>(operation: &str, mut attempt_fn: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut delay = INITIAL_BACKOFF;
    for attempt in 1..=MAX_ATTEMPTS {
        match attempt_fn().await {
            Ok(value) => return Ok(value),
            Err(e) if attempt >= MAX_ATTEMPTS || !is_transient_http_error(&e) => return Err(e),
            Err(e) => {
                log::warn!("{operation}: attempt {attempt}/{MAX_ATTEMPTS} failed, retrying: {e:#}");
                Timer::after(duration_with_jitter(delay, BACKOFF_JITTER)).await;
                delay = delay.mul_f32(BACKOFF_FACTOR);
            }
        }
    }
    // Unreachable when MAX_ATTEMPTS >= 1.
    Err(anyhow!(
        "retry loop exhausted without attempting operation (MAX_ATTEMPTS={MAX_ATTEMPTS})"
    ))
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
