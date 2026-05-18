# GH7342: Product Spec — Customizable Warping Message
## 1. Summary
Allow users to personalize the generic `Warping...` loading message shown while Warp's agent is starting work or waiting on an in-progress response. Users can keep the default copy, choose a preset alternative, enter their own short message, or hide the generic message while preserving important status-specific progress labels and controls.
## 2. Problem
The generic `Warping...` message is currently hardcoded. Users who customize Warp's appearance and workflow cannot personalize this visible loading state, and users who prefer a quieter UI cannot suppress the generic phrase.
## 3. Goals
- Preserve the existing `Warping...` experience by default.
- Let users configure the generic warping/loading message from Settings UI.
- Let users configure the same behavior from the user-editable settings file.
- Let users hide the generic message without hiding important action controls or status-specific messages.
- Keep task-specific progress messages intact so users still understand what the agent is doing.
## 4. Non-goals
- No CLI flag or launch parameter in the first implementation.
- No per-workspace, per-session, or per-conversation message overrides.
- No randomized, rotating, animated, markdown, or rich-text custom messages.
- No customization of status-specific messages such as `Searching codebase...`, `Creating diff...`, `Setting up environment`, or fallback-model messages like `Warping with Claude...`.
- No customization of any app-launch/startup splash screen or application boot loading message, even if that surface also uses `Warping...`; this feature applies only to agent loading/status surfaces inside Warp.
- No change to the app icon, shimmer animation, colors, fonts, or loading indicator placement.
## 5. Figma / Design References
Figma: none provided. This is a small Settings UI and loading-status behavior change; if design provides final copy or layout before implementation, update this spec to reference it.
## 6. Behavior
1. Default behavior is unchanged. A user who never changes the setting continues to see `Warping...` on the generic agent loading/status surfaces covered by this feature.
2. Warp exposes a user-facing setting in Settings for the generic warping message. The control is discoverable from the same area that contains other Warp Agent/Oz settings.
3. The Settings UI offers these choices:
   - `Default` — shows `Warping...`.
   - Preset alternatives — built-in plain-text options such as `Thinking...`, `Working on it...`, and `One sec...`.
   - `Custom` — lets the user enter a short plain-text message.
   - `Hidden` — suppresses the generic warping message.
4. Selecting a preset immediately uses that preset text as the generic warping message. Presets are convenience options only; they do not introduce randomization or cycling.
5. Selecting `Custom` shows a single-line text field. The field accepts plain Unicode text, including emoji, but does not render markdown, links, ANSI escapes, or multiline content.
6. Custom text is normalized before display:
   - Leading and trailing whitespace is ignored.
   - Newlines and tabs are treated as spaces.
   - Empty custom text is not saved from Settings UI; if an empty custom value is encountered from the settings file, Warp falls back to `Default`.
   - The Settings UI limits custom input to a short single-line value, targeting 80 visible characters or fewer.
7. A custom or preset message is displayed exactly where the generic `Warping...` message would otherwise appear. The Warp glyph, shimmer effect, clipping, typography, and theming remain consistent with the existing loading indicator.
8. The customization applies only to the generic loading label. More specific progress labels continue to take precedence, including but not limited to:
   - `Adjusting tasks...`
   - `Generating fix...`
   - `Creating diff...`
   - `Preparing question...`
   - `Searching codebase...`
   - `Grepping...`
   - `Finding files...`
   - `Executing command...`
   - `Waiting for command to exit...`
   - `Setting up environment`
   - fallback-model labels such as `Warping with {model}.`
9. When the setting is `Hidden`, the generic phrase and generic Warp-glyph loading text are not shown. Warp still shows any available stop, queue, auto-execute, take-over, hide-response, `Check now`, tip, or other status controls that are needed to manage the in-progress agent work.
10. When the setting is `Hidden`, status-specific progress labels listed in Behavior 8 still render normally. Hiding the generic message must not make active work look complete when Warp has a more specific status to show.
11. Settings file editing provides the same effective states as the Settings UI:
    - Default behavior when the custom text is unset or blank and generic message display is enabled.
    - Custom/preset behavior when a non-empty custom text value is configured.
    - Hidden behavior when generic message display is disabled.
12. Settings changes apply without restarting Warp. Existing visible generic warping indicators update on the next normal UI render after the setting changes or the settings file hot-reloads.
13. The setting is user-level and follows the same sync behavior as comparable public Warp Agent UI preferences. A change made on one synced device should apply on other devices after preferences sync, subject to existing sync timing.
14. Custom message content is never sent to the AI model as prompt context. Telemetry may record that the setting mode changed, but must not record the user's custom message text.
15. If a custom message is too long for the available width, it is clipped with the same end-clipping behavior as the current loading text. The loading row must not wrap, resize unexpectedly, or overlap action buttons.
16. The loading message remains accessible as normal UI text when visible. Hiding the generic message must not remove keyboard access to adjacent controls.
17. Invalid or unsupported settings-file values fail safely. Warp should either keep using the previous valid value or fall back to `Default` using the repository's existing settings-file error behavior; it should not crash or prevent the user from opening Settings.
## 7. Success Criteria
1. With default settings, the generic loading state still displays `Warping...`.
2. A user can select a preset from Settings and see that preset in the generic loading state.
3. A user can enter a custom single-line message from Settings and see it in the generic loading state.
4. A user can hide the generic message from Settings without losing stop or other in-progress controls.
5. Equivalent settings-file edits update the generic loading message without an app restart.
6. Specific status labels continue to override the custom message when the agent is performing a known action.
7. Fallback-model labels continue to render as `Warping with {model}.` or `Warping with another model.` rather than using the custom generic message.
8. Custom message text is not included in prompt context or telemetry payloads.
9. Long custom text remains single-line and clipped instead of wrapping or breaking the layout.
10. Invalid settings-file values do not crash Warp and recover through the existing settings error path or default fallback.
## 8. Validation
- Manual: start an agent request with default settings and confirm the generic label remains `Warping...`.
- Manual: select each preset and confirm the generic loading label updates.
- Manual: enter a custom message with spaces, emoji, and punctuation and confirm the normalized single-line text displays.
- Manual: set the message to `Hidden` and confirm the generic phrase disappears while stop/queue/take-over controls still work.
- Manual: trigger specific actions such as codebase search, grep, file glob, passive code diff generation, and long-running command monitoring; confirm their specific labels still override the custom message.
- Manual: edit the settings file while Warp is running and confirm hot reload updates the loading label.
- Manual: try an overly long custom value and confirm the row clips without wrapping or overlapping buttons.
- Manual or unit: verify custom message text is not included in AI request payloads and is not emitted in telemetry.
## 9. Open Questions
- Should the preset copy be finalized by Product/Design before implementation, or is the initial list in Behavior 3 acceptable?
