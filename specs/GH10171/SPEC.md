# Spec: Tab configs specify agent profile (GH-10171)

## Problem

Tab configs support `type = "agent"` to open a pane in Agent Mode,
but there is no way to associate a specific agent profile (e.g.
"Coder devbox" with full autonomy on a remote dev machine) with
that pane. Users with multiple profiles must manually switch each
time the tab config is reused.

## Goal

Add a `profile` field to the `agent`-typed tab-config pane, naming
the agent profile to apply when the pane opens.

## Behavior contract

- B1. Tab-config TOML schema accepts a `profile` string field on
  agent panes:
  ```toml
  [[panes]]
  type = "agent"
  profile = "Coder devbox"
  ```
- B2. When the tab config opens, profile lookup is by trimmed,
  case-sensitive display name. If the profile exists and the agent
  pane has no initial input to auto-dispatch, the named profile is
  applied before the pane is shown.
- B2a. If the profile exists and the agent pane has initial input
  that would be dispatched on open, Warp must show a confirmation
  before dispatch. The confirmation discloses the profile name and
  that the initial input will run under that profile's autonomy and
  environment settings. Until the user confirms, no agent input is
  dispatched. Confirming applies the named profile and dispatches
  the initial input. Choosing the default profile applies the
  default profile and dispatches the initial input. Cancelling opens
  the pane with the initial input preserved as an editable draft and
  dispatches nothing.
- B2b. If no matching profile exists, the pane opens with the
  default profile. Missing-profile warnings are scoped per
  tab-config open and per missing profile name: if multiple panes in
  the same tab-config open reference the same missing profile, show
  one toast for that name and mention the number of affected panes;
  if different profile names are missing, show one toast per missing
  name. Opening the same tab config again may show the toast again.
- B3. `profile` is optional — omitting it preserves today's
  behavior (default profile).
- B4. The setting roundtrips through tab-config import/export.
- B5. The Tab Configs settings UI gets a profile picker on the
  agent-pane row. Picker shows current profile names from
  `Settings → Agents → Profiles`.
- B6. Profile name resolution does not use partial matching.
  Renaming a profile breaks the binding — that's the same staleness
  model as missing-profile, with the same toast behavior.

## Acceptance criteria

- A1. A tab config with `profile = "Coder devbox"` opens an
  Agent Mode pane already on that profile when there is no initial
  input, and does so after confirmation when initial input exists.
- A2. With `profile` omitted, behavior is identical to today.
- A3. With `profile = "Nonexistent"`, pane opens on default, and
  the missing-profile toast follows the B2b coalescing scope.
- A4. Tab-config TOML round-trips the `profile` field through
  import/export.
- A5. If a tab config has both `profile = "Coder devbox"` and
  initial agent input, the initial input is not dispatched until
  the user confirms which profile should run it.

## Implementation pointers

- Tab-config schema in
  `app/src/persistence/tab_configs/...` (search for `pane` /
  `type = "agent"` parsing).
- Pane open path in the workspace view's tab-config restoration
  logic.
- Profile model in
  `app/src/ai/execution_profiles/...` exposes a lookup-by-name.

## Test plan

- T1. Schema round-trip with the new field.
- T2. Open path applies the named profile when present.
- T3. Open path defaults + toasts on missing profile.
- T4. UI picker shows current profile list.
- T5. An agent pane with initial input applies the selected profile
  before dispatch only after the confirmation path completes; the
  cancel path leaves the input as a draft and dispatches nothing.
- T6. Multiple panes referencing the same missing profile during
  one tab-config open produce one coalesced toast; reopening the
  tab config can show it again.

## Out of scope

- Multiple profiles per pane (e.g. fallbacks).
- Profile autocompletion in the TOML editor.
- Per-tab-config profile overrides at the tab-config level (this
  is per-pane).
