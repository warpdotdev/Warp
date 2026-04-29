# Tab color shortcuts — PRODUCT spec

## Summary

Keyboard shortcuts that recolor the currently-active tab so users can visually distinguish workflows at a glance ("red is prod, green is local"). Pressing one of the bound shortcuts assigns a fixed default color to the active tab; one shortcut clears the color. The colored state persists across restarts. Scope is limited to the keystroke → color binding for the eight default colors plus a clear; configurability and custom-color UI are out of scope.

## Goals / Non-goals

**Goals**
- Provide a one-keystroke way to mark the active tab with one of eight visually distinct colors, plus a clear shortcut.
- Reuse twarp's existing tab-color visual treatment (the surface already used by the right-click "Set color" menu) so the keyboard path produces the same on-screen result.
- Persist a tab's color across app restarts.

**Non-goals (deferred to follow-ups)**
- User-configurable shortcut bindings.
- A color picker UI or custom hex/RGB colors.
- More than eight colors, or palette swaps per theme.
- Rules that auto-color tabs (by directory, command, host, etc.). The existing directory→color default that ships with twarp continues to work but is not modified by this feature.

## Behavior

1. While twarp is the focused application, pressing one of the following key combinations sets the **active tab's** color to the listed color and leaves all other tabs unchanged:

   | Shortcut | Color  |
   |----------|--------|
   | ⌘⌥1      | Red    |
   | ⌘⌥2      | Orange |
   | ⌘⌥3      | Yellow |
   | ⌘⌥4      | Green  |
   | ⌘⌥5      | Blue   |
   | ⌘⌥6      | Purple |
   | ⌘⌥7      | Pink   |
   | ⌘⌥8      | Gray   |
   | ⌘⌥0      | *Reset (clear color)* |

   **Open question:** twarp currently ships a 6-color tab palette (Red, Green, Yellow, Blue, Magenta, Cyan) inherited from upstream. Honoring the eight colors above requires extending the palette to include Orange, Purple, Pink, and Gray, and replacing Cyan/Magenta with the README list. Whether to extend the palette or align the spec to the existing 6 is a TECH.md decision; this spec assumes the eight README colors are the target.

2. "Active tab" means the single tab that currently owns workspace focus in the focused window. Tabs in other windows, and inactive tabs in the focused window, are never affected by a shortcut press.

3. The shortcut fires regardless of which surface inside the window has keyboard focus (terminal pane, command palette, settings page, etc.), so long as twarp itself is the focused app and a tab is active. The shortcut is not swallowed by an active terminal pane.

4. ⌘⌥0 ("Reset") clears any user-assigned color from the active tab. After reset, the tab renders the same way it did before any color was ever assigned: if twarp's directory→color default would otherwise color the tab, that default takes effect; otherwise the tab shows the standard uncolored indicator.

5. Pressing a color shortcut on an active tab that already has the *same* color is a no-op from the user's perspective: no flicker, no visible animation, no change to persisted state beyond a re-assert of the same value.

6. Pressing a color shortcut on an active tab that already has a *different* color replaces the color in place. There is no intermediate uncolored frame; the tab transitions directly from the old color to the new color.

7. Pressing color shortcuts in rapid succession (e.g. ⌘⌥1 then ⌘⌥2 within a few hundred milliseconds) leaves the tab in the color of the *last* shortcut pressed. No queuing, no animation backlog.

8. The visible result of a colored tab is identical to the result of choosing the same color via the existing right-click "Set color" menu. There is exactly one tab-color visual treatment in twarp; this feature adds a second input path (keyboard) to it, not a parallel rendering.

9. If a tab contains multiple panes (split panes inside one tab), the color applies to the **tab as a whole** — the tab-bar indicator. Per-pane coloring is not introduced by this feature.

10. **Persistence:** an assigned color survives quitting and relaunching twarp. After restart, every tab restored from the previous session shows the color it had when twarp last closed (or no color, if reset/never assigned). A tab whose color was reset before quit comes back uncolored.

11. **New tabs:** a tab created after a shortcut was used inherits the same defaults it would have inherited before this feature existed (uncolored, or directory-default-colored if applicable). The shortcut state is per-tab, not a "current pen color" mode.

12. **Closed tabs:** closing a colored tab discards its color along with the tab. Reopening a closed tab is out of scope of this feature; if twarp gains tab-reopen later, that flow is responsible for restoring the color.

13. **Multiple windows:** each window has its own active tab; ⌘⌥<n> in window A only affects window A's active tab.

14. **Discoverability — keyboard surface:** the shortcuts appear in twarp's keyboard-shortcut surface (command palette / shortcut help) under names like "Set tab color: Red", "Reset tab color". They behave the same way whether invoked by keyboard or by command palette.

15. **Discoverability — right-click menu:** the existing right-click "Set color" menu shows the keyboard shortcut alongside each color, so a user discovering the menu also discovers the shortcut. Concretely:
    - Each color swatch's hover tooltip reads `<Color> — <shortcut>` (e.g. `Red — ⌘⌥1`, `Default (no color) — ⌘⌥0`).
    - In any text-row variant of the same menu, the shortcut appears as a right-aligned hint on the row (matching how other menu items in twarp display their bound shortcuts).
    - The shortcut text reflects the **currently bound** key combination, not a hardcoded label. If a user later rebinds the shortcut (out of scope of this feature), the tooltip updates automatically.
    - When a color has no binding (e.g. the user explicitly cleared it), the tooltip falls back to the color name with no shortcut suffix; no "Unbound" placeholder.

16. Color shortcuts do not steal focus, scroll the terminal, or emit telemetry distinct from the existing right-click color flow.

## Smoke test

Run against a freshly built twarp binary.

1. Open twarp. Open three tabs.
2. Focus tab 2. Press ⌘⌥1. Tab 2's indicator turns red. Tabs 1 and 3 are unchanged.
3. Focus tab 2. Press ⌘⌥0. Tab 2's indicator returns to its default (uncolored, or directory default if that applies).
4. Focus tab 3. Press ⌘⌥4. Tab 3's indicator turns green; tab 2 is unaffected.
5. With tab 3 still focused and green, press ⌘⌥4 again. Nothing visibly changes (no flicker).
6. Press ⌘⌥6 on tab 3. Tab 3 transitions directly from green to purple with no uncolored frame.
7. Quit twarp and relaunch. Tab 3 reopens purple; tabs 1 and 2 reopen uncolored.
8. With twarp focused but the terminal pane actively running a foreground process (e.g. `top`), press ⌘⌥2 on the active tab. The tab indicator turns orange and the running process is unaffected.
9. Right-click any tab and open the "Set color" menu. Hover each color swatch — the tooltip reads `<Color> — <shortcut>` (e.g. `Red — ⌘⌥1`). Hover the no-color/reset entry — the tooltip reads `Default (no color) — ⌘⌥0`.
