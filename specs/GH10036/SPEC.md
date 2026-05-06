# Spec: Setting to open review panel maximised by default (GH-10036)

## Problem

Clicking the code-review button on the top right opens the review
panel as a side panel. Users on small screens (laptops, vertical
monitors) immediately maximise it via a second click. The two-click
flow is friction for the most common case.

## Goal

Add a boolean setting `editor.review_panel_open_maximized` (default
`false`) that, when enabled, opens the review panel pre-maximised.
The non-maximised default flow is unchanged.

## Behavior contract

- B1. New setting `editor.review_panel_open_maximized: bool`,
  default `false`, `SyncToCloud::Globally(RespectUserSyncSetting::Yes)`
  (matches the convention for UI prefs in
  `app/src/settings/code.rs:19/29/40/48`).
- B2. When `true`, clicking the code-review button opens the
  panel in maximised state directly. The Esc / un-maximise
  affordance still works to shrink it.
- B3. When `false`, no behavior change — the existing two-click
  flow is preserved exactly.
- B4. Setting is exposed in **Settings → Editor → Code review**
  with the label "Open review panel maximised by default."
- B5. The setting respects the existing maximise/restore state
  machine; switching the setting at runtime does NOT
  retroactively maximise an already-open panel — only the next
  open uses the new default.

## Acceptance criteria

- A1. With setting OFF (default): clicking code-review opens
  side-panel; second click maximises. (Pixel-equivalent to today.)
- A2. With setting ON: clicking code-review opens maximised in
  one click. Esc / un-maximise restores the side-panel state.
- A3. Toggling the setting at runtime while a non-maximised panel
  is open does NOT auto-maximise it.

## Implementation sketch

- Add the setting in `app/src/settings/code.rs` near the other
  review-panel settings (search for "review_panel" in that file).
- The toggle entry point lives in the code-review-open action;
  read the setting at action dispatch time and choose between the
  existing `OpenReviewPanel` and `OpenReviewPanelMaximized`
  intents (or whatever the maximised entrypoint is — verify by
  grepping for the un-maximise button's action).

## Test plan

- T1. Setting round-trips through TOML.
- T2. Action dispatch with setting ON produces the maximised
  variant; OFF produces the side-panel variant.
- T3. Snapshot test: with setting OFF, the rendered tree is
  identical to the current build.

## Out of scope

- Per-window or per-project override of the maximise default.
- Remembering the user's last maximise state across sessions
  (separate UX decision; this setting is just a default, not a
  state restore).
