# Opt-in Native Left-Drag Selection in TUIs (GH-10353)

## Summary

Add an opt-in setting that makes bare left-drag perform Warp's native text selection inside alt-screen TUIs (vim, htop, tmux, less, etc.), instead of forwarding left button events to the TUI per the mouse-reporting protocol. When enabled, users can left-drag to select and `Cmd+C` to copy without holding `Shift`. All other mouse inputs (right-click, scroll, middle-click, modifier+click variants) continue to follow existing mouse-reporting rules. Default remains `false` to preserve current behavior.

## Problem

macOS users coming from iTerm2 expect bare left-drag to select text in any terminal context. Today, when an app inside Warp enables mouse-reporting in alt-screen mode, Warp forwards left-down/left-up/left-drag events to the TUI per the protocol, so:

- Bare left-drag draws nothing visible to the user; the TUI consumes the events.
- `Cmd+C` has nothing to copy because no native selection exists.
- The current escape hatch is `Shift+drag`, which most users don't discover.

iTerm2's default is the inverse: bare left-drag = native selection; `Option+drag` = forward to the TUI. Some Warp users prefer that model.

The issue author has a working code branch at https://github.com/spalagu/warp/tree/feat/left-drag-select-default that implements the proposal. This spec formalizes the contract before merge.

## Goals

- Add a single opt-in setting that, when enabled, makes bare left-drag a native selection gesture inside alt-screen TUIs.
- Preserve all other mouse-reporting behavior unchanged (right-click, scroll, middle-click, modifier-click variants, non-alt-screen apps).
- Match iTerm2's inverse-modifier semantics: when the setting is on, `Option+left-drag` is the explicit "forward to TUI" gesture.
- Take effect immediately for new mouse events after toggling — no app restart.

## Non-Goals

- Not changing the default. Default remains "left-drag forwards to TUI" so existing users see no behavior change.
- Not removing the `Shift+drag` fallback. `Shift+drag` continues to bypass mouse-reporting regardless of the setting.
- Not modifying right-click, scroll, or modifier-click forwarding rules.
- Not introducing per-app overrides (e.g., enable for vim but not htop) in V1.
- Not changing behavior in non-alt-screen, non-mouse-reporting apps (already native-select today).

## Behavior Contract

### B1. Setting definition

A new boolean setting `terminal.native_left_drag_select_enabled` lives in the `AltScreenReporting` setting group alongside `mouse_reporting_enabled`. Default: `false`.

### B2. Setting OFF (default — current behavior)

In alt-screen + mouse-reporting-enabled apps:

- Left button events (down, up, drag) forward to the TUI per the mouse-reporting protocol.
- `Shift+left-drag` bypasses the protocol and performs native selection (existing escape hatch).

### B3. Setting ON

In alt-screen + mouse-reporting-enabled apps:

- All bare left button events (down, up, drag) are intercepted by Warp's native selection logic — equivalent to the user always holding `Shift` for left button.
- `Cmd+C` copies the resulting selection through the existing copy path.
- `Shift+left-drag` continues to perform native selection (no regression).

### B4. Other inputs unchanged

Regardless of setting value, the following continue to follow existing mouse-reporting rules:

- Right-click and right-drag.
- Scroll-wheel events.
- Middle-click and middle-drag.
- `Cmd+left-click` and other modifier+left-click variants that already have defined behavior (e.g., link follow).

When the setting is `true`, `Option+left-drag` MUST forward to the TUI as the inverse-modifier opt-out, mirroring iTerm2 semantics. This is the documented escape hatch for users who want to interact with the TUI's own selection (e.g., tmux pane resize handles).

### B5. Live toggle

The setting takes effect immediately for new mouse events after the user changes it. An in-flight selection or in-flight TUI drag is not interrupted — the change applies to the next mouse-down.

## Settings / API surface

| Key                                            | Type   | Default | Group                |
| ---------------------------------------------- | ------ | ------- | -------------------- |
| `terminal.native_left_drag_select_enabled`     | `bool` | `false` | `AltScreenReporting` |

UI placement: **Settings → Features → Terminal → "Native Left-Drag Selection"** toggle, grouped with the existing `Mouse Reporting` toggle. Subtitle: "Left-drag selects text natively in TUIs. Hold Option to forward drag to the TUI instead."

Command Palette action: `Toggle native left-drag selection in TUIs`.

Keybinding context flag: `terminal_native_left_drag_select` (boolean) exposed for users who want to gate custom keybindings.

## Acceptance Criteria

- **A1.** With the setting `false` (default), all current behavior is preserved: left-drag in vim/htop/tmux forwards to the TUI; `Shift+drag` selects natively.
- **A2.** With the setting `true`, bare left-drag in vim/htop/tmux performs Warp's native selection (highlight visible in Warp).
- **A3.** With the setting `true`, after a bare-drag selection, `Cmd+C` copies the selected text to the clipboard.
- **A4.** With the setting `true`, `Option+left-drag` forwards to the TUI (inverse-modifier opt-out).
- **A5.** Right-click, scroll-wheel, middle-click, and `Cmd+left-click` behavior is unchanged in both modes.
- **A6.** Toggling the setting takes effect for the next mouse event without requiring an app restart.
- **A7.** The setting persists across app restarts (standard settings persistence).
- **A8.** The keybinding context flag `terminal_native_left_drag_select` reflects the current setting value.

## Implementation Pointers

A working reference implementation exists at https://github.com/spalagu/warp/tree/feat/left-drag-select-default. The pointers below mirror that branch's structure.

- **Setting definition.** Add `native_left_drag_select_enabled` to `app/src/terminal/alt_screen_reporting.rs` next to `mouse_reporting_enabled`.
- **Interception logic.** Extend `should_intercept_mouse()` in `app/src/terminal/alt_screen/mod.rs` with an `is_left_button: bool` parameter. When the setting is on and `is_left_button == true`, return `true` (intercept) for bare and `Shift`-modified events; return `false` when `Option` is held (forward to TUI).
- **Callsites to update (7 total).**
  - `app/src/terminal/block_list_element.rs` — 3 callsites.
  - `app/src/terminal/alt_screen/alt_screen_element.rs` — 4 callsites.
  - Each callsite must pass `is_left_button` derived from the originating mouse event.
- **Settings UI.** Add a toggle row in `app/src/settings_view/features_page.rs` under the existing AltScreenReporting section. Subtitle text per the Settings/API surface table above.
- **Keybinding context.** Expose `terminal_native_left_drag_select` in `app/src/settings_view/mod.rs` alongside other AltScreenReporting context flags.
- **Persistence.** Setting follows the existing AltScreenReporting persistence path; no new schema migration needed.

## Tests

- **T1.** Default-off behavior: with the setting `false`, simulate left-drag in a mouse-reporting alt-screen app; assert the event is forwarded to the TUI (no native selection rectangle).
- **T2.** Default-on bare-drag: with the setting `true`, simulate bare left-drag; assert Warp's native selection is created and the TUI receives no left button events.
- **T3.** `Cmd+C` after bare-drag: with the setting `true`, after a bare-drag selection, assert `Cmd+C` writes the selected text to the clipboard.
- **T4.** `Option+left-drag` forward: with the setting `true`, simulate `Option+left-drag`; assert the event is forwarded to the TUI and no native selection is created.
- **T5.** Other inputs unchanged: with both setting values, simulate right-click, scroll, middle-click, `Cmd+left-click`; assert behavior matches the OFF baseline in both modes.
- **T6.** Mid-session toggle: start with the setting `false`, toggle to `true` mid-session, assert next left-drag selects natively without restart.
- **T7.** Persistence: toggle the setting, restart, assert the value is preserved.
- **T8.** Context flag exposure: assert `terminal_native_left_drag_select` is queryable from the keybinding context and reflects the current setting value.

## Open Questions

- **Educational toast on first enable.** When a user enables the setting for the first time, should we surface a one-time dismissable toast explaining `Option+drag` as the forward escape hatch? Recommendation: **yes, dismissable**. iTerm2 users expect this; new users will not discover the modifier otherwise. Open for product input.
- **Naming alternatives.** `native_left_drag_select_enabled` is descriptive but long. Alternatives: `prefer_native_left_select`, `left_drag_selects_natively`. Recommendation: keep proposed name for clarity in settings search.

## Telemetry

No new telemetry events. The setting toggle reuses the existing `setting.changed` channel with key `terminal.native_left_drag_select_enabled`. Standard analytics on toggle frequency are sufficient to gauge adoption.
