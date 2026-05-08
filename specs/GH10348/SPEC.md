# Configurable Word Delimiters for Word-Deletion and Word-Navigation Shortcuts (GH-10348)

## Summary

Make the set of characters that act as word boundaries configurable for word-deletion (Delete Word Left, Delete Word Right) and word-navigation (Move Cursor Word Left, Move Cursor Word Right) shortcuts. Today these shortcuts treat path-like strings such as `/var/www/example.com/logs` as a single word, deleting backward all the way to whitespace. With a configurable delimiter set including `/`, `.`, `-`, `_`, `:`, and `=`, Delete Word Left stops at each path segment, matching expectations from VS Code, JetBrains IDEs, and most modern editors. Settings are global with per-input-context overrides (terminal, agent, editor).

## Problem

The four word-cursor shortcuts use a hard-coded boundary definition that effectively only treats whitespace as a delimiter. Real-world inputs are usually paths, URLs, identifiers, or kebab/snake-case strings — none of which are split by whitespace alone:

- `/var/www/example.com/logs` is one "word", so Delete Word Left wipes the entire path.
- `feature-branch-name` is one "word", so Move Cursor Word Right jumps the whole identifier.
- `key=value` is one "word", so users can't quickly correct just the value.

Every modern editor exposes a configurable word-separator set for exactly this reason. Warp does not.

## Goals

- One configurable delimiter set used by **all four** word-cursor shortcuts: Delete Word Left, Delete Word Right, Move Cursor Word Left, Move Cursor Word Right.
- A sensible default set that matches typical editor behavior (path/punct chars plus whitespace).
- Per-input-context overrides so terminal, agent prompt, and editor surfaces can diverge when needed.
- Live-effective changes (no app restart).

## Non-Goals

- Not redefining single-character delete (Backspace, Delete) — only the word-granular shortcuts.
- Not changing line-deletion shortcuts (Delete Line Left/Right, Delete Line).
- Not adding regex- or predicate-based boundary definitions in V1. (Tracked as a V1.5 follow-up — see Open Questions.)
- Not supporting Unicode category-based boundaries in V1 (e.g., "any punctuation"). The setting is a literal character list.

## Behavior Contract

### B1. Global setting and default

A new setting `editor.word_delimiters` (string of characters) defines the global word-boundary set. Default value:

```
"/.-_:= \t\n\r\x0b\x0c"
```

The default includes path/punct characters (`/`, `.`, `-`, `_`, `:`, `=`) plus all standard whitespace (space, tab, newline, carriage return, vertical tab, form feed). Whitespace is included for back-compat with current behavior.

### B2. Canonical word-deletion algorithm

Let `D` be the resolved **delimiter character set** for the current input context (per B4). Define:

- A **non-delimiter run** is a maximal contiguous sequence of characters NOT in `D`.
- A **delimiter run** is a maximal contiguous sequence of characters IN `D`.

The buffer is therefore an alternating sequence of non-delimiter runs and delimiter runs.

#### Delete Word Left

From the cursor position, walk LEFT and apply the following rules to a single Delete Word Left invocation:

1. If the cursor is at start-of-line, do nothing.
2. If the character immediately LEFT of the cursor is in `D`: consume the entire delimiter run that ends just left of the cursor, **then** consume the non-delimiter run that ends just left of that (if any). Stop at start-of-line.
3. Otherwise (the character immediately LEFT of the cursor is NOT in `D`): consume only the non-delimiter run from the cursor leftward to its start. Do **not** consume any preceding delimiter run on this invocation.

In all cases, do not cross start-of-line.

#### Delete Word Right

Mirror of Delete Word Left:

1. If the cursor is at end-of-line, do nothing.
2. If the character immediately RIGHT of the cursor is NOT in `D`: consume only the non-delimiter run from the cursor rightward to its end. Do **not** consume any following delimiter run on this invocation.
3. Otherwise (the character immediately RIGHT of the cursor is in `D`): consume the entire delimiter run that begins just right of the cursor, **then** consume the non-delimiter run that begins just right of that (if any). Stop at end-of-line.

In all cases, do not cross end-of-line.

#### Worked example: `/var/www/example.com|` (cursor at end)

Using the default `D` (`/.-_:= \t\n\r\x0b\x0c`):

| Step | Buffer state                        | Action                                                                                       | Result                              |
| ---- | ----------------------------------- | -------------------------------------------------------------------------------------------- | ----------------------------------- |
| 0    | `/var/www/example.com\|`            | start                                                                                        | —                                   |
| 1    | `/var/www/example.com\|`            | char left = `m` (∉ D). Rule 3 → consume `com`.                                              | `/var/www/example.\|`              |
| 2    | `/var/www/example.\|`               | char left = `.` (∈ D). Rule 2 → consume delim run `.`, then non-delim run `example`.        | `/var/www/\|`                       |
| 3    | `/var/www/\|`                       | char left = `/` (∈ D). Rule 2 → consume delim run `/`, then non-delim run `www`.            | `/var/\|`                           |
| 4    | `/var/\|`                           | char left = `/` (∈ D). Rule 2 → consume delim run `/`, then non-delim run `var`.            | `/\|`                               |
| 5    | `/\|`                               | char left = `/` (∈ D). Rule 2 → consume delim run `/`. No preceding non-delim run.          | `\|`                                |
| 6    | `\|`                                | start-of-line. No-op.                                                                        | `\|`                                |

This is the canonical reference. All examples, acceptance criteria, and tests below MUST match this algorithm.

### B3. Word-navigation semantics

Move Cursor Word Left / Right use the **same** delimiter set `D` and the **same** run-walking rules from B2, but they move the cursor instead of removing characters. The cursor lands at the position the equivalent Delete Word Left / Right would have left the cursor (the start of the consumed region for Left; the end for Right).

### B4. Per-context overrides

Three optional overrides exist:

| Setting key                | Applies in            |
| -------------------------- | --------------------- |
| `terminal.word_delimiters` | Terminal input lines  |
| `agent.word_delimiters`    | Agent prompt input    |
| `editor.word_delimiters`   | All editor surfaces (default fallback for the other contexts when their override is unset) |

Resolution order for a given input context:

1. The context-specific override, if present and non-empty.
2. Otherwise, `editor.word_delimiters`.
3. Otherwise, the built-in default from B1.

### B5. Whitespace is always a delimiter

Whitespace characters (space, tab, newline, carriage return, vertical tab, form feed) are **always** treated as delimiters, regardless of the configured set. The `WordBoundaryClassifier` ORs the user's set with the fixed whitespace set, so even if the user provides a value that omits whitespace, whitespace remains in the effective set `D`. This prevents users from accidentally configuring a state where Delete Word Left deletes across line breaks or runs forever.

### B6. Setting-value semantics, validation, and fallback

This subsection is the single source of truth for how the setting value string is interpreted, including escape sequences, validation, and missing/empty/invalid handling. The Settings/API surface and Settings UI sections reference this section.

#### B6.1. Value type and representation

- **Type.** A string of characters. Each codepoint in the string is added to the delimiter set `D`. Order is irrelevant. Duplicates are coalesced (set semantics).
- **Whitespace floor.** Per B5, the effective `D` always also contains the standard whitespace set. The user-provided value adds to that floor; it cannot subtract from it.

#### B6.2. Escape sequences in TOML config

The config file is TOML. The setting key is read as a TOML basic string, which honors the standard escape sequences:

| Escape    | Meaning              |
| --------- | -------------------- |
| `\t`      | Tab (U+0009)         |
| `\n`      | Newline (U+000A)     |
| `\r`      | Carriage return (U+000D) |
| `\\`      | Backslash            |
| `\"`      | Double quote         |
| `\uXXXX`  | Unicode codepoint    |

Whitespace can be entered literally inside the string OR via the corresponding escape. The two are equivalent.

Example (TOML):

```toml
editor.word_delimiters = "/.\\-_:= \t\n"
```

Note `\\-` produces a literal `\-`; in this default the backslash itself is not a delimiter. The actual default uses no backslash — it is shown only to demonstrate escaping mechanics.

#### B6.3. Settings UI (single-line text field)

The Settings text field accepts characters either literally (e.g., typing `/`) or as escape sequences (e.g., typing `\t`). Two display modes:

- **Literal mode (default).** Whitespace characters render as themselves. Tabs and newlines may be hard to see but are still present in the value.
- **"Show escapes" toggle.** When enabled, whitespace and control characters in the rendered field are shown as their escape representations (`\t`, `\n`, `\r`, `\v`, `\f`, space rendered as a visible middle-dot or similar) so the user can see and edit them precisely.

A "Reset to default" button restores the documented default string.

#### B6.4. Validation

- Allowed characters in the resolved set: any printable Unicode codepoint, plus space, tab, newline, carriage return, vertical tab, form feed.
- Disallowed: other ASCII control characters (e.g., NUL, BEL, ESC). If the user enters one, the Settings UI shows an inline error and refuses to save until corrected; the runtime, if it ever encounters such a value, treats the setting as invalid (see B6.5) and logs a warning.

#### B6.5. Missing / empty / whitespace-only / invalid fallback

| Stored value                              | Treated as                                           | Runtime fallback                  | Settings UI                              |
| ----------------------------------------- | ---------------------------------------------------- | --------------------------------- | ---------------------------------------- |
| Key absent / unset                        | Missing                                              | Built-in default from B1          | Field is empty; placeholder shows default |
| `""` (empty string)                       | Equivalent to missing                                | Built-in default from B1          | Field is empty; placeholder shows default |
| Whitespace-only (e.g., `" "`, `"\t\n"`)   | **Invalid** — effectively empty given the whitespace floor in B5 | Built-in default from B1; warning logged | Inline error: "Add at least one non-whitespace delimiter, or clear the field to use the default." |
| Some non-whitespace + any standard whitespace | Valid                                                | Effective `D` = user set ∪ whitespace floor | Accepted                                  |
| Contains disallowed control char          | **Invalid**                                          | Built-in default from B1; warning logged | Inline error; save disabled              |

In all "valid" cases, B5's whitespace floor still applies — whitespace IS forced into the effective set even if the user did not include it.

For per-context overrides (`terminal.word_delimiters`, `agent.word_delimiters`):

- Missing or empty: fall through to `editor.word_delimiters` per B4.
- Whitespace-only or disallowed-control: invalid; runtime falls through to `editor.word_delimiters` per B4 (NOT to the global default directly), with a warning. Settings UI shows the same inline error.

### B7. Delimiter run collapse

Consecutive delimiter characters are treated as a single boundary run (a single delimiter run per B2). Example with the default set on input `"foo//bar|"` (cursor at end):

- Press 1: char left = `r` (∉ D). Rule 3 → consume `bar`. Buffer: `"foo//|"`.
- Press 2: char left = `/` (∈ D). Rule 2 → consume the delimiter run `//`, then the non-delimiter run `foo`. Buffer: `"|"`.

The `//` run is consumed atomically as one delimiter run; it never produces an intermediate stop. This matches the canonical algorithm in B2 and aligns with VS Code, Sublime, and JetBrains conventions.

## Settings / API surface

| Key                        | Type     | Default                      | Notes                                                  |
| -------------------------- | -------- | ---------------------------- | ------------------------------------------------------ |
| `editor.word_delimiters`   | `string` | `"/.-_:= \t\n\r\x0b\x0c"`    | Global default. See B6 for value-string semantics, escape sequences, validation, and missing/empty/whitespace-only/invalid handling. Whitespace floor (B5) always applies. |
| `terminal.word_delimiters` | `string` | unset (falls back to editor) | Override for terminal input. Per B4 + B6.5, missing/empty/invalid falls through to `editor.word_delimiters` (NOT directly to the built-in default). |
| `agent.word_delimiters`    | `string` | unset (falls back to editor) | Override for agent prompt input. Same fallthrough as terminal. |

UI placement: **Settings → Editor → "Word Boundary Characters"**. Single-line text input with a "Reset to default" button and a "Show escapes" toggle (per B6.3). Validation per B6.4 — disallowed control characters trigger an inline error and disable save. Whitespace-only or empty inline error per B6.5. Below the input, a live preview line:

```
/var/www/example.com    ←| ←| ←| ←|
```

Arrows render at each position Delete Word Left would land using the current setting value, giving immediate feedback. The preview reflects the effective `D` (user value ∪ whitespace floor) so the user can see the real boundaries.

Per-context overrides are surfaced under their respective Terminal and Agent settings sections, each with the same input + preview + Show-escapes pattern and a "Use editor default" reset button.

No new keybindings, command palette actions, or context flags are added.

## Acceptance Criteria

All criteria below match the canonical algorithm in B2.

- **A1.** Delete Word Left on `/var/www/example.com|` (cursor at end) with the default `D` produces, on successive presses: `/var/www/example.|` → `/var/www/|` → `/var/|` → `/|` → `|`. (5 presses to fully clear; matches B2 worked example.)
- **A2.** Delete Word Right is the mirror. On `|/var/www/example.com` (cursor at start) with the default `D`, successive presses produce: `|/var/www/example.com` (Rule 3 — char right=`/` ∈ D — consume `/` then `var`) → `|/www/example.com` → `|/example.com` → `|.com` → `|com` → `|`.
- **A3.** With default set, on `/path/to/file|` the first Delete Word Left removes only `file`, leaving `/path/to/|`. (Rule 3: char left=`e` ∉ D, consume just the non-delimiter run `file`.)
- **A4.** Per-context override applies only in that input context; other contexts use the editor default or their own override (per B4).
- **A5.** Missing/empty setting value falls back to the built-in default from B1 (per B6.5).
- **A6.** Whitespace-only setting value is rejected as invalid; runtime falls back to the built-in default and logs a warning; Settings UI shows inline error (per B6.5).
- **A7.** Whitespace remains a delimiter even when the user provides a value with no whitespace; the whitespace floor is always present in the effective `D` (per B5).
- **A8.** Delimiter run collapse: consecutive delimiters form a single boundary run consumed atomically by Rule 2 (per B7).
- **A9.** Move Cursor Word Left / Right use the same `D` and the same run rules as Delete Word Left / Right (per B3).
- **A10.** Setting changes take effect for the next keystroke without app restart.
- **A11.** Setting persists across app restarts.

## Implementation Pointers

- **Boundary module.** Add or extend `app/src/editor/word_boundary.rs` exposing a `WordBoundaryClassifier` that takes the resolved delimiter string and provides `is_delimiter(char) -> bool` plus helper iterators for left/right boundary search.
- **Resolution helper.** Add `resolve_word_delimiters(context: InputContext) -> String` in the same module, implementing the B4 fallback chain. `InputContext` enum: `Terminal`, `Agent`, `Editor`.
- **Key handler dispatch.** Update `app/src/editor/key_handler.rs` so the four shortcut handlers (Delete Word Left, Delete Word Right, Move Cursor Word Left, Move Cursor Word Right) call into `WordBoundaryClassifier` instead of the existing whitespace-only check.
- **Whitespace floor.** Inside `WordBoundaryClassifier`, OR the user's set with the fixed whitespace set so B5 holds without callsite branching.
- **Settings schema.** Add the three keys to `app/src/settings/editor.rs` (and matching terminal/agent settings modules). All three are optional `Option<String>` except `editor.word_delimiters`, which has the literal default.
- **Settings UI.** Add the input row plus live-preview component under `app/src/settings_view/editor_page.rs`. Reuse the existing text-input component; the preview is a small custom widget rendering arrow markers at each computed boundary.
- **No persistence migration** — these are additive optional keys.

## Tests

All tests assume the canonical algorithm in B2.

### Delete Word Left

- **T1.** Default behavior on `/var/www/example.com|`: 5 successive Delete Word Left presses produce `/var/www/example.|`, `/var/www/|`, `/var/|`, `/|`, `|` (matches A1).
- **T2.** Custom delimiter set `D = {":"}` (whitespace floor still applies) on `key:value:other|`: presses produce `key:value:|`, `key:|`, `|`.
- **T3.** Run collapse on `foo//bar|` with default set: press 1 → `foo//|`; press 2 → `|`. (Two presses total — `//` is a single delimiter run consumed atomically with the preceding non-delimiter run by Rule 2.)
- **T4.** Cursor mid-non-delimiter run on `var|/www` with default set: Delete Word Left consumes only `var` from cursor leftward (Rule 3), result `|/www`.

### Delete Word Right

- **T8a.** Default set, `|/var/www/example.com`: 6 successive Delete Word Right presses produce `|/www/example.com`, `|/example.com`, `|.com`, `|com`, `|`. (Mirror of A1; matches A2.)
- **T8b.** Custom delimiter set `D = {"/"}` (whitespace floor still applies) on `|/foo/bar/baz`: presses produce `|/bar/baz`, `|/baz`, `|`.
- **T8c.** Delete Word Right at end-of-line on `foo|` is a no-op.
- **T8d.** Cursor mid-non-delimiter run on `foo|bar/baz` with default set: Delete Word Right consumes only `bar` (Rule 2 in DWR — char right=`b` ∉ D), result `foo|/baz`.

### Move Cursor Word Left / Right

- **T7.** Move Cursor Word Left / Right land at the same boundaries Delete Word Left / Right would operate on, for identical input and setting (per A9 / B3). Verify on the A1 and A2 examples.
- **T7a.** Move Cursor Word Right on `foo|/bar/baz` with default set lands at `foo/|bar/baz` after the first press, `foo/bar/|baz` after the second, `foo/bar/baz|` after the third.

### Settings resolution

- **T9a.** Context override precedence: set `editor.word_delimiters = "/"` and `terminal.word_delimiters = "."`. Assert terminal uses `.` ∪ whitespace; agent uses `/` ∪ whitespace (falls back to editor); editor uses `/` ∪ whitespace.
- **T9b.** Missing key fallback: leave `editor.word_delimiters` unset; assert default `"/.-_:= \t\n\r\x0b\x0c"` applies (per B6.5).
- **T9c.** Empty string fallback: set `editor.word_delimiters = ""`; assert default applies; no error logged.
- **T9d.** Whitespace-only invalid: set `editor.word_delimiters = "  \t"`; assert default applies; warning logged; Settings UI shows inline error (per B6.5).
- **T9e.** Disallowed control char invalid: set `editor.word_delimiters = "/\x07"`; assert default applies; warning logged; Settings UI shows inline error.
- **T9f.** Whitespace floor enforced: set `editor.word_delimiters = "/"` (no whitespace). Assert Delete Word Left still stops at spaces and newlines (per A7 / B5).
- **T9g.** Per-context override invalid → falls through to editor (not directly to default): set `editor.word_delimiters = "/"`, `terminal.word_delimiters = "  "`. Assert terminal uses editor's `/`, NOT the built-in default.

### TOML escape sequences

- **T10a.** TOML basic-string escapes: `editor.word_delimiters = "/.\t\n"` parses to set `{ '/', '.', '\t', '\n' }` ∪ whitespace floor.
- **T10b.** Literal whitespace and escape are equivalent: a literal space character and `" "` typed via the field both add space to the set (already in floor; verify no duplication issue).

### Runtime behaviors

- **T11.** Live effect: change `editor.word_delimiters` at runtime; assert next Delete Word Left keystroke uses the new set without restart.
- **T12.** Persistence: set custom value, restart app, assert value preserved and applied.
- **T13.** Multi-byte safety: with a delimiter set containing only ASCII, UTF-8 multi-byte characters (e.g., emoji, CJK) are treated as non-delimiters and not split mid-character.

## Open Questions

- **Regex / predicate boundaries (V1.5).** Should advanced users be able to define word boundaries via a regex or pattern instead of a literal character list? Recommendation: **yes, deferred to V1.5** behind a separate setting key (`editor.word_delimiter_pattern`) that, when set, takes precedence over the literal list. Out of scope for V1.
- **Unicode-aware default.** Should the default include common Unicode punctuation (em-dash, curly quotes)? Recommendation: defer pending user feedback; the literal-character model handles this once we have a concrete request.
- **Per-language overrides.** Should code-aware contexts (e.g., agent reasoning over Rust source) have language-tuned defaults? Out of scope; revisit if signal emerges.

## Telemetry

No new telemetry events. Standard `setting.changed` coverage on the three new keys is sufficient to gauge adoption and detect misconfiguration patterns.
