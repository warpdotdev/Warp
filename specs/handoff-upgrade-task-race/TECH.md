# TECH.md — Fix UpgradeOptimisticTask race during local-to-cloud handoff

## Context
During local-to-cloud handoff, the viewer's shared session sometimes hits `UpgradeOptimisticTask(UnexpectedUpgrade)` when a replayed `CreateTask` client action is applied to a root task that is already `TaskImpl::Server`. This cascades into `TaskNotFound` errors for every subsequent action because the root task is removed from the task store but never re-inserted. In the non-error case the fork's conversation history is silently trampled — the pane header shows "New cloud agent" instead of the restored conversation.

The bug is a race condition. Shorter conversations reproduce more often; longer ones tend to succeed.

### Where the `CreateTask` comes from
The server does **not** emit `CreateTask` for the handoff follow-up request — the root task already exists in the forked conversation. The `CreateTask` comes from the client-side **historical replay reconstruction** in `reconstruct_response_events_from_conversations` (`replay_agent_conversations.rs:96-121`). When the cloud agent starts session sharing, `stream_historical_agent_conversations` (`terminal_manager.rs:1236`) reconstructs synthetic `CreateTask` events for every task on the first exchange of each conversation. These events are sent inside the `AgentConversationReplayStarted`/`Ended` window.

The skip logic at `should_skip_replayed_response_for_existing_conversation` (`shared_session.rs:243`) is designed to suppress these replayed events when the conversation already exists locally. But it only works when the conversation is discoverable via `find_existing_conversation_by_server_token`, which searches `all_live_conversations_for_terminal_view` — not `conversations_by_id`.

### Current handoff flow
`complete_local_to_cloud_handoff_open` in `workspace/view.rs (13472-13579)`:

1. `fork_conversation` → `insert_forked_conversation_from_tasks` (`history_model.rs:2126`) adds the forked conversation to `conversations_by_id` and `server_token_to_conversation_id`, but **not** to `live_conversation_ids_for_terminal_view`.
2. `restore_conversation_after_view_creation` (`load_ai_conversation.rs:549`) creates AI blocks, then calls `set_active_conversation_id` (`history_model.rs:720`). This checks whether the conversation is in the live list — it isn't — so it logs an error and returns early.
3. `set_server_conversation_token_for_conversation` (`workspace/view.rs:13532`) updates the conversation's token in `conversations_by_id`. This succeeds.
4. `enter_agent_view_for_conversation` (`agent_view.rs:123`) finds the conversation in memory (`is_conversation_in_memory = true`) but not live (`is_live = false`) and falls into an **async** `load_conversation_data` branch (line 154).
5. When the async fetch completes, `restore_conversations` (`history_model.rs:656`) finally adds the conversation to `live_conversation_ids_for_terminal_view`.

### The race
The cloud agent starts session sharing and emits replay events concurrently with step 4-5. The replay includes synthetic `CreateTask` for the root task. The skip logic at `should_skip_replayed_response_for_existing_conversation` calls `find_existing_conversation_by_server_token` which searches only `live_conversation_ids_for_terminal_view`.

- **Replay arrives before async fetch completes** → conversation not in live list → skip logic can't find it → replay events are applied to a **new** conversation → `CreateTask` succeeds on an `Optimistic` root task → **no error, but fork history is silently trampled** ("New cloud agent" header).
- **Replay arrives after async fetch completes** → conversation found in live list → skip logic checks `request_id` matching → on mismatch, replay events hit the fork's `Server` root task → **`UnexpectedUpgrade`**.

Shorter conversations load faster from the server, so the async fetch is more likely to complete before replay events arrive, making the error more frequent. Longer conversations take longer to fetch, giving the replay time to arrive first (no error, but trampled history).

### Secondary bug: task lost from store on error
The `CreateTask` handler at `conversation.rs:2312` removes the root task from the store, then calls `into_server_created_task` with `?`. On error, the task is never re-inserted, causing every subsequent `AddMessagesToTask`, `UpdateTaskDescription`, etc. to fail with `TaskNotFound`.

## Proposed changes
### Register forked conversation in live list synchronously
In `complete_local_to_cloud_handoff_open` (`workspace/view.rs`), add the forked conversation to `live_conversation_ids_for_terminal_view` for the cloud-mode terminal view **before** calling `restore_conversation_after_view_creation`. This eliminates the async fetch entirely — the conversation is already in memory and live, so `set_active_conversation_id` succeeds, `enter_agent_view_for_conversation` takes the synchronous `is_live = true` path, and the replay skip logic can find the conversation to suppress replayed events.

The conversation data is already fully materialized in `conversations_by_id` from `insert_forked_conversation_from_tasks` — the async server round-trip is redundant.

A new `register_conversation_for_terminal_view` method on `BlocklistAIHistoryModel` handles the registration. It is intentionally separate from `insert_forked_conversation_from_tasks` because that method is also used for non-handoff forks where no terminal view is known yet.

## Testing and validation
- **Manual**: Trigger local-to-cloud handoff on both short (1-2 exchanges) and long (50+ exchanges) conversations. Verify: no `UpgradeOptimisticTask` or `TaskNotFound` errors in logs, pane header shows the forked conversation title (not "New cloud agent"), and the cloud agent's response streams into the restored conversation.
- **Unit tests**: The `CreateTask` handler should be tested for the case where the root task is already `Server` — it should no-op rather than error. The task store should never lose a task on error.

## Parallelization
Not beneficial — the three changes are tightly coupled to the same code paths and should land in a single PR.
