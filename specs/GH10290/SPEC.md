# Bindable Shortcut To Copy Editor File Path (GH-10290)

## Summary

Add three new bindable Command Palette actions for the Warp editor — "Copy Path" (absolute), "Copy Relative Path" (relative to project root), and "Copy Path with Line" (relative path with `:line[:column]`). Each is rebindable in Settings → Keyboard Shortcuts. All three default to unbound, matching Warp's pattern of leaving optional new actions free for the user to assign.

## Problem

Users frequently need to share or reference the path of the file currently open in Warp's editor — pasting it into a chat, a terminal command, an LLM prompt, or a bug report. Today there is no first-class action to copy the file path; users either drag the file from the tree, copy from the breadcrumb manually, or type the path. This issue specifically asks for a customizable keyboard shortcut to copy the *relative* path of the open editor file (matching the VS Code pattern users already know: `editor.action.copyPath` and `copyRelativeFilePath`).

## Goals

- Provide three discoverable, rebindable actions that copy the active editor file's path in the three formats users actually need.
- Make all three discoverable in the Command Palette regardless of binding state.
- Anchor "relative" to the same project root the file tree uses (git toplevel; fall back to workspace folder).
- Use OS-correct path separators by default.
- Provide brief confirmation toasts and friendly fallbacks when the action cannot complete.

## Non-Goals

- Changing or rebinding existing copy actions on terminal output.
- Copying GitHub-style URL paths (e.g., permalinks). Tracked as future work.
- Multi-file or multi-cursor "copy all open file paths" actions.
- Opening a picker dialog to choose between formats — each format is its own bindable action.

## Behavior Contract

### B1. New actions

Three new actions are registered in the keymap and Command Palette:

- `editor.copy_absolute_path` — copies the absolute path of the active editor file to the clipboard.
- `editor.copy_relative_path` — copies the path relative to the project root.
- `editor.copy_path_with_line` — copies the relative path with a `:line[:column]` suffix derived from the cursor or selection. See B9 for the full numbering and inclusion contract.

### B2. Project root resolution

"Project root" is resolved from the **active workspace's** root, not from
the editor file's parent directory. This separates "what is the project?"
(workspace state) from "where does this file live?" (filesystem path), and
makes the B4 outside-project fallback well-defined: a file is "outside the
project root" iff its absolute path is not a descendant of the active
workspace's resolved root.

Resolution order (the same root the file tree uses):

1. **Active workspace folder** — the folder backing the currently active
   Warp workspace (the same folder shown at the top of the file tree).
   This is the canonical "project root".
2. **Git toplevel of the workspace folder** — when the workspace folder is
   itself inside a git working tree, run `git rev-parse --show-toplevel`
   on the **workspace folder** (not the editor file's parent directory)
   and use the toplevel as the project root. This widens the root for
   monorepos where the workspace was opened on a sub-directory.
3. **No active workspace** — if Warp has no active workspace at all (e.g.,
   a single-file editor session opened outside any workspace), there is
   no project root. Behavior:
   - `editor.copy_relative_path` and `editor.copy_path_with_line` follow
     B4's outside-project fallback (copy absolute path; preserve `:N[:K]`
     for the line variant) with the B4 toasts.
   - `editor.copy_absolute_path` is unaffected.

Edge case — symlinks: both the workspace root and the editor file path are
canonicalized (resolved through symlinks) before the inside/outside
comparison. A file reached via a symlink whose target is inside the project
root is treated as inside the project; the relative path is computed from
the canonicalized paths.

### B3. No file open

If no file is open in the editor when the action fires, it is a no-op and surfaces a brief toast: "No editor file open".

### B4. File outside project root

If the file resolves but is OUTSIDE the project root (e.g., a file opened from `/tmp` while a project is active):

- `editor.copy_relative_path` falls back to the absolute path and surfaces a toast: "File is outside project root — copied absolute path instead".
- `editor.copy_path_with_line` falls back to the absolute path AND PRESERVES the line/column suffix using the same inclusion rules defined in B9. Output is `/abs/path/to/file:N` or `/abs/path/to/file:N:K` (depending on whether column is explicit). Toast: "File is outside project root — copied absolute path with line".
- `editor.copy_absolute_path` is unaffected (it always copies the absolute path).

### B5. Path separator

The copied string uses the OS-native separator:

- macOS / Linux: forward slash (`/`).
- Windows: backslash (`\`).

### B6. Confirmation toast

On success, each action surfaces a brief toast: `Copied: <truncated path>` where the path is truncated with a leading ellipsis if it exceeds 60 characters.

### B7. Editor focus context (key-bound invocation)

When invoked via a configured key binding, the action only fires when the editor pane has focus (`KeyContext = EditorFocused`). Bindings configured in Settings → Keyboard Shortcuts respect this context.

### B8. Command Palette discoverability and target editor

All three actions are visible in the Command Palette UNCONDITIONALLY (independent of editor focus) and are searchable by "copy path". The Command Palette itself moves keyboard focus to the palette input when it opens, so editor focus is necessarily lost while the palette is open. To resolve the resulting ambiguity about which editor the action targets:

- **Last-focused-editor tracking**: Warp tracks the "last focused editor" — the most recent editor pane that held focus before the Command Palette was opened. The application updates this tracker on every editor focus change.
- **Palette invocation target**: when one of the three copy-path actions is invoked from the Command Palette, the action targets the LAST-FOCUSED EDITOR captured at the moment the palette was opened. The palette dismisses on action invocation; focus returns to that editor.
- **No editor was previously focused**: if no editor pane had focus before the palette was opened (e.g., the palette was invoked from the file tree, settings, or a pane with no editor), the action is a no-op and surfaces the B3 toast: "No editor file open".
- **Visibility vs. execution**: actions remain visible in the palette regardless of the last-focused-editor state. Visibility is unconditional; execution requires a non-null last-focused editor with an open file.

### B9. `copy_path_with_line` numbering and suffix contract

`editor.copy_path_with_line` produces a deterministic clipboard string for a given cursor/selection state. The contract:

- **Line numbering**: 1-based — the first line is line `1`. This matches universal editor convention (vim, VS Code, IntelliJ).
- **Column numbering**: 1-based — the first column is column `1`. Columns count **Unicode extended grapheme clusters** (per UAX #29 GraphemeBreakProperty), NOT bytes and NOT raw codepoints. `<path>:N:K` always points to the K-th grapheme cluster of line N. Concretely:
  - A single ASCII character is one column.
  - A multi-byte UTF-8 character (e.g., `日`) is one column.
  - A base codepoint plus combining marks (e.g., `é` written as `e` + U+0301) is **one** column at the base — combining marks do NOT advance the column counter.
  - An emoji ZWJ sequence (e.g., 👨‍👩‍👧) is one column.
  - Tabs (`\t`) count as one column. Width-aware tab expansion is NOT performed.
  - This single grapheme-cluster model supersedes any earlier wording that said "codepoints" — the prior phrasing is replaced by this one.
- **When column is included** — driven by an explicit per-document boolean **`HasExplicitColumn`** that the editor maintains for each cursor position. The clipboard suffix is emitted iff `HasExplicitColumn == true`. The flag transitions are deterministic and source-of-input agnostic; provenance is not tracked at copy time:
  - `HasExplicitColumn := true` on: mouse click placing the cursor; horizontal arrow keys (Left/Right); Home/End; click-drag selection drop; programmatic positioning that supplied a column; type-edit that advances the column on the same line.
  - `HasExplicitColumn := false` on: a "go to line N" command that did not supply a column; opening a file (cursor restored to line 1, col 1 with no column intent); vertical arrow keys (Up/Down) that traveled across lines using the editor's "preferred column" memory without a fresh horizontal action since the last vertical move.
  - The previous wording listed "cursor at start of line (col 1)" and "cursor at end of line" as triggers for omitting the column. That wording was ambiguous because a deliberate click on col 1 and a fresh-from-go-to-line cursor at col 1 are indistinguishable by position alone. It is **superseded** by `HasExplicitColumn`: position alone never controls the suffix; only the boolean does.
- **No selection**: the active cursor's line and `HasExplicitColumn` apply.
- **Active selection**: the line and `HasExplicitColumn` of the SELECTION ANCHOR (where the selection began) apply — not the caret's. Reverse selections (anchor below caret) still use the anchor.
- **Output forms**: emit `<path>:<line>:<col>` when `HasExplicitColumn == true`, otherwise `<path>:<line>`. Both forms are valid outputs of the same action; which form is emitted is fully determined by `HasExplicitColumn` at invocation time.

## Settings / API surface

Three new keymap entries (default unbound):

- `editor.copy_absolute_path`
- `editor.copy_relative_path`
- `editor.copy_path_with_line`

UI: Settings → Keyboard Shortcuts → searching "copy path" reveals all three. Users assign chords as they would for any other action.

No new user-level toggle is introduced; the actions are always available.

## Acceptance Criteria

Bound-shortcut path:

- A1. Each bound action, invoked while the editor is focused, copies the correct path to the system clipboard.
- A2. `editor.copy_relative_path` produces a path relative to the git toplevel when the file is inside a git repo.
- A3. A file opened from outside the project root falls back to absolute path with the B4 toast (`copy_relative_path` and `copy_path_with_line`).
- A4. With no file open, each action is a no-op and surfaces the B3 toast.
- A5. Copied paths use the OS-native separator.
- A6. `editor.copy_path_with_line` includes the column suffix iff the editor's per-cursor `HasExplicitColumn` flag (B9) is `true`; otherwise it emits `<path>:N` only. When a selection is active, the anchor's flag and line/column are used. Columns are 1-based grapheme-cluster indices (UAX #29).
- A7. All three actions appear in the Command Palette without requiring a binding.

Outside-project + line/column path:

- A8. `editor.copy_path_with_line` invoked on a file outside the project root copies the absolute path AND preserves the `:N[:K]` suffix per B9, with the toast "File is outside project root — copied absolute path with line".

Command Palette path:

- A_palette_targets_last_focused_editor. When invoked from the Command Palette, the action targets the last-focused editor pane (captured at the moment the palette opened); the palette dismisses and focus returns to that editor.
- A_palette_no_editor_noop. When invoked from the Command Palette with no last-focused editor (palette opened from file tree, settings, or other non-editor surface), the action is a no-op and surfaces the B3 toast "No editor file open"; the action remains visible in the palette regardless.

## Implementation Pointers

Verified paths:

- Editor module: `app/src/code/editor/` (e.g., `app/src/code/editor/element.rs`, `app/src/code/editor/comment_editor.rs`) — host of editor focus state and active-file metadata.
- Command Palette: `app/src/command_palette.rs` and `app/src/search/command_palette/` — action registration and discoverability.
- Keybinding settings UI: `app/src/settings_view/keybindings.rs` — where users see/edit bindings.
- Existing keybinding view example: `app/src/editor/accept_autosuggestion_keybinding_view.rs`.
- Clipboard write API: `app/src/util/clipboard.rs` plus platform implementations under `crates/warpui/src/platform/mac/clipboard.rs`, `crates/warpui/src/windowing/winit/linux/clipboard.rs`, `crates/warpui/src/windowing/winit/windows/clipboard.rs`.
- Telemetry: existing `command_palette.action_invoked` is reused — extend payload with the new action IDs.

New modules:

- `app/src/code/editor/copy_path_actions.rs` (new) — the three action handlers, project-root resolver, and toast helpers.

## Tests

- T1. `copy_absolute_path` produces the absolute path for the active editor file.
- T2. `copy_relative_path` produces the path relative to the git toplevel.
- T3. `copy_path_with_line` produces `<relative>:line[:column]` correctly.
- T4. File outside project root falls back to absolute path with the B4 toast.
- T5. No-file case is a no-op with the B3 toast.
- T6. macOS/Linux uses forward slashes; Windows uses backslashes (platform-gated tests).
- T7. Rebinding the action in Settings → Keyboard Shortcuts persists across restarts.
- T8. All three actions are discoverable in the Command Palette even when unbound.
- T_line_only_after_goto. Run "Go to line N" with no column → `HasExplicitColumn = false` → output is `<path>:N` with NO column suffix.
- T_line_only_after_open. Open a fresh file; do not interact with the cursor → `HasExplicitColumn = false` → output is `<path>:1`.
- T_line_col_after_click_at_col_1. Mouse-click on line N column 1 → `HasExplicitColumn = true` → output is `<path>:N:1` (column IS emitted because the click set the flag, even though the position is col 1).
- T_line_col_after_arrow_horizontal. Cursor moved mid-line via arrow keys → `HasExplicitColumn = true` → output is `<path>:N:K`.
- T_line_col_after_vertical_only. From `<path>:N` (HasExplicitColumn = false), press Down arrow once → caret moves to next line via preferred-column memory; flag stays false → output is `<path>:N+1` (NO column).
- T_line_only_after_end_of_line_via_goto. Go-to-line places cursor at end of line → `HasExplicitColumn = false` → output is `<path>:N`. Pressing End from there sets the flag → subsequent copy emits `<path>:N:K`.
- T_selection_uses_anchor. With an active selection from anchor (line 5, col 3, HasExplicitColumn = true) to caret (line 12, col 1), output is `<path>:5:3`. Reverse selection (anchor below caret) still uses the anchor's line and flag.
- T_unicode_grapheme_columns. A line containing multi-byte UTF-8 ("héllo", "日本語"), combining marks (`e\u{0301}`), and a ZWJ emoji (`👨\u{200D}👩\u{200D}👧`) — column is the 1-based grapheme-cluster index per UAX #29, not bytes and not codepoints. Cursor after `日` on `日本語` → column 2. Cursor after `é` written as `e\u{0301}` → column 2 (one grapheme, not 3 codepoints). Cursor after the family ZWJ emoji → column 2.
- T_outside_project_with_line. `copy_path_with_line` invoked on a file outside the project root copies `/abs/path/to/file:N` (or `:N:K` if column is explicit) and surfaces the toast "File is outside project root — copied absolute path with line".
- T_palette_targets_last_focused_editor. With editor A focused, open Command Palette, run `editor.copy_relative_path` → clipboard contains the path of the file in editor A; focus returns to editor A after the palette dismisses.
- T_palette_no_editor_noop. With no editor previously focused (palette opened from file tree), run any of the three actions → no clipboard write, B3 toast surfaces, action still appeared in the palette list.

## Open Questions

- Should we also expose a POSIX-style normalization toggle on Windows (so users who paste paths into WSL/Git Bash get forward slashes regardless of OS)? Suggest deferring to V1.5 with a per-action setting `editor.copy_path.posix_separator_on_windows` (bool, default `false`).
- Should `editor.copy_path_with_line` also expose an absolute variant? Likely yes in V1.5; not in V1 to avoid action-list bloat.

## Telemetry

Reuse existing `command_palette.action_invoked` event with the action ID payload — no new telemetry events are needed. The three new action IDs become valid values for that event's `action_id` field.
