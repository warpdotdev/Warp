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
  change.
  - If the maximum becomes 0 (no row exceeds pane width), hide the
    horizontal scrollbar AND reset `h_offset` to 0 (there is no
    valid offset to preserve when nothing exceeds the viewport).
  - If line-wrap is toggled ON, **preserve `h_offset` internally**
    without applying it to rendering (the offset is internal state
    while wrap is on). The horizontal scrollbar is hidden. See
    "Line-wrap toggle: preserve-and-restore" below.
  - If the maximum shrinks but remains positive, clamp `h_offset`
    down to the new maximum so blank shifted columns are never
    rendered.
- B2. Horizontal scroll inputs:
  - Trackpad two-finger horizontal swipe (native macOS, Windows,
    and Linux). On Linux, two-finger horizontal swipe is sourced
    from libinput's horizontal scroll axis, matching the existing
    Warp Linux trackpad gesture support. If a Linux platform/session
    does not deliver horizontal-axis events from the windowing system
    (e.g. constrained Wayland compositors, some X11 driver setups),
    Shift + scroll wheel and the horizontal scrollbar drag remain the
    documented fallback inputs and the behavior contract is otherwise
    unchanged.
  - Shift + scroll wheel.
  - A new horizontal scrollbar at the bottom of the pane,
    visible only when content exceeds pane width.
- B3. Scroll position is per-pane and persists across the pane's
  lifetime (resets on pane close).
- B4. Vertical scroll behavior is unchanged. The two axes are
  independent.
- B5. Selection and click-to-position respect the horizontal
  offset — the user clicks where they see, not where the
  underlying buffer column is. Hit-testing uses the same terminal
  grid column model as rendering (see B8).
- B6. Auto-scroll on new output: the pane snaps back
  to horizontal offset 0 only when all of these are true:
  - `terminal.horizontal_scroll.snap_on_new_output = true`
    (per-pane override `pane.horizontal_snap_on_new_output` takes
    precedence when set).
  - The terminal is pinned to the live bottom when the output event
    is applied, not viewing scrollback.
  - The pane is on the primary screen, not the alternate screen.
    Alternate-screen / TUI applications (vim, htop, less, etc.) own
    their column model and never trigger snap.
  - The output event commits a completed rendered row strictly
    below the current bottom-of-buffer position — i.e. a true
    line-append. Tailing logs that emit `\n`-terminated full lines
    DO trigger snap. Excluded: partial writes to the current row
    (no `\n` yet), CR-then-rewrite prompt redraws (cursor returns
    to column 0 of the same row), in-place line edits via `\x1b[K`
    or other erase sequences, cursor-only updates, and any output
    that arrives while the user is viewing scrollback.
  Implementation hint: distinguish "new line committed below
  viewport" from "in-place line update" by tracking the cursor row's
  relationship to bottom-of-buffer at the moment the output event is
  applied — only transitions that move the committed bottom row
  index forward count as line-append.
- B7. Persist `terminal.horizontal_scroll.snap_on_new_output` in the
  existing `TerminalSettings` settings group
  (`app/src/terminal/settings.rs`) with TOML path
  `terminal.horizontal_scroll.snap_on_new_output`, boolean,
  default `true`, with the same global sync behavior as other
  non-private terminal settings. V1 exposes it through the
  settings file/schema and surfaces a first-class toggle under
  Settings → Terminal → Scrolling labelled "Snap horizontal scroll
  on new output". A per-pane override is available via tab/pane
  config `pane.horizontal_snap_on_new_output` (boolean, optional;
  when present, takes precedence over the global setting for that
  pane).
- B8. Offset and column model. `h_offset` is measured in TERMINAL
  GRID COLUMNS, not characters or bytes. The column model matches
  Warp's existing terminal grid:
  - Wide glyphs (CJK, emoji, double-width box drawing) occupy 2
    grid columns.
  - Combining marks and zero-width joiners occupy 0 grid columns
    (they attach to the preceding cell).
  - Tabs expand to the next tab stop based on the terminal's
    existing tab-width configuration.
  An `h_offset` of N columns scrolls each rendered row so that grid
  column N is the leftmost visible column. If column N falls inside
  a wide glyph (i.e. the glyph started at column N-1 and spans
  columns N-1 and N), the partially-clipped left edge renders per
  the existing terminal grid render rules — typically a half-block
  or whitespace placeholder cell — not an attempt to redraw the
  truncated half of the glyph. Selection, hit-testing, and the
  scrollbar thumb all use this same column model so the offset →
  buffer-column mapping is bijective for whole-cell positions and
  consistently rounded for fractional positions.

### Offset clamping & reset rules

Per-pane horizontal scroll state is `(h_offset, max_observed_columns)`
where `max_observed_columns` is the longest visible row's grid-column
length within the current visible buffer range. Clamping rules:

- On pane resize: re-clamp to
  `h_offset = clamp(h_offset, 0, max_observed_columns - viewport_columns)`.
  If `max_observed_columns - viewport_columns` is negative
  (longest line now fits), clamp to 0 and hide the scrollbar.
- On vertical scroll: do NOT auto-reset `h_offset` to 0. Recompute
  `max_observed_columns` against the new visible window's longest
  line; if shorter than `h_offset + viewport_columns`, clamp
  `h_offset` down to `max(max_observed_columns - viewport_columns, 0)`.
  Auto-reset to 0 happens only when explicitly required by the snap
  setting (B6) — never silently on vertical scroll.
### Line-wrap toggle: preserve-and-restore (uniform invariant)

- **Wrap ON.** When line-wrap is toggled ON, the horizontal
  scrollbar is hidden and `h_offset` is NOT applied to rendering
  (every row rewraps inside the viewport). The previous
  `h_offset` value is **preserved internally** as part of the
  pane's per-pane horizontal scroll state.
- **Wrap OFF.** When line-wrap is toggled OFF again, the
  preserved `h_offset` is **restored** and re-clamped against
  the current `max_observed_columns`:
  ```
  h_offset := clamp(preserved_h_offset, 0,
                    max(max_observed_columns - viewport_columns, 0))
  ```
  If the current `max_observed_columns` is smaller than the
  preserved offset's required range, the offset clamps down (it
  is never silently snapped to 0 unless the clamp resolves to
  0 because nothing exceeds the viewport).
- **Reset to 0 ONLY on explicit triggers.** `h_offset` is reset
  to 0 only when one of the following fires:
  1. The user explicitly scrolls to the leftmost column (manual
     reset via input).
  2. Snap-on-new-output triggers per B6 (line-append while
     pinned to live bottom, primary screen, snap setting on).
  3. The pane is closed (state discarded with the pane).
  Toggling line-wrap is NOT one of these triggers — wrap-toggle
  uses preserve-and-restore semantics, never a reset.
- **Rationale.** A user who scrolled horizontally to inspect
  long output, then toggled wrap on for a moment to re-read
  with wrapping, expects to land back where they were when
  wrap is toggled off again. Resetting to 0 on wrap toggle
  loses that position and is a regression of intent.
- On long-line set changes (rows appended, edited in place, or
  evicted from scrollback): recompute `max_observed_columns` against
  the current visible buffer range and clamp `h_offset` per the rules
  above. Never render blank shifted columns: if clamping cannot keep
  `h_offset` valid against any non-empty viewport, snap to 0.

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
  `terminal.horizontal_scroll.snap_on_new_output = true`.
- A5. Horizontal scrollbar appears only when content exceeds pane
  width.
- A6. With `terminal.horizontal_scroll.snap_on_new_output = false`,
  completed new shell output preserves the current horizontal offset.
- A7. Resizing the pane or vertically scrolling to shorter
  visible content **clamps** `h_offset` according to B1a and the
  Offset clamping & reset rules section, with no blank shifted
  columns. Toggling line-wrap follows the preserve-and-restore
  invariant (A_WRAP_TOGGLE), not a reset.
- A_WRAP_TOGGLE_PRESERVE. Turning line-wrap ON while
  `h_offset > 0` preserves the offset internally: the
  scrollbar disappears, content rewraps inside the viewport,
  and the preserved offset is held in pane state.
- A_WRAP_TOGGLE_RESTORE. Turning line-wrap OFF restores the
  preserved `h_offset`, clamped to the current
  `max_observed_columns`. If `max_observed_columns` has not
  shrunk below the preserved value's required range, the
  offset is restored exactly.
- A_WRAP_TOGGLE_NO_RESET. Toggling line-wrap is NEVER a reset
  trigger. `h_offset` is reset to 0 only when (1) the user
  explicitly scrolls to the leftmost column, (2) snap-on-new-
  output fires per B6, or (3) the pane closes.
- A8. In alternate-screen / TUI mode (vim, htop, less), no snap
  occurs regardless of the setting; the alternate screen owns its
  own column layout and h_offset is preserved as-is for the
  duration of the alt-screen session.
- A9. Wide glyphs (CJK, emoji), combining marks, and tabs scroll
  consistently — selection at the visible-leftmost cell of an
  h-scrolled row resolves to the same logical column as the
  rendered cell, including correct handling of half-clipped wide
  glyphs at the left edge.
- A10. On Linux with libinput two-finger horizontal swipe events
  available, the gesture scrolls horizontally; on Linux platforms
  without horizontal-axis events, Shift+wheel and scrollbar drag
  are the documented inputs and tests cover those paths.

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
- T7. Resize and vertical-scroll to shorter content clamp
  `h_offset` to the new max and hide the scrollbar when max
  offset is 0. Vertical scroll to shorter content NEVER resets
  to 0 silently (per the offset-clamping rules).
- T_line_wrap_toggle_round_trip. **Wrap-toggle preserve-and-
  restore.** Set `h_offset = 120` with line-wrap OFF. Toggle
  line-wrap ON: scrollbar hides; rendering rewraps inside
  viewport; pane state still records the preserved offset.
  Toggle line-wrap OFF again: `h_offset` is restored to 120
  (or clamped to `max_observed_columns - viewport_columns` if
  `max_observed_columns` has dropped below 120 + viewport).
  Assertion: post-toggle `h_offset >= min(120, max_valid)` and
  is NEVER 0 unless `max_valid == 0`.
- T_line_wrap_toggle_clamps_on_shrink. Set `h_offset = 200`
  with `max_observed_columns - viewport_columns = 220`. While
  wrap is ON, content evicts such that
  `max_observed_columns - viewport_columns = 80`. Toggle wrap
  OFF: `h_offset` is restored and clamped to 80, not 200.
- T_wrap_toggle_no_reset. Set `h_offset = 50`. Toggle wrap on
  then off five times. After every cycle, `h_offset` is back
  at 50 (clamped to current max). The toggle is never a reset.
- T8. Partial-row writes, prompt redraws (CR-then-rewrite), in-place
  line edits via `\x1b[K`, alternate-screen updates, and output
  received while viewing scrollback all preserve h_offset.
- T9. Wide glyph (CJK / emoji), combining mark, and tab column
  accounting: with h_offset landing inside a wide glyph, the
  partially-clipped left edge renders per existing grid rules and
  selection at that column resolves to the correct logical buffer
  column.
- T10. Per-pane override: when `pane.horizontal_snap_on_new_output`
  is set, it overrides the global
  `terminal.horizontal_scroll.snap_on_new_output` for that pane
  only; other panes follow the global setting.
- T11. Linux trackpad: with libinput horizontal-axis events
  available, two-finger swipe scrolls; with horizontal events
  absent, Shift+wheel and scrollbar drag scroll and the gesture
  is silently ignored (no errors).

## Out of scope

- Horizontal scroll inside notebook cells (different code path,
  follow-up).
- Auto-detect long lines and offer a toast suggesting line-wrap
  toggle.
- Horizontal scrollbar styling customization.
