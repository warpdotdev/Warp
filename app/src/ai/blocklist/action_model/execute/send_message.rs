use futures::{future::BoxFuture, FutureExt};
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::agent::{
    AIAgentAction, AIAgentActionResultType, AIAgentActionType, SendMessageToAgentResult,
};
use crate::ai::blocklist::history_model::BlocklistAIHistoryModel;
use crate::ai::blocklist::orchestration_events::{OrchestrationEventService, SendMessageResult};
use crate::ai::blocklist::telemetry::{
    BlocklistOrchestrationTelemetryEvent, TeamAgentCommunicationFailedEvent,
    TeamAgentCommunicationFailureReason, TeamAgentCommunicationKind,
    TeamAgentCommunicationTransport, TeamAgentOrchestrationVersion,
};
use crate::server::server_api::ai::SendAgentMessageRequest;
use crate::server::server_api::ServerApiProvider;
use warp_core::features::FeatureFlag;
use warp_core::send_telemetry_from_ctx;

use super::{ActionExecution, AnyActionExecution, ExecuteActionInput, PreprocessActionInput};

pub struct SendMessageToAgentExecutor;

impl SendMessageToAgentExecutor {
    pub fn new() -> Self {
        Self
    }

    pub(super) fn should_autoexecute(
        &self,
        _input: ExecuteActionInput,
        _ctx: &mut ModelContext<Self>,
    ) -> bool {
        true
    }

    pub(super) fn execute(
        &mut self,
        input: ExecuteActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> AnyActionExecution {
        let AIAgentAction {
            action:
                AIAgentActionType::SendMessageToAgent {
                    addresses,
                    subject,
                    message,
                },
            ..
        } = input.action
        else {
            return ActionExecution::<()>::InvalidAction.into();
        };

        let conversation_id = input.conversation_id;
        let addresses = addresses.clone();
        let subject = subject.clone();
        let message_body = message.clone();

        if FeatureFlag::OrchestrationV2.is_enabled() {
            let sender_run_id = BlocklistAIHistoryModel::as_ref(ctx)
                .conversation(&conversation_id)
                .and_then(|c| c.run_id())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let log_addresses = addresses.clone();
            let log_subject = subject.clone();
            let log_sender_run_id = sender_run_id.clone();
            let ai_client = ServerApiProvider::as_ref(ctx).get_ai_client();
            let request = SendAgentMessageRequest {
                to: addresses,
                subject,
                body: message_body,
                sender_run_id,
            };
            return ActionExecution::new_async(
                async move { ai_client.send_agent_message(request).await },
                move |result, ctx| match result {
                    Ok(response) => {
                        let message_id =
                            response.message_ids.into_iter().next().unwrap_or_default();
                        AIAgentActionResultType::SendMessageToAgent(
                            SendMessageToAgentResult::Success { message_id },
                        )
                    }
                    Err(err) => {
                        let error_message = err.to_string();
                        send_telemetry_from_ctx!(
                            BlocklistOrchestrationTelemetryEvent::TeamAgentCommunicationFailed(
                                TeamAgentCommunicationFailedEvent {
                                    communication_kind: TeamAgentCommunicationKind::Message,
                                    transport: TeamAgentCommunicationTransport::ServerApi,
                                    orchestration_version: TeamAgentOrchestrationVersion::V2,
                                    failure_reason:
                                        TeamAgentCommunicationFailureReason::RequestFailed,
                                    source_conversation_id: conversation_id,
                                    source_run_id: (!log_sender_run_id.is_empty())
                                        .then(|| log_sender_run_id.clone()),
                                    target_count: Some(log_addresses.len()),
                                    lifecycle_event_type: None,
                                    error_message: Some(error_message.clone()),
                                }
                            ),
                            ctx
                        );
                        log::warn!(
                            "Failed to send child-agent message via server API: conversation_id={conversation_id:?} sender_run_id={log_sender_run_id:?} target_agent_ids={log_addresses:?} subject={log_subject:?} error={err:#}"
                        );
                        AIAgentActionResultType::SendMessageToAgent(
                            SendMessageToAgentResult::Error(error_message),
                        )
                    }
                },
            )
            .into();
        }

        let result = OrchestrationEventService::handle(ctx).update(ctx, |svc, ctx| {
            svc.send_message(conversation_id, &addresses, subject, message_body, ctx)
        });
        let result = match result {
            SendMessageResult::MessageSent { message_id } => {
                SendMessageToAgentResult::Success { message_id }
            }
            SendMessageResult::Error(error) => SendMessageToAgentResult::Error(error),
        };

        ActionExecution::<()>::Sync(AIAgentActionResultType::SendMessageToAgent(result)).into()
    }

    pub(super) fn preprocess_action(
        &mut self,
        _action: PreprocessActionInput,
        _ctx: &mut ModelContext<Self>,
    ) -> BoxFuture<'static, ()> {
        futures::future::ready(()).boxed()
    }
}

impl Default for SendMessageToAgentExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for SendMessageToAgentExecutor {
    type Event = ();
}
