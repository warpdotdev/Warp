# Alert Maintainers When There Are Duplicates Of A PR (GH-10395)

## Summary

Surface a non-blocking maintainer alert inside the Warp Code Review experience when multiple open PRs appear to target the same underlying issue or duplicate each other's diff. Provide a cross-link list of candidate duplicates, one-click navigation between them, and emit a structured signal that can be routed to Slack or Linear so maintainers are notified outside Warp.

## Problem

Maintainers regularly receive multiple PRs for the same issue — particularly during hackathons, bug-bash days, or when an issue sits open long enough for several contributors to pick it up independently. Today there is no in-product signal that a PR being reviewed is one of several candidates. Reviewers either discover the duplicate by accident (a teammate mentions it, a CI conflict surfaces) or duplicate review effort is spent on PRs that will ultimately be closed in favor of another. This wastes reviewer time and is a poor experience for the contributor whose PR gets closed late.

## Goals

- Detect candidate-duplicate PRs using lightweight signals that do not require a backend index or LLM call.
- Surface a "Possible duplicates" callout in the Warp Code Review pane with the candidate PRs, the signal that triggered the match, and one-click navigation between them.
- Emit a structured `MaintainerAlert.DuplicatePr` signal that team admins can route to a Slack webhook and/or Linear issue.
- Allow per-PR dismissal of false positives so the same noise does not return on every open.
- Provide a per-user setting and team-level signal-destination configuration; both default to enabled but with no destinations configured.

## Non-Goals

- Auto-closing duplicate PRs. The maintainer always decides.
- Server-side merge orchestration or rebase automation.
- Cross-repo duplicate detection (V1 is single-repo; tracked as Open Question).
- Language-aware semantic duplicate detection (e.g., comparing AST diffs). V1 uses path-set and title token similarity only.
- Replacing or modifying the existing GitHub branch comparison view.

## Behavior Contract

### B1. Detection signals

Any one of the following triggers candidate-duplicate state for the open PR:

1. **Same issue reference.** Another open PR in the same repo references the same issue via `Closes #N`, `Fixes #N`, or `Resolves #N` (case-insensitive) in its body.
2. **File-overlap.** For PRs touching ≥3 files, Jaccard similarity of changed-file path sets ≥ 0.50 with another open PR.
3. **Small-PR heuristic.** For PRs touching 1–2 files, all changed paths match another open PR AND title TF-IDF cosine similarity ≥ 0.70.
4. **Rapid-fire same author.** Same author submitting more than one open PR with ≥30% diff overlap within a 24-hour window. (Captures force-push-as-new-PR cases.)

### B2. Surface in Code Review pane

A "Possible duplicates" callout renders above the diff, collapsed by default with the candidate count visible (e.g., "Possible duplicates (2)"). Expanded, it lists each candidate row with:

- PR number, title, author avatar+handle.
- Signal label that triggered the match (e.g., "References same issue #1234", "62% file overlap", "Same author within 24h").
- Click target: opens that PR in a new Code Review pane (does not replace the current one).

### B3. Maintainer signal

When ≥1 candidate is detected for a PR, emit `MaintainerAlert.DuplicatePr { pr_number, candidate_pr_numbers, signal_types }`. Configured destinations are read from team-level settings; admins configure once. Supported destinations:

- Slack webhook URL.
- Linear issue creation using a configured template.

If both are configured, both fire. If neither is configured, no external signal fires (the in-product callout still renders).

### B4. Suppression

The callout exposes a "Mark as not duplicate" button per candidate. Clicking it records a dismissal as a structured PR comment with marker `<!-- warp-dup-dismiss pr=<candidate_pr_number> -->`. On subsequent opens, the dismissed candidate is filtered out for that (PR, candidate) pair only. Dismissals are bidirectional — dismissing on PR A also hides PR A from PR B's candidate list.

### B5. Detection cadence

Detection runs:

- On Code Review pane open for a PR.
- When a PR-reviewed event fires upstream and other open PRs touching overlapping paths exist.
- Hourly in the background while the review pane stays open.

Each run is bounded to the PRs returned by GitHub's open-PR list for the repo (capped at the most recent 200 open PRs).

## Settings / API surface

User-level (Settings → Code Review → "Duplicate PR alerts"):

- `code.review.duplicate_alert.enabled` (bool, default `true`) — toggles both the in-product callout and signal emission for the current user.

Team-level (admin-managed; surfaced under team settings UI):

- `team.duplicate_alert.signal.slack_webhook` (string, default empty) — Slack incoming-webhook URL.
- `team.duplicate_alert.signal.linear_issue_template` (string, default empty) — Linear issue body template; supports `{pr_number}`, `{candidates}`, `{signals}` placeholders.

If `code.review.duplicate_alert.enabled` is `false`, neither the callout nor the signal fires for that user.

## Acceptance Criteria

- A1. A PR whose body references an issue already referenced by another open PR triggers the callout with signal label "References same issue #N".
- A2. Two PRs touching ≥3 files with ≥50% Jaccard path-set overlap each show the other in their callout with signal label "X% file overlap".
- A3. A 1–2 file PR matches another only when path sets are identical AND title cosine ≥ 0.70.
- A4. A dismissed candidate stays hidden for that (PR, candidate) pair on subsequent opens, in both directions.
- A5. The `MaintainerAlert.DuplicatePr` signal fires at most once per (PR, candidate) pair per session; subsequent detections of the same pair do not re-fire.
- A6. With `code.review.duplicate_alert.enabled = false`, no callout renders and no external signal fires.
- A7. Clicking a candidate row opens that candidate PR in a new Code Review pane without replacing the current one.

## Implementation Pointers

Verified paths:

- Code Review pane: `app/src/code_review/code_review_view.rs`, `app/src/code_review/code_review_view_integration.rs`.
- Code Review header (likely callout host): `app/src/code_review/code_review_header/header_revamp.rs`, `app/src/code_review/code_review_header/mod.rs`.
- Settings (user-level): `app/src/settings/` (sibling files like `app/src/settings/accessibility.rs` show the pattern; new `app/src/settings/code_review.rs` recommended if not yet present).
- Team server API (signal destinations): `app/src/server/server_api/team.rs`.
- Telemetry hosts: existing `telemetry.rs` files under `app/src/ai/agent/telemetry.rs` etc. show the per-domain pattern; recommend `app/src/code_review/telemetry.rs` for the new event.

New modules:

- `app/src/code_review/duplicate_detector.rs` (new) — pure detection logic over an open-PR list snapshot.
- `app/src/code_review/duplicate_callout.rs` (new) — UI for the callout above the diff.
- `app/src/code_review/duplicate_signal.rs` (new) — Slack/Linear emitter.

## Tests

- T1. Same `Closes #N` body in two open PRs produces a candidate match.
- T2. File-set Jaccard ≥ 0.50 over ≥3 files produces a match; below 0.50 does not.
- T3. Small-PR (1–2 files) path requires identical path set AND title cosine ≥ 0.70.
- T4. Dismissal persists via PR-comment marker and is bidirectional.
- T5. Signal emits at most once per (PR, candidate) pair per session.
- T6. Setting OFF mutes both the callout and the signal.
- T7. Background re-detection picks up newly opened PRs without requiring pane refresh.
- T8. Click navigation opens the candidate in a new pane.

## Open Questions

- Should detection extend to other repositories within the same team when a team has multiple? Suggest V1 single-repo only; V1.5 may extend with a team-level allowlist of repos to cross-check.
- Should the diff-overlap signal compute on raw line-add/remove sets or only on file paths? V1 uses paths only to keep the detector cheap; line-overlap is a V1.5 extension.

## Telemetry

- New event: `code_review.duplicate_detected { pr_number, candidate_count, signal_types }` — fires once per detection run that produced ≥1 candidate.
- New event: `code_review.duplicate_dismissed { pr_number, candidate_pr_number }` — fires on the "Mark as not duplicate" action.
- New event: `code_review.duplicate_signal_emitted { destination }` — fires per Slack or Linear emission.
- Reuse: existing `code_review.opened` event remains the per-pane open event.
