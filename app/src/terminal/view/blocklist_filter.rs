//! Utilities for filtering which tasks and exchanges should be shown in the blocklist.

use crate::ai::agent::{conversation::AIConversation, task::Task, AIAgentExchange};

/// Returns whether a task's exchanges should be shown in the blocklist.
pub(super) fn should_show_task_in_blocklist(task: &Task) -> bool {
    // All tasks are visible in the blocklist aside from CLI (long-running command),
    // Warp documentation search, and conversation search subtasks.
    !task.is_cli_subagent()
        && !task.is_warp_documentation_search_subagent()
        && !task.is_conversation_search_subagent()
}

/// Returns true if the conversation contains at least one exchange that would be shown in the
/// blocklist (per `should_show_task_in_blocklist`).
pub(crate) fn conversation_would_render_in_blocklist(conversation: &AIConversation) -> bool {
    if conversation.exchange_count() == 0 {
        return false;
    }

    if conversation
        .get_root_task()
        .is_some_and(|task| should_show_task_in_blocklist(task) && task.exchanges_len() > 0)
    {
        return true;
    }

    conversation
        .all_tasks()
        .any(|task| should_show_task_in_blocklist(task) && task.exchanges_len() > 0)
}

/// Returns all exchanges from a conversation that should be displayed in the blocklist.
/// Filters by task type using `should_show_task_in_blocklist`.
pub(super) fn exchanges_for_blocklist(conversation: &AIConversation) -> Vec<&AIAgentExchange> {
    conversation
        .all_exchanges_by_task()
        .into_iter()
        .filter_map(|(task_id, exchanges)| {
            conversation
                .get_task(&task_id)
                .filter(|task| should_show_task_in_blocklist(task))
                .map(|_| exchanges)
        })
        .flatten()
        .collect()
}

#[cfg(test)]
#[path = "blocklist_filter_tests.rs"]
mod tests;
