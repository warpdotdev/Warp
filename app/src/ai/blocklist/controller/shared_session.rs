// BlocklistAIController 的共享会话本地展示逻辑。
use itertools::Itertools;
use warp_multi_agent_api::response_event::ClientActions;
use warp_multi_agent_api::{client_action::Action, message::Message};

use super::response_stream::ResponseStreamId;
use super::{BlocklistAIController, RequestInput};
use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::agent::AIAgentActionId;
use crate::ai::blocklist::agent_view::AgentViewEntryOrigin;
use crate::ai::blocklist::history_model::BlocklistAIHistoryModel;
use crate::terminal::shared_session::ParticipantId;
use warpui::{AppContext, ModelContext, SingletonEntity};

#[derive(Default)]
pub(super) struct SharedSessionState {
    // The current active request id for the shared session (used if subsequent events do not provide a request id)
    current_response_id: Option<ResponseStreamId>,
    // The participant who initiated the current response stream
    current_response_initiator: Option<ParticipantId>,
    // The sharer's participant ID (set when session sharing starts)
    sharer_participant_id: Option<ParticipantId>,
}

impl BlocklistAIController {
    /// Returns the current conversation ID for the active shared session stream.
    /// Returns None if there's no active shared session conversation.
    pub(crate) fn get_current_shared_session_conversation_id(
        &self,
        app: &AppContext,
    ) -> Option<AIConversationId> {
        self.shared_session_state
            .current_response_id
            .as_ref()
            .and_then(|response_id| {
                BlocklistAIHistoryModel::as_ref(app).conversation_for_response_stream(response_id)
            })
    }

    /// Apply agent session events to the current conversation state.
    pub fn handle_shared_session_response_event(
        &mut self,
        resp: warp_multi_agent_api::ResponseEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(kind) = resp.r#type else {
            return;
        };
        match kind {
            warp_multi_agent_api::response_event::Type::Init(init) => {
                self.on_shared_init(init, ctx)
            }
            warp_multi_agent_api::response_event::Type::ClientActions(actions) => {
                self.on_shared_client_actions(actions, ctx)
            }
            warp_multi_agent_api::response_event::Type::Finished(finished) => {
                self.on_shared_finished(finished, ctx);
            }
        }
    }

    fn on_shared_init(
        &mut self,
        init_event: warp_multi_agent_api::response_event::StreamInit,
        ctx: &mut ModelContext<Self>,
    ) {
        let stream_id = ResponseStreamId::for_shared_session(&init_event);
        self.shared_session_state.current_response_id = Some(stream_id.clone());
        let terminal_view_id = self.terminal_view_id;
        let history = BlocklistAIHistoryModel::handle(ctx);

        // 如果共享会话 conversation token 已经绑定到本地对话,直接复用。
        // 否则,当当前 agent view 对话为空时复用该本地对话 ID,并绑定传入的共享 token。
        // 这样可以保留当前 agent view 里已创建终端 block 的可见性。
        let conversation_id = self
            .find_existing_conversation_by_shared_token(&init_event.conversation_id, ctx)
            .or_else(|| {
                let selected_conversation_id = self
                    .context_model
                    .as_ref(ctx)
                    .selected_conversation_id(ctx)?;

                // If the current agent view's conversation is completely empty,
                // we should just associate it with the incoming request/token.
                let should_reuse_selected_conversation = history
                    .as_ref(ctx)
                    .conversation(&selected_conversation_id)
                    .is_some_and(|conversation| {
                        conversation.exchange_count() == 0
                            && conversation.server_conversation_token().is_none()
                    });
                if !should_reuse_selected_conversation {
                    return None;
                }

                history.update(ctx, |history, ctx| {
                    history.set_server_conversation_token_for_conversation(
                        selected_conversation_id,
                        init_event.conversation_id.clone(),
                    );
                    history.set_viewing_shared_session_for_conversation(
                        selected_conversation_id,
                        true,
                    );
                    ctx.notify();
                });

                Some(selected_conversation_id)
            })
            .unwrap_or_else(|| {
                history.update(ctx, |h, ctx| {
                    h.start_new_conversation(terminal_view_id, false, true, ctx)
                })
            });

        let Some(conversation) = history.as_ref(ctx).conversation(&conversation_id) else {
            log::error!(
                "Tried to initialize shared session stream for non-existent conversation  {conversation_id:?}"
            );
            return;
        };
        let task_id = conversation.get_root_task_id().clone();

        // Ensure the action executor is in view-only mode for shared-session viewers.
        self.action_model.update(ctx, |action_model, _ctx| {
            action_model.set_view_only(true);
        });

        // Eagerly create an exchange for this request (with empty inputs) and initialize output.
        history.update(ctx, |history_model, ctx| {
            let _ = history_model.update_conversation_for_new_request_input(
                RequestInput::for_task(
                    vec![],
                    task_id,
                    &self.active_session,
                    self.get_current_response_initiator(),
                    conversation_id,
                    self.terminal_view_id,
                    ctx,
                ),
                stream_id.clone(),
                self.terminal_view_id,
                ctx,
            );

            history_model.initialize_output_for_response_stream(
                &stream_id,
                conversation_id,
                self.terminal_view_id,
                init_event.clone(),
                ctx,
            );

            // Mark conversation as in progress and active/selected
            history_model.update_conversation_status(
                self.terminal_view_id,
                conversation_id,
                ConversationStatus::InProgress,
                ctx,
            );
            history_model.set_active_conversation_id(conversation_id, self.terminal_view_id, ctx);
        });
        self.context_model.update(ctx, |context_model, ctx| {
            context_model.set_pending_query_state_for_existing_conversation(
                conversation_id,
                AgentViewEntryOrigin::SharedSessionSelection,
                ctx,
            );
        });
    }

    fn on_shared_client_actions(
        &mut self,
        actions: warp_multi_agent_api::response_event::ClientActions,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(stream_id) = self.shared_session_state.current_response_id.clone() else {
            log::warn!("Received shared session client actions with no active response stream id.");
            return;
        };

        let Some(conversation_id) =
            BlocklistAIHistoryModel::as_ref(ctx).conversation_for_response_stream(&stream_id)
        else {
            log::warn!(
                "No conversation ID for shared session response stream with id={stream_id:?}"
            );
            return;
        };

        self.update_directory_context_from_client_actions(&actions, ctx);
        let history_model = BlocklistAIHistoryModel::handle(ctx);
        history_model.update(ctx, |history_model, ctx| {
            if let Err(e) = history_model.apply_client_actions(
                &stream_id,
                actions.actions,
                conversation_id,
                self.terminal_view_id,
                ctx,
            ) {
                log::error!(
                    "Failed to apply client actions to conversation for shared session: {e:?}"
                );
            }
        });
        let Some(conversation) = history_model.as_ref(ctx).conversation(&conversation_id) else {
            log::error!("Failed to find conversation with id: {conversation_id:?}");
            return;
        };

        let new_action_results_to_apply = conversation
            .new_exchange_ids_for_response(&stream_id)
            .filter_map(|exchange_id| conversation.exchange_with_id(exchange_id))
            .flat_map(|exchange| {
                exchange
                    .input
                    .iter()
                    .filter_map(|i| i.action_result().cloned())
            })
            .collect_vec();

        // Apply finished results to unfinished actions.
        for result in new_action_results_to_apply.into_iter() {
            if self
                .action_model
                .as_ref(ctx)
                .get_action_result(&result.id)
                .is_none()
            {
                self.action_model.update(ctx, |action_model, ctx| {
                    action_model.apply_finished_action_result(conversation_id, result, ctx);
                });
            }
        }
    }

    /// Update the context model's working directory context using the most recent message context.
    fn update_directory_context_from_client_actions(
        &mut self,
        actions: &ClientActions,
        ctx: &mut ModelContext<Self>,
    ) {
        for client_action in &actions.actions {
            if let Some(Action::AddMessagesToTask(add)) = &client_action.action {
                for message in &add.messages {
                    if let Some(inner) = &message.message {
                        let ctx_opt = match inner {
                            Message::UserQuery(uq) => uq.context.as_ref(),
                            Message::SystemQuery(sq) => sq.context.as_ref(),
                            Message::ToolCallResult(tcr) => tcr.context.as_ref(),
                            _ => None,
                        };

                        if let Some(input_ctx) = ctx_opt {
                            if let Some(dir) = &input_ctx.directory {
                                self.context_model.update(ctx, |context_model, ctx| {
                                    context_model.update_directory_context(
                                        if dir.pwd.is_empty() {
                                            None
                                        } else {
                                            Some(dir.pwd.clone())
                                        },
                                        if dir.home.is_empty() {
                                            None
                                        } else {
                                            Some(dir.home.clone())
                                        },
                                        ctx,
                                    );
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    fn on_shared_finished(
        &mut self,
        finished: warp_multi_agent_api::response_event::StreamFinished,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(stream_id) = self.shared_session_state.current_response_id.take() else {
            log::warn!("Shared Finished missing request_id");
            return;
        };
        let Some(conversation_id) =
            BlocklistAIHistoryModel::as_ref(ctx).conversation_for_response_stream(&stream_id)
        else {
            log::warn!(
                "No conversation ID for shared session response stream with id={stream_id:?}"
            );
            return;
        };

        let history_model = BlocklistAIHistoryModel::handle(ctx);
        let Some(conversation) = history_model.as_ref(ctx).conversation(&conversation_id) else {
            log::error!("Failed to find conversation with id: {conversation_id:?}");
            return;
        };

        // Queue actions for viewer UI in view-only mode
        let mut actions_to_queue = vec![];
        let mut did_exchange_contain_user_query = false;

        for new_exchange_id in conversation.new_exchange_ids_for_response(&stream_id) {
            let Some(exchange) = conversation.exchange_with_id(new_exchange_id) else {
                continue;
            };
            did_exchange_contain_user_query |=
                exchange.input.iter().any(|input| input.is_user_query());

            if let Some(output) = exchange.output_status.output() {
                actions_to_queue.extend(output.get().actions().cloned().collect_vec().into_iter());
            }
        }

        if !actions_to_queue.is_empty() {
            self.action_model.update(ctx, |action_model, ctx| {
                action_model.queue_actions(actions_to_queue, conversation_id, ctx);
            });
        }

        self.handle_response_stream_finished(
            &stream_id,
            finished,
            conversation_id,
            did_exchange_contain_user_query,
            // shared session 路径不会触发本地压缩(来自远端 viewer 同步流),始终 None
            None,
            ctx,
        );
    }

    /// 查找与共享会话 conversation token 对应的本地对话。
    pub fn find_existing_conversation_by_shared_token(
        &self,
        conversation_token: &str,
        ctx: &mut ModelContext<Self>,
    ) -> Option<AIConversationId> {
        let history = BlocklistAIHistoryModel::handle(ctx);
        history
            .as_ref(ctx)
            .all_live_conversations_for_terminal_view(self.terminal_view_id)
            .find_map(|conv| {
                conv.server_conversation_token()
                    .and_then(|t| (t.as_str() == conversation_token).then_some(conv.id()))
            })
    }

    /// Marks an action as remotely executing when a viewer receives a CommandExecutionStarted event.
    /// This allows the viewer's UI to show the action as running rather than queued.
    pub fn mark_action_as_remotely_executing_in_shared_session(
        &mut self,
        action_id: &AIAgentActionId,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.action_model.update(ctx, |action_model, ctx| {
            action_model.mark_action_as_remotely_executing(action_id, conversation_id, ctx);
        });
    }

    /// Sets the participant ID for the current response.
    /// This should be called when initiating a query to track who sent it.
    pub fn set_current_response_initiator(&mut self, participant_id: ParticipantId) {
        self.shared_session_state.current_response_initiator = Some(participant_id);
    }

    /// Gets the participant ID for the current response.
    pub(super) fn get_current_response_initiator(&self) -> Option<ParticipantId> {
        self.shared_session_state.current_response_initiator.clone()
    }

    /// Sets the sharer's participant ID. Should be called when a shared session is created.
    pub fn set_sharer_participant_id(&mut self, participant_id: ParticipantId) {
        self.shared_session_state.sharer_participant_id = Some(participant_id);
    }

    /// Gets the sharer's participant ID.
    pub(super) fn get_sharer_participant_id(&self) -> Option<ParticipantId> {
        self.shared_session_state.sharer_participant_id.clone()
    }

    /// Links a forked conversation's new token to an existing conversation.
    /// This is called on the viewer side when receiving a response for a forked conversation
    /// so that new responses are added to the correct conversation.
    pub fn link_forked_conversation_token(
        &mut self,
        forked_from_token: &str,
        event: &warp_multi_agent_api::ResponseEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        // 从 StreamInit 事件取出新的共享会话 conversation id。
        let new_conversation_id = match &event.r#type {
            Some(warp_multi_agent_api::response_event::Type::Init(init)) => {
                init.conversation_id.as_str()
            }
            // Only StreamInit events have conversation_id.
            _ => return,
        };

        // Find the conversation with the forked_from token
        if let Some(conversation_id) =
            self.find_existing_conversation_by_shared_token(forked_from_token, ctx)
        {
            // Update the conversation's server_conversation_token to the new one
            BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                history.set_server_conversation_token_for_conversation(
                    conversation_id,
                    new_conversation_id.to_string(),
                );
                ctx.notify();
            });
        }
    }
}
