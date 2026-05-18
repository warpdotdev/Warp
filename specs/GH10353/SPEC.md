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
| `Cmd+left-click` and other already-defined modifier+left variants (e.g. link follow, macOS `Ctrl+left-click` right-click stand-in) | Existing Warp local handler runs (link follow, OS open, etc.). The new B3 routing latch does not subsume this. Routes via `MouseRoutingDecision::DeferToLocalHandler` — see Implementation Pointers. | No (handled locally; never forwarded to TUI). |

Click-count detection reuses Warp's existing click-count timing/tolerance heuristics — the spec does not redefine when consecutive clicks are coalesced into a double or triple click.

### B4. Other inputs unchanged

Regardless of setting value, the following continue to follow existing mouse-reporting rules:

- Right-click and right-drag.
- Scroll-wheel events.
- Middle-click and middle-drag.
- `Cmd+left-click` and other modifier+left-click variants that already have defined behavior (e.g., link follow).

When the setting is `true`, `Option+left-<anything>` (click, double-click, drag, etc.) MUST forward to the TUI as the inverse-modifier opt-out, mirroring iTerm2 semantics. This is the documented escape hatch for users who want to interact with the TUI's own selection (e.g., tmux pane resize handles).

### B4a. Modifier precedence (resolves combined-modifier ambiguity)

When more than one modifier is held on a left-button mouse-down, the dispatcher MUST evaluate the modifier rules in the following total order and return on the FIRST match. This precedence is the single source of truth and is enforced both in `should_intercept_mouse` (Implementation Pointers) and in the per-gesture latch:

1. **`Cmd`/`Meta` held** → `DeferToLocalHandler` (existing `Cmd+left-click` link-follow / OS-open path). Cmd dominates every other modifier regardless of setting value, so `Cmd+Shift+left`, `Cmd+Option+left`, and `Cmd+Shift+Option+left` all defer to the local handler.
2. **macOS-only: `Ctrl` held without `Cmd`** → `DeferToLocalHandler` (existing right-click stand-in handler). Ctrl outranks Shift and Option on macOS so `Ctrl+Shift+left` and `Ctrl+Option+left` both reach the local right-click stand-in path.
3. **`Shift` held (no `Cmd`, no `Ctrl`)** → `InterceptByWarp` (Shift-bypass invariant; preserved in both setting states). Shift outranks Option, so:
   - `Shift+left` → `InterceptByWarp` regardless of setting.
   - **`Option+Shift+left` → `InterceptByWarp`** (the Shift-bypass wins; the inverse-modifier in B4 does NOT apply when Shift is also held). This is the explicit rule resolving the round-N concern: when both Option and Shift are held with bare left, the gesture is treated as a Shift-bypass native selection and is NEVER forwarded to the TUI. Rationale: Shift+drag is the long-established native-select escape hatch and users routinely add Option mid-drag (window manager habits) without intending to change routing.
4. **`Option`/`Alt` held (no `Cmd`, no `Ctrl`, no `Shift`) AND setting `true`** → `ForwardToTui` (B4 inverse-modifier opt-out).
5. **`Option`/`Alt` held (no `Cmd`, no `Ctrl`, no `Shift`) AND setting `false`** → `ForwardToTui` (existing behavior; Option is not special when the setting is off).
6. **Bare left (no modifiers) AND setting `true` AND alt-screen + mouse-reporting** → `InterceptByWarp` (B3).
7. **Bare left (no modifiers) AND setting `false`** → `ForwardToTui` (B2 — existing behavior).
8. **Otherwise** (bare left, setting `true`, but NOT alt-screen or NOT mouse-reporting) → `InterceptByWarp` (already the existing native-select path outside alt-screen, included here for completeness).

This precedence is invariant under settings toggles mid-gesture (B6 latch) — the table is evaluated exactly once per gesture at mouse-down.

### B5. Live toggle

The setting takes effect immediately for new mouse-down events that begin AFTER the user changes it. Setting changes that occur during an in-flight gesture do not change that gesture's routing — see B6.

### B5a. First-enable educational toast (V1: yes)

The first time a user enables `terminal.native_left_drag_select_enabled` after install, Warp shows a one-time educational toast explaining the inverse-modifier escape hatch. This was an open question in round 1; V1 ships it.

- **Trigger.** Fires once per Warp install on the first transition of `terminal.native_left_drag_select_enabled` from `false` → `true`.
- **Content (literal).** *"Bare left-drag now selects text natively. To forward drag to TUI applications, hold Option (⌥) while dragging."*
- **Style.** Standard Warp toast in the existing notification region; non-blocking; visually consistent with other settings-toggle toasts.
- **Dismissal.**
  - Auto-dismisses after **8 seconds**, OR
  - On user click anywhere on the toast (including its close affordance).
- **Persistence — never shown again after first time.** Tracked via a local sidecar at `~/.config/warp/seen_toasts.json` keyed by toast id `native_left_drag_select_first_enable`. The sidecar is local-only (NOT cloud-synced) so disabling and re-enabling the setting on the same install does not re-show the toast. Disabling-then-re-enabling on a fresh install on a different machine MAY re-show it (this is intentional — local sidecar is per install).
- **Setting interaction.**
  - If the user toggles to `true`, sees the toast, then toggles to `false` and back to `true` later, the toast does NOT show again.
  - If the user toggles to `true` for the first time while a left-button gesture is in flight, the toast appears AFTER the in-flight gesture ends (the routing latch in B6 is unaffected).
- **Telemetry note.** No new telemetry fields. The toast emits the existing `toast_shown` and `toast_dismissed` events with id `native_left_drag_select_first_enable` if those events already exist; otherwise no telemetry beyond the existing `setting.changed` already covered.

### B6. Routing latch (in-flight gesture invariant)

The route for any left-button gesture is decided ONCE at the mouse-down event and LATCHED for the rest of the gesture's lifetime.

- At mouse-down, the dispatcher reads:
  1. The current value of `terminal.native_left_drag_select_enabled`.
  2. The current modifier state (`Shift`, `Option`, `Cmd`, `Ctrl`).
  3. The current alt-screen + mouse-reporting status of the active terminal.
  4. The current **shared-session-reader status** of the active terminal — i.e., whether this pane is the canonical input writer for a shared session, or a read-only follower. Read-only follower panes MUST NOT forward mouse events to the TUI under any setting value; they always compute either `InterceptByWarp` (bare/Shift left, setting on) or `DeferToLocalHandler` (modifier-keyed left with existing local handler). `ForwardToTui` is computed only when this pane is the canonical writer.
- From those inputs it computes **one of three terminal routing decisions for a fresh mouse-down**: `InterceptByWarp`, `DeferToLocalHandler`, or `ForwardToTui` (matching the `MouseRoutingDecision` enum in Implementation Pointers; the fourth variant `FollowExistingLatch` is reserved for subsequent `MouseMove` / `MouseUp` events in the same gesture and is never produced for a fresh `MouseDown`). That decision is stored against the gesture and applied to every subsequent mouse-move and mouse-up event in the same gesture — the dispatcher returns `FollowExistingLatch` for those events so callers know to consult the per-gesture map.
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
- **A12.** Combined-modifier precedence: with the setting `true`, `Option+Shift+left-drag` performs native selection in Warp (Shift-bypass wins; the gesture is NOT forwarded to the TUI). `Cmd+Option+left-click` follows the link-follow / OS-open local handler (Cmd dominates). `Ctrl+Option+left-click` on macOS follows the right-click stand-in local handler. None of these gestures are forwarded to the TUI.
- **A13.** Shared-session read-only follower: in a pane with `SessionRole == ReadOnlyFollower`, bare left-drag with the setting `true` performs native selection, AND bare left-drag with the setting `false` ALSO performs native selection (the follower never forwards mouse events to the TUI). `Option+left-drag` in the follower with the setting `true` ALSO performs native selection rather than forwarding. The canonical writer pane in the same shared session continues to behave per A1–A12.
- **A14.** No idle-timeout cancellation: a left-button mouse-down followed by ≥ 1 s of motionless hold before the user begins to drag MUST behave identically to a normal drag — the latch from the original mouse-down MUST still be in effect, and the subsequent drag and up events MUST be routed per that latch. Tested at 1 s, 5 s, and 25 s holds.

## Implementation Pointers

A working reference implementation exists at https://github.com/spalagu/warp/tree/feat/left-drag-select-default. The pointers below mirror that branch's structure.

- **Setting definition.** Add `native_left_drag_select_enabled` to `app/src/terminal/alt_screen_reporting.rs` next to `mouse_reporting_enabled`.
- **Interception logic.** Extend `should_intercept_mouse()` in `app/src/terminal/alt_screen/mod.rs` (verified existing function: `pub fn should_intercept_mouse(model: &TerminalModel, shift: bool, ctx: &AppContext) -> bool`) so that it carries enough modifier and event-kind information to preserve every existing modifier-keyed left-click behavior (e.g., `Cmd+left-click` link follow, OS-specific link openers, future modifier handlers). Concretely, the function MUST take a single `MouseEventContext` struct rather than a single `shift: bool`, with the following fields:

  ```rust
  pub struct MouseEventContext {
      pub is_left_button: bool,                 // true for Left / LeftDrag
      pub event_kind: MouseEventKind,           // MouseDown | MouseMove | MouseUp
      pub modifiers: ModifierState,             // shift, ctrl, alt_or_option, cmd_or_meta
      pub click_count: u8,                      // 1 for single, 2 double, 3 triple, etc.
      pub gesture_id: GestureId,                // see "gesture_id source" below
      pub session_role: SessionRole,            // CanonicalWriter | ReadOnlyFollower
                                                // (shared-session-reader status — see B6/4)
  }

  pub enum SessionRole {
      CanonicalWriter,   // this pane owns the PTY write end; TUI forwarding is permitted
      ReadOnlyFollower,  // shared-session viewer; TUI forwarding is suppressed
  }
  ```

  Return type changes from `bool` to a tetra-state
  `MouseRoutingDecision` so that pre-existing local Warp handlers
  (link follow on `Cmd+click`, future modifier+left handlers) are
  NOT lumped in with TUI forwarding:

  ```rust
  pub enum MouseRoutingDecision {
      InterceptByWarp,        // Warp native selection / caret model
      DeferToLocalHandler,    // Warp's existing modifier+left handler
                              // (e.g. Cmd+click link follow). NOT
                              // forwarded to the TUI.
      ForwardToTui,           // mouse-reporting forward
      FollowExistingLatch,    // mid-gesture move/up: read the
                              // per-gesture latch
  }
  ```

  Routing rules expressed against `MouseEventContext`. The rules
  are listed in evaluation order; the function returns on the
  FIRST matching rule. This order is normative and matches the
  precedence table in B4a:
  - `is_left_button == false` → return `ForwardToTui` (existing
    right/middle/scroll rules apply elsewhere; this function does
    not own them).
  - `is_left_button == true && event_kind != MouseDown` → return
    `FollowExistingLatch` (mouse-move and mouse-up consult the
    per-gesture map; do not recompute).
  - `is_left_button == true && event_kind == MouseDown &&
    session_role == ReadOnlyFollower` → the function MUST NOT
    return `ForwardToTui` for this pane. Continue evaluating the
    remaining rules below; any rule that would return
    `ForwardToTui` is rewritten to return `InterceptByWarp`
    instead. `DeferToLocalHandler` returns are unaffected. This
    preserves the existing shared-session invariant that only the
    canonical writer can drive TUI input.
  - `is_left_button == true && event_kind == MouseDown &&
    modifiers.cmd_or_meta` → return `DeferToLocalHandler`. This
    preserves Warp's existing `Cmd+left-click` handler (link
    follow, OS open) — the click is NOT forwarded to the TUI and
    is NOT intercepted as native selection. The new B3/B4 routing
    latch does NOT subsume modifier-keyed left clicks. The same
    handling applies to `Ctrl+left-click` on macOS (which Warp
    today treats as a right-click stand-in via a local handler):
    `DeferToLocalHandler` so the existing right-click-stand-in
    handler runs, NOT `ForwardToTui`.
  - `is_left_button == true && event_kind == MouseDown &&
    modifiers.alt_or_option && setting_on` → return `ForwardToTui`
    (B4 inverse-modifier opt-out).
  - `is_left_button == true && event_kind == MouseDown &&
    modifiers.shift` → return `InterceptByWarp` (existing
    Shift-bypass; preserved in both setting states).
  - `is_left_button == true && event_kind == MouseDown &&
    setting_on && bare-or-shift-only` → return `InterceptByWarp`.
  - Otherwise → return `ForwardToTui`.

  Any future left-button modifier handler MUST add an explicit
  branch in this function before the bare-left rule, returning
  `DeferToLocalHandler` (not `ForwardToTui`) if it is owned by
  Warp's local code path. The function MUST consider the FULL
  modifier state, not just `is_left_button`.
- **Per-gesture routing latch.** The latch lives at the mouse-event-dispatch layer (the same site that decides intercept-vs-forward today, i.e. the callers of `should_intercept_mouse`). Maintain a per-gesture routing map keyed on `(button, gesture_id)`:
  - On left-button mouse-down: compute the routing decision from `should_intercept_mouse(...)` once, insert `(Left, gesture_id) → Decision` into the map.
  - On any subsequent left-button mouse-move / mouse-up belonging to the same gesture: read the decision from the map; do NOT recompute via `should_intercept_mouse`.
  - On gesture end (mouse-up or gesture-cancel/focus-loss/mouse-capture-loss): remove the entry.
  - The map is local to the dispatch layer; it does not cross terminal boundaries. Right/middle/scroll buttons each maintain their own per-gesture entries if they need analogous semantics, but those follow existing rules and are out of scope for this spec.

- **gesture_id source (where the identifier comes from).** The dispatch layer that already feeds `should_intercept_mouse` receives `Event::LeftMouseDown { position, click_count, modifiers, .. }`, `Event::LeftMouseDragged { position, modifiers, .. }`, and `Event::LeftMouseUp { position, modifiers, .. }` (verified call sites: `app/src/terminal/alt_screen/alt_screen_element.rs:889-941`). The current event stream does NOT expose a stable per-gesture identifier today, so V1 introduces a small new module:

  - **`GestureSession` `(new)`** — lives in the existing event-dispatch crate alongside the input event types (target module: `app/src/terminal/input/gesture.rs` `(new module)`, re-exported from `app/src/terminal/input/mod.rs`). Owned by the same struct that today consumes `Event::LeftMouseDown`/`Dragged`/`Up`. Responsibilities:
    1. On every `Event::LeftMouseDown`, increment a `u64` counter and assign that value as the gesture's `GestureId`.
    2. Track the active left-button gesture in an `Option<ActiveGesture { id: GestureId, last_seen_at: Instant }>` field.
    3. Tag every subsequent `Event::LeftMouseDragged` and `Event::LeftMouseUp` with the same `GestureId` until any of the following gesture-end signals arrives. Note: there is **no time-based idle timeout** — a user may legitimately hold the left button motionless for many seconds before starting to drag (e.g., reading text under the cursor, long-press-then-drag, system context menu hesitation). Releasing the latch on a wall-clock idle would invalidate those gestures and turn a Warp-intercepted long-press into a TUI-forwarded drag (or vice versa) mid-stroke, violating B6. The active gesture is therefore released ONLY on one of these OS- or app-level events:
       - `Event::LeftMouseUp` arrives — primary path. Latch released, `ActiveGesture = None`.
       - A `focus-loss` / `window-deactivate` / `mouse-capture-loss` / `pointer-leave-window` event arrives from the platform — the OS has told us the gesture is no longer ours. Release the latch.
       - A new `Event::LeftMouseDown` arrives while `ActiveGesture` is still `Some(_)` (defensive recovery — should be impossible in a well-behaved platform but handles the case where the platform dropped the prior mouse-up). The prior gesture's entry is removed from the routing map and the new `MouseDown` allocates a fresh `GestureId` and computes a fresh routing decision.
       - The owning input dispatcher is dropped (terminal closed, pane removed) — bulk cleanup of any outstanding `ActiveGesture`.
       - **(Optional, far-side safety net only.)** Implementations MAY release the latch after a generous `30 s` of no `LeftMouseDragged`/`LeftMouseUp`/`LeftMouseDown` for the active gesture. Thirty seconds is chosen to be safely longer than any plausible legitimate long-press-then-drag while still bounding the lifetime of a leaked id if the platform never emits a mouse-up or focus-loss. The 30 s timer is OPTIONAL because the events above already cover every documented platform path on macOS/Linux/Windows; the previous round's 200 ms value is explicitly NOT permitted as it cancels legitimate long-press-then-drag gestures within the human reaction-time window.
    4. After release, also drop the corresponding entry from the routing map maintained above.

  - **Platform integration.** On platforms where the windowing layer already exposes a stable per-pointer-interaction identifier (e.g., macOS `NSEvent.eventNumber` for a button-down sequence; Linux/Windows winit `DeviceId` plus a sequence counter), `GestureSession` MAY adopt that platform id directly instead of allocating its own counter. V1 does not require platform parity here — the internal counter is the authoritative source; platform ids are an optimization.

  - **No re-entrancy across terminals.** `GestureSession` is per-input-dispatcher (one per terminal-input owner). A second terminal pane has its own `GestureSession`. Right/middle/scroll buttons do NOT participate in `GestureSession`; they keep their existing rules.

  - **Why a new module instead of reusing existing event ids.** The existing `Event::LeftMouseDown`/`Dragged`/`Up` carry no shared correlation field today (verified). Without `GestureSession` the per-gesture latch cannot be keyed reliably; mid-drag `should_intercept_mouse` calls would have no way to consult the original mouse-down's decision. The `(new)` marker in this section reflects that this struct does not exist yet and is introduced by this spec.
- **Callsites to update (7 total).**
  - `app/src/terminal/block_list_element.rs` — 3 callsites.
  - `app/src/terminal/alt_screen/alt_screen_element.rs` — 4 callsites.
  - Each callsite MUST construct a full `MouseEventContext` from
    the originating mouse event and pass it to
    `should_intercept_mouse(...)`. Concretely, the callsite is
    responsible for populating EVERY field of `MouseEventContext`:
    - `is_left_button` — `true` only for `LeftMouseDown`,
      `LeftMouseDragged`, `LeftMouseUp`.
    - `event_kind` — mapped from the originating event variant
      (`MouseDown` for `LeftMouseDown`, `MouseMove` for
      `LeftMouseDragged`, `MouseUp` for `LeftMouseUp`).
    - `modifiers` — the FULL `ModifierState` from the originating
      event (`shift`, `ctrl`, `alt_or_option`, `cmd_or_meta`).
      Callsites MUST NOT pass a partial modifier state (e.g.
      `option_held` only); doing so would silently break
      `Shift+drag`, `Cmd+click`, and any future modifier handler.
    - `click_count` — the click count from the same click-count
      detector that already drives word-/line-select outside
      alt-screen. Required at every callsite so double/triple
      semantics in B3 work correctly.
    - `gesture_id` — obtained from `GestureSession` (see
      "gesture_id source" above): the active id for `MouseMove`
      / `MouseUp`, the newly-allocated id for `MouseDown`. Never
      `Default::default()`; never absent.
  - On `MouseDown` the callsite computes the routing decision via
    `should_intercept_mouse(...)` and inserts
    `(Left, gesture_id) → Decision` into the per-gesture routing
    map BEFORE acting on the decision. On subsequent `MouseMove` /
    `MouseUp` the callsite reads the latched decision from the map
    and dispatches accordingly (`InterceptByWarp` →
    Warp selection model; `DeferToLocalHandler` → existing
    Warp local handler; `ForwardToTui` → mouse-reporting
    forward). It does NOT recompute `should_intercept_mouse` for
    move/up.
  - Right-click, middle-click, and scroll callsites are NOT in
    this list and continue to use their existing routing.
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
- **T15.** Combined modifier precedence — `Option+Shift+left-drag`: with the setting `true` in an alt-screen mouse-reporting app, simulate `Option+Shift+left-down` → `Option+Shift+left-drag` → `Option+Shift+left-up`. Assert (a) Warp's native selection is created, (b) the TUI receives ZERO left-button events, (c) the latch resolved to `InterceptByWarp` (Shift-bypass wins over Option inverse). Verifies A12 and B4a rule 3.
- **T16.** Combined modifier precedence — `Cmd+Option+left-click` and `Ctrl+Option+left-click`: with the setting `true` and a clickable link under the cursor, simulate `Cmd+Option+left-click`; assert the existing link-follow handler fires and the TUI receives ZERO left-button events. On macOS, simulate `Ctrl+Option+left-click`; assert the existing right-click-stand-in handler fires and the TUI receives ZERO left-button events. Verifies A12 and B4a rules 1–2.
- **T17.** Shared-session read-only follower never forwards: open a shared session with one canonical writer and one read-only follower pane. On the follower pane with the setting `false`, simulate bare left-drag; assert native selection is performed in the follower AND no PTY input is written. Repeat with the setting `true`; assert the same. Repeat with `Option+left-drag` and the setting `true`; assert native selection is performed in the follower (Option inverse-modifier is suppressed for followers). On the canonical writer pane, repeat the same three gestures and assert behavior matches A1, A2, A4 respectively. Verifies A13 and B6 input 4.
- **T18.** No idle-timeout invalidates long-press-then-drag: with the setting `true`, simulate a bare left-down, then idle for 1 s with no events, then begin and complete a drag. Assert the drag is intercepted by Warp (latch from the original mouse-down still in effect). Repeat with 5 s and 25 s idle holds; assert the same outcome. Repeat all three with the setting `false` and bare left-down (forward-to-TUI latch); assert all three drag events are forwarded to the TUI. Verifies A14 and the no-idle-timeout rule in `GestureSession`.
- **T_first_enable_toast.** First-enable toast appears once: with a clean install (no `~/.config/warp/seen_toasts.json` entry for `native_left_drag_select_first_enable`), toggle the setting `false` → `true` and assert (a) the toast appears with the literal copy from B5a, (b) it auto-dismisses after 8s when no click occurs, (c) the sidecar gains the entry, (d) toggling `true` → `false` → `true` again does NOT show the toast a second time on the same install, (e) clicking the toast within the 8s window dismisses it immediately and still records the sidecar entry.

## Open Questions

- **Naming alternatives.** `native_left_drag_select_enabled` is descriptive but long. Alternatives: `prefer_native_left_select`, `left_drag_selects_natively`. Recommendation: keep proposed name for clarity in settings search.

(The previous Open Question about the first-enable educational toast is now resolved as **yes** in V1 — see B5a above.)

## Telemetry

No new telemetry events. The setting toggle reuses the existing `setting.changed` channel with key `terminal.native_left_drag_select_enabled`. Standard analytics on toggle frequency are sufficient to gauge adoption.
