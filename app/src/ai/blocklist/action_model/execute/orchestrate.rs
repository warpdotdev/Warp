//! Executor for the `orchestrate` tool call.
//!
//! Mirrors [`super::ask_user_question::AskUserQuestionExecutor`]: the action
//! moves into an async-pending state until the user clicks one of the three
//! terminal buttons (Launch / Launch without orchestration / Reject) on the
//! [`render_orchestrate_config_card`](crate::ai::blocklist::block::view_impl::orchestration::render_orchestrate_config_card).
//! The button click dispatches an [`AIBlockAction::OrchestrateActionDecision`]
//! which is forwarded to [`Self::submit_decision`].
//!
//! The Launch path is split between this executor and Phase C's per-agent
//! `CreateAgentTask` dispatch:
//!
//! * **This executor** receives the user's `Launch` decision plus the
//!   resolved run-wide configuration and per-agent slice. It emits an
//!   [`OrchestrateExecutorEvent::CreateAgentBatch`] event that an upper
//!   layer (terminal view / pane group) is expected to handle.
//! * **Phase C** is the upper-layer handler that fans the batch out to N
//!   parallel `CreateAgentTask` GraphQL calls and reports per-agent
//!   outcomes back through [`Self::complete_launch`]. Until Phase C is
//!   wired up, the Launch path resolves immediately with an empty
//!   `agents` slice in the [`OrchestrateActionResult::Launched`] result;
//!   the LLM sees a Launched result with zero per-agent outcomes (the
//!   M=0 partial-failure case in PRODUCT.md), which is a defensible
//!   placeholder until the CreateAgentTask wiring lands.
//!
//! Reject and Launch-without-orchestration are fully functional today:
//! Reject closes the action with `Cancelled` (which maps to
//! `ConvertToAPITypeError::Ignore` so nothing reaches the wire — the
//! server's input interceptor synthesizes the generic
//! `Message_ToolCallResult.Cancel` marker on the next user input);
//! Launch-without-orchestration emits `OrchestrateActionResult::LaunchDenied`.

use std::collections::HashMap;

use ai::agent::action::{OrchestrateAgentRunConfig, OrchestrateExecutionMode};
use ai::agent::action_result::{
    OrchestrateActionResult, OrchestrateAgentOutcomeEntry, OrchestrateExecutionMode as ResultMode,
};
use futures::{future::BoxFuture, FutureExt};
use warpui::{Entity, ModelContext};

use crate::ai::agent::{AIAgentActionId, AIAgentActionResultType, AIAgentActionType};

use super::{ActionExecution, AnyActionExecution, ExecuteActionInput, PreprocessActionInput};

/// User decision conveyed from the OrchestrateConfigCard's three terminal
/// buttons.
#[derive(Clone, Debug)]
pub enum OrchestrateDecision {
    /// User clicked **Launch**. Carries the resolved run-wide configuration
    /// the user committed to. The executor expands this into per-agent
    /// `CreateAgentTask` calls and reports outcomes back via
    /// [`OrchestrateExecutor::complete_launch`].
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
    /// resolved (Phase C). Order matches `agent_run_configs[]` input order.
    Launched {
        model_id: String,
        harness: String,
        execution_mode: OrchestrateExecutionMode,
        agents: Vec<OrchestrateAgentOutcomeEntry>,
    },
    LaunchDenied,
    /// No `CreateAgentTask` calls were issued at all. Reserved for the
    /// strictly no-children-launched failure case (per spec).
    Failure {
        error: String,
    },
    Cancelled,
}

/// Per-action state for an in-flight orchestrate action.
struct PendingOrchestrate {
    sender: async_channel::Sender<OrchestrateChannelMessage>,
}

pub struct OrchestrateExecutor {
    pending: HashMap<AIAgentActionId, PendingOrchestrate>,
}

impl Default for OrchestrateExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl OrchestrateExecutor {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
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

        let (sender, receiver) = async_channel::bounded(1);
        self.pending
            .insert(action_id, PendingOrchestrate { sender });

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
    /// resolves the pending action.
    ///
    /// Phase B: Reject and LaunchWithoutOrchestration close the action with
    /// the corresponding terminal result. Launch closes with an empty
    /// `agents` slice (the M=0 placeholder) until Phase C wires up the
    /// per-agent `CreateAgentTask` dispatch.
    pub fn submit_decision(
        &mut self,
        action_id: &AIAgentActionId,
        decision: OrchestrateDecision,
        _ctx: &mut ModelContext<Self>,
    ) {
        let Some(pending) = self.pending.remove(action_id) else {
            log::warn!(
                "OrchestrateExecutor: no pending action for id {action_id:?} on submit_decision"
            );
            return;
        };
        let message = match decision {
            // TODO(QUALITY-569 phase C): expand this into N parallel
            // CreateAgentTask calls (one per agent_run_configs[i]) via
            // futures::future::join_all over the existing CreateAgentTask
            // GraphQL plumbing. Outcomes MUST be reported in
            // agent_run_configs[] input order, not completion order. For
            // now we resolve immediately with an empty agents slice (the
            // M=0 partial-failure case per spec) so Phase B is functionally
            // testable end-to-end.
            OrchestrateDecision::Launch {
                model_id,
                harness,
                execution_mode,
                agents: _agents,
            } => OrchestrateChannelMessage::Launched {
                model_id,
                harness,
                execution_mode,
                agents: Vec::new(),
            },
            OrchestrateDecision::LaunchWithoutOrchestration => {
                OrchestrateChannelMessage::LaunchDenied
            }
            OrchestrateDecision::Reject => OrchestrateChannelMessage::Cancelled,
        };
        let _ = pending.sender.try_send(message);
    }

    /// Used by the cancellation interceptor (e.g. when the user navigates
    /// away mid-action) to drop the pending action without emitting a
    /// terminal result. Maps to the same Cancelled path Reject uses.
    pub fn cancel(&mut self, action_id: &AIAgentActionId) {
        if let Some(pending) = self.pending.remove(action_id) {
            let _ = pending
                .sender
                .try_send(OrchestrateChannelMessage::Cancelled);
        }
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

impl Entity for OrchestrateExecutor {
    type Event = ();
}

#[cfg(test)]
impl OrchestrateExecutor {
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}
