# Spec: Persistent weekly USD spend status bar (GH-9965)

## Problem

AI spend visibility is split across two surfaces today: a per-turn
credit chip on the latest agent response, and the
Settings → Billing & Usage dashboard (several clicks away from a
normal terminal/agent workflow). Users have no glanceable signal
for "where am I in my budget for the week?"

## Goal

Add an opt-in persistent indicator at the bottom-right of the Warp
window showing the user's running 7-day rolling USD spend on Agent
Mode. Setting controls visibility; default off (don't push spend in
users' faces unless they want it).

## Behavior contract

- B1. New setting
  `agents.show_weekly_spend_status: bool` (default `false`,
  `SyncToCloud::Globally(RespectUserSyncSetting::Yes)`).
- B2. When ON, a status-bar item shows "$XX.XX this week" in the
  same status-bar slot used by other agent indicators (verify slot
  by grepping the existing status-bar render path).
- B3. The indicator updates on every agent turn finish AND on a
  60-second timer (cheap; the spend value comes from the same
  billing model that already polls).
- B4. Hover tooltip shows: rolling-7d total, current month total,
  and a "View details →" link that opens Settings → Billing & Usage.
- B5. Click on the indicator opens Settings → Billing & Usage
  (one-click bypass of the menu drill-down).
- B6. When billing data is unavailable (e.g., user is logged out,
  or BYOK without credit accounting), the indicator is hidden —
  not stuck on $0.00 which would be misleading.
- B7. Default OFF: the issue's request is opt-in; users who don't
  want to see spend reminders aren't forced to. The setting is
  one click in Settings → Agents → Display.

## Acceptance criteria

- A1. Setting OFF: no status-bar entry, layout pixel-equivalent.
- A2. Setting ON, billing data available: shows "$XX.XX this week"
  with hover tooltip and click-through.
- A3. Setting ON, billing unavailable: status-bar slot is empty.
- A4. Number updates within ≤2 seconds of an agent turn finishing.

## Privacy / telemetry

- The dollar value lives only in the local UI. No new telemetry
  events are emitted by enabling/disabling this indicator beyond
  the existing `setting_changed` event with the boolean value.
- The indicator is read-only — it cannot send the value anywhere.

## Test plan

- T1. Setting round-trips through TOML.
- T2. With billing data fixture and setting ON, the
  rendered status-bar tree contains the spend label.
- T3. With billing data fixture and setting OFF, the slot is empty
  (snapshot test).
- T4. Click handler dispatches the
  `OpenSettingsPage(BillingAndUsage)` action.

## Out of scope (V1)

- Daily / monthly toggles. V1 is 7-day rolling only — the most
  asked-for window in the issue thread.
- Visual warning thresholds ("80% of budget"). The user's budget
  is set elsewhere; surfacing that intersection is V2.
- Real-time SSE updates instead of the 60s poll + per-turn refresh.
