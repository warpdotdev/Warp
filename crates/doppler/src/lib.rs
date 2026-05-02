// SPDX-License-Identifier: AGPL-3.0-only
//
// Doppler CLI integration: detection (PDX-49) and TTL-cached secret fetcher
// (PDX-53). This crate is intentionally narrow — it only wraps the local
// `doppler` CLI. Login flows, project pickers, status readers, refetch on
// 401, error-state UI and multi-account scoping live elsewhere.
//
// Hard rules enforced here:
//   * Secret values are NEVER written to logs, files, or `Debug` output.
//   * Secrets are NEVER persisted to disk; the cache is in-memory only.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Output;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

mod runner;
mod status;

pub use runner::{CommandRunner, TokioCommandRunner};
pub use status::{parse_configure_all, read_status, DopplerStatus, ScopedBinding};

/// Default time-to-live for cached secrets. Five minutes balances
/// responsiveness with avoiding excessive CLI invocations.
pub const DEFAULT_TTL: Duration = Duration::from_secs(5 * 60);

/// A wrapper around a fetched secret string.
///
/// The wrapper is the *only* sanctioned way to hold a doppler secret in
/// memory. It zeroizes on drop and its `Debug` impl never reveals the value.
pub struct SecretValue(zeroize::Zeroizing<String>);

impl SecretValue {
    /// Construct from an owned string. Trims a single trailing newline if
    /// present (the doppler CLI emits one with `--plain`).
    fn new(mut value: String) -> Self {
        if value.ends_with('\n') {
            value.pop();
            if value.ends_with('\r') {
                value.pop();
            }
        }
        Self(zeroize::Zeroizing::new(value))
    }

    /// Returns the underlying secret. Callers MUST NOT log, persist, or
    /// otherwise leak the returned string.
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue(***)")
    }
}

impl Clone for SecretValue {
    fn clone(&self) -> Self {
        Self(zeroize::Zeroizing::new(self.0.as_str().to_string()))
    }
}

/// Errors produced by the doppler integration.
#[derive(Debug, thiserror::Error)]
pub enum DopplerError {
    /// The `doppler` binary was not found on `PATH`.
    #[error("doppler CLI not installed: {install_hint}")]
    NotInstalled { install_hint: String },

    /// Doppler reported that the user is not logged in.
    #[error("doppler is not authenticated; run `doppler login`")]
    NotAuthenticated,

    /// The current working directory has no doppler project/config bound.
    #[error("no doppler project bound; run `doppler setup`")]
    NoProjectBound,

    /// The requested secret name does not exist in the bound config.
    #[error("doppler secret not found: {0}")]
    KeyMissing(String),

    /// Doppler API was unreachable.
    #[error("doppler API unreachable")]
    Unreachable,

    /// Could not spawn the `doppler` process.
    #[error("failed to spawn doppler: {0}")]
    Spawn(#[from] std::io::Error),

    /// Doppler exited non-zero with an unrecognised stderr.
    #[error("doppler exited with code {code}: {stderr}")]
    NonZeroExit { code: i32, stderr: String },
}

/// Detect the `doppler` binary on `PATH`.
///
/// Returns the absolute path on success. On failure, returns
/// [`DopplerError::NotInstalled`] with a platform-specific install hint.
pub fn detect() -> Result<PathBuf, DopplerError> {
    match which::which("doppler") {
        Ok(path) => Ok(path),
        Err(_) => Err(DopplerError::NotInstalled {
            install_hint: install_hint().to_string(),
        }),
    }
}

/// Returns the platform-specific install hint shown to users when the
/// `doppler` binary cannot be found.
pub fn install_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "brew install dopplerhq/cli/doppler"
    } else if cfg!(target_os = "linux") {
        "curl -Ls https://cli.doppler.com/install.sh | sh"
    } else if cfg!(target_os = "windows") {
        "scoop install doppler"
    } else {
        "https://docs.doppler.com/docs/cli"
    }
}

struct CacheEntry {
    value: SecretValue,
    expires_at: Instant,
}

/// Async client for fetching secrets from the local `doppler` CLI with an
/// in-memory TTL cache.
pub struct DopplerClient {
    ttl: Duration,
    cache: RwLock<HashMap<String, CacheEntry>>,
    runner: Arc<dyn CommandRunner>,
}

impl DopplerClient {
    /// Construct a client with the given TTL, using the default
    /// [`TokioCommandRunner`] to spawn `doppler`.
    pub fn new(ttl: Duration) -> Self {
        Self::with_runner(ttl, Arc::new(TokioCommandRunner))
    }

    /// Construct a client with a custom [`CommandRunner`]. Used by tests to
    /// substitute the real CLI.
    pub fn with_runner(ttl: Duration, runner: Arc<dyn CommandRunner>) -> Self {
        Self {
            ttl,
            cache: RwLock::new(HashMap::new()),
            runner,
        }
    }

    /// Fetch a secret by name. Returns a cached value if one exists and is
    /// still within TTL, otherwise spawns `doppler secrets get NAME --plain`.
    pub async fn get(&self, name: &str) -> Result<SecretValue, DopplerError> {
        // Fast path: cache hit.
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(name) {
                if entry.expires_at > Instant::now() {
                    tracing::debug!("doppler cache hit for {}", name);
                    return Ok(entry.value.clone());
                }
            }
        }

        // Slow path: drop expired entry (if any) and refetch.
        {
            let mut cache = self.cache.write().await;
            if let Some(entry) = cache.get(name) {
                if entry.expires_at <= Instant::now() {
                    cache.remove(name);
                }
            }
        }

        tracing::debug!("doppler fetching {}", name);
        let output = self
            .runner
            .run(&["secrets", "get", name, "--plain"])
            .await?;

        let value = parse_output(name, output)?;

        // Cache.
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                name.to_string(),
                CacheEntry {
                    value: value.clone(),
                    expires_at: Instant::now() + self.ttl,
                },
            );
        }

        Ok(value)
    }

    /// Drop a single cached entry.
    ///
    /// Synchronous: uses an opportunistic non-blocking write. Concurrent
    /// `get` callers may briefly hold the lock; in that case the caller can
    /// retry. In practice the cache is uncontended.
    pub fn invalidate(&self, name: &str) {
        loop {
            match self.cache.try_write() {
                Ok(mut cache) => {
                    cache.remove(name);
                    return;
                }
                Err(_) => std::thread::yield_now(),
            }
        }
    }

    /// Drop all cached entries.
    pub fn clear(&self) {
        loop {
            match self.cache.try_write() {
                Ok(mut cache) => {
                    cache.clear();
                    return;
                }
                Err(_) => std::thread::yield_now(),
            }
        }
    }
}

impl Default for DopplerClient {
    fn default() -> Self {
        Self::new(DEFAULT_TTL)
    }
}

/// Parse the output of `doppler secrets get NAME --plain`, mapping known
/// non-zero exit messages onto the proper [`DopplerError`] variants.
fn parse_output(name: &str, output: Output) -> Result<SecretValue, DopplerError> {
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        return Ok(SecretValue::new(stdout));
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let lower = stderr.to_lowercase();

    if lower.contains("not authenticated") || lower.contains("you must login") {
        return Err(DopplerError::NotAuthenticated);
    }
    if lower.contains("no config selected")
        || lower.contains("no project")
        || lower.contains("setup configuration")
    {
        return Err(DopplerError::NoProjectBound);
    }
    if lower.contains("secret not found")
        || lower.contains(&format!("could not find secret \"{}\"", name.to_lowercase()))
        || lower.contains(&format!("secret '{}' not found", name.to_lowercase()))
    {
        return Err(DopplerError::KeyMissing(name.to_string()));
    }
    if lower.contains("could not reach") || lower.contains("network") || lower.contains("dial tcp")
    {
        return Err(DopplerError::Unreachable);
    }

    Err(DopplerError::NonZeroExit {
        code: output.status.code().unwrap_or(-1),
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_hint_is_nonempty() {
        assert!(!install_hint().is_empty());
    }

    #[test]
    fn secret_value_debug_redacts() {
        let s = SecretValue::new("super-secret".to_string());
        assert_eq!(format!("{:?}", s), "SecretValue(***)");
        assert_eq!(s.expose(), "super-secret");
    }

    #[test]
    fn secret_value_trims_trailing_newline() {
        let s = SecretValue::new("hello\n".to_string());
        assert_eq!(s.expose(), "hello");
        let s = SecretValue::new("hello\r\n".to_string());
        assert_eq!(s.expose(), "hello");
        let s = SecretValue::new("no-newline".to_string());
        assert_eq!(s.expose(), "no-newline");
    }
}
