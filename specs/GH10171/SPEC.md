# Spec: Tab configs specify agent profile (GH-10171)

## Problem

Tab configs support `type = "agent"` to open a pane in Agent Mode,
but there is no way to associate a specific agent profile (e.g.
"Coder devbox" with full autonomy on a remote dev machine) with
that pane. Users with multiple profiles must manually switch each
time the tab config is reused.

A tab config can also be authored, shared, or imported. Allowing
it to silently bind a high-autonomy profile and dispatch input on
open is a trust hazard. The spec must define a consent model so
that imported / shared configs cannot escalate autonomy without a
visible user gate.

## Goal

Add a `profile` field to the `agent`-typed tab-config pane, naming
the agent profile to apply when the pane opens. Gate any
profile-bound agent dispatch behind a trust/consent check on
open, with the gate auto-bypassed only for tab configs the user
authored or saved themselves.

## Behavior contract

### Schema and lookup

- B1. Tab-config TOML schema accepts a `profile` string field on
  agent panes:
  ```toml
  [[panes]]
  type = "agent"
  profile = "Coder devbox"
  ```
- B1a. The `profile` field accepts EITHER a bare display name
  (e.g. `"Coder devbox"`) OR a qualified
  `source:name` form (e.g. `"team:reviewer"`,
  `"user:Coder devbox"`, `"builtin:Default"`).
- B1b. Profiles are uniquely identified internally by the
  `(name, source)` pair where `source ∈ {user, team, builtin}`.

### Profile resolution

- B-LOOKUP. Profile resolution by `profile` field:
  1. If `profile` is qualified `source:name`, look up exactly
     that `(name, source)` pair. If not found → missing.
  2. If `profile` is a bare name, search in priority order
     `user > team > builtin`, first match wins.
  3. **Ambiguity check.** If the bare name matches more than one
     profile WITHIN the highest-priority source where it appears
     (e.g. user has two profiles both named `reviewer`), the
     binding is ambiguous → see B-MISSING / B-DISABLED-AMBIG.
  4. Renaming a profile breaks the binding — same as missing.
  5. Partial matching is never used.

### Trust & Consent model

- B-TRUST. Each tab config carries an internal trust marker
  recorded in user settings: `owned` (the user authored or saved
  this tab config in their own settings) or `imported` (the tab
  config came from a share link, file import, team config, or
  any other external source).
- B-TRUST-1. The first time the user explicitly saves a tab
  config to their own settings (whether authored from scratch or
  saved-from-import), it is marked `owned` from that point on.
- B-TRUST-2. **Confirmation required (imported configs).** When a
  tab config is `imported` AND any agent pane in it has either:
  - a non-default profile bound, OR
  - a profile whose autonomy mode is anything other than Manual
    (i.e., any elevated-autonomy profile, including the default
    if it has been raised), OR
  - any open-time agent input (initial prompt field, `commands`
    array, or any other tab-config-driven dispatch path)
  then the pane MUST display an inline disclosure card before
  dispatching ANY agent input. The card states:
  > "This tab config wants to use the **'[name]'** profile
  > (autonomy: **[level]**) and will run **[summary of input]**
  > on open."
  The user must click **Confirm** before any dispatch occurs.
  The disclosure resets per session for imported configs (asked
  again on first open of a new session).
- B-TRUST-3. **Confirmation bypassed (owned configs).** A tab
  config marked `owned` skips the disclosure card and dispatches
  the configured profile + input directly on open, because the
  user is the source of record.
- B-TRUST-4. The confirmation gate covers ALL open-time agent
  inputs uniformly — the initial prompt field, the `commands`
  array, and any future tab-config-driven dispatch path. None
  may dispatch until the user has confirmed (when required).
- B-TRUST-5. Cancel on the disclosure card opens the pane with
  the configured profile NOT applied (default profile), the
  initial input preserved as an editable draft, and the
  `commands` array discarded (with an inline note "[N] commands
  not run — confirm to enable").

### Missing / ambiguous / disabled-agent fallback

- B-MISSING. If `profile` is set but cannot be resolved at open
  time (renamed, deleted, never existed, or ambiguous bare
  name), the pane opens in **Disabled-Agent state**:
  - No agent process is started.
  - The initial prompt input AND the `commands` array are HELD
    (not dispatched, not discarded).
  - A one-time toast appears scoped to the affected tab-config
    open. Wording:
    - missing: `"Profile '[name]' not found — this tab will not
      start an agent. Edit the tab config or install the
      profile."`
    - ambiguous: `"Profile name '[name]' is ambiguous — qualify
      with source:name (e.g. user:[name])."`
  - The toast dismisses on user click or after 8 seconds.
- B-MISSING-1. There is NO silent fallback to a different
  profile (default or otherwise) when the configured profile
  cannot be resolved. The user must edit the config to fix it.
- B-MISSING-2. Multiple panes in the same tab-config open
  referencing the same missing/ambiguous name produce ONE
  coalesced toast for that name, mentioning the number of
  affected panes. Distinct names get distinct toasts. Reopening
  the same tab config in a later session may show the toast
  again.
- B-MISSING-3. The Disabled-Agent state offers an inline "Edit
  tab config" affordance. On save with a now-resolvable profile,
  the pane re-attempts open and proceeds through the normal
  trust/consent path (B-TRUST-2 / B-TRUST-3).

### Other

- B3. `profile` is optional — omitting it preserves today's
  behavior (default profile, subject to the trust gate if the
  config is `imported` AND has open-time agent input).
- B4. The setting roundtrips through tab-config import/export,
  including the qualified `source:name` form.
- B5. The Tab Configs settings UI gets a profile picker on the
  agent-pane row. Picker shows current profile names from
  `Settings → Agents → Profiles`. Profiles with conflicting
  bare names are shown in qualified `source:name` form to make
  the user's choice unambiguous in the saved config.
- B6. Profile name resolution does not use partial matching.

### Ordering invariant

- B-ORDER. **Profile resolution AND the trust check both
  complete before ANY initial input or `commands` array is
  dispatched.** The agent dispatcher receives a fully bound
  profile context as the argument of its very first invocation.
  Implementations MUST NOT spawn the agent, dispatch input, or
  begin command execution before profile resolution and consent
  resolution have both terminated.

## Acceptance criteria

- A1. A tab config (owned) with `profile = "Coder devbox"` opens
  an Agent Mode pane already on that profile, with no
  confirmation card, regardless of whether initial input exists.
- A2. With `profile` omitted, behavior is identical to today
  (subject to A-TRUST-IMPORTED if imported).
- A3. With `profile = "Nonexistent"`, the pane opens in
  Disabled-Agent state with the missing-profile toast per
  B-MISSING coalescing scope. Initial input AND `commands` are
  held, not dispatched.
- A-AMBIG. With a bare `profile = "reviewer"` matching two
  profiles in the highest-priority source, the pane opens in
  Disabled-Agent state with the ambiguous-name toast. Qualifying
  to `team:reviewer` resolves the binding.
- A4. Tab-config TOML round-trips the `profile` field through
  import/export, including qualified `source:name` form.
- A5. (Owned-config flow.) An owned tab config with both
  `profile = "Coder devbox"` and initial agent input dispatches
  the input under that profile on open with no confirmation
  card.
- A-TRUST-IMPORTED. (Imported-config flow.) An imported tab
  config with any non-default / elevated-autonomy profile OR any
  open-time agent input MUST show the disclosure card and MUST
  NOT dispatch anything until the user confirms. Cancel preserves
  the prompt as draft, discards `commands`, opens pane on default
  profile.
- A-TRUST-OWNED-PROMOTE. Saving an imported tab config to user
  settings flips its trust marker to `owned`; subsequent opens
  do not show the disclosure card.
- A-COMMANDS. Same trust + missing-profile rules apply
  identically to the `commands` array as to the initial prompt
  field.
- A-DISABLED-HOLD. In Disabled-Agent state, neither the initial
  prompt field nor the `commands` array runs. Editing the config
  to a resolvable profile and saving re-attempts open and then
  follows the normal trust/consent path.

## Implementation pointers

- Tab-config schema in
  `app/src/persistence/tab_configs/...` (search for `pane` /
  `type = "agent"` parsing). Add `profile: Option<String>` and
  trust marker (`source: TabConfigSource { Owned, Imported }`).
- Pane open path in the workspace view's tab-config restoration
  logic. Insert ordering gate per B-ORDER: resolve → trust check
  → dispatch.
- Profile model in
  `app/src/ai/execution_profiles/...` exposes a lookup-by
  `(name, source)` plus a bare-name resolver implementing
  `user > team > builtin` with ambiguity detection.
- Disabled-Agent state should be a first-class pane state, not
  an ad-hoc "no agent process" branch — surface it in the pane
  view, the status bar, and the inline edit-config affordance.

## Test plan

- T1. Schema round-trip with the new field, including qualified
  `source:name` form.
- T2. Open path applies the named profile when present (owned
  config flow).
- T3. Open path enters Disabled-Agent state on missing profile,
  emits the coalesced toast, and HOLDS both initial input and
  `commands` array (T3-INPUT, T3-COMMANDS sub-cases).
- T3-AMBIG. Open path enters Disabled-Agent state on ambiguous
  bare name, emits the ambiguous-name toast.
- T4. UI picker shows current profile list, surfaces qualified
  `source:name` for any duplicates.
- T5. Imported tab config with elevated profile + initial input
  shows the disclosure card; Confirm dispatches; Cancel
  preserves draft, discards `commands`, opens on default.
- T5-OWNED. Owned tab config with the same content dispatches
  immediately with no card.
- T5-PROMOTE. Saving imported → owned flips trust marker;
  subsequent open skips card.
- T6. Multiple panes referencing the same missing profile during
  one tab-config open produce one coalesced toast; reopening the
  tab config can show it again.
- T-COMMANDS. `commands` array is gated by the same
  consent/missing-profile rules as the initial prompt; both are
  held in Disabled-Agent state.
- T-ORDER. **Ordering test.** Profile resolution AND trust
  check both terminate before the dispatcher's first invocation.
  Verified via instrumentation that records the time of profile
  bind, trust resolution, and first dispatch — assertion is
  `bind_done <= dispatch_start && trust_done <= dispatch_start`.

## Out of scope

- Multiple profiles per pane (e.g. fallbacks).
- Profile autocompletion in the TOML editor.
- Per-tab-config profile overrides at the tab-config level (this
  is per-pane).
- Cross-machine profile sync — profiles are looked up locally;
  team/builtin sources are populated through existing channels.
