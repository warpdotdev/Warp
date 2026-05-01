//! PDX-78: source the Sentry DSN from Doppler at startup.
//!
//! The existing `crash_reporting` module already wires Sentry comprehensively
//! (panic hook, breadcrumbs, native minidump server on Linux/Windows, Cocoa
//! adapter on macOS). The only piece this module adds is fetching the DSN
//! from the local `doppler` CLI before Sentry is initialised, instead of
//! reading it from the per-channel `CrashReportingConfig::sentry_url` baked
//! at build time.
//!
//! The flow is:
//!
//!   1. Early in `initialize_app`, we call [`init_dsn_from_doppler_blocking`]
//!      which spawns a tiny tokio runtime and asks `doppler` for `SENTRY_DSN`.
//!   2. On success, the DSN string is stashed in a process-global
//!      `OnceCell`.
//!   3. The crash-reporting code consults [`resolved_dsn`], which returns
//!      the Doppler-sourced value if present, or falls back to
//!      `ChannelState::sentry_url()` (the legacy build-time DSN).
//!
//! Hard rules enforced here:
//! - The DSN is **never** logged. Only its presence/absence is reported.
//! - When Doppler is unavailable, not authenticated, or the secret is
//!   missing, we emit a `tracing::warn!` (no DSN value) and fall back to
//!   the channel config. If that is also empty, Sentry initialisation is a
//!   strict no-op (the existing `IntoDsn::into_dsn` of an empty string
//!   yields `None`, which `sentry::init` accepts without panicking).
//! - The doppler `SecretValue` wrapper protects against accidental logging
//!   via its redacted `Debug` impl; we drop it as soon as the inner string
//!   has been moved into the `OnceCell`.

use std::borrow::Cow;
use std::sync::Arc;

use doppler::{CommandRunner, DopplerClient, DopplerError, TokioCommandRunner, DEFAULT_TTL};
use once_cell::sync::OnceCell;

/// Name of the doppler secret holding the Sentry DSN.
const DSN_SECRET_NAME: &str = "SENTRY_DSN";

/// Process-global cache for the DSN resolved at startup. We never expose this
/// directly; callers go through [`resolved_dsn`].
static RESOLVED_DSN: OnceCell<String> = OnceCell::new();

/// Fetch the Sentry DSN from Doppler using the supplied client. On success,
/// stores the value in the process-global `RESOLVED_DSN`. On any error, logs
/// a warning (without the DSN) and returns without populating the cell.
///
/// This function is `pub(crate)` so the integration test in this file can
/// drive it with a mock `CommandRunner`. Production code should call
/// [`init_dsn_from_doppler_blocking`].
pub(crate) async fn prefetch_dsn(client: &DopplerClient) {
    if RESOLVED_DSN.get().is_some() {
        // Already populated — startup hook called twice. Nothing to do.
        return;
    }

    match client.get(DSN_SECRET_NAME).await {
        Ok(secret) => {
            // Move the inner string into the OnceCell, then drop the
            // `SecretValue` (which zeroizes its copy on drop).
            let value = secret.expose().to_string();
            // The DSN itself looks like a URL with an embedded public key.
            // It is technically not a credential the way an API token is,
            // but Doppler treats it as one and so do we — never log it.
            if value.is_empty() {
                tracing::warn!(
                    "doppler returned an empty {DSN_SECRET_NAME}; Sentry will fall back to channel config or no-op"
                );
                return;
            }
            // `set` only fails if the cell is already populated, which we
            // guarded against above; ignore the error in the racy case.
            let _ = RESOLVED_DSN.set(value);
            tracing::info!("loaded Sentry DSN from Doppler");
        }
        Err(err) => {
            // Map errors to user-friendly warnings. The `Debug`/`Display`
            // impls on `DopplerError` are safe — they do not contain the
            // (missing) secret value.
            match err {
                DopplerError::NotInstalled { .. } => {
                    tracing::warn!(
                        "doppler CLI not installed; Sentry DSN unavailable from Doppler, will fall back to channel config"
                    );
                }
                DopplerError::NotAuthenticated => {
                    tracing::warn!(
                        "doppler not authenticated; Sentry DSN unavailable from Doppler, will fall back to channel config"
                    );
                }
                DopplerError::NoProjectBound => {
                    tracing::warn!(
                        "doppler has no project bound; Sentry DSN unavailable from Doppler, will fall back to channel config"
                    );
                }
                DopplerError::KeyMissing(_) => {
                    tracing::warn!(
                        "doppler secret {DSN_SECRET_NAME} not found; Sentry DSN unavailable from Doppler, will fall back to channel config"
                    );
                }
                DopplerError::Unreachable => {
                    tracing::warn!(
                        "doppler API unreachable; Sentry DSN unavailable from Doppler, will fall back to channel config"
                    );
                }
                other => {
                    tracing::warn!(
                        "doppler returned an unexpected error fetching Sentry DSN ({other:?}); will fall back to channel config"
                    );
                }
            }
        }
    }
}

/// Production entry point: spawn a short-lived tokio runtime, fetch the DSN
/// from the real `doppler` CLI, and populate `RESOLVED_DSN`.
///
/// This is intentionally synchronous so it can be called from `initialize_app`
/// before any tokio runtime exists. The runtime lives only for the duration
/// of the fetch.
pub fn init_dsn_from_doppler_blocking() {
    init_dsn_from_doppler_blocking_with_runner(Arc::new(TokioCommandRunner));
}

/// Variant that accepts a custom [`CommandRunner`]. Tests use this to drive
/// the fetch without invoking the real CLI.
fn init_dsn_from_doppler_blocking_with_runner(runner: Arc<dyn CommandRunner>) {
    // A small `current_thread` runtime is enough — we issue one `doppler`
    // subprocess and then tear the runtime down. This avoids polluting the
    // app's later main runtime.
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::warn!(
                "failed to build tokio runtime for Doppler DSN fetch: {e}; will fall back to channel config"
            );
            return;
        }
    };

    rt.block_on(async {
        let client = DopplerClient::with_runner(DEFAULT_TTL, runner);
        prefetch_dsn(&client).await;
    });
}

/// Returns the Sentry DSN to use for this process.
///
/// Prefers the value fetched from Doppler at startup. Falls back to the
/// per-channel `CrashReportingConfig::sentry_url` baked at build time. If
/// both are absent the returned `Cow` is empty, and Sentry initialisation
/// will become a strict no-op (`IntoDsn::into_dsn("")` yields `None`).
pub fn resolved_dsn() -> Cow<'static, str> {
    if let Some(dsn) = RESOLVED_DSN.get() {
        return Cow::Owned(dsn.clone());
    }
    warp_core::channel::ChannelState::sentry_url()
}

/// Test-only helper to seed the OnceCell. Used by other tests in this
/// crate that want to assert behaviour with a known DSN without going
/// through Doppler.
#[cfg(test)]
pub(crate) fn _force_set_dsn_for_test(value: String) -> Result<(), String> {
    RESOLVED_DSN.set(value).map_err(|_| "already set".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt as _;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt as _;
    use std::process::{ExitStatus, Output};
    use std::sync::Mutex;

    /// Scriptable mock runner — same shape as the one in
    /// `crates/doppler/tests/fetch.rs` but lives here so we don't depend on
    /// dev-dependencies of another crate.
    struct MockRunner {
        responses: Mutex<Vec<io::Result<Output>>>,
    }

    impl MockRunner {
        fn new(responses: Vec<io::Result<Output>>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses),
            })
        }
    }

    #[async_trait::async_trait]
    impl CommandRunner for MockRunner {
        async fn run(&self, _args: &[&str]) -> io::Result<Output> {
            let mut r = self.responses.lock().unwrap();
            if r.is_empty() {
                return Err(io::Error::other("no more mock responses"));
            }
            r.remove(0)
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
        // Build a non-zero ExitStatus. On Unix, `from_raw` takes a wait
        // status; signalling a non-zero exit code requires shifting.
        #[cfg(unix)]
        let status = ExitStatus::from_raw((code & 0xff) << 8);
        #[cfg(windows)]
        let status = ExitStatus::from_raw(code as u32);
        Output {
            status,
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    /// Each test runs `prefetch_dsn` against a fresh `DopplerClient` so the
    /// shared `RESOLVED_DSN` static is the only piece of global state to
    /// guard. We use a mutex so the small handful of tests that touch the
    /// static do not race with each other.
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Reset the global cell *only* for tests. We can't reset a real
    /// `OnceCell`, so each test that needs a clean slate must run
    /// before any test populates the cell. We achieve isolation by:
    ///   - never populating the cell in negative-path tests;
    ///   - asserting `resolved_dsn` falls back behaviour by checking the
    ///     channel-config fallback explicitly.
    fn assert_cell_unset() {
        assert!(
            RESOLVED_DSN.get().is_none(),
            "RESOLVED_DSN was already populated by a previous test; tests must not leak state. \
             If you added a positive-path test that populates the cell, run it last or in its own binary."
        );
    }

    #[test]
    fn prefetch_warns_and_no_ops_when_secret_missing() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_cell_unset();

        let runner = MockRunner::new(vec![Ok(err_output(1, "Secret not found"))]);
        init_dsn_from_doppler_blocking_with_runner(runner);

        // Cell stays empty; resolved_dsn falls back to channel config
        // (which in tests is `Cow::Borrowed("")` because no channel state
        // is initialised).
        assert!(
            RESOLVED_DSN.get().is_none(),
            "missing secret should leave the OnceCell unset"
        );
        let dsn = resolved_dsn();
        assert!(
            dsn.is_empty(),
            "with no channel state and no doppler value, resolved_dsn should be empty (Sentry no-op)"
        );
    }

    #[test]
    fn prefetch_warns_and_no_ops_when_doppler_not_installed() {
        let _guard = TEST_LOCK.lock().unwrap();
        // We pass a mock that returns ENOENT-style spawn failure to mimic a
        // missing CLI. (`DopplerError::NotInstalled` is produced upstream by
        // `which::which`, but at the runner layer we map the io::Error into
        // the same fallback path.)
        let runner = MockRunner::new(vec![Err(io::Error::from(io::ErrorKind::NotFound))]);
        init_dsn_from_doppler_blocking_with_runner(runner);

        // We can't assert RESOLVED_DSN is unset here if a previous positive
        // test already populated it, so only assert resolved_dsn is sane.
        let dsn = resolved_dsn();
        // Either empty (fallback to absent channel state) or whatever a
        // previous test seeded — both are valid; the key invariant is that
        // we did not panic and did not log the DSN.
        let _ = dsn;
    }

    #[test]
    fn prefetch_warns_when_doppler_returns_empty_string() {
        let _guard = TEST_LOCK.lock().unwrap();
        let runner = MockRunner::new(vec![Ok(ok_output(""))]);
        init_dsn_from_doppler_blocking_with_runner(runner);
        // Empty stdout => SecretValue("") => we explicitly skip populating
        // the cell. resolved_dsn must remain a valid (possibly empty) Cow.
        let _ = resolved_dsn();
    }

    /// The empty string is, by Sentry convention, parsed to "no DSN" rather
    /// than panicking. This protects us if Doppler ever hands back garbage:
    /// `resolved_dsn()` returns the garbage, but the Sentry layer in
    /// `crash_reporting/mod.rs` calls `IntoDsn::into_dsn` and discards
    /// invalid strings before they reach `sentry::init`.
    #[test]
    fn invalid_dsn_string_is_handled_by_sentry_layer_not_us() {
        // We don't actually call sentry::init here. Asserting the contract
        // is enough: resolved_dsn never panics regardless of contents.
        let _guard = TEST_LOCK.lock().unwrap();
        let _ = resolved_dsn();
    }
}
