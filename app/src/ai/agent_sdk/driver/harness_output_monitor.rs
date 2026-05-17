//! Background monitor that scans the running harness block for known runtime
//! failure substrings (e.g. invalid API key, exhausted credits) and reports
//! the first hit so the driver can fail the task fast instead of letting the
//! harness drift mid-run.
//!
//! The monitor is feature-parity with the auth preflight check on the
//! failure-reporting side (`Failed + AuthenticationRequired`), but instead
//! of running its own command it observes the main harness CLI block via
//! `TerminalDriver::find_first_match_in_block_output`, which reuses the
//! existing find-feature DFA infrastructure (`RegexDFAs`).
//!
//! The set of needles is per-harness, supplied by each
//! `ThirdPartyHarness::runtime_error_patterns` impl, so adding a new pattern
//! is a one-line change in the harness file.

use std::sync::Arc;
use std::time::Duration;

use regex::escape;
use warpui::ModelSpawner;

use super::terminal::BlockOutputMatch;
use super::AgentDriver;
use crate::terminal::model::block::BlockId;
use crate::terminal::model::find::RegexDFAs;

/// Adaptive polling schedule expressed as `(poll_interval, phase_duration)`.
/// The first phase runs short polls so fast-failing harnesses are caught
/// quickly; the second phase backs off so longer healthy runs don't pay a
/// per-second cost.
///
/// Total budget: 30s (six 5s ticks) + 60s (four 15s ticks) = 90s, 10 polls.
const SCAN_SCHEDULE: &[(Duration, Duration)] = &[
    (Duration::from_secs(5), Duration::from_secs(30)),
    (Duration::from_secs(15), Duration::from_secs(60)),
];

/// Gap between consecutive plaintext snapshots while confirming a
/// detected runtime failure. The harness has this much time to either
/// (a) print more output and keep the loop alive, or (b) stay
/// completely quiet so we can declare it stalled.
const STALL_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Total budget for the stall-confirmation loop. After this elapses
/// without two consecutive byte-identical snapshots, we treat the
/// detection as unconfirmed and let the harness continue running so its
/// own retry logic gets room to recover.
const STALL_CONFIRMATION_BUDGET: Duration = Duration::from_secs(60);

/// Result of a successful scan tick: the originating needle (so the driver
/// can surface the exact pattern that matched in the failure message) plus
/// the matching row(s) as plaintext.
#[derive(Debug, Clone)]
pub(crate) struct DetectedHarnessError {
    pub pattern: String,
    pub excerpt: String,
}

/// Build a combined case-insensitive DFA from the harness's static patterns.
///
/// Each needle is regex-escaped so substrings match literally. Returns
/// `None` when `patterns` is empty (the scanner becomes a no-op) or when
/// DFA construction fails — the latter should never happen for escaped
/// literals, but is logged at warn level when it does.
pub(crate) fn build_dfas(patterns: &[&'static str]) -> Option<RegexDFAs> {
    if patterns.is_empty() {
        return None;
    }
    let escaped: Vec<String> = patterns.iter().map(|p| escape(p)).collect();
    let refs: Vec<&str> = escaped.iter().map(String::as_str).collect();
    match RegexDFAs::new_many(
        &refs, false, // enable_unicode_word_boundary
        false, // case_sensitive
    ) {
        Ok(dfas) => Some(dfas),
        Err(err) => {
            log::warn!("Failed to build harness output DFAs: {err}");
            None
        }
    }
}

/// Map a `matched_text` produced by the combined DFA back to the originating
/// needle. The DFA matched `(p1|p2|…)`, so the matched substring is exactly
/// one of `patterns` up to case — we lowercase-compare to identify it.
pub(crate) fn pattern_for_match(
    matched_text: &str,
    patterns: &[&'static str],
) -> Option<&'static str> {
    let matched_lower = matched_text.to_lowercase();
    patterns
        .iter()
        .copied()
        .find(|p| p.to_lowercase() == matched_lower)
}

/// Returns `true` when the harness block produced no new visible output
/// between `before` and `after`. Byte-identical plaintext means no fresh
/// API request output, no spinner frame change, no scrollback movement —
/// the harness is genuinely stuck on the line that matched our pattern.
///
/// Either side being `None` (a missed snapshot fetch) returns `false`
/// so we default to "not confirmed" rather than killing the harness on a
/// transient lookup error.
fn outputs_stalled(before: Option<&str>, after: Option<&str>) -> bool {
    matches!((before, after), (Some(a), Some(b)) if a == b)
}

/// Look up the first DFA match in the harness block via the foreground
/// terminal driver. `None` when the block is gone or no pattern matches.
async fn find_match_once(
    block_id: &BlockId,
    dfas: &Arc<RegexDFAs>,
    foreground: &ModelSpawner<AgentDriver>,
) -> Option<BlockOutputMatch> {
    let block_id_for_tick = block_id.clone();
    let dfas_for_tick = Arc::clone(dfas);
    foreground
        .spawn(move |me, ctx| {
            me.terminal_driver
                .as_ref(ctx)
                .find_first_match_in_block_output(&block_id_for_tick, &dfas_for_tick, ctx)
        })
        .await
        .ok()
        .flatten()
}

/// Fetch the harness block's visible plaintext (no ANSI; secrets
/// obfuscated). Used by the stall confirmation loop to compare two
/// snapshots taken `STALL_POLL_INTERVAL` apart.
async fn fetch_plaintext(
    block_id: &BlockId,
    foreground: &ModelSpawner<AgentDriver>,
) -> Option<String> {
    let block_id_for_tick = block_id.clone();
    foreground
        .spawn(move |me, ctx| {
            me.terminal_driver
                .as_ref(ctx)
                .block_output_plaintext(&block_id_for_tick, ctx)
        })
        .await
        .ok()
        .flatten()
}

/// Run the stall-confirmation loop after a pattern hit.
///
/// Polls the harness block's plaintext every [`STALL_POLL_INTERVAL`] for up
/// to [`STALL_CONFIRMATION_BUDGET`]. Returns `(Some(hit), elapsed)` once
/// two consecutive snapshots are byte-identical AND a pattern is still
/// present in the block at that point. Returns `(None, elapsed)` when the
/// budget exhausts without stabilization, or when the pattern scrolled
/// out of the visible window during recovery.
///
/// `elapsed` is always reported back so the outer scan schedule's budget
/// can absorb the confirmation time — a flaky harness that keeps
/// retripping the same pattern can't extend the watch window
/// indefinitely.
async fn confirm_stall(
    block_id: &BlockId,
    dfas: &Arc<RegexDFAs>,
    foreground: &ModelSpawner<AgentDriver>,
) -> (Option<BlockOutputMatch>, Duration) {
    let mut previous = fetch_plaintext(block_id, foreground).await;
    let mut elapsed = Duration::ZERO;
    while elapsed < STALL_CONFIRMATION_BUDGET {
        warpui::r#async::Timer::after(STALL_POLL_INTERVAL).await;
        elapsed += STALL_POLL_INTERVAL;
        let current = fetch_plaintext(block_id, foreground).await;
        if outputs_stalled(previous.as_deref(), current.as_deref()) {
            // Output settled. Re-run the DFA so we report the post-dwell
            // match in case the row positions shifted while we waited.
            return (find_match_once(block_id, dfas, foreground).await, elapsed);
        }
        previous = current;
    }
    (None, elapsed)
}

/// Watch the given block for harness output errors on the
/// [`SCAN_SCHEDULE`].
///
/// On every pattern hit, run a stall-confirmation loop (up to
/// [`STALL_CONFIRMATION_BUDGET`]) and only resolve with
/// `Some(DetectedHarnessError)` when the harness output stabilizes with
/// the pattern still present. If the harness keeps producing output
/// (e.g. spinner frames during an automatic retry), the detection is
/// dropped and the outer schedule resumes normal polling.
///
/// Returns `None` when the schedule completes without a confirmed hit
/// (or when `patterns` is empty / DFA construction fails).
/// Cancellation-safe: dropping the future stops both loops.
pub(crate) async fn watch_block_for_errors(
    block_id: BlockId,
    patterns: &'static [&'static str],
    foreground: &ModelSpawner<AgentDriver>,
) -> Option<DetectedHarnessError> {
    if patterns.is_empty() {
        return None;
    }
    let dfas = Arc::new(build_dfas(patterns)?);

    for &(interval, phase_duration) in SCAN_SCHEDULE {
        let mut elapsed = Duration::ZERO;
        while elapsed < phase_duration {
            warpui::r#async::Timer::after(interval).await;
            elapsed += interval;

            if find_match_once(&block_id, &dfas, foreground)
                .await
                .is_none()
            {
                continue;
            }

            // Candidate match observed. Confirm via the stall loop so we
            // don't false-positive while the harness is mid-retry.
            let (confirmed, confirmation_elapsed) =
                confirm_stall(&block_id, &dfas, foreground).await;
            elapsed += confirmation_elapsed;

            let Some(hit) = confirmed else {
                log::info!(
                    "Detected harness failure pattern but output never \
                     stabilized within {STALL_CONFIRMATION_BUDGET:?}; \
                     deferring (harness may be retrying)"
                );
                continue;
            };

            // The DFA matched one of our needles, so `pattern_for_match`
            // should always resolve. Fall back to the matched substring
            // verbatim if it somehow doesn't.
            let pattern = pattern_for_match(&hit.matched_text, patterns)
                .map(str::to_owned)
                .unwrap_or_else(|| hit.matched_text.clone());
            return Some(DetectedHarnessError {
                pattern,
                excerpt: cap_excerpt(&hit.excerpt),
            });
        }
    }
    None
}

/// Cap excerpt length so we don't blow up status messages or logs with
/// terminal-width rows.
const EXCERPT_MAX_LEN: usize = 240;
fn cap_excerpt(excerpt: &str) -> String {
    if excerpt.chars().count() <= EXCERPT_MAX_LEN {
        return excerpt.to_owned();
    }
    let mut out: String = excerpt.chars().take(EXCERPT_MAX_LEN).collect();
    out.push('…');
    out
}

#[cfg(test)]
#[path = "harness_output_monitor_tests.rs"]
mod tests;
