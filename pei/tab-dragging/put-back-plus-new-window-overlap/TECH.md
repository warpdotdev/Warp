# TECH.md — Fix overlap between put-back and preview-promotion during a single tab drag
## Context
On `pei/drag-tabs-out-of-windows`, dragging a tab out of a multi-tab window and back in repeatedly can end with the same pane group living in **both** the source window and the promoted preview window. The user-visible symptoms are:
- The total tab count grows by one after a single drop.
- The persistence layer logs `SQLite Model error: error saving app state: UNIQUE constraint failed: terminal_panes.uuid` immediately after `finalize_preview_as_new_window -> NoOp (source placeholder already consumed by put-back)`.
The earlier fix in `pei/tab-dragging/pane-uuid-collision-on-handoff/TECH.md` patched the persistence race by extending `CrossWindowTabDrag::is_active()` through the async source-window close. It does not address the structural bug below: the drop handler can execute two logically exclusive resolution paths in one drag, each leaving a copy of the pane group behind.
## Current state
`CrossWindowTabDrag::on_drop` (`app/src/workspace/cross_window_tab_drag.rs:692-744`) attempts to resolve three mutually exclusive outcomes:
1. Phase `InsertedInTarget` → `finalize_handoff`. When `source == target` with a dedicated preview (cross_window_tab_drag.rs:897-909), this branch closes the preview and returns `NoOp` — the tab lives in the source, the preview is gone. Correct.
2. Phase `Floating` with a dedicated preview → `finalize_preview_as_new_window` (cross_window_tab_drag.rs:833-874). Unconditionally promotes the preview (`set_is_tab_drag_preview(false)`, `show_window_and_focus_app`, `deferred_focus`), then either returns `NoOp` (if `source_placeholder_consumed`) or `RemoveSourceTab` / `CloseSourceWindow`.
3. Phase `Floating` drop-time re-resolve → if the cursor happens to sit over a tab bar at mouse-up, `on_drop` returns `DropResult::DropInto { target }` and `handle_drop_result` runs `self.perform_handoff(target, ctx)` + a second `finalize` (`app/src/workspace/view.rs:23197-23207`).
The `source_placeholder_consumed` flag (cross_window_tab_drag.rs:159-164) was added to cover the interleaving where branch 2 fires after a prior put-back already removed the placeholder, so the source now holds the real tab and the preview must not ask for another removal. That flag is necessary but not sufficient — it only gates the `DropResult` return value, not the side effects.
### Overlap A — second `perform_handoff` via drop-time re-resolve uses a stale `source_tab_index`
`on_drop` only guards the re-resolve on `!drag.drop_resolution_attempted`. It has no awareness of `source_placeholder_consumed`. Sequence:
1. User drags tab at index `i` out → preview holds the pane group.
2. User hovers source tab bar → `HandoffNeeded` → `perform_handoff` branch `target==caller` (view.rs:22886-22947) runs: view tree transferred back to source, `remove_tab_without_undo(source_tab_index)` then `insert_transferred_tab_at_index(info.transferred_tab, info.insertion_index)`. `source_placeholder_consumed = true`. Preview's `tabs` is **not** cleared by put-back — it still has its own copy of the pane group ViewHandle.
3. User drags out of tab bar → `reverse_handoff` (cross_window_tab_drag.rs:1168-1244). Source removes at `target_insertion_index`; preview calls `tabs.clear()` + `insert_transferred_tab_at_index`. Phase → `Floating`.
4. User releases with cursor still over the source tab bar. `on_drop` sees `phase=Floating`, dedicated preview, `drop_resolution_attempted=false` → returns `DropResult::DropInto { target: source }`.
5. `handle_drop_result` runs `self.perform_handoff(target=source, ctx)`. Re-enters the `target.window_id == caller_window_id` branch. At this point:
   - `drag.source_tab_index()` still returns the **original** `i` (never updated).
   - `self.tabs` has already been reshuffled by step 2's insert + step 3's remove, so `tabs.get(i)` points at an unrelated tab.
   - `unsubscribe_to_view(&tabs[i].pane_group)` and `remove_tab_without_undo(i)` now mutate that unrelated tab.
   - `execute_handoff_back_to_caller` transfers the preview's pane group back into the source *again*, and `insert_transferred_tab_at_index` inserts a fresh `TabData` referencing it.
6. Second `finalize` runs in phase `InsertedInTarget` with `source == target` → closes the preview, returns `NoOp`.
Net result: the pane group is present in the source twice (from step 2's still-live insert and from step 5's re-insert), a bystander tab has been wrongly removed, and the preview may have already been closed asynchronously while still holding a `TabData` pointing at the same pane group for a tick. `save_app` during any intermediate tick snapshots two windows with the same `terminal_panes.uuid`.
### Overlap B — `finalize_preview_as_new_window` promotes before checking `source_placeholder_consumed`
```rust path=/Users/pei/repos/warp-internal/app/src/workspace/cross_window_tab_drag.rs start=840
if let Some(ws) = WorkspaceRegistry::as_ref(ctx).get(preview_window_id, ctx) {
    ws.update(ctx, |ws, ctx| {
        ws.set_is_tab_drag_preview(false);
        ws.sync_window_button_visibility(ctx);
        ws.update_titlebar_height(ctx);
        ctx.notify();
    });
}
ctx.windows().show_window_and_focus_app(preview_window_id);
Self::deferred_focus(preview_window_id, ctx);

if drag.source_placeholder_consumed {
    return DropResult::NoOp;
}
```
Every `Floating` drop with a dedicated preview unconditionally promotes the preview window, *then* decides whether to also touch the source. In the `source_placeholder_consumed = true` case, that's exactly when the user has put the tab back — they did not ask for a new window, they asked for the tab to live in the source. Yet the preview, which still holds a `TabData` referencing the same pane group after `reverse_handoff`'s `insert_transferred_tab_at_index`, is now a permanent window. Even though the source's `tabs.len()` is correct on paper, the pane-group ViewHandle is reachable from two distinct `Workspace::tabs` vectors, and the first `save_app` after `is_active()` drops to `false` trips the UNIQUE constraint.
The observed log sequence matches exactly:
```
09:54:52.160 finalize branch=Floating+dedicated_preview -> finalize_preview_as_new_window (CREATES NEW WINDOW)
09:54:52.162 finalize_preview_as_new_window -> NoOp (source placeholder already consumed by put-back)
09:54:52.162 dispatching global action for workspace:save_app
09:54:52.163 SQLite error 2067 (A UNIQUE constraint failed): terminal_panes.uuid
```
### Relevant files
- `app/src/workspace/cross_window_tab_drag.rs (692-744, 753-826, 833-874, 878-930, 1168-1244)` — drop entry + three resolve paths + reverse_handoff.
- `app/src/workspace/view.rs (22866-22985, 23161-23208)` — `perform_handoff` + `handle_drop_result`.
- `pei/tab-dragging/pane-uuid-collision-on-handoff/TECH.md` — existing persistence-race fix; this change layers on top of it.
## Proposed changes
Make the drop paths **actually** mutually exclusive. The guiding invariant:
> If `source_placeholder_consumed` is true, the tab already lives in the source. The preview must be closed, not promoted, and no further `perform_handoff` may run.
### 1. Short-circuit `on_drop` when the placeholder has already been consumed
In `CrossWindowTabDrag::on_drop` (cross_window_tab_drag.rs:692-744), skip the drop-time re-resolve entirely when `source_placeholder_consumed` is true. The put-back already committed the tab to the source; re-resolving would only produce the stale-index double-handoff in Overlap A.
Concretely: wrap the existing `if matches!(drag.phase, DragPhase::Floating) && drag.has_dedicated_preview_window() && !drag.drop_resolution_attempted` block with an additional `&& !drag.source_placeholder_consumed`. Add a log line in the skipped branch so regression tests can catch if it starts firing again.
### 2. Collapse `finalize_preview_as_new_window` NoOp into a preview close
Rename the current function into two outcomes and branch on `source_placeholder_consumed` **before** promoting the preview:
- `source_placeholder_consumed = true`: do not touch `is_tab_drag_preview`, do not `show_window_and_focus_app`, do not `deferred_focus` the preview. Instead, close the preview via `ctx.windows().close_window(preview_window_id, TerminationMode::ContentTransferred)` (mirroring the `finalize_handoff` source==target branch at cross_window_tab_drag.rs:903-906) and return `NoOp`. Then register the pending close via `register_pending_source_close(preview_window_id)` in `finalize`'s post-processing match so `is_active()` stays true until `on_window_closed` fires — preventing the same UNIQUE race the earlier TECH.md fixed for `CloseSourceWindow` and `RemoveSourceTabAndClosePreview`.
- `source_placeholder_consumed = false` (genuine new-window case): keep the existing promotion code path unchanged — `set_is_tab_drag_preview(false)`, focus, return `CloseSourceWindow` / `RemoveSourceTab` depending on `source_was_single_tab()`.
Because the promote path returns `CloseSourceWindow` / `RemoveSourceTab` (the latter is already safe today because it runs synchronously in `handle_drop_result`), no further registration changes are needed for it.
### 3. Add the new `NoOp + close-preview` result to `finalize`'s registration match
In `CrossWindowTabDrag::finalize` (cross_window_tab_drag.rs:799-823), the post-result match already registers `CloseSourceWindow` and `RemoveSourceTabAndClosePreview`. Extend it to also register for the new "Floating + placeholder already consumed" result. The cleanest shape is a dedicated `DropResult` variant:
```rust path=null start=null
DropResult::ClosePreviewOnly { preview_window_id: WindowId }
```
with `finalize_preview_as_new_window`'s consumed-branch returning it and the registration match adding:
```rust path=null start=null
DropResult::ClosePreviewOnly { preview_window_id } => {
    self.register_pending_source_close(*preview_window_id);
}
```
`handle_drop_result` (view.rs:23161-23208) gets a new arm:
```rust path=null start=null
DropResult::ClosePreviewOnly { preview_window_id } => {
    ctx.windows().close_window(preview_window_id, TerminationMode::ContentTransferred);
}
```
This keeps the drop-resolution policy explicit at the call site instead of relying on side effects inside `finalize_preview_as_new_window`.
### 4. Keep `perform_handoff`'s put-back branch defensive
Even with (1) preventing the known re-resolve path into a stale-index handoff, `perform_handoff`'s `target.window_id == caller_window_id` branch should bail out early when `source_placeholder_consumed` is already true. Any future code path that reaches this branch after a put-back is by definition a bug, and the cheap guard turns it into an observable `log::warn` + `reset_to_floating` instead of silent `tabs.get(source_tab_index)` data corruption.
Implementation: at the top of the `target == caller_window_id` block in `Workspace::perform_handoff` (view.rs:22886), add:
```rust path=null start=null
if CrossWindowTabDrag::as_ref(ctx)
    .source_placeholder_consumed()
{
    log::warn!(
        "tab_drag: perform_handoff target==caller called with source_placeholder_consumed=true -> reset_to_floating"
    );
    CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
        drag.reset_to_floating();
    });
    return;
}
```
This requires exposing a `source_placeholder_consumed(&self) -> bool` accessor on `CrossWindowTabDrag` alongside the existing getters (cross_window_tab_drag.rs:312-360). It is distinct from the existing `source_placeholder_tab_index()` helper: callers here care about the boolean, not the index.
## Testing and validation
### Manual repro
The user's existing repro (drag a tab out, back, and then release in empty space repeatedly) must no longer produce:
- Duplicate tabs (`ActivateTab(N)` log lines with `N >= tab_count_before_drag + 1`).
- `UNIQUE constraint failed: terminal_panes.uuid` errors anywhere in the session.
New repro to exercise the `DropInto` guard (Overlap A):
1. Window A has ≥ 2 tabs.
2. Drag tab 1 out so a preview is created.
3. Drag the preview back over window A's tab bar to trigger a put-back.
4. Drag back off the tab bar, then release the mouse with the cursor still over the tab bar.
5. Expect: tab is inserted exactly once at the cursor position; preview is closed; no UNIQUE error.
### Automated checks
- `cargo fmt`.
- `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings`.
- `cargo nextest run -p warp --features drag_tabs_to_windows`.
- Extend `app/src/workspace/cross_window_tab_drag_tests.rs` (creating it if absent) with unit tests for:
  - `on_drop` returning `NoOp` (not `DropInto`) when `source_placeholder_consumed = true`.
  - `finalize_preview_as_new_window` → new `ClosePreviewOnly` branch when consumed, old `CloseSourceWindow` / `RemoveSourceTab` branches when not consumed.
  - `finalize` registering `pending_source_window_closes` for the `ClosePreviewOnly` variant.
## Risks and mitigations
- **Accidentally blocking a legitimate new-window drop.** Overlap B's fix only changes behavior when `source_placeholder_consumed` is true, which is only set inside the put-back branch of `perform_handoff`. A drag that never put-back keeps today's promotion path bit-for-bit.
- **Leaking the preview window if the close-preview path's `on_window_closed` never fires.** Same class of risk the earlier `pending_source_window_closes` fix addressed; the `finish_pending_source_close` lifecycle hook already handles stale entries in `Workspace::on_window_closed`.
- **New `DropResult` variant touches several call sites.** `handle_drop_result` in view.rs is the only consumer of `DropResult`; the change is local.
## Follow-ups
- Longer-term: make the preview's `tabs` vector the single source of truth during a multi-tab drag and drop the dual-ownership state entirely. `perform_handoff` put-back would then move the `TabData` from preview to source (not clone-through-a-`TransferredTab`), and the `source_placeholder_consumed` flag disappears. Out of scope for this fix because it touches `reverse_handoff`, `execute_handoff_back_to_caller`, and all the window-promotion paths.
