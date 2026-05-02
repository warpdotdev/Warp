# TECH.md — Per-tab theme override via launch configurations

Issue: https://github.com/warpdotdev/warp/issues/478
Related: https://github.com/warpdotdev/warp/issues/2618
Product spec: `specs/GH478/product.md`

## Context

The Warp theme is currently a single global value derived from
`ThemeSettings`. The renderer reads it through the global `Appearance`
singleton; nothing in the rendering pipeline today accepts a per-tab theme
parameter. Adding per-tab overrides therefore touches three layers — the
launch configuration schema, the persisted tab snapshot, and the renderer's
theme lookup — but does not require changing the theme storage format or
adding any new theme types.

The rest of this section enumerates the existing surfaces the implementation
will modify or read from. All file references are at HEAD on `master`.

### Launch configuration schema

`app/src/launch_configs/launch_config.rs` defines the YAML data model:

- `LaunchConfig` (lines 14–20) — top-level: `name`, `active_window_index`,
  `windows: Vec<WindowTemplate>`.
- `WindowTemplate` (lines 36–41) — `active_tab_index`, `tabs:
  Vec<TabTemplate>`. **No theme field today.**
- `TabTemplate` (lines 180–187) — `title`, `layout: PaneTemplateType`,
  `color: Option<AnsiColorIdentifier>`. The `color` field (line 186) is the
  closest precedent for the new `theme` field: same `Option<…>` shape, same
  serde skip-if-none treatment, deserialized into the same struct, and
  forwarded into the runtime tab. Importantly the file already imports
  `crate::themes::theme::AnsiColorIdentifier` (line 7), so importing
  `ThemeKind` from the same module is a one-line addition.
- `PaneTemplateType` (lines 92–113) — pane-level layout. Not modified by this
  spec; per-pane theming is explicitly out of scope per `product.md`.

### Theme model

`app/src/themes/theme.rs` defines:

- `ThemeKind` enum (lines 42–100) — the canonical theme identifier. Includes
  built-in themes (`Dark`, `Light`, `Dracula`, `DarkCity`, …) plus
  `Custom(CustomTheme)` and `CustomBase16(CustomTheme)` variants for
  user-supplied themes. Already derives `Serialize`, `Deserialize`,
  `JsonSchema`, and `settings_value::SettingsValue`, so it round-trips through
  serde with no additional work.
- `Display for ThemeKind` (lines 112+) — the canonical user-facing string for
  each variant. Used for both the global theme setting and the theme picker;
  reusable as-is for launch configuration YAML.

### Theme settings and resolution

`app/src/settings/theme.rs` defines:

- `ThemeSettings` group (lines 14–48) — three fields: `theme_kind`,
  `use_system_theme`, `selected_system_themes`. The `theme_kind` field's
  `toml_path` is `"appearance.themes.theme"` (line 23).
- `derived_theme_kind(theme_settings, system_theme) -> ThemeKind`
  (lines 69–78) — applies the system-light/dark logic. **This is the function
  the per-tab override must wrap**: when an override is present we want to
  return it directly; when absent we want this function's existing answer.
- `active_theme_kind(theme_settings, app) -> ThemeKind` (lines 81–83) — calls
  `derived_theme_kind` with `app.system_theme()`. Currently the only entry
  point for "what theme should we show?" outside the appearance manager.

### Renderer entry point

`app/src/appearance.rs` re-exports the singleton `Appearance` from
`warp_core::ui::appearance` (line 34) and defines `AppearanceManager`
(lines 40–47) that subscribes to `ThemeSettings` (line 51) and pushes theme,
font, and icon changes into `Appearance`. `Appearance::handle(ctx)` is the
canonical "give me the active theme" handle used by the terminal view, the
theme picker, and other UI surfaces.

`Appearance` today holds **one** theme. It does not have a notion of "the
theme for tab X". Plumbing per-tab overrides will therefore introduce a
*resolved theme* lookup that consults the active tab's override before
falling back to `Appearance`'s global value (see *Proposed changes* below).

### Tab and window state

`app/src/app_state.rs`:

- `WindowSnapshot` (lines 43–59) — persisted window state. **No theme field.**
- `TabSnapshot` (lines 61–69) — persisted per-tab state. Carries
  `default_directory_color` and `selected_color: SelectedTabColor` (the
  per-tab indicator color from `#7` in `product.md`). **No theme field.**
  This is where the persisted override is added.
- `TabSnapshot::color()` (lines 71–75) — resolves the indicator color. The
  per-tab theme override resolution will live next to this method and follow
  the same shape.

### Existing precedent — per-tab color flow

The path that loads `TabTemplate.color` into a runtime tab is the closest
analog to what this spec adds, and the implementation should mirror it
exactly. The flow today:

1. `TabTemplate.color: Option<AnsiColorIdentifier>` parsed from YAML
   (`launch_config.rs:186`).
2. On launch-config open, `app/src/workspace/view.rs:3517–3519` writes
   `tab_template.color` into `self.tabs[…].selected_color` via
   `SelectedTabColor::Color`. Tabs without `color` get `SelectedTabColor::Unset`.
3. `TabSnapshot.selected_color` persists this through session save/restore.
4. `TabSnapshot::color()` resolves `selected_color` against
   `default_directory_color` and the resulting `Option<AnsiColorIdentifier>`
   is rendered as the tab indicator.

The new theme override follows the same four steps, replacing
`AnsiColorIdentifier` with `ThemeKind` and routing the resolved value into
the renderer instead of the indicator widget.

## Proposed changes

The implementation is broken into four areas, in dependency order. Each step
compiles and passes tests on its own.

### 1. Schema: add `theme` to `TabTemplate` and `WindowTemplate`

File: `app/src/launch_configs/launch_config.rs`.

- Add `use crate::themes::theme::ThemeKind;` next to the existing
  `AnsiColorIdentifier` import on line 7.
- Add a new field to `TabTemplate` (lines 180–187):

  ```rust
  #[serde(skip_serializing_if = "Option::is_none", default)]
  pub theme: Option<ThemeKind>,
  ```

- Add the same field to `WindowTemplate` (lines 36–41) so a launch
  configuration can theme an entire window without repeating the value on
  every tab. Window-level inheritance is resolved at open time
  (`workspace/view.rs`, step 3 below); the runtime never needs to consult
  `WindowTemplate` after that.
- Update `impl TryFrom<TabSnapshot> for TabTemplate` (lines 189–200) to copy
  the override out of the snapshot. This keeps "save layout as launch
  configuration" round-tripping symmetric, satisfying behavior #10 of
  `product.md`.
- Update `impl From<WindowSnapshot> for WindowTemplate` (lines 43–70) to
  emit the window-level `theme` when every tab in the window has the same
  explicit override (the coalescing rule in behavior #10). When emitted at
  the window level the per-tab fields are dropped; otherwise per-tab
  fields are emitted and the window field is `None`.

`ThemeKind` already derives `Serialize` and `Deserialize`, so YAML support
comes for free. Round-trip and serde tests live in
`launch_configs/launch_config_tests.rs` (referenced by the `#[path]`
attribute on line 11) and gain coverage in step 5 below.

### 2. Persisted state: add `theme_override` to `TabSnapshot`

File: `app/src/app_state.rs`.

- Add `pub theme_override: Option<ThemeKind>` to `TabSnapshot` (lines 61–69).
  Place it next to `selected_color` so the "what's been overridden on this
  tab" fields stay grouped.
- Mirror this in the runtime tab type (`app/src/tab.rs`, `TabData` around
  line 134, alongside the existing `selected_color` field).
- Wire snapshot ↔ runtime conversion. The codebase has bidirectional
  conversions between `TabSnapshot` and the runtime tab; both directions
  copy the new field unchanged.
- `WindowSnapshot` does **not** gain a theme field. Window-level themes are a
  pure launch-configuration convenience: at open time we expand them into
  per-tab overrides (step 3) and never persist the window-level form. This
  keeps session restore unambiguous — every tab's effective override is
  stored on the tab itself.

### 3. Apply on open: window/tab template → runtime tab

File: `app/src/workspace/view.rs`, the `for_each` block at lines 3506–3520
that already applies `tab_template.color`.

- After the existing `selected_color` assignment (lines 3517–3519), assign
  `theme_override`:

  ```rust
  let resolved_theme = tab_template
      .theme
      .clone()
      .or_else(|| window.theme.clone());
  self.tabs[start_index + tab_index].theme_override = resolved_theme;
  ```

  This implements behavior #2 of `product.md` — tab-level wins, window-level
  is the fallback. It also closes over `window` exactly the way
  `tab_template` already does.
- No other code path in this function changes. Tabs without `theme` (and
  without a window-level value) get `theme_override = None`, which is
  identical to the pre-feature behavior.

### 4. Renderer: theme resolution in `Appearance` lookup

This is the architecturally interesting step. Two options were considered:

**Option A — Resolved-theme accessor on `Appearance`**: extend `Appearance`
with `pub fn theme_for_tab(&self, override: Option<&ThemeKind>) -> &WarpTheme`
that returns the override's resolved theme when `Some`, or
`self.theme()` when `None`. Every consumer that reads the active theme is
updated to call `theme_for_tab(active_tab.theme_override.as_ref())`.

**Option B — Per-tab `Appearance` instances**: clone `Appearance` per
overridden tab and swap which one is consulted on tab activation.

Option A is recommended. It localizes the change (one new method on
`Appearance`, one new lookup point per consumer), keeps font / icon /
non-theme appearance state shared (those are still global today), and fits
the existing "Appearance is one global thing, but the value it returns can
depend on context" pattern that already governs system-light/dark resolution.
Option B duplicates state, complicates change propagation
(`AppearanceManager`'s subscription would have to fan out to every
overridden tab), and tempts later code into per-tab font or icon overrides
that this spec explicitly does not promise.

Concretely:

- Add `pub fn theme_for(&self, override: Option<&ThemeKind>) -> &WarpTheme`
  to `warp_core::ui::appearance::Appearance` (the canonical type re-exported
  from `app/src/appearance.rs:34`). When `override` is `Some`, look the theme
  up via the same loader the global theme already uses; when `None`, return
  the existing `self.theme()`. Loader fallbacks (unknown theme name,
  missing custom theme file) reuse the existing global-theme fallback path,
  satisfying behaviors #11 and #12 of `product.md`.
- Update the terminal cell renderer (`app/src/terminal/view.rs` and
  `app/src/terminal/color.rs`, the consumers identified in the codebase
  map) to consult `theme_for(tab.theme_override.as_ref())` rather than the
  global `Appearance::theme`. The exact call sites will be enumerated when
  the implementation PR is opened — they all flow through the existing
  `Appearance::as_ref(ctx).theme()` lookup, so the change is a mechanical
  swap with the new accessor at each site.
- Window chrome (title bar, sidebar, settings views, tab strip) continues to
  consult `Appearance::theme()` directly with no override argument,
  satisfying behavior #4 of `product.md`.

### 5. Right-click menu: "Reset theme"

The existing tab context menu already exposes per-tab attributes. Add a
"Reset theme" entry that:

- Is shown only when the active tab's `theme_override` is `Some`.
- On click, sets `theme_override` to `None` on the tab and triggers the
  renderer's existing theme-changed redraw path (the one used today when the
  global theme changes).

This menu entry is the only in-app surface for clearing an override; setting
overrides remains the launch-configuration file's job per `product.md`.
Telemetry: emit a single counter on click using the existing tab-menu
telemetry conventions; no new event schema is required.

## Testing and validation

### Unit tests

- `app/src/launch_configs/launch_config_tests.rs`:
  - YAML round-trip for `TabTemplate` with `theme: "Solarized Dark"`.
  - YAML round-trip for `TabTemplate` with `theme: { custom: { … } }` to
    cover the `Custom` variant.
  - Round-trip for `WindowTemplate.theme` set with all tabs un-themed,
    asserting tabs inherit the window value at open time.
  - Round-trip for a launch configuration mixing some themed tabs and some
    un-themed tabs in the same window.
  - Negative test: an unknown theme string deserializes successfully into
    `Some(ThemeKind::…)`-fallback or surfaces a deserialization error that
    the launch-config loader catches and converts to a logged warning. The
    behavior here matches whatever path `ThemeKind` already uses for
    `appearance.themes.theme`; the test pins it.

- `app/src/app_state.rs` (or its test module):
  - `TabSnapshot` with `theme_override: Some(ThemeKind::Dracula)` round-trips
    through whatever serializer session-restore uses.
  - `TabSnapshot::color()` continues to return the same value with and
    without `theme_override` set (the indicator color and the theme override
    are independent — behavior #7 of `product.md`).

- `app/src/themes/theme.rs` or `app/src/appearance.rs`:
  - `Appearance::theme_for(None)` returns the same reference as
    `Appearance::theme()`.
  - `Appearance::theme_for(Some(&ThemeKind::Light))` resolves to the Light
    theme regardless of the global setting.
  - `Appearance::theme_for(Some(&ThemeKind::Custom(missing)))` returns the
    global theme (fallback path) and increments whatever warning counter the
    global custom-theme loader uses today.

### Integration tests

`crates/integration/` is the home for user-flow tests per `CONTRIBUTING.md`.
Add coverage for:

- Open a launch configuration with three tabs — one with `theme: "Dracula"`,
  one with `theme: "Solarized Light"`, one un-themed. Assert each tab
  renders with the expected theme and that switching the active tab does not
  change the rendering of the inactive tabs.
- Change the global theme via settings while the above launch configuration
  is open. Assert the un-themed tab redraws to match the new global theme;
  the two themed tabs do not redraw (behavior #6 of `product.md`).
- Right-click the Dracula-themed tab → "Reset theme". Assert the tab
  redraws to match the global theme and that the menu entry disappears.
- Quit and relaunch with session restore enabled. Assert the Dracula and
  Solarized Light overrides persist; the previously-reset tab continues to
  follow the global theme.

### Manual verification

- macOS, Linux, Windows: open the test launch configuration above and visually
  confirm each tab's terminal background and ANSI palette match the chosen
  theme. Confirm the window chrome and tab strip continue to follow the
  global theme.
- Toggle system light/dark: confirm un-themed tabs follow the system, themed
  tabs do not.
- Save layout as launch configuration with mixed overrides; reopen the saved
  YAML and confirm the per-tab `theme:` fields match what was set.
- Save layout as launch configuration with every tab on the same explicit
  override; confirm the YAML coalesces to a window-level `theme:` per
  behavior #10 of `product.md`.
- Edit a launch configuration's `theme:` value; reopen Warp; confirm
  already-open tabs from a previous run keep their original override, new
  tabs from the edited launch configuration get the new value (behavior #13).

### Tooling

- `cargo fmt` and
  `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings`
  must pass per `CONTRIBUTING.md` style rules.
- `cargo nextest run` for unit tests; `crates/integration` for end-to-end.
- `./script/presubmit` before pushing.

## Risks and mitigations

- **Risk: rendering pipeline is hotter than expected and an extra
  `Option<&ThemeKind>` argument shows up in profiling.** Mitigation: the
  override is a small enum and the `theme_for` lookup is one branch and one
  `HashMap` hit at most. The existing per-tab indicator color path executes
  on the same render frame and has not been a bottleneck. If profiling
  flags it post-merge, cache the resolved `&WarpTheme` on the runtime tab.

- **Risk: subtle visual mismatch where a UI surface today reads from
  `Appearance::theme()` but logically belongs *inside* a tab (e.g. block
  output styling) and we miss it during the consumer-update sweep.**
  Mitigation: the integration tests above visually exercise terminal
  background, ANSI palette, and block output; the manual verification step
  walks across the in-tab UI surfaces explicitly. Any miss surfaces as "this
  bit didn't change theme" rather than as a crash, and is fixable with a
  follow-up call-site update.

- **Risk: round-trip save/load asymmetry produces a launch configuration that
  no longer reproduces the user's tabs.** Mitigation: behavior #10 in
  `product.md` is precise about when `theme:` is emitted at the tab vs.
  window level, and the launch-configuration round-trip tests pin both
  directions.

- **Risk: an unknown theme string in a launch configuration shipped between
  Warp versions silently downgrades a user's tab.** Mitigation: behavior #11
  of `product.md` mandates a logged warning identifying the offending
  launch configuration, tab, and theme name. The fallback is the same
  one global-theme uses today, so the tab still opens.

- **Risk: persisted `theme_override` collides with a future per-window or
  per-pane theme feature.** Mitigation: the field is named
  `theme_override` (not `theme`) precisely to leave room for a separate
  resolved theme to live elsewhere later. Per-pane theming is explicitly
  out of scope; per-window theming, if added, would also produce a
  per-tab override at open time and reuse this same field.

## Follow-ups (deliberately not in this PR)

Tracked separately so that this spec stays scoped:

- **Auto-theme by SSH host or hostname** (raised by `stevenchanin`,
  `pyronaur`, `zethon`, `janderegg` in `#478`). Requires a detection layer
  that observes process state inside the tab. Would consume the
  `theme_override` field this spec adds.
- **Auto-theme by cwd** (`pyronaur`). Same shape as the SSH case; consumes
  the field.
- **Runtime escape-code or shell-hook protocol for setting a tab's theme**
  (raised by `yatharth` for Claude Code session signaling). Defines a wire
  format and a security model; consumes the field.
- **In-app per-tab theme picker submenu in the right-click menu** (open
  question in `product.md`). Strictly additive to this spec.
- **Wallpaper-per-tab** (`scottaw66`, `SheepDomination`). Different surface
  area entirely (asset loading, layering); does not conflict with this
  spec.
- **Closing `#2618` as a duplicate** of `#478` once this spec lands.
