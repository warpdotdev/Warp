//! Executor for the `orchestrate` tool call.
//!
//! Mirrors [`super::ask_user_question::AskUserQuestionExecutor`]: the action
//! moves into an async-pending state until the user clicks one of the three
//! terminal buttons (Launch / Launch without orchestration / Reject) on the
//! [`render_orchestrate_config_card`](crate::ai::blocklist::block::view_impl::orchestration::render_orchestrate_config_card).
//! The button click dispatches an [`AIBlockAction::OrchestrateActionDecision`]
//! which is forwarded to [`Self::submit_decision`].
//!
//! ## Launch flow (Phase C)
//!
//! When the user clicks **Launch**, [`Self::submit_decision`] runs run-wide
//! pre-dispatch validation against [`OrchestrateExecutionMode`] and the
//! resolved harness:
//!
//! * If the run-wide config is invalid (e.g. Remote without an environment
//!   id, Remote+OpenCode, or [`FeatureFlag::OrchestrationV2`] disabled while
//!   targeting Remote), no `CreateAgentTask` is issued at all and the
//!   executor resolves with [`OrchestrateActionResult::Failure`]. This is
//!   the "pre-dispatch failure" path per spec.
//!
//! * Otherwise it constructs N [`StartAgentRequest`]s — one per
//!   `agent_run_configs[i]`, each carrying the resolved run-wide
//!   model/harness/execution-mode plus the per-agent name and prompt — and
//!   emits a single [`OrchestrateExecutorEvent::CreateAgentBatch`] event
//!   that the terminal view consumes and re-emits as N
//!   `Event::StartAgentConversation`s, fanning out into N parallel
//!   `CreateAgentTask` GraphQL calls through the same path
//!   [`super::start_agent::StartAgentExecutor`] uses.
//!
//! Per-agent outcomes are tracked in input-order [`AgentSlot`]s. The
//! executor subscribes to [`BlocklistAIHistoryEvent`]s and resolves slots as
//! `StartedNewConversation` + `ConversationServerTokenAssigned` /
//! `UpdatedConversationStatus` fire for each child. Once every slot has
//! resolved, the aggregated [`OrchestrateActionResult::Launched`] is emitted
//! with `agents` in `agent_run_configs[]` input order regardless of which
//! `CreateAgentTask` returned first. The M=0 case (every per-agent dispatch
//! failed) still emits `Launched` per spec — `Failure` is reserved for the
//! pre-dispatch case where no `CreateAgentTask` was issued.
//!
//! ## Reject / LaunchWithoutOrchestration
//!
//! Reject closes the action with `Cancelled` (which maps to
//! `ConvertToAPITypeError::Ignore` so nothing reaches the wire — the
//! server's input interceptor synthesizes the generic
//! `Message_ToolCallResult.Cancel` marker on the next user input);
//! Launch-without-orchestration emits `OrchestrateActionResult::LaunchDenied`.

use std::collections::HashMap;

use ai::agent::action::{OrchestrateAgentRunConfig, OrchestrateExecutionMode};
use ai::agent::action_result::{
    OrchestrateActionResult, OrchestrateAgentOutcome, OrchestrateAgentOutcomeEntry,
    OrchestrateExecutionMode as ResultMode,
};
use futures::{future::BoxFuture, FutureExt};
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::agent::{
    AIAgentActionId, AIAgentActionResultType, AIAgentActionType, StartAgentExecutionMode,
};
use crate::ai::blocklist::BlocklistAIHistoryEvent;
use crate::BlocklistAIHistoryModel;

use super::start_agent::StartAgentRequest;
use super::{ActionExecution, AnyActionExecution, ExecuteActionInput, PreprocessActionInput};

/// User decision conveyed from the OrchestrateConfigCard's three terminal
/// buttons.
#[derive(Clone, Debug)]
pub enum OrchestrateDecision {
    /// User clicked **Launch**. Carries the resolved run-wide configuration
    /// the user committed to. The executor expands this into per-agent
    /// `CreateAgentTask` calls and aggregates the outcomes.
    Launch {
        model_id: String,
        harness: String,
        execution_mode: OrchestrateExecutionMode,
        agents: Vec<OrchestrateAgentRunConfig>,
    },
    /// User clicked **Launch without orchestration**. The lead agent
    /// continues without spawning the team.
    LaunchWithoutOrchestration,
    /// User clicked **Reject**. The action is cancelled; the server-side
    /// input interceptor synthesizes the generic
    /// `Message_ToolCallResult.Cancel` marker on the next user input.
    Reject,
}

/// Internal channel payload that resolves the executor's pending future.
enum OrchestrateChannelMessage {
    /// Final per-agent outcomes after all `CreateAgentTask` calls have
    /// resolved. Order matches `agent_run_configs[]` input order.
    Launched {
        model_id: String,
        harness: String,
        execution_mode: OrchestrateExecutionMode,
        agents: Vec<OrchestrateAgentOutcomeEntry>,
    },
    LaunchDenied,
    /// Run-wide pre-dispatch validation failed: no `CreateAgentTask` was
    /// issued at all. Reserved for the strictly no-children-launched case
    /// per spec.
    Failure {
        error: String,
    },
    Cancelled,
}

/// One per-agent slot in an in-flight Launch. Slots are kept in
/// `agent_run_configs[]` input order; each resolves independently as its
/// child conversation's lifecycle fires through `BlocklistAIHistoryEvent`s.
struct AgentSlot {
    /// Configured agent name. Used to match `StartedNewConversation` events
    /// against the slot when multiple children share the same parent.
    name: String,
    /// Set once a `StartedNewConversation` has been matched to this slot.
    child_conversation_id: Option<AIConversationId>,
    /// Set once the slot has resolved (success or failure).
    outcome: Option<OrchestrateAgentOutcome>,
}

/// State for a Launch decision that has been validated and dispatched.
/// Held for the lifetime of the parallel per-agent `CreateAgentTask`
/// in-flight window.
struct InFlightLaunch {
    parent_conversation_id: AIConversationId,
    model_id: String,
    harness: String,
    execution_mode: OrchestrateExecutionMode,
    slots: Vec<AgentSlot>,
    /// Sender on the channel that the executor's `execute()` future is
    /// awaiting. Resolved with the aggregated `Launched` outcome once every
    /// slot has been settled.
    sender: async_channel::Sender<OrchestrateChannelMessage>,
}

/// Per-action state for an in-flight orchestrate action awaiting the user's
/// decision (pre-Launch). Captures the parent conversation id from
/// `execute()` so it can be threaded through to per-agent
/// `StartAgentRequest`s when the user clicks Launch.
struct PendingOrchestrate {
    sender: async_channel::Sender<OrchestrateChannelMessage>,
    conversation_id: AIConversationId,
}

pub struct OrchestrateExecutor {
    /// Actions awaiting the user's decision (pre-Launch).
    pending: HashMap<AIAgentActionId, PendingOrchestrate>,
    /// Actions whose Launch decision has been dispatched and which are now
    /// awaiting per-agent `CreateAgentTask` resolutions.
    in_flight: HashMap<AIAgentActionId, InFlightLaunch>,
}

impl OrchestrateExecutor {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let history_model = BlocklistAIHistoryModel::handle(ctx);
        ctx.subscribe_to_model(&history_model, Self::handle_history_event);
        Self {
            pending: HashMap::new(),
            in_flight: HashMap::new(),
        }
    }

    /// Returns `true` so the action skips the standard Blocked Run/Cancel
    /// confirmation UI and goes straight into the async-pending
    /// [`Self::execute`] state. From there the user confirms or cancels via
    /// the OrchestrateConfigCard's own three terminal buttons. The standard
    /// Blocked UI is wrong for orchestrate because the card embeds its own
    /// purpose-built buttons (Launch / Launch without orchestration /
    /// Reject) and doesn't need a generic Run/Cancel pair.
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
        _ctx: &mut ModelContext<Self>,
    ) -> impl Into<AnyActionExecution> {
        if !matches!(input.action.action, AIAgentActionType::Orchestrate { .. }) {
            return ActionExecution::InvalidAction;
        }
        let action_id = input.action.id.clone();
        let conversation_id = input.conversation_id;

        let (sender, receiver) = async_channel::bounded(1);
        self.pending.insert(
            action_id,
            PendingOrchestrate {
                sender,
                conversation_id,
            },
        );

        ActionExecution::new_async(async move { receiver.recv().await }, move |result, _ctx| {
            match result {
                Ok(OrchestrateChannelMessage::Launched {
                    model_id,
                    harness,
                    execution_mode,
                    agents,
                }) => AIAgentActionResultType::Orchestrate(OrchestrateActionResult::Launched {
                    model_id,
                    harness,
                    execution_mode: execution_mode_to_result(execution_mode),
                    agents,
                }),
                Ok(OrchestrateChannelMessage::LaunchDenied) => {
                    AIAgentActionResultType::Orchestrate(OrchestrateActionResult::LaunchDenied)
                }
                Ok(OrchestrateChannelMessage::Failure { error }) => {
                    AIAgentActionResultType::Orchestrate(OrchestrateActionResult::Failure { error })
                }
                Ok(OrchestrateChannelMessage::Cancelled) | Err(_) => {
                    AIAgentActionResultType::Orchestrate(OrchestrateActionResult::Cancelled)
                }
            }
        })
    }

    pub(super) fn preprocess_action(
        &mut self,
        _input: PreprocessActionInput,
        _ctx: &mut ModelContext<Self>,
    ) -> BoxFuture<'static, ()> {
        futures::future::ready(()).boxed()
    }

    /// Receives a button click from `render_orchestrate_config_card` and
    /// resolves (or, for Launch, begins resolving) the pending action.
    pub fn submit_decision(
        &mut self,
        action_id: &AIAgentActionId,
        decision: OrchestrateDecision,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(pending) = self.pending.remove(action_id) else {
            log::warn!(
                "OrchestrateExecutor: no pending action for id {action_id:?} on submit_decision"
            );
            return;
        };

        match decision {
            OrchestrateDecision::Reject => {
                let _ = pending
                    .sender
                    .try_send(OrchestrateChannelMessage::Cancelled);
            }
            OrchestrateDecision::LaunchWithoutOrchestration => {
                let _ = pending
                    .sender
                    .try_send(OrchestrateChannelMessage::LaunchDenied);
            }
            OrchestrateDecision::Launch {
                model_id,
                harness,
                execution_mode,
                agents,
            } => {
                self.dispatch_launch(
                    action_id.clone(),
                    pending,
                    model_id,
                    harness,
                    execution_mode,
                    agents,
                    ctx,
                );
            }
        }
    }

    /// Validates the run-wide Launch config and either dispatches per-agent
    /// `CreateAgentTask`s (success path) or resolves the action with
    /// [`OrchestrateActionResult::Failure`] (pre-dispatch failure path).
    fn dispatch_launch(
        &mut self,
        action_id: AIAgentActionId,
        pending: PendingOrchestrate,
        model_id: String,
        harness: String,
        execution_mode: OrchestrateExecutionMode,
        agents: Vec<OrchestrateAgentRunConfig>,
        ctx: &mut ModelContext<Self>,
    ) {
        if let Err(error) = validate_launch_config(&execution_mode, &harness, &agents) {
            let _ = pending
                .sender
                .try_send(OrchestrateChannelMessage::Failure { error });
            return;
        }

        let parent_conversation_id = pending.conversation_id;
        let parent_run_id = BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&parent_conversation_id)
            .and_then(|conversation| conversation.run_id());

        let start_agent_execution_mode =
            match build_start_agent_execution_mode(&execution_mode, &harness, &model_id) {
                Ok(mode) => mode,
                Err(error) => {
                    let _ = pending
                        .sender
                        .try_send(OrchestrateChannelMessage::Failure { error });
                    return;
                }
            };

        // For remote / non-default-local modes, parent_run_id is mandatory
        // (per StartAgentExecutor::execute).
        if matches!(
            &start_agent_execution_mode,
            StartAgentExecutionMode::Remote { .. }
                | StartAgentExecutionMode::Local {
                    harness_type: Some(_)
                }
        ) && parent_run_id.is_none()
        {
            let _ = pending.sender.try_send(OrchestrateChannelMessage::Failure {
                error: "Parent run_id is not yet available; the orchestrator must \
                        receive its first server response before launching child agents."
                    .to_string(),
            });
            return;
        }

        let requests: Vec<StartAgentRequest> = agents
            .iter()
            .map(|agent| StartAgentRequest {
                name: agent.name.clone(),
                prompt: agent.prompt.clone(),
                execution_mode: start_agent_execution_mode.clone(),
                lifecycle_subscription: None,
                parent_conversation_id,
                parent_run_id: parent_run_id.clone(),
            })
            .collect();

        let slots: Vec<AgentSlot> = agents
            .iter()
            .map(|agent| AgentSlot {
                name: agent.name.clone(),
                child_conversation_id: None,
                outcome: None,
            })
            .collect();

        self.in_flight.insert(
            action_id,
            InFlightLaunch {
                parent_conversation_id,
                model_id,
                harness,
                execution_mode,
                slots,
                sender: pending.sender,
            },
        );

        ctx.emit(OrchestrateExecutorEvent::CreateAgentBatch(requests));
    }

    /// Used by the cancellation interceptor (e.g. when the user navigates
    /// away mid-action) to drop the pending action without emitting a
    /// terminal result. Maps to the same Cancelled path Reject uses. Also
    /// drops any in-flight Launch.
    pub fn cancel(&mut self, action_id: &AIAgentActionId) {
        if let Some(pending) = self.pending.remove(action_id) {
            let _ = pending
                .sender
                .try_send(OrchestrateChannelMessage::Cancelled);
        }
        if let Some(in_flight) = self.in_flight.remove(action_id) {
            let _ = in_flight
                .sender
                .try_send(OrchestrateChannelMessage::Cancelled);
        }
    }

    fn handle_history_event(
        &mut self,
        event: &BlocklistAIHistoryEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.in_flight.is_empty() {
            return;
        }

        match event {
            BlocklistAIHistoryEvent::StartedNewConversation {
                new_conversation_id,
                ..
            } => self.try_match_new_child(*new_conversation_id, ctx),
            BlocklistAIHistoryEvent::ConversationServerTokenAssigned {
                conversation_id, ..
            } => self.try_resolve_slot_with_agent_id(*conversation_id, ctx),
            BlocklistAIHistoryEvent::UpdatedConversationStatus {
                conversation_id, ..
            } => self.try_resolve_slot_with_status(*conversation_id, ctx),
            _ => {}
        }
    }

    /// Matches a newly-created child conversation against an unfilled slot
    /// for whichever in-flight launch its `parent_conversation_id` belongs
    /// to. Slots prefer name matches; if no name match is found we fall
    /// back to the first unfilled slot (covers the case where the child's
    /// `agent_name` has not been set by the time `StartedNewConversation`
    /// fires).
    fn try_match_new_child(
        &mut self,
        new_conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let history = BlocklistAIHistoryModel::as_ref(ctx);
        let Some(conversation) = history.conversation(&new_conversation_id) else {
            return;
        };
        let Some(parent_id) = conversation.parent_conversation_id() else {
            return;
        };
        let agent_name = conversation.agent_name().map(str::to_string);

        for in_flight in self.in_flight.values_mut() {
            if in_flight.parent_conversation_id != parent_id {
                continue;
            }
            // Prefer name match.
            let matched_idx = agent_name.as_deref().and_then(|name| {
                in_flight
                    .slots
                    .iter()
                    .position(|slot| slot.child_conversation_id.is_none() && slot.name == name)
            });
            // Fall back to FIFO match.
            let matched_idx = matched_idx.or_else(|| {
                in_flight
                    .slots
                    .iter()
                    .position(|slot| slot.child_conversation_id.is_none())
            });
            if let Some(idx) = matched_idx {
                in_flight.slots[idx].child_conversation_id = Some(new_conversation_id);
                return;
            }
        }
    }

    /// Resolves the slot associated with `conversation_id` when its server
    /// token is assigned (success path).
    fn try_resolve_slot_with_agent_id(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let agent_id = BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&conversation_id)
            .and_then(|c| c.orchestration_agent_id());
        let Some(agent_id) = agent_id else {
            return;
        };

        let mut completed = None;
        for (action_id, in_flight) in self.in_flight.iter_mut() {
            if let Some(slot) = in_flight.slots.iter_mut().find(|slot| {
                slot.child_conversation_id == Some(conversation_id) && slot.outcome.is_none()
            }) {
                slot.outcome = Some(OrchestrateAgentOutcome::Launched {
                    agent_id: agent_id.clone(),
                });
                if all_slots_resolved(&in_flight.slots) {
                    completed = Some(action_id.clone());
                }
                break;
            }
        }
        if let Some(action_id) = completed {
            self.complete_in_flight(&action_id);
        }
    }

    /// Resolves the slot associated with `conversation_id` when its
    /// conversation status transitions into a terminal failure state
    /// (Error / Cancelled / Blocked).
    fn try_resolve_slot_with_status(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let history = BlocklistAIHistoryModel::as_ref(ctx);
        let Some(conversation) = history.conversation(&conversation_id) else {
            return;
        };
        let Some(error_msg) = child_error_message_for_status(
            conversation.status(),
            conversation.status_error_message(),
        ) else {
            return;
        };

        let mut completed = None;
        for (action_id, in_flight) in self.in_flight.iter_mut() {
            if let Some(slot) = in_flight.slots.iter_mut().find(|slot| {
                slot.child_conversation_id == Some(conversation_id) && slot.outcome.is_none()
            }) {
                slot.outcome = Some(OrchestrateAgentOutcome::Failed {
                    error: error_msg.clone(),
                });
                if all_slots_resolved(&in_flight.slots) {
                    completed = Some(action_id.clone());
                }
                break;
            }
        }
        if let Some(action_id) = completed {
            self.complete_in_flight(&action_id);
        }
    }

    /// Aggregates the per-slot outcomes of an in-flight launch (preserving
    /// `agent_run_configs[]` input order) and resolves the executor's
    /// pending channel with [`OrchestrateChannelMessage::Launched`]. Slots
    /// that were never matched or never resolved are reported as `Failed`
    /// with a generic error. Per spec, this is a `Launched` result even if
    /// every slot failed (the M=0 partial-failure case).
    fn complete_in_flight(&mut self, action_id: &AIAgentActionId) {
        let Some(in_flight) = self.in_flight.remove(action_id) else {
            return;
        };
        let agents: Vec<OrchestrateAgentOutcomeEntry> = in_flight
            .slots
            .into_iter()
            .map(|slot| OrchestrateAgentOutcomeEntry {
                name: slot.name,
                outcome: slot
                    .outcome
                    .unwrap_or_else(|| OrchestrateAgentOutcome::Failed {
                        error: "Child agent did not complete startup".to_string(),
                    }),
            })
            .collect();
        let _ = in_flight
            .sender
            .try_send(OrchestrateChannelMessage::Launched {
                model_id: in_flight.model_id,
                harness: in_flight.harness,
                execution_mode: in_flight.execution_mode,
                agents,
            });
    }
}

/// Whether every slot has resolved (success or failure).
fn all_slots_resolved(slots: &[AgentSlot]) -> bool {
    slots.iter().all(|slot| slot.outcome.is_some())
}

/// Run-wide pre-dispatch validation. Returns `Err(error_message)` if the
/// config is unworkable; in that case no `CreateAgentTask` will be issued.
fn validate_launch_config(
    execution_mode: &OrchestrateExecutionMode,
    harness: &str,
    agents: &[OrchestrateAgentRunConfig],
) -> Result<(), String> {
    if agents.is_empty() {
        return Err("Cannot launch orchestration with zero agents.".to_string());
    }
    match execution_mode {
        OrchestrateExecutionMode::Local => Ok(()),
        OrchestrateExecutionMode::Remote { environment_id } => {
            if environment_id.trim().is_empty() {
                return Err("Choose an environment before launching.".to_string());
            }
            if Harness::parse_orchestration_harness(harness) == Some(Harness::OpenCode) {
                return Err(
                    "OpenCode is not supported in remote mode. Switch to a different \
                     harness before launching."
                        .to_string(),
                );
            }
            if !FeatureFlag::OrchestrationV2.is_enabled() {
                return Err(
                    "Remote child agents require orchestration v2, which is not enabled."
                        .to_string(),
                );
            }
            Ok(())
        }
    }
}

/// Builds the per-agent [`StartAgentExecutionMode`] from the run-wide
/// orchestrate config. The orchestrate tool currently shares one execution
/// mode across all per-agent dispatches; per-agent overrides can be added
/// later by extending [`OrchestrateAgentRunConfig`].
fn build_start_agent_execution_mode(
    execution_mode: &OrchestrateExecutionMode,
    harness: &str,
    model_id: &str,
) -> Result<StartAgentExecutionMode, String> {
    let trimmed_harness = harness.trim();
    let harness_type = if trimmed_harness.is_empty() {
        None
    } else {
        Some(trimmed_harness.to_string())
    };
    match execution_mode {
        OrchestrateExecutionMode::Local => Ok(StartAgentExecutionMode::Local { harness_type }),
        OrchestrateExecutionMode::Remote { environment_id } => {
            Ok(StartAgentExecutionMode::Remote {
                environment_id: environment_id.clone(),
                skill_references: Vec::new(),
                model_id: model_id.to_string(),
                computer_use_enabled: false,
                worker_host: String::new(),
                harness_type: trimmed_harness.to_string(),
                title: String::new(),
            })
        }
    }
}

/// Maps a child conversation's terminal status into a user-facing error
/// message. Returns `None` for non-terminal states (`InProgress`, `Success`).
/// Mirrors the corresponding logic in
/// [`super::start_agent::start_agent_error_message_for_status`] but is
/// duplicated here to avoid making that helper public.
fn child_error_message_for_status(
    status: &ConversationStatus,
    error_message: Option<&str>,
) -> Option<String> {
    match status {
        ConversationStatus::Error => Some(
            error_message
                .filter(|message| !message.trim().is_empty())
                .unwrap_or("Child agent failed to initialize")
                .to_string(),
        ),
        ConversationStatus::Cancelled => {
            Some("Child agent was cancelled before initialization".to_string())
        }
        ConversationStatus::Blocked { blocked_action } => {
            let blocked_action = blocked_action.trim();
            Some(if blocked_action.is_empty() {
                "Child agent startup was blocked before initialization".to_string()
            } else {
                blocked_action.to_string()
            })
        }
        ConversationStatus::InProgress | ConversationStatus::Success => None,
    }
}

fn execution_mode_to_result(mode: OrchestrateExecutionMode) -> ResultMode {
    match mode {
        OrchestrateExecutionMode::Local => ResultMode::Local,
        OrchestrateExecutionMode::Remote { environment_id } => {
            ResultMode::Remote { environment_id }
        }
    }
}

/// Events emitted by the [`OrchestrateExecutor`].
///
/// The terminal view subscribes to this and re-emits each
/// [`StartAgentRequest`] in [`OrchestrateExecutorEvent::CreateAgentBatch`]
/// as an [`crate::terminal::view::Event::StartAgentConversation`], fanning
/// the batch out to N parallel `CreateAgentTask`s through the same path
/// [`super::start_agent::StartAgentExecutor`] uses.
pub enum OrchestrateExecutorEvent {
    /// Resolved per-agent [`StartAgentRequest`]s in
    /// `agent_run_configs[]` input order. Order is preserved so that the
    /// view can dispatch them in a predictable sequence; per-agent outcomes
    /// are still collected back into input order by
    /// [`OrchestrateExecutor`] regardless of which `CreateAgentTask`
    /// returns first.
    CreateAgentBatch(Vec<StartAgentRequest>),
}

impl Entity for OrchestrateExecutor {
    type Event = OrchestrateExecutorEvent;
}

#[cfg(test)]
impl OrchestrateExecutor {
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }
}
