use std::{collections::HashMap, sync::Arc};

use crate::server::telemetry::{CLISubagentControlState, TelemetryEvent};
use instant::Instant;
use parking_lot::FairMutex;
use serde::{Deserialize, Serialize};
use warp_core::send_telemetry_from_ctx;
use warpui::{Entity, EntityId, ModelContext, ModelHandle, SingletonEntity};

use crate::ai::blocklist::context_model::block_context_from_terminal_model;
use crate::{
    ai::{
        agent::{
            conversation::AIConversationId, task::TaskId, AIAgentActionId, AIAgentActionResultType,
            AIAgentContext, CancellationReason, ReadShellCommandOutputResult,
            RequestCommandOutputResult, TransferShellCommandControlToUserResult,
            WriteToLongRunningShellCommandResult,
        },
        blocklist::{
            agent_view::{AgentViewController, AgentViewEntryOrigin},
            BlocklistAIActionEvent, BlocklistAIActionModel, BlocklistAIController,
            BlocklistAIHistoryEvent,
        },
    },
    terminal::{
        model::block::BlockId,
        model_events::{ModelEvent, ModelEventDispatcher},
        TerminalModel,
    },
    BlocklistAIHistoryModel,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserTakeOverReason {
    Manual,
    Stop,
    /// The agent explicitly transferred control to the user via the
    /// TransferShellCommandControlToUser tool call.
    TransferFromAgent {
        /// The reason the agent gave for transferring control.
        reason: String,
    },
}

#[derive(Debug, Clone, Default)]
struct ActiveCLISubagentState {
    task_id: Option<TaskId>,
    last_snapshot_at: Option<Instant>,
}

impl UserTakeOverReason {
    pub fn is_stop(&self) -> bool {
        matches!(self, Self::Stop)
    }

    pub fn is_transfer_from_agent(&self) -> bool {
        matches!(self, Self::TransferFromAgent { .. })
    }

    pub fn transfer_reason(&self) -> Option<&str> {
        match self {
            Self::TransferFromAgent { reason } => Some(reason.as_str()),
            _ => None,
        }
    }
}

/// Represents which party is in control of the active long running command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LongRunningCommandControlState {
    /// The agent is in control.
    ///
    /// When the agent has control, the user cannot submit input to the command.
    Agent {
        /// `true` if the agent is blocked on approval from the user for submitting input.
        is_blocked: bool,
        /// `true` if agent responses should be hidden in the UI.
        should_hide_responses: bool,
    },
    /// The user is in control.
    User { reason: UserTakeOverReason },
}

impl LongRunningCommandControlState {
    pub fn is_agent_in_control(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }

    pub fn is_agent_blocked(&self) -> bool {
        matches!(
            self,
            Self::Agent {
                is_blocked: true,
                ..
            }
        )
    }

    pub fn is_user_in_control(&self) -> bool {
        matches!(self, Self::User { .. })
    }

    pub fn should_hide_responses(&self) -> bool {
        matches!(
            self,
            Self::Agent {
                should_hide_responses: true,
                ..
            }
        )
    }

    pub fn user_take_over_reason(&self) -> Option<&UserTakeOverReason> {
        match &self {
            LongRunningCommandControlState::Agent { .. } => None,
            LongRunningCommandControlState::User { reason } => Some(reason),
        }
    }
}

/// Responsible for managing 'control' (e.g. write permissions) for the active long running
/// agent-requested command.
///
/// Control state is canonically stored on the relevant command `Block` owned by terminal model,
/// but wrapping update APIs in this controller ensures consistent update semantics and makes
/// control state updates subscribable.
pub struct CLISubagentController {
    controller: ModelHandle<BlocklistAIController>,
    action_model: ModelHandle<BlocklistAIActionModel>,
    agent_view_controller: Option<ModelHandle<AgentViewController>>,
    terminal_model: Arc<FairMutex<TerminalModel>>,
    terminal_view_id: EntityId,
    // Active or recently-active CLI subagent state, keyed by the associated block.
    active_subagents_by_block: HashMap<BlockId, ActiveCLISubagentState>,
}

impl CLISubagentController {
    pub fn new(
        controller: &ModelHandle<BlocklistAIController>,
        action_model: &ModelHandle<BlocklistAIActionModel>,
        agent_view_controller: Option<ModelHandle<AgentViewController>>,
        terminal_model: Arc<FairMutex<TerminalModel>>,
        model_event_dispatcher: &ModelHandle<ModelEventDispatcher>,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let history_model = BlocklistAIHistoryModel::handle(ctx);
        ctx.subscribe_to_model(&history_model, Self::handle_history_model_event);

        ctx.subscribe_to_model(action_model, |me, event, ctx| match event {
            BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(_) => {
                let mut terminal_model = me.terminal_model.lock();
                let active_block = terminal_model.block_list_mut().active_block_mut();
                active_block.update_is_agent_blocked(true);

                let action_id = active_block.requested_command_action_id().cloned();
                ctx.emit(CLISubagentEvent::UpdatedControl {
                    block_id: active_block.id().clone(),
                    requested_command_action_id: action_id,
                    agent_has_control: active_block.is_agent_in_control(),
                });
            }
            BlocklistAIActionEvent::ExecutingAction(..) => {
                let mut terminal_model = me.terminal_model.lock();
                let active_block = terminal_model.block_list_mut().active_block_mut();
                active_block.update_is_agent_blocked(false);

                let action_id = active_block.requested_command_action_id().cloned();
                ctx.emit(CLISubagentEvent::UpdatedControl {
                    block_id: active_block.id().clone(),
                    requested_command_action_id: action_id,
                    agent_has_control: active_block.is_agent_in_control(),
                });
            }
            BlocklistAIActionEvent::FinishedAction { action_id, .. } => {
                let snapshot_block_id = me
                    .action_model
                    .as_ref(ctx)
                    .get_action_result(action_id)
                    .and_then(|result| snapshot_block_id_for_action_result(&result.result))
                    .cloned();

                // OpenWarp BYOP fallback: agent 自起 LRC 后,上游 server 路径会推
                // `BlocklistAIHistoryEvent::CreatedSubtask` 触发
                // `handle_history_model_event` 把 block 升级为 monitored 态;BYOP 没有
                // 这条 server 事件源,如果不补,active_block 永远停在
                // `Agent { long_running_control_state: None }` —— `is_agent_in_control()`
                // / `is_agent_monitoring()` / `is_agent_tagged_in()` 全 false,命中
                // `view.rs:6841-6853` `is_input_box_visible` 长命令分支的兜底 false 路径,
                // 输入框消失;同时 `controller.rs:1112-1119` 分支读
                // `subagent_task_id`,fallback 若不真实创建 conversation task,下一轮
                // query 路由失败,触发"Could not find conversation for response stream"。
                //
                // 实现走 silent 路径:`create_silent_cli_subagent_task_for_conversation`
                // 真实创建 subtask 但不 emit `CreatedSubtask`(原方法 emit 时机由调用方
                // 控制),让本 hook 在升级 block 后再手动 emit。这样可以走完整的上游
                // 升级链路并创建 SubagentView 浮窗,且配合 `cli.rs:431-446` 的 root_task
                // 兜底,task 暂无 exchange 时 view 用 root last_exchange 占位创建,后续
                // 用户 follow-up query 路由到 subtask 触发 `AppendedExchange`,view 自身
                // 订阅(`cli.rs:355-399`)会自动 replace model 到真实 exchange。
                //
                // 副作用:浮窗创建瞬间用 root last_exchange 渲染,可能短暂闪现 root 内容
                // 一帧;但 `set_agent_interaction_mode_for_agent_monitored_command` 升级
                // 后 SubagentView 的过滤会让其只显示 task_id 关联内容,体验上和上游一致。
                let upgrade_target = snapshot_block_id.as_ref().and_then(|block_id| {
                    let terminal_model = me.terminal_model.lock();
                    let block = terminal_model.block_list().block_with_id(block_id)?;
                    if !block.is_agent_driving_command()
                        || block.long_running_control_state().is_some()
                    {
                        return None;
                    }
                    let conversation_id = block.ai_conversation_id()?;
                    Some((block_id.clone(), conversation_id))
                });

                let upgraded_task_id =
                    if let Some((block_id, conversation_id)) = upgrade_target.as_ref() {
                        let history_model = BlocklistAIHistoryModel::handle(ctx);
                        let block_id_for_create = block_id.clone();
                        let conversation_id = *conversation_id;
                        match history_model.update(ctx, |history_model, _| {
                            history_model.create_silent_cli_subagent_task_for_conversation(
                                block_id_for_create,
                                conversation_id,
                            )
                        }) {
                            Ok(task_id) => {
                                log::info!(
                                    "[byop] BYOP LRC monitor fallback: silent subtask created \
                                 block={block_id:?} task={task_id:?} \
                                 conversation={conversation_id:?}"
                                );
                                Some(task_id)
                            }
                            Err(e) => {
                                log::error!(
                                    "[byop] BYOP LRC monitor fallback create_silent_subagent_task \
                                 failed: {e:?}"
                                );
                                None
                            }
                        }
                    } else {
                        None
                    };

                let mut terminal_model = me.terminal_model.lock();
                let mut emit_spawn_event_for: Option<(
                    BlockId,
                    TaskId,
                    AIConversationId,
                    Option<AIAgentActionId>,
                )> = None;
                if let (Some((block_id, conversation_id)), Some(task_id)) =
                    (upgrade_target.as_ref(), upgraded_task_id.as_ref())
                {
                    if let Some(block) = terminal_model.block_list_mut().mut_block_from_id(block_id)
                    {
                        match block.set_agent_interaction_mode_for_agent_monitored_command(
                            task_id,
                            *conversation_id,
                        ) {
                            Ok(()) => {
                                let action_id = block.requested_command_action_id().cloned();
                                emit_spawn_event_for = Some((
                                    block_id.clone(),
                                    task_id.clone(),
                                    *conversation_id,
                                    action_id,
                                ));
                            }
                            Err(e) => {
                                log::error!(
                                    "[byop] BYOP LRC monitor fallback: \
                                     set_agent_interaction_mode_for_agent_monitored_command \
                                     failed: {e:?}"
                                );
                            }
                        }
                    }
                }
                let active_block = terminal_model.block_list_mut().active_block_mut();
                active_block.update_is_agent_blocked(false);

                let action_id = active_block.requested_command_action_id().cloned();
                let updated_control_block_id = active_block.id().clone();
                let updated_control_agent_has_control = active_block.is_agent_in_control();
                drop(terminal_model);

                ctx.emit(CLISubagentEvent::UpdatedControl {
                    block_id: updated_control_block_id,
                    requested_command_action_id: action_id,
                    agent_has_control: updated_control_agent_has_control,
                });

                // Updates the last snapshot timestamp for the active block after the agent has read the block output.
                if let Some(snapshot_block_id) = snapshot_block_id {
                    me.active_subagents_by_block
                        .entry(snapshot_block_id.clone())
                        .or_default()
                        .last_snapshot_at = Some(Instant::now());
                    ctx.emit(CLISubagentEvent::UpdatedLastSnapshot);
                }

                // OpenWarp BYOP: silent_create_for_byop 不 emit CreatedSubtask,这里手动
                // 触发 SpawnedSubagent 让 terminal_view 创建 CLISubagentView 浮窗。
                // active_subagents_by_block.task_id 同步更新,确保 BlockCompleted 钩子在
                // LRC 结束时正确清理。
                //
                // 注意:terminal_model 锁已在上面 drop,避免之前 reproduce 的死锁(emit
                // SpawnedSubagent → terminal_view::handle_cli_subagent_controller_event
                // → CLISubagentView::new 同步内部还会再 lock terminal_model 之类导致
                // FairMutex 重入)。
                if let Some((block_id, task_id, conversation_id, action_id)) = emit_spawn_event_for
                {
                    me.active_subagents_by_block
                        .entry(block_id.clone())
                        .or_default()
                        .task_id = Some(task_id.clone());
                    log::info!(
                        "[byop] BYOP LRC monitor fallback: emit SpawnedSubagent \
                         block={block_id:?} task={task_id:?}"
                    );
                    ctx.emit(CLISubagentEvent::SpawnedSubagent {
                        task_id,
                        conversation_id,
                        block_id,
                        initial_requested_command_action_id: action_id,
                    });
                }
            }
            _ => (),
        });

        ctx.subscribe_to_model(model_event_dispatcher, |me, event, ctx| {
            if let ModelEvent::BlockCompleted(block_completed_event) = event {
                let terminal_model = me.terminal_model.lock();
                let Some(block) = terminal_model
                    .block_list()
                    .block_with_id(&block_completed_event.block_id)
                else {
                    return;
                };

                let block_id = block.id().clone();
                let conversation_id = block.ai_conversation_id();
                let requested_command_action_id = block.requested_command_action_id().cloned();
                let was_agent_tagged_in = block.interaction_mode().is_agent_tagged_in();
                let has_agent_metadata = block.agent_interaction_metadata().is_some();
                drop(terminal_model);
                let removed_subagent_state = me.active_subagents_by_block.remove(&block_id);
                if removed_subagent_state
                    .as_ref()
                    .is_some_and(|state| state.last_snapshot_at.is_some())
                {
                    ctx.emit(CLISubagentEvent::UpdatedLastSnapshot);
                }

                if removed_subagent_state
                    .as_ref()
                    .is_some_and(|state| state.task_id.is_some())
                {
                    let is_inline_agent_view =
                        me.agent_view_controller.as_ref().is_some_and(|controller| {
                            controller.read(ctx, |controller, _| controller.is_inline())
                        });

                    if is_inline_agent_view {
                        // Mark conversation as successfully completed BEFORE exiting agent view.
                        // The command finished naturally, so this is a successful completion.
                        if let Some(conversation_id) = conversation_id {
                            me.controller.update(ctx, |controller, ctx| {
                                controller.cancel_conversation_progress(
                                    conversation_id,
                                    CancellationReason::OptimisticCLISubagentCompletion,
                                    ctx,
                                );
                            });
                        }
                    }

                    ctx.emit(CLISubagentEvent::FinishedSubagent {
                        block_id,
                        conversation_id,
                        initial_requested_command_action_id: requested_command_action_id,
                    });
                }

                // Exit inline agent view if agent was tagged in or had metadata (was in control).
                if let Some(agent_view_controller) = &me.agent_view_controller {
                    agent_view_controller.update(ctx, |controller, ctx| {
                        if controller.is_inline() && (was_agent_tagged_in || has_agent_metadata) {
                            controller.exit_agent_view(ctx);
                        }
                    });
                }
            }
        });

        Self {
            controller: controller.clone(),
            action_model: action_model.clone(),
            agent_view_controller,
            terminal_model,
            terminal_view_id,
            active_subagents_by_block: HashMap::new(),
        }
    }

    pub fn is_agent_in_control(&self) -> bool {
        let terminal_model = self.terminal_model.lock();
        terminal_model
            .block_list()
            .active_block()
            .is_agent_in_control()
    }

    pub(crate) fn is_agent_in_control_or_tagged_in(&self) -> bool {
        let terminal_model = self.terminal_model.lock();
        terminal_model
            .block_list()
            .active_block()
            .is_agent_in_control_or_tagged_in()
    }

    pub fn last_snapshot_at(&self, block_id: &BlockId) -> Option<Instant> {
        self.active_subagents_by_block
            .get(block_id)
            .and_then(|state| state.last_snapshot_at)
    }

    /// Force the currently in-flight poll for the given long-running command block to
    /// resolve immediately with a fresh snapshot, bypassing the agent-set timeout.
    /// Backs the `Check now` affordance surfaced next to the `Last seen by agent ...`
    /// indicator in the warping footer.
    pub fn request_force_refresh(&self, block_id: &BlockId, ctx: &mut ModelContext<Self>) {
        let executor_handle = self.action_model.as_ref(ctx).shell_command_executor(ctx);
        let block_id = block_id.clone();
        executor_handle.update(ctx, move |executor, _| {
            executor.force_refresh_block(&block_id);
        });
    }

    pub fn switch_control_to_user(&self, reason: UserTakeOverReason, ctx: &mut ModelContext<Self>) {
        let should_cancel_conversation = !reason.is_transfer_from_agent();
        let mut terminal_model = self.terminal_model.lock();

        let active_block = terminal_model.block_list_mut().active_block_mut();
        let block_id = active_block.id().clone();
        if let Err(e) = active_block.take_over_control_for_user(reason) {
            log::error!("Failed to take control for user: {e:?}");
            return;
        }

        let action_id = active_block.requested_command_action_id().cloned();
        let conversation_id = active_block.ai_conversation_id();
        let agent_has_control = active_block.is_agent_in_control();
        // Conversation cancellation potentially takes a lock on terminal model if the
        // cancelled action is a shell command action, so we have to drop the terminal
        // model lock before actually cancelling the conversation.
        drop(terminal_model);

        // Only cancel conversation if user manually took control (not when agent transfers control).
        if should_cancel_conversation {
            if let Some(conversation_id) = conversation_id {
                self.controller.update(ctx, |controller, ctx| {
                    controller.cancel_conversation_progress(
                        conversation_id,
                        CancellationReason::ManuallyCancelled,
                        ctx,
                    );
                });
            }
        }

        ctx.emit(CLISubagentEvent::UpdatedControl {
            block_id: block_id.clone(),
            requested_command_action_id: action_id,
            agent_has_control,
        });

        send_telemetry_from_ctx!(
            TelemetryEvent::CLISubagentControlStateChanged {
                conversation_id,
                block_id,
                control_state: CLISubagentControlState::UserInControl,
            },
            ctx
        );
    }

    pub fn handoff_active_command_control_to_agent(&self, ctx: &mut ModelContext<Self>) {
        let mut terminal_model = self.terminal_model.lock();

        let active_block = terminal_model.block_list_mut().active_block_mut();
        let conversation_id = active_block.ai_conversation_id();
        let block_id = active_block.id().clone();
        // Check if control was transferred from agent before handoff.
        let was_transfer_from_agent = active_block
            .long_running_control_state()
            .and_then(|state| state.user_take_over_reason())
            .is_some_and(|reason| reason.is_transfer_from_agent());
        if let Err(e) = active_block.handoff_control_to_agent() {
            log::error!("Failed to handoff control to agent: {e:?}");
            return;
        }
        let action_id = active_block.requested_command_action_id().cloned();
        let agent_has_control = active_block.is_agent_in_control();
        drop(terminal_model);
        if let Some(agent_view_controller) = &self.agent_view_controller {
            agent_view_controller.update(ctx, |controller, ctx| {
                if !controller.is_inline() {
                    if let Err(e) = controller.try_enter_inline_agent_view(
                        conversation_id,
                        AgentViewEntryOrigin::LongRunningCommand,
                        ctx,
                    ) {
                        log::error!("Failed to enter inline agent view for LRC handoff: {e}");
                    }
                }
            });
        }

        // Trigger an auto-resume of the conversation when handing control to the agent.
        if let Some(conversation_id) = conversation_id {
            let is_viewing_shared_session = BlocklistAIHistoryModel::as_ref(ctx)
                .conversation(&conversation_id)
                .is_some_and(|conversation| conversation.is_viewing_shared_session());
            if !is_viewing_shared_session {
                let resume_context = {
                    let terminal_model = self.terminal_model.lock();
                    block_context_from_terminal_model(&terminal_model, &block_id, false)
                        .map(Box::new)
                        .map(AIAgentContext::Block)
                        .into_iter()
                        .collect()
                };
                self.controller.update(ctx, |controller, ctx| {
                    controller.resume_conversation(
                        conversation_id,
                        /*can_attempt_resume_on_error*/ true,
                        /*is_auto_resume_after_error*/ false,
                        resume_context,
                        ctx,
                    );
                });
            }
        }

        ctx.emit(CLISubagentEvent::UpdatedControl {
            block_id: block_id.clone(),
            requested_command_action_id: action_id,
            agent_has_control,
        });

        // Emit a special event if control was transferred from agent, so the executor can be notified.
        if was_transfer_from_agent {
            ctx.emit(CLISubagentEvent::ControlHandedBackAfterTransfer);
        }

        send_telemetry_from_ctx!(
            TelemetryEvent::CLISubagentControlStateChanged {
                conversation_id,
                block_id,
                control_state: CLISubagentControlState::AgentInControl,
            },
            ctx
        );
    }

    pub fn toggle_hide_responses(&self, ctx: &mut ModelContext<Self>) {
        let mut terminal_model = self.terminal_model.lock();
        let active_block = terminal_model.block_list_mut().active_block_mut();

        if active_block.toggle_subagent_response_visibility() {
            let conversation_id = active_block.ai_conversation_id();
            let block_id = active_block.id().clone();
            let is_hidden = active_block.should_hide_responses();

            ctx.emit(CLISubagentEvent::ToggledHideResponses);

            if let Some(conversation_id) = conversation_id {
                send_telemetry_from_ctx!(
                    TelemetryEvent::CLISubagentResponsesToggled {
                        conversation_id,
                        block_id,
                        is_hidden,
                    },
                    ctx
                );
            }
        }
    }

    fn handle_history_model_event(
        &mut self,
        event: &BlocklistAIHistoryEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        if event
            .terminal_view_id()
            .is_some_and(|id| id != self.terminal_view_id)
        {
            return;
        }
        match event {
            BlocklistAIHistoryEvent::CreatedSubtask {
                task_id,
                conversation_id,
                ..
            } => {
                let history_model = BlocklistAIHistoryModel::handle(ctx);
                let Some(cli_subagent_block_id) = history_model
                    .as_ref(ctx)
                    .conversation(conversation_id)
                    .and_then(|c| c.get_task(task_id))
                    .and_then(|task| task.cli_subagent_block_id())
                else {
                    return;
                };

                let mut terminal_model = self.terminal_model.lock();
                let Some(block) = terminal_model
                    .block_list_mut()
                    .mut_block_from_id(&cli_subagent_block_id)
                else {
                    return;
                };
                let block_id = block.id().clone();
                if let Err(e) = block.set_agent_interaction_mode_for_agent_monitored_command(
                    task_id,
                    *conversation_id,
                ) {
                    log::error!("Could not update interaction mode to agent-monitored: {e:?}",);
                    return;
                };

                let action_id = block.requested_command_action_id().cloned();
                let agent_has_control = block.is_agent_in_control();
                drop(terminal_model);

                // When the CLI subagent is first created for a long running command,
                // the agent now has control. Emit an UpdatedControl event so that
                // shared-session state can reflect this initial control state.
                ctx.emit(CLISubagentEvent::UpdatedControl {
                    block_id: block_id.clone(),
                    requested_command_action_id: action_id.clone(),
                    agent_has_control,
                });
                self.active_subagents_by_block
                    .entry(block_id.clone())
                    .or_default()
                    .task_id = Some(task_id.clone());

                ctx.emit(CLISubagentEvent::SpawnedSubagent {
                    task_id: task_id.clone(),
                    conversation_id: *conversation_id,
                    block_id: block_id.clone(),
                    initial_requested_command_action_id: action_id,
                });
            }
            BlocklistAIHistoryEvent::UpgradedTask {
                optimistic_id: old_id,
                confirmed_task_id: new_id,
                ..
            } => {
                let block_id =
                    self.active_subagents_by_block
                        .iter()
                        .find_map(|(block_id, state)| {
                            (state.task_id.as_ref() == Some(old_id)).then_some(block_id.clone())
                        });
                if let Some(block_id) = block_id {
                    let mut terminal_model = self.terminal_model.lock();
                    if let Some(block) =
                        terminal_model.block_list_mut().mut_block_from_id(&block_id)
                    {
                        match block.upgrade_cli_subagent_task_id(new_id.clone()) {
                            Ok(()) => {
                                if let Some(state) =
                                    self.active_subagents_by_block.get_mut(&block_id)
                                {
                                    state.task_id = Some(new_id.clone());
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "Tried to upgrade CLISubagent task ID for non-existent block: {e:?}"
                                );
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CLISubagentEvent {
    // Emitted when a CLI subagent is spawned for a running command block.
    SpawnedSubagent {
        task_id: TaskId,
        block_id: BlockId,
        conversation_id: AIConversationId,

        /// The ID of the requested command for which this subagent was spawned, if any.
        ///
        /// None if the subagent was spawned by entering agent mode during a user-executed command,
        /// rather than a requested command.
        initial_requested_command_action_id: Option<AIAgentActionId>,
    },
    // Emitted when a CLI subagent's execution ends.
    FinishedSubagent {
        block_id: BlockId,
        conversation_id: Option<AIConversationId>,
        initial_requested_command_action_id: Option<AIAgentActionId>,
    },
    UpdatedControl {
        block_id: BlockId,
        requested_command_action_id: Option<AIAgentActionId>,
        agent_has_control: bool,
    },
    UpdatedLastSnapshot,
    ToggledHideResponses,
    /// Emitted when the user hands control back to the agent after a
    /// TransferShellCommandControlToUser action.
    ControlHandedBackAfterTransfer,
}

impl Entity for CLISubagentController {
    type Event = CLISubagentEvent;
}

fn snapshot_block_id_for_action_result(result: &AIAgentActionResultType) -> Option<&BlockId> {
    // Enumerates all possible action result types that read a command output.
    match result {
        AIAgentActionResultType::RequestCommandOutput(
            RequestCommandOutputResult::LongRunningCommandSnapshot { block_id, .. },
        ) => Some(block_id),
        AIAgentActionResultType::WriteToLongRunningShellCommand(
            WriteToLongRunningShellCommandResult::Snapshot { block_id, .. },
        ) => Some(block_id),
        AIAgentActionResultType::ReadShellCommandOutput(
            ReadShellCommandOutputResult::LongRunningCommandSnapshot { block_id, .. },
        ) => Some(block_id),
        AIAgentActionResultType::TransferShellCommandControlToUser(
            TransferShellCommandControlToUserResult::Snapshot { block_id, .. },
        ) => Some(block_id),
        _ => None,
    }
}
