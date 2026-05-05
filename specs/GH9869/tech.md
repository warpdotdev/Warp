# Technical spec: Voice input hotkey picker (GH-9869)

This spec is the implementation companion to `product.md`. It picks
the data-model approach, the UI changes, and the testable invariants.

## Current implementation

- **Setting enum:** `VoiceInputToggleKey` at
  [app/src/settings/ai.rs:110-138](app/src/settings/ai.rs). 10
  variants. Derives `SettingsValue` (TOML `snake_case`).
- **Persistence:** `agents.voice.voice_input_toggle_key` TOML path,
  `SyncToCloud::Never` (intentional — see product spec §non-goals).
- **Display:** `VoiceInputToggleKey::display_name`
  ([app/src/settings/ai.rs:169-198](app/src/settings/ai.rs)) returns
  platform-aware strings ("Option (Left)" on macOS,
  "Alt (Left)" on Windows/Linux).
- **Mapping to `KeyCode`:** `VoiceInputToggleKey::to_key_code`
  ([app/src/settings/ai.rs:200-213](app/src/settings/ai.rs)) maps
  variants to `warpui_core::platform::KeyCode`.
- **Dropdown UI:** [app/src/settings_view/ai_page.rs:498-532](app/src/settings_view/ai_page.rs)
  builds a `Dropdown<AISettingsPageAction>` from
  `VoiceInputToggleKey::all_possible_values()`.
- **Underlying `KeyCode` enum:** [crates/warpui_core/src/platform/keyboard.rs:85](crates/warpui_core/src/platform/keyboard.rs)
  already supports `CapsLock` (line 210), `F1`–`F35` (lines 437+),
  and the long tail.
- **Action dispatch:** `block_list_element.rs:3072` and
  `alt_screen_element.rs:620` compare incoming `key_code` to the
  configured one. No code changes needed there for new variants —
  they speak `KeyCode`, not the enum.

## Data model: `Custom(KeyCode)` variant on the existing enum

Choice (a) from product.md §risks: extend `VoiceInputToggleKey` with
a `Custom(KeyCode)` variant.

```rust
pub enum VoiceInputToggleKey {
    None,
    Fn,
    AltLeft, AltRight,
    ControlLeft, ControlRight,
    SuperLeft, SuperRight,
    ShiftLeft, ShiftRight,
    CapsLock,                          // NEW
    F1, F2, F3, F4, F5, F6,            // NEW
    F7, F8, F9, F10, F11, F12,         // NEW
    Custom(KeyCode),                   // NEW
}
```

Why (a) over (b) (separate `Option<KeyCode>` override field):
- Single source of truth for "what key fires voice input."
- The dropdown selection state cleanly maps to one enum value.
- Avoids a "two settings disagree" failure mode where a user changes
  the dropdown but the override is stale.

Cost of (a):
- `SettingsValue` derive macro must support enum variants with
  payloads. If it does not (verify before implementing), we have two
  options: extend the macro, or use a manual `serde::Deserialize` /
  `serde::Serialize` impl with a tagged form
  (`{ kind = "custom", key = "f19" }`).

### TOML format

For known variants: existing snake_case (no change).
```toml
voice_input_toggle_key = "alt_left"
voice_input_toggle_key = "caps_lock"
voice_input_toggle_key = "f1"
```
For `Custom`:
```toml
voice_input_toggle_key = { custom = "F19" }
# or
voice_input_toggle_key = { custom = "Backslash" }
```
The `Custom` payload is the `KeyCode` `Debug` representation
(matches Serde's default for the existing `KeyCode` `Serialize`
derive).

### Migration

No migration needed. Existing TOML values continue to deserialize.
New variants only appear in TOML if the user picks them in the UI.

## UI changes (ai_page.rs)

### Dropdown sectioning

Replace the flat `dropdown.add_items(values.into_iter().map(...))`
loop at
[app/src/settings_view/ai_page.rs:517-528](app/src/settings_view/ai_page.rs)
with a sectioned builder:

```text
[Disabled]
  None

[Modifier keys]
  Fn (macOS only)
  Option (Left), Option (Right)
  Control (Left), Control (Right)
  Command (Left), Command (Right)
  Shift (Left), Shift (Right)
  Caps Lock                ← NEW

[Function keys]            ← NEW SECTION
  F1, F2, F3, F4, F5, F6,
  F7, F8, F9, F10, F11, F12

[Other]                    ← NEW SECTION
  Custom key…              ← opens press-to-capture modal
```

If `Dropdown` does not support section headings today, add a
non-clickable `DropdownItem` constructor (or use a thin separator
`DropdownItem`). The TECH spec recommends adding the heading
support; it is small and reusable.

### Press-to-capture modal

New module `app/src/settings_view/voice_hotkey_capture_modal.rs`.

- Triggered when the user clicks "Custom key…".
- Renders a focused modal: title "Press the key for voice input",
  body with the captured key (initially empty), Cancel + Confirm
  buttons, an inline error area.
- Listens for raw key events at the top level (using whatever modal
  key-event hook the existing settings modals use — verify by
  inspecting one existing modal first).
- Filters per B5 (product.md): rejects unbindable keys with an
  inline error; accepts any other single key.
- On Confirm, dispatches
  `AISettingsPageAction::SetVoiceInputToggleKey(VoiceInputToggleKey::Custom(key_code))`.
- On Cancel or Escape, closes without dispatch.
- The modal's reject-list (B5) is constructed at open time from:
  - The currently-bound app-quit shortcut (read from key bindings).
  - The currently-bound voice-input shortcut (the value being
    replaced — but allow re-confirming the same key).
  - A static list: `[Enter, Escape, Tab, Backspace, Space]`.

### Caps Lock toggle vs. hold semantics

Per product.md §risk 3, on macOS Caps Lock fires only on
toggle-on/toggle-off, not on press/release. The handler in
`block_list_element.rs:3072` uses press-and-release tracking. For
the `CapsLock` variant on macOS, switch the handler to
press-to-toggle: first event starts recording, second event stops.

Implementation: a new helper `VoiceInputToggleKey::is_toggle_mode()`
returns `true` for `CapsLock` on macOS, `false` otherwise. The
handler branches on this. The dropdown tooltip for the `CapsLock`
entry on macOS displays "Tap to start, tap again to stop" instead
of "Hold to talk."

## Display name for new variants

Extend `VoiceInputToggleKey::display_name`
([app/src/settings/ai.rs:169](app/src/settings/ai.rs)):

```rust
match self {
    // existing arms unchanged
    VoiceInputToggleKey::CapsLock => "Caps Lock",
    VoiceInputToggleKey::F1 => "F1",
    // ... F2..F12
    VoiceInputToggleKey::Custom(key_code) => {
        // Use a new helper that mirrors what the keymap layer shows
        // for this KeyCode, falling back to {key_code:?}.
        Box::leak(format_keycode_display(key_code).into_boxed_str())
    }
}
```

Note: `display_name` currently uses `Box::leak` for runtime-formatted
strings. The pattern is unchanged for `Custom`.

`tooltip_message` ([app/src/settings/ai.rs:247](app/src/settings/ai.rs))
gets matching arms.

## Mapping to `KeyCode`

Extend `VoiceInputToggleKey::to_key_code`
([app/src/settings/ai.rs:200](app/src/settings/ai.rs)):

```rust
VoiceInputToggleKey::CapsLock => Some(KeyCode::CapsLock),
VoiceInputToggleKey::F1 => Some(KeyCode::F1),
// ... F2..F12
VoiceInputToggleKey::Custom(key_code) => Some(*key_code),
```

`keystroke()` at line 218: the existing modifier-only path returns
a `Keystroke` with the modifier flag set. For non-modifier keys
(F-keys, CapsLock, custom), construct a `Keystroke { key:
keycode_string(key_code).to_string(), .. Default::default() }`.

## Settings reset on corrupted TOML (A6)

When `VoiceInputToggleKey` deserialization fails (user hand-edited
TOML with `voice_input_toggle_key = "enter"`), the `SettingsValue`
deserializer falls back to `Default` today. We need to additionally
fire a one-time toast.

Implementation: add a `validate()` method on `VoiceInputToggleKey`
called during `AISettings::initialize()`. If the loaded value
deserializes successfully but the resulting `KeyCode` is in B5's
reject list, reset to `None`, fire the toast via the existing
toast/notification channel, and log a warn.

## Test plan

### Unit tests (`app/src/settings/ai.rs::tests`)

- T1: TOML round-trip for each new variant (`caps_lock`, `f1`–`f12`).
- T2: TOML round-trip for `Custom(KeyCode::F19)` and
  `Custom(KeyCode::Backslash)`.
- T3: `display_name` returns expected strings on each platform.
- T4: `to_key_code` returns the correct `KeyCode` for each variant.
- T5: `is_toggle_mode()` returns true for `CapsLock` on macOS,
  false otherwise.
- T6: A TOML file with an invalid `voice_input_toggle_key = "enter"`
  loads as `None` and triggers the validation reset path.

### Unit tests (`app/src/settings_view/voice_hotkey_capture_modal_test.rs`)

- T7: Pressing F19 in the modal captures `KeyCode::F19`.
- T8: Pressing the app-quit shortcut shows an inline error and does
  not dispatch.
- T9: Pressing Escape closes without dispatch.
- T10: Holding only Shift for >800ms then releasing captures
  `KeyCode::ShiftLeft` (B3 carve-out).

### Integration test (`app/src/integration_testing/`)

- IT1: Open the AI settings page, click "Custom key…", press F19,
  click Confirm. Verify `AISettings.voice_input_toggle_key.value() ==
  Custom(KeyCode::F19)`.
- IT2: Set the setting to `CapsLock` on macOS, press CapsLock once,
  verify voice input starts; press CapsLock again, verify it stops.
- IT3: Set the setting to `F5`, hold F5, verify voice input is
  active; release, verify it stops.

## Files touched

- `app/src/settings/ai.rs` — extend enum, `display_name`,
  `to_key_code`, `keystroke`, `tooltip_message`, add
  `is_toggle_mode()` and `validate()`.
- `app/src/settings_view/ai_page.rs` — sectioned dropdown,
  "Custom key…" entry, modal trigger.
- `app/src/settings_view/voice_hotkey_capture_modal.rs` (new) —
  press-to-capture modal.
- `app/src/terminal/block_list_element.rs:3072` — branch on
  `is_toggle_mode()` for press-to-toggle vs hold-to-talk.
- `app/src/terminal/alt_screen/alt_screen_element.rs:620` — same.
- `app/src/settings_view/voice_hotkey_capture_modal_test.rs` (new) —
  T7–T10.
- `app/src/integration_testing/settings/voice_hotkey_test.rs` (new)
  — IT1–IT3.

## Out-of-scope follow-ups

- Modifier-combination hotkeys (Cmd+Shift+V).
- Per-tab or per-profile voice hotkey.
- Cloud sync for voice hotkey (would conflict with cross-device
  keyboard variation).
- Remapping the OS-wide voice input shortcut.

## Open questions for maintainer review

1. Does the `SettingsValue` derive macro support enum variants with
   payloads (`Custom(KeyCode)`), or do we need a manual
   `Serialize`/`Deserialize` impl?
2. For the dropdown sectioning, is there an existing
   `DropdownItem::heading()` constructor, or should we add one?
3. Confirm the macOS Caps Lock press-to-toggle decision (vs
   "document the difference and don't fix"). Discord and Krisp
   both go press-to-toggle.
4. Should the press-to-capture modal expose the captured `KeyCode`
   spelling to the user (debug aid), or only the friendly display
   name? Recommendation: friendly name with a hover tooltip
   showing `KeyCode::F19` for users who care.
