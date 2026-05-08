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
  - **"Select all visible"** — selects every row that is at least
    PARTIALLY inside the conversation list's scrollable viewport
    rectangle at the moment the action is invoked. The selection
    is computed from the conversation-list MODEL using the
    viewport's scroll offset and item height — it is NOT driven by
    the virtualization overscan window or the materialized DOM,
    both of which are implementation details that can vary by
    overscan factor or platform. Specifically:
    - The set is `{ row | row.bottom > viewport.top AND
      row.top < viewport.bottom }`, evaluated against the model
      (not the DOM).
    - Overscan-only rows (rendered for scroll smoothing but
      outside the visible rectangle) are NOT included.
    - Rows that scroll into view AFTER the action are NOT
      auto-selected.
    - The set is deterministic for a given (scroll offset, item
      list, viewport size) tuple regardless of virtualization
      configuration.
    This is the **safe default**.
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
  list immediately with a fade-out transition (optimistic UI).
- **Reconciling optimistic removal with per-id failure.** The
  optimistic removal is reversed for any id that the server
  reports as `"error"` or `"forbidden"` (and never tombstoned —
  see Storage / API contract). The reconciliation is:
  1. On confirmation, all N rows are removed from the rendered
     list and added to the in-memory tombstone set.
  2. The client issues the batched server request(s).
  3. As each batch response arrives, ids returned with status
     `"deleted"` or `"not_found"` remain removed and stay
     tombstoned for the undo window. ids returned with status
     `"error"` or `"forbidden"` are RE-INSERTED into the list at
     their original positions, removed from the tombstone set,
     and rendered with an inline error indicator and Retry
     affordance.
  4. The header chip and snackbar count are updated to reflect
     the actual number of successfully tombstoned conversations.
     If reconciliation reduces the count to 0, the snackbar is
     dismissed and an error chip surfaces instead.

### B4. Undo

- After deletion, a snackbar appears at the bottom of the
  conversation list:
  > Deleted N conversations. **Undo**
- The snackbar persists for 5 seconds.
- **Undo-window start for multi-batch deletes.** When the
  selection exceeds the 500-id batch cap and the client splits
  into multiple sequential batches, the 5-second undo timer
  starts when the **FINAL batch's response is received** (or its
  retry budget is exhausted), NOT at confirmation time and NOT
  per-batch. The snackbar appears with a progress indicator
  during multi-batch processing
  (`Deleting 1,200 conversations… (batch 2 of 3)`); it does NOT
  show "Undo" until all batches have settled. This guarantees:
  - The user never has the timer expire on later batches before
    earlier batches return.
  - "Undo" within the 5-second window restores the entire
    selection (subject to per-id reconciliation in B3), not a
    partial subset.
  - For a single-batch delete (≤500 ids) this rule reduces to
    "timer starts when the single batch response is received",
    matching the simple case.
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

- Endpoint: `POST /api/conversations:batchDelete`
  - Method: **`POST`** with a JSON body. We deliberately do NOT
    use `DELETE` here. Many HTTP intermediaries (CDNs, proxies,
    some load balancers) and several client HTTP libraries either
    drop or refuse to forward bodies on `DELETE` requests; the
    behavior is implementation-defined per RFC 9110 §9.3.5. To
    avoid that interop landmine for a feature whose blast radius
    is destructive bulk deletion, we use a `POST` to a
    `:batchDelete` action sub-resource (the same convention used
    by Google Cloud APIs and recommended by AIP-165). Single-row
    delete continues to use `DELETE /api/conversations/{id}`
    unchanged — only the batch endpoint is `POST`.
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

- **Undo endpoint**: `POST /api/conversations:batchRestore`
  - Body: `{ ids: ["...", "...", ...] }`
  - The server retains tombstone state for at least the 5-second
    client undo window plus a server-side grace period sufficient
    to cover clock skew and request latency.

### Authentication & per-conversation authorization

The destructive batch-delete and batch-restore endpoints are
authenticated and authorized BY CONVERSATION, not just by user
session. The contract is:

- **Authentication.** Both `POST /api/conversations:batchDelete`
  and `POST /api/conversations:batchRestore` require a valid Warp
  user session token in the `Authorization` header, identical to
  the existing single-row `DELETE /api/conversations/{id}`
  endpoint. Unauthenticated requests return `401 Unauthorized`
  with NO per-id `results` body — the request is rejected at the
  edge before any id is processed.
- **Per-conversation authorization (delete).** For each id in the
  request body, the server independently verifies that the
  authenticated user has delete permission on that conversation:
  - The conversation's owning user must equal the authenticated
    user, OR the authenticated user must hold an explicit
    workspace role granting delete-conversation permission for
    the conversation's workspace.
  - Workspace-scoped conversations require the same workspace
    membership and role check the server already enforces on
    single-row delete.
  - Authorization is checked AFTER the id is parsed but BEFORE
    any tombstone/state mutation for that id.
  - An id the user does not own and does not have workspace
    permission for returns `status: "forbidden"` for that id with
    NO `error_message` field that could leak conversation
    existence (see "Information disclosure" below). The other ids
    in the same batch are processed normally — one forbidden id
    does not poison the batch.
- **Per-conversation authorization (restore).** Restore uses the
  STRICTER check:
  - The id must be present in the server-side tombstone set
    associated with the SAME authenticated user that issued the
    delete. Restoring a tombstone created by a different user is
    rejected with `status: "forbidden"` per id.
  - Tombstones are scoped to `(user_id, conversation_id)` on the
    server. There is no cross-user undo path.
  - Restore can only resurrect conversations the user could
    delete in the first place; workspace permission changes
    between delete and restore that REVOKE delete permission also
    revoke restore (returned as `"forbidden"`).
- **Information disclosure.** `"not_found"` and `"forbidden"` are
  intentionally distinct in the response so the client can render
  accurate inline status, but the server MUST NOT leak whether a
  conversation outside the user's authorization scope actually
  exists. A request for an id that exists but belongs to another
  user MUST return `"forbidden"`, never `"not_found"` and never
  `"deleted"`. A request for an id that does not exist anywhere
  returns `"not_found"`.
- **Logging.** Every per-id `"forbidden"` outcome is logged
  server-side with the authenticated user id, the requested
  conversation id, and the reason; the client never sees those
  details.
- **Rate limit interaction.** The `429 Too Many Requests` rate
  limit defined in "Bulk-delete safety limits" applies AFTER
  authentication; an unauthenticated request gets `401`, not
  `429`. The rate limit is keyed by authenticated user id, not by
  IP, to prevent shared-network users from cross-rate-limiting
  each other.

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
- Q3. ~~Should "Select all" warn before selecting >N (e.g., 500)
  conversations?~~ **Resolved**: V1 keeps the >100 confirmation
  chip on "Select all" defined in B2 (`Select all 247?
  [Confirm] [Cancel]`). The earlier "no warning" wording in this
  question was inconsistent with B2 and is superseded — B2 is the
  authoritative contract. The B3 delete confirmation always
  shows the count as a second guardrail, but it does not replace
  the >100 selection-time confirmation.
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
