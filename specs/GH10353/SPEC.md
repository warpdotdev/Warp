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
- Take effect immediately for new mouse events after toggling — no app restart, but in-flight gestures remain consistent (see B6 routing latch).

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

### B3. Setting ON — bare-left click semantics

In alt-screen + mouse-reporting-enabled apps with `terminal.native_left_drag_select_enabled = true`, every bare-left interaction is INTERCEPTED by Warp's native selection. The TUI receives no bare-left button events. To forward bare-left to the TUI in ON mode, the user holds `Option` (see B4).

The full set of bare-left gestures and their ON-mode behavior:

| Gesture | ON-mode behavior | Forwarded to TUI? |
|---|---|---|
| Single bare left-click (down then up, no movement) | Intercepted by Warp; clears any existing native selection; places editor caret in Warp's selection model at the click position. | No. |
| Bare left mouse-down then up without movement (alias of single click) | Same as single click. | No. |
| Double bare left-click | Intercepted; performs Warp's native word-select on the clicked position (existing native word-select code path). | No. |
| Triple bare left-click | Intercepted; performs Warp's native line-select. | No. |
| Quadruple-or-more bare left-click | Intercepted; performs paragraph-select if Warp's native selection model exposes it; otherwise clamps to triple-click line-select semantics. | No. |
| Drag (down → move → up) | Intercepted; Warp's native drag-select (existing B3 contract). | No. |
| `Shift+left-drag` | Intercepted; native selection (no regression from Setting OFF). | No. |
| `Option+left-<anything>` (click, drag, etc.) | See B4 — inverse modifier forwards to TUI. | Yes. |
| `Cmd+left-click` and other already-defined modifier+left variants (e.g. link follow) | Existing behavior preserved (B4 also). | Per existing rules. |

Click-count detection reuses Warp's existing click-count timing/tolerance heuristics — the spec does not redefine when consecutive clicks are coalesced into a double or triple click.

### B4. Other inputs unchanged

Regardless of setting value, the following continue to follow existing mouse-reporting rules:

- Right-click and right-drag.
- Scroll-wheel events.
- Middle-click and middle-drag.
- `Cmd+left-click` and other modifier+left-click variants that already have defined behavior (e.g., link follow).

When the setting is `true`, `Option+left-<anything>` (click, double-click, drag, etc.) MUST forward to the TUI as the inverse-modifier opt-out, mirroring iTerm2 semantics. This is the documented escape hatch for users who want to interact with the TUI's own selection (e.g., tmux pane resize handles).

### B5. Live toggle

The setting takes effect immediately for new mouse-down events that begin AFTER the user changes it. Setting changes that occur during an in-flight gesture do not change that gesture's routing — see B6.

### B6. Routing latch (in-flight gesture invariant)

The route for any left-button gesture is decided ONCE at the mouse-down event and LATCHED for the rest of the gesture's lifetime.

- At mouse-down, the dispatcher reads:
  1. The current value of `terminal.native_left_drag_select_enabled`.
  2. The current modifier state (`Shift`, `Option`, `Cmd`).
  3. The current alt-screen + mouse-reporting status of the active terminal.
- From those inputs it computes one of two routing decisions: **intercept-by-Warp** or **forward-to-TUI**. That decision is stored against the gesture and applied to every subsequent mouse-move and mouse-up event in the same gesture.
- The latch holds until the gesture ends (mouse-up, or platform-specific gesture-end events such as drag-cancel, focus loss, mouse capture loss). On gesture end, the latch is released.
- Mid-gesture changes that DO NOT switch routing of the in-flight gesture:
  - User toggles `terminal.native_left_drag_select_enabled` between mouse-down and mouse-up.
  - User releases or presses `Option`, `Shift`, or `Cmd` between mouse-down and mouse-up.
  - The TUI sends DECSET/DECRST that turns mouse-reporting on/off mid-gesture.
- New gestures that start AFTER any of those changes use the new state when computing their own latch.

The latch is per-gesture, not per-button-globally: a fresh left-button mouse-down always starts a new latch. Right/middle/scroll gestures are unaffected by this latch (they keep following existing rules independently).

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
- **A6.** Toggling the setting takes effect for the next mouse-down event without requiring an app restart.
- **A7.** The setting persists across app restarts (standard settings persistence).
- **A8.** The keybinding context flag `terminal_native_left_drag_select` reflects the current setting value.
- **A9.** With the setting `true`, single, double, and triple bare-left-clicks each match the row in B3: caret-place, word-select, line-select. None of these clicks are forwarded to the TUI.
- **A10.** Routing latch invariant: toggling `terminal.native_left_drag_select_enabled` during an in-flight left-button gesture does NOT change that gesture's routing; the next gesture uses the new value.
- **A11.** Routing latch invariant for modifiers: releasing or pressing `Option` (or `Shift`/`Cmd`) during an in-flight left-button gesture does NOT change that gesture's routing.

## Implementation Pointers

A working reference implementation exists at https://github.com/spalagu/warp/tree/feat/left-drag-select-default. The pointers below mirror that branch's structure.

- **Setting definition.** Add `native_left_drag_select_enabled` to `app/src/terminal/alt_screen_reporting.rs` next to `mouse_reporting_enabled`.
- **Interception logic.** Extend `should_intercept_mouse()` in `app/src/terminal/alt_screen/mod.rs` (verified existing function: `pub fn should_intercept_mouse(model: &TerminalModel, shift: bool, ctx: &AppContext) -> bool`) with the additional inputs needed for left-button routing: `is_left_button: bool` and `option_held: bool`. When the setting is on and `is_left_button == true`, return `true` (intercept) for bare and `Shift`-modified events; return `false` when `Option` is held (forward to TUI).
- **Per-gesture routing latch.** The latch lives at the mouse-event-dispatch layer (the same site that decides intercept-vs-forward today, i.e. the callers of `should_intercept_mouse`). Maintain a per-gesture routing map keyed on `(button, gesture_id)`:
  - On left-button mouse-down: compute the routing decision from `should_intercept_mouse(...)` once, insert `(Left, gesture_id) → Decision` into the map.
  - On any subsequent left-button mouse-move / mouse-up belonging to the same gesture: read the decision from the map; do NOT recompute via `should_intercept_mouse`.
  - On gesture end (mouse-up or gesture-cancel/focus-loss/mouse-capture-loss): remove the entry.
  - The map is local to the dispatch layer; it does not cross terminal boundaries. Right/middle/scroll buttons each maintain their own per-gesture entries if they need analogous semantics, but those follow existing rules and are out of scope for this spec.
- **Callsites to update (7 total).**
  - `app/src/terminal/block_list_element.rs` — 3 callsites.
  - `app/src/terminal/alt_screen/alt_screen_element.rs` — 4 callsites.
  - Each callsite must pass `is_left_button` and `option_held` derived from the originating mouse event AND consult the per-gesture routing map before recomputing.
- **Click-count semantics.** Reuse Warp's existing click-count detection (the same path that today drives word-select on double-click outside alt-screen). No new click-count tracker is introduced.
- **Settings UI.** Add a toggle row in `app/src/settings_view/features_page.rs` under the existing AltScreenReporting section. Subtitle text per the Settings/API surface table above.
- **Keybinding context.** Expose `terminal_native_left_drag_select` in `app/src/settings_view/mod.rs` alongside other AltScreenReporting context flags.
- **Persistence.** Setting follows the existing AltScreenReporting persistence path; no new schema migration needed.

## Tests

- **T1.** Default-off behavior: with the setting `false`, simulate left-drag in a mouse-reporting alt-screen app; assert the event is forwarded to the TUI (no native selection rectangle).
- **T2.** Default-on bare-drag: with the setting `true`, simulate bare left-drag; assert Warp's native selection is created and the TUI receives no left button events.
- **T3.** `Cmd+C` after bare-drag: with the setting `true`, after a bare-drag selection, assert `Cmd+C` writes the selected text to the clipboard.
- **T4.** `Option+left-drag` forward: with the setting `true`, simulate `Option+left-drag`; assert the event is forwarded to the TUI and no native selection is created.
- **T5.** Other inputs unchanged: with both setting values, simulate right-click, scroll, middle-click, `Cmd+left-click`; assert behavior matches the OFF baseline in both modes.
- **T6.** Mid-session toggle: start with the setting `false`, toggle to `true` mid-session, assert next left-drag (one started AFTER the toggle) selects natively without restart.
- **T7.** Persistence: toggle the setting, restart, assert the value is preserved.
- **T8.** Context flag exposure: assert `terminal_native_left_drag_select` is queryable from the keybinding context and reflects the current setting value.
- **T9.** Single bare-click in ON mode: with the setting `true`, simulate a bare left-down then left-up at the same position; assert Warp places the editor caret in its native selection model and the TUI receives no left button events.
- **T10.** Double bare-click in ON mode: with the setting `true`, simulate two bare left-clicks within the click-count window; assert Warp performs native word-select and the TUI receives no left button events.
- **T11.** Triple bare-click in ON mode: with the setting `true`, simulate three bare left-clicks within the click-count window; assert Warp performs native line-select and the TUI receives no left button events.
- **T12.** Routing latch — setting toggle mid-drag: with the setting `false`, send a left-down event (latches to forward-to-TUI); toggle the setting to `true`; send the corresponding mouse-move and mouse-up. Assert all three events were forwarded to the TUI and Warp produced no native selection. Repeat the inverse: start with the setting `true`, latch to intercept, toggle to `false` mid-drag, assert Warp's native selection completes through mouse-up.
- **T13.** Routing latch — modifier release mid-drag in ON mode: with the setting `true`, send a bare left-down (latches to intercept), simulate `Option` being pressed mid-drag; assert the in-flight gesture remains intercepted by Warp through mouse-up. Inverse: send `Option+left-down` (latches to forward), release `Option` mid-drag; assert the gesture is still forwarded to the TUI through mouse-up.
- **T14.** New gesture after toggle uses new value: after T12 completes, the very next mouse-down should compute its latch from the now-current setting value (and current modifier state), not from the previous gesture's latch. Verified for both toggle directions.

## Open Questions

- **Educational toast on first enable.** When a user enables the setting for the first time, should we surface a one-time dismissable toast explaining `Option+drag` as the forward escape hatch? Recommendation: **yes, dismissable**. iTerm2 users expect this; new users will not discover the modifier otherwise. Open for product input.
- **Naming alternatives.** `native_left_drag_select_enabled` is descriptive but long. Alternatives: `prefer_native_left_select`, `left_drag_selects_natively`. Recommendation: keep proposed name for clarity in settings search.

## Telemetry

No new telemetry events. The setting toggle reuses the existing `setting.changed` channel with key `terminal.native_left_drag_select_enabled`. Standard analytics on toggle frequency are sufficient to gauge adoption.
