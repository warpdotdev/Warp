# Option to clamp/disable truecolor BCE bg painting (GH-10278)

## Summary

Add a setting that controls how Warp renders truecolor (24-bit) ANSI background
codes (`\x1b[48;2;R;G;Bm`). Today, Warp paints the truecolor background
edge-to-edge across the block width on every emitting line. Tools that emit
contiguous bg-colored lines (Claude Code's diff renderer, `delta`, `git diff`
with custom config) cause the viewport to fill with a wall of color and read
as if the entire screen is tinted.

The new setting offers three modes: keep current behavior (`flood`), paint
only behind printed glyphs (`clamp_to_text`), or skip truecolor bg painting
entirely (`disabled`).

## Problem

- `\x1b[48;2;R;G;Bm` is widely emitted by modern diff/highlight tools.
- Warp applies the active bg color to the rest of the line/block per BCE
  semantics, producing large flat color rectangles.
- For multi-line diffs, the cumulative effect is perceived as a full-screen
  tint that overwhelms the rendered content.
- Users want a way to dial this back without disabling truecolor or losing fg
  highlighting.

## Goals

- A first-class setting controlling truecolor bg painting with three modes:
  `flood`, `clamp_to_text`, `disabled`.
- Surface the setting in Settings UI under Terminal → Appearance.
- Allow a per-pane override via tab config.
- Apply changes to new output without restart.

## Non-Goals

- Not changing 8-bit / 16-color BCE behavior.
- Not changing 256-color (`\x1b[48;5;Nm`) handling.
- Not changing how palette colors are configured.
- Not changing inline background semantics inside a printed character cell.
- Not retroactively repainting existing scrollback when the setting changes.
- Not changing Warp's own block-level diff highlight (non-ANSI driven).

## Behavior Contract

### B1. Setting enum

`terminal.truecolor_background_mode` enum: `"flood"` (default), `"clamp_to_text"`,
`"disabled"`.

### B2. Mode `flood` (default)

Unchanged behavior — `\x1b[48;2;R;G;Bm` paints from cursor to end-of-line /
end-of-block per current rules. This preserves backwards compatibility for
users who rely on the existing visual.

### B3. Mode `clamp_to_text`

When a line ends with the truecolor bg still active, the background paints
only the cells that contain printed glyphs (or whitespace explicitly written).
Cells past the last written column render with the terminal default
background. Implicit BCE fill from cursor-to-EOL is suppressed for truecolor.

### B4. Mode `disabled`

Truecolor bg ANSI codes are PARSED but NOT rendered as background fill. The
cell foreground continues to use any active fg color. State is treated as if
the user had emitted `\x1b[49m` (default bg) for the affected cells.

### B5. Per-pane override

Setting applies to all terminal panes by default. Per-pane override via tab
config `pane.truecolor_background_mode` is supported for power users running
mixed workloads.

### B6. Live application

The setting applies to NEW output from the moment of change. Already-rendered
scrollback is NOT retroactively repainted; the toggle effect is "from now on".

### B7. Other color depths unchanged

256-color (`\x1b[48;5;Nm`) and 16-color BCE behavior is UNCHANGED in all
modes — only truecolor (24-bit) bg is gated by this setting.

### B8. Block-level highlight unaffected

Warp's own per-line block colors that are not driven by ANSI (e.g. command
output highlight, error block tint) are unaffected by this setting.

## Settings / API surface

- `terminal.truecolor_background_mode`: enum, default `"flood"`. Stored in
  user terminal settings.
- `pane.truecolor_background_mode`: optional per-pane override in tab config.
- Settings UI: Settings → Terminal → Appearance → "Truecolor background
  painting" radio group:
  - `Flood (default)` — paints the full line.
  - `Clamp to text` — paints only behind printed text.
  - `Disabled` — does not paint truecolor backgrounds.

## Acceptance Criteria

- A1: Default mode is `flood` and matches current rendering byte-for-byte for
  a recorded `vim_24bitcolors_bce` reference test.
- A2: `clamp_to_text` does not paint cells past the last printed glyph on
  multi-line `git diff` colored output.
- A3: `disabled` renders truecolor bg cells with the terminal default
  background; foreground colors remain intact.
- A4: 256-color bg output is unchanged in `disabled` mode.
- A5: Setting changes take effect for new output without restart.
- A6: Already-rendered scrollback is not repainted on toggle.
- A7: Per-pane tab-config override correctly overrides the global setting.

## Implementation Pointers

Verified paths (via `git ls-files`):

- ANSI parser / handler: `app/src/terminal/model/ansi/handler.rs`,
  `app/src/terminal/model/ansi/mod.rs`,
  `app/src/terminal/model/grid/ansi_handler.rs`.
- Crate-level ANSI: `crates/warp_terminal/src/model/ansi/mod.rs`,
  `crates/warp_terminal/src/model/ansi/control_sequence_parameters.rs`.
- Existing truecolor BCE reference test fixtures:
  `app/src/terminal/ref_tests/data/vim_24bitcolors_bce/` (use as the
  flood-mode regression baseline).
- Settings module: `app/src/settings/` — add a new file
  `app/src/settings/truecolor_background_mode.rs` (new module) wired into
  the existing terminal settings struct.

Likely change shape:

1. Add the enum + serde + default in the new settings module.
2. Thread the mode into the renderer where truecolor bg cells are committed
   to the grid, gating the BCE fill path on mode.
3. Wire the Settings UI control under Terminal → Appearance.

## Tests

- T1: Flood mode matches `vim_24bitcolors_bce` recorded grid (regression).
- T2: `clamp_to_text` against a synthetic multi-line diff recording — assert
  cells beyond last glyph use default bg.
- T3: `disabled` mode — truecolor bg cells render as default bg, fg color
  preserved.
- T4: 256-color bg output unchanged across all three modes.
- T5: Foreground color preserved when truecolor bg is disabled.
- T6: Per-pane override changes a single pane without affecting siblings.
- T7: Setting change applies to subsequent output only.
- T8: Toggling the setting does not repaint scrollback.

## Open Questions

- In `clamp_to_text`, should regular spaces (`\x20`) emitted between glyphs
  be considered "written" cells (and therefore bg-painted)? Recommendation:
  yes — explicit writes (including spaces) are bg-painted; clamp means "no
  implicit fill past last write". This preserves intent for runs like
  `\x1b[48;2;...m  text  \x1b[0m` while still suppressing the EOL flood.

## Telemetry

No new telemetry events. If the setting is changed, the existing settings
update event already records the key change without payload extensions.
