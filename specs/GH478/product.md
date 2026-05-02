# PRODUCT.md — Per-tab theme override via launch configurations

Issue: https://github.com/warpdotdev/warp/issues/478
Related: https://github.com/warpdotdev/warp/issues/2618 (set warp theme in launch configuration)

## Summary

Today the Warp theme is a single global value (`appearance.themes.theme` in
`settings.toml`). Switching themes affects every open window and tab at once.
Users who keep many tabs open across distinct contexts — different projects,
local vs. remote machines, production vs. development environments — have asked
for years (`#478`, 55+ upvotes; `#2618`) for a way to give specific tabs a
different theme so context is visible at a glance.

This spec covers a focused first cut of that capability: a tab can carry a
**theme override** that comes from the launch configuration that opened it (or
from a tab restored from a previous session). The override scopes the theme to
that single tab; the global theme setting is unchanged and continues to apply
to every tab that does not have an override. A window-level default override is
also defined so a launch configuration can theme all of its tabs at once.

This spec deliberately scopes out automatic theme switching (SSH host
detection, agent conversation type, escape-code-driven runtime changes). Those
appear in `#478` discussion and warrant their own product spec once the
override mechanism this spec defines exists to build on.

Figma: none provided.

## Goals / Non-goals

In-scope surfaces:

- The launch configuration YAML schema (`~/.warp/launch_configurations/*.yaml`)
  gains an optional theme field at the tab level and at the window level. The
  values are theme identifiers — the same names users already see in the theme
  picker (`"Dark"`, `"Solarized Dark"`, `"Dracula"`, `"Dark City"`, etc.) and
  the same custom-theme references the rest of the theme system already
  accepts.
- Tabs opened from a launch configuration that specifies a theme render with
  that theme: terminal background, foreground, ANSI palette, and any
  theme-derived UI surfaces inside the tab follow the override.
- The override persists with the tab so that on restart / session restore the
  tab continues to render with the same theme without depending on the user
  re-running the launch configuration.
- A user can clear a tab-level override (returning the tab to the global theme)
  through the same right-click tab menu that already exposes per-tab
  attributes.

Out of scope (explicitly **not** part of this spec):

- Automatic theming based on SSH host, hostname, working directory, or
  `whoami` (raised in `#478` comments). These require separate detection and
  policy mechanisms and are tracked as follow-ups.
- A new escape-code or shell-side protocol for setting a tab's theme at
  runtime (raised in `#478` comments by users wanting hooks to color Claude
  Code sessions). Once a per-tab override field exists internally a runtime
  setter is a small follow-up; defining the protocol is its own surface.
- Per-pane theming. Panes inside a tab continue to share one theme; the
  override is a tab-level concept.
- Changes to the global theme storage path
  (`appearance.themes.theme` in `settings.toml`), the theme picker UI, or
  custom-theme loading. The override reuses the existing theme identifier type
  unchanged.
- Onboarding or settings-page surfacing of the new field. Discoverability lives
  in the launch configuration docs and the right-click tab menu only.

## Behavior

1. A launch configuration YAML file may include a `theme:` field on any tab
   entry. The accepted value is a theme identifier exactly as it appears in
   `settings.toml` today (e.g. `theme: "Dark City"`, `theme: "Solarized Dark"`,
   `theme: "Dracula"`). Custom theme references use the same form the global
   theme setting accepts. Omitting the field leaves the tab using whatever
   theme would otherwise apply (per behavior #3).

2. A launch configuration YAML file may include a `theme:` field on any window
   entry. When present, every tab in that window with no tab-level `theme:`
   inherits this window-level value. A tab-level `theme:` always wins over a
   window-level one.

3. Theme resolution for a tab is, in order: (a) the tab's own override, if
   any; (b) the window-level override of the window that opened the tab, if
   any and inherited at open time; (c) the global theme as derived from
   `ThemeSettings` and the system theme, exactly as today. If none of the
   override sources apply the tab's behavior is bit-for-bit identical to a
   tab opened without this feature.

4. When a tab has an override the override applies to: the terminal cell
   foreground/background, the ANSI 16-color palette used by the terminal grid,
   and any in-tab UI surfaces whose colors are derived from the active theme
   (block backgrounds, command output styling, accent colors). The window
   chrome (title bar, sidebar, settings views, the tab strip itself) continues
   to follow the global theme so that windows holding mixed-theme tabs remain
   visually coherent at the window level.

5. Switching tabs is instant: there is no flash, no progressive paint, and no
   redraw of unrelated tabs. The previously-active tab's content does not
   change appearance because of the switch — only the rendering of the
   newly-active tab reflects its (possibly different) theme. A user
   alt-tabbing through ten tabs with different overrides sees each tab in its
   own theme as it becomes active.

6. Changing the global theme (via the theme picker, settings page, or system
   light/dark switch) updates every tab that does **not** have an override.
   Tabs with overrides are unaffected by the global change; their rendering
   stays on the override value.

7. The existing per-tab `color:` field on a tab template (the small colored
   indicator next to the tab title) is independent of the new `theme:` field.
   Both can be set on the same tab; both are honored. Neither implies the
   other.

8. A tab opened with a theme override surfaces a "Reset theme" entry in the
   right-click tab context menu, alongside the existing per-tab attributes
   (color, title, etc.). Choosing "Reset theme" clears the tab's override; the
   tab immediately re-renders using whatever theme falls out of the resolution
   order in #3 (typically the global theme). The menu entry is hidden for
   tabs that have no override.

9. Tab overrides survive session restore. A tab that had an override when the
   user last quit Warp opens with the same override on relaunch. A tab with
   no override opens with no override (i.e. it tracks the current global
   theme).

10. Saving a window's current state to a launch configuration (`File → Save
    Layout as Launch Configuration` and equivalents) emits `theme:` entries on
    each tab that has an override. Tabs without overrides emit no `theme:`
    field. Window-level `theme:` is emitted only when every tab in the window
    shared the same explicit override at save time, in which case the per-tab
    fields are coalesced up to the window. (This keeps round-trip save/load
    clean for the common "themed launch config" case.)

11. An unknown theme identifier in a launch configuration (e.g. a theme that
    has been deleted, or a typo) is treated the same way an unknown global
    theme is treated today: the tab falls back to the global resolved theme,
    a warning is logged, and opening the launch configuration does not fail.
    Other tabs in the same launch configuration are unaffected.

12. Custom themes referenced by an override behave identically to custom
    themes used as the global theme — they are loaded from the user's themes
    directory, fail-soft to the global theme if the file is missing, and
    obey the same trust/validation rules already in the theme loader.

13. The override persists per-tab, not per-launch-configuration. If a user
    edits a launch configuration's theme after first opening it, already-open
    tabs that came from that launch configuration keep their original
    override; new tabs opened by re-running the launch configuration get the
    new value. This matches how the existing per-tab `color:` field behaves.

14. Accessibility: the theme override does not change any text content,
    accessible labels, or focus order. Screen readers continue to report tab
    titles and contents identically. The "Reset theme" menu entry has the
    accessible label "Reset theme" and is announced as a menu item.

15. The feature applies on every supported platform (macOS, Linux, Windows).
    It is gated behind no feature flag and is on by default once shipped.

## User-visible failure modes

- **Unknown theme name in YAML** — tab opens with the global theme; a one-line
  warning is written to the Warp log identifying the launch configuration
  filename, the tab title (or index, if untitled), and the unrecognized theme
  string. The launch configuration as a whole still opens.
- **Custom theme file missing** — same fallback as the global theme exhibits
  today: tab opens with the global theme, warning logged.
- **Window-level theme set, tab-level not set, tab opens with override
  inherited; user later edits the YAML to remove the window-level value** —
  already-open tabs keep their existing override (per #13). New tabs opened
  from the edited launch configuration get no override.

## Open questions

- Should the right-click "Reset theme" entry also offer a submenu of theme
  names so a user can change a tab's override without editing YAML? This spec
  takes the more conservative position of "reset only" so the launch
  configuration file remains the single source of truth for setting overrides;
  a richer in-app picker is a candidate follow-up.
- Should saving a launch configuration record the *resolved* theme of each
  tab, or only the explicit override the tab carries? This spec records only
  explicit overrides (#10) so a saved launch configuration that contains no
  `theme:` fields continues to track the global theme on every machine it is
  loaded on. The alternative (record resolved theme always) would lock the
  saved configuration to a snapshot of the user's current global theme, which
  is rarely the desired behavior for a portable launch configuration.
