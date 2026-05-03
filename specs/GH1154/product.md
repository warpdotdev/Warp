# Hide Warp icon from the Dock when using Hotkey window
## Summary
Users who primarily access Warp through the dedicated Hotkey window on macOS can choose to hide Warp from the Dock so Warp stays available through the global hotkey without occupying a persistent Dock slot.
This feature adds an opt-in setting for that workflow while preserving the current visible-Dock behavior by default.
Related issue: #1154
## Problem
The dedicated Hotkey window is designed for users who keep Warp running in the background and summon it with a shortcut. For that workflow, a persistent Warp Dock icon can feel unnecessary and add Dock clutter, but removing it must not make Warp hard to recover or change behavior for users who use normal windows.
## Goals
1. Let macOS users hide the Warp Dock icon when they use the dedicated Hotkey window.
2. Preserve current behavior for existing users unless they opt in.
3. Keep the user in control from Warp settings and make the recovery path clear.
4. Avoid affecting the separate "Show/hide all windows" activation hotkey mode.
## Non-goals
1. This feature does not add a menu bar extra or status bar item.
2. This feature does not change the global hotkey registration model, Quake Mode window sizing, pinning, or auto-hide behavior.
3. This feature does not hide Warp from the Dock for normal-window-only workflows.
4. This feature does not change how Warp appears on Linux or Windows.
## Figma
Figma: none provided.
## Behavior
1. On macOS, when the "Global hotkey" setting is set to "Dedicated hotkey window", Settings > Features shows a new switch for hiding Warp from the Dock.
2. The setting is off by default for all users, including users who already have the dedicated Hotkey window enabled.
3. When the setting is off, Warp behaves exactly as it does today: the Dock icon remains visible whenever Warp is running, and Dock interactions continue to open, focus, or create Warp windows according to existing behavior.
4. When the setting is on and a dedicated Hotkey window keybinding is configured, Warp removes its icon from the Dock while remaining running in the background.
5. While the Dock icon is hidden, the configured dedicated Hotkey window shortcut remains the primary way to show and hide Warp. Pressing the shortcut opens the dedicated Hotkey window if needed, focuses it when showing, and hides it when toggled away, matching existing Hotkey window behavior.
6. Hiding the Dock icon does not close sessions, tabs, panes, or normal windows. Any visible Warp window remains visible, usable, and focusable.
7. Hiding the Dock icon does not change Hotkey window layout settings: pinned edge, size percentages, display selection, background blur, and "hide window when unfocused" continue to behave as configured.
8. The setting applies only while the effective global hotkey mode is "Dedicated hotkey window". If the user switches the global hotkey mode to "Disabled" or "Show/hide all windows", Warp shows its Dock icon again immediately.
9. If the user clears the dedicated Hotkey window keybinding while the Dock-hiding setting is on, Warp shows its Dock icon again so the user is not left without an obvious way to return to the app.
10. If the user later restores a dedicated Hotkey window keybinding and the Dock-hiding setting is still on, Warp hides the Dock icon again.
11. If the user turns the setting off, Warp restores the Dock icon immediately without requiring an app restart.
12. The setting state is persisted with the dedicated Hotkey window settings. Restarting Warp reapplies the effective Dock visibility based on the saved setting, the saved hotkey mode, and whether a dedicated Hotkey window keybinding is configured.
13. On non-macOS platforms, the setting is not shown and has no effect if present in synced or imported settings.
14. If macOS cannot apply the hidden-Dock state for any reason, Warp leaves the Dock icon visible and continues running normally.
15. The setting label or helper text communicates that hiding the Dock icon also removes Warp from normal Dock-based app discovery. If macOS also removes Warp from the app switcher as part of this state, the UI should not imply that Cmd-Tab remains available.
16. Dock visibility changes must not interrupt an in-progress terminal session, command execution, AI session, update prompt, modal, or notification flow.
17. Users can still open normal Warp windows through existing in-app actions while the Dock icon is hidden. Those normal windows do not by themselves force the Dock icon to return unless the user changes the global hotkey mode, clears the dedicated Hotkey window keybinding, or disables the Dock-hiding setting.
## Success criteria
1. A macOS user can enable the dedicated Hotkey window, enable Dock hiding, and verify that Warp disappears from the Dock while still opening from the configured hotkey.
2. The Dock icon returns immediately when the user disables the setting, changes away from dedicated Hotkey window mode, or removes the dedicated Hotkey window keybinding.
3. Existing users and non-macOS users see no behavior change unless they opt into the new macOS setting.
4. Session state and window contents survive every Dock visibility transition.
## Validation notes
1. Validate the behavior against the numbered invariants above on macOS with the dedicated Hotkey window enabled.
2. Validate that non-macOS builds ignore the setting and do not expose macOS-specific UI.
3. Validate that settings persistence and restart behavior match the effective-state rules in Behavior 8 through 12.
## Open questions
1. Should this setting be stored only locally on each machine, or should it sync with the existing dedicated Hotkey window settings? The current product expectation is that it persists with the dedicated Hotkey window settings, while non-macOS clients ignore it.
2. Should the setting remain visible but disabled when the dedicated Hotkey window mode is selected without a keybinding, or should it remain enabled while the effective Dock-hiding behavior stays inactive until a keybinding exists?
