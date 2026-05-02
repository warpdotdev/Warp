# TECH.md — Per-tab theme overrides driven by directory and launch configurations

Issue: https://github.com/warpdotdev/warp/issues/478
Related: https://github.com/warpdotdev/warp/issues/2618
Product spec: `specs/GH478/product.md`

## Context

The Warp theme is currently a single global value derived from
`ThemeSettings`. The renderer reads it through the global `Appearance`
singleton, which today owns exactly one resolved `WarpTheme`. Per-tab
overrides therefore require changes in four layers — the launch
configuration schema, a new directory-pattern settings group, the
persisted tab snapshot (with override-source semantics), and the
renderer's theme lookup (with a theme catalog so non-active themes can be
borrowed by reference). None of this requires changing the theme storage
format or adding new theme types.

All file references are at HEAD on `master`.

### Launch configuration schema

`app/src/launch_configs/launch_config.rs` defines the YAML data model:

- `LaunchConfig` (lines 14–20) — `name`, `active_window_index`, `windows`.
- `WindowTemplate` (lines 36–41) — `active_tab_index`, `tabs`. **No theme
  field today.**
- `TabTemplate` (lines 180–187) — `title`, `layout`, `color:
  Option<AnsiColorIdentifier>`. The `color` field (line 186) is the
  closest precedent for a per-tab theme field.
- The file already imports `crate::themes::theme::AnsiColorIdentifier`
  (line 7).

### Theme model and global theme settings

- `app/src/themes/theme.rs:42–100` — `ThemeKind` enum, the canonical
  theme identifier. Built-in variants plus `Custom(CustomTheme)` and
  `CustomBase16(CustomTheme)`. Derives `Serialize`, `Deserialize`,
  `JsonSchema`, `settings_value::SettingsValue`.
- `app/src/themes/theme.rs:112+` — `Display for ThemeKind` returns the
  canonical user-facing string (`"Dark City"`, `"Solarized Dark"`, etc.).
- `app/src/settings/theme.rs:14–48` — `ThemeSettings` group; `theme_kind`
  field with `toml_path: "appearance.themes.theme"` (line 23).
- `app/src/settings/theme.rs:69–78` — `derived_theme_kind`, applies
  system-light/dark logic. Wrapped by per-tab resolution.
- `app/src/settings/theme.rs:81–83` — `active_theme_kind`, current entry
  point for "what theme should we show?".

### Renderer entry point

- `app/src/appearance.rs:34` re-exports `Appearance` from
  `warp_core::ui::appearance`.
- `app/src/appearance.rs:40–47` — `AppearanceManager`; subscribes to
  `ThemeSettings` (line 51) and pushes changes into `Appearance`.
- `Appearance` today owns one `WarpTheme`. Per-tab overrides require
  it to own a *catalog* (see *Proposed changes* §5).

### Tab and window state

- `app/src/app_state.rs:43–59` — `WindowSnapshot`. **No theme field.**
- `app/src/app_state.rs:61–69` — `TabSnapshot`. Carries
  `default_directory_color` and `selected_color: SelectedTabColor` (the
  per-tab indicator color).
- `app/src/app_state.rs:71–75` — `TabSnapshot::color()`, the resolution
  helper for the indicator.
- `app/src/tab.rs:134–143` — `TabData` (runtime), holds `selected_color`.

### Existing precedent — per-tab color

The path that loads `TabTemplate.color` into a runtime tab is the
closest analog. Today:

1. `TabTemplate.color: Option<AnsiColorIdentifier>` parsed from YAML
   (`launch_config.rs:186`).
2. On launch-config open, `app/src/workspace/view.rs:3517–3519` writes
   `tab_template.color` into `self.tabs[…].selected_color` via
   `SelectedTabColor::Color`.
3. `TabSnapshot.selected_color` persists this through session save/restore.

The new override follows the same path but with richer types (an
override-source enum, see §3) and an apply-time resolver (§2) instead of
direct serde (which is what Oz's first-pass review correctly flagged).

### Existing pane cwd tracking

The active pane's cwd is already tracked for tab title generation and
breadcrumbs. The directory-pattern feature (§4) hooks into the existing
"focused pane cwd changed" event rather than introducing new shell-side
plumbing.

## Proposed changes

Six steps in dependency order. Each compiles standalone.

### 1. Schema: `theme: Option<String>` on `TabTemplate` and `WindowTemplate`

File: `app/src/launch_configs/launch_config.rs`.

Add a new field to both structs:

```rust
#[serde(skip_serializing_if = "Option::is_none", default)]
pub theme: Option<String>,
```

The field is `Option<String>` rather than `Option<ThemeKind>` for two
reasons that directly address Oz's first-pass concerns:

- **Serde-compat (Oz concern 1).** `ThemeKind` derives `Deserialize`,
  but its accepted string form (variant names / snake-case) is not
  guaranteed to match the human-readable display strings the product
  spec promises (`"Dark City"`, `"Solarized Dark"`). Going through
  `String` decouples the YAML surface from the enum's internal
  representation.
- **Field-level fallback (Oz concern 2).** A direct `ThemeKind`
  deserialization on an unknown string would fail the entire YAML load
  and reject every tab in the file. With `Option<String>` the load
  always succeeds; resolution and fallback happen at apply time per the
  product spec's behavior #11.

The `From<WindowSnapshot> for WindowTemplate` (lines 43–70) and
`TryFrom<TabSnapshot> for TabTemplate` (lines 189–200) impls are updated
to copy *manual* overrides out of the snapshot. Directory-matched
overrides are not emitted into saved launch configurations (per product
spec behavior #10).

Round-trip and serde tests live in
`launch_configs/launch_config_tests.rs` (referenced by the `#[path]`
attribute on line 11) and gain coverage in §6.

### 2. Theme-name resolver

A new helper, alongside `ThemeKind` in `app/src/themes/theme.rs`:

```rust
/// Resolve a free-form theme reference (from a launch configuration or
/// from `directory_overrides`) to a `ThemeKind`.
///
/// Accepts:
///   * Display strings: "Dark City", "Solarized Dark", "Dracula"
///   * Snake-case strings: "dark_city", "solarized_dark", "dracula"
///   * Custom theme names registered via the theme loader
///
/// Matching is case-insensitive on whitespace-stripped input.
/// Returns `None` for unknown names; callers log a warning and fall
/// through to the next resolution layer.
pub fn resolve_theme_ref(raw: &str) -> Option<ThemeKind> { ... }
```

Implementation:

1. Trim and case-fold the input.
2. Walk built-in `ThemeKind` variants; return the first whose `Display`
   string or snake-case form matches.
3. Consult the custom-theme registry the existing loader maintains
   (the same registry that powers the theme picker).
4. Return `None` on no match.

This satisfies Oz concerns 1 and 2 by making YAML-format coupling
explicit and keeping unknown-theme handling field-level.

### 3. `ThemeOverride` source enum on `TabSnapshot` and `TabData`

File: `app/src/app_state.rs` and `app/src/tab.rs`.

Add a new type:

```rust
/// The source of a tab's theme override. The enum exists so "Reset
/// theme" can clear a manual override without also dismissing a
/// directory match that the tab would otherwise still receive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeOverride {
    /// Set explicitly: launch-configuration tab-level `theme:`,
    /// "Pin theme" menu, or saved-and-restored manual pin.
    Manual(ThemeKind),
    /// Set automatically by `directory_overrides` matching against the
    /// focused pane's cwd. Not persisted across sessions; recomputed on
    /// startup from the current settings + cwd.
    Cwd(ThemeKind),
}
```

Add `pub theme_override: Option<ThemeOverride>` to:

- `TabSnapshot` (`app_state.rs:61–69`), placed next to `selected_color`.
- `TabData` (`tab.rs:134`), same.

Snapshot ↔ runtime conversion copies the field unchanged. Session
serialization persists only `Manual(_)` overrides; `Cwd(_)` overrides are
re-derived on startup (per product spec behavior #13).

`WindowSnapshot` does not gain a theme field — window-level launch-config
theme is expanded to per-tab `Manual` overrides at open time and not
persisted at the window level.

### 4. Directory-pattern overrides settings group

A new settings group, defined in a new file
`app/src/settings/directory_overrides.rs` and exported from
`app/src/settings/mod.rs`:

```rust
define_settings_group!(DirectoryThemeOverrides, settings: [
    overrides: Map {
        // Stored as TOML table: keys = directory paths, values = theme refs.
        type: BTreeMap<String, String>,
        default: BTreeMap::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.themes.directory_overrides",
        max_table_depth: 1,
        description: "Map of directory paths to theme names. The active \
                      pane's cwd is matched against keys (longest prefix \
                      wins); the matched theme overrides the global theme \
                      for that tab.",
    },
]);
```

Resolution helper, alongside the group:

```rust
/// Returns the theme to use for `cwd`, walking the directory-overrides
/// map by longest-prefix match. Returns `None` when no key matches.
/// Tilde expansion and trailing-slash normalization happen here.
pub fn directory_theme_for(
    overrides: &BTreeMap<String, String>,
    cwd: &Path,
) -> Option<ThemeKind> { ... }
```

Match semantics (per product spec #2, #3):

- Keys are tilde-expanded once at evaluation time.
- A key matches a cwd if it is a prefix at a path-component boundary
  (use `Path::starts_with` after normalization, not `str::starts_with`).
- The longest matching key wins.
- Each value is resolved via `resolve_theme_ref`; unresolved values are
  skipped and a warning is logged once at settings-change time, not
  per-match.

Matching is invoked from two places:

- **Tab creation** — when a tab is added (from a launch configuration or
  from "new tab"), if the tab has no `Manual` override the focused
  pane's cwd is matched. On hit, the tab's `theme_override` is set to
  `Cwd(...)`.
- **Cwd-change events** — the existing pane-cwd tracker emits a
  "focused pane cwd changed" event used for tab title updates. Add a
  subscriber that re-runs directory matching for the affected tab when
  the new cwd differs from the previous match key. The runtime tab
  retains the last-matched key so re-matching is `O(1)` in the no-change
  case.

### 5. Renderer: `Appearance` gains a theme catalog

This addresses Oz concern 3 (`Appearance::theme_for` cannot return
`&WarpTheme` for arbitrary overrides because `Appearance` only owns the
global `WarpTheme` today).

`Appearance` is extended in `warp_core::ui::appearance`:

```rust
pub struct Appearance {
    // existing fields...

    /// Currently-active global theme. Always present in the catalog
    /// below, but tracked separately so `theme()` is O(1).
    global_theme: Arc<WarpTheme>,

    /// Lazy cache of all themes referenced by per-tab overrides.
    /// Keyed by `ThemeKind` so custom themes get their own entries.
    /// Wrapped in `RwLock` because lookup is on the hot rendering path
    /// while population happens on the (much rarer) settings-change path.
    theme_cache: parking_lot::RwLock<HashMap<ThemeKind, Arc<WarpTheme>>>,
}

impl Appearance {
    /// Active global theme. Unchanged contract.
    pub fn theme(&self) -> Arc<WarpTheme> {
        self.global_theme.clone()
    }

    /// Per-tab theme lookup. `override` is taken straight from
    /// `TabData::theme_override.as_ref().map(ThemeOverride::kind)`.
    /// Returns the global theme when `override` is `None`.
    pub fn theme_for(&self, override_kind: Option<&ThemeKind>) -> Arc<WarpTheme> {
        match override_kind {
            None => self.global_theme.clone(),
            Some(kind) if *kind == self.global_theme.kind() => {
                self.global_theme.clone()
            }
            Some(kind) => {
                if let Some(t) = self.theme_cache.read().get(kind).cloned() {
                    return t;
                }
                self.load_and_cache(kind)
            }
        }
    }

    fn load_and_cache(&self, kind: &ThemeKind) -> Arc<WarpTheme> {
        // Load via the same path the global theme loader uses.
        // On failure, log warning and return self.global_theme.clone()
        // (callers see a fall-through to global theme; per behavior #11).
    }
}
```

Cache-invalidation rules:

- `AppearanceManager` already subscribes to `ThemeSettings` change events
  (`app/src/appearance.rs:51`). On every change it (a) updates
  `global_theme` and (b) calls a new `theme_cache.clear()` so any
  previously-cached entries are reloaded next time they are looked up.
  The hit-rate cost is bounded by the number of distinct overridden
  themes open at once (typically ≤ 5).
- Custom-theme file changes (which already invalidate the global theme
  via the existing watcher) trigger the same cache clear.

Renderer call-site update: every place that today reads
`Appearance::as_ref(ctx).theme()` and is **inside a tab's render path**
becomes `Appearance::as_ref(ctx).theme_for(tab.theme_override.as_ref().map(ThemeOverride::kind))`.
The exact list of call sites (terminal cell renderer in
`app/src/terminal/view.rs`, color derivation in
`app/src/terminal/color.rs`, any block-styling consumers) is enumerated
during implementation; the change is mechanical at each site.

Window-chrome consumers (title bar, sidebar, settings views, tab strip)
continue to call `Appearance::theme()` with no override argument, per
product spec #15.

### 6. Right-click menu: Pin theme / Reset theme

The existing tab context menu gains two entries:

- **Pin theme...** — opens a submenu listing built-in themes plus any
  loaded custom themes (the same list the theme picker shows). On
  click, sets the tab's `theme_override` to `Manual(chosen)`. Always
  visible.
- **Reset theme** — clears any `Manual(_)` override on the tab; if the
  focused pane's cwd matches a `directory_overrides` key the override
  is set to `Cwd(_)` immediately, otherwise it becomes `None`. Visible
  only when the tab has a `Manual` override.

Both entries trigger the existing theme-changed redraw path used today
when the global theme changes (no new render-invalidation work).

Telemetry: one counter per click, using the existing tab-menu telemetry
conventions. No new event schema.

## Testing and validation

### Unit tests

`app/src/themes/theme.rs` (or sibling test module):

- `resolve_theme_ref("Dark City")` → `Some(ThemeKind::DarkCity)`.
- `resolve_theme_ref("dark_city")` → `Some(ThemeKind::DarkCity)`.
- `resolve_theme_ref("  DARK CITY  ")` → `Some(ThemeKind::DarkCity)`
  (case-insensitive, whitespace-tolerant).
- `resolve_theme_ref("My Custom Theme")` → `Some(ThemeKind::Custom(...))`
  when registered; `None` otherwise.
- `resolve_theme_ref("Definitely Not A Theme")` → `None`.

`app/src/launch_configs/launch_config_tests.rs`:

- YAML round-trip of `TabTemplate { theme: Some("Solarized Dark") }`.
- A YAML file with one valid and one unknown `theme:` value loads
  successfully; resolution applied at apply time, only the unknown tab
  falls through, the file is **not** rejected.
- `WindowTemplate.theme` set with all tabs un-themed: round-trip
  preserves the window-level value.
- Saving a snapshot whose tabs have only `Cwd(_)` overrides emits no
  `theme:` fields in the resulting YAML.

`app/src/settings/directory_overrides.rs` (new):

- `directory_theme_for(map, "~/Work/medone/apps/admin-api")` returns
  the theme mapped to `"~/Work/medone"`.
- `directory_theme_for(map, "~/Work/medone-archive")` returns `None`
  when only `"~/Work/medone"` is mapped (component-boundary match).
- Two overlapping keys `"~/Work"` and `"~/Work/medone"`: longer wins
  for paths under `medone`, shorter wins elsewhere under `~/Work`.
- Unresolved theme value: that entry is skipped, `directory_theme_for`
  on a path that would have matched it returns `None` (or the next
  match), and one warning is logged.

`app/src/app_state.rs` (or test module):

- `TabSnapshot { theme_override: Some(Manual(Dracula)) }` round-trips
  through session-restore serialization.
- `TabSnapshot { theme_override: Some(Cwd(Dracula)) }` does **not**
  persist — the Cwd variant is re-derived on startup from current
  settings + cwd. (This pins behavior #13.)
- `TabSnapshot::color()` returns the same value with and without
  `theme_override` set (color and theme are independent — #19).

`app/src/appearance.rs` (or `warp_core` tests):

- `Appearance::theme_for(None)` returns the same `Arc` as
  `Appearance::theme()`.
- `Appearance::theme_for(Some(&ThemeKind::Light))` returns a `WarpTheme`
  whose kind is `Light` regardless of the active global theme.
- `Appearance::theme_for(Some(&ThemeKind::Custom(missing)))` falls back
  to the global theme and increments the existing missing-custom-theme
  warning counter.
- After `ThemeSettings` changes, the cache is cleared: a subsequent
  `theme_for(Some(&kind))` reloads from disk rather than returning
  stale custom-theme content.

### Integration tests

`crates/integration/`:

- Open a launch configuration with three tabs — `theme: "Dracula"`,
  `theme: "Solarized Light"`, no `theme:`. Assert each tab renders the
  expected theme and that switching tabs does not redraw inactive tabs.
- Configure `[appearance.themes.directory_overrides]` with two entries.
  Open one tab in each matched directory and one in an unmatched
  directory. Assert each tab renders the expected theme. `cd` the
  unmatched tab into a matched directory; assert the tab redraws with
  the matched theme. `cd` it back out; assert it redraws with the
  global theme.
- Tab with `Manual(Dracula)` from a launch config sits in a directory
  that maps to `Solarized Dark`. Assert Dracula wins (manual > cwd).
  Right-click → Reset theme. Assert the tab redraws with Solarized
  Dark (the cwd match takes over).
- Edit `directory_overrides` while Warp is running. Assert all tabs
  whose effective theme changes redraw, others do not.
- Quit and relaunch. Assert manually-pinned tabs restore their pin;
  cwd-matched tabs re-derive their theme from current settings + cwd.
- Unknown theme name in a launch configuration: the file opens,
  exactly that one tab falls through, the warning appears in the log,
  other tabs render correctly.

### Manual verification

- macOS, Linux, Windows: visually confirm terminal background and ANSI
  palette match the expected theme for each tab in the test scenarios.
- Confirm window chrome (title bar, sidebar, tab strip) follows the
  global theme in all scenarios.
- Toggle system light/dark while a mix of themed/unthemed tabs is open;
  confirm only unthemed tabs follow the system.
- Save layout as launch configuration; confirm tabs with manual pins
  emit `theme:`, tabs themed only by directory matching do not.
- After implementation, invoke the `verify-ui-change-in-cloud` skill
  per the repository rule for user-facing client changes.

### Tooling

- `cargo fmt` and
  `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings`
  must pass.
- `cargo nextest run` for unit tests; `crates/integration` for
  end-to-end.
- `./script/presubmit` before pushing.

## Risks and mitigations

- **Render hot-path cost of the per-tab lookup.** The added work per
  frame is one `Option` deref, one `Arc` clone in the common case (no
  override or override == global), or one `RwLock::read` + `HashMap`
  hit when the override resolves to a non-global cached theme. The
  existing per-tab indicator color path runs on the same frame and has
  not been a bottleneck. If profiling flags it post-merge, cache the
  `Arc<WarpTheme>` directly on `TabData` and invalidate on
  override-change; the change is local.

- **Cache stampedes when the user changes the global theme with many
  overridden tabs open.** `theme_cache.clear()` followed by lazy
  reload means each unique overridden theme reloads once. Bounded by
  open-tab cardinality.

- **Mismatch between `directory_overrides` keys and how the OS reports
  paths.** Tilde expansion happens in the resolver; symlinks are not
  followed (matching whatever the shell's `pwd` reports). This is
  documented in the product spec; integration tests cover the common
  cases.

- **`Cwd(_)` override mistakenly persists across sessions.** Session
  serialization explicitly emits only `Manual(_)`; the integration test
  for restart behavior pins this.

- **Concern 1 / Concern 2 / Concern 3 from Oz's first review.**
  Addressed in §1 (Option<String> + apply-time resolver), §2
  (`resolve_theme_ref` returns `Option<ThemeKind>`), and §5 (theme
  catalog, `Arc<WarpTheme>` ownership) respectively. Each concern has
  a corresponding unit test above.

- **Per-pane theme creep.** The override field lives on the tab, not
  the pane, and the resolver consults the focused pane's cwd. A future
  per-pane theming feature would need its own data path; this spec
  does not lock that out but does not pre-pay for it either.

- **Persistence schema collision with future per-window theming.** The
  field is named `theme_override` (not `theme`) so a separate resolved
  theme can live elsewhere if added later.

## Follow-ups (deliberately not in this PR)

- **Glob pattern support in `directory_overrides`** (open question in
  product.md) — extends key matching from prefix to glob.
- **Auto-theme by SSH host or hostname** (`stevenchanin`, `pyronaur`,
  `zethon`, `janderegg` in #478). Requires detection inside the tab's
  shell; consumes the `Manual` / new `Ssh` variant of `ThemeOverride`.
- **Runtime escape-code or shell-hook protocol for setting a tab's
  theme** (`yatharth`, for Claude-Code session signaling). Defines a
  wire format and a security model; consumes the override field.
- **In-tab "Pin theme" command in the command palette** (so users can
  pin without right-clicking). Strictly additive.
- **Wallpaper-per-tab** (`scottaw66`, `SheepDomination`). Different
  surface area entirely; does not conflict.
- **Closing #2618 as a duplicate** of #478 once this spec lands.
