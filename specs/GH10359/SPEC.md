# Spec: Bulk delete chat history (GH-10359)

## Summary

Add multi-select and bulk-delete affordances to the conversation
list, so users can clean up chat history in batches instead of
deleting conversations one row at a time. Includes a 5-second undo
window after each bulk delete.

## Problem

Today, deleting chat conversations is a per-row action. Users with
months of accumulated chat history have no efficient way to clean
up dozens of conversations at once. Issue #10359 asks for standard
multi-select + bulk-delete, with an undo affordance to soften the
risk of accidental destructive actions.

## Goals

- Enter a multi-select mode in the conversation list.
- Select via per-row checkbox, Cmd/Ctrl-click, Shift-click, or
  "Select all visible" / "Select all".
- Delete the entire selection with a single confirmation.
- Provide a 5-second undo affordance after deletion. Within the
  window, deletions are reversible; after, they are permanent.
- Keep keyboard navigation parity with mouse interactions.

## Non-Goals

- Not a chat-export feature.
- Not a chat-archive workflow (archive vs delete is a separate
  decision out of scope here).
- Not a server-side admin or workspace-wide bulk-deletion tool.
- Not a search-then-delete flow; selection is driven from the
  visible conversation list.
- Not changing single-row delete semantics. Single-row delete
  continues to work exactly as it does today.

## Behavior Contract

### B1. Entering selection mode

- A "Select" button appears in the conversation list header.
  Clicking it enters selection mode and renders a checkbox column
  on each conversation row.
- Right-clicking any conversation row exposes a "Select multiple"
  menu item that also enters selection mode and pre-selects the
  right-clicked row.
- While in selection mode, the header replaces the "Select"
  button with selection-mode controls (see B2 and B3).

### B2. Selection actions

- Per-row checkbox toggles the row's selection.
- Header controls (precise semantics):
  - **"Select all visible"** — selects only the rows currently
    materialized in the virtualized list's DOM (i.e., the rows
    actually rendered in the viewport plus any virtualization
    overscan window). Rows that scroll into view AFTER the action
    are NOT auto-selected. This is the **safe default**.
  - **"Select all"** — selects every conversation matching the
    current filter/search query, **including non-materialized
    rows** that the virtualization layer has not rendered. The
    client first fetches a count of the matching set; if the count
    exceeds **100**, the UI shows a confirmation chip
    (`Select all 247? [Confirm] [Cancel]`) before applying. With
    no active filter, "Select all" applies to all conversations;
    with an active filter, it applies to the filtered set only
    (e.g. `Select all 47 matching "foo"?`).
  - **"Clear selection"** — clears the selection set without
    exiting selection mode.
- Selection set tracks rows by **conversation ID**. The selection
  persists across filter/search changes — previously-selected
  conversations remain selected even if they are no longer visible
  due to a tightened filter. When this happens, the header chip
  surfaces both totals, e.g. `12 selected · 8 not currently
  visible`.
- Mouse modifiers:
  - Cmd-click (macOS) / Ctrl-click (Win/Linux) on a row toggles
    that single row's selection.
  - Shift-click on a row extends the selection from the most
    recently selected anchor row to the clicked row, inclusive.
- Selected count is shown live in the header, e.g.
  `12 selected`.

### B3. Delete action

- Header includes a "Delete selected (N)" button.
  - Disabled when N == 0.
  - **Skip-confirmation is NOT supported for bulk delete in V1.**
    Every bulk-delete operation prompts, regardless of any
    existing single-row "skip confirmations" preference. The
    blast radius of a bulk delete is materially larger than a
    single row, so V1 intentionally does not honor a skip-confirm
    preference here. Single-row delete continues to honor its
    existing preference unchanged.
- Confirmation dialog copy:
  > Delete N conversations? You have 5 seconds to undo.
  >
  > [Cancel] [Delete]
- After confirmation, all N conversations are removed from the
  list immediately with a fade-out transition.

### B4. Undo

- After deletion, a snackbar appears at the bottom of the
  conversation list:
  > Deleted N conversations. **Undo**
- The snackbar persists for 5 seconds.
- Clicking "Undo" within the 5-second window restores all
  deleted conversations to their original positions in the list,
  with their prior unread / pinned / starred state intact.
- After 5 seconds, the snackbar dismisses and the deletions
  become permanent (the underlying tombstones are committed and
  cannot be recovered through this flow).
- If the user triggers another bulk delete while a snackbar is
  active, the previous deletion is committed immediately
  (tombstones flushed) and a new snackbar is shown for the new
  batch. Undo only ever applies to the most recent batch.

### B4a. Undo accessibility

The 5-second undo window must be reachable without a mouse and
must work with reduced-motion and assistive-technology settings:

- The snackbar's "Undo" button is keyboard-reachable: pressing
  Tab while the snackbar is visible focuses the Undo button. Esc
  returns focus to the prior context (typically the conversation
  list) without invoking Undo.
- Screen readers announce the snackbar on appearance:
  > Deleted N conversations. Undo available for 5 seconds. Press
  > Tab to focus undo.
- The snackbar element uses `role="alert"` and
  `aria-live="assertive"` so the announcement is not deferred.
- For users with `prefers-reduced-motion`, the snackbar appears
  and dismisses without slide / fade animations; the 5-second
  visibility window is unchanged.
- Cmd+Z (macOS) / Ctrl+Z (Win/Linux) while focus is anywhere
  inside the conversation list also triggers Undo, provided the
  invocation occurs within the 5-second window. After the
  window, the shortcut is a no-op for this flow.

### B5. Active-conversation refocus

- If any of the deleted conversations was the currently active
  conversation, focus moves to the next-most-recent remaining
  conversation in the list.
- If the list becomes empty, the active-conversation pane shows
  the existing empty state used elsewhere.

### B6. Keyboard

- In selection mode:
  - Up / Down arrows move the focus row without changing
    selection.
  - Space toggles selection on the focused row.
  - Shift+Up / Shift+Down extend the selection by one row.
  - Cmd/Ctrl+A selects all visible (matches B2 "Select all
    visible").
  - Cmd/Ctrl+Shift+A selects all (matches B2 "Select all").
  - Delete / Backspace triggers the same flow as the
    "Delete selected (N)" button (with the B3 confirmation).
  - Esc exits selection mode and clears the selection.

### B7. State persistence

- Selection mode and the current selection set are **not**
  persisted across app restarts. Re-launching the app returns the
  conversation list to normal (non-selection) mode with no rows
  selected.
- The 5-second undo window is **not** persisted across app
  restarts. If the user quits Warp during the 5s window, the
  pending deletion is committed on shutdown.

## Settings / API surface

- No new user-facing settings.
- Existing single-row delete-confirmation preference (if any) is
  respected for single-row deletes; bulk delete intentionally
  does NOT honor a skip-confirm preference in V1 (see B3).
- Storage layer: `app/src/storage/conversations.rs` gains a
  batch-delete API and a tombstone window so the in-memory state
  can restore the batch within the 5s undo window without a full
  reload.

### Storage / API contract (bulk delete)

Bulk delete is implemented as a **single batch endpoint** rather
than a client-side fan-out of per-row deletes. This bounds the
failure surface and gives the server a single point of accounting.

- Endpoint: `DELETE /api/conversations`
  - Body: `{ ids: ["...", "...", ...] }`
  - Response: `{ results: [{ id, status, error_message? }, ...] }`
    where `status` is one of `"deleted" | "not_found" |
    "forbidden" | "error"`. `error_message` is present only when
    `status == "error"`.

- **Atomicity**: best-effort, **per-id**. Partial failure does
  not roll back the successful deletions. Each id outcome is
  reported individually so the client can surface granular
  feedback.

- **Per-batch limit**: maximum **500 ids per batch**. The client
  splits a larger selection into sequential 500-id batches and
  surfaces aggregate progress in the snackbar / header chip
  (e.g. `Deleting 1,200 conversations… (batch 2 of 3)`).

- **Failure handling**:
  - Successful per-id deletions enter the undo tombstone set and
    can be restored within the 5s window.
  - Per-id errors (`"error"` / `"forbidden"`) are NOT tombstoned;
    those rows remain in the conversation list with an inline
    error indicator and a re-tryable affordance.
  - On a network-level error mid-batch (no usable response),
    retry the failed batch up to **3 times** with exponential
    backoff (250ms, 500ms, 1s). After exhaustion, surface a
    user-visible error in the snackbar with a "Retry" affordance.

- **Undo endpoint**: `POST /api/conversations/restore`
  - Body: `{ ids: ["...", "...", ...] }`
  - The server retains tombstone state for at least the 5-second
    client undo window plus a server-side grace period sufficient
    to cover clock skew and request latency.

### Bulk-delete safety limits

- **Hard cap**: a single bulk-delete operation may target at most
  **5,000 conversations**. Selecting more than this and pressing
  Delete shows an error chip:
  > Maximum 5,000 conversations per delete — narrow your
  > selection.
- **Server-side rate limit**: at most **10 batch-delete
  requests per minute per user**. Exceeding this returns
  `429 Too Many Requests` and the client surfaces a transient
  error with retry-after guidance.

## Acceptance Criteria

- A1. Clicking "Select" enters selection mode; clicking again
  (now labeled "Cancel" or equivalent) exits and clears the
  selection.
- A2. Selecting via checkbox, Cmd-click, and Shift-click all
  produce the expected selection set; the live header count is
  accurate.
- A3. "Delete selected (N)" always prompts the bulk-delete
  confirmation. The per-row "skip confirmations" preference is
  not honored for bulk delete in V1.
- A4. Within 5s, clicking Undo restores all deleted conversations
  to their original positions and state.
- A5. Deleting the active conversation as part of a bulk batch
  refocuses to the next-most-recent remaining conversation.
- A6. Keyboard: Space toggles, Shift+Arrow extends, Cmd/Ctrl+A
  selects visible, Cmd/Ctrl+Shift+A selects all, Esc exits.
- A7. Restarting the app while in selection mode returns the
  list to normal mode with no selection.
- A8. Quitting during a 5s undo window commits the deletion on
  next launch (no zombie undo snackbar).
- A9. Triggering a second bulk delete during an active undo
  window commits the previous batch immediately and starts a new
  5s window for the new batch.

## Implementation Pointers

- `app/src/conversation_list/*.rs` — selection-mode state, row
  rendering with checkbox column.
- `app/src/conversation_list/header.rs` (new or extended) —
  selection toolbar with "Select all visible", "Select all",
  "Clear selection", "Delete selected (N)", and live count.
- `app/src/conversation_list/snackbar.rs` (new) — 5s undo
  snackbar component, reusable for future similar flows.
- `app/src/storage/conversations.rs` — batch-delete API,
  tombstone window, commit-on-timeout / commit-on-shutdown /
  commit-on-next-batch logic.
- `app/src/keybindings/conversation_list.rs` — keyboard map for
  selection mode.

## Tests

- T1. Selection-mode toggle: enter from header button, enter
  from row context menu, exit via Esc, exit via header.
- T2. Mixed selection paths: checkbox + Cmd-click + Shift-click
  produce the same selection set the user would expect from
  standard list semantics.
- T3. Bulk delete shows the confirmation dialog even when
  single-row skip-confirmation preference is on.
- T4. Undo within 5s restores all deleted conversations,
  including their original ordering and state (unread / pinned /
  active).
- T5. Active-conversation refocus on bulk delete picks the next
  most-recent remaining conversation; empty-state shown when
  list is fully drained.
- T6. Full keyboard navigation: every interaction in B6 is
  covered with a deterministic test.
- T7. Selection cleared on app restart (no persisted selection).
- T8. Quitting during 5s undo commits the deletion on next
  launch; the restored conversations do NOT reappear.
- T9. Back-to-back bulk deletes: second delete commits the
  first; undo only restores the second batch.

## Open Questions

- Q1. Should bulk-delete be gated behind a feature flag for the
  first preview rollout? Proposal: **yes**, gate to the Preview
  channel for one cycle, then promote to Stable.
- Q2. Should the snackbar duration be user-configurable? V1
  proposal: no — 5s is a fixed default matching macOS Finder's
  recover semantics. Revisit if accessibility feedback demands a
  longer window.
- Q3. Should "Select all" warn before selecting >N (e.g., 500)
  conversations? V1 proposal: no warning, but the confirmation
  dialog already shows the count.
- Q4. ~~Does the storage layer need a server-side bulk-delete
  endpoint, or is per-row deletion fanned-out client-side
  acceptable for V1?~~ **Resolved**: V1 uses a single batch
  endpoint (`DELETE /api/conversations`) with explicit per-id
  results, a 500-id batch cap, exponential-backoff retry, and
  tombstone-based undo. See **Storage / API contract** above.
  Client-side fan-out is rejected because it leaves
  partial-failure semantics undefined and makes server-side rate
  limiting and accounting unreliable.

## Telemetry

- No new event. Extend the existing `conversation.delete` event
  with two new fields:
  - `count: u32` — number of conversations in the deletion
    (1 for single-row delete, N for bulk delete).
  - `bulk: bool` — `true` when the deletion came from the
    bulk-delete flow, `false` for single-row.
- An additional `undone: bool` field on the same event records
  whether the user invoked Undo within the 5s window. Events for
  bulk deletions fire after the 5s window closes (or on
  early-commit due to next-batch / shutdown), so the field is
  always known at emit time.
