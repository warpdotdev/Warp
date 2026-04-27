# Cross-Window Tab Drag — Deferred Ghost Handoff v2

## Problem

The current implementation uses `InsertedInTarget`: the moment the cursor enters a target tab bar, the dragged tab's pane group (terminals, editors, etc.) is transferred into the target window's view tree. This causes two distinct performance problems:

**1. Sustained double-render cost.** Once the tab is in the target, the target window renders two sets of terminal content on every frame — its own tabs plus the transferred tab's terminal. Terminal rendering is expensive. This is why dragging is laggy even with the cursor held still in the center of the target.

**2. Boundary oscillation spikes.** When the cursor moves near the tab bar edge, it repeatedly crosses in and out. Each crossing triggers `execute_handoff` (transfer in) or `reverse_handoff` (transfer back), plus a `workspace:save_app` snapshot of both windows. These are expensive and cause additional frame drops near the edge.

The one-time transfer on a deliberate drop is acceptable. The problem is paying the transfer cost continuously during hover.

## Solution

Defer the view-tree transfer to drop time. During hover, show two cheap visual elements in the target window:

1. **Insertion slot** — an empty row/column space with `fg_overlay_1` background at the predicted drop position. Matches exactly what same-window drag shows for the dragged tab's origin slot.
2. **Floating chip** — a small tab-shaped card (icon + title) rendered as a window-level overlay, center-anchored to the cursor. Follows the cursor smoothly on every drag event.

Zero view-tree operations during hover. The real `transfer_view_tree_to_window` call happens only on drop.

## State machine

```
Floating ──(enters other window's tab bar)──► GhostInTarget ──(drop)──► perform_handoff ──► finalize
                    no view-tree transfer          │
                                                   └──(cursor leaves)──► Floating
```

The existing `InsertedInTarget` phase is kept for the back-to-caller path only (multi-tab drag hovering back over the source window's own tab bar), which is lower-frequency and requires a live transfer.

## Data model

### `DragPhase::GhostInTarget` (new variant)

```rust
GhostInTarget {
    target_window_id: WindowId,
    /// Where in the target's tab list the slot appears.
    target_insertion_index: usize,
    /// Cursor in target window coordinates. Updated on every drag event.
    ghost_cursor_in_target: Vector2F,
}
```

### `ActiveDrag` new field

```rust
ghost_tab_title: String,   // captured once when Floating → GhostInTarget
```

### New public type + accessor

```rust
pub struct GhostState {
    pub insertion_index: usize,
    pub cursor_in_window: Vector2F,
    pub title: String,
}

impl CrossWindowTabDrag {
    pub fn ghost_state_for_window(&self, window_id: WindowId) -> Option<GhostState>;
}
```

## Control flow

### Entering ghost (`on_drag_while_floating`)

When `cross_window_attach_target` finds a non-self target:

1. Capture `ghost_tab_title` from preview workspace's first tab.
2. Compute `ghost_cursor_in_target = drag_center_on_screen - target_window_bounds.origin()`.
3. Set `phase = GhostInTarget { target_window_id, target_insertion_index, ghost_cursor_in_target }`.
4. Call `show_window_and_focus_app(target_wid)` — preview window falls behind via z-order, no explicit hide.
5. Notify target to re-render (shows slot + chip).
6. Return `DragResult::Handled` (multi-tab) or `DragResult::AdjustDraggable` (single-tab). **No `HandoffNeeded`.**

### Cursor move in ghost (`on_drag_while_ghost`)

On every drag event while in `GhostInTarget`:

1. Reposition preview window to follow cursor (same as Floating), so it's in position if user moves off.
2. Hit-test `still_over_target`. If false: transition to `Floating`, restore preview to front, notify target to erase ghost.
3. If still over target: recompute `target_insertion_index` and `ghost_cursor_in_target`. If either changed, update phase and notify target.

### Drop while ghost (`on_drop`)

```rust
if let DragPhase::GhostInTarget { target_window_id, target_insertion_index, .. } = drag.phase {
    return DropResult::DropInto {
        target: AttachTarget { window_id: target_window_id, insertion_index: target_insertion_index },
    };
}
```

Workspace receives `DropResult::DropInto`, calls `perform_handoff` (real view-tree transfer), then `finalize`. Same path as before.

### `finalize` while ghost (failsafe)

If `finalize` is called while still in `GhostInTarget` (unusual; e.g. window closed mid-drag), notify target to erase ghost and treat as a floating drop (`finalize_preview_as_new_window` / `FocusSelf`).

## Rendering

### Floating chip — `Workspace::render()` in `view.rs`

Added to the root workspace `Stack` as a positioned overlay child:

```rust
if let Some(ghost) = CrossWindowTabDrag::as_ref(app).ghost_state_for_window(self.window_id) {
    stack.add_positioned_overlay_child(
        render_cross_window_ghost_chip(&ghost, appearance),
        OffsetPositioning::offset_from_parent(
            ghost.cursor_in_window,
            ParentOffsetBounds::Unbounded,
            ParentAnchor::TopLeft,
            ChildAnchor::Center,  // chip center tracks cursor
        ),
    );
}
```

`render_cross_window_ghost_chip`: terminal icon (24×24) + title text, `fg_overlay_1` background, rounded corners, max-width capped at 200px. Visually identical to the `Draggable` overlay that same-window drag paints at the cursor.

`ParentOffsetBounds::Unbounded` — chip can go anywhere in the window without being clipped.

### Insertion slot — horizontal tab bar (`view.rs`)

Before rendering tab `i` when `ghost.insertion_index == i`:

```rust
tab_bar.add_child(render_ghost_tab_slot(tab_width, theme));
```

`render_ghost_tab_slot`: `ConstrainedBox(Container(Empty).with_background(fg_overlay_1)).with_width(tab_width)`. Empty content, same width as a normal tab, same background as same-window drag's origin slot.

### Insertion slot — vertical tabs (`vertical_tabs.rs`)

Before rendering tab group `i` when `ghost.insertion_index == i`:

```rust
groups.add_child(render_ghost_vertical_tab_slot(row_height, theme));
```

`render_ghost_vertical_tab_slot`: `ConstrainedBox(Container(Empty).with_background(fg_overlay_1)).with_height(row_height)`. Height from last-frame position of `tab_position_id(0)`, fallback ~58px.

**Why empty content?** The floating chip already shows what's being dragged. The slot's only job is to show WHERE it will land — same as same-window drag.

**Why near-zero-height elements don't break index tracking:** The slot is the ONLY ghost element; it's a plain `ConstrainedBox` with no interactive children, so it doesn't add a `SavePosition` entry. `tab_insertion_index_for_cursor` compares cursor Y to saved tab positions (`tab_position_id(i)`). Because the slot has no saved position of its own, it doesn't shift the Y-coordinates that the index computation reads — the ghost slot position moves correctly as the cursor moves.

Actually wait — the slot DOES add height to the flex column, shifting all tab groups below it down by `row_height`. This means the saved positions from the previous frame (without the slot) are correct, but next-frame positions (with the slot) are shifted. To avoid confusion, the slot height is constrained to exactly the last-frame tab row height, meaning the cursor is always in a consistent position relative to the tab it's "between." This is a one-frame lag but imperceptible at 60fps.

## Preview window during GhostInTarget

**Multi-tab (dedicated preview window):** Preview falls behind target via z-order when `show_window_and_focus_app(target_wid)` is called. Preview is still repositioned to follow cursor each frame (so it's correctly placed if user moves off). No `hide_window` call.

**Single-tab (source IS preview):** Same — source window falls behind target via z-order, Draggable still receives drag events since macOS delivers them to the drag-owning window regardless of z-order.

On ghost clear (cursor leaves target): `show_window_and_focus_app(preview_wid)` restores preview to front.

## Performance

| Phase | Cost per drag event |
|---|---|
| Floating | Reposition preview window (O(1)) |
| **GhostInTarget (new)** | Update Vector2F + notify target (O(1)) |
| InsertedInTarget (existing) | Tab reorder in target (O(n tabs)) |
| Transitioning | Blocked |

No `transfer_view_tree_to_window` during hover. No `reverse_handoff`. No `workspace:save_app` snapshots between enter and drop.

## Files to change

- `app/src/workspace/cross_window_tab_drag.rs` — add `GhostInTarget` variant with cursor field, `ghost_tab_title` on `ActiveDrag`, `GhostState` struct, `ghost_state_for_window`, `on_drag_while_ghost`, update `on_drag_while_floating`, `on_drop`, `finalize`.
- `app/src/workspace/view.rs` — add `render_cross_window_ghost_chip`, `render_ghost_tab_slot`, insert slot into horizontal tab bar loop, add chip overlay to root `Stack` in `render()`.
- `app/src/workspace/view/vertical_tabs.rs` — add `render_ghost_vertical_tab_slot`, insert slot into groups loop.
