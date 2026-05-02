use std::sync::Arc;
use std::time::Duration;

use crate::ai::ambient_agents::AmbientAgentTaskId;
use anyhow::{anyhow, Context, Result};
#[cfg(not(target_family = "wasm"))]
use futures::future::Either;
#[cfg(not(target_family = "wasm"))]
use warpui::r#async::Timer;

use crate::ai::agent::ReceivedMessageInput;
use crate::server::server_api::ai::{AIClient, AgentRunEvent, ReadAgentMessageResponse};
use crate::server::server_api::ServerApi;

pub(crate) const DEFAULT_AGENT_MESSAGE_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Hydrates `new_message` agent events into full message payloads and delivery
/// acknowledgements.
#[derive(Clone)]
pub(crate) struct MessageHydrator {
    ai_client: Arc<dyn AIClient>,
    task_scoped_server_api: Option<Arc<ServerApi>>,
    task_id: Option<AmbientAgentTaskId>,
    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    fetch_timeout: Duration,
}

impl MessageHydrator {
    pub(crate) fn new(ai_client: Arc<dyn AIClient>) -> Self {
        Self::with_fetch_timeout(ai_client, DEFAULT_AGENT_MESSAGE_FETCH_TIMEOUT)
    }

    pub(crate) fn for_task(server_api: Arc<ServerApi>, task_id: AmbientAgentTaskId) -> Self {
        let ai_client: Arc<dyn AIClient> = server_api.clone();
        Self {
            ai_client,
            task_scoped_server_api: Some(server_api),
            task_id: Some(task_id),
            fetch_timeout: DEFAULT_AGENT_MESSAGE_FETCH_TIMEOUT,
        }
    }

    pub(crate) fn with_fetch_timeout(
        ai_client: Arc<dyn AIClient>,
        fetch_timeout: Duration,
    ) -> Self {
        Self {
            ai_client,
            task_scoped_server_api: None,
            task_id: None,
            fetch_timeout,
        }
    }

    async fn read_message(&self, message_id: &str) -> Result<ReadAgentMessageResponse> {
        match (self.task_scoped_server_api.as_ref(), self.task_id) {
            (Some(server_api), Some(task_id)) => {
                server_api
                    .read_agent_message_for_task(&task_id, message_id)
                    .await
            }
            _ => self.ai_client.read_agent_message(message_id).await,
        }
        .with_context(|| format!("Failed to read agent message {message_id}"))
    }

    pub(crate) async fn hydrate_event_for_recipient(
        &self,
        event: &AgentRunEvent,
        recipient_run_id: &str,
    ) -> Option<ReceivedMessageInput> {
        if event.event_type != "new_message" || event.run_id != recipient_run_id {
            return None;
        }

        let message = match self.read_message_from_event_with_timeout(event).await {
            Ok(message) => message,
            Err(err) => {
                log::warn!(
                    "Failed to hydrate agent message for event ref_id={:?}: {err:#}",
                    event.ref_id
                );
                return None;
            }
        };
        if message.body.is_empty() {
            log::warn!(
                "Hydrated empty-body agent message: message_id={} event_sequence={} recipient_run_id={} sender_run_id={} subject={:?} task_id={:?}",
                message.message_id,
                event.sequence,
                recipient_run_id,
                message.sender_run_id,
                message.subject,
                self.task_id.map(|task_id| task_id.to_string())
            );
        }

        Some(ReceivedMessageInput {
            message_id: message.message_id,
            sender_agent_id: message.sender_run_id,
            addresses: vec![recipient_run_id.to_string()],
            subject: message.subject,
            message_body: message.body,
        })
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) async fn read_message_with_timeout(
        &self,
        message_id: &str,
    ) -> Result<ReadAgentMessageResponse> {
        let read_message = self.read_message(message_id);
        let timeout = Timer::after(self.fetch_timeout);
        futures::pin_mut!(read_message);
        futures::pin_mut!(timeout);

        match futures::future::select(read_message, timeout).await {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(anyhow!("Timed out reading agent message {message_id}")),
        }
    }

    #[cfg(target_family = "wasm")]
    pub(crate) async fn read_message_with_timeout(
        &self,
        message_id: &str,
    ) -> Result<ReadAgentMessageResponse> {
        self.read_message(message_id).await
    }

    pub(crate) async fn read_message_from_event_with_timeout(
        &self,
        event: &AgentRunEvent,
    ) -> Result<ReadAgentMessageResponse> {
        let Some(message_id) = event.ref_id.as_deref() else {
            return Err(anyhow!("Agent event is missing ref_id"));
        };
        self.read_message_with_timeout(message_id).await
    }

    pub(crate) async fn mark_message_delivered(&self, message_id: &str) -> Result<()> {
        match (self.task_scoped_server_api.as_ref(), self.task_id) {
            (Some(server_api), Some(task_id)) => {
                server_api
                    .mark_message_delivered_for_task(&task_id, message_id)
                    .await
            }
            _ => self.ai_client.mark_message_delivered(message_id).await,
        }
        .with_context(|| format!("Failed to mark agent message {message_id} as delivered"))
    }

    pub(crate) async fn mark_messages_delivered_best_effort<'a, I>(
        &self,
        message_ids: I,
    ) -> Vec<(String, anyhow::Error)>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut failures = Vec::new();
        // TODO(REMOTE-1266): Parallelize delivery acknowledgements for bursty
        // batches once the parent-bridge restore path is hardened enough to
        // tolerate a concurrent FuturesUnordered/join_all flow here.
        for message_id in message_ids {
            if let Err(err) = self.mark_message_delivered(message_id).await {
                failures.push((message_id.to_string(), err));
            }
        }
        failures
    }
}
