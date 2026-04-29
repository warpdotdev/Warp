---
name: 01 — Tab color shortcuts
status: draft
---

# Tab color shortcuts

## Summary

Keyboard shortcuts that color-tag the currently active tab, so users can visually distinguish workflows at a glance ("red tab is prod, green tab is local"). Eight color shortcuts plus one reset, on top of the tab-color surface that already exists upstream — this feature only adds the keyboard surface, not the rendering or storage of tab colors.

## Behavior

1. The following default keyboard shortcuts are registered and dispatch to the active tab in the focused window:

   | Shortcut | Action |
   |---|---|
   | `⌘⌥1` | Set color: Red |
   | `⌘⌥2` | Set color: Yellow |
   | `⌘⌥3` | Set color: Green |
   | `⌘⌥4` | Set color: Cyan |
   | `⌘⌥5` | Set color: Blue |
   | `⌘⌥6` | Set color: Magenta |
   | `⌘⌥7` | Set color: White |
   | `⌘⌥8` | Set color: Black |
   | `⌘⌥0` | Reset color |

   **Open question:** README §2 lists Red/Orange/Yellow/Green/Blue/Purple/Pink/Gray as the tentative defaults. The existing upstream `AnsiColorIdentifier` palette only has eight colors (Red, Green, Yellow, Blue, Magenta, Cyan, White, Black) — Orange, Purple, Pink, and Gray are not present. This spec uses the eight ANSI identifiers as-is so the feature ships small and stays aligned with how the rest of the tab-color subsystem renders. If the user wants Orange/Purple/Pink/Gray, that requires extending `AnsiColorIdentifier` and the theme palette — a separate, larger change. README is marked "Tentative defaults (configurable)", so this divergence is in-bounds; flag in review if you want a different default mapping or want to expand the palette before shipping.

2. The eight color shortcuts (⌘⌥1–⌘⌥8) **unconditionally set** the active tab to the named color. Pressing the shortcut for the color a tab already has is a no-op (visually unchanged). This differs from the existing tab-context-menu's toggle-off behavior — the dedicated reset shortcut (⌘⌥0) is the only way to clear a color via keyboard, which keeps each color shortcut predictable.

3. The reset shortcut (⌘⌥0) clears the active tab's manual color. After reset:
   - If the tab has a directory-driven color (via the existing automatic-coloring feature), that color becomes visible.
   - Otherwise the tab returns to the default uncolored appearance.
   - Pressing ⌘⌥0 on an already-uncolored tab is a no-op (no error, no visible change).

4. Shortcuts only affect the **currently active tab** in the **currently focused window**. Inactive tabs and tabs in other windows are untouched. There is no multi-select coloring and no "color all tabs" action in this feature.

5. Each color shortcut produces an immediate visible change — the tab indicator (and any other surfaces that already show tab color upstream) updates synchronously, with no confirmation modal or transient state.

6. The shortcuts are listed in the keybindings settings page like any other built-in shortcut, under a category that groups them together (e.g. "Tabs"). Users can rebind any of them, unbind them, or assign the same actions to other keys through the existing settings surface — no special UI is added for this feature.

7. The color set is fixed at the eight named colors above plus reset. There is no UI for picking arbitrary colors via this feature; users who want a different palette adjust the theme's ANSI palette globally, which is the existing mechanism that drives tab color rendering.

8. Color names map to the existing ANSI color identifiers used by the rest of the tab-color subsystem, so the rendered hue is theme-aware (light vs. dark themes apply their own ANSI palette) and matches the color a user would get by picking that color from the existing tab context menu.

9. Persistence matches the existing manual-tab-color behavior: a tab's color is saved with the rest of the tab/session state and restored when the workspace is restored. Closing and reopening a tab in the same session does not preserve color (a closed tab is gone). Restart of the app preserves color via the normal session restore path.

10. Shortcut handling respects normal focus rules:
    - When focus is in a non-tab surface that captures the keystroke (e.g. an open modal, command palette, settings page editor, the AI assistant input), the shortcut does not change tab color and is handled by that surface or ignored, consistent with how other ⌘⌥-number shortcuts behave today.
    - When the user is typing in the terminal (the normal case), the shortcut takes effect on the active tab.

11. With zero tabs (an impossible state in practice but worth being explicit), the shortcuts are a no-op.

12. The feature does not gate on any new feature flag. It ships unconditionally, like other built-in default keybindings.

## Smoke test

Run against a freshly built twarp binary.

1. Open twarp. Open three tabs.
2. Focus tab 2. Press `⌘⌥1`. Tab 2's color indicator turns red.
3. Press `⌘⌥1` again on tab 2. Tab 2 stays red (unconditional set, no toggle-off).
4. Press `⌘⌥3` on tab 2. Tab 2's color changes to green.
5. Press `⌘⌥0` on tab 2. Tab 2 returns to the default (uncolored) appearance.
6. Press `⌘⌥0` on tab 2 again. No change, no error.
7. Press `⌘⌥5` on tab 3. Tab 3 turns blue. Tab 2 unaffected.
8. Cycle through `⌘⌥1`..`⌘⌥8` on tab 1, confirming each maps to: red, yellow, green, cyan, blue, magenta, white, black.
9. Open the keybindings settings page. Confirm the nine new entries are listed and rebindable.
10. Quit twarp and relaunch with workspace restore enabled. The previously colored tabs come back with their colors.
