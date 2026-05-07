# Spec: Improve default proportional font readability (GH-9953)

## Problem

The default proportional font used in Settings panes and rendered
Markdown looks cramped and is hard to read compared to GitHub's
rendering of the same content. The issue includes a side-by-side
screenshot.

## Goal

Two-part fix: (1) ship a more readable default proportional font
choice, (2) expose the proportional font as a user-configurable
setting (it currently isn't).

## Behavior contract

- B1. New setting `appearance.proportional_font_family:
  Option<String>`. `None` / omitted means "Default" and resolves to
  a bundled-independent platform stack:
  - macOS: system proportional UI font (`system-ui` / San
    Francisco).
  - Windows: `Segoe UI Variable`, then `Segoe UI`, then the system
    sans-serif fallback.
  - Linux: `Noto Sans`, then `DejaVu Sans`, then `Liberation Sans`,
    then the fontconfig `sans-serif` fallback.
  Same `SyncToCloud::Globally(RespectUserSyncSetting::Yes)` as
  other appearance settings.
- B1a. A synced non-empty font family is a user preference, not a
  guaranteed installed font. If the synced value is empty,
  whitespace-only, or unavailable on the current OS, Warp renders
  with the current platform's Default stack, keeps the stored value
  unchanged, and shows the family as unavailable in Settings with a
  "Reset to Default" action. Reset clears the stored family so the
  setting returns to `None` / omitted.
- B2. New setting
  `appearance.proportional_font_size: f32`
  (default `14.0`, clamped to `[10.0, 24.0]`).
- B3. New setting
  `appearance.proportional_font_line_height: f32`
  (default `1.55`, clamped to `[1.0, 2.5]`).
- B4. The default values are chosen to match GitHub's typography
  baseline (1.5–1.6 line-height, 14–16px). Verified by side-by-side
  screenshot in the test plan.
- B5. Settings → Appearance gets a "Proportional text" subsection
  with these three controls and a live preview block.
- B6. Existing fixed-pixel layouts that depend on the current
  cramped baseline are audited and migrated to em-relative or
  line-height-relative spacing where they would otherwise break
  on larger fonts.

## Acceptance criteria

- A1. Default install on macOS, Windows, and Linux uses the
  platform Default stack and renders Markdown side-by-side with
  GitHub visually comparable (same paragraph fits in same vertical
  space ±10%).
- A2. User can change the family in Settings → Appearance and the
  change applies to Settings panes and rendered Markdown without
  restart.
- A3. Increasing the size to the configured maximum of 24 doesn't
  break Settings layout (tabs don't overflow, buttons don't clip).
- A4. An unavailable synced font family falls back to Default on
  the current platform and can be reset to Default from Settings.

## Implementation pointers

- Existing proportional font is set in
  `app/src/appearance.rs` (search for "proportional" or
  "ui_font"). Replace the hard-coded value with a setting read.
- Markdown renderer reads the proportional font from the same
  source — verify the helper used by the editor's
  `BufferBlockStyle` paths.

## Test plan

- T1–T3. Setting round-trips for family / size /
  proportional-font line-height.
- T4. Snapshot test: rendered README.md fixture matches a stored
  reference within tolerance.
- T5. Settings page renders without overflow at default and at
  max font-size (24).
- T6. Empty, whitespace-only, and unavailable synced font-family
  values render with the platform Default stack and expose Reset to
  Default.

## Out of scope

- Bundling Inter or any custom font with the binary (use system
  defaults; user can install Inter manually if desired).
- Separate fonts for Settings vs Markdown — they share one knob
  in V1.
