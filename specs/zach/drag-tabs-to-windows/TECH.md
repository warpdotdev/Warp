# Drag Tabs to Windows (Tech Spec)

## 1. Problem

Dragging tabs to new or existing windows is not just an extension of local tab reordering. The feature crosses three boundaries that the old in-window reorder path does not:

1. it moves a live tab, including its real `PaneGroup` view tree and terminal state, between different windows
2. it creates, hides, shows, reuses, and closes native windows while the drag is still in progress
3. it must keep one continuous drag gesture alive across reorder, detach, attach, reverse-attach, and drop without re-entrant view mutations corrupting state

The implementation therefore needs to coordinate source workspace state, target workspace state, preview-window lifecycle, native z-order and focus, and cleanup of transferred views. The goal is to preserve the product behavior in `specs/zach/drag-tabs-to-windows/PRODUCT.md` without introducing duplicate tabs, blank windows, stale subscriptions, or destructive close behavior.

## 2. Relevant code

- `specs/zach/drag-tabs-to-windows/PRODUCT.md` — product behavior the implementation must satisfy
- `app/src/workspace/mod.rs:1-78` — registers `WorkspaceRegistry` and `CrossWindowTabDrag`
- `app/src/workspace/registry.rs:1-66` — window → workspace lookup used across source, preview, and target windows
- `app/src/workspace/cross_window_tab_drag.rs (1-1109)` — singleton drag state machine, attach targeting, handoff, reverse-handoff, and drop finalization
- `app/src/workspace/view.rs (751-950)` — `TransferredTab` and workspace fields that support preview-window and transfer lifecycle handling
- `app/src/workspace/view.rs (14818-15017)` — tab-bar rendering changes that hide transferred tabs and make drag state UI-agnostic
- `app/src/workspace/view.rs (17618-17817)` — `DropTab` handling and handoff/drop integration with the singleton
- `app/src/workspace/view.rs (19630-20240)` — `on_tab_drag`, `perform_handoff`, transfer helpers, local cleanup, and insertion-index logic
- `app/src/workspace/view/vertical_tabs.rs (721-920)` — vertical-tabs UI dispatching the same drag actions as the top tab bar
- `app/src/root_view.rs (491-690)` — `create_transferred_window`, which creates preview windows and adopts transferred pane groups
- `app/src/tab.rs (1340-1605)` — top tab-bar `Draggable` wiring and tooltip/hover suppression while dragging
- `app/src/app_state.rs (236-359)` — snapshot logic that skips preview windows
- `ui/src/core/app.rs (661-860)` — `AppContext` state for view/window ownership and focus suppression
- `ui/src/core/app.rs (2908-3107)` — `transfer_view_to_window`, `transfer_view_tree_to_window`, and `transfer_structural_children`
- `ui/src/core/app.rs (3693-3892)` — focus suppression and view-focus behavior used during preview-window creation/finalization
- `ui/src/platform/mod.rs (521-720)` — `WindowManager` interface, `ordered_window_ids`, and `cancel_synthetic_drag`
- `crates/warpui/src/windowing/winit/window.rs (346-545)` — front-to-back window ordering on the winit backend
- `crates/warpui/src/windowing/winit/window.rs (556-755, 1348-1547)` — window creation and no-focus behavior for `WindowStyle::PositionedNoFocus`
- `crates/warpui/src/platform/mac/objc/window.m (366-565)` — AppKit drag-event routing and frame-constraint behavior during drag
- `integration/src/test/workspace.rs (71-270)` — drag feature gate and test helpers
- `integration/src/test/workspace.rs (520-833)` — integration coverage for continued drag after attach and single-tab handoff continuity

## 3. Current state

The branch already has the right overall architecture for this feature. The main opportunity for the tech spec is to make the architectural boundaries explicit so future changes do not collapse everything back into `Workspace` or regress into a copy-based implementation.

### Local reorder still exists and should stay separate

`Workspace::on_tab_drag(...)` still handles normal in-window reorder when the gesture remains local. That path continues to use the existing adjacent-tab threshold logic and mutate `self.tabs` directly.

That is good. Cross-window drag should layer on top of local reorder rather than replace it.

### Cross-window drag state is singleton-owned

The current branch stores multi-window drag state in `CrossWindowTabDrag`, not in any particular `Workspace`. That is the right ownership boundary because one gesture can involve:

- a source window
- an optional dedicated preview window
- one or more target windows
- repeated transitions between floating and inserted states before mouse-up

A per-workspace model would either duplicate state or require one workspace to imperatively mutate another during drag processing.

### The transferable unit is a live view tree

`TransferredTab` carries the real `PaneGroup` plus the tab metadata and `DraggableState` needed to preserve continuity. The current branch transfers the live view tree between windows via `transfer_view_tree_to_window(...)` instead of recreating tabs from serialized state.

That is the right implementation approach. The feature needs identity preservation, not reconstruction.

### The drag UI is intentionally shared across tab presentations

Both the classic top tab bar and vertical tabs dispatch the same workspace actions:

- `StartTabDrag`
- `DragTab`
- `DropTab`

Only the presentation-specific drag geometry differs. This is a strong design choice and should be preserved: drag orchestration belongs in workspace-level logic, not in individual tab UIs.

### Preview windows are treated as real workspaces, but temporary ones

For multi-tab drags, the current branch creates a real workspace window via `NewWorkspaceSource::TransferredTab`, then swaps the placeholder pane group for the transferred one. It also tracks `is_tab_drag_preview` and `suppress_detach_panes_on_window_close` to keep the preview out of persistence and to avoid destroying transferred panes during cleanup.

This is the right pattern because it reuses the normal workspace machinery while still marking preview windows as temporary implementation detail windows.

### Cross-platform windowing support exists in the architecture now

The current code no longer needs to be described as "macOS-only architecture." The winit backend now tracks front-to-back window ordering and treats `WindowStyle::PositionedNoFocus` specially, and the integration test gate checks the feature flag rather than the target OS.

That does not mean every backend is equally battle-tested yet, but the architectural contract is now cross-platform and should be documented that way.

## 4. Proposed changes

The best way to build and extend this feature is to preserve the current architecture and tighten its boundaries rather than simplify it into a more local but less correct implementation.

### 4.1 Keep `Workspace` as the event ingress and source-side cleanup owner

`Workspace` should remain responsible for:

- receiving `StartTabDrag`, `DragTab`, and `DropTab`
- deciding whether a drag is:
  - local reorder
  - first-time detach into a cross-window drag
  - or a forwarded event for an already-active cross-window drag
- extracting `TransferredTab` data from its own tab list
- applying caller-local cleanup returned by `DropResult`

This keeps the ownership model simple:

- `Workspace` owns its own tab list and subscriptions
- `CrossWindowTabDrag` owns the global drag lifecycle

### 4.2 Keep `CrossWindowTabDrag` as a singleton state machine

`CrossWindowTabDrag` should continue to own:

- `ActiveDrag`
- `DragSource`
  - `SingleTabWindow`
  - `MultiTabWindow { source_tab_index, preview_window_id }`
- `DragPhase`
  - `Floating`
  - `InsertedInTarget`
  - `Transitioning`
- attach-target detection
- handoff and reverse-handoff transitions
- drop finalization semantics

The important part is not just centralization. It is the explicit state machine. `Transitioning` is a necessary guard against re-entrant drag processing while views are being moved across windows.

### 4.3 Continue transferring live `PaneGroup` view trees

The implementation should continue to use `TransferredTab` plus `transfer_view_tree_to_window(...)` as the transfer mechanism.

This preserves:

- running terminal/process identity
- pane/view identity across attach and reverse-handoff
- panel-open state
- drag continuity through `DraggableState`

The `AppContext` support for `view_to_window`, structural parent/child tracking, and `transfer_structural_children(...)` is part of this design. It ensures non-rendered structural children move with the root pane group instead of being stranded in the old window.

Rebuilding a tab from serialized state would be simpler on paper but wrong for this feature.

### 4.4 Preserve the two preview strategies

The split between single-tab and multi-tab sources is correct and should remain.

#### Single-tab source

For a single-tab source window:

- the source window itself becomes the floating preview
- no dedicated preview window is created
- the window is repositioned directly to follow the drag
- handoff moves the real pane group out of the original window and can reverse back into it

This matches the product behavior and avoids unnecessary window creation.

Because the source window is already serving as the preview in this mode, the drag should not also present a second visual drag representation for the source tab. The likely fix is to keep using the existing `Draggable` event flow for continuity, but suppress the `Draggable` overlay paint for the source tab while the single-tab window is acting as the floating preview. Concretely, the single-tab path in `Workspace::on_tab_drag(...)` should set `DraggableState::set_suppress_overlay_paint(true)` when `begin_single_tab_drag(...)` takes over, then clear that suppression when the drag is handed off into another window, reversed back, or dropped/finalized. That keeps the tab rendered only once — inside the moving source window — and removes the extra overlay/paint churn that likely causes the observed text jitter when the drag starts directly on the tab.

#### Multi-tab source

For a multi-tab source window:

- the dragged tab is detached into a dedicated preview window
- the source window immediately switches to an adjacent tab if needed
- the preview window uses `WindowStyle::PositionedNoFocus`
- the live pane group is transferred into that preview window

This prevents the source window from rendering invalid content while the dragged tab is elsewhere.

### 4.5 Keep handoff and reverse-handoff explicit

Handoffs should continue to be explicit transitions rather than hidden side effects inside `on_drag(...)`.

The current split is good:

- `CrossWindowTabDrag::on_drag(...)` decides whether a handoff is needed
- `Workspace::perform_handoff(...)` routes to the appropriate concrete execution path
- `execute_handoff_single_tab_to_other(...)`
- `execute_handoff_back_to_caller(...)`
- `execute_handoff_multi_tab_to_other(...)`
- `reverse_handoff(...)`

This is the right separation because the singleton can reason globally about the drag phase, while the caller workspace still controls when its own subscriptions and tab list are updated.

### 4.6 Keep target-side reordering in the inserted state

Once a tab is attached into a target window, reordering within that target should stay inside `DragPhase::InsertedInTarget`.

That logic should remain separate from the local adjacent-swap path because cross-window reorder needs:

- screen-space coordinates rather than purely local tab-bar coordinates
- explicit leave-detection to trigger reverse-handoff
- insertion-index math that works after handoff, not just neighbor swapping

`Workspace::tab_insertion_index_for_cursor(...)` plus `CrossWindowTabDrag::on_drag_while_inserted(...)` is the right abstraction pair for this.

### 4.7 Keep the drag UI thin and reusable

Top tabs and vertical tabs should continue to be thin producers of drag events rather than owning drag orchestration. The UI layer should:

- host the `Draggable`
- report drag rectangles back to `Workspace`
- decide only presentation-specific things such as drag axis or row/group layout

It should not own:

- source/target workspace mutation
- preview-window lifecycle
- attach-target selection
- cross-window drop finalization

That keeps the feature reusable across tab presentations and prevents architectural duplication.

### 4.8 Preserve transfer-aware lifecycle handling

The best implementation keeps the current lifecycle rules:

- `prepare_for_transferred_tab_attach(...)` before a pane group moves out
- `close_window_for_content_transfer(...)` for source windows that are closing only because their content moved
- `TerminationMode::ContentTransferred` for transfer-driven closes
- `is_tab_drag_preview` for temporary preview windows
- `suppress_detach_panes_on_window_close` to avoid tearing down already-transferred panes
- `get_app_state(...)` skipping preview windows

Together these enforce the product invariants around:

- no destructive close warning during transfer
- no pane teardown after a successful transfer
- no preview-window persistence

### 4.9 Preserve platform-level contracts, not OS-specific assumptions

The tech spec should define the required platform capabilities, not encode "macOS behavior" as the architecture.

The required platform contracts are:

- create a window at exact bounds
- allow a drag preview window to exist without stealing focus
- expose front-to-back ordering of Warp windows for attach targeting
- let the drag loop continue correctly when the preview window moves or is hidden/shown
- let transfer-driven window closes avoid destructive close semantics

The current branch already moves in that direction:

- `WindowStyle::PositionedNoFocus`
- `ordered_window_ids()`
- `cancel_synthetic_drag()`
- preview-focus suppression in `AppContext`

The architecture should continue to be written in terms of those contracts, even where backend validation is still ongoing.

### 4.10 Rendering should preserve identity without visual duplication

The current branch hides the transferred source tab by rendering it at zero width while a cross-window drag is active, and suppresses hover-only overlays during drag.

That is preferable to removing the tab entry outright during the drag because it preserves:

- stable tab indices
- stable drag bookkeeping
- consistent local/source cleanup after drop

The same principle applies elsewhere: prefer preserving identity and hiding temporary visual state over destructively rewriting state mid-gesture.

For the single-tab-source path specifically, "hiding temporary visual state" should mean suppressing the tab's drag overlay while the window itself is the preview, not rendering both the moving window and a dragged tab overlay at the same time. That preserves identity without producing the visible text jitter/shimmer reported when the drag begins on the tab surface itself.

## 5. End-to-end flow

### Multi-tab detach → attach → reorder → drop

1. The tab UI emits `DragTab`.
2. `Workspace::on_tab_drag(...)` sees that the drag has moved outside the local tab bar.
3. The source workspace builds `TransferredTab` from the dragged tab.
4. `create_transferred_window(...)` creates a preview window and transfers the live `PaneGroup` tree into it.
5. `CrossWindowTabDrag::begin_multi_tab_drag(...)` stores the source window, source tab index, preview window, window size, drag offsets, and initial phase.
6. The source workspace switches to an adjacent tab if the dragged tab had been active.
7. While floating, `on_drag_while_floating(...)` keeps the preview window aligned under the cursor and queries `cross_window_attach_target(...)`.
8. If the cursor enters a target tab bar, `CrossWindowTabDrag` returns `DragResult::HandoffNeeded`.
9. `Workspace::perform_handoff(...)` moves the live tab into the target workspace.
10. While inserted in the target, `on_drag_while_inserted(...)` reorders the tab continuously within the target tab bar.
11. If the cursor leaves the target tab bar, `reverse_handoff(...)` moves the tab back into the preview window and returns the state machine to `Floating`.
12. On mouse-up, `CrossWindowTabDrag::on_drop(...)` finalizes the drag and returns `DropResult`.
13. `Workspace::handle_drop_result(...)` performs the source-side cleanup only.

### Single-tab source window

1. `Workspace::on_tab_drag(...)` treats the single-tab source as cross-window drag immediately.
2. The source window itself is repositioned and becomes the floating preview.
3. `begin_single_tab_drag(...)` stores the source window as both source and preview.
4. If the tab attaches into another window, the live `PaneGroup` moves into that target.
5. If the cursor leaves the target tab bar, `reverse_handoff(...)` moves the tab back into the original window and the drag continues.
6. On drop, the final window is focused and `deferred_focus(...)` restores input focus to the resulting active tab.

### Shared across top tabs and vertical tabs

The same workspace/state-machine flow should apply regardless of whether the drag began from:

- the classic horizontal tab bar
- the vertical tabs panel

The UI-specific layer should only affect drag geometry and presentation, not cross-window coordination.

## 6. Risks and mitigations

### Re-entrant drag handling during view transfer

Risk:

- moving views between windows triggers invalidation while a drag event is still in flight

Mitigation:

- keep `DragPhase::Transitioning` as an explicit guardrail around handoff and reverse-handoff

### Duplicate tabs or stale view subscriptions

Risk:

- a transferred tab remains visible or subscribed in the wrong workspace

Mitigation:

- `prepare_for_transferred_tab_attach(...)` before transfer
- `insert_transferred_tab_at_index(...)` on receipt
- hide the transferred source tab visually rather than deleting bookkeeping early
- remove or close the source side only during finalization

### Wrong attach target when windows overlap

Risk:

- the cursor hits a visually occluded tab bar and the tab inserts into the wrong window

Mitigation:

- `cross_window_attach_target(...)` should continue to prefer `ordered_window_ids()` plus actual tab-bar hit testing

### Focus theft from preview windows

Risk:

- preview creation or reverse-handoff steals focus from the active typing context

Mitigation:

- `WindowStyle::PositionedNoFocus`
- `AppContext::set_suppress_focus_for_window(...)`
- `deferred_focus(...)` only at finalization or target activation

### Single-tab drag jitter from double-rendered preview state

Risk:

- the single-tab path repositions the real window while the source tab's `Draggable` still paints its own drag overlay, producing extra paint churn and visible text jitter when the drag starts on the tab itself

Mitigation:

- when `begin_single_tab_drag(...)` takes ownership, suppress overlay painting on the source tab's `DraggableState`
- clear that suppression on handoff, reverse-handoff, and final drop cleanup so later drags render normally

### Source or preview windows destroy transferred panes on close

Risk:

- transfer-driven window close behaves like destructive close

Mitigation:

- `suppress_detach_panes_on_window_close`
- `close_window_for_content_transfer(...)`
- `TerminationMode::ContentTransferred`

### Preview windows leak into persistence

Risk:

- app-state snapshots capture transient preview windows

Mitigation:

- continue skipping `is_tab_drag_preview()` workspaces in `get_app_state(...)`

### Drift between tab UIs

Risk:

- top tabs and vertical tabs evolve separate drag behavior and stop matching product expectations

Mitigation:

- keep drag orchestration in shared workspace/state-machine code
- keep the UI layer thin and event-based

## 7. Testing and validation

### Current automated coverage

The branch already includes integration coverage for:

- attaching a dragged tab into another window and continuing to drag before drop
- starting from a single-tab window, attaching into another window, then dragging back out
- asserting final window count, total tab count, focus, and editor state

These tests are feature-gated via `drag_tabs_feature_enabled()` rather than gated to macOS only, which matches the current architecture better.

### Validation this feature should keep

- integration coverage for detach → attach → reorder → drop
- integration coverage for single-tab handoff → reverse-handoff → drop
- integration coverage or regression tests for repeated attach/detach cycles without tab duplication
- manual validation for overlapping-window attach targeting
- manual validation that preview/source windows never flash blank or transparent
- manual validation that no close-confirmation dialog appears for transfer-driven closes
- manual validation that the resulting active tab is focused after drop
- manual validation that both top tabs and vertical tabs drive the same cross-window behavior

### What should not be treated as sufficient

- local reorder tests alone
- snapshot/restore tests alone
- backend-specific manual validation without verifying the shared workspace/state-machine behavior

The highest-risk areas are still continuous drag across attach/detach transitions, z-order targeting, and lifecycle cleanup.

## 8. Follow-ups

- Add broader automated coverage for repeated attach/detach cycles, especially reverse-handoff edge cases.
- Add focused validation for vertical-tabs initiated drags so the shared action/state-machine contract stays honest.
- Continue hardening backend validation for preview focus and ordered-window targeting where native window behavior differs.
- If future tab UIs are added, require them to reuse the shared workspace drag actions rather than introducing a second cross-window drag implementation.
