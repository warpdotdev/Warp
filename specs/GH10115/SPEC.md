# Per-Window Zoom Level (GH-10115)

## Summary

Make `Cmd +` / `Cmd -` / `Cmd 0` (and equivalent Command Palette / menu actions) affect ONLY the focused OS window. Today these shortcuts mutate a single global zoom value that re-renders every open Warp window. After this change, each window owns its own zoom-level state. New windows open at the user's configured global default. Per-window zoom is in-memory only in V1 — closing and reopening the app resets all windows to the global default. Tabs and splits inside a single window continue to share that window's zoom.

## Problem

`Cmd +` in any Warp window currently zooms ALL Warp windows simultaneously because the zoom level is a single global setting. Users with multi-monitor setups, comparison workflows, or accessibility needs (e.g., a reading-focused window at higher zoom alongside a code-focused window at default zoom) have no way to keep one window large without enlarging text everywhere. The issue author has confirmed in-memory-only scoping is acceptable for V1 — persistence is not required to deliver value.

## Goals

- Keyboard shortcuts (`zoom_in`, `zoom_out`, `zoom_reset`) and any explicit `zoom_to_level` action mutate ONLY the focused window's zoom state.
- Command Palette zoom commands and View menu zoom items behave identically — focused-window-scoped.
- New windows open at the user's global zoom default (the value in Settings → Appearance → Zoom).
- All tabs and splits inside a single window share that window's zoom level. Switching tab or focused split inside the same window does not change the zoom.
- The Settings → Appearance → Zoom UI label is updated to disambiguate it as the default for new windows, not a global override.
- Optional one-shot "Apply to all open windows" button gives users a fast escape hatch when they want to re-sync everything.

## Non-Goals

- Per-pane / per-tab zoom inside a single window (V2 candidate at earliest).
- Per-monitor adaptive zoom that auto-scales when a window moves between displays.
- Persisting per-window zoom across app restart (V1.5 candidate — see Open Questions).
- Changing the underlying zoom mechanism, range, or step size.
- Touch / trackpad pinch-zoom semantics (already focused-window-scoped where supported).

## Behavior Contract

### B1. Zoom actions are focused-window-scoped

`zoom_in`, `zoom_out`, `zoom_reset`, and any `zoom_to_level(value)` action mutate the zoom-level field on the FOCUSED window's per-window state object only. Other open windows observe no change.

### B2. New windows inherit the global default

When a new Warp window is opened (via `Cmd N`, "New Window", or any other entry point), it initializes its zoom-level to the value of `appearance.zoom_level` (the global default). It does NOT inherit the focused window's current zoom.

### B3. Tabs and splits share window zoom

All tabs and splits inside a window share that window's zoom-level value. Creating a new tab or split, switching the active tab, or moving focus between splits does not change the rendered zoom. Moving a tab to a different window adopts the destination window's zoom.

### B4. In-memory only in V1

Per-window zoom state lives in the in-memory window state object. Closing and reopening the app resets every window to the global default. This matches the issue author's "in-memory only is acceptable" allowance and keeps V1 small.

### B5. Settings label disambiguation

The Settings → Appearance → Zoom control is relabeled to "Default zoom for new windows" (or equivalent localized string) so users understand changing it does not retro-apply to open windows.

### B6. Settings change does not retro-apply

Changing the global default in Settings ONLY affects subsequently-opened windows. Already-open windows keep their current per-window zoom. An explicit "Apply to all open windows" button next to the setting resets every open window to the new default value.

### B7. Clamping is per-window

Existing min/max zoom clamps (e.g., 0.5x..3.0x) apply per window. Each window can independently be at its own clamped value; reaching the max in one window does not affect another window's range.

## Settings / API surface

- `appearance.zoom_level` (existing): unchanged in schema, semantics, and storage. Now documented as "default for new windows". User-facing label updated to match.
- No new persisted settings in V1.
- Internal: per-window state object gains a `zoom_level: f32` field initialized from `appearance.zoom_level` at window-creation time.
- New UI control: "Apply to all open windows" button in Settings → Appearance → Zoom. Resets every open window's per-window zoom to the current global default.

## Acceptance Criteria

- A1. With windows A and B open, `Cmd +` while A is focused increases A's rendered zoom; B's rendered zoom is unchanged (visual diff).
- A2. With A at 1.5x and B at 1.0x, `Cmd 0` while A is focused resets A to global default; B remains at 1.0x.
- A3. Opening a new window C while A is at 1.5x and global default is 1.0x produces C at 1.0x.
- A4. Inside window A: switching tabs, switching splits, and creating new splits do not change A's rendered zoom.
- A5. Quitting and relaunching the app produces all windows at the global default.
- A6. Changing the global default from 1.0x to 1.25x does not change zoom in any already-open window.
- A7. Clicking "Apply to all open windows" resets every open window to the current global default.
- A8. Min/max clamps work per window: A can be at 3.0x while B is at 1.0x, and `Cmd +` in B still increases B without affecting A.

## Implementation Pointers

Verified in this codebase:

- `crates/warpui_core/src/zoom.rs` — current zoom implementation. Likely owns the global zoom state today.
- `app/src/appearance.rs` — appearance settings; holds `zoom_level` default.
- `app/src/settings_view/appearance_page.rs` — Settings UI page where the relabel and "Apply to all open windows" button land.
- `crates/warp_core/src/ui/appearance.rs` — appearance theming surface.

Likely-touched (verify during implementation):

- Per-window state container — where window-scoped state already lives. Add `zoom_level: f32` field initialized from the global default.
- Zoom action handlers — change call sites from "mutate global zoom" to "mutate focused window's zoom".
- Window-creation entry points — read `appearance.zoom_level` at creation time and seed the per-window field.

Net-new modules: none expected. The change is mostly relocating where the zoom value is stored and read, plus a small Settings UI addition.

## Tests

- T1. Unit: `zoom_in` action with multiple registered windows mutates only the focused window's zoom field.
- T2. Unit: opening a new window seeds its zoom from the global default, not from the currently-focused window.
- T3. Integration: tab switch and split focus changes inside a single window do not emit zoom updates.
- T4. Integration: app restart starts every window at the global default.
- T5. Integration: changing the global default does not emit zoom updates for open windows.
- T6. Integration: "Apply to all open windows" emits zoom updates for every open window.
- T7. Unit: per-window clamping — independent windows can sit at independent clamped values.
- T8. Visual / snapshot: render two windows at different zooms and assert they differ.

## Open Questions

- V1.5: should per-window zoom persist across app restart by mapping a stable window identity (e.g., saved-window-config ID, or display ID) to a stored zoom? Suggest yes, keyed by saved window/workspace config rather than ephemeral window instance.
- Should the "Apply to all open windows" button live next to the global default in Settings, or in the View menu, or both? Suggest Settings only for V1 to keep menu surface area small.
- Touch trackpad pinch-zoom: confirm it already mutates only the focused window. If not, fold into the same scoping change.

## Telemetry

No new events. If `zoom.changed` (or equivalent) already exists, extend its payload with a `scope: "window"` field so future analytics can distinguish per-window vs legacy global behavior.
