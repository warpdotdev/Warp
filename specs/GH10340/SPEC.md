# Folder Tree View for Code Review File List (GH-10340)

## Summary

Add a VS Code-style folder-grouped tree view to the Code Review changed-files
list, with a header toggle between Flat and Tree modes. Tree mode supports
collapsible folders, compact single-child folder chains, folder-level change
counts and aggregate `+/-` line counts, and preserves selection across mode
switches.

## Problem

The Code Review changed-files list is currently flat. For PRs that touch many
files spread across nested directories, this flat layout makes it hard to
scan changes by area, hard to scope review per-package, and hides the shape of
the change. VS Code's Explorer-style tree view is the expected mental model
for a hierarchical changed-files list.

## Goals

- Tree view of changed files grouped by directory.
- Collapsible folders with expand/collapse affordance.
- User-visible toggle to switch between Flat and Tree views.
- Preserve current selection when switching between modes.
- Folder-level summaries: count of changed files and aggregate `+N -M` line
  counts where available.
- Compact-folder behavior matching VS Code (collapse single-child chains).

## Non-Goals

- Not a full file explorer; this is scoped to the Code Review changed-files
  list only.
- No new git operations exposed from the tree (stage/discard/etc.).
- Not synchronizing folder expansion state across sessions in V1; tracked as
  V1.5 (see Open Questions).
- No symbol-level / hunk-level grouping; entries remain at file granularity.

## Behavior Contract

### B1. Mode toggle

A header button group "Flat | Tree" switches the file list between modes.
Default is Flat to preserve current behavior. The user's choice persists per
user (across restarts).

### B2. Tree rendering and compact folders

In Tree mode the file list renders as a hierarchy of folders rooted at the
review's repo root. Single-child folder chains collapse into a compact label
matching VS Code's Explorer (e.g. `app/src/foo/bar/baz.rs` displays under a
compact folder row `app/src/foo/bar` when no other siblings exist at the
intermediate levels). A "Compact folders" overflow option (default ON)
toggles this behavior. When OFF, every directory level renders as its own
row.

### B3. Folder rows

Each folder row shows:

- folder name (or compact path),
- count of changed files contained (recursive), and
- aggregate `+N -M` line counts when the diff stats are available.

### B4. File rows

Each file row under a folder shows:

- filename,
- change kind indicator (M / A / D / R),
- per-file `+N -M` line counts, and
- status icon.

Selecting a file row opens the diff using the same routing as Flat mode.

### B5. Expand / collapse

- Click on a folder row toggles expand/collapse.
- Right arrow expands a collapsed folder; on an expanded folder it moves to
  the first child.
- Left arrow collapses an expanded folder; on a collapsed folder or file it
  moves to the parent.
- Cmd-click (macOS) / Ctrl-click (Windows/Linux) on a folder row toggles all
  descendants.

### B6. Search / filter

The existing search/filter input filters files in both modes. In Tree mode,
folders containing matches auto-expand to reveal the matches. Non-matching
siblings are dimmed by default; a "Show all" toggle preserves siblings as
fully visible.

### B7. Keyboard navigation

- Up/Down arrows navigate rows in display order (folders and files
  interleaved per the tree).
- Enter on a file opens the diff.
- Enter on a folder toggles its expanded state.
- Type-ahead jumps to the next row whose label matches the typed prefix.

### B8. Selection preservation across mode switch

Switching between Flat and Tree preserves the currently selected file. Its
row is auto-revealed in the new mode (parents expanded as needed in Tree
mode; row scrolled into view in Flat mode).

## Settings / API surface

- `code.review.file_list_mode` — `"flat"` | `"tree"`, default `"flat"`.
- `code.review.tree_compact_folders` — `bool`, default `true`.

UI placement:

- Header button group "Flat | Tree".
- Overflow menu entry "Compact folders" (toggle).

## Acceptance Criteria

- A1. Toggle Flat ↔ Tree switches the rendering correctly.
- A2. Default is Flat; existing users see no change until they opt in.
- A3. Compact-folders ON collapses single-child chains; OFF renders one row
  per level.
- A4. Folder counts and aggregate `+N -M` are correct (sum of descendants).
- A5. Currently selected file is preserved across mode toggle and revealed.
- A6. Search filter works in both modes; in Tree mode, parent folders of
  matching files auto-expand.
- A7. Keyboard navigation (arrows, Enter, Cmd-click) behaves per B5–B7.
- A8. `code.review.file_list_mode` and `code.review.tree_compact_folders`
  persist across restart.

## Implementation Pointers

- New code under `app/src/code_review/file_list/` for the tree data model and
  Tree-mode renderer; reuse the existing flat renderer.
- Build a tree from the flat path list once per file-list update; cache
  expanded-state by path for the active review.
- Reuse existing diff-open routing on file-row activation.
- Wire the header button group and overflow toggle into the existing Code
  Review header component.

## Tests

- T1. Tree construction from a flat path list (correctness of structure,
  parent links, file counts).
- T2. Compact-folder collapsing of single-child chains.
- T3. Mode toggle preserves selection (flat → tree → flat).
- T4. Search auto-expands ancestor folders of matching files.
- T5. Keyboard arrow navigation in Tree mode (including parent/child moves).
- T6. Folder-level `+/-` aggregation matches sum of descendants.
- T7. Cmd-click on folder toggles all descendants.
- T8. Settings persistence across restart for both new keys.
- T9. Tree construction performance: ≤16ms for 1000 changed files.

## Open Questions

- Should folder expansion state persist across review sessions for the same
  PR? Suggested: yes, in V1.5, keyed by `(repo, pr_id, folder_path)`.
- Should folder rows expose a "review folder" affordance (mark all files in
  folder as reviewed)? Out of scope for V1.

## Telemetry

Extend the existing `code_review.opened` event with a `file_list_mode`
field (`"flat"` | `"tree"`). No new event types are introduced.
