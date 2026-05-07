# Spec: Horizontal scrolling per terminal pane (GH-9828)

## Problem

Wide diff hunks, long log lines, and `tree` output regularly
exceed the visible width of a narrow pane. Today, lines wrap (or
truncate, depending on settings), preventing a quick lateral
glance at content beyond the viewport. Users want trackpad-swipe
horizontal scroll to peek at long lines without resizing the pane
or switching focus.

## Goal

Add per-pane horizontal scrolling for the terminal output grid
when the longest line in the visible buffer range exceeds pane
width AND the user has line-wrap disabled. Trackpad horizontal
deltas, Shift-wheel, and a horizontal scrollbar all drive it.

## Behavior contract

- B1. Horizontal scrolling is gated on:
  - The pane's `terminal.line_wrap = false` setting (existing).
  - At least one visible row exceeding pane width.
  When either condition is false, the pane behaves as today (no
  horizontal scroll, no scrollbar).
- B1a. `h_offset` is a logical column offset clamped to
  `max(visible_longest_line_columns - visible_pane_columns, 0)`.
  Recompute that maximum whenever the visible buffer range changes,
  the pane resizes, line-wrap toggles, or rendered line contents
  change. If the maximum becomes 0 or line-wrap is enabled, reset
  `h_offset` to 0 and hide the horizontal scrollbar. If the maximum
  shrinks but remains positive, clamp `h_offset` down to the new
  maximum so blank shifted columns are never rendered.
- B2. Horizontal scroll inputs:
  - Trackpad two-finger horizontal swipe (native macOS/Windows).
  - Shift + scroll wheel.
  - A new horizontal scrollbar at the bottom of the pane,
    visible only when content exceeds pane width.
- B3. Scroll position is per-pane and persists across the pane's
  lifetime (resets on pane close).
- B4. Vertical scroll behavior is unchanged. The two axes are
  independent.
- B5. Selection and click-to-position respect the horizontal
  offset — the user clicks where they see, not where the
  underlying buffer column is.
- B6. Auto-scroll on new output: by default the pane snaps back
  to horizontal offset 0 only when all of these are true:
  - `terminal.h_scroll_snap_on_new_output = true`.
  - The terminal is pinned to the live bottom when the output event
    is applied, not viewing scrollback.
  - The output event advances terminal content by appending a
    completed rendered row, creating a new prompt, or creating a new
    command block.
  Do not snap for partial writes to the current row, prompt redraws
  that do not advance the rendered row, cursor-only updates,
  alternate-screen/TUI updates, or output that arrives while the
  user is viewing scrollback.
- B7. Persist `terminal.h_scroll_snap_on_new_output` in the
  existing `TerminalSettings` settings group
  (`app/src/terminal/settings.rs`) with TOML path
  `terminal.h_scroll_snap_on_new_output`, default `true`, and the
  same global sync behavior as other non-private terminal settings.
  V1 exposes it through the settings file/schema only; no first-class
  Settings UI toggle is required.

## Acceptance criteria

- A1. With line-wrap off and a long line visible, a two-finger
  horizontal swipe scrolls the line; the rest of the pane scrolls
  in lockstep so columns stay aligned.
- A2. Selection across a horizontally-scrolled region copies the
  underlying logical text, not the visible-only fragment.
- A3. With line-wrap on (today's default), no horizontal scroll
  affordance appears.
- A4. While pinned to the live bottom, a completed new shell output
  row snaps back to offset 0 when
  `h_scroll_snap_on_new_output = true`.
- A5. Horizontal scrollbar appears only when content exceeds pane
  width.
- A6. With `h_scroll_snap_on_new_output = false`, completed new
  shell output preserves the current horizontal offset.
- A7. Resizing the pane, toggling line-wrap on, or vertically
  scrolling to shorter visible content clamps or resets `h_offset`
  according to B1a, with no blank shifted columns.

## Implementation pointers

- Terminal grid render lives in `app/src/terminal/...`. The grid
  already tracks per-pane viewport state; horizontal offset is a
  new field there.
- Selection logic in `app/src/terminal/model/blocks/selection.rs`
  uses buffer coordinates (column index), so the horizontal-offset
  → buffer-column mapping is the only adjustment in the click /
  drag path.
- Add the snap setting beside other terminal settings in
  `app/src/terminal/settings.rs`, using generated settings schema
  and TOML persistence rather than a custom config path.

## Test plan

- T1. Render with a long line + line-wrap-off + h-offset = 5;
  rendered cells start at column 5.
- T2. Two-finger horizontal swipe event increments h_offset.
- T3. Selection across an h-scrolled region returns logical text.
- T4. While pinned to live bottom, completed new output snaps back
  to offset 0 with the default snap setting.
- T5. With line-wrap on, h-scroll inputs are ignored.
- T6. With snap disabled, completed new output preserves h_offset.
- T7. Resize / line-wrap toggle / vertical scroll to shorter
  content clamps or resets h_offset and hides the scrollbar when
  max offset is 0.
- T8. Partial-row writes, prompt redraws, alternate-screen updates,
  and output received while viewing scrollback preserve h_offset.

## Out of scope

- Horizontal scroll inside notebook cells (different code path,
  follow-up).
- Auto-detect long lines and offer a toast suggesting line-wrap
  toggle.
- Horizontal scrollbar styling customization.
