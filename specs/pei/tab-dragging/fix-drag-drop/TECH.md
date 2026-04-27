# Tab Dragging: Drag/Drop Bugs (horizontal and vertical)
## Problem statement
The feature under `pei/drag-tabs-out-of-windows` ships cross-window tab
drag for both the horizontal tab bar and the vertical tabs panel. Four
bugs were reported against that code:
1. **Can't drag a vertical tab out of the panel reliably.** Starting a
   drag on a vertical tab frequently does the wrong thing — either it
   detaches immediately on the first frame (so you can't reorder within
   the panel) or it never detaches even when dragging well outside the
   panel, depending on where the cursor lands. Horizontal tab dragging
   works.
2. **No drop zone on another window's vertical tabs panel.** Once a
   detached tab *is* in flight, placing it over another window's *vertical
   tabs panel* does nothing: no drop-zone indicator, no tab appears. The
   same gesture over the other window's *horizontal tab bar* already works
   — the tab physically appears in that bar as the user hovers.
3. **Same-window re-drop spawns an empty ghost window (vertical tabs).**
   Releasing the mouse while the cursor is back over the source window
   creates a brand-new window that is completely empty and transparent,
   instead of re-inserting the tab into the source window.
4. **Same-window re-drop spawns an empty ghost window (horizontal tabs,
   regression).** After the fix for bugs 1–3 landed, the exact
   re-drop-into-source-window gesture now fails in the *horizontal*
   layout: a put-back handoff fires mid-drag, immediately bounces back
   via `reverse_handoff`, and the subsequent drop falls into the
   "promote preview to new window" branch. Dev-log evidence of the
   reproduction:
   ```text path=null start=null
   on_drag_while_floating -> HandoffNeeded target_wid=0 caller_wid=0
   perform_handoff branch=target==caller (put-back)
   execute_handoff_back_to_caller -> InsertedInTarget target_wid=0 idx=0
   reverse_handoff caller_wid=0 target_wid=0 (Transitioning->Floating)
   ...
   on_drop phase=Floating
   on_drop drop-time re-resolve result=None
   finalize -> finalize_preview_as_new_window (CREATES NEW WINDOW)
   handle_drop_result RemoveSourceTab caller_wid=0 transferred_tab_index=1
   SQLite error 2067: UNIQUE constraint failed: terminal_panes.uuid
   ```
## Current design (expected happy path)
The `CrossWindowTabDrag` singleton (`app/src/workspace/cross_window_tab_drag.rs`)
drives a three-state machine: `Floating` → `Transitioning` → `InsertedInTarget`.
- `Workspace::on_tab_drag` (`app/src/workspace/view.rs`) runs on every tab
  drag event. If the drag has left the tab-bar area (or the source is a
  single-tab window), it kicks off a cross-window drag; otherwise it does
  a local reorder.
- While **Floating**, every `on_drag` call invokes `cross_window_attach_target`.
  If a target is found, the state machine returns `DragResult::HandoffNeeded`;
  `Workspace::perform_handoff` then moves the pane-group view tree into the
  target window and calls `insert_transferred_tab_at_index` on the target
  workspace. That insertion *is* the visual cue — the user sees a real tab
  materialize in the target.
- On **drop** from `InsertedInTarget`, `finalize_handoff` finishes the
  transfer and closes the preview window (multi-tab case).
- On **drop** from `Floating`, `finalize_preview_as_new_window` promotes
  the preview window to a permanent window and tells the source workspace
  to drop the detached tab.
Bug 1 is a failure of the detach-axis check in `on_tab_drag`. Bug 2 is a
failure of the detection step in `cross_window_attach_target`. Bug 3 is a
failure to re-resolve the attach target at drop time. Bug 4 is a
hit-test **hysteresis** between the entry check
(`cross_window_attach_target`, with a 12 px margin) and the stay check
(`on_drag_while_inserted::still_over_target_tab_bar`, with no margin),
compounded by a persistence race where `CrossWindowTabDrag::is_active()`
flips to `false` too early.
## Findings
### 1. Bug 1 root cause: detach-axis check uses the wrong rect
The original detach check in `Workspace::on_tab_drag` called
`tab_bar_rect_for_window`, which returned a single `Option<RectF>`:
```rust path=null start=null
pub(crate) fn tab_bar_rect_for_window(window_id: WindowId, app: &AppContext) -> Option<RectF> {
    app.element_position_by_id_at_last_frame(window_id, TAB_BAR_POSITION_ID)
        .or_else(|| {
            app.element_position_by_id_at_last_frame(window_id, VERTICAL_TABS_PANEL_POSITION_ID)
        })
}
```
It preferred the horizontal `TAB_BAR_POSITION_ID` and only fell back to
the vertical panel when the horizontal rect was missing. But
`Workspace::tab_bar_mode` keeps the horizontal tab bar rendered on top
even when the vertical tabs panel is open (default is
`ShowTabBar::Stacked`), so the horizontal rect is always present and the
vertical fallback is never taken.
The detach check inferred orientation from the returned rect's aspect
ratio (`is_vertical = rect.height() > rect.width()`) and applied a
perpendicular-axis check:
```rust path=null start=null
if is_vertical {
    drag_center.x() < rect.min_x() - DETACH_SENSITIVITY
        || drag_center.x() > rect.max_x() + DETACH_SENSITIVITY
} else {
    drag_center.y() < rect.min_y() - DETACH_SENSITIVITY
        || drag_center.y() > rect.max_y() + DETACH_SENSITIVITY
}
```
For vertical-tab drags: the horizontal rect is wide / short, so
`is_vertical = false` and the code does a Y-axis check against the
horizontal bar. The drag center starts somewhere inside the vertical
panel (Y well below the horizontal bar), so the very first drag event
trips the check and detach fires immediately — there's no way to reorder
within the panel, and conversely subtle-to-diagnose state corruption
follows when the detach-then-floating path misdetects where the cursor
actually is.
### 2. Bug 2 root cause: `cross_window_attach_target` hit-tested only one rect
The same `tab_bar_rect_for_window` preference problem affected cross-window
hit-testing. `cross_window_attach_target` asked for a single rect per
candidate window and checked `rect.contains_point(cursor)`:
- Cursor over the horizontal bar → hit → handoff fires → tab appears in
  the target (this is the visual cue users see for horizontal).
- Cursor over the vertical tabs panel → miss (the horizontal rect
  doesn't cover that area) → no handoff → nothing renders.
An earlier revision of this document mis-attributed this to a z-order /
margin issue; those hardening changes (multi-candidate iteration, 12 px
margin, skip-on-missing) are still shipped and still valuable, but the
root cause was hit-testing a single rect per window when both
presentations coexist.
### 3. Bug 3 root cause: dropping over the source while still `Floating`
When the cursor was over the source window's own tab bar but the
`Floating` handoff never fired, `on_drop` entered
`finalize_preview_as_new_window`, which unconditionally promoted the
preview window to permanent. Nothing in `on_drop` re-ran
`cross_window_attach_target` to check for a last-moment same-window drop.
### 3b. Bug 4 root cause: hit-test hysteresis between entry and stay checks
`cross_window_attach_target` in
`app/src/workspace/cross_window_tab_drag.rs` tests each candidate tab
bar with `expanded_rect(tab_bar_on_screen, TAB_BAR_HIT_MARGIN)`
(`TAB_BAR_HIT_MARGIN = 12.0`). This lets the user trigger a handoff a
few pixels past the tab bar edge — good.
But the corresponding stay-check in
`on_drag_while_inserted::still_over_target_tab_bar` used the
**unexpanded** rect: `tab_bar_on_screen.contains_point(…)`. Any cursor
position sitting in the 12 px margin between the two thresholds
satisfies the entry check on one frame, fails the stay check on the
very next frame, and triggers `reverse_handoff`. The dragged tab
bounces back into the preview window.
From the log (tab-bar Y range is `[0, 34]`, `initial_drag_center_offset.y
≈ 17`):
- Handoff fires at `tab_position.y = 29.48` → tab-center y ≈ 46.48,
  inside `[0 − 12, 34 + 12] = [-12, 46]` → **entry OK**.
- Next frame at `tab_position.y = 25.53` → tab-center y ≈ 42.53, outside
  `[0, 34]` → **stay NOT OK** → `reverse_handoff` fires.
The put-back handoff is undone one frame after it starts. The drag
enters `Floating` again. The user then releases the mouse, `on_drop`
tries the drop-time re-resolution, but by that time the cached
`last_drag_center_on_screen` reflects a `source_window_origin` that has
drifted (the preview window's origin chases the cursor, and
`caller_window_id` for the last frame can be the preview window whose
bounds are being rewritten each tick). In the captured repro the
resolved screen cursor was `(645, -1011)` — nowhere near any tab bar —
so re-resolution returned `None` and the "promote preview to new window"
branch ran.
### 3c. Bug 4 root cause: persistence race on finalize
Even after the hysteresis trigger was kept out, a second race produced
the SQLite UNIQUE constraint fire-storm:
1. `CrossWindowTabDrag::finalize()` was called from `on_drop`.
2. `finalize` did `self.active_drag.take()` on its first line, so
   `CrossWindowTabDrag::is_active()` flipped to `false` immediately.
3. `finalize_preview_as_new_window` promoted the preview window to
   permanent (clearing `is_tab_drag_preview` on the preview workspace)
   and returned `DropResult::RemoveSourceTab { transferred_tab_index }`.
4. Back in the `DropTab` workspace action, `handle_drop_result` was
   called — but between steps 2 and this step, `workspace:save_app`
   actions fired twice. `get_app_state` in `app/src/app_state.rs` was
   already protected by `CrossWindowTabDrag::is_active()`, but that
   now returned `false`, so both workspaces were serialized:
   - the source workspace still holding the detached tab at index 1
     (the pane group had already moved), and
   - the preview-turned-permanent workspace now holding the same pane
     group.
   Both tried to insert the same `terminal_panes.uuid` and SQLite
   rejected it with error 2067 on every subsequent save until the user
   did something that dirtied state on one of the two sides.
### 3d. Bug 4 additional root cause: stale `source_tab_index` after put-back+reverse
The third layer of Bug 4 is that after a put-back handoff followed by a
reverse_handoff, the `ActiveDrag`'s cached `source_tab_index` is
semantically stale. The full sequence in the captured repro:
1. `begin_multi_tab_drag` marks source tab at index 1 as detached and
   moves its pane group to the preview window. `source_tab_index=1`.
2. `HandoffNeeded` fires with `target == caller` (put-back). In
   `Workspace::perform_handoff`:
   - `remove_tab_without_undo(source_tab_index=1)` removes the detached
     placeholder from the source workspace (source now has `N-1` tabs).
   - `execute_handoff_back_to_caller` moves the pane group back from
     preview into source.
   - `insert_transferred_tab_at_index(transferred_tab, insertion_index=0)`
     re-inserts the tab into source at index 0 (source now has `N` tabs
     again, but at different positions).
3. The cursor then moves outside the tab bar, `reverse_handoff` fires
   in `CrossWindowTabDrag::on_drag_while_inserted`:
   - `target_workspace.remove_tab_without_undo(target_insertion_index=0)`
     removes the put-back tab from source (source now has `N-1` tabs
     and no reference to the dragged pane group).
   - The pane group is moved back into the preview window.
4. User drops in empty space. Phase is `Floating`.
   `finalize_preview_as_new_window` runs and returns
   `DropResult::RemoveSourceTab { transferred_tab_index: 1 }`
   — using the original, now-stale `source_tab_index=1`.
5. `handle_drop_result::RemoveSourceTab(1)` calls
   `self.remove_tab_without_undo(1, ctx)` on the source workspace. But
   source no longer has the tab at index 1 (it was removed in step 3,
   then re-added at 0, then removed again). Depending on how many tabs
   remain, this either panics on `debug_assert` (len ≤ index) or
   silently removes the wrong tab, leaving the source window empty.
This is why the user reports “the old window I dragged from becomes
empty” after releasing in empty space following a put-back→reverse
sequence.
### 4. Why the resulting window looks empty and transparent
The preview window was created with `WindowStyle::PositionedNoFocus` and
`is_tab_drag_preview = true`. While the flag is set,
`Workspace::tab_bar_mode` forces `ShowTabBar::Stacked` and `get_app_state`
skips preview workspaces entirely when serializing app state.
`finalize_preview_as_new_window` cleared `is_tab_drag_preview` but two
problems layered on top:
- **SQLite UNIQUE-constraint storm on drop.** Logs showed
  `UNIQUE constraint failed: terminal_panes.uuid` firing 6+ times
  immediately after `DropTab`. The detached tab still sat in the source
  workspace's `self.tabs` (marked `detached = true`) holding a
  `ViewHandle<PaneGroup>` to the pane-group now living in the preview;
  once `set_is_tab_drag_preview(false)` flipped, both workspaces tried to
  serialize the same terminal pane UUID before `handle_drop_result` /
  `remove_tab_without_undo` had a chance to remove the source tab.
- **Preview window style never re-chromes.** Flipping the boolean on the
  workspace did not re-issue a `WindowStyle::Normal` equivalent at the
  platform layer; the native NSWindow kept its non-key, non-standard
  behavior.
## Implementation
### 1. `tab_bar_rects_for_window` helper (replaces `tab_bar_rect_for_window`)
New helper in `app/src/workspace/view.rs` returns a `Vec<RectF>`
containing whichever of the horizontal `TAB_BAR_POSITION_ID` and vertical
`VERTICAL_TABS_PANEL_POSITION_ID` rects are currently laid out (0, 1, or
2 rects):
```rust path=null start=null
pub(crate) fn tab_bar_rects_for_window(window_id: WindowId, app: &AppContext) -> Vec<RectF> {
    let mut rects = Vec::with_capacity(2);
    if let Some(rect) = app.element_position_by_id_at_last_frame(window_id, TAB_BAR_POSITION_ID) {
        rects.push(rect);
    }
    if let Some(rect) =
        app.element_position_by_id_at_last_frame(window_id, VERTICAL_TABS_PANEL_POSITION_ID)
    {
        rects.push(rect);
    }
    rects
}
```
The old `tab_bar_rect_for_window` was removed — every call site now needs
both rects, so the single-rect helper became dead code.
### 2. Detach-axis check fix in `on_tab_drag` (the real Bug 1 fix)
`Workspace::on_tab_drag` (`app/src/workspace/view.rs`) now treats the
drag as "attached" if *any* tab-bar presentation says the drag is within
its perpendicular axis. Drag is "outside" only when every rendered
presentation says it's outside its own perpendicular axis:
```rust path=null start=null
let rects = tab_bar_rects_for_window(ctx.window_id(), ctx);
let is_drag_outside_tab_bar = if rects.is_empty() {
    // Fallback: hardcoded horizontal bar check when no rect has been laid out.
    let drag_y = position.min_y();
    !(-DETACH_SENSITIVITY..=TAB_BAR_HEIGHT + DETACH_SENSITIVITY).contains(&drag_y)
} else {
    rects.into_iter().all(|rect| {
        let is_vertical = rect.height() > rect.width();
        if is_vertical {
            drag_center.x() < rect.min_x() - DETACH_SENSITIVITY
                || drag_center.x() > rect.max_x() + DETACH_SENSITIVITY
        } else {
            drag_center.y() < rect.min_y() - DETACH_SENSITIVITY
                || drag_center.y() > rect.max_y() + DETACH_SENSITIVITY
        }
    })
};
```
Behavior:
- Vertical reorder (drag up/down inside the panel): horizontal rect says
  Y is outside; vertical rect says X is within the panel X range → at
  least one rect says "inside" → no detach.
- Vertical detach (drag horizontally out of the panel): horizontal rect
  says Y is outside; vertical rect says X has left the panel X range →
  both rects say "outside" → detach fires.
- Horizontal reorder (drag left/right within the bar): horizontal rect
  says Y is within → at least one rect says "inside" → no detach.
- Horizontal detach (drag down out of the bar, window with no vertical
  panel): horizontal rect says Y is outside; no vertical rect → `all()`
  is true → detach fires.
### 3. Cross-window detection uses both rects (the real Bug 2 fix)
`cross_window_attach_target` and the `still_over_target_tab_bar` branch
in `on_drag_while_inserted` (both in
`app/src/workspace/cross_window_tab_drag.rs`) now iterate
`tab_bar_rects_for_window` and treat the cursor as over the tab bar if
*any* rect contains it (with the `TAB_BAR_HIT_MARGIN` expansion). This
makes cross-window drop work for vertical tabs: hovering over the
vertical panel now registers a hit, fires the handoff, and the inserted
tab appears in the vertical panel just like the horizontal case.
### 4. Hardened `cross_window_attach_target` (supporting change)
Also in `cross_window_tab_drag.rs`:
- Added `TAB_BAR_HIT_MARGIN = 12.0` (px) and `expanded_rect` helper so
  small cursor overshoot still counts.
- Iterates *all* z-behind windows and returns the first whose expanded
  tab-bar rect contains the cursor, rather than stopping at the first
  whose window bounds contain the cursor.
- Windows whose tab-bar rects haven't been laid out yet are skipped
  rather than returning None, so subsequent z-behind windows still get
  a chance to match.
### 5. Drop-time re-resolution (the real Bug 3 fix)
New `DropResult::DropInto { target }` variant.
`CrossWindowTabDrag::on_drop` now re-runs `cross_window_attach_target`
one more time with the last cached cursor position (gated by a
`drop_resolution_attempted` flag on `ActiveDrag` so it only happens
once). If the re-resolution succeeds, it returns
`DropResult::DropInto { target }` and leaves `active_drag` in place so
the caller can run `perform_handoff`. Otherwise it delegates to a new
`finalize` method (the original terminal-state code, extracted so the
caller can call it after the re-resolution handoff completes).
`ActiveDrag` gained `last_drag_center_on_screen`,
`last_caller_window_id`, and `drop_resolution_attempted` fields; the
first two are updated every frame in `on_drag`.
`Workspace::handle_drop_result` handles `DropResult::DropInto` by
calling `perform_handoff(target, ctx)` then `drag.finalize(ctx)` then
recursively re-invoking `handle_drop_result` on the returned variant.
### 5b. Hit-test hysteresis fix (the Bug 4 bouncing fix)
`on_drag_while_inserted::still_over_target_tab_bar` in
`app/src/workspace/cross_window_tab_drag.rs` now expands each tab-bar
rect by the same `TAB_BAR_HIT_MARGIN` used by
`cross_window_attach_target`:
```rust path=null start=null
expanded_rect(tab_bar_on_screen, TAB_BAR_HIT_MARGIN)
    .contains_point(drag_center_on_screen)
```
With this change, the entry and stay thresholds are the exact same
geometry, so a cursor that triggers a handoff on frame N cannot
spuriously trigger a `reverse_handoff` on frame N+1 just by sitting
still in the margin.
### 6. Persistence race prevention (extended for Bug 4)
`app/src/app_state.rs::get_app_state` already skipped per-window
serialization while `CrossWindowTabDrag::is_active()` returned `true`.
The Bug 4 repro showed that this was not enough: `finalize` called
`active_drag.take()` on its first line, so `is_active()` flipped to
`false` *before* the workspace view had a chance to run
`handle_drop_result` (which is where the source tab is actually
removed). Save actions fired during `handle_drop_result` saw
`is_active() = false`, tried to serialize both the source workspace
(with a detached tab still in `self.tabs`) and the promoted preview
workspace (now holding the same pane group), and collided on
`terminal_panes.uuid`.
The fix is a new `pending_source_cleanup: bool` on
`CrossWindowTabDrag`:
- `finalize` sets `pending_source_cleanup = true` right after
  `active_drag.take()`. `is_active()` now returns
  `active_drag.is_some() || pending_source_cleanup`.
- `Workspace`'s `DropTab` action handler calls
  `CrossWindowTabDrag::finish_source_cleanup()` **after**
  `self.handle_drop_result(...)` returns, flipping the flag back to
  `false`.
Because `handle_drop_result` is synchronous (the only async-ish branch
is `DropResult::DropInto`, which re-enters `finalize` and
`handle_drop_result` recursively within the same action dispatch),
this keeps `is_active()` continuously `true` from `begin_*_drag` all
the way through source-tab removal, closing the window between
detach and cleanup where saves could slip in.
### 6b. Stale source-tab-index prevention (Bug 4 §3d fix)
`ActiveDrag` gained a new `source_placeholder_consumed: bool` field
(default `false`).
- `Workspace::perform_handoff`'s put-back branch (where
  `target.window_id == caller_window_id`) calls a new
  `CrossWindowTabDrag::mark_source_placeholder_consumed()` method
  right after `remove_tab_without_undo(source_tab_index)`. That sets
  `source_placeholder_consumed = true`.
- `finalize_preview_as_new_window` now checks the flag and returns
  `DropResult::NoOp` instead of `RemoveSourceTab` / `CloseSourceWindow`
  if the source placeholder was already consumed. The promoted
  preview window is still the only state change; the source workspace
  is left untouched (it has no reference to the dragged tab after the
  put-back+reverse sequence).
This prevents the caller from running `remove_tab_without_undo` on a
stale index, which was both tripping `debug_assert` in some paths and
leaving the source window empty in others.
### 7. Promoted-preview re-chroming
`finalize_preview_as_new_window` now calls
`sync_window_button_visibility(ctx)` and `update_titlebar_height(ctx)`
on the promoted workspace after clearing `is_tab_drag_preview`, so the
native titlebar / traffic-lights come back and `tab_bar_mode`
re-evaluates. No changes to `app/src/root_view.rs`.
### 8. Dead `is_drag_preview_workspace` field removed
Deleted the unused second preview flag from `Workspace` and its
initialization site; only `is_tab_drag_preview` remains.
## Known remaining gap
The drop-time re-resolution in `on_drop` depends on
`last_drag_center_on_screen`, which is computed as
`window_bounds(caller_window_id).origin() + drag_center_in_window` each
frame in `on_drag`. In the Bug 4 repro, `caller_window_id` on the last
few frames is the preview window, whose bounds are continuously
rewritten by `set_and_cache_window_bounds` inside
`on_drag_while_floating`. The cached cursor can therefore drift far
from where the user actually released the mouse, which is why the
drop-time re-resolution returns `None` in some cases. The hit-test
hysteresis fix (§5b) closes the primary bounce that exposes this drift
— but if a user manages to land in `Floating` with the cursor truly
outside any tab bar, the empty-new-window behavior is still possible
and is the expected outcome. If we ever want the drop-time
re-resolution to be reliable under preview-chasing, the fix is to cache
the raw screen-space cursor directly from the `MouseDragEvent` path
instead of reconstructing it from `caller_window_origin +
drag_in_window` each frame. That is out of scope for this change.
## Files touched
- `app/src/workspace/view.rs` — new `tab_bar_rects_for_window` helper,
  removal of `tab_bar_rect_for_window`, detach-axis check rewritten in
  `on_tab_drag`, removal of `is_drag_preview_workspace`,
  `DropResult::DropInto` handling in `handle_drop_result`,
  `CrossWindowTabDrag::finish_source_cleanup()` call after
  `handle_drop_result` in the `DropTab` action handler.
- `app/src/workspace/cross_window_tab_drag.rs` — `tab_bar_rects_for_window`
  wiring in `cross_window_attach_target` / `on_drag_while_inserted`,
  `TAB_BAR_HIT_MARGIN` / `expanded_rect`, multi-candidate iteration,
  `DropResult::DropInto`, split of `on_drop` / `finalize`, new
  `ActiveDrag` cursor/resolution fields, re-chrome calls in
  `finalize_preview_as_new_window`, new `pending_source_cleanup` field
  + `finish_source_cleanup()` method, hysteresis fix in
  `still_over_target_tab_bar` to use `expanded_rect(_, TAB_BAR_HIT_MARGIN)`,
  new `ActiveDrag::source_placeholder_consumed` field +
  `mark_source_placeholder_consumed()` method, and a
  `DropResult::NoOp` early-return in `finalize_preview_as_new_window`
  when the flag is set.
- `app/src/app_state.rs` — `CrossWindowTabDrag::is_active()` early-out
  in `get_app_state` (now also covers the
  `pending_source_cleanup = true` post-finalize window).
