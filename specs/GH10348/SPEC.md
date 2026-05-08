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

### B2. Word-deletion semantics

For Delete Word Left and Delete Word Right, characters in the delimiter set are word boundaries. Delete Word Left removes characters from the cursor leftward until (and including) the next delimiter run, OR start-of-line, whichever comes first. Delete Word Right is the mirror: rightward until a delimiter run or end-of-line.

### B3. Word-navigation semantics

Move Cursor Word Left / Right use the **same** delimiter set as deletion. Delimiter characters mark boundaries; the cursor lands at the boundary between a delimiter run and a non-delimiter run.

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

Whitespace characters (space, tab, newline, carriage return, vertical tab, form feed) are **always** treated as delimiters, regardless of the configured set. Removing them from the setting string has no effect on whitespace-as-delimiter behavior. This prevents users from accidentally configuring a state where Delete Word Left deletes across line breaks or runs forever.

### B6. Empty / missing setting fallback

An empty string or a missing setting falls back to the default from B1. Whitespace-only strings also fall back (since whitespace is already always a delimiter — an effectively empty configuration).

### B7. Delimiter run collapse

Consecutive delimiter characters are treated as a single boundary run. Example with the default set on input `"foo//bar"`:

- Cursor at end of `bar` → Delete Word Left removes `bar`. Cursor is now between `//` and end. Buffer: `"foo//"`.
- Delete Word Left again removes the `//` run. Buffer: `"foo"`.
- Delete Word Left again removes `foo`. Buffer: `""`.

This three-step pattern (non-delimiter → delimiter run → non-delimiter) matches VS Code, Sublime, and JetBrains conventions.

## Settings / API surface

| Key                        | Type     | Default                      | Notes                                                  |
| -------------------------- | -------- | ---------------------------- | ------------------------------------------------------ |
| `editor.word_delimiters`   | `string` | `"/.-_:= \t\n\r\x0b\x0c"`    | Global default. Whitespace always implicit.            |
| `terminal.word_delimiters` | `string` | unset (falls back to editor) | Override for terminal input.                           |
| `agent.word_delimiters`    | `string` | unset (falls back to editor) | Override for agent prompt input.                       |

UI placement: **Settings → Editor → "Word Boundary Characters"**. Single-line text input with a "Reset to default" button. Below the input, a live preview line:

```
/var/www/example.com    ←| ←| ←| ←|
```

Arrows render at each position Delete Word Left would land using the current setting value, giving immediate feedback.

Per-context overrides are surfaced under their respective Terminal and Agent settings sections, each with the same preview pattern and a "Use editor default" reset button.

No new keybindings, command palette actions, or context flags are added.

## Acceptance Criteria

- **A1.** Default delimiter set produces editor-conventional behavior: Delete Word Left from end of `/var/www/example.com` removes `com`, then `.`, then `example`, then `.`, then `www`, then `/`, then `var`, then `/`, then everything before.
- **A2.** With default set, typing `/path/to/file` and pressing Delete Word Left stops at the last `/` (removes only `file`).
- **A3.** Per-context override applies only in that input context; other contexts use the editor default or their own override.
- **A4.** Empty or missing setting value falls back to the default.
- **A5.** Whitespace remains a delimiter even when the user removes it from the configured string.
- **A6.** Delimiter run collapse: consecutive delimiters form a single boundary run (per B7).
- **A7.** Move Cursor Word Left / Right use the same delimiter set as Delete Word Left / Right (parity).
- **A8.** Setting changes take effect for the next keystroke without app restart.
- **A9.** Setting persists across app restarts.

## Implementation Pointers

- **Boundary module.** Add or extend `app/src/editor/word_boundary.rs` exposing a `WordBoundaryClassifier` that takes the resolved delimiter string and provides `is_delimiter(char) -> bool` plus helper iterators for left/right boundary search.
- **Resolution helper.** Add `resolve_word_delimiters(context: InputContext) -> String` in the same module, implementing the B4 fallback chain. `InputContext` enum: `Terminal`, `Agent`, `Editor`.
- **Key handler dispatch.** Update `app/src/editor/key_handler.rs` so the four shortcut handlers (Delete Word Left, Delete Word Right, Move Cursor Word Left, Move Cursor Word Right) call into `WordBoundaryClassifier` instead of the existing whitespace-only check.
- **Whitespace floor.** Inside `WordBoundaryClassifier`, OR the user's set with the fixed whitespace set so B5 holds without callsite branching.
- **Settings schema.** Add the three keys to `app/src/settings/editor.rs` (and matching terminal/agent settings modules). All three are optional `Option<String>` except `editor.word_delimiters`, which has the literal default.
- **Settings UI.** Add the input row plus live-preview component under `app/src/settings_view/editor_page.rs`. Reuse the existing text-input component; the preview is a small custom widget rendering arrow markers at each computed boundary.
- **No persistence migration** — these are additive optional keys.

## Tests

- **T1.** Default behavior on `/var/www/example.com`: Delete Word Left from end pops segments per A1.
- **T2.** Custom delimiter set (`":"` only): on `key:value:other`, Delete Word Left stops at each `:`.
- **T3.** Context override precedence: set `editor.word_delimiters = "/"` and `terminal.word_delimiters = "."`. Assert terminal uses `.`, agent uses `/` (falls back to editor), editor uses `/`.
- **T4.** Empty setting fallback: set `editor.word_delimiters = ""`, assert default behavior.
- **T5.** Whitespace forced as delimiter: set `editor.word_delimiters = "/"` (no whitespace). Assert Delete Word Left still stops at spaces and newlines.
- **T6.** Run collapse on `"foo//bar"`: three Delete Word Left presses produce `"foo//"`, then `"foo"`, then `""`.
- **T7.** Word-navigation parity: Move Cursor Word Left / Right land at the same boundaries Delete Word Left / Right operate on, for the same input and setting.
- **T8.** Live effect: change setting at runtime, assert next keystroke uses new boundary set.
- **T9.** Persistence: set custom value, restart, assert value preserved and applied.
- **T10.** Multi-byte safety: with a delimiter set containing only ASCII, ensure UTF-8 multi-byte characters (e.g., emoji, CJK) are treated as non-delimiters and not split mid-character.

## Open Questions

- **Regex / predicate boundaries (V1.5).** Should advanced users be able to define word boundaries via a regex or pattern instead of a literal character list? Recommendation: **yes, deferred to V1.5** behind a separate setting key (`editor.word_delimiter_pattern`) that, when set, takes precedence over the literal list. Out of scope for V1.
- **Unicode-aware default.** Should the default include common Unicode punctuation (em-dash, curly quotes)? Recommendation: defer pending user feedback; the literal-character model handles this once we have a concrete request.
- **Per-language overrides.** Should code-aware contexts (e.g., agent reasoning over Rust source) have language-tuned defaults? Out of scope; revisit if signal emerges.

## Telemetry

No new telemetry events. Standard `setting.changed` coverage on the three new keys is sufficient to gauge adoption and detect misconfiguration patterns.
