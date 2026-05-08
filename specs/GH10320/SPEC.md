# File Tree Search (GH-10320)

## Summary

Add an in-tree search box to the File Tree / Project Explorer, openable via
`Cmd+F` (`Ctrl+F` on Windows/Linux) when the File Tree has focus, and via a
discoverable magnifier button in the tree header. Live-filters the tree to
filename and path-component matches, auto-expands folders containing matches,
and supports next/previous-match navigation. Matches VS Code's Explorer
search behavior at the V1 level.

## Problem

The File Tree currently has no in-panel search. Users with large repos must
scroll or use the global Find (which is content search, not filename) to
locate files in the tree. The expected interaction is `Cmd+F` while the
File Tree is focused, plus a visible magnifier button — both consistent with
VS Code's Explorer.

## Goals

- In-tree search box accessible via `Cmd+F` when the tree has focus.
- Header magnifier button as a discoverable affordance for the same search.
- Live-filter the tree to nodes matching filename or relative-path
  components.
- Auto-expand folders that contain matches.
- Jump-to-match navigation (next / previous).
- Preserve prior selection and prior expand/collapse state when search is
  cleared.

## Non-Goals

- No full-text content search; the existing global Find continues to handle
  that.
- No regex or glob matching in V1; deferred to V1.5 (see Open Questions).
- No cross-workspace search.
- No fuzzy code-symbol search; this is filename / path search only.

## Behavior Contract

### B1. Activation

When the File Tree is the active focus context:

- `Cmd+F` (macOS) / `Ctrl+F` (Windows/Linux) opens a search box pinned at
  the top of the tree panel.
- A magnifier icon button in the tree header opens the same search box.
- `Esc` closes the search box and restores focus to the previously focused
  tree row.

The shortcut MUST NOT trigger the File Tree search when focus is in the
terminal, an editor, or other panes — those keep their existing `Cmd+F`
behaviors.

### B2. Match semantics

- Case-insensitive substring match.
- Match is checked against each path component (folder names and the file
  basename) and against the full relative path string.
- A folder match reveals all of the folder's children as in-context matches
  (folder-as-context).
- Matching is locale-insensitive for ASCII; Unicode normalized using NFC
  before matching.

### B3. Live filter

The tree updates as the user types:

- Folders containing matches auto-expand.
- The visibility of non-matching siblings is controlled by the boolean
  setting `file_tree.search.dim_non_matches` (default `true`):
  - When `true` (default): non-matching siblings remain visible but
    **dimmed** (de-emphasized) so the user keeps spatial context.
  - When `false`: non-matching siblings are **hidden** entirely; only
    matching files plus their ancestor folders (auto-expanded) are
    rendered.
- A match count appears next to the search box (e.g. "12 matches").

### B4. Navigation

- `F3` or `Enter` jumps to the next match.
- `Shift+F3` or `Shift+Enter` jumps to the previous match.
- The active match is visually highlighted distinctly from other matches.
- The active match scrolls into view if not already visible.
- Wraps from last → first and first → last.

### B5. Clear and restore

Clearing the search input (deleting all characters) restores:

- the prior tree expand/collapse state, and
- the prior selected row.

Closing the search box via `Esc` does the same.

### B6. Empty match

When the query yields no matches:

- A "No matches" indicator appears next to the search box.
- With `file_tree.search.dim_non_matches = true` (default): the tree is
  fully dimmed.
- With `file_tree.search.dim_non_matches = false`: the tree renders empty
  (only the panel chrome remains visible).

### B7. Persistence

The search query is NOT persisted across closing and reopening the tree
panel. Each panel-open starts with an empty search.

## Settings / API surface

- `file_tree.search.dim_non_matches` — `bool`, default `true`. When
  `true`, non-matching siblings are dimmed (visible but de-emphasized).
  When `false`, non-matching siblings are hidden entirely (only matching
  files and their ancestor folders are rendered).
- Keybinding registration for `Cmd+F` (mac) / `Ctrl+F` (win/linux), scoped
  to a new keymap context flag `"FileTreeFocused"` (added via
  `context.set.insert("FileTreeFocused")` in the file-tree view's
  `keymap_context` impl) so it does not collide with editor or terminal
  `Cmd+F`.

## Acceptance Criteria

- A1. `Cmd+F` opens the search box when the File Tree has focus.
- A2. The magnifier button in the tree header opens the search box.
- A3. Live-filter updates per keystroke within ≤16ms for a 5,000-node tree.
- A4. Folders auto-expand to reveal matches as the user types.
- A5. Setting `file_tree.search.dim_non_matches` switches between dimming
  (`true`, default — non-matches visible but dimmed) and hiding (`false`
  — non-matches hidden entirely).
- A6. `Enter` / `F3` cycles to the next match; `Shift+Enter` / `Shift+F3`
  cycles to the previous; wraps at the ends.
- A7. `Esc` closes the search and restores prior focus and state.
- A8. Clearing the input restores the prior selection and expand state.
- A9. The match count is visible while the search box is open.

## Implementation Pointers

> Paths verified against worktree at commit `86940541`. If reorganizing,
> update tooling accordingly (no behavior change).

- **New module** `app/src/code/file_tree/search.rs` for the matching
  pipeline (substring match, NFC normalization, ancestor expansion
  tracking) and match-list state.
- **Update existing view** `app/src/code/file_tree/view.rs`
  (`FileTreeView`, ~3,170 lines) for the filter + highlight render
  pipeline, auto-expand on match, prior-state snapshot, and search-box
  state hookup. The render path lives in
  `app/src/code/file_tree/view/render.rs`.
- **Magnifier button**: there is no separate `header.rs` today — header
  rendering is inlined inside `app/src/code/file_tree/view.rs` (search for
  the existing toolbar / header row; the magnifier button is added as a
  new icon child of that row).
- **Container view**: `app/src/workspace/view/left_panel.rs`
  (`LeftPanelView`) hosts the `FileTreeView`; no changes there beyond
  forwarding focus state if needed.
- **Keymap context**: register the new context-set flag
  `"FileTreeFocused"` via the `keymap_context` impl associated with
  `FileTreeView` (pattern: `context.set.insert("FileTreeFocused")` —
  consistent with existing flags like `"EditorFocused"` set in
  `app/src/terminal/view.rs:26640`). Bind `Cmd+F` / `Ctrl+F` to the new
  search action under predicate `id!("FileTreeFocused")` so it does not
  collide with editor / terminal `Cmd+F`.
- **Snapshot prior state**: snapshot expand/collapse state and selected
  row when the search box opens; restore on `Esc` / clear (per B5). Reuse
  the existing `FileTreeView` expansion-state container.
- **Settings**: add `file_tree.search.dim_non_matches: bool` (default
  `true`) to the file-tree settings group. Surface a toggle in
  `app/src/settings_view/code_page.rs` under the file-tree section
  (there is no separate `editor_page.rs`; `code_page.rs` houses
  code/file-tree settings).
- **No persistence migration** — the new setting is additive with a
  default.

## Tests

- T1. `Cmd+F` with file-tree focus opens the search box; with terminal /
  editor focus it does not.
- T2. Substring match across both file basenames and folder names.
- T3. Auto-expand of all ancestor folders on a match.
- T4. Next / previous navigation cycles, including wraparound.
- T5. `Esc` restores prior focus and tree state.
- T6. Setting `file_tree.search.dim_non_matches` switches between dimming
  (`true`) and hiding (`false`) behavior. Default value is `true`.
- T7. Clearing the input restores the prior selection and expand state.
- T8. Performance: live filter updates ≤16ms per keystroke on a
  5,000-node tree.
- T9. Unicode + locale-insensitive matching (NFC normalized).

## Open Questions

- Should results also support diff / git status filters (e.g. "show only
  modified files")? Suggested: V1.5 as a separate filter row, not part of
  the substring search.
- Should the search support a `path:` prefix to scope matching to path
  components only? Deferred.

## Telemetry

The actual existing telemetry event for file-tree open/close is
`TelemetryEvent::FileTreeToggled` (label "File Tree Toggled" — see
`app/src/server/telemetry/events.rs:5658`); there is no
`file_tree.opened` event today.

Two options, in order of preference:

1. **Preferred — extend the existing `FileTreeToggled` variant** in
   `app/src/server/telemetry/events.rs` with an optional
   `search_used: bool` field that is `true` if the user opened the
   search box at any point during the tree-panel session. No new event
   type introduced.
2. **Alternative — add a net-new event** `FileTreeSearchUsed` (label
   `"FileTree.SearchUsed"`) emitted once per tree-panel session if the
   user opened the search box. Use this only if extending
   `FileTreeToggled` is awkward for the existing call sites.

Pick option 1 unless implementation-time review shows the existing
variant is consumed in a way that makes adding the field risky.
