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
4. **Rapid-fire same author.** Same author submitting more than one open PR within a 24-hour window where the changed-file path sets have Jaccard similarity ≥ 0.30 (path-only overlap, identical to the metric used in B1.2). V1 explicitly does **not** compute line-level diff overlap; the term `diff overlap` is defined here as `|paths(A) ∩ paths(B)| / |paths(A) ∪ paths(B)|` over the changed-file path sets only. (Captures force-push-as-new-PR cases.) Line-level overlap is deferred to V1.5 (see Open Questions).

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

#### B4a. Dismissal trust model

The `<!-- warp-dup-dismiss pr=N -->` marker is **only respected when the comment author has actual `push` (write) permission or higher on the repository at the time the marker is evaluated**. This prevents external contributors — and lower-privileged org members — from forging suppression markers to hide duplicate alerts from maintainers.

**Authoritative permission check (required).** Trust is determined by an explicit per-author repository permission lookup, **not** by `author_association` alone. `author_association` only describes the author's relationship to the PR's discussion thread (e.g., `MEMBER` = member of the org that owns the repo, which does **not** imply repository write access). The detector calls:

```
GET /repos/{owner}/{repo}/collaborators/{username}/permission
```

and treats the marker as trusted **only if** the response's `permission` field is one of `{ admin, maintain, write }`. The values `triage`, `read`, and `none` are rejected.

The result of this lookup is cached per `(repo, username)` for 10 minutes to bound API cost; cache misses re-fetch on the next detection run.

**`author_association` is used only as a fast-path negative filter** — comments with `author_association ∈ { CONTRIBUTOR, FIRST_TIME_CONTRIBUTOR, FIRST_TIMER, NONE, MANNEQUIN }` are rejected without making the permission API call, since these associations cannot correspond to write access. Comments with `author_association ∈ { OWNER, MEMBER, COLLABORATOR }` proceed to the authoritative `permission` lookup before being trusted.

Comments whose author fails the permission check carrying the same marker text are **IGNORED** — the candidate continues to surface in the callout as if the marker were absent. The detector logs a sanitized debug entry (no comment body; only `pr_number`, `author_association`, and resolved `permission`) when a non-trusted marker is encountered.

The "Mark as not duplicate" UI button is only enabled for users whose GitHub identity passes the same `permission ∈ { admin, maintain, write }` check against the repo; users without write permission see the button disabled with tooltip "Requires repo write access".

### B-Secret. Slack webhook secret-material handling

The Slack incoming-webhook URL is treated as **secret material** and follows team-secret-grade controls.

**Storage.**
- The webhook URL is stored in the team-level encrypted secret store, never in plain TOML, never in user-readable config files, never inline in this repo's settings tree.
- If Warp already has a `team_secrets`-style schema, the webhook is persisted there. If no such schema exists yet, this spec marks it as `(new) team-level encrypted secret store` infrastructure that must be in place before the feature ships.
- Encrypted at rest using the team's existing key material (envelope encryption against the team key); never written to disk in cleartext.

**Access control.**
- Only users with **team admin** role can READ or WRITE the webhook value.
- Non-admin team members see a redacted display only: `"•••••• (configured)"` if a webhook is set, `"(not configured)"` otherwise. The actual URL is never returned by the API to non-admin clients.
- Admin reads of the webhook (e.g., to copy it for rotation) are themselves an audited event in telemetry: `team.duplicate_alert.webhook_revealed { admin_user_id }`.

**Transmission.**
- The webhook is invoked over **TLS only**. Plain HTTP webhook URLs are rejected at save time with a validation error.
- The webhook URL is **never** included in logs, telemetry payloads, error messages, panic traces, or crash reports — emit error metadata stripped of URL components. A dedicated sanitizer wraps every log/error path that could touch the URL: it replaces the URL with `[redacted-slack-webhook]`.

**Rotation.**
- Admins may replace the webhook value at any time via team settings. The previous value is overwritten in place — there is no history kept and no way for the API to return prior values.
- Rotation takes effect immediately for the next signal emission; in-flight emissions complete against whichever value they captured at request time.
- Setting the value to empty string disables external Slack signaling entirely (the in-product callout still renders).

**Linear template handling.**
- The Linear issue template is **not** treated as secret material (it is a body template with placeholders, not a credential). However, it is admin-write / team-read so non-admins cannot inject content into outgoing Linear issues.

### B5. Detection cadence

Detection runs:

- On Code Review pane open for a PR.
- When a PR-reviewed event fires upstream and other open PRs touching overlapping paths exist.
- Hourly in the background while the review pane stays open.

Each run is bounded to the PRs returned by GitHub's open-PR list for the repo (capped at the most recent 200 open PRs).

## Settings / API surface

User-level (Settings → Code Review → "Duplicate PR alerts"):

- `code.review.duplicate_alert.enabled` (bool, default `true`) — when `false`, suppresses the in-product "Possible duplicates" callout in the Code Review pane for **this user only**. This setting has **no effect** on team-level external signal emission (Slack/Linear); see the independence rules below for the authoritative scope of each layer.

Team-level (admin-managed; surfaced under team settings UI):

- `team.duplicate_alert.signal.slack_webhook` (string, default empty) — Slack incoming-webhook URL.
- `team.duplicate_alert.signal.linear_issue_template` (string, default empty) — Linear issue body template; supports `{pr_number}`, `{candidates}`, `{signals}` placeholders.

### Scope of toggles — user vs team

The user-level toggle and team-level external-routing config govern **independent** layers. They do not chain. The table below is the authoritative contract; any other prose in this spec that appears to contradict it is superseded by this section.

| Setting | Scope | What it controls | What it does NOT control | Default |
|---|---|---|---|---|
| `code.review.duplicate_alert.enabled` | Per-user | Whether **this user** sees the in-product "Possible duplicates" callout in the Code Review pane. | Team-level external signal emission (Slack/Linear). External signals are governed solely by team-level destination config and the dedupe rules in B-Dedup. | `true` |
| `team.duplicate_alert.signal.slack_webhook` | Per-team (admin) | Slack destination for the team-level external alert. | Per-user in-product callout rendering. | empty (off) |
| `team.duplicate_alert.signal.linear_issue_template` | Per-team (admin) | Linear issue template for the team-level external alert. | Per-user in-product callout rendering. | empty (off) |

**Independence rules.**

- A user with `enabled = false` sees no callout in their UI. This affects that user only and has no effect on signal emission.
- The team-level external signal (Slack/Linear) fires **once per dedupe-tuple** (see B-Dedup) regardless of any individual user's toggle. Maintainers receive the routed signal even if individual reviewers have callouts disabled.
- Disabling the user-level toggle does **not** suppress team-level routing. Disabling team-level routing (admin clears the destinations) does not suppress per-user callouts.

### B-Dedup. Persistent dedupe / cooldown for external signals

External signals (Slack/Linear) are deduped by the **unordered pair** `({pr_a, pr_b}, signal_type)` where `signal_type ∈ { same_issue_ref, file_overlap, small_pr_match, rapid_fire_same_author }`. The pair is canonicalized by sorting the two PR numbers ascending before keying the dedupe store, so opening either PR in the pair produces the same key. This matches the spec's promise that the cooldown applies when **either** PR in the pair is opened — there is no separate `(A→B)` vs `(B→A)` cooldown.

- Dedupe state is persisted in a **server-side store** (e.g., the team-settings DB or a per-team Redis namespace), not in process memory and not in client-side storage. This survives client app restarts, different reviewers opening the same PR, and Warp version upgrades.
- TTL: **7 days**. Within the TTL window, the same tuple does **not** re-fire — even across review sessions, app restarts, or different reviewers opening either PR in the pair.
- When the TTL expires, the next detection event re-fires the external signal **once**, and the TTL clock resets. This handles long-lived PRs where the duplicate situation persists and needs another nudge.
- The in-product callout is **not** subject to dedupe — it always renders the current candidate set on every pane open. Dedupe only governs the team-level external signal.
- Dismissal (B4) is a stronger suppression than dedupe: a dismissed `(source_pr, candidate_pr)` pair never re-fires the external signal regardless of TTL, until both sides delete their dismissal markers.

## Acceptance Criteria

- A1. A PR whose body references an issue already referenced by another open PR triggers the callout with signal label "References same issue #N".
- A2. Two PRs touching ≥3 files with ≥50% Jaccard path-set overlap each show the other in their callout with signal label "X% file overlap".
- A3. A 1–2 file PR matches another only when path sets are identical AND title cosine ≥ 0.70.
- A4. A dismissed candidate stays hidden for that (PR, candidate) pair on subsequent opens, in both directions.
- A5. The `MaintainerAlert.DuplicatePr` external signal is deduped by the canonical unordered pair `({pr_a, pr_b}, signal_type)` with a 7-day TTL on a **server-side persistent** store; it does not re-fire within TTL across sessions, app restarts, or different reviewers, and opening either PR in the pair hits the same dedupe key.
- A5a. After the 7-day TTL elapses, the next detection of the same canonical pair re-fires the external signal exactly once and resets the TTL.
- A6. With `code.review.duplicate_alert.enabled = false`, the **in-product callout** does not render for that user. Team-level external signals continue to fire (deduped per A5) regardless of any individual user's toggle.
- A7. Clicking a candidate row opens that candidate PR in a new Code Review pane without replacing the current one.
- A8. A `<!-- warp-dup-dismiss pr=N -->` marker on a comment whose author returns `permission ∈ { admin, maintain, write }` from `GET /repos/{owner}/{repo}/collaborators/{username}/permission` is honored; the candidate is suppressed.
- A9. The same marker text on a comment whose author returns `permission ∈ { triage, read, none }` (or whose `author_association` is in the negative-filter set `{CONTRIBUTOR, FIRST_TIME_CONTRIBUTOR, FIRST_TIMER, NONE, MANNEQUIN}`) is **ignored** and the candidate continues to surface — even if `author_association` is `MEMBER`.
- A10. The Slack webhook URL is never returned by any API to a non-admin caller, never appears in logs/telemetry/error messages, and is rejected at save time if the URL scheme is not HTTPS.

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
- T5. External signal does not re-fire for the same canonical unordered pair `({pr_a, pr_b}, signal_type)` within the 7-day TTL — and opening PR B after PR A within the window also does **not** re-fire (verifies bidirectional keying).
- T_dedup_ttl. After exactly 7 days have elapsed since the previous emission, the next detection of the same canonical pair re-fires once and resets the TTL.
- T_dedup_across_sessions. Dedupe state survives client restart, multiple reviewers opening either PR, and is read from the server-side store (not from in-process memory).
- T6. User toggle OFF mutes the per-user in-product callout but does **not** suppress team-level external signals.
- T7. Background re-detection picks up newly opened PRs without requiring pane refresh.
- T8. Click navigation opens the candidate in a new pane.
- T_dismissal_trust_write_respected. A `warp-dup-dismiss` marker on a comment whose author returns `permission ∈ { admin, maintain, write }` from the collaborators-permission API suppresses the candidate.
- T_dismissal_trust_member_without_write_ignored. A marker on a comment whose `author_association = MEMBER` but whose collaborators-permission lookup returns `read` (e.g., an org member who is not a repo collaborator) is **ignored** and the candidate continues to surface — verifies that `author_association` alone is not trusted.
- T_dismissal_trust_external_ignored. The same marker text on a comment from a CONTRIBUTOR/FIRST_TIME_CONTRIBUTOR/NONE author is rejected by the fast-path `author_association` filter without an API call; the candidate still surfaces.
- T_webhook_secret_redacted. Logs, telemetry payloads, error messages, and panic traces produced during a Slack emission do not contain the webhook URL — only the literal `[redacted-slack-webhook]`.
- T_webhook_admin_only. A non-admin team member's read of the webhook value returns the redacted display only; the API never returns the cleartext URL to non-admins.
- T_webhook_https_only. Saving a webhook with `http://` scheme is rejected at validation time.

## Open Questions

- Should detection extend to other repositories within the same team when a team has multiple? Suggest V1 single-repo only; V1.5 may extend with a team-level allowlist of repos to cross-check.
- Should the diff-overlap signal compute on raw line-add/remove sets or only on file paths? V1 uses paths only to keep the detector cheap; line-overlap is a V1.5 extension.

## Telemetry

- New event: `code_review.duplicate_detected { pr_number, candidate_count, signal_types }` — fires once per detection run that produced ≥1 candidate.
- New event: `code_review.duplicate_dismissed { pr_number, candidate_pr_number }` — fires on the "Mark as not duplicate" action.
- New event: `code_review.duplicate_signal_emitted { destination }` — fires per Slack or Linear emission.
- Reuse: existing `code_review.opened` event remains the per-pane open event.
