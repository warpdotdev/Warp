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

## Surface scope

V1 changes apply to a defined set of surfaces. The new tokens
read from the same Appearance state that the rest of the app uses,
but only the listed surfaces consume the new tokens; all other UI
surfaces continue using existing typography unchanged in V1.

V1 surfaces:

- **Settings panes** — every page under Settings reads the new
  proportional font tokens.
- **Rendered Markdown blocks** — Markdown rendered in agent
  output and other in-product Markdown surfaces (changelog,
  product help text rendered as Markdown).

Out-of-scope surfaces for V1 (continue using existing typography):

- Block headers, command bar, prompt, and other shell chrome.
- Tooltips, toast notifications, popover chrome.
- Login / onboarding surfaces.

These surfaces may opt in in a follow-up; V1 keeps them on
existing typography to minimize visual churn.

## Behavior contract

- B1. New setting `appearance.proportional_font_family:
  Option<String>`. `None` / omitted means "Default" and resolves to
  a bundled-independent platform stack:
  - macOS: system proportional UI font (`system-ui` / San
    Francisco), then `-apple-system`, `BlinkMacSystemFont`,
    then the system sans-serif fallback.
  - Windows: `Segoe UI Variable`, then `Segoe UI`, then the system
    sans-serif fallback.
  - Linux: a system-available stack —
    `system-ui, "Cantarell", "Ubuntu", "Liberation Sans",
    "DejaVu Sans", "Noto Sans", sans-serif`.
    Inter is preferred where the user has installed it, but is
    **not bundled** with the app; absent Inter, the OS-provided
    UI font is used.

  All three settings (family, size, line-height) use
  `SyncToCloud::Globally(RespectUserSyncSetting::Yes)` like other
  appearance settings.

- B1a. **Validation.** A non-empty user-supplied value is parsed
  as a CSS family-list string. Allowed characters per family:
  alphanumerics, ASCII spaces, hyphens, periods, and underscores.
  Family names containing other characters MUST be quoted with
  ASCII double-quotes; the only structural separator between
  families is a comma. Values that fail to parse are rejected on
  save with an inline error in Settings; the previously-valid
  value is retained. Programmatic writes (e.g., from sync) that
  fail to parse are dropped, the family setting is cleared to
  `None` for this device, and a telemetry warning fires with the
  rule id (never the raw value).

- B1b. **Missing-font fallback.** Resolution proceeds left-to-right
  through the parsed family list. If no listed family matches an
  installed font on the current OS, the platform Default stack
  (B1) is appended as a final fallback. If the parsed family list
  is empty after parsing, Default is used directly. The stored
  value is never modified by fallback — it remains the user's
  preference.

- B1c. **Per-platform sync schema.** All three settings sync as a
  per-platform map with a shared default key:

  ```json
  {
    "default": "<value or null>",
    "darwin":  "<value or null>",
    "win":     "<value or null>",
    "linux":   "<value or null>"
  }
  ```

  - On read, each platform consumes its own key. If that key is
    absent, it falls back to `default`. If `default` is also absent,
    the platform Default stack / clamp default applies.
  - On write, Warp writes the active platform's key. The `default`
    key is written by Settings only when the user explicitly
    chooses "Apply to all platforms"; otherwise it is left
    untouched.
  - Reset-to-Default clears the active platform's key only,
    reverting that platform to either `default` or the platform
    Default stack / clamp default.

- B1d. **Synced unsupported family.** A synced non-empty family
  that is unavailable on the current OS renders with the platform
  Default stack via B1b cascade. Settings shows the family as
  unavailable with a "Reset to Default" action. Reset clears the
  stored family so the setting returns to `None` for the active
  platform.

- B2. New setting
  `appearance.proportional_font_size: f32`
  (default `14.0`, clamped to `[14.0, 24.0]`). Persistence and
  sync follow B1c (per-platform map with shared default).
  Manually edited or synced values outside the range are clamped
  on read to the nearest bound; the stored value is left unchanged
  to preserve user intent if the bounds expand later.

- B3. New setting
  `appearance.proportional_font_line_height: f32`
  (default `1.55`, clamped to `[1.0, 2.5]`). Persistence and sync
  follow B1c (per-platform map with shared default). Same clamp
  behavior as B2.

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
  change applies to V1 surfaces (Settings panes and rendered
  Markdown) without restart. Out-of-scope surfaces (Surface
  scope) are unchanged.
- A3. Setting size to the configured maximum of `24` does not
  break Settings layout (tabs don't overflow, buttons don't clip)
  and is reflected in the live preview and rendered Markdown.
- A3a. Setting size to the configured minimum of `14` renders
  cleanly with no clipping at the smallest supported breakpoint.
- A3b. Programmatic / synced size of `13` clamps to `14` on read;
  size of `25` clamps to `24` on read. Stored value is left
  unchanged.
- A3c. Programmatic / synced line-height of `0.9` clamps to `1.0`
  on read; `2.7` clamps to `2.5` on read. Stored value is left
  unchanged.
- A4. An unavailable synced font family (e.g., a peer device wrote
  `"Foo Bar Custom"` that isn't installed on the local OS) falls
  back to the platform Default stack via B1b cascade and exposes
  Reset to Default in Settings.
- A5. Per-platform sync round-trip: a value written on macOS is
  read back on macOS only; Linux on the same account either reads
  its own platform key or falls back to `default` / platform
  Default stack per B1c.
- A6. Invalid family input (e.g., a string with disallowed
  punctuation) is rejected in Settings with an inline error; the
  previous value is retained.

## Implementation pointers

- Existing proportional font is set in
  `app/src/appearance.rs` (search for "proportional" or
  "ui_font"). Replace the hard-coded value with a setting read.
- Markdown renderer reads the proportional font from the same
  source — verify the helper used by the editor's
  `BufferBlockStyle` paths.
- Settings sync layer: extend the existing per-setting sync codec
  to encode/decode the per-platform map in B1c. Existing scalar
  values from older clients deserialize into the `default` key.

## Test plan

- T1–T3. Setting round-trips for family / size / line-height,
  including the per-platform sync schema (B1c) — write on darwin,
  read on darwin, read on linux without darwin key (should hit
  `default`), read on linux without any key (should hit Default
  stack / clamp default).
- T4. Snapshot test: rendered README.md fixture matches a stored
  reference within tolerance.
- T5. Settings page renders without overflow at default and at
  max font-size (24), and with no clipping at min (14).
- T6. Empty, whitespace-only, and unavailable synced font-family
  values render with the platform Default stack and expose Reset
  to Default.
- T7. Clamp boundary tests: size `13`→`14`, size `25`→`24`,
  line-height `0.9`→`1.0`, line-height `2.7`→`2.5`. Stored value
  unchanged after read.
- T8. Validation: invalid family input rejected with inline error;
  programmatic write of invalid family drops the value and emits
  a telemetry warning with rule id only.
- T9. Visual comparison harness: side-by-side rendering of a
  fixture Markdown document and a fixture Settings pane against a
  stored reference at sizes `14`, `18`, and `24`. Each comparison
  asserts pixel-difference within tolerance and asserts paragraph
  vertical-space parity with the GitHub baseline (±10%).
- T10. Synced unsupported family: peer-device sync sets
  `proportional_font_family` to a family not installed on the
  current OS — local device renders with platform Default stack
  and Settings exposes Reset to Default.

## Out of scope

- Bundling Inter or any custom font with the binary (use system
  defaults; user can install Inter manually if desired).
- Separate fonts for Settings vs Markdown — they share one knob
  in V1.
- Extending the new typography to the out-of-scope surfaces
  enumerated in Surface scope (block headers, command bar,
  tooltips, etc.) — targeted for a follow-up.
