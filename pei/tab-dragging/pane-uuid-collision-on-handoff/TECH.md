# TECH.md — Fix `terminal_panes.uuid` UNIQUE collision during cross-window tab drag

## Context

On the `pei/drag-tabs-out-of-windows` branch, every cross-window tab drop that closes the source window produces:

```
SQLite error 2067 (UNIQUE constraint failed): terminal_panes.uuid
SQLite Model error: error saving app state: UNIQUE constraint failed: terminal_panes.uuid
```

`save_app_state` (the only writer of `terminal_panes`) runs a delete-all-then-insert-all transaction in `app/src/persistence/sqlite.rs:808`. A UNIQUE error on insert means the `AppState` passed in contains **two tabs whose `TerminalPaneSnapshot.uuid` is identical**. That duplicate exists because during a handoff the source workspace's `self.tabs[source_tab_index].pane_group` and the target/preview workspace's newly inserted tab's `pane_group` are the **same** `ViewHandle<PaneGroup>`.

### Existing guard and why it's leaky

`app/src/app_state.rs:310` already documents and guards this race:

```rust
let cross_window_drag_active = CrossWindowTabDrag::as_ref(app).is_active();
...
if cross_window_drag_active {
    continue;
}
```

`CrossWindowTabDrag::is_active()` (`app/src/workspace/cross_window_tab_drag.rs:279`) returns `active_drag.is_some() || pending_source_cleanup`. `pending_source_cleanup` is set inside `finalize()` and cleared at the end of the `DropTab` handler (`app/src/workspace/view.rs:20297`).

The guard works for saves dispatched between `finalize()` and the end of `DropTab`, but it does **not** hold long enough for the `CloseSourceWindow` / `RemoveSourceTabAndClosePreview` paths:

1. `finalize()` takes `active_drag`, sets `pending_source_cleanup = true`, and returns `DropResult::CloseSourceWindow { ... }`.
2. `handle_drop_result` calls `close_window_for_content_transfer`, which calls `ctx.windows().close_window(..., TerminationMode::ContentTransferred)`. This is `close_window_async` in `crates/warpui_core/src/windowing/state.rs:138`; the actual window close / `Workspace::on_window_closed` / `WorkspaceRegistry::unregister` runs on a later tick.
3. `DropTab` handler calls `finish_source_cleanup()` → `pending_source_cleanup = false`.
4. Between step 3 and the OS-delivered window close, the source workspace is still in `WorkspaceRegistry` with its original `TabData { pane_group }` intact. Any `workspace:save_app` dispatched in that window (e.g. from the deferred focus, an active-window-changed observer, or a neighbor window's `ResizeMove`) calls `get_app_state` with `is_active() == false` → both source and target are snapshotted → same UUID inserted twice → UNIQUE failure.

The log confirms this timing, e.g.:

```
15:58:53.886 finalize_handoff -> CloseSourceWindow transferred_tab_index=0
15:58:53.886 dispatching global action for workspace:save_app
15:58:53.887 WindowId(1) will close                            <- async close notification
15:58:53.887 SQLite error 2067 (UNIQUE constraint failed)      <- save already ran
15:58:53.962 dealloc native window 0x88ef40000                 <- source actually deallocated
```

`finalize_preview_as_new_window -> NoOp (source placeholder already consumed by put-back)` hits the same collision because `pending_source_cleanup` is cleared unconditionally by the `DropTab` handler even when no source cleanup runs.

### Relevant files

- `app/src/workspace/cross_window_tab_drag.rs (100-109, 271-327, 707-764, 816-867)` — `CrossWindowTabDrag`, `is_active`, `finalize`, `finalize_handoff`, `finalize_preview_as_new_window`.
- `app/src/workspace/view.rs (20269-20300)` — `DropTab` handler; calls `handle_drop_result` then `finish_source_cleanup`.
- `app/src/workspace/view.rs (22708-22712, 22630-22648)` — `close_window_for_content_transfer`, `on_window_closed`.
- `app/src/app_state.rs (302-351)` — `get_app_state` and the drag guard.
- `crates/warpui_core/src/windowing/state.rs (138-141)` — `close_window` is async.

## Proposed changes

Shift the cleanup signal from a **temporal** guard (`bool` cleared by the action handler) to a **lifecycle** guard (set of source windows that still hold the duplicate UUID, cleared when each source window actually closes).

### 1. Track pending source-window closures on the drag model

In `cross_window_tab_drag.rs`:

- Replace `pending_source_cleanup: bool` with `pending_source_window_closes: HashSet<WindowId>`.
- Add `register_pending_source_close(window_id)` and `finish_pending_source_close(window_id)`.
- `is_active()` becomes `active_drag.is_some() || !pending_source_window_closes.is_empty()`.
- Drop the existing no-op `finish_source_cleanup()` path; callers switch to `finish_pending_source_close`.

The set holds a window id iff that source window's close has been requested as part of a handoff and its `on_window_closed` has not yet run. This is the exact window during which the source still carries a duplicate UUID but no drag is active.

### 2. Register the pending close in `finalize` only when a close was actually requested

Only the `CloseSourceWindow` and `RemoveSourceTabAndClosePreview` branches need the guard to outlive `DropTab`:

- `finalize_handoff` single-tab path (returns `CloseSourceWindow`) → register `drag.source_window_id`.
- `finalize_handoff` multi-tab path (returns `RemoveSourceTabAndClosePreview`) → register `drag.preview_window_id()` (the preview is the window being closed; the source itself keeps its tabs without the duplicate once `RemoveSourceTab` runs synchronously in `handle_drop_result`).
- `finalize_preview_as_new_window` single-tab branch (returns `CloseSourceWindow`) → register `drag.source_window_id`.
- `FocusSelf`, `RemoveSourceTab`, and the `NoOp` branches do **not** register anything (no pending close, no duplicate state after `handle_drop_result` returns).

### 3. Clear the pending entry from the closing workspace's lifecycle hook

In `Workspace::on_window_closed` (`app/src/workspace/view.rs:22630`), after the existing `WorkspaceRegistry::unregister` call, also:

```rust
CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
    drag.finish_pending_source_close(window_id);
});
```

This runs after the workspace has been unregistered, so any `save_app` fired from the same tick sees both `is_active() == false` **and** the source workspace already gone from `WorkspaceRegistry::get` — no duplicate remains.

### 4. Drop the premature clear in the `DropTab` handler

In the `DropTab` arm of `Workspace::handle_action` (`app/src/workspace/view.rs:20291-20299`), remove the unconditional `finish_source_cleanup()` call. The new lifecycle-driven clear in step 3 replaces it.

### 5. Preserve the existing `get_app_state` call site

`app_state.rs` needs no change: it continues to call `CrossWindowTabDrag::as_ref(app).is_active()`. The semantics of `is_active` are stricter now but the guard shape is identical.

## Testing and validation

### Manual repro

Before the fix, any tab drag whose drop routes through `CloseSourceWindow` reliably produces `SQLite Model error: error saving app state: UNIQUE constraint failed: terminal_panes.uuid` in the log. After the fix, a similar repro sequence must produce zero such errors.

Repro steps (run with `RUST_BACKTRACE=1 ./script/run --features drag_tabs_to_windows`):

1. Open two windows; keep two tabs in window A, one in window B.
2. Drag B's only tab onto A's tab bar and drop. (`finalize_handoff -> CloseSourceWindow`.)
3. Drag a tab from A out to empty space. (`finalize_preview_as_new_window` → new window.)
4. From that new window, drag its only tab back onto A's tab bar. (`finalize_handoff -> CloseSourceWindow` again.)
5. Perform a put-back: multi-tab drag out, then drop back on the source tab bar. (`finalize_preview_as_new_window -> NoOp (source placeholder already consumed by put-back)`.)

Expected: no `UNIQUE constraint failed` entries in the log across the whole session.

### Automated checks

- `cargo fmt`.
- `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings`.
- `cargo nextest run -p warp --features drag_tabs_to_windows` for any `cross_window_tab_drag` or `app_state` tests.
- If time allows, extend `app/src/workspace/cross_window_tab_drag_tests.rs` (or the existing test module alongside it) with a unit test that: constructs a drag, calls `finalize` on the `CloseSourceWindow` branch, asserts `is_active()` is still true, invokes `finish_pending_source_close(window_id)`, asserts `is_active()` is now false.

## Risks and mitigations

- **Never clearing the set (window close notification doesn't fire)**: if `on_window_closed` somehow never runs, the drag model would stay "active" forever and persistence would be permanently disabled for the session. Mitigation: also provide an escape hatch — have `begin_single_tab_drag` / `begin_multi_tab_drag` discard any stale entries older than the current session start, and log a warning if `finish_pending_source_close` is called for an unknown id.
- **Interaction with `verify-ui-change-in-cloud` or integration tests that mock windows**: the new lifecycle hook runs in the same `on_window_closed` code path that integration tests already drive, so this should be transparent. If a test environment skips `on_window_closed` we'll add explicit cleanup there.

## Follow-ups

- The deeper design issue — that `Workspace::self.tabs` on the source keeps a `ViewHandle<PaneGroup>` pointing to the same pane that now logically belongs to the target — is still present. A future refactor should make `perform_handoff` atomically remove the source `TabData` and reinsert it on `reverse_handoff`, so that the UUID duplication state never exists in the first place. That change is larger because it touches `reverse_handoff` and the placeholder-tab flow; the current spec intentionally stops at closing the persistence race.
