use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

use super::transcript::{PendingToolCall, TranscriptMessage};

#[derive(Clone, Default)]
pub(crate) struct ConversationStore {
    inner: Arc<RwLock<HashMap<String, ConversationState>>>,
    turn_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct ConversationState {
    pub(crate) conversation_id: String,
    pub(crate) task_id: String,
    pub(crate) messages: Vec<TranscriptMessage>,
    pub(crate) pending_tool_calls: HashMap<String, PendingToolCall>,
    pub(crate) completed_tool_call_ids: HashSet<String>,
}

impl ConversationStore {
    pub(crate) async fn lock_turn(&self, conversation_id: &str) -> OwnedMutexGuard<()> {
        if let Some(lock) = self.turn_locks.read().await.get(conversation_id).cloned() {
            return lock.lock_owned().await;
        }

        let mut guard = self.turn_locks.write().await;
        guard
            .entry(conversation_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
            .lock_owned()
            .await
    }

    pub(crate) async fn load_or_create(
        &self,
        conversation_id: String,
        task_id: String,
        seed_messages: Vec<TranscriptMessage>,
    ) -> (ConversationState, bool) {
        if let Some(existing) = self.inner.read().await.get(&conversation_id).cloned() {
            return (existing, false);
        }

        let mut guard = self.inner.write().await;
        if let Some(existing) = guard.get_mut(&conversation_id) {
            if existing.messages.is_empty() {
                existing.messages = seed_messages;
            }
            if existing.task_id.is_empty() {
                existing.task_id = task_id;
            }
            return (existing.clone(), false);
        }

        let state = ConversationState {
            conversation_id,
            task_id,
            messages: seed_messages,
            pending_tool_calls: HashMap::new(),
            completed_tool_call_ids: HashSet::new(),
        };
        let state_for_return = state.clone();
        guard.insert(state.conversation_id.clone(), state);

        (state_for_return, true)
    }

    pub(crate) async fn save(&self, state: ConversationState) {
        self.inner
            .write()
            .await
            .insert(state.conversation_id.clone(), state);
    }
}
