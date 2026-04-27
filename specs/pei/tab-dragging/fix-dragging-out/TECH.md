# Tab Dragging: Horizontal vs Vertical Audit
## Problem
Dragging a terminal tab out of a window into a new window works reasonably well when the tab lives in the horizontal tab bar, but fails or behaves incorrectly when the tab lives in the vertical tabs panel (`TabSettings::use_vertical_tabs`). The two presentations dispatch the same `StartTabDrag` / `DragTab` / `DropTab` actions, but the workspace-level drag implementation and the `CrossWindowTabDrag` singleton were written with only the horizontal tab bar in mind. Several code paths bake in horizontal-layout assumptions (drag axis, detach threshold, preview-window positioning, attach-target detection, insertion math, target hit-testing) that do not apply to vertical tabs.

## Scope
Shared action surface — both presentations dispatch the same `WorkspaceAction`s:

- Horizontal: `app/src/tab.rs (1669-1683)` — `Draggable::new(...).on_drag_start(StartTabDrag).on_drag(DragTab{tab_index, tab_position}).on_drop(DropTab)`.
- Vertical: `app/src/workspace/view/vertical_tabs.rs (2027-2041)` — same action trio.

Shared downstream code — both enter the same entry points:

- `Workspace::on_tab_drag` (`app/src/workspace/view.rs:22924`)
- `CrossWindowTabDrag::on_drag` / `on_drop` (`app/src/workspace/cross_window_tab_drag.rs`)
- `Workspace::perform_handoff` (`app/src/workspace/view.rs:22823`)
- `Workspace::handle_drop_result` (`app/src/workspace/view.rs:23079`)

The audit below enumerates the places where that shared code silently assumes a horizontal tab bar.

## Findings
### 1. Drag axis is hard-locked to `VerticalOnly` for vertical tabs
Horizontal tab, `app/src/tab.rs (1678-1683)`:

```rust path=null start=null
let draggable = if FeatureFlag::DragTabsToWindows.is_enabled() {
    draggable
} else {
    draggable.with_drag_axis(DragAxis::HorizontalOnly)
};
```

When the cross-window flag is on, the horizontal tab can be dragged in any direction — which is what lets the user pull the tab *down* out of the tab bar to trigger detach.

Vertical tab, `app/src/workspace/view/vertical_tabs.rs:2040`:

```rust path=null start=null
.with_drag_axis(DragAxis::VerticalOnly)
.finish();
```

This is unconditional. Regardless of `FeatureFlag::DragTabsToWindows`, the vertical-tab `Draggable` only emits drags along the Y axis. The drag rectangle reported to `on_drag` never moves in X, so the user cannot express "pull this tab out of the panel" in cursor geometry. Reordering inside the panel still works (it is Y-driven), but cross-window detach is effectively unreachable from the vertical-tabs UI.

This is the single biggest blocker. Even if every other issue below were fixed, the drag events produced by the vertical-tab `Draggable` would still never satisfy the detach heuristic.

### 2. Detach threshold uses Y against the horizontal tab-bar height
`Workspace::on_tab_drag` (`app/src/workspace/view.rs:22924-22964`):

```rust path=null start=null
const DETACH_SENSITIVITY: f32 = 10.0;
let tab_bar_top = 0.0;
let tab_bar_bottom = TAB_BAR_HEIGHT;     // 34.0
let drag_y = position.min_y();
let is_drag_outside_tab_bar = drag_y < (tab_bar_top - DETACH_SENSITIVITY)
    || drag_y > (tab_bar_bottom + DETACH_SENSITIVITY);
...
if (is_drag_outside_tab_bar || source_is_single_tab)
    && FeatureFlag::DragTabsToWindows.is_enabled()
{
    // begin cross-window drag
}
```

`TAB_BAR_HEIGHT` is the horizontal bar's height (`view.rs:524`, `34.0`). The detach check therefore asks "has the cursor moved above the top or below the bottom of the horizontal tab bar?"

For vertical tabs the tab row spans the full height of the window (inside the vertical panel, panel width ≈ 248 px), so:

- `drag_y` is always well below `tab_bar_bottom = 34` as soon as any tab that is not the first is being dragged, causing *every* vertical-tabs drag to be classified as "outside tab bar" — including ordinary local reorders.
- The correct detach axis for a vertical panel is X (cursor leaving the panel on the right for left-docked tabs, or on the left for right-docked tabs), not Y.

In practice the `DragAxis::VerticalOnly` lock in §1 masks this bug by preventing any drags from reaching `on_tab_drag` with meaningful X changes. Fixing §1 without fixing this threshold will cause every vertical-tab drag to be treated as a detach attempt from the first pixel.

### 3. Preview-window positioning anchors to `tab_position_id(0)`
`Workspace::on_tab_drag` (`view.rs:22982-23036`):

```rust path=null start=null
let last_known_target_tab_origin_in_window = ctx
    .element_position_by_id(tab_position_id(0))
    .map(|rect| vec2f(rect.min_x(), rect.min_y()))
    .unwrap_or_else(|| vec2f(0.0, 0.0));
let window_position = drag_origin_on_screen - last_known_target_tab_origin_in_window;
```

The intent is Chrome-style: "compute the preview window's screen origin so that the first tab of the preview window lines up exactly where the source tab currently is under the cursor." That works when both source and preview render the tab at roughly the same spot in window-local coordinates — which is true for horizontal tabs (both windows show their first tab near `(left-padding, 0)` in the tab bar).

For vertical tabs the source's `tab_position_id(0)` rect is *inside the sidebar*, roughly at `x ≈ GROUP_HORIZONTAL_PADDING = 8`, `y ≈ control-bar height + padding`. Subtracting that from `drag_origin_on_screen` places the new preview window's origin a nearly full sidebar width to the left of the cursor. The user sees the detached window appear well away from the mouse.

Both `on_tab_drag` at detach time and `CrossWindowTabDrag::on_drag_while_floating` / `on_drag_while_inserted` (cross_window_tab_drag.rs:491-500, 534-542) use the same `tab_position_id(0)` anchor, so the misalignment also affects the floating motion, not just initial placement.

### 4. Attach-target detection relies on `TAB_BAR_POSITION_ID`
`cross_window_attach_target` and `on_drag_while_inserted` locate candidate drop zones by:

```rust path=null start=null
ctx.element_position_by_id_at_last_frame(window_id, TAB_BAR_POSITION_ID)
```

`TAB_BAR_POSITION_ID = "workspace_view:tab_bar"` (`view.rs:555`) is only applied to the horizontal bar by `Workspace::render_tab_bar` (`view.rs:17218-17229`).

When a workspace has `TabSettings::use_vertical_tabs` set, the horizontal bar is skipped entirely — `view.rs:21537` only renders the stacked bar when `tab_bar_mode == ShowTabBar::Stacked`, and the config-panel path renders the vertical panel with a *different* position id, `VERTICAL_TABS_PANEL_POSITION_ID = "workspace_view:vertical_tabs_panel"` (`view.rs:18593-18597`).

Consequences:

- `cross_window_attach_target` returns `None` for any candidate window that is using vertical tabs — you cannot drop a tab into a vertical-tabs window.
- `on_drag_while_inserted`'s "is the cursor still over the target tab bar?" check (`cross_window_tab_drag.rs:413-425`) also never matches, so a tab that somehow reached `InsertedInTarget` in a vertical-tabs window would immediately trigger `reverse_handoff` on the next drag event.

This is not a layout offset bug: it is a complete miss. No amount of cursor movement lets the singleton see the vertical-tabs window as a target.

### 5. Insertion-index computation is X-only
`Workspace::tab_insertion_index_for_cursor` (`view.rs:22729-22767`):

```rust path=null start=null
let cursor_x_in_window = cursor_position_on_screen.x() - window_bounds.min_x();
...
for (index, tab_position) in &visible_tabs {
    if cursor_x_in_window < tab_position.center().x() {
        return *index;
    }
}
```

The method is documented as ignoring Y "once we know the cursor is over the tab bar." That is correct for horizontal bars but wrong for vertical panels, where tab rects are stacked along Y and the X coordinates of all rows are essentially identical.

For a vertical-tabs target:

- The first comparison `cursor_x_in_window < tab_position.center().x()` returns true almost immediately because the cursor is inside the panel at a smaller X than the tab-row center, so insertion index is always `0`.
- The "last index + 1" fallback would be a symmetric mistake in the opposite direction.

`on_drag_while_inserted` (`cross_window_tab_drag.rs:428-462`) feeds directly into this method, so cross-window reordering inside a vertical-tab target would be incorrect as well.

The local reorder path at least detects the axis explicitly: `on_tab_drag` branches on `TabSettings::use_vertical_tabs` to choose between `calculate_updated_tab_index` and `calculate_updated_tab_index_vertical` (`view.rs:23052-23058`). The cross-window path lacks the equivalent branch.

### 6. `calculate_updated_tab_index_vertical` exists but is dead for cross-window
`calculate_updated_tab_index_vertical` (`view.rs:23161-23194`) correctly uses Y midpoints to do adjacent swaps inside a vertical-tabs window. It is only reached on the *local-reorder* branch, after the cross-window branch has already bailed out. So vertical-tab reorder inside a single window works; vertical-tab reorder after a cross-window attach does not, because `tab_insertion_index_for_cursor` is used there instead.

Additionally, the vertical-tabs UI already emits richer drop metadata (`VerticalTabsPaneDropTargetData`, `TabBarLocation::TabIndex`, `TabBarHoverIndex::BeforeTab(n)` — see `vertical_tabs.rs:1113-1121, 2055-2063`), but `on_tab_drag` never reads it. A future implementation can probably reuse this `DropTarget` metadata instead of writing a second "cursor → insertion index" calculator.

### 7. Source-window detach/close handling
When a multi-tab source window loses the dragged tab, `on_tab_drag` switches to an adjacent tab (`view.rs:23039-23046`). This is purely index arithmetic and is not horizontal-specific, so it works. However, the preview window's *layout* is whatever the new window decides (based on global `TabSettings`):

- If the source was using vertical tabs, the live `PaneGroup` is transferred into a new window that will render with vertical tabs too (because `use_vertical_tabs` is a per-user setting, not per-window).
- The preview window's `tab_position_id(0)` will be inside a new sidebar. The first-frame window reposition in `on_drag_while_floating` (`cross_window_tab_drag.rs:534-554`) then re-corrects the origin using that new, vertical-tabs origin — so a "first frame" mis-position becomes immediately visible and corrects only on the next drag tick.
- If the user drops before that correction, they get a preview window whose tab row is offset relative to the cursor.

### 8. Single-tab source path assumes horizontal presentation
`begin_single_tab_drag` (`view.rs:22992-23009`) repositions the *existing* window to the computed `window_position` and suppresses the draggable overlay. That logic does not consult `TabSettings`. For a single-tab vertical-tabs window the drag axis is already `VerticalOnly` (§1), so this path never executes from the vertical-tabs UI, but if it did, the same `tab_position_id(0)` offset bug (§3) would apply to the single-tab case as well.

## Problems and user-visible symptoms
Mapping the findings to what a user would actually observe today when the source window has vertical tabs enabled and `FeatureFlag::DragTabsToWindows` is on:

- **The drag never detaches.** §1 prevents the cursor from producing X movement, and §2 would misfire on the first Y pixel anyway. The drag stays a local reorder.
- **If §1 were lifted, every drag would immediately detach.** §2's Y-threshold is tripped by the first row below the top of the panel.
- **Detached preview windows appear at the wrong position.** §3: the new window is shifted left by nearly the panel width, so the cursor is no longer inside the dragged tab.
- **Vertical-tabs windows cannot be drop targets.** §4: `cross_window_attach_target` never recognizes them.
- **Reordering inside a vertical-tabs drop target is broken.** §5, §6: insertion index is always 0 or tail because only X is compared.
- **No reverse-handoff out of a vertical-tabs target.** §4's hit test also fails, so the singleton would keep thinking the cursor is "not over the tab bar" and immediately reverse any handoff that did occur.

## Suggested directions for a fix (not exhaustive)
The audit surfaces a shared theme: the cross-window drag code currently treats "the tab bar" as a single 1-D horizontal strip. Supporting vertical tabs requires generalizing that assumption. Candidate directions, ordered from narrow to broad:

1. **Gate the drag axis on presentation, not just the feature flag.** Vertical tabs should follow the same rule as horizontal tabs: when `FeatureFlag::DragTabsToWindows` is on, drop the axis lock entirely; when off, keep the local-only lock (`VerticalOnly` for vertical tabs, `HorizontalOnly` for horizontal).
2. **Parameterize the detach threshold by layout.** Introduce an `Orientation::Horizontal | Vertical` enum (or read `TabSettings::use_vertical_tabs`) in `on_tab_drag` so the detach check tests the appropriate axis and bounds (panel width vs. tab-bar height).
3. **Give the vertical tabs panel its own `TAB_BAR_POSITION_ID` equivalent, or make the lookup orientation-aware.** A natural refactor is to expose a `Workspace::tab_bar_hit_rect(window_id, ctx)` method that returns whichever of the two rects is actually rendered, and have `cross_window_attach_target` / `on_drag_while_inserted` consume that.
4. **Reuse `tab_position_id(n)` rects for insertion math in both orientations.** `tab_insertion_index_for_cursor` should detect whether the rects are laid out along X or Y (e.g., by comparing the spread of centers) or take an explicit orientation parameter, and compare against the dominant axis. `calculate_updated_tab_index_vertical` is an existing template.
5. **Use the vertical-tabs `DropTarget` metadata instead of re-deriving insertion index.** `VerticalTabsPaneDropTargetData::tab_hover_index` already expresses `BeforeTab(n)` / `OverTab(n)`. Letting the `CrossWindowTabDrag` state machine consume those events (or the hovered index stored on the workspace) avoids writing a second geometry-based calculator for vertical mode.
6. **Anchor preview-window origin to the dragged tab's own rect, not `tab_position_id(0)`.** The current code approximates "where the first tab lives" and assumes that matches the dragged tab's visual position. Using the dragged tab's actual on-screen origin (already available via the `position: RectF` passed to `on_tab_drag`) removes the implicit "tab 0 is at the top-left of the tab bar" assumption that breaks for vertical panels.

Items 1–4 are the minimum needed for a functional vertical-tabs drag. Item 5 is an architectural simplification. Item 6 applies to both orientations and may also remove subtle positioning drift in horizontal mode.

## Files to touch (likely)
- `app/src/workspace/view/vertical_tabs.rs` — drag-axis gating on the vertical `Draggable`.
- `app/src/workspace/view.rs` — `on_tab_drag` detach threshold, preview-window anchor, `tab_insertion_index_for_cursor`, orientation-aware `tab_bar` rect lookup, and a `SavePosition` on the vertical panel that the singleton can resolve.
- `app/src/workspace/cross_window_tab_drag.rs` — `cross_window_attach_target`, `on_drag_while_floating`, `on_drag_while_inserted` to consume an orientation-aware tab-bar rect and insertion-index helper.
- `app/src/tab.rs` — only if the horizontal path also adopts the dragged-tab-origin preview anchor (item 6 above).
