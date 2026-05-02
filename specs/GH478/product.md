# PRODUCT.md — Per-tab theme overrides driven by directory and launch configurations

Issue: https://github.com/warpdotdev/warp/issues/478
Related: https://github.com/warpdotdev/warp/issues/2618 (set warp theme in launch configuration)

## Summary

The Warp theme is a single global value today (`appearance.themes.theme` in
`settings.toml`); switching it affects every open tab at once. Users have asked
for years (`#478`, 55+ upvotes; `#2618`) for tabs to render with different
themes when they represent different contexts — different projects, local vs.
remote machines, production vs. development.

This spec covers a focused first cut of per-tab theme overrides driven by
**three** sources, in priority order:

1. A user-visible **manual** override on a tab (set via the launch
   configuration YAML or via a right-click menu).
2. A **directory-pattern** auto-match: the user maps directory paths to
   themes in `settings.toml`, and tabs whose active pane's cwd matches a
   pattern render with the mapped theme. This is the path most users in the
   issue thread describe (`pyronaur`, `janderegg`, `milopersic`): "I `cd`
   into project A, my theme should change."
3. A **launch-configuration window-level default** that themes every tab a
   given launch configuration opens unless the tab itself has another
   override.

The global theme remains the fallback when none of the three sources apply.
Window chrome (title bar, sidebar, settings views, the tab strip) continues
to follow the global theme so windows holding mixed-theme tabs remain
visually coherent at the window level.

This spec deliberately scopes out automatic theming triggered by SSH host,
hostname, runtime escape codes, or shell hooks. Those appear in `#478`
discussion and are listed as follow-ups that consume the override field this
spec introduces.

Figma: none provided.

## Goals / Non-goals

In-scope surfaces:

- A new settings map `appearance.themes.directory_overrides` whose keys are
  directory paths (tilde-expanded) and whose values are theme identifiers.
  Matching uses longest-prefix-wins.
- The launch-configuration YAML schema gains an optional `theme:` field at
  the tab level and at the window level.
- A persisted per-tab override that survives session restore.
- A right-click tab menu entry for **Pin theme** (manually override the
  active tab's theme to a chosen theme) and **Reset theme** (clear a
  manual override; cwd-pattern matching may then reapply).

Out of scope:

- SSH-host-driven, hostname-driven, or `whoami`-driven theming
  (`stevenchanin`, `pyronaur`, `zethon`, `janderegg`).
- Escape-code or shell-hook protocols for runtime theme switching
  (`yatharth`, for Claude-Code session signaling).
- Per-pane theming. Panes inside a tab continue to share one theme.
- Per-tab wallpaper or graphics (`scottaw66`, `SheepDomination`).
- Changes to the global theme storage path
  (`appearance.themes.theme`), the theme picker UI, or custom-theme loading.
  Overrides reuse the existing theme identifier type.

## Resolution order

A tab's effective theme is determined by walking these layers and returning
the first hit:

1. **Manual override**, if any. Sources: a tab-level `theme:` in a launch
   configuration; the right-click "Pin theme" menu.
2. **Directory match**, if any. The active pane's current working directory
   is matched against `appearance.themes.directory_overrides`; if a key is
   a prefix of the cwd, the longest such key wins.
3. **Launch-configuration window-level default**, if the tab was opened from
   a launch configuration with a window-level `theme:` and no closer
   override applies.
4. **Global theme** as derived from `ThemeSettings` and the system theme,
   exactly as today.

If none of the override sources resolve to a known theme, behavior is
bit-for-bit identical to today.

## Behavior

### Directory-pattern overrides

1. `settings.toml` accepts a new section
   `[appearance.themes.directory_overrides]` whose entries map a directory
   path to a theme identifier. Example:

   ```toml
   [appearance.themes.directory_overrides]
   "~/Work/medone"   = "Dark City"
   "~/Work/bondwise" = "Solarized Dark"
   "~/Work/checkpt"  = "Dracula"
   ```

   Theme values are the same string form accepted by the global
   `appearance.themes.theme` (the parser tolerates both display names like
   `"Dark City"` and snake-case like `"dark_city"`; see #14 below).

2. Keys are tilde-expanded to absolute paths at match time. Trailing slashes
   are normalized away. Symlinks are not resolved — the cwd as the shell
   reports it is what's matched.

3. Match resolution: a key matches if it is a prefix of the active pane's
   cwd at a path-component boundary. `~/Work/medone` matches both
   `~/Work/medone` and `~/Work/medone/apps/admin-api`, but does **not**
   match `~/Work/medone-archive` (no component boundary). When multiple
   keys match, the longest one wins (most specific).

4. The cwd evaluated for a tab is the cwd of the **focused pane** in that
   tab. A tab whose focused pane is in a non-shell context (notebook,
   settings view, etc.) has no cwd and falls through directory matching.

5. When a tab's active pane changes cwd (because the user ran `cd`, opened
   a subdirectory, or moved focus to a pane in a different cwd), directory
   matching re-runs. If the new cwd matches a different key, the tab
   immediately re-renders with the new theme. If it matches no key, the
   tab falls through to the next layer in the resolution order.

6. Adding, editing, or removing entries in `directory_overrides` while
   Warp is running re-evaluates every open tab. Tabs whose effective theme
   changes redraw; tabs whose effective theme is unchanged do not.

7. A theme name in `directory_overrides` that does not resolve to a known
   theme is treated the same way an unknown launch-configuration theme is
   (#11): a warning is logged identifying the offending key, the entry is
   skipped for matching purposes, and the rest of the map continues to
   work.

7a. **`directory_overrides` is stored locally and never synced to Warp's
    cloud.** Directory paths can encode employer, customer, and project
    names (`~/Work/<client>/<engagement>/...`); cloud-syncing the keys
    would push that organizational context off-machine. Users on
    multiple machines who want shared themes today set them per-machine.
    An opt-in cloud-sync mode is a candidate follow-up. The global
    theme setting (`appearance.themes.theme`) and per-tab pins set via
    the right-click menu remain user-controllable surfaces; only this
    map is local-only.

### Launch-configuration overrides

8. A launch configuration YAML may include `theme:` on any tab entry.
   Accepted values are theme identifiers in the form documented in #14.
   Omitting the field leaves the tab to fall through the resolution order.

9. A launch configuration YAML may include `theme:` on any window entry.
   Tabs in that window with no tab-level `theme:` and no directory match
   inherit this window-level value (per the resolution order: #3 sits
   below #2).

10. Saving a window's current state to a launch configuration emits
    `theme:` entries on each tab whose effective theme came from a manual
    override. Tabs themed only by directory matching emit no `theme:`
    field — directory matching is config-level, not tab-level, so a saved
    launch configuration that relies on it stays portable. Window-level
    `theme:` is emitted only when every tab in the window shared the same
    explicit manual override at save time.

### YAML / settings format

11. An unknown theme identifier — anywhere it appears — never causes a
    file-level load failure. The deserializer accepts any string; the
    resolver runs at apply time and falls back to the next resolution
    layer for the affected entry only. Other tabs in the same launch
    configuration, and other entries in the same `directory_overrides`
    map, are unaffected. Each unknown name produces exactly one logged
    warning per load.

12. Custom themes referenced by an override behave identically to custom
    themes used as the global theme — loaded from the user's themes
    directory, fail-soft to the next layer if the file is missing, same
    trust/validation rules as the global theme loader.

13. Overrides persist per-tab through session restore, by source:
    - **Manual pin** — kept on relaunch.
    - **Launch-configuration window-level default** — kept on relaunch.
      The launch configuration that opened the tab is not necessarily
      reopened on restore, so the value travels with the tab.
    - **Directory match** — not stored; recomputed on relaunch from the
      current `directory_overrides` and the restored cwd. Editing
      `directory_overrides` between sessions therefore takes effect on
      the next launch.

14. The accepted form for any theme reference (in
    `directory_overrides`, in launch-config tab `theme:`, in launch-config
    window `theme:`) is a single string. Both the human-readable display
    form (`"Dark City"`, `"Solarized Dark"`, `"Dracula"`) and the
    snake-case form (`"dark_city"`, `"solarized_dark"`, `"dracula"`) are
    accepted. Matching is case-insensitive on whitespace-stripped input.
    Custom themes are referenced by their custom-theme name, same as
    today's global setting.

### Rendering scope

15. When a tab has an effective override (from any layer), the override
    applies to: the terminal cell foreground/background, the ANSI 16-color
    palette used by the terminal grid, and any in-tab UI surfaces whose
    colors are derived from the active theme (block backgrounds, command
    output styling, accent colors). The window chrome (title bar, sidebar,
    settings views, the tab strip itself) continues to follow the global
    theme.

16. Switching tabs is instant — no flash, no progressive paint. Only the
    rendering of the newly-active tab reflects its (possibly different)
    theme; inactive tabs do not redraw on switch.

17. Changing the global theme updates every tab whose effective theme
    falls through to the global layer. Tabs with overrides at any
    higher-priority layer are unaffected.

### User affordances

18. The right-click tab context menu gains two entries, alongside the
    existing per-tab attributes:
    - **Pin theme...** — opens a submenu listing available themes.
      Choosing one sets a manual override on the tab, which wins over
      directory matching and the global theme. Visible at all times.
    - **Reset theme** — clears the tab's manual override. Visible only
      when the tab has a manual override. After reset, the tab falls
      through to the directory-match / window-default / global layers in
      that order; if a directory match applies, the tab redraws with the
      directory-matched theme.

19. The existing per-tab `color:` field on a tab template (the small
    colored indicator next to the tab title) is independent of the new
    theme override. Both can be set; both are honored.

20. The feature applies on every supported platform (macOS, Linux,
    Windows). It is gated behind no feature flag and is on by default once
    shipped. An empty `directory_overrides` map (the default) plus no
    launch-config theme fields plus no pinned themes is bit-for-bit
    identical to current behavior.

### Accessibility

21. The override does not change any text content, accessible labels, or
    focus order. Screen readers continue to report tab titles and
    contents identically. The new menu entries have accessible labels
    "Pin theme" (with a submenu) and "Reset theme".

## User-visible failure modes

- **Unknown theme name** — anywhere it appears, the entry is skipped at
  apply time, a one-line warning is written to the Warp log identifying
  the source (launch configuration filename + tab title or index;
  `directory_overrides` key), and the rest of the configuration loads
  normally.
- **Custom theme file missing** — same fallback the global theme uses
  today: tab opens with the next-layer theme, warning logged.
- **Two `directory_overrides` keys are equivalent after tilde expansion**
  — last-write-wins per TOML semantics; a warning is logged identifying
  the duplicate.
- **A pane's cwd is unavailable** (non-shell pane content) — that pane
  contributes no cwd to directory matching; if it is the focused pane the
  tab falls through to the window/global layers.

## Open questions

- Should `directory_overrides` keys support glob patterns (`~/Work/*`)?
  This spec proposes prefix-matching only because that covers the
  most-requested use case ("a folder of projects, each in a subfolder")
  and avoids the edge cases of glob semantics on Windows paths. Globs are
  a candidate follow-up.
- Should "Reset theme" also clear a launch-config-set manual override
  (#18 says it does)? An alternative is "Reset theme" only clears
  pin-from-menu overrides and a separate "Forget launch config theme"
  entry handles YAML-set overrides. This spec takes the simpler
  one-entry-clears-all-manual position and lists the alternative as a
  follow-up.
- Should saving a launch configuration emit `directory_overrides`
  entries (so themes travel with the launch config)? This spec keeps the
  two surfaces independent: launch configs carry only manual overrides;
  directory matching is a settings-level concept that does not roundtrip
  through saved launch configs. A future "shareable theme bundle" feature
  could compose them.
