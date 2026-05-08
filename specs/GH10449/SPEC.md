# Spec: AI credit usage in the bottom status line (GH-10449)

## Summary

Add an opt-in status-bar segment that shows the user's AI credit
consumption for the current billing period (e.g., `89/2500`),
mirroring the existing "context remaining" segment so users can
monitor remaining credits at a glance without opening Settings.

## Problem

Today, users can only see how many AI credits they have used by
opening Settings → Billing & Usage. For users on metered plans
who run agent turns frequently, this round-trip is slow and
discourages cost awareness mid-session. Issue #10449 asks for a
small, always-visible segment in the bottom status line that
shows `<used>/<allowance>`, directly analogous to the existing
"96% context remaining" segment.

## Goals

- Provide an opt-in status-bar segment showing
  `<used>/<allowance>` for the current billing period.
- Click on the segment navigates to Settings → Billing & Usage.
- Segment updates within ≤2s of an agent-turn completion event.
- Visual state escalates by usage threshold (neutral → yellow →
  red) so high-spend users notice without opening Settings.
- Hidden by default; user opts in from Appearance → Status Bar.

## Non-Goals

- Not a real-time billing telemetry feed.
- Not a notification system (no toasts on threshold crossings).
- Not a quota-block enforcer; the segment never gates command
  execution.
- Not a replacement for the Billing & Usage pane; it summarizes,
  it does not duplicate the full breakdown.
- Not a per-agent or per-task cost breakdown; it only shows the
  scope-level totals already returned by the billing API.

## Behavior Contract

### B1. Display format

- Format: `<used>/<allowance>` rendered as plain integers, e.g.
  `89/2500`. No thousands separators in V1 to keep the segment
  compact at narrow window widths.
- Threshold colors:
  - `<80%` used → default status-bar foreground.
  - `≥80%` and `<95%` used → yellow / warning foreground (uses
    the existing status-bar warning token).
  - `≥95%` used → red / danger foreground (uses the existing
    status-bar danger token).
- "Unlimited" plan: when the billing source returns
  `plan == "unlimited"` (or equivalent), display `∞/∞` and use
  default foreground regardless of `used`.
- Indeterminate state (used or allowance is null/missing while
  scope is otherwise valid): display `—/—` and tooltip explains
  "Usage data unavailable for this scope."

### B2. Refresh & caching

- Source of truth: the same billing client / endpoint that powers
  Settings → Billing & Usage. The status-bar segment MUST NOT
  introduce a second fetch pipeline.
- Cache TTL ≤2s after the most recent agent-turn completion event
  for the active conversation. The segment subscribes to the same
  agent-turn completion signal already used by the conversation
  list.
- Background refresh: when the segment is visible, refresh on a
  60s interval as a floor, plus on every agent-turn completion
  (debounced to ≤2s).
- On fetch error: retain the last successfully-fetched value, and
  show a stale indicator in the tooltip ("Last updated 12s ago,
  retrying").
- After 30s of consecutive fetch failures, hide the segment
  entirely. Resume display once a fetch succeeds.

### B3. Scope (workspace selector)

- The segment follows the current workspace-billing-context
  selector (the same selector that drives the Billing pane).
- When the user switches workspace scope (e.g. "Personal" → "Acme
  Workspace"), the segment refetches and re-renders within the
  ≤2s debounce window.
- Tooltip shows scope name + allowance period, e.g.:
  `Acme Workspace · Monthly allowance · resets Dec 1`.

### B4. Click action

- Single click on the segment: opens Settings → Billing & Usage,
  scoped to the workspace shown in the segment.
- Right-click: opens a small context menu with "Hide credits in
  status bar" (toggles `status_bar.show_ai_credits` to `false`)
  and "Open Billing & Usage".
- Keyboard: focusable via the existing status-bar tab order;
  Enter activates the click action.

### B5. Visibility & opt-in

- Default state: segment is hidden
  (`status_bar.show_ai_credits = false`).
- Opt-in surface: Settings → Appearance → Status Bar, alongside
  any existing status-bar visibility toggles.
- When the user has no billing context at all (e.g., signed-out
  state), the segment stays hidden even if the toggle is on, and
  the toggle subtitle reads "Sign in to see credit usage."

## Settings / API surface

- New setting:
  - Key: `status_bar.show_ai_credits`
  - Type: `bool`
  - Default: `false` (opt-in)
  - Sync: `SyncToCloud::Globally(RespectUserSyncSetting::Yes)`,
    consistent with other Appearance settings.
  - TOML namespace: `[status_bar]` table,
    `show_ai_credits = false`.
- No new billing API endpoint. Reuses the existing usage fetch
  consumed by the Billing & Usage pane.
- No new color tokens. Reuses existing status-bar `warning` and
  `danger` foreground tokens.

## Acceptance Criteria

- A1. With `status_bar.show_ai_credits = true` and a metered plan,
  the segment renders `<used>/<allowance>` matching the values
  shown in Settings → Billing & Usage.
- A2. Threshold colors match B1: <80% default, 80–94% yellow,
  ≥95% red. Verified at the boundary values.
- A3. Clicking the segment opens Settings → Billing & Usage.
- A4. Switching workspace billing scope updates the segment to
  the new scope's used/allowance within 2s.
- A5. Simulated 30s of consecutive fetch failures hides the
  segment; a subsequent successful fetch restores it.
- A6. With `plan == "unlimited"`, the segment renders `∞/∞` and
  ignores threshold colors.
- A7. With `status_bar.show_ai_credits = false`, the segment is
  not rendered and incurs no extra fetch.
- A8. Right-click → "Hide credits in status bar" flips the
  setting to `false` and the segment disappears immediately.

## Implementation Pointers

- `app/src/status_bar/*.rs` — add a new segment struct/component
  alongside the existing context-remaining segment. Place after
  the context-remaining segment to preserve familiar ordering.
- `app/src/billing/*.rs` — extend the existing billing client to
  expose a thin observable `current_usage()` that the status-bar
  segment subscribes to. Do NOT introduce a parallel fetch loop.
- `app/src/settings/appearance/status_bar.rs` — add the
  `show_ai_credits` toggle row, with a subtitle that explains the
  signed-out / no-billing-context fallback.
- Reuse the existing agent-turn-completion event channel to drive
  the ≤2s refresh debounce.

## Tests

- T1. Segment formatting: numeric values, threshold colors at
  79/80/94/95/100 percent, unlimited rendering, indeterminate
  rendering.
- T2. Fetch debounce: rapid agent-turn completions coalesce into
  a single fetch within 2s.
- T3. Stale-on-error: fetch failure retains last value and
  surfaces stale indicator in tooltip.
- T4. Hidden-after-failure: 30s of consecutive failures hides the
  segment; one success restores it.
- T5. Click routing: click navigates to Settings → Billing &
  Usage scoped to the current workspace.
- T6. Workspace-scope switch updates the segment within 2s and
  the tooltip shows the new scope name.
- T7. Toggle off (`show_ai_credits = false`) prevents rendering
  AND prevents subscription to billing fetches (no extra network
  cost when opted out).
- T8. Right-click context menu hides the segment and persists the
  setting flip.

## Open Questions

- Q1. For unlimited plans, should we instead show daily-spend in
  USD as a more useful number? V1 proposal: no — show `∞/∞` and
  punt to the Billing pane for spend breakdown. Revisit once the
  USD-spend spec (sibling PR #10224) ships.
- Q2. Should the segment be visible by default for new users on
  metered plans, since they are the audience that benefits most?
  V1 proposal: keep opt-in to avoid surprising existing users
  with new chrome; promote via a one-time tooltip on the Billing
  pane.
- Q3. Should we localize the `∞` glyph or render the literal word
  "Unlimited"? V1 proposal: glyph, with the word in the tooltip.

## Telemetry

- No new telemetry events. The billing fetch reuses the existing
  Billing-pane fetch path and its existing instrumentation.
- One existing event SHOULD gain a `surface` field if it does not
  already have one, so we can attribute fetches to
  `status_bar` vs `billing_pane`. This is a no-op if the field
  already exists.
