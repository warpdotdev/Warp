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
- B2. When the tab config opens, the named profile is applied to
  the pane before any agent input is dispatched. The profile
  lookup is by display name; if no matching profile exists, the
  pane opens with the default profile and a one-time toast warns
  the user.
- B3. `profile` is optional — omitting it preserves today's
  behavior (default profile).
- B4. The setting roundtrips through tab-config import/export.
- B5. The Tab Configs settings UI gets a profile picker on the
  agent-pane row. Picker shows current profile names from
  `Settings → Agents → Profiles`.
- B6. Profile name resolution is case-sensitive and trimmed (no
  partial matching). Renaming a profile breaks the binding —
  that's the same staleness model as missing-profile, with the
  same toast.

## Acceptance criteria

- A1. A tab config with `profile = "Coder devbox"` opens an
  Agent Mode pane already on that profile.
- A2. With `profile` omitted, behavior is identical to today.
- A3. With `profile = "Nonexistent"`, pane opens on default,
  toast warns of the missing profile name.
- A4. Tab-config TOML round-trips the `profile` field through
  import/export.

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

## Out of scope

- Multiple profiles per pane (e.g. fallbacks).
- Profile autocompletion in the TOML editor.
- Per-tab-config profile overrides at the tab-config level (this
  is per-pane).
