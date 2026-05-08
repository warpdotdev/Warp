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

## Default font stacks (per platform, no Inter)

The platform Default stack is what Warp uses when
`proportional_font_family` is `None`. Warp does **not** bundle the
Inter typeface, so Inter is **not** in any platform Default stack.
Users who have Inter installed locally and want Inter as their
primary proportional font MUST opt in by setting
`proportional_font_family` explicitly (e.g.,
`"Inter, system-ui, sans-serif"`). The Default behavior is
deterministic per-platform:

| Platform   | Default stack                                                                |
| ---------- | ---------------------------------------------------------------------------- |
| macOS      | `system-ui, -apple-system, "SF Pro Text", "Helvetica Neue", sans-serif`      |
| Windows    | `system-ui, "Segoe UI Variable", "Segoe UI", sans-serif`                     |
| Linux      | `system-ui, "Cantarell", "Ubuntu", "Liberation Sans", sans-serif`            |

Note: Inter is NOT in the default fallback. Users who have Inter
installed and want it as primary must explicitly configure
`proportional_font_family` to include it.

## Behavior contract

- B1. New setting `appearance.proportional_font_family:
  Option<String>`. `None` / omitted means "Default" and resolves to
  the per-platform Default stack listed above. The stored value is
  the **canonical serialization** (B1a) of the user-supplied
  family list.

  All three settings (family, size, line-height) use
  `SyncToCloud::Globally(RespectUserSyncSetting::Yes)` like other
  appearance settings.

- B1a. **Validation, parsing, and canonical serialization.** A
  non-empty user-supplied value is parsed as a CSS font-family
  list per CSS Fonts Module Level 4 grammar:

  - Families are separated by commas.
  - Each family is either an unquoted identifier sequence (one or
    more `<custom-ident>` tokens joined by ASCII space), or a
    double-quoted `<string>`.
  - Inside a quoted string, `"` and `\` MUST be backslash-escaped
    (`\"`, `\\`).

  After successful parse, Warp stores a **canonical serialization**
  of the family list (used for storage, sync, and Settings UI
  display):

  1. Each parsed family token is normalized:
     - If the family name matches `<custom-ident>` (alphanumerics,
       underscore, hyphen, no leading digit, no spaces, no special
       chars): emit unquoted (e.g., `Arial`, `Open-Sans`).
     - Otherwise (contains spaces, commas, quotes, or any character
       outside `<custom-ident>`): emit double-quoted with
       backslash-escaping for `"` and `\` (e.g.,
       `"Helvetica Neue"`, `"My \"weird\" Font"`).
  2. Families are joined with `, ` (comma + ASCII space).
  3. **Round-trip identity:** parsing the canonical serialization
     and re-canonicalizing MUST yield the same string. Tested by
     T_canonical_serialization.

  Values that fail to parse are rejected on save with an inline
  error in Settings; the previously-valid (already-canonical)
  value is retained. Programmatic writes (e.g., from sync) that
  fail to parse are dropped, the family setting is cleared to
  `None` for this device, and a telemetry warning fires with the
  rule id (never the raw value).

- B1b. **Missing-font fallback.** Resolution proceeds left-to-right
  through the parsed family list. If no listed family matches an
  installed font on the current OS, the platform Default stack
  is appended as a final fallback. If the parsed family list is
  empty after parsing, Default is used directly. The stored value
  is never modified by fallback — it remains the user's preference
  in canonical form.

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
  - Family values written across platforms are always canonical
    form per B1a. A peer device receiving a value re-parses it on
    read; if parse fails, B1a programmatic-write rules apply
    (drop value, fire telemetry).

- B1d. **Synced unsupported family.** A synced non-empty family
  that is unavailable on the current OS renders with the platform
  Default stack via B1b cascade. Settings shows the family as
  unavailable with a "Reset to Default" action. Reset clears the
  stored family so the setting returns to `None` for the active
  platform.

- B1e. **Render-time CSS construction (no string interpolation).**
  The font family value MUST NEVER be concatenated into a CSS
  string as raw user text. The renderer takes the **parsed token
  list** (the structured representation produced in B1a, not the
  stored canonical string) and constructs the CSS `font-family`
  declaration by feeding each token through the same canonical
  serializer. Equivalently: the path from user input to CSS is
  always `String -> ParsedFamilyList -> CssString`, never
  `String -> CssString`. This eliminates CSS injection vectors
  via crafted family names (e.g., a family name containing
  `</style><script>` cannot break out of the CSS context because
  the serializer treats it as a quoted-string token and escapes
  it).

  Implementation MUST use a token-list → CSS-string serialization
  pass; the renderer MUST NOT pass user input through string
  interpolation.

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
  with these three controls and a live preview block. Family input
  shows canonical serialization; the inline error on parse failure
  references the offending token.

- B6. Existing fixed-pixel layouts that depend on the current
  cramped baseline are audited and migrated to em-relative or
  line-height-relative spacing where they would otherwise break
  on larger fonts.

## Acceptance criteria

- A1. Default install on macOS, Windows, and Linux uses the
  platform Default stack listed in "Default font stacks" (NO Inter
  in the stack on any platform) and renders Markdown side-by-side
  with GitHub visually comparable (same paragraph fits in same
  vertical space ±10%).
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
- A6. Invalid family input (e.g., unbalanced quotes, illegal
  characters in an unquoted identifier) is rejected in Settings
  with an inline error; the previous canonical value is retained.
- A7. Canonical-form round-trip: writing
  `inter , "Open Sans" ,Arial` (with stray spaces) is stored as
  `Inter, "Open Sans", Arial` (or the user's preferred case).
  Reading back yields the same canonical string.
- A8. CSS-injection guard: a family value containing
  `</style><script>alert(1)</script>` is parsed as a single
  quoted-string token, stored canonically with proper escaping,
  and serialized into the CSS `font-family` declaration as a
  quoted CSS string. The injected token cannot break out of the
  CSS context.

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
- Family-list parser/serializer is a single module exposing
  `parse_family_list(s: &str) -> Result<FamilyList, ParseErr>` and
  `serialize_family_list(list: &FamilyList) -> String`. The
  renderer's CSS builder takes `&FamilyList`, not `&str`, to
  enforce B1e at the type level.

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
- T_canonical_serialization. Round-trip: a parsed family list,
  re-serialized through the canonical serializer, parses again to
  the same family list. Run on a fixture set including stray
  spaces, missing/extra commas-around-spaces, mixed quoting.
- T_canonical_quoting. Family names containing spaces are
  emitted double-quoted; names containing `"` and `\` are emitted
  with backslash-escaped quoted form; simple identifiers stay
  unquoted.
- T_inter_not_in_default. The platform Default stack on each of
  macOS, Windows, and Linux does NOT contain `Inter`. Tested by
  asserting the literal Default stack strings.
- T_render_no_injection. Setting
  `proportional_font_family` to a value containing
  `</style><script>alert(1)</script>` results in a CSS
  `font-family` declaration in which the malicious token is
  emitted as a quoted CSS string with proper escaping and does
  not break out of the CSS rule. Confirmed by parsing the
  rendered CSS back and asserting one quoted-string token plus
  a closing semicolon.

## Out of scope

- Bundling Inter or any custom font with the binary (use system
  defaults; user can install Inter manually if desired).
- Separate fonts for Settings vs Markdown — they share one knob
  in V1.
- Extending the new typography to the out-of-scope surfaces
  enumerated in Surface scope (block headers, command bar,
  tooltips, etc.) — targeted for a follow-up.
