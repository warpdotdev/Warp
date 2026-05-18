# GH7342: Tech Spec — Customizable Warping Message
## 1. Context
This spec implements the behavior in `product.md`: personalize only the generic `Warping...` agent loading label while preserving more specific progress messages.
App-launch/startup splash screens and application boot loading surfaces are explicitly out of scope for this implementation, even if they also use `Warping...` copy.
Relevant current code:
- `app/src/ai/blocklist/block/view_impl/common.rs:133` defines `LOAD_OUTPUT_MESSAGE: &str = "Warping..."` plus the other task-specific loading-message constants.
- `app/src/ai/blocklist/block/view_impl/common.rs (278-387)` chooses the loading text in `render_warping_indicator`. Most branches return action-specific messages; the final fallback uses `props.default_warping_text`.
- `app/src/ai/blocklist/block/view_impl/common.rs (480-546)` renders the label as `MaybeShimmeringText`, using shimmer for active loading text and static text for waiting states.
- `app/src/ai/blocklist/block/view_impl/common.rs (709-743)` renders text through `render_output_status_text`; shimmering text goes through `shimmering_warp_loading_text`.
- `app/src/ai/loading/shimmering_warp_loading_text.rs:14` prepends the Warp glyph to the provided loading text and applies the shimmer animation.
- `app/src/ai/blocklist/block/status_bar.rs (748-828)` decides whether the latest exchange should show a warping indicator, resolves fallback-model copy via `resolve_fallback_warping_message`, and passes `default_warping_text` into `WarpingProps`.
- `app/src/ai/blocklist/block/status_bar.rs (1095-1172)` renders cloud setup/pre-first-exchange status labels such as `Setting up environment`; these should remain status-specific and should not be replaced by the custom generic message.
- `app/src/settings/ai.rs (1204-1542)` contains public Warp Agent settings that are synced and exposed through `settings.toml` via `toml_path`, including examples in `agents.warp_agent.other.*`.
- `app/src/settings_view/ai_page.rs (5592-5744)` renders the `Other` section of AI settings and already hosts similar agent-level UI preferences.
- `app/src/settings/init.rs (195-282)` hot-reloads `settings.toml` and reloads public settings on settings-file changes.
The main design choice is to store the behavior as simple public settings rather than a richer enum. A boolean plus optional custom string maps cleanly to Settings UI, TOML editing, default behavior, and the hidden state without requiring a custom serialized shape.
## 2. Proposed Changes
### 2.1 Add public AI settings
In `app/src/settings/ai.rs`, add two public settings to `AISettings` near the other `agents.warp_agent.other.*` preferences:
- `show_warping_message: ShowWarpingMessage`
  - Type: `bool`
  - Default: `true`
  - Supported platforms: `SupportedPlatforms::ALL`
  - Sync: `SyncToCloud::Globally(RespectUserSyncSetting::Yes)`
  - Private: `false`
  - TOML path: `agents.warp_agent.other.show_warping_message`
  - Description: whether the generic Warp Agent loading message is shown.
- `custom_warping_message: CustomWarpingMessage`
  - Type: `String`
  - Default: `String::new()`
  - Supported platforms: `SupportedPlatforms::ALL`
  - Sync: `SyncToCloud::Globally(RespectUserSyncSetting::Yes)`
  - Private: `false`
  - TOML path: `agents.warp_agent.other.custom_warping_message`
  - Description: custom text used in place of the generic `Warping...` message.
Add a small resolver method on `AISettings`, for example:
- `generic_warping_message(&self) -> GenericWarpingMessage`
Define `GenericWarpingMessage` as an app-level enum such as:
- `Text(String)`
- `Hidden`
Resolution rules:
1. If `show_warping_message` is false, return `Hidden`.
2. Normalize `custom_warping_message` by trimming leading/trailing whitespace and replacing ASCII newlines/tabs with spaces.
3. If normalized custom text is non-empty, return `Text(normalized_custom_text)`.
4. Otherwise return `Text(DEFAULT_WARPING_MESSAGE)`.
Move the default copy to a single shared constant, e.g. `DEFAULT_WARPING_MESSAGE: &str = "Warping..."`, so settings resolution and renderer fallback do not duplicate string literals. Keep existing exported `LOAD_OUTPUT_MESSAGE` as an alias or update call sites carefully to avoid a broad rename.
### 2.2 Preserve status-specific rendering precedence
Update `app/src/ai/blocklist/block/status_bar.rs` so `render_warping_indicator_for_latest_exchange` computes the generic fallback like this:
1. Call `resolve_fallback_warping_message(...)`.
2. If fallback-model text exists, pass `GenericWarpingMessage::Text(fallback_text)` into `WarpingProps`.
3. Otherwise pass `AISettings::as_ref(app).generic_warping_message()`.
This keeps fallback-model messages higher priority than the custom generic text, matching Behavior 8 and 9.
Do not change branches that render cloud setup progress, pre-first-exchange setup, blocked/waiting states, action-specific loading labels, or task-specific messages. Those branches already provide explicit strings before the final generic fallback in `render_warping_indicator`.
### 2.3 Teach the warping indicator to hide only the generic label
Change `WarpingProps` in `app/src/ai/blocklist/block/view_impl/common.rs` from:
- `default_warping_text: String`
to:
- `default_warping_text: GenericWarpingMessage`
or a similarly named type that can represent hidden state.
In `render_warping_indicator`, the final fallback branch currently returns `props.default_warping_text.clone()`. Adjust the message-selection flow so the selected value can be either:
- a concrete `String` from an action-specific branch, fallback-model branch, preset, custom text, or default text
- hidden, only when the final generic fallback is selected and the setting is disabled
Add a rendering representation for hidden generic text. Two workable approaches:
1. Add `MaybeShimmeringText::Hidden` and make `render_output_status_text` return `Empty::new().finish()` for it.
2. Keep `MaybeShimmeringText` unchanged but have `render_warping_indicator` omit the text child when the chosen generic label is hidden.
Prefer option 1 if it keeps the existing `render_warping_indicator_base` call shape simple. Ensure `should_indent_tip_for_warp_glyph` is false for hidden text so tips do not reserve glyph indentation when no glyph is shown.
The hidden state must not suppress the status row's controls. `buttons`, `secondary_element`, and suffix rendering should continue to work. If a future call site has hidden generic text and no controls or secondary content, it is acceptable for the rendered row to occupy minimal/empty space; the current active exchange path normally has at least the stop button.
### 2.4 Add Settings UI controls
Update `app/src/settings_view/ai_page.rs`, primarily the `OtherAIWidget` area:
- Add UI state for a message mode dropdown and a single-line custom message editor.
- Add `AISettingsPageAction` variants for:
  - selecting default
  - selecting hidden
  - selecting each preset
  - selecting custom mode
  - saving custom text
- Render a row labeled `Warping message` or `Agent loading message`.
- Dropdown items:
  - `Default: Warping...`
  - `Thinking...`
  - `Working on it...`
  - `One sec...`
  - `Custom`
  - `Hidden`
- When `Custom` is selected, show a single-line editor similar to `StartupShellView` in `app/src/settings_view/features/startup_shell.rs (91-189)`.
- Save custom text on Enter and blur. Disable saving when the normalized text is empty; keep the previous valid custom value and show an inline invalid/error state if needed.
- When a preset is selected, set `show_warping_message = true` and write the preset string to `custom_warping_message`.
- When default is selected, set `show_warping_message = true` and clear `custom_warping_message`.
- When hidden is selected, set `show_warping_message = false` without clearing `custom_warping_message`, so users can unhide and recover their previous custom text.
- Include search terms such as `warping loading message custom hide agent oz`.
Telemetry may record mode-level changes such as `Default`, `Preset`, `Custom`, or `Hidden`, but must not include the custom text.
### 2.5 Settings file behavior
The TOML-facing behavior should be:
```toml
[agents.warp_agent.other]
show_warping_message = true
custom_warping_message = "Thinking..."
```
Examples:
- Default: `show_warping_message = true` and `custom_warping_message` unset or blank.
- Custom/preset: `show_warping_message = true` and `custom_warping_message` set to non-empty text.
- Hidden: `show_warping_message = false`.
Because public settings already hot-reload through `settings/init.rs`, no special file watcher should be needed. Confirm views that render the status bar are notified when `AISettings` changes; if the warping indicator does not update on settings change, add a focused subscription in `BlocklistAIStatusBar` similar to its existing settings/model subscriptions.
### 2.6 Rollout and compatibility
No data migration is needed:
- Existing users have no stored values and therefore get `show_warping_message = true` plus blank custom text, resolving to `Warping...`.
- Older clients ignore unknown TOML keys or native preference entries according to existing settings behavior.
- If a synced custom string reaches an older/newer client with different UI presets, it still works as plain custom text.
## 3. Testing and Validation
Map tests to product behavior:
- Behavior 1: add a unit test for the resolver default path: enabled + blank custom text returns `Text("Warping...")`.
- Behavior 3-6: add resolver tests for preset/custom text, whitespace trimming, newline/tab normalization, emoji preservation, and blank custom fallback.
- Behavior 8: add or update tests around `render_warping_indicator` message selection so action-specific labels still win over the generic fallback.
- Behavior 9-10: test or manually verify hidden generic state renders without the generic text/glyph but keeps buttons and preserves status-specific labels.
- Behavior 11-12: manually verify settings-file hot reload updates the visible label without restart.
- Behavior 14: inspect telemetry call sites added for this feature and verify they never include the custom message string.
- Behavior 15: manually verify long custom text clips in a narrow pane and does not wrap or overlap buttons.
Suggested commands once implemented:
- Targeted unit tests for settings/message resolution, using the relevant crate test command already used for `app/src/settings`.
- Targeted unit tests for `app/src/ai/blocklist/block/view_impl/common.rs` if a message-selection helper is extracted.
- Existing settings schema/validation tests if adding the public settings changes generated schema output.
- Manual app verification for Settings UI, TOML hot reload, default loading, custom loading, hidden loading, and action-specific override states.
## 4. Risks and Mitigations
- Risk: replacing too many loading labels could remove useful progress information. Mitigation: only apply the custom/hidden value at the final generic fallback and leave all explicit action-specific branches unchanged.
- Risk: hidden generic text could remove access to stop/take-over controls. Mitigation: hide only the text/glyph, not the status row or buttons.
- Risk: custom text could leak into telemetry or model prompts. Mitigation: keep it as renderer/UI preference state only; telemetry records mode but not contents.
- Risk: settings UI and settings file could disagree on hidden/custom semantics. Mitigation: use the resolver as the single source of truth for both surfaces.
- Risk: layout regressions with long custom strings. Mitigation: preserve existing clipping and single-line rendering, and add narrow-pane manual verification.
## 5. Parallelization
Parallelization is not recommended for the initial implementation. The change is modest but crosses tightly coupled settings definitions, Settings UI state, and the loading renderer; splitting these across agents would create merge conflicts in `ai_page.rs` and require frequent coordination. A single implementation branch should make the settings contract, renderer changes, and validation together.
## 6. Follow-ups
- Add a CLI launch/session override only if users ask for a non-persistent per-run message after the Settings UI ships.
- Consider a separate follow-up for app-launch/startup splash copy if Product wants that surface to become configurable later.
