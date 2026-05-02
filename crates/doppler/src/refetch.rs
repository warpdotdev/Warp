// SPDX-License-Identifier: AGPL-3.0-only
//
// PDX-54 [A4.6]: 401-triggered re-fetch logic.
//
// When a downstream provider returns 401, the cached secret is presumed
// stale (rotated upstream, revoked, etc.). This module provides:
//
// * `DopplerClient::refetch` — the primitive: drop the cache entry, then
//   re-`get` from the CLI. Use it directly when the caller wants to drive
//   its own retry logic.
//
// * `with_refetch_on_unauthorized` — a higher-order helper that runs the
//   caller's operation, and if the operation returns an error the caller
//   classifies as "unauthorized", invalidates and refetches the secret
//   exactly once before retrying. The single-retry budget bounds the
//   damage when the credential really is broken — we never loop forever.

use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

use crate::{CommandRunner, DopplerClient, DopplerError, SecretValue};

/// Error returned by [`with_refetch_on_unauthorized`].
///
/// Generic over the provider's error type `E` — typically the HTTP client
/// error type the caller already uses.
#[derive(Debug, Error)]
pub enum RefetchError<E>
where
    E: std::error::Error + 'static,
{
    /// The Doppler CLI itself failed (initial fetch or refetch).
    #[error("doppler error: {0}")]
    Doppler(#[source] DopplerError),
    /// The provider operation failed and the failure was *not* classified
    /// as unauthorized, or it was unauthorized but the post-refetch retry
    /// also failed.
    #[error("provider error: {0}")]
    Provider(#[source] E),
}

impl<E: std::error::Error + 'static> From<DopplerError> for RefetchError<E> {
    fn from(value: DopplerError) -> Self {
        RefetchError::Doppler(value)
    }
}

impl DopplerClient {
    /// Drop the cached entry for `(cwd, name)` and re-fetch from the CLI.
    ///
    /// The new value replaces any existing cache entry. Use this when an
    /// out-of-band signal (e.g. a 401 from a downstream provider) has
    /// invalidated the cached secret.
    pub async fn refetch(
        &self,
        name: &str,
        cwd: Option<&Path>,
    ) -> Result<SecretValue, DopplerError> {
        // `invalidate` is synchronous and lock-free under contention, so
        // the next `get` cannot return a stale entry.
        self.invalidate(name, cwd);
        self.get(name, cwd).await
    }
}

/// Run `op(secret)` once, and if it returns an error the caller classifies
/// as unauthorized, refetch the secret and retry exactly once.
///
/// `is_unauthorized` is a predicate the caller supplies to inspect the
/// provider's error type — typically `|e| e.status() == 401` or similar.
/// The closure may also be lifted out of an enum variant (`matches!(e,
/// MyError::Unauthorized)`) for clients that have already classified the
/// failure mode.
///
/// Returns:
/// - `Ok(t)` from the first successful call (no refetch).
/// - `Ok(t)` from the post-refetch retry.
/// - `Err(RefetchError::Doppler(_))` if Doppler itself fails (initial
///   `get` or refetch).
/// - `Err(RefetchError::Provider(_))` for the original error if it was not
///   unauthorized, or for the second-attempt error if the retry also
///   failed.
///
/// `op` is called at most twice. If both attempts return unauthorized,
/// the second error is the one returned — letting the caller observe
/// "we already tried fresh credentials and it's still 401".
pub async fn with_refetch_on_unauthorized<Op, Fut, T, E>(
    client: &DopplerClient,
    name: &str,
    cwd: Option<&Path>,
    op: Op,
    is_unauthorized: impl Fn(&E) -> bool,
) -> Result<T, RefetchError<E>>
where
    Op: Fn(SecretValue) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::error::Error + 'static,
{
    // First attempt with whatever the cache currently holds.
    let first = client.get(name, cwd).await?;
    match op(first).await {
        Ok(t) => return Ok(t),
        Err(e) if is_unauthorized(&e) => {
            tracing::debug!("doppler: refetching {name} after 401 from provider");
            // Fall through to the refetch + retry path. The original
            // error `e` is intentionally dropped — only the retry's
            // outcome propagates.
            let _ = e;
        }
        Err(e) => return Err(RefetchError::Provider(e)),
    }

    let fresh = client.refetch(name, cwd).await?;
    op(fresh).await.map_err(RefetchError::Provider)
}

/// Convenience: build a [`DopplerClient`] with the supplied runner and
/// then run [`with_refetch_on_unauthorized`] against it. Lets tests
/// construct a client per-test without the surrounding ceremony.
pub async fn with_refetch_on_unauthorized_using_runner<Op, Fut, T, E>(
    runner: Arc<dyn CommandRunner>,
    ttl: std::time::Duration,
    name: &str,
    cwd: Option<&Path>,
    op: Op,
    is_unauthorized: impl Fn(&E) -> bool,
) -> Result<T, RefetchError<E>>
where
    Op: Fn(SecretValue) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::error::Error + 'static,
{
    let client = DopplerClient::with_runner(ttl, runner);
    with_refetch_on_unauthorized(&client, name, cwd, op, is_unauthorized).await
}
