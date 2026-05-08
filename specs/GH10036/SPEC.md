# Spec: Setting to open review panel maximised by default (GH-10036)

## Problem

Clicking the code-review button on the top right opens the review
panel as a side panel. Users on small screens (laptops, vertical
monitors) immediately maximise it via a second click. The two-click
flow is friction for the most common case.

## Goal

Add a boolean setting `code.editor.review_panel.open_maximized`
(default `false`) that, when enabled, opens the review panel
pre-maximised. The non-maximised default flow is unchanged.

The TOML key uses the existing `code.editor.*` namespace already in
use by other code-editor settings in this surface, with the
`review_panel` sub-namespace grouping panel-specific preferences.

### Canonical setting key

> **The single canonical TOML key is `code.editor.review_panel.open_maximized`.**
>
> This is the only spelling that appears anywhere in the
> implementation, the user-facing settings UI label, the
> documentation, the test fixtures, the migration code, the
> telemetry event payloads, or the changelog entry. Variants such as
> `editor.review_panel_open_maximized`,
> `code.editor.reviewPanel.openMaximized`,
> `code_editor.review_panel.open_maximized`, or any other casing /
> separator combination MUST NOT appear in any artifact shipped under
> this spec. Reviewers should grep the diff for those variants and
> reject any occurrence.
>
> The key is in `snake_case` segments separated by `.`, matching the
> convention already used for `code.editor.*` sibling keys in
> `app/src/settings/code.rs`.

## Behavior contract

- B1. New setting `code.editor.review_panel.open_maximized: bool`,
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

- T1. Setting round-trips through TOML under the
  `code.editor.review_panel.open_maximized` key.
- T2. Action dispatch with setting ON produces the maximised
  variant; OFF produces the side-panel variant.
- T3. Snapshot test: with setting OFF, the rendered tree is
  identical to the current build.
- T4. Runtime toggle: toggling
  `code.editor.review_panel.open_maximized` while the review panel
  is closed updates subsequent open behavior; toggling while the
  panel is open does not affect the active panel size (matches A3).

## Out of scope

- Per-window or per-project override of the maximise default.
- Remembering the user's last maximise state across sessions
  (separate UX decision; this setting is just a default, not a
  state restore).
