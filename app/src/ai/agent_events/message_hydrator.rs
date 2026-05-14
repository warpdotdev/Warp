use crate::ai::agent::ReceivedMessageInput;
use crate::ai::agent_events::AgentRunEvent;

/// OpenWarp 本地构建不再从云端 mailbox 拉取消息正文或发送 delivered 回执。
/// 该类型保留本地 harness 桥接调用面的无副作用兼容语义。
#[derive(Clone)]
pub(crate) struct MessageHydrator;

impl MessageHydrator {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn hydrate_event_for_recipient(
        &self,
        event: &AgentRunEvent,
        recipient_run_id: &str,
    ) -> Option<ReceivedMessageInput> {
        if event.event_type != "new_message" || event.run_id != recipient_run_id {
            return None;
        }

        None
    }

    pub(crate) async fn mark_messages_delivered_best_effort<'a, I>(
        &self,
        _message_ids: I,
    ) -> Vec<(String, anyhow::Error)>
    where
        I: IntoIterator<Item = &'a str>,
    {
        Vec::new()
    }
}
