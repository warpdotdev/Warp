---
name: 01 — Tab color shortcuts
status: draft
---

# Tab color shortcuts — TECH

Companion to [PRODUCT.md](PRODUCT.md). Section numbers below refer to PRODUCT.md.

## Context

The tab-color rendering and storage stack already exists upstream and is intact in this checkout. This feature only adds the keyboard surface, a hover tooltip on the tab color indicator, and a small palette extension within `AnsiColorIdentifier` (no new color enum).

Because the chosen palette (PRODUCT §1) is the eight ANSI colors — Red, Yellow, Green, Cyan, Blue, Magenta, White, Black — the existing `AnsiColorIdentifier` enum is already the right type. We do **not** introduce a new `TabColor` enum, do not change persistence, and do not need a YAML-deserialize shim. The two ANSI colors not currently in `TAB_COLOR_OPTIONS` (White, Black) are added there so the right-click menu and the keyboard surface offer the same set.

Relevant files on master:

- `app/src/tab.rs:79` — `SelectedTabColor` enum (`Unset` / `Cleared` / `Color(AnsiColorIdentifier)`).
- `app/src/tab.rs:143` — `TabData::selected_color: SelectedTabColor`.
- `app/src/tab.rs:443-540` — right-click color picker built from `TAB_COLOR_OPTIONS`, dispatches `WorkspaceAction::ToggleTabColor { color, tab_index }`.
- `app/src/tab.rs:1209-1290` — `TabComponent::compute_background_and_border` resolves the colored fill + border from `tab_data.color()`.
- `app/src/ui_components/color_dot.rs:18` — `pub(crate) const TAB_COLOR_OPTIONS: [AnsiColorIdentifier; 6] = [Red, Green, Yellow, Blue, Magenta, Cyan]`. This is the source of truth for both the right-click menu and the settings page palette.
- `app/src/workspace/action.rs:210` — `WorkspaceAction::ToggleTabColor { color: AnsiColorIdentifier, tab_index: usize }`. Used by the right-click menu. Toggle semantics: same color → clear, different color → set. **Stays unchanged.**
- `app/src/workspace/view.rs:5068` — `Workspace::toggle_tab_color(index, color, ctx)` — handles `ToggleTabColor`. Already chooses between `SelectedTabColor::Cleared` and `SelectedTabColor::Unset` based on `FeatureFlag::DirectoryTabColors`. Already emits `TabTelemetryAction::SetColor` / `ResetColor`.
- `app/src/workspace/mod.rs:488-560` — pattern for `EditableBinding::new("workspace:activate_first_tab", …, WorkspaceAction::ActivateTabByNumber(1)).with_key_binding("cmdorctrl-1")` etc. This is the precedent the new shortcuts follow.
- `app/src/workspace/mod.rs:935-944` — pattern for `EditableBinding::new("workspace:close_active_tab", …, WorkspaceAction::CloseActiveTab)` — precedent for "shortcut acts on active tab", with no per-tab parameter.
- `app/src/persistence/sqlite.rs:899-902` — serialize `selected_color` to the `tabs` table column via `serde_yaml`. No change needed; `AnsiColorIdentifier` already round-trips.
- `crates/warp_core/src/ui/theme/mod.rs:539` — `AnsiColorIdentifier { Black, Red, Green, Yellow, Blue, Magenta, Cyan, White }`. Untouched.
- `crates/warpui_core/src/keymap.rs:645` — `EditableBinding` builder API. We use `with_key_binding`, `with_group`, `with_context_predicate`.
- Workspace bindings group enum (location to confirm during impl — search for `BindingGroup::Navigation` / `BindingGroup::Close` declarations). New variant `BindingGroup::TabColor` lives there.

Upstream `oz-agent/APP-4321-active-tab-color-indication` (commit `86570e7`) is a pure visual upgrade for the active+colored tab (saturated border, distinct opacity for active vs hovered). It only touches `app/src/tab.rs` and `app/src/workspace/view/vertical_tabs.rs` — the `app/src/root_view.rs` hunks in that commit are unrelated free-tier-model-layer churn that twarp deletes anyway, and must be dropped during the cherry-pick.

## Proposed changes

### 1. Cherry-pick the upstream active-color-indication commit (prerequisite)

Cherry-pick `86570e7` from `upstream/oz-agent/APP-4321-active-tab-color-indication` onto the impl branch as the **first commit**, dropping the `root_view.rs` hunks. Result: the active colored tab gets a saturated border and a clearly brighter fill than inactive/hovered colored tabs. PRODUCT §1 ("indicator turns red") and the smoke test (visual recognition) implicitly assume this treatment.

If the cherry-pick conflicts more than trivially (e.g. master has moved on past the upstream branch point in `tab.rs`), fall back to porting the same diff by hand — the diff is small enough (≈40 lines net) that hand-porting is realistic.

### 2. Extend `TAB_COLOR_OPTIONS` from 6 to 8

Replace `app/src/ui_components/color_dot.rs:18`:

```rust
pub(crate) const TAB_COLOR_OPTIONS: [AnsiColorIdentifier; 8] = [
    AnsiColorIdentifier::Red,
    AnsiColorIdentifier::Yellow,
    AnsiColorIdentifier::Green,
    AnsiColorIdentifier::Cyan,
    AnsiColorIdentifier::Blue,
    AnsiColorIdentifier::Magenta,
    AnsiColorIdentifier::White,
    AnsiColorIdentifier::Black,
];
```

Order matches the PRODUCT.md table so the right-click menu's swatch order and the keyboard shortcut numbering line up visually.

The settings page at `app/src/settings_view/appearance_page.rs` iterates `TAB_COLOR_OPTIONS` to render swatch rows in two/three places (around lines 1222, 2471, 4819). With the array size change to 8, those rows automatically grow; verify the layout still fits and adjust the swatch row width if needed. No new settings UI is added — configurable bindings are out of scope.

No change to `AnsiColorIdentifier`, `SelectedTabColor`, persistence, or rendering.

### 3. New action variants

In `app/src/workspace/action.rs`, add two variants near `ToggleTabColor` (line 210):

```rust
SetActiveTabColor { color: AnsiColorIdentifier },
ResetActiveTabColor,
```

They take no `tab_index` — the handler resolves the active tab. Two variants (rather than one parameterized `Option<AnsiColorIdentifier>`) keeps the dispatch readable and matches the pattern of existing tab actions like `CloseActiveTab` / `ActivateTabByNumber`.

Keep `ToggleTabColor` unchanged. The right-click menu's existing toggle UX is correct for that surface; PRODUCT §3 forbids toggle semantics on the keyboard, and §7 requires both surfaces to render the same color identically. Two actions, one per input surface.

Add the new variants to whatever exhaustive-match arms exist for `WorkspaceAction` (e.g. the persistability filter at `app/src/workspace/action.rs:730` — match the surrounding pattern).

### 4. New handler methods

In `app/src/workspace/view.rs`, alongside `toggle_tab_color` (line 5068):

- `set_tab_color(&mut self, index: usize, color: AnsiColorIdentifier, ctx: &mut ViewContext<Self>)` — unconditional set. If `self.tabs[index].color() == Some(color)`, return without notifying (PRODUCT §3: same-color is a no-op). Otherwise set `self.tabs[index].selected_color = SelectedTabColor::Color(color)`, emit `TabTelemetryAction::SetColor`, and `ctx.notify()`.
- `reset_tab_color(&mut self, index: usize, ctx: &mut ViewContext<Self>)` — unconditional reset. If the tab is already uncolored (`selected_color` is `Unset`, or `Cleared` when `DirectoryTabColors` is enabled), return without notifying (PRODUCT §4 last bullet). Otherwise set to `SelectedTabColor::Cleared` if `FeatureFlag::DirectoryTabColors` is enabled, else `Unset` — same branch the existing `toggle_tab_color` already uses. Emit `TabTelemetryAction::ResetColor` and `ctx.notify()`.

Bounds-check the index identically to `toggle_tab_color` and `log::warn!` on miss. Both methods are pub.

Add thin "active tab" wrappers that resolve the active tab and delegate:

```rust
pub fn set_active_tab_color(&mut self, color: AnsiColorIdentifier, ctx: &mut ViewContext<Self>) {
    let Some(index) = self.active_tab_index() else { return };
    self.set_tab_color(index, color, ctx);
}

pub fn reset_active_tab_color(&mut self, ctx: &mut ViewContext<Self>) {
    let Some(index) = self.active_tab_index() else { return };
    self.reset_tab_color(index, ctx);
}
```

If there's no existing `active_tab_index()` helper, read whatever field the rest of the workspace view reads. PRODUCT §13: zero-tab state is a no-op (the `else { return }` covers it).

### 5. Action dispatch

In the `WorkspaceAction` match arm in `app/src/workspace/view.rs` (~line 20019, alongside the existing `ToggleTabColor` arm), add:

```rust
SetActiveTabColor { color } => self.set_active_tab_color(*color, ctx),
ResetActiveTabColor => self.reset_active_tab_color(ctx),
```

### 6. Register the nine keybindings

In `app/src/workspace/mod.rs`, in the same block as `workspace:activate_first_tab` (around lines 488-560), add nine `EditableBinding` entries:

```rust
EditableBinding::new(
    "workspace:set_active_tab_color_red",
    "Set tab color: Red",
    WorkspaceAction::SetActiveTabColor { color: AnsiColorIdentifier::Red },
)
.with_context_predicate(id!("Workspace"))
.with_group(bindings::BindingGroup::TabColor.as_str())
.with_key_binding("cmdorctrl-alt-1"),
// … repeat for Yellow/2, Green/3, Cyan/4, Blue/5, Magenta/6, White/7, Black/8 …
EditableBinding::new(
    "workspace:reset_active_tab_color",
    "Reset tab color",
    WorkspaceAction::ResetActiveTabColor,
)
.with_context_predicate(id!("Workspace"))
.with_group(bindings::BindingGroup::TabColor.as_str())
.with_key_binding("cmdorctrl-alt-0"),
```

Notes:

- `cmdorctrl-alt-<n>` (= ⌘⌥<n> on mac, Ctrl+Alt+<n> on Linux/Windows) is unbound today — re-`grep` `cmdorctrl-alt-[0-9]` and `cmd-alt-[0-9]` against `app/src/` immediately before adding to confirm. The existing `cmdorctrl-<n>` bindings for tab activation are unaffected.
- Add `BindingGroup::TabColor` to the bindings enum (file location: search for where `BindingGroup::Navigation` / `BindingGroup::Close` are defined — likely `app/src/workspace/bindings.rs`). The string label appears as the section heading in the keybindings settings page; pick "Tab color" (PRODUCT §16).
- The `id!("Workspace")` context predicate matches the existing tab bindings in this file, which gives us PRODUCT §12's focus-rules behavior: shortcuts are inactive when a modal/palette/settings-editor has captured focus, but active when the terminal pane has focus (the terminal pane does not push a competing context predicate).

### 7. Hover tooltip on the tab color indicator (PRODUCT §17)

When a tab has a manually-set color, hovering its indicator shows a tooltip in the form `<Color> — <shortcut>` (e.g. `Red — ⌘⌥1`). Implementation:

- The colored region of a tab is rendered by `TabComponent::compute_background_and_border` (`app/src/tab.rs:1209-1290`) — the entire tab fill carries the color. Attach the tooltip at this surface so any hover on the tab while it's colored surfaces the tooltip. If twarp already attaches a tooltip to tabs (e.g. for the path/session name), augment that tooltip's text rather than registering a second one.
- Build a helper function — colocate it with the tab component (or in `app/src/tab_color_tooltip.rs` if it grows):

  ```rust
  fn tab_color_shortcut_tooltip(
      color: AnsiColorIdentifier,
      keymap: &KeymapState, // or whatever lookup type twarp uses
  ) -> String {
      let id = match color {
          AnsiColorIdentifier::Red     => "workspace:set_active_tab_color_red",
          AnsiColorIdentifier::Yellow  => "workspace:set_active_tab_color_yellow",
          AnsiColorIdentifier::Green   => "workspace:set_active_tab_color_green",
          AnsiColorIdentifier::Cyan    => "workspace:set_active_tab_color_cyan",
          AnsiColorIdentifier::Blue    => "workspace:set_active_tab_color_blue",
          AnsiColorIdentifier::Magenta => "workspace:set_active_tab_color_magenta",
          AnsiColorIdentifier::White   => "workspace:set_active_tab_color_white",
          AnsiColorIdentifier::Black   => "workspace:set_active_tab_color_black",
      };
      match keymap.bound_key_for(id) {
          Some(key) => format!("{} — {}", color, key.to_glyph_string()),
          None => color.to_string(),
      }
  }
  ```

  The exact `keymap.bound_key_for` shape depends on twarp's keymap query API (search for an existing call site that reads a binding's bound key for display — there is precedent, e.g. menu items in `app/src/menu/mod.rs:927` use `with_custom_description(bindings::MAC_MENUS_CONTEXT, …)`). Reuse the existing helper rather than inventing a new one.

- The tooltip text is sourced live from the bound `EditableBinding`. If the user rebinds the shortcut, the tooltip updates automatically. If the user has unbound the shortcut for that color, the tooltip falls back to just `<Color>` with no shortcut suffix (no "Unbound" placeholder).
- Uncolored tabs: do not attach a color tooltip. The reset shortcut is discoverable through the keybindings settings page only — this is intentional per PRODUCT §17.
- The right-click "Set color" menu remains as-is. We do **not** add shortcut hints to its swatches; the tab indicator is the chosen discovery surface.

Add a unit test that stubs a known binding for `"workspace:set_active_tab_color_red"` and asserts the formatter produces `"Red — ⌘⌥1"` (or whatever the canonical glyph form is in twarp), and that an unbound id produces `"Red"` with no suffix.

### 8. Persistence

No change. `selected_color` already round-trips through sqlite (PRODUCT §10 satisfied by existing code).

### 9. Feature flag

None. PRODUCT §14 requires the feature ships unconditionally.

## Testing and validation

| PRODUCT § | Verification |
|-----------|--------------|
| §1 (shortcut → color set) | Smoke test step 2/9. Plus: unit test on `Workspace::set_active_tab_color` asserting `selected_color` becomes `Color(AnsiColorIdentifier::Red)` for the active tab. |
| §2 (only active tab) | Unit test: `set_active_tab_color`/`reset_active_tab_color` mutate only the active tab's `selected_color`; siblings untouched. |
| §3 (idempotent set, no toggle-off) | Unit test: invoke `set_tab_color(idx, Red)` twice; assert `selected_color` stays `Color(Red)` and the early-return short-circuits the `ctx.notify()` (or just assert `notify_count == 1`). Smoke step 4. |
| §4 (reset → directory default or uncolored) | Two unit tests on `reset_tab_color`: with `FeatureFlag::DirectoryTabColors` off, reset produces `Unset`; with it on, reset produces `Cleared` and `TabData::color()` returns the directory default. Smoke steps 6, 7. |
| §5 (different color replaces in place) | Unit test: set Red, then Green; assert final state is `Color(Green)` with no intermediate `Cleared` or `Unset`. Smoke step 5. |
| §6 (rapid presses) | Same as §5 — sequencing is sync; no animation buffer to test. |
| §7 (visual parity with menu) | Manual: right-click → "Red", then ⌘⌥0 ⌘⌥1 — confirm the rendered tab is pixel-identical. |
| §8 (per-tab, not per-pane) | No new test; `TabData::selected_color` is already at tab granularity. |
| §9 (multiple windows) | Manual only. State is per-`Workspace`, which is per-window; the action dispatches per-window via `id!("Workspace")`. |
| §10 (persistence across restart) | Smoke step 12. Plus: sqlite round-trip test next to existing tests in `app/src/persistence/sqlite_tests.rs:139, 222, 294`: persist a tab with `Color(AnsiColorIdentifier::White)` (a newly-supported value in `TAB_COLOR_OPTIONS`), re-read, assert equality. |
| §11 (new tabs unaffected) | Unit test: open a new tab after `set_active_tab_color`; assert its `selected_color` is `Unset`. |
| §12 (focus rules) | Smoke step 10 (terminal pane running `top`). The `id!("Workspace")` context predicate already implies window-level dispatch and is the same predicate the existing tab bindings use. |
| §13 (zero-tab no-op) | Unit test: with `self.tabs.is_empty()`, both `set_active_tab_color` and `reset_active_tab_color` return without panic and without notify. |
| §15 (no extra telemetry) | Unit test asserts `set_active_tab_color` emits exactly one `TabTelemetryAction::SetColor` (or `ResetColor`) per effective change, and zero on no-op. |
| §16 (entries listed and rebindable) | Smoke step 11 (visual check in keybindings settings). |
| §17 (tab-indicator hover tooltip) | Unit test on the tooltip formatter (bound and unbound cases — see §7). Smoke step 3. |

Required new unit tests live in:

- `app/src/tab.rs` (or wherever the new tooltip helper lands) — formatter test.
- `app/src/workspace/view_test.rs` — `set_active_tab_color` / `reset_active_tab_color` happy paths and §3/§5/§11/§13 cases.
- `app/src/persistence/sqlite_tests.rs` — round-trip with one of the newly-listed colors.

Run `./script/presubmit` until green before opening the impl PR (twarp-next workflow rule).

Integration test: skip. The keybinding routing layer is config-shaped and the keymap system has its own coverage. Manual smoke test is the canonical pre-merge check for the visual/keystroke flow.

## Risks and mitigations

- **Risk: keybinding conflict introduced upstream between spec and impl.** Mitigation: re-grep `cmdorctrl-alt-[0-9]` and `cmd-alt-[0-9]` immediately before adding the bindings.
- **Risk: settings page layout breaks at 8 swatches.** Mitigation: visual check at every `TAB_COLOR_OPTIONS` consumer site (1222, 2471, 4819) before opening the PR.
- **Risk: White and Black ANSI swatches read poorly on light/dark themes respectively.** Mitigation: visual check on both bundled light and dark themes during smoke testing. If unreadable, the existing `compute_background_and_border` logic is the right place to pick a contrast-aware fill, but keep changes there minimal — palette-rendering polish is out of scope.
- **Risk: tab tooltip integration conflicts with an existing per-tab tooltip (e.g. path/session name).** Mitigation: prefer augmenting the existing tooltip text over registering a second tooltip; if no per-tab tooltip exists today, add one only on colored tabs.

## Follow-ups

- User-configurable bindings (out of scope per PRODUCT.md non-goals).
- Custom-color picker (out of scope).
- Palette extension to the README §2 colors (Orange, Purple, Pink, Gray) — separate, larger feature requiring a `TabColor` enum decoupled from `AnsiColorIdentifier`. Tracked as a follow-up if the current ANSI palette proves insufficient.
- Surfacing the reset shortcut (⌘⌥0) via something other than the settings page, if discoverability proves to be a real problem in use.
