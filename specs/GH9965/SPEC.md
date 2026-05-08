# Spec: Persistent 7-day USD spend status bar (GH-9965)

## Problem

AI spend visibility is split across two surfaces today: a per-turn
credit chip on the latest agent response, and the
Settings → Billing & Usage dashboard (several clicks away from a
normal terminal/agent workflow). Users have no glanceable signal
for "where am I in my budget over the last week?"

## Goal

Add an opt-in persistent indicator at the bottom-right of the Warp
window showing the user's running 7-day rolling USD spend on Agent
Mode. Setting controls visibility; default off (don't push spend in
users' faces unless they want it).

User-facing label: **"7-day spend"** (rolling 7-day window). All UI
strings in the spec use "7-day" rather than "weekly" to make the
window unambiguous and avoid implying a calendar week boundary.

## Billing scope

The indicator reflects the active billing context for the current
window/workspace.

- For users with a single billing context (personal only, or a single
  team/workspace), the indicator shows that context's spend with no
  scope label necessary in the chip itself.
- For users with multiple contexts (e.g., personal + one or more
  team/workspace billing accounts), the indicator follows the
  workspace selector — the same selector that drives which workspace
  the agent runs against. Switching the workspace selector switches
  the indicator's scope on the next refresh tick.
- The hover tooltip always names the active scope explicitly
  (e.g., "Personal" or "Acme Workspace") so users with multiple
  contexts can confirm what the dollar figure represents.
- When no billing context is resolvable (signed out, workspace
  switching mid-flight, or BYOK without credit accounting), the
  indicator hides entirely rather than displaying a zero or
  ambiguous value.

## Behavior contract

- B1. New setting
  `agents.show_weekly_spend_status: bool` (default `false`,
  `SyncToCloud::Globally(RespectUserSyncSetting::Yes)`). The setting
  key keeps the existing `weekly` token for backwards-compat with
  any in-progress drafts; user-facing copy uses "7-day spend".
- B2. When ON, a status-bar item shows "$XX.XX · 7-day spend" in the
  right-hand status-bar group, immediately to the left of the
  context-remaining chip. See "Status-bar placement" below for the
  precise slot, ordering, and width-constrained collapse rules.
- B3. The indicator updates on every agent turn finish AND on a
  60-second refresh timer. Failure handling uses a separate
  exponential-backoff retry timer; the 60 s cadence governs only the
  success path. See "Refresh vs failure-retry timers" below.
- B4. Hover tooltip shows: active scope name, rolling 7-day total
  (USD with 2 decimal places), current month total, the caption
  "rolling 7 days", and a "View details →" link that opens
  Settings → Billing & Usage.
- B5. Click on the indicator opens Settings → Billing & Usage
  (one-click bypass of the menu drill-down).
- B6. When billing data is unavailable, the indicator is hidden —
  not stuck on $0.00 which would be misleading. The hide thresholds
  are split: scope-unresolved hides immediately; ≥30 s without a
  successful fetch surfaces a "stale" tooltip annotation but keeps
  the chip visible; ≥5 minutes of continuous failure hides the chip
  entirely. See "Refresh vs failure-retry timers" below.
- B7. Default OFF: the issue's request is opt-in; users who don't
  want to see spend reminders aren't forced to. The setting is
  one click in Settings → Agents → Display.

## Data source

- **Source of truth.** The indicator reads from the same
  Billing & Usage backend response that already powers the
  Settings → Billing & Usage pane. **No new endpoint** is added for
  V1, but V1 depends on an **additive** field on the existing
  response.
- **Required field: `usage_rolling_7d_usd`.** The indicator requires a
  `usage_rolling_7d_usd: f64` (or equivalent fixed-point USD) field on
  the existing `BillingUsage` response. This is the only field that
  delivers the rolling 7-day window directly; today's response carries
  the current-month total and per-day buckets but does not expose a
  precomputed rolling 7-day USD figure with the freshness guarantees
  this indicator needs (≤2s post-turn).
  - Computing 7-day rolling on the client from per-day buckets is
    rejected as the V1 plan because (a) per-day buckets settle
    asynchronously after a turn and would not satisfy A4, and (b) it
    pushes monetary aggregation into the client where rounding rules
    must match the server-side dashboard exactly.
  - Backend additive change: extend the existing `BillingUsage`
    response with `usage_rolling_7d_usd`. Same endpoint, same auth,
    same scope resolution; one additional field. **This is not a new
    endpoint** and is the explicit "no new endpoint" interpretation
    for this spec.
  - Until the backend confirms the additive field can ship in the V1
    window, this dependency is tracked in **Open questions** below
    and blocks V1.
- **Refresh cadence.** The client cache is invalidated on every
  agent-turn-completion event and on the 60-second timer. The post-
  turn refresh is required to land in the indicator within ≤2s of
  the turn-completion event (B3 / A4). Refresh and failure-retry are
  separate timers (see "Refresh vs failure-retry timers" below); the
  60-second timer governs only the success path.
- **Cache TTL.** Cached value is considered fresh for ≤2s following
  a turn completion. Outside that window the cache follows the 60s
  timer.
- **Hide-on-unresolved-scope.** When the active billing scope is
  unresolved (signed out, workspace switching, BYOK without credit
  accounting), the indicator hides immediately — no stale value is
  shown across scope changes.

### Refresh vs failure-retry timers

The 60-second refresh cadence and the failure-handling behavior are
governed by **two independent timers**. Treating them as one (as the
round-1 draft did) caused a contradiction: a 30-second hide-on-failure
rule cannot coexist with a 60-second refresh cadence — the indicator
would hide before its next legitimate refresh attempt.

- **Refresh timer (success path).** Period 60 s, plus an immediate
  refresh on every `agent_turn_completed` event. Each tick fetches
  from the existing Billing & Usage subscription. On success, the
  cached value updates, the failure counter resets to zero, and the
  failure-retry timer is cancelled.
- **Failure-retry timer.** On a refresh failure, the refresh timer
  pauses and the failure-retry timer takes over with exponential
  backoff: **5 s, 10 s, 20 s, 40 s, 80 s (cap)**. Each retry
  attempts the same fetch. On success, the failure-retry timer is
  cancelled, the refresh timer resumes its 60 s cadence, and the
  failure counter is cleared.
- **Stale tooltip threshold (≥30 s since last success).** While the
  failure-retry timer is active and ≥30 s have elapsed since the
  most recent successful fetch, the tooltip adds a "stale"
  annotation: "Updated Xs ago — couldn't refresh." The chip itself
  does not change appearance; only the tooltip surfaces the
  staleness so the status-bar layout does not shift. **The chip
  remains visible at this stage** — it shows the last-known value
  with the stale annotation in the tooltip.
- **Hide-on-prolonged-failure (≥5 min consecutive failures).** If the
  failure-retry timer has been active continuously for 5 minutes
  without a successful fetch, the indicator hides entirely. It
  re-appears on the first successful fetch, at which point the
  refresh timer resumes its 60 s cadence.
- **Reconciliation note.** Round-1 specified "hide after 30 seconds
  of consecutive failures." Round 2 separates the two thresholds:
  ≥30 s → tooltip "stale" annotation, chip remains visible; ≥5 min
  → hide. The 30 s value is preserved as the *staleness marker*,
  not the *hide threshold*, which removes the conflict with the 60 s
  refresh cadence.

## Acceptance criteria

- A1. Setting OFF: no status-bar entry, layout pixel-equivalent.
- A2. Setting ON, billing data available: shows
  "$XX.XX · 7-day spend" with hover tooltip and click-through.
- A3a. Setting ON, billing unresolved (signed out, workspace
  switching mid-flight, BYOK without credit accounting): status-bar
  slot is empty immediately.
- A3b. Setting ON, ≥30 s since most recent successful fetch (transient
  failures continuing): chip remains visible with last-known value;
  tooltip shows "Updated Xs ago — couldn't refresh".
- A3c. Setting ON, ≥5 minutes of continuous failed fetches: status-
  bar slot is empty; reappears on first successful fetch.
- A4. Number updates within ≤2 seconds of an agent turn finishing.
- A5. Status-bar placement: the segment is positioned in the right-
  hand status-bar group, in the order specified in "Status-bar
  placement" below. Window-resize tests confirm the documented
  collapse priority.

## Status-bar placement

The 7-day-spend segment lives in the **right-hand status-bar slot
group** — the same group that already hosts the context-remaining
indicator (the "96% context" chip) and other right-aligned chips. It
is NOT a new slot group and does not introduce a new region in the
status-bar layout.

### Slot ordering (left → right within the right-hand group)

```
[ 7-day spend ] [ context % ] [ debug / dev indicators ] [ menu chips ]
```

The 7-day-spend chip sits immediately to the **left** of the
context-remaining chip. Rationale: spend is the lower-frequency, more
human-readable indicator and reads first when the eye scans the
right-hand cluster; the context indicator updates per token and is
treated as a more transient, machine-state signal that anchors the
group.

### Collision and collapse priority

When window width is constrained, status-bar segments collapse in
priority order. Lower-priority segments collapse to icon-only first,
then hide; higher-priority segments stay full until they cannot fit.
The 7-day-spend chip is treated as **medium priority** in the
existing scheme:

1. **Highest priority (never collapse first).** Context-remaining,
   active-error chip.
2. **Medium priority — collapses to "$XX.XX" without the
   "· 7-day spend" caption when width is tight.** 7-day spend.
3. **Medium priority — hides under further width constraint.**
   7-day spend (after the icon-only collapse).
4. **Lowest priority (collapse first).** Debug / dev / experimental
   chips.

Concrete behavior at three breakpoints:

| Available right-cluster width | 7-day-spend rendering |
|-------------------------------|------------------------|
| Wide (default)                | "$XX.XX · 7-day spend" with full caption |
| Narrow                        | "$XX.XX" only, caption dropped |
| Very narrow (sub-threshold)   | Hidden; reappears when width returns |

The chip never wraps onto a second line; the status bar is always a
single row. Width thresholds are inherited from the existing
status-bar collapse logic — no new threshold values are introduced.

## Privacy / telemetry

- The dollar value lives only in the local UI. No new telemetry
  events are emitted by enabling/disabling this indicator beyond
  the existing `setting_changed` event with the boolean value.
- The indicator is read-only — it cannot send the value anywhere.

## Test plan

- T1. Setting round-trips through TOML.
- T2. With billing data fixture and setting ON, the
  rendered status-bar tree contains the spend label
  "$XX.XX · 7-day spend".
- T3. With billing data fixture and setting OFF, the slot is empty
  (snapshot test).
- T4. Click handler dispatches the
  `OpenSettingsPage(BillingAndUsage)` action.
- T5a. Hidden-on-unresolved-scope: indicator disappears immediately
  when the scope is unresolved (signed-out fixture; workspace
  switching mid-flight; BYOK without credit accounting).
- T5b. Stale-tooltip-after-30s: with a fault-injecting fake client
  failing every fetch after one initial success, the chip remains
  visible at t+35 s with the tooltip carrying the "stale"
  annotation. (Asserts the 30 s threshold marks staleness, not
  hiding.)
- T5c. Hidden-after-5min: with the same fault-injecting client, the
  chip is hidden by t+5 min 5 s, and reappears within ≤2 s of the
  next successful fetch.
- T5d. Backoff schedule: assert retry attempts occur at +5 s, +10 s,
  +20 s, +40 s, +80 s, +80 s, ... after the first failure (cap at
  80 s) and that the schedule resets to the 60 s success cadence on
  recovery.
- T6. Refresh timing: a simulated agent-turn-completion event
  causes the indicator to reflect the new value within 2 seconds.
- T7. Tooltip totals: tooltip renders the active scope name,
  exact USD with 2 decimal places, and the "rolling 7 days"
  caption.
- T8. Stale-on-error single-failure: a single failed fetch following
  a successful one keeps the chip's value visible (within the 30 s
  staleness window the tooltip does not yet add the "stale"
  annotation; the annotation appears only past 30 s).
- T9. Multi-scope: switching the workspace selector causes the
  next refresh tick to show that scope's value and label, not the
  prior scope's stale value.
- T10. Status-bar placement: snapshot the right-hand status-bar
  group with the chip ON; assert the 7-day-spend chip is rendered
  immediately to the left of the context-remaining chip.
- T11. Width-constrained collapse: at the "narrow" breakpoint the
  chip renders as "$XX.XX" without the "· 7-day spend" caption; at
  the "very narrow" breakpoint the chip is hidden; at "wide" the
  full label is rendered.
- T12. Field dependency: a fixture response missing
  `usage_rolling_7d_usd` causes the chip to hide (treated as
  unavailable) and surfaces a single warn-level log entry — does
  not crash, does not synthesize a client-side value.

## Open questions

- **OQ1. Backend `usage_rolling_7d_usd` field.** V1 depends on the
  Billing & Usage backend exposing a `usage_rolling_7d_usd` field
  on the existing response (additive, no new endpoint). Until
  backend confirms this can ship in the V1 window, V1 cannot ship.
  Tracked as a hard dependency. If backend pushes back, fall-back
  options (in order of preference):
  1. Server-side rolling calculation gated behind a feature flag
     so the client can begin opt-in adoption while the backend
     stabilises.
  2. Client-side rolling computation from per-day buckets, accepting
     the relaxed A4 (drop "≤2 s after turn completion" to
     "≤2 s after the next per-day bucket settles") — explicitly
     called out as a regression from V1 goals.

## Out of scope (V1)

- Daily / monthly toggles. V1 is 7-day rolling only — the most
  asked-for window in the issue thread.
- Visual warning thresholds ("80% of budget"). The user's budget
  is set elsewhere; surfacing that intersection is V2.
- Real-time SSE updates instead of the 60s poll + per-turn refresh.
- A new dedicated billing endpoint optimised for the indicator.
  V1 reuses the existing Billing & Usage endpoint.
