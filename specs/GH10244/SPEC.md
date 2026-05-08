# Shift-click bulk selection + bulk delete in Warp Drive (GH-10244)

## Summary

Warp Drive (saved objects: workflows, notebooks, env-var collections, Plans)
lacks multi-select. Users want familiar file-list ergonomics: Shift-click for
contiguous range, Cmd/Ctrl-click for individual toggle, plus a compact
selection bar with bulk Delete and bulk Move. This spec defines the V1
selection model, the bulk action set, keyboard parity, and how selection
interacts with filtering.

## Problem

- Drive items (especially AI-generated Plans) accumulate quickly.
- The only way to delete N items today is one at a time.
- There is no way to move multiple items into a folder in one gesture.
- The interaction model differs from every other "list of saved things" UI
  the user already knows.

## Goals

- Multi-select via Shift-click (range), Cmd/Ctrl-click (toggle).
- `Cmd+A` to select all visible/filtered, `Esc` to clear.
- Compact selection bar at ≥2 selected with Delete, Move to…, and Cancel.
- Bulk Delete with confirmation; bulk Move with folder picker.
- Selection persists across filter narrowing so a search-then-select-all flow
  works correctly.
- Keyboard parity for arrow navigation, `Shift+Arrow` extension, `Space`
  toggle, `Enter` open.

## Non-Goals

- No bulk edit of metadata (rename, retag) in V1; deferred to V1.5.
- No server-side admin bulk tools.
- No in-app undo window for Plan deletion (Plans are higher-stakes than chat
  history; undo is intentionally NOT mirrored from #10457).
- No change to single-item behavior (open, run, share) at all.
- Not the same flow as PR #10457 (chat history bulk delete) — Drive selection
  is its own model.

## Behavior Contract

### B1. Single-click selects one row

A plain click on a row clears any existing multi-selection and selects that
one row. The clicked row becomes the new "anchor" for future Shift extension.

### B2. Cmd/Ctrl-click toggles a single row

`Cmd-click` on macOS / `Ctrl-click` on Windows/Linux toggles the clicked row
in or out of the selection set. Anchor remains at the previous anchor; if no
anchor exists, the toggled row becomes the anchor.

### B3. Shift-click extends a contiguous range

`Shift-click` selects every row from the anchor through the clicked row,
inclusive. Anchor stays put on subsequent shift-clicks (so multiple
shift-clicks adjust the range from the same anchor). First shift-click with
no prior anchor sets the anchor to the clicked row.

### B4. Cmd/Ctrl+A selects all matching

`Cmd+A` (mac) / `Ctrl+A` (win/linux) selects every row in the currently
visible filtered set. With no filter active, it selects everything in the
current Drive view.

### B5. Esc clears

`Esc` clears selection and anchor and dismisses the selection bar.

### B6. Selection bar appears at ≥2 selected

When selection size is ≥2, a compact bar pins to the top of the Drive panel
with: `<N> selected · Delete · Move to… · Cancel`. The bar disappears when
selection drops to 0 or 1. At exactly 1 selected, normal single-row affordances
apply (no bulk bar).

### B7. Bulk Delete

Triggered from the selection bar Delete action. Shows a modal:
`Delete <N> items? This cannot be undone.` `[Delete] [Cancel]`.

- No 5-second undo window (deliberately differs from chat-history bulk delete).
- Per-item failures are collected and surfaced in a result toast:
  `Deleted X. Y failed.` with a link to view failed items.
- Recommendation in Open Questions: server-side soft tombstone for 30 days
  is a separate decision.

### B8. Bulk Move to folder

Triggered from the selection bar Move to… action. Opens the existing folder
picker. Moves all selected items to the chosen folder.

- Items already in the destination folder are a silent no-op (not an error).
- Mixed-type selections are allowed; folder picker is the same.

### B9. Selection survives filtering

Items selected before a filter/search narrows the visible set REMAIN
selected even when filtered out of view. The selection bar shows
`<visible> + <hidden> selected` so the user can see the hidden count and
decide whether to clear before acting. Bulk actions still apply to all
selected items, visible or not.

### B10. Keyboard navigation

- `Up` / `Down` move focus (single-select replaces previous selection).
- `Shift+Up` / `Shift+Down` extend selection contiguously from anchor.
- `Space` toggles the focused row's membership in the multi-selection.
- `Enter` opens the focused item.
- `Cmd+A`, `Esc` as defined above.

## Settings / API surface

No new user-facing settings. Internal additions:

- `WarpDriveSelection` model holding `{ anchor: Option<RowId>, set: HashSet<RowId> }`.
- Bulk delete and bulk move route through existing per-item endpoints with a
  client-side iterator bounded to a sane batch (e.g. 200 per chunk).
- A server-side batch endpoint may already exist; if so, use it and let the
  client fall back to serial calls.

## Acceptance Criteria

- A1: Shift-click between two rows selects every row in between, inclusive,
  and the anchor remains at the original anchor row.
- A2: Cmd/Ctrl-click toggles individual rows in/out without disturbing the
  rest of the set.
- A3: `Cmd+A` selects all currently visible rows when a filter is active and
  all rows otherwise.
- A4: `Esc` clears selection and dismisses the selection bar.
- A5: Selection bar appears when selection is ≥2 and disappears at 0 or 1.
- A6: Bulk delete shows the confirm modal and removes all selected items;
  per-item failures surface in a result toast.
- A7: Bulk move opens the folder picker and moves all selected items.
- A8: Selection persists when the user types into the filter / clears the
  filter.
- A9: Keyboard navigation full path: arrow movement, `Shift+Arrow` extension,
  `Space` toggle, `Enter` open.

## Implementation Pointers

Verified paths (via `git ls-files`):

- Drive surfaces:
  - `app/src/settings_view/warp_drive_page.rs` — settings-view list of saved
    objects.
  - `app/src/search/command_palette/warp_drive/data_source.rs` — palette data
    source for Drive items.
  - `app/src/search/command_palette/warp_drive/mod.rs` and the
    `*_search_item.rs` siblings for typed rows.
  - `app/src/integration_testing/warp_drive/mod.rs`,
    `app/src/integration_testing/warp_drive/assertion.rs` — existing test
    harness for Drive behavior.

Likely change shape:

1. New module `app/src/settings_view/warp_drive/selection.rs` (new module)
   holding the selection state, anchor logic, and reducer for click/keyboard
   events.
2. Wire selection into the row click handler and keyboard input on the Drive
   list view.
3. Add the selection bar component above the list with Delete / Move to… /
   Cancel.
4. Bulk delete / move plumb through existing per-item APIs; chunked client
   loop bounded to 200 items.
5. Persist selection across filter changes by keeping the selection set keyed
   on stable row IDs, not on visible position.

## Tests

- T1: Shift-click selects an inclusive contiguous range from anchor.
- T2: Cmd/Ctrl-click toggles a single row without affecting others.
- T3: Cmd-click then Shift-click compound — Shift extends from existing
  anchor, not from the most recent Cmd-click row.
- T4: `Cmd+A` with a filter selects only visible filtered rows.
- T5: `Esc` clears selection and dismisses the selection bar.
- T6: Selection bar appears at 2 selected, disappears at 1 and at 0.
- T7: Bulk delete with a partial-failure mock surfaces the failed items.
- T8: Bulk move into an existing folder; items already in destination are
  silent no-ops.
- T9: Selection persists after typing into the filter and after clearing it.
- T10: Keyboard nav full path — arrows, `Shift+Arrow`, `Space`, `Enter`,
  `Cmd+A`, `Esc`.

## Open Questions

- Should bulk delete for Plans support a chat-history-style 5-second in-app
  undo? Plans are more precious than chat messages. Recommendation: NO
  in-app undo by default; instead, record a 30-day server-side soft
  tombstone that admins can recover. Final decision deferred to engineering.
- Should mixed-type selections (workflow + notebook + Plan) all share the
  same bulk-delete confirm copy, or should each type get its own count
  breakdown in the modal? Recommendation: single combined count for V1;
  itemized breakdown if user testing surfaces confusion.

## Telemetry

Extend the existing Drive action event with two fields:

- `bulk: bool` — whether the action was a bulk action (≥2 items).
- `count: u32` — number of items affected.

No new event names. Same applies to `delete` and `move` action events.
