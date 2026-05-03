# Hide Warp icon from the Dock when using Hotkey window
## Context
This tech spec implements the behavior in `product.md` for GitHub issue #1154.
Warp already has two mutually exclusive global hotkey modes:
1. `app/src/settings/mod.rs (239-276)` defines `GlobalHotkeyMode`, including "Dedicated hotkey window" for Quake Mode and "Show/hide all windows" for the activation hotkey.
2. `app/src/settings/mod.rs (292-329)` defines `QuakeModeSettings`, which already stores the dedicated Hotkey window keybinding, pinning, display, size, and auto-hide options.
3. `app/src/terminal/keys_settings.rs (16-65)` persists the dedicated Hotkey window settings at `global_hotkey.dedicated_window.settings`, the dedicated Hotkey window enabled flag at `global_hotkey.dedicated_window.enabled`, and the activation hotkey settings at `global_hotkey.toggle_all_windows.*`.
4. `app/src/terminal/keys_settings.rs (89-112)` enforces that Quake Mode and the activation hotkey are mutually exclusive.
5. `app/src/settings_view/features_page.rs (5309-5417)` renders the "Global hotkey" settings widget and conditionally renders dedicated Hotkey window controls only when `GlobalHotkeyMode::QuakeMode` is selected.
6. `app/src/root_view.rs (440-499)` registers fixed actions for showing and hiding the dedicated Hotkey window and for showing or hiding all non-Quake windows.
7. `app/src/root_view.rs (497-531)` registers the saved global hotkey keybindings on startup.
8. `app/src/root_view.rs (1342-1499)` owns dedicated Hotkey window state transitions: create the pinned window, show it, focus it, hide it, and move it between screens.
9. `app/src/root_view.rs (1499-1538)` owns the activation hotkey behavior for normal windows and should not be affected by Dock hiding.
10. macOS global shortcut registration is implemented in `crates/warpui/src/platform/mac/delegate.rs (384-399)` and `crates/warpui/src/platform/mac/objc/app.m (56-83)`.
11. macOS window show/hide behavior for the dedicated Hotkey window is implemented in `crates/warpui/src/platform/mac/objc/window.m (923-1009)`.
12. macOS Dock reopen behavior currently lives in `crates/warpui/src/platform/mac/objc/app.m (301-311)`, which opens a new window when the Dock icon is clicked and no windows are visible.
The current code does not expose an app-level API for changing the macOS Dock activation policy at runtime. The implementation should add that platform capability and apply it from the effective hotkey settings.
## Proposed changes
### Settings model
1. Add a new field to `QuakeModeSettings`, for example `hide_dock_icon: bool`, with default `false`.
2. Store it inside the existing `global_hotkey.dedicated_window.settings` table so it is versioned with the rest of the dedicated Hotkey window settings.
3. Give the schema description macOS-specific wording, such as "Whether Warp should hide its Dock icon while the dedicated hotkey window is active and configured."
4. Add `KeysSettings` helpers mirroring the existing Quake Mode helpers:
   - `set_hide_dock_icon_when_using_quake_mode_and_write_to_user_defaults(value, ctx)`
   - optionally `toggle_hide_dock_icon_when_using_quake_mode_and_write_to_user_defaults(ctx)` if the settings UI uses toggle actions consistently.
5. Keep the setting value independent from the effective behavior. The stored value may be true while effective behavior is false because the user selected another global hotkey mode, removed the keybinding, or is running on a non-macOS platform.
### Effective Dock visibility
1. Introduce a small pure helper that computes whether Warp should hide the Dock icon:
   - target OS is macOS,
   - `KeysSettings::global_hotkey_mode(ctx)` is `GlobalHotkeyMode::QuakeMode`,
   - `quake_mode_settings.hide_dock_icon` is true,
   - `quake_mode_settings.keybinding` is `Some`.
2. The helper should return "hide Dock icon" rather than "show Dock icon" so non-macOS and unsupported paths naturally default to visible.
3. Call this helper whenever hotkey-related settings change and at startup after settings are loaded.
4. Suggested application points:
   - after `maybe_register_global_window_shortcuts` in `app/src/root_view.rs`, so startup applies the saved effective state;
   - after `FeaturesPageView::set_global_hotkey_mode`, keybinding save/clear actions, and the new Dock-hiding toggle, or from a shared settings-change helper so changes made through settings files also apply;
   - from `FeaturesPageView::handle_hotkey_settings_update` only as a UI refresh path, not as the only application path, because the settings page may not be open.
5. Avoid tying Dock visibility to whether the dedicated Hotkey window is currently visible. The product behavior depends on the configured dedicated-hotkey workflow, not the transient window open/hidden state.
### Platform API
1. Add an app-level platform method such as `AppContext::set_dock_icon_visible(bool)` backed by `platform::Delegate`.
2. Add a default no-op implementation on `platform::Delegate` or update all platform delegates with no-ops so Linux, Windows, web, tests, and integration tests compile without macOS behavior.
3. Implement the macOS delegate by setting the NSApplication activation policy on the main thread:
   - visible: `NSApplicationActivationPolicyRegular`,
   - hidden: `NSApplicationActivationPolicyAccessory`.
4. Prefer keeping the Objective-C boundary small:
   - either expose a helper in `crates/warpui/src/platform/mac/objc/app.m` and declare it in `app.h`,
   - or call `setActivationPolicy:` from Rust in `crates/warpui/src/platform/mac/delegate.rs` if that matches nearby macOS platform patterns.
5. Log and leave the app visible if the policy call fails. Do not panic or block settings persistence.
6. When restoring the Dock icon, use `NSApplicationActivationPolicyRegular` even if no windows are visible so Dock-based reopen behavior in `crates/warpui/src/platform/mac/objc/app.m (301-311)` remains available.
### Settings UI
1. Extend `FeaturesPageAction` with a new action for toggling or setting the Dock-hiding preference.
2. In `GlobalHotkeyWidget`, render the new switch only under the `GlobalHotkeyMode::QuakeMode` branch and only on macOS.
3. Place it near the existing dedicated Hotkey window controls, after the keybinding row and before or after the existing pin/auto-hide controls.
4. Use copy that makes the scope explicit, for example:
   - label: "Hide Dock icon"
   - helper or tooltip: "When a dedicated hotkey is configured, keep Warp running without showing it in the Dock. Use the hotkey to show Warp again."
5. If the keybinding is missing, either disable the switch with helper text or allow changing the saved preference while clearly indicating that it applies once a keybinding is configured. The product spec leaves this UX choice open.
6. Preserve existing local-only/sync indicator conventions used by nearby settings rows.
### Startup and recovery behavior
1. Apply the effective Dock visibility after settings are loaded and before or alongside global hotkey registration.
2. If a user starts Warp with Dock hiding enabled but no dedicated Hotkey window keybinding, force the effective state to Dock-visible.
3. If the user switches from Quake Mode to the activation hotkey or disabled mode, immediately restore the Dock icon before unregistering or replacing shortcuts if possible. This ordering keeps an obvious recovery path if shortcut registration fails.
4. If the macOS activation policy transition removes the app from Cmd-Tab, do not attempt to work around it in this feature. Reflect the OS behavior in UI copy.
### Telemetry
1. If the existing settings telemetry automatically records `FeaturesPageAction` changes, ensure the new action gets a descriptive event value.
2. Do not add detailed per-hotkey telemetry for Dock visibility transitions unless product analytics explicitly needs it.
3. Never log the user’s actual global hotkey keybinding as part of this feature.
## End-to-end flow
1. User selects "Dedicated hotkey window" and saves a global keybinding.
2. `KeysSettings` stores Quake Mode as enabled and activation hotkey as disabled.
3. User enables "Hide Dock icon".
4. The settings action stores `quake_mode_settings.hide_dock_icon = true`.
5. Shared effective-state code sees macOS + Quake Mode + keybinding + setting enabled.
6. `AppContext` calls the platform delegate to set the macOS activation policy to accessory.
7. Warp disappears from the Dock but remains running and responding to the registered global hotkey.
8. If the user disables the setting, clears the keybinding, or changes global hotkey mode, the same effective-state code restores regular activation policy and the Dock icon returns.
## Risks and mitigations
1. Risk: Users can lose their obvious app entry point if the Dock icon is hidden without a working hotkey. Mitigation: effective Dock hiding requires a configured dedicated Hotkey window keybinding and restores the Dock icon when the keybinding is removed.
2. Risk: Dock policy changes may behave differently across macOS versions. Mitigation: isolate the policy change behind a macOS platform API, log failures, and keep default behavior visible.
3. Risk: Applying Dock visibility only from the settings page misses changes from settings file edits or synced settings. Mitigation: put effective-state application in a shared helper invoked from startup and model/settings change paths, not only from UI event handlers.
4. Risk: Hiding the Dock icon could unexpectedly affect Cmd-Tab. Mitigation: document this in product copy and avoid promising Dock-only behavior that macOS cannot provide.
5. Risk: Adding a required trait method can break non-macOS builds. Mitigation: provide default no-op behavior or update test, Linux, Windows, web, and integration delegates in the same implementation.
## Testing and validation
1. Unit-test the pure effective-state helper for product Behavior 2, 4, 8, 9, 10, and 13:
   - default setting false never hides;
   - Quake Mode + setting true + keybinding hides on macOS;
   - disabled mode, activation hotkey mode, missing keybinding, and non-macOS all remain visible.
2. Add or update settings serialization tests if this repository has coverage for generated settings schemas or TOML paths, verifying the new field defaults to false and round-trips in `global_hotkey.dedicated_window.settings`.
3. Add UI/action tests around `FeaturesPageAction` if nearby settings actions are covered, verifying the switch updates `QuakeModeSettings` without changing the selected global hotkey mode.
4. Build or typecheck affected Rust crates on Linux to catch trait and settings-model regressions even though macOS behavior is no-op there.
5. Run a macOS manual validation pass:
   - enable dedicated Hotkey window and keybinding;
   - enable Dock hiding and confirm the Dock icon disappears;
   - press the hotkey and confirm the Hotkey window still shows and hides;
   - disable Dock hiding and confirm the Dock icon returns without restart;
   - clear the keybinding and confirm the Dock icon returns;
   - switch to "Show/hide all windows" and confirm the Dock icon returns.
6. Validate session preservation by running a long-lived command, toggling Dock hiding on and off, and confirming the command, pane, and tab remain intact.
7. Validate restart behavior by quitting and relaunching with the setting enabled and a configured dedicated hotkey, then verifying the effective Dock state matches the saved settings.
## Parallelization
Implementation can split across two agents after spec approval:
1. Platform/settings agent: add the setting, effective-state helper, platform API, macOS activation policy, and unit tests.
2. UI/validation agent: add the Settings > Features switch, copy, action handling, and UI-focused tests/manual validation.
The agents should coordinate on the exact setting field name and effective-state helper API before coding to avoid conflicts in `KeysSettings` and `features_page.rs`.
## Follow-ups
1. Consider whether Warp should offer a menu bar extra for hidden-Dock workflows. That is explicitly out of scope for this issue.
2. Consider adding user-facing docs for the macOS dedicated Hotkey window workflow once the setting ships.
