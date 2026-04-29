# Tab color shortcuts — TECH spec

Companion to [PRODUCT.md](PRODUCT.md). Behavior numbers below refer to that file.

## Context

Most of the tab-color machinery already exists on `master`; the work here is mainly **palette extension** (6 ANSI colors → 8 README colors) and **wiring keyboard shortcuts** to the active tab. The right-click "Set color" / "Reset color" menu is functional today.

Relevant files on master:

- `app/src/tab.rs:79` — `SelectedTabColor` enum (`Unset` / `Cleared` / `Color(AnsiColorIdentifier)`).
- `app/src/tab.rs:143` — `TabData::selected_color: SelectedTabColor`.
- `app/src/tab.rs:443-540` — right-click color picker built from `TAB_COLOR_OPTIONS`, dispatches `WorkspaceAction::ToggleTabColor { color, tab_index }`.
- `app/src/tab.rs:1209-1290` — `TabComponent::compute_background_and_border` resolves the colored fill + border from `tab_data.color()`.
- `app/src/ui_components/color_dot.rs:18` — `pub(crate) const TAB_COLOR_OPTIONS: [AnsiColorIdentifier; 6] = [Red, Green, Yellow, Blue, Magenta, Cyan]` (this is the source of truth for both the right-click menu and the settings page palette).
- `app/src/workspace/action.rs:210` — `WorkspaceAction::ToggleTabColor { color: AnsiColorIdentifier, tab_index: usize }`.
- `app/src/workspace/view.rs:5068` — `Workspace::toggle_tab_color(index, color, ctx)` — **toggle** semantics: pressing the same color the tab already has clears it (to `Cleared` if `DirectoryTabColors` is on, else `Unset`).
- `app/src/workspace/mod.rs:488-560` — pattern for `EditableBinding::new("workspace:activate_first_tab", …, WorkspaceAction::ActivateTabByNumber(1)).with_key_binding("cmdorctrl-1")` etc. This is the precedent the new shortcuts should follow.
- `app/src/workspace/mod.rs:935-944` — pattern for `EditableBinding::new("workspace:close_active_tab", …, WorkspaceAction::CloseActiveTab)` — precedent for "shortcut acts on active tab", with no per-tab parameter.
- `app/src/app_state.rs:66` — `AppStateTab::selected_color: SelectedTabColor`. Persistence flows through here.
- `app/src/persistence/sqlite.rs:899-902` — serialize `selected_color` to the `tabs` table column via `serde_yaml`.
- `app/src/persistence/sqlite.rs:2699-2709` — deserialize on load, mapping the YAML back into `SelectedTabColor`.
- `app/src/launch_configs/launch_config_tests.rs` — multiple test fixtures construct `selected_color: SelectedTabColor::default()`. These compile-break if the enum's shape changes.
- `crates/warp_core/src/ui/theme/mod.rs:539` — `AnsiColorIdentifier { Black, Red, Green, Yellow, Blue, Magenta, Cyan, White }`. Widely used by the theme system and terminal rendering — **must not** be extended with non-ANSI variants like Orange/Pink/Gray.

Upstream `oz-agent/APP-4321-active-tab-color-indication` (commit `86570e7`) is a pure visual upgrade for the active+colored tab (saturated border, distinct opacity for active vs hovered). It only touches `app/src/tab.rs` and `app/src/workspace/view/vertical_tabs.rs` — the `app/src/root_view.rs` hunks in that commit are unrelated free-tier-model-layer churn that twarp deletes anyway, and must be dropped during the cherry-pick.

## Proposed changes

### 1. Cherry-pick the upstream active-color-indication commit (prerequisite)

Cherry-pick `86570e7` from `upstream/oz-agent/APP-4321-active-tab-color-indication` onto the impl branch as the **first commit**, dropping the `root_view.rs` hunks. Result: the active colored tab gets a saturated border and a clearly brighter fill than inactive/hovered colored tabs. This is what the README intends visually and what the PRODUCT.md smoke test implicitly assumes ("indicator turns red" — readable on the active tab without squinting).

If the cherry-pick conflicts more than trivially (e.g. master has moved on past the upstream branch point in `tab.rs`), fall back to porting the same diff by hand — the diff is small enough (≈40 lines net) that hand-porting is realistic.

### 2. Introduce a `TabColor` enum decoupled from `AnsiColorIdentifier`

`AnsiColorIdentifier` is the wrong type to extend: it's the 8 standard ANSI terminal colors, used throughout terminal rendering and theme code. The README's palette (Red, Orange, Yellow, Green, Blue, Purple, Pink, Gray) is a **UI palette**, not an ANSI palette. Introduce a separate type.

New file `app/src/tab_color.rs` (or add to `app/src/tab.rs` if it stays small):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TabColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Gray,
}

impl TabColor {
    pub const ALL: [TabColor; 8] = [
        Self::Red, Self::Orange, Self::Yellow, Self::Green,
        Self::Blue, Self::Purple, Self::Pink, Self::Gray,
    ];

    /// Resolves to the swatch color used to tint the tab indicator.
    /// ANSI-aligned colors look up the active theme's ANSI palette so
    /// they match terminal output; off-ANSI colors use fixed sRGB values
    /// chosen to read clearly on both light and dark themes.
    pub fn to_color_u(self, theme: &WarpTheme) -> ColorU { … }

    /// Reverse mapping for the directory→color default (which produces
    /// `AnsiColorIdentifier`). Used so a directory rule keyed on
    /// `AnsiColorIdentifier::Magenta` still surfaces as a `TabColor`.
    pub fn from_ansi(c: AnsiColorIdentifier) -> Option<TabColor> {
        match c {
            AnsiColorIdentifier::Red     => Some(TabColor::Red),
            AnsiColorIdentifier::Yellow  => Some(TabColor::Yellow),
            AnsiColorIdentifier::Green   => Some(TabColor::Green),
            AnsiColorIdentifier::Blue    => Some(TabColor::Blue),
            AnsiColorIdentifier::Magenta => Some(TabColor::Purple),
            AnsiColorIdentifier::Cyan    => Some(TabColor::Blue),
            AnsiColorIdentifier::Black | AnsiColorIdentifier::White => None,
        }
    }
}
```

Concrete swatch values for the four off-ANSI colors (Orange, Purple, Pink, Gray) are an open question for the impl agent — pick values that are themable (use `WarpTheme` accent / surface colors where possible) and fall back to sRGB constants only if no themed value reads correctly. Document the chosen values in a `DECISIONS.md` if they end up hardcoded.

### 3. Re-shape `SelectedTabColor` around `TabColor`

In `app/src/tab.rs`:

```rust
pub enum SelectedTabColor {
    Unset,
    Cleared,
    Color(TabColor),  // was: AnsiColorIdentifier
}

impl SelectedTabColor {
    pub fn resolve(self, default: Option<AnsiColorIdentifier>) -> Option<TabColor> {
        match self {
            SelectedTabColor::Color(c) => Some(c),
            SelectedTabColor::Cleared  => None,
            SelectedTabColor::Unset    => default.and_then(TabColor::from_ansi),
        }
    }
}
```

Note that `resolve` now returns `Option<TabColor>` rather than `Option<AnsiColorIdentifier>`. The directory→color default (set elsewhere via `default_directory_color: Option<AnsiColorIdentifier>` on `TabData`) is still produced as an `AnsiColorIdentifier` because the directory-rules system uses the ANSI palette; we map it down to `TabColor` only at the point of rendering.

Update `TabData::color()` (and any callers) to expect `Option<TabColor>`. The render path in `app/src/tab.rs:1209-1290` and `app/src/workspace/view/vertical_tabs.rs` already needs a `ColorU` — switch the lookup from `theme.ansi_*` to `tab_color.to_color_u(theme)`.

**Backward compatibility for persisted YAML.** Existing tabs persisted on master serialize as e.g. `Color: red`, `Color: magenta`. The new shape would serialize as `Color: red`, `Color: purple`. Implement a custom `Deserialize` for `SelectedTabColor::Color(TabColor)` that accepts both:

- `red`, `green`, `yellow`, `blue` — pass-through.
- `magenta` — map to `Purple`.
- `cyan` — map to `Blue` (closest visual fit; document in `DECISIONS.md`).
- `orange`, `purple`, `pink`, `gray` — new variants.

This is a compatibility shim, not a long-term contract; twarp is pre-alpha. Keep the shim simple and add a unit test that round-trips both old and new tags.

### 4. Update `TAB_COLOR_OPTIONS` and the right-click menu

Replace `app/src/ui_components/color_dot.rs:18`:

```rust
pub(crate) const TAB_COLOR_OPTIONS: [TabColor; 8] = TabColor::ALL;
```

Tooltips and menu labels currently come from `AnsiColorIdentifier::Display`. Add `Display`/`label()` for `TabColor`. Update every site that imports `AnsiColorIdentifier` from `color_dot.rs` (the right-click menu in `tab.rs`, the settings page in `app/src/settings_view/appearance_page.rs`, lines around 110, 1222, 2471, 4819) to use `TabColor` instead.

### 5. New workspace action: `SetActiveTabColor`

In `app/src/workspace/action.rs`, add **one** parameterized action (cleaner than nine variants):

```rust
SetActiveTabColor(Option<TabColor>),  // None = reset
```

Existing `ToggleTabColor { color: AnsiColorIdentifier, tab_index: usize }` stays — it's the right-click menu's contract (toggle on repeated click of the same dot, parameterized by tab index). **Do not** repurpose `ToggleTabColor` for the keyboard path: PRODUCT.md §5 forbids toggle semantics on the keyboard ("pressing the same color is a no-op"), and the menu needs to keep its toggle behavior. Two actions, one per input surface.

Update the `Debug`-friendly enumeration around `app/src/workspace/action.rs:730` (the match-all-variants list) to include `SetActiveTabColor(_)`. Update the parameter type on `ToggleTabColor` to `TabColor`.

### 6. Implement `SetActiveTabColor` in `Workspace`

In `app/src/workspace/view.rs`, add:

```rust
pub fn set_active_tab_color(&mut self, color: Option<TabColor>, ctx: &mut ViewContext<Self>) {
    let Some(active) = self.tabs.get_mut(self.active_tab_index) else { return };
    let new_state = match color {
        Some(c) => SelectedTabColor::Color(c),
        None    => if FeatureFlag::DirectoryTabColors.is_enabled()
                       { SelectedTabColor::Cleared } else { SelectedTabColor::Unset },
    };
    if active.selected_color == new_state {
        return; // PRODUCT §5: no-op when value is unchanged
    }
    active.selected_color = new_state;
    send_telemetry_from_ctx!(
        TelemetryEvent::TabOperations {
            action: match color {
                Some(_) => TabTelemetryAction::SetColor,
                None    => TabTelemetryAction::ResetColor,
            },
        },
        ctx
    );
    ctx.notify();
}
```

Wire it in the dispatch in `app/src/workspace/view.rs:20019` next to `ToggleTabColor`:

```rust
SetActiveTabColor(color) => self.set_active_tab_color(*color, ctx),
```

Persistence is already triggered by the existing `ctx.notify()` flow + `AppState` snapshotting on tab mutations — no new persistence code required, only the YAML deserialize shim from §3.

### 7. Register the nine keybindings

In `app/src/workspace/mod.rs`, in the same block as `workspace:activate_first_tab` (around lines 488-560), add nine `EditableBinding` entries:

```rust
EditableBinding::new(
    "workspace:set_active_tab_color_red",
    "Set tab color: Red",
    WorkspaceAction::SetActiveTabColor(Some(TabColor::Red)),
)
.with_context_predicate(id!("Workspace"))
.with_group(bindings::BindingGroup::TabColor.as_str())  // new group, see below
.with_key_binding("cmdorctrl-alt-1"),
// … repeat for Orange/2, Yellow/3, Green/4, Blue/5, Purple/6, Pink/7, Gray/8 …
EditableBinding::new(
    "workspace:reset_active_tab_color",
    "Reset tab color",
    WorkspaceAction::SetActiveTabColor(None),
)
.with_context_predicate(id!("Workspace"))
.with_group(bindings::BindingGroup::TabColor.as_str())
.with_key_binding("cmdorctrl-alt-0"),
```

`cmdorctrl-alt-<n>` (= ⌘⌥<n> on mac, Ctrl+Alt+<n> on Linux/Windows) is unbound today — verified by `grep` against `app/src/`. The existing `cmdorctrl-<n>` bindings for tab activation are unaffected.

Add `BindingGroup::TabColor` to whatever enum lives at `app/src/workspace/bindings.rs` (the file where `BindingGroup::Close`, `Navigation`, etc. are defined — confirm the path during impl).

### 8. Settings-page surface

The settings page at `app/src/settings_view/appearance_page.rs` currently iterates `TAB_COLOR_OPTIONS` to render a 6-swatch row in two places (around lines 1222, 2471, 4819). With the palette change to 8, those rows automatically grow; verify the layout still fits and adjust the swatch row width if needed. No new settings UI is added — configurable bindings are out of scope.

### 9. Right-click menu tooltips show the keyboard shortcut

PRODUCT.md §15 requires the right-click "Set color" menu to surface the keyboard shortcut alongside each color, so a user discovering the menu also discovers the shortcut. Two surfaces to update:

- **`render_color_dot` tooltip** (`app/src/tab.rs:470-482`, the new picker). The `tooltip` string is built locally:
  ```rust
  let tooltip = match ansi_id {
      None => "Default (no color)".to_string(),
      Some(id) => id.to_string(),
  };
  ```
  Change this to also format the bound shortcut. Build a helper that, given a `TabColor` (or `None` for reset), looks up the active binding for the corresponding `EditableBinding` (e.g. `"workspace:set_active_tab_color_red"` / `"workspace:reset_active_tab_color"`) and returns its key-combination string in twarp's standard glyph form (⌘⌥1 etc.). Format as `"<Color> — <shortcut>"`. If the binding lookup returns no current key combo (user unbound it), fall back to just `<Color>` — no `Unbound` placeholder.

- **`legacy_color_option_menu_items`** (`app/src/tab.rs:518-540`, the icon-row variant). Each item is built via `MenuItemFields::new_with_icon(..., color_option.to_string())`. Replace the third argument with the same `<Color> — <shortcut>` formatter, **or** prefer twarp's existing right-aligned-shortcut-hint affordance on `MenuItemFields` if one exists (check the `MenuItemFields` API around `app/src/menu.rs` — the `MAC_MENUS_CONTEXT` description override pattern in `mod.rs:927` suggests there's already a way to attach shortcut hints to menu items; if so, use it, since it'll layout the shortcut on the right of the row instead of inline in the label).

The shortcut text must source from the live `EditableBinding` (look up by `id`), not a hardcoded string, so a future "configurable bindings" feature flips this for free. Search the codebase for existing call sites that read a binding's bound key for display — there is precedent (e.g. menu items in `mod.rs:927` use `with_custom_description(bindings::MAC_MENUS_CONTEXT, …)`); the impl agent should reuse that helper rather than inventing a new one.

Add a small unit test that stubs a known binding for `"workspace:set_active_tab_color_red"` and asserts the formatter produces `"Red — ⌘⌥1"` (or whatever the canonical glyph form is in twarp), and that an unbound id produces `"Red"` with no suffix.

### 10. Decision: Should `ToggleTabColor` remain ANSI-typed or move to `TabColor`?

Move it to `TabColor`. The right-click menu now offers `TabColor::ALL`; the action's parameter should match. Update `Workspace::toggle_tab_color` accordingly, plus the dispatch sites at `tab.rs:487, 492, 533`.

## Testing and validation

| PRODUCT § | Verification |
|-----------|--------------|
| §1 (shortcut → color set) | Manual smoke test step 2/4. Plus: unit test on `Workspace::set_active_tab_color` asserting `selected_color` becomes `Color(TabColor::Red)` for the active tab. |
| §2 (only active tab) | Unit test: assert other tabs' `selected_color` is unchanged after `set_active_tab_color`. |
| §3 (works regardless of inner focus) | Manual smoke test step 8 (terminal pane running `top`). The `id!("Workspace")` context predicate already implies window-level dispatch; no extra test. |
| §4 (reset → directory default or uncolored) | Two unit tests: with `FeatureFlag::DirectoryTabColors` off, reset produces `Unset`; with it on, reset produces `Cleared` and `TabData::color()` returns the directory default. |
| §5 (idempotent set) | Unit test: invoke `set_active_tab_color(Some(Red))` twice; assert `ctx.notify()` is called only once (or, simpler, that `selected_color` stays equal — the early-return short-circuits the notify). |
| §6 (replace in place) | Unit test: set Red, then Green; assert final state is `Color(Green)` with no intermediate `Cleared`. |
| §7 (rapid presses) | Same as §6 — sequencing is sync; no animation buffer to test. |
| §8 (visual parity with menu) | Manual: right-click → "Red", then ⌘⌥0 ⌘⌥1 — confirm the rendered tab is pixel-identical. |
| §9 (per-tab, not per-pane) | No new test; the existing `TabData::selected_color` is already at tab granularity. |
| §10 (persistence across restart) | Add a sqlite round-trip test next to existing tests in `app/src/persistence/sqlite_tests.rs:139, 222, 294`: persist a tab with `Color(TabColor::Purple)`, re-read, assert equality. Plus a separate **legacy-YAML compatibility** test: insert a row with `selected_color = Color: magenta` (the master serialization) and assert it loads as `Color(TabColor::Purple)`. |
| §11 (new tabs unaffected) | Unit test: open a new tab after `set_active_tab_color`; assert its `selected_color` is `Unset`. |
| §13 (multiple windows) | Manual only — multi-window integration tests are heavy; the action dispatches per-window and the state is per-`Workspace`, which is per-window. Add a code comment if non-obvious. |
| §14 (shortcut surface lists actions) | Visual check in keybindings settings: nine new entries appear under "Tab color". Verified by reading `BindingGroup::TabColor`. |
| §15 (right-click menu shows shortcuts) | Manual smoke step 9 (hover each color in the right-click menu, confirm tooltip text). Plus the unit test in §9 covering the `(color, binding) → tooltip` formatter, including the unbound fallback. |
| §16 (no extra telemetry) | Unit test asserts `set_active_tab_color` emits exactly one `TabTelemetryAction::SetColor` (or `ResetColor`) per call. |

Required new unit tests live in:

- `app/src/tab.rs` (or `app/src/tab_color.rs` if extracted) — `TabColor` serde round-trip + legacy-tag deserialization.
- `app/src/workspace/view_test.rs` — `set_active_tab_color` happy paths and §2/§5/§6/§11 cases.
- `app/src/persistence/sqlite_tests.rs` — round-trip incl. legacy YAML.

Run `./script/presubmit` until green before opening the impl PR (twarp-next workflow rule).

Integration test: skip. Reading `crates/integration` for a "press shortcut, assert tab indicator color" precedent is in scope of the impl agent if a clean precedent exists; if not, manual smoke test is sufficient — the workflow is one-keystroke and the assertion is visual.

## Risks and mitigations

- **Risk: legacy persisted YAML breaks.** Mitigation: deserialize shim in §3 plus the dedicated test. Twarp is pre-alpha so the blast radius is small (only people who ran master before this feature), but the shim is cheap.
- **Risk: palette swatches for Orange/Pink/Gray look wrong on one of the bundled themes.** Mitigation: prefer themed lookups; if hardcoding, screenshot every bundled theme manually before opening the PR. Document any theme that needs a per-theme override.
- **Risk: keybinding conflict introduced upstream between spec and impl.** Mitigation: re-grep `cmdorctrl-alt-[0-9]` and `cmd-alt-[0-9]` immediately before adding the bindings.
- **Risk: settings page layout breaks at 8 swatches.** Mitigation: visual check at every edit site (1222, 2471, 4819) before opening the PR.

## Follow-ups

- User-configurable bindings (out of scope per PRODUCT.md non-goals).
- Custom-color picker (out of scope).
- Removing the legacy-YAML deserialize shim once we're confident no twarp install carries master-era persisted state.
