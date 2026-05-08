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
- No segment-side telemetry beyond click events. The segment
  MUST NOT emit usage totals, scope names, workspace identifiers,
  plan tier, or allowance reset times to telemetry. All
  usage-shape telemetry remains owned by the Billing & Usage pane.

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
  `plan == "unlimited"` (or equivalent), display `∞` glyph and
  use default foreground regardless of `used`.
- Indeterminate state (used or allowance is null/missing while
  scope is otherwise valid): display `—/—` and tooltip explains
  "Usage data unavailable for this scope."

### B1a. Display state matrix

The Billing & Usage state model exposes a small set of plan and
account states. Each maps to ONE concrete status-bar render. The
segment never invents new visual variants — every state below
reuses already-defined status-bar tokens.

| Billing & Usage state | Segment glyph / text | Color tokens | Tooltip | Click action |
|---|---|---|---|---|
| Unlimited plan | `∞` glyph (or "Unlimited" if locale lacks the glyph) | Default fg; thresholds NOT applied | "Unlimited plan · current monthly billing usage: $X" if a USD figure is exposed by the existing billing source, else "Unlimited plan" | Open Settings → Billing & Usage |
| Usage-based / metered | `<used>/<allowance>` (integers) | <80% default · 80–94% warning · ≥95% danger | "<scope> · Monthly allowance · resets <date>" | Open Settings → Billing & Usage |
| Add-on credits on top of base allowance | `<used>/<base>+<addon>` (e.g. `89/2500+500`) | Thresholds computed against `(base + addon_total)` | "<scope> · Base 2500 (resets <date>) · Add-on 500 (expires <date>) · Add-on 250 (expires <date>)" | Open Settings → Billing & Usage |
| Enterprise plan with contract limit | `<used>/<allowance>` | Same threshold rules as metered | "<scope> · Enterprise · resets <date>" | Open Settings → Billing & Usage |
| Enterprise plan, no contract numeric limit | Plan name only ("Enterprise"); no numeric usage | Default fg; thresholds NOT applied | "Enterprise plan · usage tracked centrally" | Open Settings → Billing & Usage |
| Delinquent (account past due) | `!` glyph | Danger fg | "Account past due" | Open Settings → Billing & Usage → Payment |
| Restricted (admin-disabled) | Lock glyph | Default fg | "Restricted by admin" | Open Settings → Billing & Usage (informational) |
| Missing allowance (used known, allowance unknown) | `<used>/—` | Default fg; thresholds NOT applied (avoid false alarms) | "Allowance unknown" | Open Settings → Billing & Usage |
| Indeterminate (both null while scope is valid) | `—/—` | Default fg | "Usage data unavailable for this scope." | Open Settings → Billing & Usage |
| Signed-out / no billing context | Segment hidden | n/a | n/a | n/a |

The "+addon" display only renders when the billing source
explicitly returns one or more add-on credit pools attached to
the current scope; if the API does not surface add-ons for the
account, the segment falls back to the metered render.

### B2. Refresh & caching (subscription model)

- Source of truth: the same Billing & Usage state stream that
  powers Settings → Billing & Usage. The status-bar segment MUST
  NOT introduce a second fetch pipeline, second client, or
  second polling loop.
- ONE shared, ref-counted subscription per process. The
  status-bar segment is a downstream observer of the existing
  Billing & Usage state stream. New observers (additional
  status-bar segments coming online in other windows or tabs,
  the Billing pane opening, etc.) ATTACH to the existing
  subscription via ref-count rather than triggering a new fetch
  or starting an additional polling loop. Last observer detaching
  drops the subscription.
- Cache TTL: ≤2s after the most-recent agent-turn-completion
  event for the active conversation. The segment subscribes to
  the same agent-turn completion signal already used by the
  conversation list and the Billing pane; it does NOT subscribe
  per-segment to billing fetches directly.
- Background refresh: when the shared subscription is alive,
  refresh on a 60s floor PLUS on every agent-turn completion
  (debounced to ≤2s). Refresh cadence is owned by the shared
  subscription, not by individual segments.
- Multiple windows / tabs: every additional visible segment is a
  no-op observer on the shared subscription. Per process there
  is exactly ONE billing fetch loop regardless of how many
  windows or tabs render the segment.
- On fetch error: retain the last successfully-fetched value, and
  show a stale indicator in the tooltip ("Last updated 12s ago,
  retrying").
- After 30s of consecutive fetch failures, hide the segment
  entirely. Resume display once a fetch succeeds. Hide-on-failure
  is computed from the shared subscription's error counter, not
  per-segment.

### B3. Scope (workspace selector)

- The segment follows the current workspace-billing-context
  selector (the same selector that drives the Billing pane).
- When the user switches workspace scope (e.g. "Personal" → "Acme
  Workspace"), the segment refetches and re-renders within the
  ≤2s debounce window.
- Tooltip shows scope name + allowance period, e.g.:
  `Acme Workspace · Monthly allowance · resets Dec 1`.

### B3a. Authorization & redaction

- Authorization: the data fetch path uses the SAME Billing &
  Usage authorization checks as the Billing pane. If the user
  lacks billing-view permission for the active workspace (e.g.
  member-without-billing-access on a team workspace), the segment
  hides entirely — there is no degraded "see used but not
  allowance" mode. The segment never bypasses the pane's authz.
- Workspace-scope changes: workspace-scope transitions go through
  the existing workspace-billing-context resolver. The segment
  never resolves workspace identity on its own. If the resolver
  reports the active workspace is not authorized for billing
  read, the segment hides without flashing stale numbers from a
  previous scope.
- Redaction at the chrome layer: because the segment is in
  always-visible chrome (and visible in screenshares,
  screenshots, screen recordings), the tooltip MUST NOT reveal
  workspace identifiers (workspace UUID, slug, billing customer
  id) that the user does not already see in the workspace
  selector. The tooltip uses the same human-readable workspace
  name the selector shows, never raw identifiers.
- No raw allowance/usage values are sent to telemetry from the
  segment (see Telemetry below).

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
- A6. With `plan == "unlimited"`, the segment renders `∞` and
  ignores threshold colors.
- A7. With `status_bar.show_ai_credits = false`, the segment is
  not rendered and incurs no extra fetch.
- A8. Right-click → "Hide credits in status bar" flips the
  setting to `false` and the segment disappears immediately.
- A9. Display-state matrix coverage: each row of B1a renders the
  exact glyph/text and color token specified, including
  delinquent, restricted, missing-allowance, enterprise-no-limit,
  and add-on credit cases.
- A10. Single shared subscription: with N status-bar segments
  visible simultaneously across windows/tabs (N ≥ 2), the
  process performs exactly ONE billing-state subscription and
  the underlying fetch cadence is unchanged from N=1.
- A11. Authorization: a user lacking billing-view permission for
  the active workspace has the segment hidden, even with
  `status_bar.show_ai_credits = true`. Switching to a workspace
  the user can view billing for makes the segment appear without
  ever flashing the previous workspace's numbers.
- A12. Telemetry payload contains only `{ event:
  "status_bar.segment_clicked", segment: "ai_credits" }`. No
  used / allowance / scope name / workspace id / plan tier
  fields are emitted from segment-owned code paths.

## Implementation Pointers

These pointers reference verified paths in the current codebase
(`git ls-files | grep -iE "(billing|status_bar|appearance)"`).
Where no module exists yet, the pointer is marked `(new module)`.

- Billing data source (existing):
  - `app/src/billing/mod.rs` — current billing module entrypoint.
    Extend with a thin observable that the status-bar segment can
    attach to. Do NOT introduce a parallel fetch loop.
  - `app/src/settings_view/billing_and_usage/mod.rs`,
    `app/src/settings_view/billing_and_usage_page.rs`,
    `app/src/settings_view/billing_and_usage/usage_history_model.rs`
    — current Billing & Usage pane. The status-bar segment
    consumes the SAME state stream this pane subscribes to.
  - `crates/graphql/src/api/billing.rs` — existing GraphQL
    billing query layer. No new endpoint required; segment uses
    whatever this layer already returns.
- Status-bar segment (new):
  - The current "context remaining" footer chip lives in
    `app/src/ai/blocklist/agent_view/agent_input_footer/mod.rs`
    (`update_context_window_button` at the agent input footer)
    and `chips.rs` next to it. The new credits segment can be
    added in a sibling module, e.g.
    `app/src/ai/blocklist/agent_view/agent_input_footer/ai_credits_chip.rs`
    `(new module)`, exposed via the existing chip selection
    pipeline used by `AgentToolbarItemKind`.
  - Existing block-level status bar:
    `app/src/ai/blocklist/block/status_bar.rs` (block-scoped, not
    the workspace footer — reference for status-bar styling
    tokens but not the integration site).
- Settings:
  - `app/src/settings/mod.rs` — register the new
    `status_bar.show_ai_credits` key here following the pattern
    used by sibling files (`accessibility.rs`, `block_visibility.rs`).
  - `app/src/settings_view/appearance_page.rs` — add the toggle
    row, with subtitle covering the signed-out / no-billing-context
    fallback.
- Agent-turn completion event channel: reuse the same channel the
  Billing pane and the conversation list already consume to drive
  the ≤2s refresh debounce. No new event channel.
- Workspace billing-context resolver: reuse the existing resolver
  that the Billing pane uses for workspace-scoped fetches; do not
  add a segment-local resolver.

If during implementation any of these paths have moved, update
the pointer in this spec rather than introducing a parallel
module to keep the no-extra-pipeline invariant intact.

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
- T9. Display-state matrix: each row of B1a renders the exact
  glyph/text and color token specified — covers unlimited,
  metered, add-on, enterprise-with-limit, enterprise-no-limit,
  delinquent, restricted, missing-allowance, indeterminate, and
  signed-out.
- T10. Shared subscription ref-count: spawning N≥2 segments
  results in exactly one underlying billing subscription; the
  last segment removed disposes the subscription. Verified by
  asserting the fetch counter increments at the same cadence as
  N=1.
- T11. Authorization gating: a user without billing-view
  permission for the active workspace never sees the segment,
  even with `status_bar.show_ai_credits = true`. Switching from a
  permitted workspace to a non-permitted one hides the segment
  without flashing previous-scope numbers.
- T12. Telemetry shape: assert click telemetry payload is exactly
  `{ event: "status_bar.segment_clicked", segment: "ai_credits" }`
  — no `used`, `allowance`, `scope_name`, `workspace_id`,
  `plan_tier`, or `reset_at` fields.

## Open Questions

- Q1. For unlimited plans, should we instead show daily-spend in
  USD as a more useful number? V1 proposal: no — show `∞` and
  punt to the Billing pane for spend breakdown. Revisit once the
  USD-spend spec (sibling PR #10224) ships.
- Q2. Should the segment be visible by default for new users on
  metered plans, since they are the audience that benefits most?
  V1 proposal: keep opt-in to avoid surprising existing users
  with new chrome; promote via a one-time tooltip on the Billing
  pane.
- Q3. Should we localize the `∞` glyph or render the literal word
  "Unlimited"? V1 proposal: glyph, with the word in the tooltip.
- Q4. For the add-on display `<used>/<base>+<addon>`, is the
  threshold computed against `base + addon_total` or against
  `base` alone with add-ons treated as overflow buffer? V1
  proposal: against `base + addon_total` (matches what the
  Billing pane shows as "available").

## Telemetry

- The status-bar segment emits exactly ONE event:
  - `{ event: "status_bar.segment_clicked", segment: "ai_credits" }`
  - No additional fields. No usage totals (`used`, `allowance`,
    `remaining_pct`), no scope names, no workspace identifiers
    (workspace id, slug, billing customer id), no plan tier, no
    reset timestamps. The `surface` discriminator stays scoped to
    pane-vs-segment attribution and never carries usage shape.
- The billing fetch itself reuses the existing Billing-pane fetch
  path and its existing instrumentation. If that path already
  has a `surface` field, the segment's attached observer
  contributes the value `"status_bar"`; otherwise the field is
  added with values constrained to the closed set
  `{"billing_pane", "status_bar"}`.
- Explicit non-goal: no segment-side telemetry beyond the click
  event. Any additional usage-shape telemetry (allowance crossing
  thresholds, plan-tier distributions, reset timing) remains
  owned by the Billing & Usage pane.
