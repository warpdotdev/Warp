---
name: 01 — Tab color shortcuts
status: draft
---

# Tab color shortcuts — PRODUCT

## Summary

Keyboard shortcuts that recolor the currently-active tab so users can visually distinguish workflows at a glance ("red is prod, green is local"). Pressing one of the bound shortcuts assigns one of eight ANSI colors to the active tab; one shortcut clears the color. The colored state persists across restarts. Scope is limited to the keystroke → color binding for the eight ANSI colors plus a clear; configurability and custom-color UI are out of scope.

## Goals / Non-goals

**Goals**
- Provide a one-keystroke way to mark the active tab with one of eight visually distinct colors, plus a clear shortcut.
- Reuse twarp's existing tab-color visual treatment (the surface already used by the right-click "Set color" menu) so the keyboard path produces the same on-screen result.
- Persist a tab's color across app restarts.
- Surface each color's bound shortcut directly on the colored tab via a hover tooltip, so users discover the shortcut without opening a menu.

**Non-goals (deferred to follow-ups)**
- User-configurable shortcut bindings (rebinding via the existing keybindings settings page works; no new UI for it).
- A color picker UI or custom hex/RGB colors.
- More than eight colors, or extending the palette beyond `AnsiColorIdentifier`. The README §2 lists a different eight (Red/Orange/Yellow/Green/Blue/Purple/Pink/Gray) marked as "tentative defaults"; this feature uses the existing ANSI palette to keep scope small. Palette extension is a separate, larger change.
- Rules that auto-color tabs (by directory, command, host, etc.). The existing directory→color default that ships upstream continues to work but is not modified by this feature.
- Adding shortcut hints to the right-click "Set color" menu. Discoverability lives on the tab indicator (see §17) instead of inside the menu.

## Behavior

1. While twarp is the focused application, pressing one of the following key combinations sets the **active tab's** color to the listed color and leaves all other tabs unchanged:

   | Shortcut | Color   |
   |----------|---------|
   | ⌘⌥1      | Red     |
   | ⌘⌥2      | Yellow  |
   | ⌘⌥3      | Green   |
   | ⌘⌥4      | Cyan    |
   | ⌘⌥5      | Blue    |
   | ⌘⌥6      | Magenta |
   | ⌘⌥7      | White   |
   | ⌘⌥8      | Black   |
   | ⌘⌥0      | *Reset (clear color)* |

   Color names map to the existing `AnsiColorIdentifier` enum used by the rest of the tab-color subsystem, so the rendered hue is theme-aware (light vs. dark themes apply their own ANSI palette) and matches the color a user would get by picking that color from the existing tab context menu.

2. "Active tab" means the single tab that currently owns workspace focus in the focused window. Tabs in other windows, and inactive tabs in the focused window, are never affected by a shortcut press.

3. The eight color shortcuts (⌘⌥1–⌘⌥8) **unconditionally set** the active tab to the named color. Pressing the shortcut for the color a tab already has is a no-op from the user's perspective: no flicker, no animation, no telemetry beyond a re-assert of the same value. This differs from the right-click menu's toggle-off behavior — the dedicated reset shortcut (⌘⌥0) is the only way to clear a color via keyboard, which keeps each color shortcut predictable.

4. ⌘⌥0 ("Reset") clears any user-assigned color from the active tab. After reset:
   - If the tab has a directory-driven color (via the existing automatic-coloring feature), that color becomes visible.
   - Otherwise the tab returns to the default uncolored appearance.
   - Pressing ⌘⌥0 on an already-uncolored tab is a no-op (no error, no visible change).

5. Pressing a color shortcut on an active tab that already has a *different* color replaces the color in place. There is no intermediate uncolored frame; the tab transitions directly from the old color to the new color.

6. Pressing color shortcuts in rapid succession (e.g. ⌘⌥1 then ⌘⌥2 within a few hundred milliseconds) leaves the tab in the color of the *last* shortcut pressed. No queuing, no animation backlog.

7. The visible result of a colored tab is identical to the result of choosing the same color via the existing right-click "Set color" menu. There is exactly one tab-color visual treatment in twarp; this feature adds a second input path (keyboard) to it, not a parallel rendering.

8. If a tab contains multiple panes (split panes inside one tab), the color applies to the **tab as a whole** — the tab-bar indicator. Per-pane coloring is not introduced by this feature.

9. **Multiple windows:** each window has its own active tab; ⌘⌥<n> in window A only affects window A's active tab.

10. **Persistence:** an assigned color survives quitting and relaunching twarp. After restart, every tab restored from the previous session shows the color it had when twarp last closed (or no color, if reset/never assigned). A tab whose color was reset before quit comes back uncolored. Closing a tab discards its color along with the tab.

11. **New tabs:** a tab created after a shortcut was used inherits the same defaults it would have inherited before this feature existed (uncolored, or directory-default-colored if applicable). The shortcut state is per-tab, not a "current pen color" mode.

12. **Focus rules:** Shortcut handling respects normal focus precedence:
    - When focus is in a surface that captures the keystroke (an open modal, command palette, settings page text editor), the shortcut is handled by that surface and does not change tab color, consistent with how other ⌘⌥-number shortcuts behave today.
    - When the user is typing in a terminal pane (the normal case), the shortcut still takes effect on the active tab — terminal panes do not swallow ⌘⌥-N. This holds even when a foreground process is running in the pane (e.g. `top`).

13. **Zero tabs:** with no tabs (an unusual state), the shortcuts are a no-op.

14. The feature does not gate on any new feature flag. It ships unconditionally, like other built-in default keybindings.

15. Color shortcuts do not steal focus, scroll the terminal, or emit telemetry distinct from the existing tab-color flow (one `TabTelemetryAction::SetColor` or `ResetColor` per effective change, same as the right-click menu).

16. **Discoverability — keybindings surface:** the nine shortcuts appear in twarp's keybindings settings page like any other built-in shortcut, under a category that groups them together. They are also discoverable in the command palette / shortcut help under names like "Set tab color: Red", "Reset tab color". Users can rebind, unbind, or assign the same actions to other keys through the existing settings surface — no special UI is added for this feature.

17. **Discoverability — tab-indicator hover tooltip:** hovering a tab's color indicator (the colored region/dot rendered on the tab in the tab bar) shows a tooltip in the form `<Color> — <shortcut>` (e.g. `Red — ⌘⌥1`).
    - The shortcut text reflects the **currently bound** key combination, sourced live from the corresponding `EditableBinding`. If a user rebinds the shortcut, the tooltip updates automatically.
    - If the user has unbound the shortcut for that color, the tooltip falls back to just `<Color>` with no shortcut suffix; no "Unbound" placeholder.
    - Uncolored tabs do not show a color tooltip from this feature. The reset shortcut (⌘⌥0) is discoverable through the keybindings settings page only.
    - The right-click "Set color" menu remains unchanged — its swatches do not surface the keyboard shortcut. The tab indicator is the chosen discovery surface so users don't have to open a menu first.

## Smoke test

Run against a freshly built twarp binary.

1. Open twarp. Open three tabs.
2. Focus tab 2. Press `⌘⌥1`. Tab 2's color indicator turns red.
3. Hover tab 2's color indicator. Tooltip reads `Red — ⌘⌥1`.
4. Press `⌘⌥1` again on tab 2. Tab 2 stays red (unconditional set, no toggle-off, no flicker).
5. Press `⌘⌥3` on tab 2. Tab 2's color changes directly to green with no intermediate uncolored frame.
6. Press `⌘⌥0` on tab 2. Tab 2 returns to the default (uncolored) appearance.
7. Press `⌘⌥0` on tab 2 again. No change, no error.
8. Press `⌘⌥5` on tab 3. Tab 3 turns blue. Tab 2 unaffected.
9. Cycle through `⌘⌥1`..`⌘⌥8` on tab 1, confirming each maps to: red, yellow, green, cyan, blue, magenta, white, black.
10. With tab 1 still focused and running a foreground process in its terminal (e.g. `top`), press `⌘⌥2`. Tab 1's indicator turns yellow and the running process is unaffected.
11. Open the keybindings settings page. Confirm the nine new entries are listed under their group and rebindable.
12. Quit twarp and relaunch with workspace restore enabled. The previously colored tabs come back with their colors; the previously reset tab comes back uncolored.
