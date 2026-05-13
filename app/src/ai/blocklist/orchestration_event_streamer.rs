use super::history_model::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};
use super::orchestration_events::{OrchestrationEventService, PendingEvent, PendingEventDetail};
use crate::ai::agent::{
    conversation::{AIConversationId, ConversationStatus},
    ReceivedMessageInput,
};
use crate::ai::agent_events::{
    run_agent_event_driver, AgentEventConsumer, AgentEventConsumerControlFlow,
    AgentEventDriverConfig, AgentEventStreamClient, AgentEventStreamClientEventSource,
    AgentRunEvent, DisabledAgentEventStreamClient, MessageHydrator,
};
use anyhow::anyhow;
use async_trait::async_trait;
use futures::channel::mpsc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use warp_core::features::FeatureFlag;
use warp_multi_agent_api as api;
use warpui::r#async::Timer;
use warpui::{Entity, ModelContext, SingletonEntity};

/// How often (milliseconds) the drain timer checks for SSE events.
const SSE_DRAIN_INTERVAL_MS: u64 = 500;

/// Tracks messages awaiting server-side delivery confirmation.
struct PendingDeliveryConfirmation {
    message_ids: Vec<String>,
}

/// Per-event item delivered from the SSE background task to the entity.
struct SseStreamItem {
    event: AgentRunEvent,
    fetched_message: Option<ReceivedMessageInput>,
}

/// State for a single active SSE connection.
struct SseConnectionState {
    /// Receives parsed events from the background SSE task.
    event_receiver: mpsc::UnboundedReceiver<SseStreamItem>,
    /// Generation counter; used to discard stale callbacks after reconnect.
    generation: u64,
}

struct SseForwardingConsumer {
    tx: mpsc::UnboundedSender<SseStreamItem>,
    self_run_id: String,
    hydrator: MessageHydrator,
}

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
impl AgentEventConsumer for SseForwardingConsumer {
    async fn on_event(
        &mut self,
        event: AgentRunEvent,
    ) -> anyhow::Result<AgentEventConsumerControlFlow> {
        let fetched_message = self
            .hydrator
            .hydrate_event_for_recipient(&event, &self.self_run_id)
            .await;

        self.tx
            .unbounded_send(SseStreamItem {
                event,
                fetched_message,
            })
            .map_err(|_| anyhow!("SSE event receiver dropped"))?;

        Ok(AgentEventConsumerControlFlow::Continue)
    }
}

/// Async network coordinator for v2 orchestration event delivery via SSE.
/// Opens persistent SSE connections to the server and forwards events into
/// the OrchestrationEventService. Owns watched run_ids, event cursors used
/// for deduplication, lifecycle reporting, and delivery confirmation.
/// SSE retries with exponential backoff on failure.
pub struct OrchestrationEventStreamer {
    agent_event_stream_client: Arc<dyn AgentEventStreamClient>,
    watched_run_ids: HashMap<AIConversationId, HashSet<String>>,
    event_cursor: HashMap<AIConversationId, i64>,
    pending_delivery: HashMap<AIConversationId, PendingDeliveryConfirmation>,
    conversation_statuses: HashMap<AIConversationId, ConversationStatus>,
    /// Active SSE connections keyed by conversation.
    sse_connections: HashMap<AIConversationId, SseConnectionState>,
    /// Monotonic counter for SSE connection generations. Ensures stale
    /// callbacks from replaced connections are discarded.
    next_sse_generation: u64,
}

pub enum OrchestrationEventStreamerEvent {
    // Reserved for future use (e.g., status signals to the controller).
}

impl OrchestrationEventStreamer {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let agent_event_stream_client = Arc::new(DisabledAgentEventStreamClient);
        let history_model = BlocklistAIHistoryModel::handle(ctx);
        ctx.subscribe_to_model(&history_model, |me, event, ctx| {
            me.handle_history_event(event, ctx);
        });
        Self {
            agent_event_stream_client,
            watched_run_ids: HashMap::new(),
            event_cursor: HashMap::new(),
            pending_delivery: HashMap::new(),
            conversation_statuses: HashMap::new(),
            sse_connections: HashMap::new(),
            next_sse_generation: 0,
        }
    }

    /// 用显式传入的事件流客户端构造 streamer,避免测试里从 `ServerApiProvider` 查询。
    #[cfg(test)]
    pub(super) fn new_with_clients_for_test(
        agent_event_stream_client: Arc<dyn AgentEventStreamClient>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let history_model = BlocklistAIHistoryModel::handle(ctx);
        ctx.subscribe_to_model(&history_model, |me, event, ctx| {
            me.handle_history_event(event, ctx);
        });
        Self {
            agent_event_stream_client,
            watched_run_ids: HashMap::new(),
            event_cursor: HashMap::new(),
            pending_delivery: HashMap::new(),
            conversation_statuses: HashMap::new(),
            sse_connections: HashMap::new(),
            next_sse_generation: 0,
        }
    }

    /// Registers a run_id to watch for events on a conversation.
    /// Called by the start_agent executor for child run_ids and by
    /// self-registration for the conversation's own token.
    ///
    /// If SSE mode is active for this conversation, the current connection is
    /// torn down and a new one is opened with the updated run_id set.
    pub fn register_watched_run_id(
        &mut self,
        conversation_id: AIConversationId,
        run_id: String,
        ctx: &mut ModelContext<Self>,
    ) {
        self.watched_run_ids
            .entry(conversation_id)
            .or_default()
            .insert(run_id);

        // Reconnect SSE with the updated run_ids when a new child is spawned.
        if self.sse_connections.contains_key(&conversation_id) {
            self.reconnect_sse(conversation_id, ctx);
        }
    }

    fn handle_history_event(
        &mut self,
        event: &BlocklistAIHistoryEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        match event {
            BlocklistAIHistoryEvent::UpdatedConversationStatus {
                conversation_id,
                is_restored,
                ..
            } => {
                if !*is_restored {
                    self.on_conversation_status_updated(*conversation_id, ctx);
                }
            }
            BlocklistAIHistoryEvent::ConversationAgentIdAssigned {
                conversation_id, ..
            } => self.on_server_token_assigned(*conversation_id, ctx),
            BlocklistAIHistoryEvent::UpdatedStreamingExchange {
                conversation_id,
                exchange_id,
                ..
            } => self.on_streaming_exchange_updated(*conversation_id, *exchange_id, ctx),
            BlocklistAIHistoryEvent::RemoveConversation {
                conversation_id, ..
            }
            | BlocklistAIHistoryEvent::DeletedConversation {
                conversation_id, ..
            } => {
                self.watched_run_ids.remove(conversation_id);
                self.event_cursor.remove(conversation_id);
                self.pending_delivery.remove(conversation_id);
                self.conversation_statuses.remove(conversation_id);
                // Dropping the SSE connection state closes the channel,
                // causing the task's next send to fail and terminate.
                self.sse_connections.remove(conversation_id);
            }
            BlocklistAIHistoryEvent::StartedNewConversation { .. }
            | BlocklistAIHistoryEvent::CreatedSubtask { .. }
            | BlocklistAIHistoryEvent::UpgradedTask { .. }
            | BlocklistAIHistoryEvent::AppendedExchange { .. }
            | BlocklistAIHistoryEvent::ReassignedExchange { .. }
            | BlocklistAIHistoryEvent::SetActiveConversation { .. }
            | BlocklistAIHistoryEvent::ClearedActiveConversation { .. }
            | BlocklistAIHistoryEvent::ClearedConversationsInTerminalView { .. }
            | BlocklistAIHistoryEvent::UpdatedTodoList { .. }
            | BlocklistAIHistoryEvent::UpdatedAutoexecuteOverride { .. }
            | BlocklistAIHistoryEvent::SplitConversation { .. }
            | BlocklistAIHistoryEvent::UpdatedConversationMetadata { .. }
            | BlocklistAIHistoryEvent::UpdatedConversationArtifacts { .. } => {}
            BlocklistAIHistoryEvent::RestoredConversations {
                conversation_ids, ..
            } => {
                self.on_restored_conversations(conversation_ids.clone(), ctx);
            }
        }
    }

    /// Handles restoration of conversations on startup (or driver re-attach).
    ///
    /// Re-establishes orchestration event delivery state that is not persisted
    /// directly in memory: watched run_ids, the per-conversation event cursor,
    /// and — for `Success` parents with watched children — the SSE event loop.
    fn on_restored_conversations(
        &mut self,
        conversation_ids: Vec<AIConversationId>,
        ctx: &mut ModelContext<Self>,
    ) {
        // Orchestration v2 owns the events endpoints and the cursor model.
        // V1 conversations may carry a run_id but the v2-only event APIs
        // would return spurious 4xx responses, so skip restore entirely
        // when V2 is disabled.
        if !FeatureFlag::OrchestrationV2.is_enabled() {
            return;
        }

        for conv_id in conversation_ids {
            let (run_id, cursor, status, is_viewer, child_run_ids) = {
                let history_model = BlocklistAIHistoryModel::as_ref(ctx);
                let Some(conversation) = history_model.conversation(&conv_id) else {
                    continue;
                };
                let is_viewer = conversation.is_viewing_shared_session();
                let run_id = conversation.run_id();
                let cursor = conversation.last_event_sequence().unwrap_or(0);
                let status = conversation.status().clone();
                let child_run_ids = history_model
                    .child_conversations_of(conv_id)
                    .into_iter()
                    .filter(|child| !child.is_viewing_shared_session())
                    .filter_map(|child| child.run_id())
                    .collect::<Vec<_>>();
                (run_id, cursor, status, is_viewer, child_run_ids)
            };

            // Shared-session viewers receive updates through session sharing;
            // subscribing here would re-inject events the session has already
            // processed.
            if is_viewer {
                continue;
            }

            // OpenWarp:恢复后只使用本地 SQLite 持久化的 cursor,不再补取云端 task。
            self.event_cursor.insert(conv_id, cursor);
            self.conversation_statuses.insert(conv_id, status.clone());

            // 登记自身 run_id,用于过滤 self lifecycle events,并作为 SSE 订阅集合的基础。
            if let Some(ref own) = run_id {
                self.watched_run_ids
                    .entry(conv_id)
                    .or_default()
                    .insert(own.clone());
            }

            // 本地恢复已维护 parent→child 索引,从已恢复的 child conversation 收集 run_id。
            if !child_run_ids.is_empty() {
                let watched = self.watched_run_ids.entry(conv_id).or_default();
                for child_run_id in child_run_ids {
                    watched.insert(child_run_id);
                }
            }

            self.maybe_start_delivery_after_restore(conv_id, &status, ctx);
        }
    }

    /// Starts event delivery for a restored conversation if the parent is
    /// currently `Success` and has at least one watched run_id. `InProgress`
    /// parents are deferred to `on_conversation_status_updated` once they
    /// next transition to `Success`.
    fn maybe_start_delivery_after_restore(
        &mut self,
        conv_id: AIConversationId,
        status: &ConversationStatus,
        ctx: &mut ModelContext<Self>,
    ) {
        let has_watched = self
            .watched_run_ids
            .get(&conv_id)
            .is_some_and(|w| !w.is_empty());
        if !has_watched {
            return;
        }
        if matches!(status, ConversationStatus::Success) {
            self.start_event_delivery(conv_id, ctx);
        }
    }

    fn on_conversation_status_updated(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let (current_status, previous_status) = {
            let Some(conversation) =
                BlocklistAIHistoryModel::as_ref(ctx).conversation(&conversation_id)
            else {
                self.conversation_statuses.remove(&conversation_id);
                return;
            };
            let prev = self
                .conversation_statuses
                .insert(conversation_id, conversation.status().clone());
            (conversation.status().clone(), prev)
        };

        let became_success = matches!(&current_status, ConversationStatus::Success)
            && !matches!(previous_status.as_ref(), Some(ConversationStatus::Success));

        // Open an SSE stream when a conversation with watched run_ids
        // becomes idle.
        if became_success && self.watched_run_ids.contains_key(&conversation_id) {
            self.start_event_delivery(conversation_id, ctx);
        }
    }

    fn on_server_token_assigned(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let run_id = {
            let Some(conversation) =
                BlocklistAIHistoryModel::as_ref(ctx).conversation(&conversation_id)
            else {
                return;
            };
            // Shared session viewers must not subscribe to events — the
            // actual agent handles event delivery. Subscribing here would
            // re-inject events the session has already processed.
            if conversation.is_viewing_shared_session() {
                return;
            }
            let Some(run_id) = conversation.run_id() else {
                return;
            };
            run_id
        };
        self.register_watched_run_id(conversation_id, run_id, ctx);
    }

    fn on_streaming_exchange_updated(
        &mut self,
        conversation_id: AIConversationId,
        exchange_id: crate::ai::agent::AIAgentExchangeId,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(pending) = self.pending_delivery.get(&conversation_id) else {
            return;
        };

        let Some(conversation) =
            BlocklistAIHistoryModel::as_ref(ctx).conversation(&conversation_id)
        else {
            return;
        };
        let Some(exchange) = conversation.exchange_with_id(exchange_id) else {
            return;
        };

        // Check if the exchange output contains any of the messages we're
        // waiting to confirm.
        let pending_ids: HashSet<&str> = pending.message_ids.iter().map(String::as_str).collect();
        let mut confirmed_ids = Vec::new();
        if let Some(output) = exchange.output_status.output() {
            for msg in &output.get().messages {
                if let crate::ai::agent::AIAgentOutputMessageType::MessagesReceivedFromAgents {
                    messages,
                } = &msg.message
                {
                    for received in messages {
                        if pending_ids.contains(received.message_id.as_str()) {
                            confirmed_ids.push(received.message_id.clone());
                        }
                    }
                }
            }
        }

        if confirmed_ids.is_empty() {
            return;
        }

        // Remove confirmed messages from pending.
        if let Some(pending) = self.pending_delivery.get_mut(&conversation_id) {
            pending.message_ids.retain(|id| !confirmed_ids.contains(id));
            if pending.message_ids.is_empty() {
                self.pending_delivery.remove(&conversation_id);
            }
        }

        let hydrator = MessageHydrator::new();
        ctx.spawn(
            async move {
                hydrator
                    .mark_messages_delivered_best_effort(confirmed_ids.iter().map(String::as_str))
                    .await
            },
            |_, failures, _| {
                for (message_id, err) in failures {
                    log::warn!("Failed to confirm message delivery for {message_id}: {err:#}");
                }
            },
        );
    }

    /// Feeds a batch of fetched events through the OrchestrationEventService,
    /// updating the in-memory and persisted cursors and tracking message IDs
    /// awaiting delivery confirmation.
    fn handle_event_batch(
        &mut self,
        conversation_id: AIConversationId,
        self_run_id: &str,
        previous_cursor: i64,
        events: Vec<AgentRunEvent>,
        messages: Vec<ReceivedMessageInput>,
        ctx: &mut ModelContext<Self>,
    ) {
        let max_seq = events
            .iter()
            .map(|e| e.sequence)
            .max()
            .unwrap_or(previous_cursor);
        self.event_cursor.insert(conversation_id, max_seq);

        // Persist the cursor to SQLite so that after a restart we can resume
        // event delivery from this sequence number without re-delivering
        // events the parent has already acted on.
        BlocklistAIHistoryModel::handle(ctx).update(ctx, |model, ctx| {
            model.update_event_sequence(conversation_id, max_seq, ctx);
        });

        // Track message IDs for server-side mark_delivered calls.
        let message_ids: Vec<String> = events
            .iter()
            .filter(|e| e.event_type == "new_message" && e.run_id == self_run_id)
            .filter_map(|e| e.ref_id.clone())
            .collect();
        if !message_ids.is_empty() {
            self.pending_delivery
                .entry(conversation_id)
                .or_insert_with(|| PendingDeliveryConfirmation {
                    message_ids: Vec::new(),
                })
                .message_ids
                .extend(message_ids);
        }

        let lifecycle_events = convert_lifecycle_events(&events, self_run_id);
        if messages.is_empty() && lifecycle_events.is_empty() {
            return;
        }

        let pending = build_pending_events(messages, lifecycle_events);
        OrchestrationEventService::handle(ctx).update(ctx, |svc, ctx| {
            svc.enqueue_event_batch(conversation_id, pending, ctx);
        });
    }

    /// Opens an SSE connection for the given conversation if one isn't already active.
    fn start_event_delivery(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        if !self.sse_connections.contains_key(&conversation_id) {
            self.start_sse_connection(conversation_id, ctx);
        }
    }

    /// Opens a long-lived SSE connection for `conversation_id`. Events are
    /// sent through an mpsc channel and drained by a periodic timer.
    fn start_sse_connection(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(watched) = self.watched_run_ids.get(&conversation_id) else {
            return;
        };
        if watched.is_empty() {
            return;
        }

        let watched: Vec<String> = watched.iter().cloned().collect();
        let cursor = self
            .event_cursor
            .get(&conversation_id)
            .copied()
            .unwrap_or(0);

        let agent_event_stream_client = self.agent_event_stream_client.clone();
        let self_run_id = BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&conversation_id)
            .and_then(|c| c.run_id())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let (tx, rx) = mpsc::unbounded();
        let generation = self.next_sse_generation;
        self.next_sse_generation += 1;

        self.sse_connections.insert(
            conversation_id,
            SseConnectionState {
                event_receiver: rx,
                generation,
            },
        );

        log::info!(
            "Opening SSE stream for {conversation_id:?} (gen={generation}, \
             run_ids={watched:?}, since={cursor})"
        );

        let config = AgentEventDriverConfig::retry_forever(watched.clone(), cursor);
        let source = AgentEventStreamClientEventSource::new(agent_event_stream_client);
        let hydrator = MessageHydrator::new();

        ctx.spawn(
            async move {
                let mut consumer = SseForwardingConsumer {
                    tx,
                    self_run_id,
                    hydrator,
                };
                run_agent_event_driver(source, config, &mut consumer).await
            },
            move |me, result, ctx| {
                let is_current = me
                    .sse_connections
                    .get(&conversation_id)
                    .is_some_and(|s| s.generation == generation);
                if !is_current {
                    return;
                }

                me.drain_sse_events(conversation_id, ctx);

                if let Err(err) = result {
                    log::warn!(
                        "SSE driver exited for {conversation_id:?} (gen={generation}): {err:#}"
                    );
                    me.reconnect_sse(conversation_id, ctx);
                }
            },
        );

        // Start periodic event drain.
        self.start_sse_drain_timer(conversation_id, generation, ctx);
    }

    /// Periodically fires to drain buffered SSE events into the event service.
    fn start_sse_drain_timer(
        &self,
        conversation_id: AIConversationId,
        generation: u64,
        ctx: &mut ModelContext<Self>,
    ) {
        ctx.spawn(
            async move {
                Timer::after(Duration::from_millis(SSE_DRAIN_INTERVAL_MS)).await;
            },
            move |me, _, ctx| {
                let is_current = me
                    .sse_connections
                    .get(&conversation_id)
                    .is_some_and(|s| s.generation == generation);
                if !is_current {
                    return;
                }
                me.drain_sse_events(conversation_id, ctx);
                me.start_sse_drain_timer(conversation_id, generation, ctx);
            },
        );
    }

    /// Drains all buffered SSE events and feeds them through the
    /// `handle_event_batch` sink.
    fn drain_sse_events(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(sse) = self.sse_connections.get_mut(&conversation_id) else {
            return;
        };

        let cursor = self
            .event_cursor
            .get(&conversation_id)
            .copied()
            .unwrap_or(0);

        let mut events = Vec::new();
        let mut messages = Vec::new();

        while let Ok(item) = sse.event_receiver.try_recv() {
            // Deduplicate: discard events at or below the cursor.
            if item.event.sequence > cursor {
                if let Some(msg) = item.fetched_message {
                    messages.push(msg);
                }
                events.push(item.event);
            }
        }

        if events.is_empty() {
            return;
        }

        let self_run_id = BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&conversation_id)
            .and_then(|c| c.run_id())
            .map(|s| s.to_string())
            .unwrap_or_default();

        self.handle_event_batch(conversation_id, &self_run_id, cursor, events, messages, ctx);
    }

    /// Tears down the current SSE connection and opens a new one with the
    /// latest watched run_ids and cursor.
    fn reconnect_sse(&mut self, conversation_id: AIConversationId, ctx: &mut ModelContext<Self>) {
        // Drain buffered events before dropping the channel so we don't
        // discard already-fetched message bodies.
        self.drain_sse_events(conversation_id, ctx);
        self.sse_connections.remove(&conversation_id);

        if self.watched_run_ids.contains_key(&conversation_id) {
            self.start_sse_connection(conversation_id, ctx);
        }
    }
}

impl Entity for OrchestrationEventStreamer {
    type Event = OrchestrationEventStreamerEvent;
}

impl SingletonEntity for OrchestrationEventStreamer {}

fn parse_occurred_at(s: &str) -> prost_types::Timestamp {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| prost_types::Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        })
        .unwrap_or_else(|_| {
            let now = chrono::Utc::now();
            prost_types::Timestamp {
                seconds: now.timestamp(),
                nanos: now.timestamp_subsec_nanos() as i32,
            }
        })
}

fn convert_lifecycle_events(events: &[AgentRunEvent], self_run_id: &str) -> Vec<api::AgentEvent> {
    events
        .iter()
        .filter(|e| e.event_type != "new_message" && e.run_id != self_run_id)
        .filter_map(|event| {
            let lifecycle_type = match event.event_type.as_str() {
                // New canonical event types aligned with task states.
                "run_in_progress" => api::LifecycleEventType::InProgress,
                "run_succeeded" => api::LifecycleEventType::Succeeded,
                "run_failed" => api::LifecycleEventType::Failed,
                // Legacy event types mapped to new variants for backward compat.
                #[allow(deprecated)]
                "run_started" => api::LifecycleEventType::InProgress,
                #[allow(deprecated)]
                "run_idle" => api::LifecycleEventType::Succeeded,
                #[allow(deprecated)]
                "run_restarted" => api::LifecycleEventType::InProgress,
                "run_errored" => api::LifecycleEventType::Errored,
                "run_cancelled" => api::LifecycleEventType::Cancelled,
                "run_blocked" => api::LifecycleEventType::Blocked,
                _ => return None,
            };
            let timestamp = parse_occurred_at(&event.occurred_at);
            // TODO: Parse richer detail payloads (reason, error_message) from
            // the server event log once the schema supports them.
            let detail = match lifecycle_type {
                api::LifecycleEventType::Errored => {
                    super::orchestration_events::LifecycleEventDetailPayload {
                        stage: Some(
                            super::orchestration_events::LifecycleEventDetailStage::Runtime,
                        ),
                        reason: event.ref_id.clone(),
                        ..Default::default()
                    }
                }
                _ => super::orchestration_events::LifecycleEventDetailPayload::default(),
            };
            let event_id = Uuid::new_v4().to_string();
            Some(super::orchestration_events::build_lifecycle_event(
                event_id,
                event.run_id.clone(),
                lifecycle_type,
                timestamp,
                &detail,
            ))
        })
        .collect()
}

fn build_pending_events(
    messages: Vec<ReceivedMessageInput>,
    lifecycle_events: Vec<api::AgentEvent>,
) -> Vec<PendingEvent> {
    let mut pending = Vec::with_capacity(messages.len() + lifecycle_events.len());
    for msg in &messages {
        pending.push(PendingEvent {
            event_id: msg.message_id.clone(),
            source_agent_id: msg.sender_agent_id.clone(),
            attempt_count: 0,
            detail: PendingEventDetail::Message {
                message_id: msg.message_id.clone(),
                addresses: msg.addresses.clone(),
                subject: msg.subject.clone(),
                message_body: msg.message_body.clone(),
            },
        });
    }
    for event in lifecycle_events {
        pending.push(PendingEvent {
            event_id: event.event_id.clone(),
            source_agent_id: String::new(),
            attempt_count: 0,
            detail: PendingEventDetail::Lifecycle { event },
        });
    }
    pending
}

#[cfg(test)]
#[path = "orchestration_event_streamer_tests.rs"]
mod tests;
