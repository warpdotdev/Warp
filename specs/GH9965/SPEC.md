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
  same status-bar slot used by other agent indicators (verify slot
  by grepping the existing status-bar render path).
- B3. The indicator updates on every agent turn finish AND on a
  60-second timer (cheap; the spend value comes from the same
  billing model that already polls).
- B4. Hover tooltip shows: active scope name, rolling 7-day total
  (USD with 2 decimal places), current month total, the caption
  "rolling 7 days", and a "View details →" link that opens
  Settings → Billing & Usage.
- B5. Click on the indicator opens Settings → Billing & Usage
  (one-click bypass of the menu drill-down).
- B6. When billing data is unavailable, the indicator is hidden —
  not stuck on $0.00 which would be misleading. See "Data source"
  below for the specific unavailability conditions.
- B7. Default OFF: the issue's request is opt-in; users who don't
  want to see spend reminders aren't forced to. The setting is
  one click in Settings → Agents → Display.

## Data source

- **Source of truth.** The indicator reads from the same
  Billing & Usage backend endpoint that already powers the
  Settings → Billing & Usage pane. No new endpoint is added for V1;
  the indicator subscribes to the existing billing model in the
  client.
- **Refresh cadence.** The client cache is invalidated on every
  agent-turn-completion event and on the 60-second timer. The post-
  turn refresh is required to land in the indicator within ≤2s of
  the turn-completion event (B3 / A4).
- **Cache TTL.** Cached value is considered fresh for ≤2s following
  a turn completion. Outside that window the cache follows the 60s
  timer.
- **Stale-on-error.** If the most recent fetch fails (network blip,
  5xx), the indicator retains the last-known value with a "stale"
  annotation in the tooltip ("Updated Xs ago — couldn't refresh").
  The chip itself does not change appearance; only the tooltip
  surfaces the staleness so the layout doesn't shift.
- **Hide-on-prolonged-failure.** After 30 seconds of consecutive
  fetch failures, the indicator hides entirely. It re-appears on the
  first successful fetch.
- **Hide-on-unresolved-scope.** When the active billing scope is
  unresolved (signed out, workspace switching, BYOK without credit
  accounting), the indicator hides immediately — no stale value is
  shown across scope changes.

## Acceptance criteria

- A1. Setting OFF: no status-bar entry, layout pixel-equivalent.
- A2. Setting ON, billing data available: shows
  "$XX.XX · 7-day spend" with hover tooltip and click-through.
- A3. Setting ON, billing unavailable (unresolved scope or 30s of
  consecutive failures): status-bar slot is empty.
- A4. Number updates within ≤2 seconds of an agent turn finishing.

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
- T5. Hidden-unavailable: indicator disappears when scope is
  unresolved (e.g., signed-out fixture) or after 30 seconds of
  consecutive fetch failures (driven by a fault-injecting fake
  client).
- T6. Refresh timing: a simulated agent-turn-completion event
  causes the indicator to reflect the new value within 2 seconds.
- T7. Tooltip totals: tooltip renders the active scope name,
  exact USD with 2 decimal places, and the "rolling 7 days"
  caption.
- T8. Stale-on-error: a single failed fetch following a successful
  one keeps the chip's value visible and adds the "stale"
  annotation to the tooltip.
- T9. Multi-scope: switching the workspace selector causes the
  next refresh tick to show that scope's value and label, not the
  prior scope's stale value.

## Out of scope (V1)

- Daily / monthly toggles. V1 is 7-day rolling only — the most
  asked-for window in the issue thread.
- Visual warning thresholds ("80% of budget"). The user's budget
  is set elsewhere; surfacing that intersection is V2.
- Real-time SSE updates instead of the 60s poll + per-turn refresh.
- A new dedicated billing endpoint optimised for the indicator.
  V1 reuses the existing Billing & Usage endpoint.
