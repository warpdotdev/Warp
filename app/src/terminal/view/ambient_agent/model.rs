use std::time::Duration;

use instant::Instant;
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;
use warpui::r#async::{SpawnedFutureHandle, Timer};
use warpui::{Entity, EntityId, ModelContext, SingletonEntity};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent_conversations_model::AgentConversationsModel;
use crate::ai::ambient_agents::spawn::{spawn_task, AmbientAgentEvent};
use crate::ai::ambient_agents::task::HarnessConfig;
use crate::ai::ambient_agents::{
    AgentConfigSnapshot, AmbientAgentTaskId, AmbientAgentTaskState, AttachmentInput,
    SpawnAgentRequest, OUT_OF_CREDITS_TASK_FAILURE_MESSAGE, SERVER_OVERLOADED_TASK_FAILURE_MESSAGE,
};
use crate::ai::api_error::AIApiError;
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::blocklist::BlocklistAIPermissions;
use crate::ai::llms::{LLMId, LLMPreferences};

use super::AmbientAgentProgressUIState;

/// Tracks progress timestamps for each step during ambient agent spawning.
#[derive(Debug, Clone)]
pub struct AgentProgress {
    /// When the agent run was requested.
    pub spawned_at: Instant,
    /// When the run was claimed by a worker.
    pub claimed_at: Option<Instant>,
    /// When the agent harness began executing.
    pub harness_started_at: Option<Instant>,
    /// When the agent stopped.
    pub stopped_at: Option<Instant>,
}

/// Status of the ambient agent run.
#[derive(Debug, Clone)]
pub enum Status {
    /// Not in an ambient agent session.
    NotAmbientAgent,
    /// First-time ambient-agent setup.
    Setup,
    /// The user is composing their ambient agent prompt.
    Composing,
    /// Waiting for the ambient agent run to be ready.
    WaitingForSession { progress: AgentProgress },
    /// The agent is running and the session is ready.
    AgentRunning,
    /// The agent failed.
    Failed {
        progress: AgentProgress,
        error_message: String,
    },
    /// The user needs to authenticate with GitHub.
    NeedsGithubAuth {
        progress: AgentProgress,
        error_message: String,
        auth_url: String,
    },
    /// The agent was cancelled.
    Cancelled { progress: AgentProgress },
}

/// Model to track the state of an ambient agent run.
pub struct AmbientAgentViewModel {
    status: Status,

    /// The request with which the ambient agent was spawned, if it was spawned.
    request: Option<SpawnAgentRequest>,

    /// The terminal view this model is part of.
    terminal_view_id: EntityId,

    /// Whether this ambient agent view has a parent terminal view to return to.
    /// `false` for standalone views.
    /// `true` for nested views (pushed onto an existing terminal's pane stack).
    has_parent_terminal: bool,

    /// Handle for the periodic timer that updates progress durations.
    progress_timer_handle: Option<SpawnedFutureHandle>,

    /// UI state for rendering the ambient agent progress screen.
    pub ui_state: AmbientAgentProgressUIState,

    /// The task ID for the current ambient-agent task, if one has been spawned.
    task_id: Option<AmbientAgentTaskId>,

    /// The local conversation associated with this ambient-agent run, if any.
    /// Set for remote child agents spawned via `start_agent` so the `run_id`
    /// from the server response can be wired back to the conversation.
    conversation_id: Option<AIConversationId>,

    /// Selected execution harness for the ambient-agent run.
    /// Defaults to `Harness::Oz`. Used to populate `AgentConfigSnapshot.harness` on spawn.
    harness: Harness,
    /// Whether the optimistic InitialUserQuery block has been inserted for the current run.
    has_inserted_ambient_agent_user_query_block: bool,
    /// Whether the harness CLI (e.g. `claude`, `gemini`) has started running for a non-oz run.
    /// Used to transition the ambient-agent setup UI out of the pre-first-exchange phase when
    /// there is no oz `AppendedExchange` to key off of.
    harness_command_started: bool,
}

impl AmbientAgentViewModel {
    pub fn new(
        terminal_view_id: EntityId,
        has_parent_terminal: bool,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let ui_state = AmbientAgentProgressUIState::new(ctx);

        Self {
            status: Status::NotAmbientAgent,
            request: None,
            terminal_view_id,
            has_parent_terminal,
            progress_timer_handle: None,
            ui_state,
            task_id: None,
            conversation_id: None,
            harness: Harness::default(),
            has_inserted_ambient_agent_user_query_block: false,
            harness_command_started: false,
        }
    }

    pub fn request(&self) -> Option<&SpawnAgentRequest> {
        self.request.as_ref()
    }

    /// Returns the agent progress for tracking spawn steps.
    /// Returns `None` if not in the `WaitingForSession`, `Failed`, `NeedsGithubAuth`, or `Cancelled` state.
    pub fn agent_progress(&self) -> Option<&AgentProgress> {
        match &self.status {
            Status::WaitingForSession { progress }
            | Status::Failed { progress, .. }
            | Status::NeedsGithubAuth { progress, .. }
            | Status::Cancelled { progress } => Some(progress),
            _ => None,
        }
    }

    pub fn selected_harness(&self) -> Harness {
        self.harness
    }

    pub fn set_harness(&mut self, harness: Harness, ctx: &mut ModelContext<Self>) {
        if self.harness == harness {
            return;
        }
        self.harness = harness;
        ctx.emit(AmbientAgentViewModelEvent::HarnessSelected);
    }

    /// True when the run is configured to use a non-Oz execution harness and the
    /// required feature flags are enabled.
    pub(super) fn is_third_party_harness(&self) -> bool {
        FeatureFlag::AgentHarness.is_enabled() && self.harness != Harness::Oz
    }

    /// Whether the harness CLI has started running. Only meaningful for non-oz runs.
    pub(super) fn harness_command_started(&self) -> bool {
        self.harness_command_started
    }

    /// Marks the harness CLI as started and emits `HarnessCommandStarted`.
    /// Idempotent: subsequent calls after the first are no-ops and do not re-emit.
    pub(super) fn mark_harness_command_started(&mut self, ctx: &mut ModelContext<Self>) {
        debug_assert!(
            self.harness != Harness::Oz,
            "harness_command_started is only meaningful for non-oz runs"
        );
        if self.harness_command_started {
            return;
        }
        self.harness_command_started = true;
        ctx.emit(AmbientAgentViewModelEvent::HarnessCommandStarted);
    }

    /// Whether or not this terminal session is for an ambient agent.
    pub fn is_ambient_agent(&self) -> bool {
        !matches!(self.status, Status::NotAmbientAgent)
    }

    /// Returns the task ID for the current ambient-agent task, if one has been spawned.
    pub fn task_id(&self) -> Option<AmbientAgentTaskId> {
        self.task_id
    }

    pub fn has_inserted_ambient_agent_user_query_block(&self) -> bool {
        self.has_inserted_ambient_agent_user_query_block
    }

    pub fn set_has_inserted_ambient_agent_user_query_block(&mut self, has_inserted: bool) {
        self.has_inserted_ambient_agent_user_query_block = has_inserted;
    }

    /// Returns whether this ambient agent view has a parent terminal to return to.
    pub fn has_parent_terminal(&self) -> bool {
        self.has_parent_terminal
    }

    /// Sets whether this ambient agent view has a parent terminal to return to.
    /// This should be called when pushing the view onto an existing pane stack.
    pub fn set_has_parent_terminal(&mut self, has_parent: bool) {
        self.has_parent_terminal = has_parent;
    }

    /// Whether or not this terminal session is in the setup state (first-time environment creation).
    pub fn is_in_setup(&self) -> bool {
        matches!(self.status, Status::Setup)
    }

    /// Whether or not this terminal session is currently setting up an ambient agent run.
    pub fn is_configuring_ambient_agent(&self) -> bool {
        matches!(self.status, Status::Composing)
    }

    /// Whether or not this terminal session is waiting for an ambient agent session to be ready.
    pub fn is_waiting_for_session(&self) -> bool {
        matches!(self.status, Status::WaitingForSession { .. })
    }

    /// Whether or not the ambient agent failed to spawn.
    pub fn is_failed(&self) -> bool {
        matches!(self.status, Status::Failed { .. })
    }

    /// Whether or not the ambient agent was cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self.status, Status::Cancelled { .. })
    }

    /// Whether or not the ambient agent needs GitHub authentication.
    pub fn is_needs_github_auth(&self) -> bool {
        matches!(self.status, Status::NeedsGithubAuth { .. })
    }

    /// Whether or not the ambient agent is currently running.
    pub fn is_agent_running(&self) -> bool {
        matches!(self.status, Status::AgentRunning)
    }

    /// Whether or not we should show a status footer (loading, error, auth, or cancelled).
    pub fn should_show_status_footer(&self) -> bool {
        if false {
            return false;
        }

        self.is_waiting_for_session()
            || self.is_failed()
            || self.is_needs_github_auth()
            || self.is_cancelled()
    }

    /// Returns the error message if the agent is in a failed state.
    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            Status::Failed { error_message, .. } => Some(error_message),
            _ => None,
        }
    }

    /// Returns the GitHub auth URL if the agent needs GitHub authentication.
    pub fn github_auth_url(&self) -> Option<&str> {
        match &self.status {
            Status::NeedsGithubAuth { auth_url, .. } => Some(auth_url),
            _ => None,
        }
    }

    /// Returns the error message for GitHub authentication failures.
    pub fn github_auth_error_message(&self) -> Option<&str> {
        match &self.status {
            Status::NeedsGithubAuth { error_message, .. } => Some(error_message),
            _ => None,
        }
    }

    /// Enter the setup state for first-time environment creation.
    pub fn enter_setup(&mut self, ctx: &mut ModelContext<Self>) {
        self.status = Status::Setup;
        ctx.emit(AmbientAgentViewModelEvent::EnteredSetupState);
    }

    /// Transition from Setup to Composing state.
    pub fn enter_composing_from_setup(&mut self, ctx: &mut ModelContext<Self>) {
        self.status = Status::Composing;
        ctx.emit(AmbientAgentViewModelEvent::EnteredComposingState);
    }

    /// 用于加入已在运行的 ambient agent 共享会话。
    pub fn enter_viewing_existing_session(
        &mut self,
        task_id: AmbientAgentTaskId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.task_id = Some(task_id);

        if matches!(self.status, Status::NotAmbientAgent) {
            self.status = Status::AgentRunning;
        }
        ctx.notify();
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Reset the status back to NotAmbientAgent.
    pub fn reset_status(&mut self, ctx: &mut ModelContext<Self>) {
        self.status = Status::NotAmbientAgent;
        self.task_id = None;
        self.conversation_id = None;
        self.has_inserted_ambient_agent_user_query_block = false;
        self.harness_command_started = false;
        self.stop_progress_timer();
        ctx.notify();
    }

    /// Sets the local conversation ID associated with this ambient-agent run.
    pub fn set_conversation_id(&mut self, id: Option<AIConversationId>) {
        self.conversation_id = id;
    }

    /// Spawn an ambient agent with the given prompt and current session configuration.
    pub fn spawn_agent(
        &mut self,
        prompt: String,
        attachments: Vec<AttachmentInput>,
        ctx: &mut ModelContext<Self>,
    ) {
        let model_id = LLMPreferences::as_ref(ctx)
            .get_active_base_model(ctx, Some(self.terminal_view_id))
            .id
            .to_string();

        let computer_use_enabled = Some(
            FeatureFlag::AgentModeComputerUse.is_enabled()
                && BlocklistAIPermissions::as_ref(ctx)
                    .get_computer_use_setting(ctx, Some(self.terminal_view_id))
                    .is_enabled()
                && computer_use::is_supported_on_current_platform(),
        );

        let harness_override =
            (self.harness != Harness::Oz).then(|| HarnessConfig::from_harness_type(self.harness));

        let config = Some(AgentConfigSnapshot {
            environment_id: None,
            model_id: Some(model_id),
            computer_use_enabled,
            harness: harness_override,
            ..Default::default()
        });

        let request = SpawnAgentRequest {
            prompt,
            config,
            title: None,
            team: None,
            skill: None,
            attachments,
            interactive: None,
            parent_run_id: None,
            runtime_skills: vec![],
            referenced_attachments: vec![],
        };

        self.spawn_internal(request, ctx);
    }

    /// Spawn an ambient agent with a fully-constructed request.
    pub fn spawn_agent_with_request(
        &mut self,
        request: SpawnAgentRequest,
        ctx: &mut ModelContext<Self>,
    ) {
        // Apply pane settings from the request.
        if let Some(config) = request.config.as_ref() {
            if let Some(model_id) = config.model_id.as_deref() {
                LLMPreferences::handle(ctx).update(ctx, |prefs, ctx| {
                    prefs.update_preferred_agent_mode_llm(
                        &LLMId::from(model_id),
                        self.terminal_view_id,
                        ctx,
                    )
                });
            }
        }

        self.spawn_internal(request, ctx);
    }

    /// Spawn an ambient agent given `request`.
    fn spawn_internal(&mut self, mut request: SpawnAgentRequest, ctx: &mut ModelContext<Self>) {
        request.interactive = Some(true);
        self.request = Some(request.clone());
        let stream = spawn_task(request, None);

        ctx.spawn_stream_local(
            stream,
            |me, event_result, ctx| {
                // If we're in Cancelled or Failed state, ignore most events from the stream
                // except for TaskSpawned (which we need to handle for early cancellation).
                let ignore_events =
                    matches!(me.status, Status::Cancelled { .. } | Status::Failed { .. });

                match event_result {
                    Ok(event) => match event {
                        AmbientAgentEvent::TaskSpawned { task_id, run_id } => {
                            // Store the task ID for later use (e.g., populating details panel)
                            me.task_id = Some(task_id);

                            // If we already transitioned to Cancelled state, stop processing the
                            // stale spawn event.
                            if matches!(me.status, Status::Cancelled { .. }) {
                                return;
                            }

                            // Wire the run_id to the associated conversation for
                            // orchestration v2. This unblocks the parent agent's
                            // pending start_agent tool call.
                            if let Some(conversation_id) = me.conversation_id {
                                let terminal_view_id = me.terminal_view_id;
                                let spawned_task_id = Some(task_id);
                                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                                    history.assign_run_id_for_conversation(
                                        conversation_id,
                                        run_id,
                                        spawned_task_id,
                                        terminal_view_id,
                                        ctx,
                                    );
                                });
                            }

                            // Mark the task as manually opened so it appears in the conversation list
                            // even though its server-side source may not be user-initiated.
                            AgentConversationsModel::handle(ctx).update(ctx, |model, ctx| {
                                model.mark_task_as_manually_opened(task_id, ctx);
                            });

                            // Emit event so terminal view knows to show the info button
                            ctx.emit(AmbientAgentViewModelEvent::ProgressUpdated);
                        }
                        AmbientAgentEvent::StateChanged {
                            state,
                            status_message,
                        } => {
                            // Ignore state changes if we're already in a terminal state
                            if ignore_events {
                                return;
                            }

                            if let Status::WaitingForSession { progress } = &mut me.status {
                                match state {
                                    AmbientAgentTaskState::Cancelled => {
                                        me.handle_cancellation(ctx);
                                    }
                                    AmbientAgentTaskState::Queued
                                    | AmbientAgentTaskState::Pending => {
                                        // Clear later states in case the agent failed to start and was retried.
                                        progress.claimed_at = None;
                                        progress.harness_started_at = None;
                                        ctx.emit(AmbientAgentViewModelEvent::ProgressUpdated);
                                    }
                                    AmbientAgentTaskState::Claimed => {
                                        if progress.claimed_at.is_none() {
                                            progress.claimed_at = Some(Instant::now());
                                            progress.harness_started_at = None;
                                            ctx.emit(AmbientAgentViewModelEvent::ProgressUpdated);
                                        }
                                    }
                                    AmbientAgentTaskState::InProgress => {
                                        if progress.harness_started_at.is_none() {
                                            progress.harness_started_at = Some(Instant::now());
                                            ctx.emit(AmbientAgentViewModelEvent::ProgressUpdated);
                                        }
                                    }
                                    AmbientAgentTaskState::Succeeded => {}
                                    AmbientAgentTaskState::Failed
                                    | AmbientAgentTaskState::Error
                                    | AmbientAgentTaskState::Blocked
                                    | AmbientAgentTaskState::Unknown => {
                                        let error = status_message
                                            .map(|msg| msg.message)
                                            .unwrap_or_else(|| "Agent failed".to_string());
                                        me.handle_spawn_error(error, ctx);
                                    }
                                }
                            }
                        }
                        AmbientAgentEvent::SessionStarted { .. } => {
                            // Ignore session started if we're already in a terminal state
                            if ignore_events {
                                return;
                            }

                            me.stop_progress_timer();
                            me.status = Status::AgentRunning;
                            ctx.emit(AmbientAgentViewModelEvent::SessionReady);
                        }
                        AmbientAgentEvent::AtCapacity => {
                            if ignore_events {
                                return;
                            }

                            if matches!(me.status, Status::WaitingForSession { .. }) {
                                // 去云端分支:不再展示 agent capacity 模态
                            }
                        }
                        AmbientAgentEvent::TimedOut => {}
                    },
                    Err(err) => {
                        // Ignore errors if we're already in a terminal state
                        if ignore_events {
                            return;
                        }
                        let error_message = err.to_string();

                        // Check if this is a ClientError with an auth_url
                        use crate::ai::api_error::ClientError;
                        if let Some(client_error) = err.downcast_ref::<ClientError>() {
                            if let Some(auth_url) = &client_error.auth_url {
                                me.handle_needs_github_auth(
                                    auth_url.clone(),
                                    client_error.error.clone(),
                                    ctx,
                                );
                                return;
                            }
                        }
                        if let Some(ai_api_error) = err.downcast_ref::<AIApiError>() {
                            match ai_api_error {
                                AIApiError::QuotaLimit => {
                                    me.handle_spawn_error(
                                        OUT_OF_CREDITS_TASK_FAILURE_MESSAGE.to_string(),
                                        ctx,
                                    );
                                    ctx.emit(AmbientAgentViewModelEvent::ShowAICreditModal);
                                    return;
                                }
                                AIApiError::ServerOverloaded => {
                                    me.handle_spawn_error(
                                        SERVER_OVERLOADED_TASK_FAILURE_MESSAGE.to_string(),
                                        ctx,
                                    );
                                    return;
                                }
                                _ => {}
                            }
                        }
                        me.handle_spawn_error(error_message, ctx);
                    }
                }
            },
            |_me, _ctx| {},
        );

        self.status = Status::WaitingForSession {
            progress: AgentProgress {
                spawned_at: Instant::now(),
                claimed_at: None,
                harness_started_at: None,
                stopped_at: None,
            },
        };
        self.start_progress_timer(ctx);
        ctx.emit(AmbientAgentViewModelEvent::DispatchedAgent);
    }

    /// Starts the periodic timer that updates the progress UI while waiting for a session.
    fn start_progress_timer(&mut self, ctx: &mut ModelContext<Self>) {
        // Don't start a new timer if one is already running.
        if self.progress_timer_handle.is_some() {
            return;
        }

        let handle = ctx.spawn(
            async move {
                Timer::after(Duration::from_millis(200)).await;
            },
            |me, _unit, ctx| {
                me.progress_timer_handle = None;

                // Check if still waiting for session.
                if matches!(me.status, Status::WaitingForSession { .. }) {
                    ctx.emit(AmbientAgentViewModelEvent::ProgressUpdated);
                    me.start_progress_timer(ctx);
                }
            },
        );

        self.progress_timer_handle = Some(handle);
    }

    fn stop_progress_timer(&mut self) {
        if let Some(handle) = self.progress_timer_handle.take() {
            handle.abort();
        }
    }

    /// Handles a spawn error by transitioning to the Failed state.
    fn handle_spawn_error(&mut self, error_message: String, ctx: &mut ModelContext<Self>) {
        self.stop_progress_timer();

        let now = Instant::now();

        // Extract or create progress tracking.
        let progress = if let Status::WaitingForSession { mut progress } =
            std::mem::replace(&mut self.status, Status::NotAmbientAgent)
        {
            progress.stopped_at = Some(now);
            progress
        } else {
            // If not in WaitingForSession, create a new progress with current time.
            AgentProgress {
                spawned_at: now,
                claimed_at: None,
                harness_started_at: None,
                stopped_at: Some(now),
            }
        };

        self.status = Status::Failed {
            progress,
            error_message: error_message.clone(),
        };
        ctx.emit(AmbientAgentViewModelEvent::Failed { error_message });
    }

    /// Handles the need for GitHub authentication by transitioning to the NeedsGithubAuth state.
    fn handle_needs_github_auth(
        &mut self,
        auth_url: String,
        error_message: String,
        ctx: &mut ModelContext<Self>,
    ) {
        self.stop_progress_timer();

        let now = Instant::now();

        // Extract or create progress tracking.
        let progress = if let Status::WaitingForSession { mut progress } =
            std::mem::replace(&mut self.status, Status::NotAmbientAgent)
        {
            progress.stopped_at = Some(now);
            progress
        } else {
            // If not in WaitingForSession, create a new progress with current time.
            AgentProgress {
                spawned_at: now,
                claimed_at: None,
                harness_started_at: None,
                stopped_at: Some(now),
            }
        };

        self.status = Status::NeedsGithubAuth {
            progress,
            error_message,
            auth_url,
        };

        ctx.emit(AmbientAgentViewModelEvent::NeedsGithubAuth);
    }

    /// Handles cancellation by transitioning to the Cancelled state.
    fn handle_cancellation(&mut self, ctx: &mut ModelContext<Self>) {
        self.stop_progress_timer();

        let now = Instant::now();

        // Extract or create progress tracking.
        let progress = if let Status::WaitingForSession { mut progress } =
            std::mem::replace(&mut self.status, Status::NotAmbientAgent)
        {
            progress.stopped_at = Some(now);
            progress
        } else {
            // If not in WaitingForSession, create a new progress with current time.
            AgentProgress {
                spawned_at: now,
                claimed_at: None,
                harness_started_at: None,
                stopped_at: Some(now),
            }
        };

        self.status = Status::Cancelled { progress };

        ctx.emit(AmbientAgentViewModelEvent::Cancelled);
    }

    /// Cancels the ambient agent task if one is currently running.
    /// Sends a cancellation request to the server (if task_id is available) and transitions to the Cancelled state.
    pub fn cancel_task(&mut self, ctx: &mut ModelContext<Self>) {
        if !self.is_waiting_for_session() {
            log::warn!("Attempted to cancel ambient agent task but not in WaitingForSession state");
            return;
        }

        if self.task_id.is_none() {
            log::info!("Cancelling ambient agent task before task_id was received");
        }

        // Always transition to cancelled state immediately, regardless of whether we have a task_id.
        // This provides immediate UI feedback to the user.
        self.handle_cancellation(ctx);
    }
}

/// Events emitted by the ambient agent view model.
#[derive(Debug, Clone)]
pub enum AmbientAgentViewModelEvent {
    /// The user has entered the setup state (first-time environment creation).
    EnteredSetupState,
    /// The user has entered the composing state (typing their prompt).
    EnteredComposingState,
    /// The ambient agent run has been dispatched.
    DispatchedAgent,
    /// The spawn progress has been updated (e.g., task claimed or in-progress).
    ProgressUpdated,
    /// The ambient agent has started sharing its session.
    SessionReady,
    /// The ambient agent failed.
    Failed { error_message: String },
    /// Request to show the agent credits modal.
    ShowAICreditModal,
    /// The ambient agent needs GitHub authentication.
    NeedsGithubAuth,
    /// The ambient agent was cancelled.
    Cancelled,
    /// The selected execution harness (Oz / Claude Code) changed.
    HarnessSelected,
    /// The harness CLI (for non-oz runs) has started executing in the shared session.
    /// Fires once per run and signals the transition out of the pre-first-exchange phase
    /// for claude / gemini / other third-party harnesses.
    HarnessCommandStarted,
}

impl Entity for AmbientAgentViewModel {
    type Event = AmbientAgentViewModelEvent;
}
