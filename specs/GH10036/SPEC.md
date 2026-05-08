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
- B4. Setting is exposed in the existing **"Code Editor and Review"**
  category in the Code settings page (verified in
  `app/src/settings_view/code_page.rs` lines 314 and 438, where the
  category is built via
  `Category::new("Code Editor and Review", code_editor_review_widgets)`).
  The setting is appended to the `code_editor_review_widgets`
  `Vec` adjacent to the existing `show_code_review_button`,
  `show_code_review_diff_stats`, and
  `auto_open_code_review_pane_on_first_agent_change` widgets — NOT
  in a separate "Editor → Code review" subpage (that path does not
  exist in the current settings tree).

  The user-facing label is "Open review panel maximised by default."
  The settings widget is built and appended in BOTH branches of the
  `cfg(...)` split at lines 297/301 and 421/425 (so both feature
  configurations get the toggle), matching the surrounding pattern
  for sibling code-review widgets.

- B5. The setting respects the existing maximise/restore state
  machine implemented in `app/src/workspace/view.rs`. The relevant
  pieces of the existing state machine are:
  - `PaneGroup::is_right_panel_maximized` (line 910) — the in-memory
    flag for the active pane group.
  - `RightPanelSnapshot::is_maximized` (lines 3852, 10030–10035) —
    the persisted prior state, restored on tab restore via
    `apply_right_panel_snapshot` (line 3847).
  - `toggle_right_panel_maximized` (line 8280) — toggles the flag
    and calls `view.set_maximized(...)` (lines 8284–8288).

  The new setting interacts with this state machine by these rules:

  1. Switching the setting at runtime does NOT retroactively
     maximise an already-open panel — only the next open uses the
     new default. (Same as before.)
  2. **Tab/session restore takes precedence over the setting.** When
     a tab is restored with a `RightPanelSnapshot` (i.e., the panel
     was open at suspend time), the panel reopens to
     `right_panel_snapshot.is_maximized` regardless of the setting
     value. Reasoning: the snapshot encodes the user's last explicit
     choice for that specific tab; the setting only affects the
     "fresh open" path where there is no prior state to honor.
  3. **Closing then reopening within a session uses the setting.**
     Once the panel is closed (no `RightPanelSnapshot` is in effect
     for the next open action), the next open through
     `setup_code_review_panel` (line 8001) or
     `open_code_review_panel_from_arg` (line 8055) consults the
     setting and seeds `is_right_panel_maximized` from it.
  4. **"Previously maximised before being closed" specifically.** If
     the user maximises the panel, then closes it, and then reopens
     it later (without a tab restore in between), the reopen behavior
     is governed by the setting, not by the prior in-session
     maximise state. Rationale: the in-memory `is_right_panel_maximized`
     flag is per-pane-group and is not retained across an explicit
     close; the only cross-close persistence is `RightPanelSnapshot`,
     which only applies on tab/session restore. This matches the
     spirit of the setting: it is a default for fresh opens, and the
     close-then-reopen sequence is a "fresh open" for our purposes.

  Acceptance B5 is the conjunction of B5.1–B5.4. Each sub-rule has
  a corresponding test in the Test plan.

## Acceptance criteria

- A1. With setting OFF (default), starting from no prior state:
  clicking code-review opens the side-panel; clicking the maximise
  control (or invoking the existing
  `workspace:toggle_maximize_code_review_panel` action) maximises
  it. Pixel-equivalent to today.
- A2. With setting ON, starting from no prior state: clicking
  code-review opens the panel already maximised in a single click.
  Invoking `workspace:toggle_maximize_code_review_panel` (Esc / the
  maximise control) restores the side-panel state.
- A3. Toggling the setting at runtime while a non-maximised panel
  is open does NOT auto-maximise it.
- A4. Tab/session restore: a tab whose `RightPanelSnapshot` has
  `is_maximized = true` reopens maximised on app restart even when
  the setting is OFF. A tab whose snapshot has
  `is_maximized = false` reopens as a side-panel on restart even
  when the setting is ON. The snapshot is authoritative for
  restored tabs; the setting only governs fresh opens. (Codifies
  B5.2 and prevents a regression where the new setting would
  override a saved per-tab state.)
- A5. Close-then-reopen within a session: maximise the panel, close
  it, then reopen via the code-review button (or
  `OpenCodeReviewPanel`). With the setting OFF, the reopen is a
  side-panel. With the setting ON, the reopen is maximised.
  Codifies B5.4.

## Implementation sketch

- Add the new setting in `app/src/settings/code.rs` by extending
  the existing `define_settings_group!(CodeSettings, settings: [
  ... ])` block. There is currently no `review_panel` anchor in
  that file; the existing siblings are `code_as_default_editor`,
  `codebase_context_enabled`, `auto_indexing_enabled`,
  `dismissed_code_toolbelt_new_feature_popup`,
  `show_project_explorer`, and `show_global_search`. Append the new
  field at the end of the array, matching the cadence and field
  shape of `show_project_explorer`:

  ```rust
  // app/src/settings/code.rs — appended to the CodeSettings group
  review_panel_open_maximized: ReviewPanelOpenMaximized {
      type: bool,
      default: false,
      supported_platforms: SupportedPlatforms::ALL,
      sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
      private: false,
      toml_path: "code.editor.review_panel.open_maximized",
      description: "Whether the code review panel opens already maximised.",
  },
  ```

- The settings UI widget is added in
  `app/src/settings_view/code_page.rs`. Both `code_editor_review_widgets`
  branches (around lines 297/301 and 421/425) extend their `Vec`
  with the new toggle. Place the new widget adjacent to the
  existing `show_code_review_button` /
  `auto_open_code_review_pane_on_first_agent_change` widgets within
  the `Category::new("Code Editor and Review", ...)` grouping (lines
  314, 438). Do NOT introduce a new category or settings page.

- The runtime entry points are `setup_code_review_panel` (line
  8001 of `app/src/workspace/view.rs`) and
  `open_code_review_panel_from_arg` (line 8055). At each entry
  point, before the `pane_group.update(...)` block that reads
  `is_right_panel_maximized`, branch on the source of the open:

  - If the open is part of a tab restore that supplied a
    `RightPanelSnapshot` (i.e., `apply_right_panel_snapshot` at line
    3847 has already seeded `pane_group.is_right_panel_maximized`),
    do nothing extra — the snapshot wins (B5.2 / A4).
  - Otherwise (fresh open, including close-then-reopen — see
    B5.4 / A5), read `code_settings.review_panel_open_maximized`
    and write it into `pane_group.is_right_panel_maximized`
    before calling `view.set_maximized(...)`. The existing
    `set_maximized` call at line 8123 / 8288 then performs the
    visible toggle. We do NOT introduce a new
    `OpenCodeReviewPanelMaximized` action variant; we seed the
    state on the existing path.

- The action `workspace:toggle_maximize_code_review_panel` (right_panel.rs
  lines 351, 407, 1042) is unmodified — it continues to flip
  `is_right_panel_maximized` for the current panel and is the
  affordance referenced in A2's "Esc / un-maximise" path.

## Test plan

- T1. Setting round-trips through TOML under the canonical
  `code.editor.review_panel.open_maximized` key (no other
  spelling).
- T2. Fresh open with setting ON seeds
  `is_right_panel_maximized = true`; with setting OFF it stays
  `false`. Asserted by driving `setup_code_review_panel` directly
  with no prior `RightPanelSnapshot` and inspecting the
  `PaneGroup` state.
- T3. Snapshot test: with setting OFF, the rendered tree on the
  fresh-open path is identical to the current build (regression
  guard for the default-off requirement).
- T4. Runtime toggle while a non-maximised panel is open does NOT
  retroactively maximise it (A3). Asserts the in-memory
  `is_right_panel_maximized` flag is unchanged after a settings
  write.
- T5. Tab restore overrides the setting (A4 / B5.2). With setting
  OFF, restore a tab whose `RightPanelSnapshot` has
  `is_maximized = true` and assert the restored panel is maximised.
  With setting ON, restore a tab whose snapshot has
  `is_maximized = false` and assert the restored panel is a
  side-panel.
- T6. Close-then-reopen within a session uses the setting (A5 /
  B5.4). Maximise the panel, close it, then reopen via
  `OpenCodeReviewPanel`. With setting OFF, the reopen is a
  side-panel; with ON, the reopen is maximised.
- T7. The settings widget is appended to the `code_editor_review_widgets`
  `Vec` in BOTH `cfg(...)` branches of
  `app/src/settings_view/code_page.rs` (lines 297/301 and 421/425),
  inside the `Category::new("Code Editor and Review", ...)` group
  — not in any new category.
- T8. Grep test: no occurrence of `editor.review_panel_open_maximized`,
  `code.editor.reviewPanel.openMaximized`,
  `code_editor.review_panel.open_maximized`, or any other variant
  appears in the diff. Only the canonical
  `code.editor.review_panel.open_maximized` is present.

## Out of scope

- Per-window or per-project override of the maximise default.
- Remembering the user's last maximise state across sessions
  (separate UX decision; this setting is just a default, not a
  state restore).
