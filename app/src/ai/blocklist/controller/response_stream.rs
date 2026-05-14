use std::{cell::RefCell, rc::Rc, sync::Arc};

use crate::ai::api_error::AIApiError;
use anyhow::anyhow;
use chrono::{DateTime, Local, TimeDelta};
use futures::channel::oneshot;
use futures_util::StreamExt;
use uuid::Uuid;
use warp_multi_agent_api::response_event;
use warpui::{Entity, ModelContext};

use crate::{
    ai::agent::{
        api::{self, ConvertToAPITypeError},
        conversation::AIConversationId,
        AIAgentInput, AIIdentifiers, CancellationReason,
    },
    ai::blocklist::BlocklistAIHistoryModel,
    network::NetworkStatus,
    report_error, send_telemetry_from_ctx,
};
use warpui::SingletonEntity;

/// BYOP 路径的请求分流参数。从 LLMId、settings、conversation 中提取后
/// 一次性塞给 spawn closure(ctx 不能跨 await 边界)。
pub(super) struct PendingTitleGeneration {
    pub(super) input: crate::ai::agent_providers::chat_stream::TitleGenInput,
    pub(super) user_query: String,
    pub(super) task_id: String,
}

struct ByopDispatch {
    base_url: String,
    api_key: String,
    model_id: String,
    /// 显式指定的 API 协议类型,chat_stream 据此映射 genai AdapterKind。
    api_type: crate::settings::AgentProviderApiType,
    /// Provider 级 reasoning effort 偏好。`Auto` 时不向 genai 传 effort,
    /// 由 adapter 自己按模型名后缀推断;非 Auto 经 client capability gate 后注入。
    reasoning_effort: crate::settings::ReasoningEffortSetting,
    extra_headers: Vec<(String, String)>,
    /// conversation 的 root task id — 必须用本地已注册的 id,
    /// 否则下游 `Action::AddMessagesToTask` 在 task_store 找不到会 `TaskNotFound`。
    root_task_id: String,
    /// 本轮模型输出应该写入的 task id。普通对话等于 root task;CLI subagent 后续轮为 subtask。
    target_task_id: String,
    /// 是否需要 emit `CreateTask` 把 Optimistic root 升级为 Server task。
    /// 仅首轮(root task 还没 source)需要;再次发会触发 `UnexpectedUpgrade`。
    needs_create_task: bool,
    /// 标题生成模型参数。仅在首轮(needs_create_task)且 active title_model
    /// 解码为合法 BYOP id 时填充;否则不启动后台标题生成。
    title_gen: Option<TitleGenParams>,
    /// LRC 场景绑定的 `command_id`(= LRC block id 字符串)。
    lrc_command_id: Option<String>,
    /// 是否需要在 chat_stream 中合成 subagent CreateTask 来升级 optimistic CLI subtask。
    lrc_should_spawn_subagent: bool,
    /// 选中模型的上下文窗口(tokens)。0/None ⇒ 用户未填且 catalog 也无,
    /// chat_stream 跳过 context_window_usage 计算,UI 维持 100% 占位。
    context_window: Option<u32>,
}

/// 标题生成专用的 BYOP 配置(可能与主 base 模型同 provider 也可能不同)。
pub(crate) struct TitleGenParams {
    pub base_url: String,
    pub api_key: String,
    pub model_id: String,
    pub api_type: crate::settings::AgentProviderApiType,
    pub reasoning_effort: crate::settings::ReasoningEffortSetting,
}

fn byop_dispatch_info(
    params: &api::RequestParams,
    ai_identifiers: &AIIdentifiers,
    ctx: &warpui::AppContext,
) -> Option<ByopDispatch> {
    let (provider, api_key, model_id) =
        crate::ai::agent_providers::lookup_byop(ctx, &params.model)?;
    let extra_headers = provider.extra_headers.clone();
    // 从 provider.models 里找当前模型条目,取其 context_window(tokens)。
    // 0 视为未填,后续走 None 分支 ⇒ chat_stream 不算占用率。
    let context_window = provider
        .models
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.context_window)
        .filter(|n| *n > 0);
    let conversation_id = ai_identifiers.client_conversation_id.as_ref()?;
    let history = BlocklistAIHistoryModel::as_ref(ctx);
    let conversation = history.conversation(conversation_id)?;
    let root_task_id = conversation.get_root_task_id().to_string();
    let target_task_id = params
        .byop_target_task_id
        .clone()
        .unwrap_or_else(|| root_task_id.clone());
    // compute_active_tasks 只返回 `task.source().is_some()` 的 task —
    // 因此非空 ⇒ root 已经升级为 Server 状态,不要再 emit CreateTask。
    let needs_create_task = conversation.compute_active_tasks().is_empty();

    // 标题生成:只在首轮触发(避免每轮重复打标题)。
    // 解析 active title_model:可能是 base_model 自己,也可能是用户独立选的另一个 BYOP 模型。
    // 任一模型不是 BYOP 编码(比如 fallback 到非 BYOP 默认),则跳过 — OpenWarp 主路径都是 BYOP,
    // 实际 fallback 到 base 时,base 自己就是 BYOP。
    let llm_prefs = crate::ai::llms::LLMPreferences::as_ref(ctx);
    let title_gen = if needs_create_task {
        let title_id = llm_prefs.get_active_title_model(ctx, None).id.clone();
        crate::ai::agent_providers::lookup_byop(ctx, &title_id).map(
            |(t_provider, t_api_key, t_model_id)| {
                let t_effort =
                    llm_prefs.get_reasoning_effort(None, t_provider.api_type, &t_model_id);
                TitleGenParams {
                    base_url: t_provider.base_url,
                    api_key: t_api_key,
                    model_id: t_model_id,
                    api_type: t_provider.api_type,
                    reasoning_effort: t_effort,
                }
            },
        )
    } else {
        None
    };

    let reasoning_effort = llm_prefs.get_reasoning_effort(None, provider.api_type, &model_id);
    Some(ByopDispatch {
        base_url: provider.base_url,
        api_key,
        model_id,
        api_type: provider.api_type,
        reasoning_effort,
        extra_headers,
        root_task_id,
        target_task_id,
        needs_create_task,
        title_gen,
        lrc_command_id: params.lrc_command_id.clone(),
        lrc_should_spawn_subagent: params.lrc_should_spawn_subagent,
        context_window,
    })
}

fn pending_title_generation_from_byop(
    params: &api::RequestParams,
    byop: &ByopDispatch,
) -> Option<PendingTitleGeneration> {
    let title_gen = byop.title_gen.as_ref()?;
    let user_query = params.input.iter().find_map(|input| {
        if let AIAgentInput::UserQuery { query, .. } = input {
            Some(query.clone())
        } else {
            None
        }
    })?;

    Some(PendingTitleGeneration {
        input: crate::ai::agent_providers::chat_stream::TitleGenInput {
            base_url: title_gen.base_url.clone(),
            api_key: title_gen.api_key.clone(),
            model_id: title_gen.model_id.clone(),
            api_type: title_gen.api_type,
            reasoning_effort: title_gen.reasoning_effort,
        },
        user_query,
        task_id: byop.root_task_id.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResponseStreamId(String);

impl ResponseStreamId {
    pub fn for_shared_session(init_event: &response_event::StreamInit) -> Self {
        // Make the stream ID unique per viewing by appending a local UUID
        // This prevents collisions when replaying the same conversation multiple times
        // (either on close-and-reopen or when viewing the same shared session from multiple terminals)
        Self(format!("{}-{}", init_event.request_id, Uuid::new_v4()))
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// Model wrapping an agent API response stream.
///
/// Emits events when the output corresponding to the stream is updated, typically after receiving
/// each response chunk.
///
/// Handles retries internally - retries are only attempted if no ClientActions events have been
/// received yet, ensuring we don't retry after the AI has started executing actions.
pub struct ResponseStream {
    id: ResponseStreamId,
    params: api::RequestParams,
    retry_count: usize,
    start_time: DateTime<Local>,
    time_to_latest_event: TimeDelta,
    cancellation_tx: Option<oneshot::Sender<()>>,
    /// Store the original error for telemetry when retries succeed
    original_error: Option<String>,
    /// Track whether we've received any client actions
    /// If true, we cannot retry on subsequent errors since actions may have been executed
    has_received_client_actions: bool,
    /// AI identifiers for telemetry emission
    ai_identifiers: AIIdentifiers,

    /// Whether this request can attempt to resume the conversation on error.
    /// This is true for all requests except those that are themselves the result of a resume
    /// triggered by a previous error.
    can_attempt_resume_on_error: bool,

    pending_title_generation: Option<PendingTitleGeneration>,

    /// Whether we should attempt to resume the conversation after the stream finishes.
    ///
    /// This is set when we receive a retryable error after client actions have been received
    /// and `can_attempt_resume_on_error` is true.
    should_resume_conversation_after_stream_finished: bool,

    /// Unique, internal id for the current request.
    ///
    /// This ensures that the model never emits events for a request that was already cancelled (or
    /// retried) and is still receiving lagging events.
    ///
    /// Note this is unique compared to `id`; this is unique across retry requests while the response
    /// stream id remains stable.
    current_request_id: Option<Uuid>,
}

impl ResponseStream {
    pub fn new(
        params: api::RequestParams,
        ai_identifiers: AIIdentifiers,
        can_attempt_resume_on_error: bool,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let (cancellation_tx, cancellation_rx) = oneshot::channel();
        let start_time = Local::now();

        let request_id = Uuid::new_v4();
        let params_clone = params.clone();
        // BYOP 路径: 若选中的 base model 是用户自定义 provider 编码的 LLMId,
        // 则在 spawn 前从 ctx 中取出 (provider, api_key, model_id, root_task_id),
        // 走自定义 chat completions。否则走 warp 自家 multi-agent 端点(原有路径)。
        let byop_dispatch = byop_dispatch_info(&params, &ai_identifiers, ctx);
        let pending_title_generation = byop_dispatch
            .as_ref()
            .and_then(|byop| pending_title_generation_from_byop(&params, byop));
        let _ = ctx.spawn(
            async move {
                if let Some(byop) = byop_dispatch {
                    crate::ai::agent_providers::chat_stream::generate_byop_output(
                        crate::ai::agent_providers::chat_stream::ByopOutputInput {
                            params: params_clone,
                            base_url: byop.base_url,
                            api_key: byop.api_key,
                            model_id: byop.model_id,
                            api_type: byop.api_type,
                            reasoning_effort: byop.reasoning_effort,
                            extra_headers: byop.extra_headers,
                            task_id: byop.root_task_id,
                            target_task_id: byop.target_task_id,
                            needs_create_task: byop.needs_create_task,
                            lrc_command_id: byop.lrc_command_id,
                            lrc_should_spawn_subagent: byop.lrc_should_spawn_subagent,
                            context_window: byop.context_window,
                            cancellation_rx,
                        },
                    )
                    .await
                } else {
                    byop_required_response_stream(cancellation_rx).await
                }
            },
            move |me, stream, ctx| {
                me.handle_response_stream_result(request_id, stream, ctx);
            },
        );
        Self {
            id: ResponseStreamId(Uuid::new_v4().to_string()),
            params: params.clone(),
            start_time,
            time_to_latest_event: TimeDelta::seconds(0),
            cancellation_tx: Some(cancellation_tx),
            retry_count: 0,
            original_error: None,
            has_received_client_actions: false,
            ai_identifiers,
            can_attempt_resume_on_error,
            pending_title_generation,
            should_resume_conversation_after_stream_finished: false,
            current_request_id: Some(request_id),
        }
    }

    pub(super) fn take_pending_title_generation(&mut self) -> Option<PendingTitleGeneration> {
        self.pending_title_generation.take()
    }

    pub fn id(&self) -> &ResponseStreamId {
        &self.id
    }

    pub fn is_lrc_tag_in_request(&self) -> bool {
        self.params.lrc_should_spawn_subagent
    }

    /// OpenWarp BYOP 本地会话压缩:返回本流是否在跑 SummarizeConversation,
    /// 以及 overflow 标记。controller 在 handle_response_stream_finished 的
    /// Done 分支据此调 commit_summarization 把摘要落到 conversation.compaction_state。
    pub fn summarization_overflow(&self) -> Option<bool> {
        self.params.input.iter().find_map(|input| match input {
            crate::ai::agent::AIAgentInput::SummarizeConversation { overflow, .. } => {
                Some(*overflow)
            }
            _ => None,
        })
    }

    /// Returns true if we should attempt to resume the conversation after the stream finishes.
    pub fn should_resume_conversation_after_stream_finished(&self) -> bool {
        self.should_resume_conversation_after_stream_finished
    }

    /// Helper function to emit AgentModeError telemetry for error that is retryable (not user visible).
    fn emit_retryable_agent_mode_error_telemetry(
        &self,
        error: String,
        ctx: &mut ModelContext<Self>,
    ) {
        send_telemetry_from_ctx!(
            crate::TelemetryEvent::AgentModeError {
                identifiers: self.ai_identifiers.clone(),
                error,
                is_user_visible: false,
                will_attempt_to_resume: false,
            },
            ctx
        );
    }

    fn retry(&mut self, ctx: &mut ModelContext<Self>) {
        self.retry_count += 1;
        self.has_received_client_actions = false; // Reset for the new attempt

        let (cancellation_tx, cancellation_rx) = oneshot::channel();
        if let Some(old_cancellation_tx) = self.cancellation_tx.take() {
            let _ = old_cancellation_tx.send(());
        }
        self.cancellation_tx = Some(cancellation_tx);

        let request_id = Uuid::new_v4();
        self.current_request_id = Some(request_id);
        let params = self.params.clone();
        let byop_dispatch = byop_dispatch_info(&params, &self.ai_identifiers, ctx);
        let _ = ctx.spawn(
            async move {
                if let Some(byop) = byop_dispatch {
                    crate::ai::agent_providers::chat_stream::generate_byop_output(
                        crate::ai::agent_providers::chat_stream::ByopOutputInput {
                            params,
                            base_url: byop.base_url,
                            api_key: byop.api_key,
                            model_id: byop.model_id,
                            api_type: byop.api_type,
                            reasoning_effort: byop.reasoning_effort,
                            extra_headers: byop.extra_headers,
                            task_id: byop.root_task_id,
                            target_task_id: byop.target_task_id,
                            needs_create_task: byop.needs_create_task,
                            lrc_command_id: byop.lrc_command_id,
                            lrc_should_spawn_subagent: byop.lrc_should_spawn_subagent,
                            context_window: byop.context_window,
                            cancellation_rx,
                        },
                    )
                    .await
                } else {
                    byop_required_response_stream(cancellation_rx).await
                }
            },
            move |me, stream, ctx| {
                me.handle_response_stream_result(request_id, stream, ctx);
            },
        );
    }

    /// Cancels the stream. The conversation_id is preserved in the emitted event for async handling.
    pub(super) fn cancel(
        &mut self,
        reason: CancellationReason,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.current_request_id = None;
        let Some(cancellation_tx) = self.cancellation_tx.take() else {
            return;
        };
        let _ = cancellation_tx.send(());
        ctx.emit(ResponseStreamEvent::AfterStreamFinished {
            cancellation: Some(StreamCancellation {
                reason,
                conversation_id,
            }),
        });
    }

    fn handle_response_stream_result(
        &mut self,
        request_id: Uuid,
        stream_result: Result<api::ResponseStream, ConvertToAPITypeError>,
        ctx: &mut ModelContext<Self>,
    ) {
        match stream_result {
            Ok(stream) => {
                ctx.spawn_stream_local(
                    stream,
                    move |me, event, ctx| {
                        me.handle_response_stream_event(request_id, event, ctx);
                    },
                    move |me, ctx| {
                        me.on_response_stream_complete(request_id, ctx);
                    },
                );
            }
            Err(e) => {
                log::error!("Failed to send request to multi-agent API: {e:?}");
                self.on_response_stream_complete(request_id, ctx);
            }
        }
    }

    fn handle_response_stream_event(
        &mut self,
        request_id: Uuid,
        event: api::Event,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.current_request_id.is_none_or(|id| id != request_id) {
            return;
        }
        self.time_to_latest_event = Local::now().signed_duration_since(self.start_time);

        match &event {
            Ok(response_event) => {
                if let Some(event_type) = &response_event.r#type {
                    match event_type {
                        warp_multi_agent_api::response_event::Type::Init(init_event) => {
                            // Capture server_output_id from StreamInit event
                            self.ai_identifiers.server_output_id =
                                Some(crate::ai::agent::ServerOutputId::new(
                                    init_event.request_id.clone(),
                                ));
                        }
                        warp_multi_agent_api::response_event::Type::ClientActions(_) => {
                            // Mark that we've received client actions
                            self.has_received_client_actions = true;
                        }
                        warp_multi_agent_api::response_event::Type::Finished(finished_event) => {
                            // Emit retry success telemetry on successful completion
                            if matches!(
                                finished_event.reason,
                                Some(warp_multi_agent_api::response_event::stream_finished::Reason::Done(_)) | None
                            ) {
                                // Emit retry success telemetry if this was a successful completion after retries
                                if self.retry_count > 0 {
                                    if let Some(original_error) = &self.original_error {
                                        send_telemetry_from_ctx!(
                                            crate::TelemetryEvent::AgentModeRequestRetrySucceeded {
                                                identifiers: self.ai_identifiers.clone(),
                                                retry_count: self.retry_count,
                                                original_error: original_error.clone(),
                                            },
                                            ctx
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                ctx.emit(ResponseStreamEvent::ReceivedEvent(Consumable::new(event)));
            }
            Err(e) => {
                // Store original error if this is the first error
                if self.retry_count == 0 {
                    self.original_error = Some(format!("{e:?}"));
                }

                // Only retry if:
                // 1. We haven't received any client actions yet (this is the first event or only init events)
                // 2. The error is retryable
                // 3. We haven't exceeded max retries
                // 4. We're online
                const MAX_RETRIES: usize = 3;
                let network_status = NetworkStatus::as_ref(ctx);
                let is_online = network_status.is_online();
                let is_retryable = e.is_retryable();

                let should_retry = !self.has_received_client_actions
                    && is_retryable
                    && self.retry_count < MAX_RETRIES
                    && is_online;

                if should_retry {
                    log::warn!(
                        "MultiAgent request failed, retrying (attempt {}/{}) - Error: {e:?}",
                        self.retry_count + 1,
                        MAX_RETRIES
                    );
                    // Only emit error telemetry here if we're retrying.
                    // Final errors that aren't being retried are emitted elsewhere.
                    self.emit_retryable_agent_mode_error_telemetry(format!("{e:?}"), ctx);
                    self.retry(ctx);
                    // Don't emit the error event, we're retrying
                    // TODO: emit a separate event if controller needs to know about failures that are being retried
                    return;
                }

                // If we can't retry (because client actions were received) but the error is
                // retryable and we're allowed to attempt a resume, signal that the controller
                // should resume the conversation after the stream completes.
                let should_attempt_resume = self.has_received_client_actions
                    && is_retryable
                    && self.can_attempt_resume_on_error;
                if should_attempt_resume {
                    self.should_resume_conversation_after_stream_finished = true;
                }

                log::warn!(
                    "MultiAgent request failed after {} retries: has_received_client_actions={}, is_retryable={}, is_online={is_online}",
                    self.retry_count,
                    self.has_received_client_actions,
                    e.is_retryable()
                );
                report_error!(anyhow!(e.clone()).context(format!(
                    "MultiAgent request failed after {} retries",
                    self.retry_count
                )));

                ctx.emit(ResponseStreamEvent::ReceivedEvent(Consumable::new(event)));
            }
        }
    }

    fn on_response_stream_complete(&mut self, request_id: Uuid, ctx: &mut ModelContext<Self>) {
        if self.current_request_id.is_none_or(|id| id != request_id) {
            return;
        }
        ctx.emit(ResponseStreamEvent::AfterStreamFinished { cancellation: None });
        self.cancellation_tx = None;
    }
}

#[derive(Debug)]
pub struct Consumable<T> {
    value: Rc<RefCell<Option<T>>>,
}

impl<T> Consumable<T> {
    fn new(value: T) -> Self {
        Consumable {
            value: Rc::new(RefCell::new(Some(value))),
        }
    }

    pub(super) fn consume(&self) -> Option<T> {
        self.value.borrow_mut().take()
    }
}

impl<T> Clone for Consumable<T> {
    fn clone(&self) -> Self {
        Consumable {
            value: Rc::clone(&self.value),
        }
    }
}

/// Cancellation context preserved for async event handling.
/// Includes conversation_id because truncation can remove exchange mappings before the event is processed.
#[derive(Debug, Clone)]
pub struct StreamCancellation {
    pub reason: CancellationReason,
    pub conversation_id: AIConversationId,
}

#[derive(Debug, Clone)]
pub enum ResponseStreamEvent {
    ReceivedEvent(Consumable<api::Event>),
    AfterStreamFinished {
        /// Some for cancellation (with context), None for natural completion (uses dynamic lookup).
        cancellation: Option<StreamCancellation>,
    },
}

impl Entity for ResponseStream {
    type Event = ResponseStreamEvent;
}

async fn byop_required_response_stream(
    cancellation_rx: oneshot::Receiver<()>,
) -> Result<api::ResponseStream, ConvertToAPITypeError> {
    log::debug!("No BYOP provider selected for OpenWarp agent request");
    let error_stream = futures::stream::once(async {
        Err(Arc::new(AIApiError::Other(anyhow!(
            "OpenWarp requires a configured BYOP provider in Settings"
        ))))
    })
    .take_until(cancellation_rx);
    Ok(Box::pin(error_stream))
}
