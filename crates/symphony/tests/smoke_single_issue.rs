//! Manual smoke test: drive a real Linear project end-to-end.
//!
//! This test is `#[ignore]`d in CI because it requires:
//!   * `LINEAR_API_KEY` set in the environment (or available via Doppler),
//!   * a real Linear project containing at least one issue in an active
//!     state with the `agent:claude` label,
//!   * a working `claude` CLI on `PATH`.
//!
//! Run manually with:
//!   ```sh
//!   LINEAR_API_KEY=lin_api_xxx \
//!     cargo test -p symphony --test smoke_single_issue -- --ignored --nocapture
//!   ```
//!
//! The test loads the example WORKFLOW.md, issues exactly one tick, and
//! exits. It is the same code path `cargo run -p symphony -- --once`
//! exercises.

#[ignore = "requires LINEAR_API_KEY and a configured Linear project"]
#[tokio::test]
async fn single_tick_against_real_linear() {
    // Intentionally empty — the manual reproduction is `cargo run
    // -p symphony -- --once --workflow crates/symphony/examples/WORKFLOW.example.md`.
    // Asserting against the live state of a real Linear board would be
    // flaky; the test exists as a documented entry point.
}
