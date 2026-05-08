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
- `editor.copy_path_with_line` — copies the relative path with `:line[:column]` suffix where `column` is appended only when the cursor has an explicit column position (e.g., `src/main.rs:42:7`; `src/main.rs:42` if no column).

### B2. Project root resolution

"Project root" is the same root used by the file tree:

1. The git toplevel (`git rev-parse --show-toplevel`) of the editor file's parent directory.
2. If not in a git repo, the workspace folder containing the file.
3. If neither resolves, the action falls back per B4.

### B3. No file open

If no file is open in the editor when the action fires, it is a no-op and surfaces a brief toast: "No editor file open".

### B4. File outside project root

If the file resolves but is OUTSIDE the project root (e.g., a file opened from `/tmp` while a project is active), `editor.copy_relative_path` and `editor.copy_path_with_line` fall back to the absolute path and surface a toast: "File is outside project root — copied absolute path instead". `editor.copy_absolute_path` is unaffected.

### B5. Path separator

The copied string uses the OS-native separator:

- macOS / Linux: forward slash (`/`).
- Windows: backslash (`\`).

### B6. Confirmation toast

On success, each action surfaces a brief toast: `Copied: <truncated path>` where the path is truncated with a leading ellipsis if it exceeds 60 characters.

### B7. Editor focus context

The keymap bindings are active only when the editor pane has focus (`KeyContext = EditorFocused`). Bindings configured in Settings → Keyboard Shortcuts only fire under this context.

### B8. Command Palette discoverability

All three actions are visible in the Command Palette unconditionally and searchable by "copy path". When invoked from the Command Palette while the editor is not focused, B3's no-op-with-toast behavior applies.

## Settings / API surface

Three new keymap entries (default unbound):

- `editor.copy_absolute_path`
- `editor.copy_relative_path`
- `editor.copy_path_with_line`

UI: Settings → Keyboard Shortcuts → searching "copy path" reveals all three. Users assign chords as they would for any other action.

No new user-level toggle is introduced; the actions are always available.

## Acceptance Criteria

- A1. Each bound action copies the correct path to the system clipboard.
- A2. `editor.copy_relative_path` produces a path relative to the git toplevel when the file is inside a git repo.
- A3. A file opened from outside the project root falls back to absolute path with the B4 toast.
- A4. With no file open, each action is a no-op and surfaces the B3 toast.
- A5. Copied paths use the OS-native separator.
- A6. `editor.copy_path_with_line` includes column only when the cursor has an explicit column position (otherwise `:line` only).
- A7. All three actions appear in the Command Palette without requiring a binding.

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

## Open Questions

- Should we also expose a POSIX-style normalization toggle on Windows (so users who paste paths into WSL/Git Bash get forward slashes regardless of OS)? Suggest deferring to V1.5 with a per-action setting `editor.copy_path.posix_separator_on_windows` (bool, default `false`).
- Should `editor.copy_path_with_line` also expose an absolute variant? Likely yes in V1.5; not in V1 to avoid action-list bloat.

## Telemetry

Reuse existing `command_palette.action_invoked` event with the action ID payload — no new telemetry events are needed. The three new action IDs become valid values for that event's `action_id` field.
