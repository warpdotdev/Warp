//! Inline view for the orchestrate (`RunAgents`) confirmation card.
//!
//! Each card is a `View` keyed by `AIAgentActionId`, embedded by
//! `AIBlock` via `ChildView`. Keybindings and Accept dispatch live on
//! the view; only `RejectRequested` flows back to the parent.
use ai::agent::action::{RunAgentsAgentRunConfig, RunAgentsExecutionMode, RunAgentsRequest};
use ai::agent::action_result::{RunAgentsAgentOutcomeKind, RunAgentsResult};
use ai::agent::orchestration_config::{
    matches_active_config, OrchestrationConfig, OrchestrationConfigStatus,
};
use ai::skills::SkillReference;
use pathfinder_geometry::vector::vec2f;
use std::rc::Rc;
use warpui::elements::{
    Border, ChildView, Container, CornerRadius, CrossAxisAlignment, Empty, Flex, MainAxisSize,
    OffsetPositioning, ParentElement, Radius, Stack, Text,
};
use warpui::keymap::{FixedBinding, Keystroke};
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::ai::agent::icons;
use crate::ai::agent::{AIAgentActionId, AIAgentActionResultType};
use crate::ai::blocklist::action_model::{
    AIActionStatus, BlocklistAIActionEvent, BlocklistAIActionModel, RunAgentsExecutor,
    RunAgentsExecutorEvent, RunAgentsSpawningSnapshot,
};
use crate::ai::blocklist::agent_view::orchestration_pill_bar::render_static_agent_pill;
use crate::ai::blocklist::block::model::AIBlockModel;
use crate::ai::blocklist::block::view_impl::WithContentItemSpacing;
use crate::ai::blocklist::block::AIBlock;
use crate::ai::blocklist::inline_action::inline_action_header::{HeaderConfig, InteractionMode};
use crate::ai::blocklist::inline_action::inline_action_icons;
use crate::ai::blocklist::inline_action::orchestration_controls::{
    self as oc, OrchestrationControlAction, OrchestrationPickerHandles,
};
use crate::ai::blocklist::inline_action::requested_action::{
    render_requested_action_row_for_text, CTRL_C_KEYSTROKE, ENTER_KEYSTROKE,
};
use crate::ai::llms::{LLMPreferences, LLMPreferencesEvent};
use crate::appearance::Appearance;
use crate::menu::{Event as MenuEvent, Menu, MenuItemFields, MenuVariant};
use crate::ui_components::blended_colors;
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{ButtonSize, KeystrokeSource, NakedTheme};
use crate::view_components::compactible_action_button::{
    CompactibleActionButton, RenderCompactibleActionButton, MEDIUM_SIZE_SWITCH_THRESHOLD,
};
use crate::view_components::compactible_split_action_button::CompactibleSplitActionButton;
use crate::view_components::dropdown::DropdownEvent;
use crate::view_components::FilterableDropdownEvent;

const RUN_AGENTS_CARD_TITLE: &str = "Can I add additional agents to this task?";

const RUN_AGENTS_EDITOR_OPEN: &str = "RunAgentsEditorOpen";

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([
        FixedBinding::new(
            "enter",
            RunAgentsCardViewAction::Accept,
            id!(RunAgentsCardView::ui_name()),
        ),
        FixedBinding::new(
            "numpadenter",
            RunAgentsCardViewAction::Accept,
            id!(RunAgentsCardView::ui_name()),
        ),
        FixedBinding::new(
            "ctrl-c",
            RunAgentsCardViewAction::Reject,
            id!(RunAgentsCardView::ui_name()),
        ),
        FixedBinding::new(
            "cmdorctrl-e",
            RunAgentsCardViewAction::ToggleEdit,
            id!(RunAgentsCardView::ui_name()),
        ),
        // Esc closes the editor; Reject is Ctrl-C.
        FixedBinding::new(
            "escape",
            RunAgentsCardViewAction::DiscardEdits,
            id!(RunAgentsCardView::ui_name()) & id!(RUN_AGENTS_EDITOR_OPEN),
        ),
    ]);
}

/// Per-action edit state for the orchestrate confirmation card.
/// Delegates run-wide config fields to `oc::OrchestrationEditState`
/// and adds card-specific fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAgentsEditState {
    pub is_editor_open: bool,
    pub orch: oc::OrchestrationEditState,
    pub agent_run_configs: Vec<RunAgentsAgentRunConfig>,
    pub base_prompt: String,
    pub summary: String,
    /// Run-wide skills propagated to each child at dispatch.
    pub skills: Vec<SkillReference>,
}

impl RunAgentsEditState {
    pub fn from_request(req: &RunAgentsRequest) -> Self {
        Self {
            is_editor_open: false,
            orch: oc::OrchestrationEditState::from_run_agents_fields(
                &req.model_id,
                &req.harness_type,
                &req.execution_mode,
            ),
            agent_run_configs: req.agent_run_configs.clone(),
            base_prompt: req.base_prompt.clone(),
            summary: req.summary.clone(),
            skills: req.skills.clone(),
        }
    }

    pub fn to_request(&self) -> RunAgentsRequest {
        RunAgentsRequest {
            summary: self.summary.clone(),
            base_prompt: self.base_prompt.clone(),
            skills: self.skills.clone(),
            model_id: self.orch.model_id.clone(),
            harness_type: self.orch.harness_type.clone(),
            execution_mode: self.orch.execution_mode.clone(),
            agent_run_configs: self.agent_run_configs.clone(),
        }
    }
}

impl OrchestrationControlAction for RunAgentsCardViewAction {
    fn execution_mode_toggled(is_remote: bool) -> Self {
        Self::ExecutionModeToggled { is_remote }
    }
    fn model_changed(model_id: String) -> Self {
        Self::ModelChanged { model_id }
    }
    fn harness_changed(harness_type: String) -> Self {
        Self::HarnessChanged { harness_type }
    }
    fn environment_changed(environment_id: String) -> Self {
        Self::EnvironmentChanged { environment_id }
    }
    fn worker_host_changed(worker_host: String) -> Self {
        Self::WorkerHostChanged { worker_host }
    }
}

/// Per-action UI handles. Picker views are lazily created on first
/// editor open.
#[derive(Default, Clone)]
struct RunAgentsCardHandles {
    reject_button: Option<CompactibleActionButton>,
    edit_button: Option<CompactibleActionButton>,
    accept_button: Option<CompactibleSplitActionButton>,
    pickers: OrchestrationPickerHandles<RunAgentsCardViewAction>,
}

#[derive(Clone, Debug)]
pub enum RunAgentsCardViewAction {
    Accept,
    AcceptWithoutOrchestration,
    ToggleAcceptMenu,
    Reject,
    ToggleEdit,
    DiscardEdits,
    ExecutionModeToggled { is_remote: bool },
    ModelChanged { model_id: String },
    HarnessChanged { harness_type: String },
    EnvironmentChanged { environment_id: String },
    WorkerHostChanged { worker_host: String },
}

#[derive(Clone, Debug)]
pub enum RunAgentsCardViewEvent {
    RejectRequested,
}

pub struct RunAgentsCardView {
    action_id: AIAgentActionId,
    state: RunAgentsEditState,
    /// Snapshot of the request as received from the tool call, used to
    /// reset on "Discard edits".
    original_request: RunAgentsRequest,
    handles: RunAgentsCardHandles,
    spawning: Option<RunAgentsSpawningSnapshot>,
    /// Set when the active config was approved and matched the request,
    /// causing immediate dispatch without user confirmation.
    auto_launched: bool,
    /// Set when the action has a `RunAgentsResult::Denied` result in
    /// history (e.g. orchestration was disabled at dispatch time).
    is_denied: bool,
    /// Retained from construction so `update_request()` can re-evaluate
    /// the auto-launch condition when `agent_run_configs` arrives via
    /// streaming after the initial empty chunk.
    active_config: Option<(OrchestrationConfig, OrchestrationConfigStatus)>,

    // Split-button accept menu state
    is_accept_menu_open: bool,
    accept_menu: ViewHandle<Menu<RunAgentsCardViewAction>>,
    position_id_prefix: String,

    action_model: ModelHandle<BlocklistAIActionModel>,
    block_model: Rc<dyn AIBlockModel<View = AIBlock>>,
}

/// Returns `true` when the conditions for auto-launching are met.
///
/// Extracted from `try_auto_launch_on_stream_complete` so the
/// decision logic can be unit-tested without constructing a full
/// `RunAgentsCardView`.
pub(crate) fn should_auto_launch(
    auto_launched: bool,
    is_denied: bool,
    is_spawning: bool,
    state: &RunAgentsEditState,
    active_config: &Option<(OrchestrationConfig, OrchestrationConfigStatus)>,
) -> bool {
    if auto_launched
        || is_denied
        || is_spawning
        || state.is_editor_open
        || state.agent_run_configs.is_empty()
    {
        return false;
    }
    match active_config {
        Some((config, status)) => {
            let request = state.to_request();
            status.is_approved() && matches_active_config(&request, config)
        }
        None => false,
    }
}

/// Computes the `is_denied` flag at construction time.
///
/// The card is denied when either the action already has a `Denied`
/// result in history *or* the active config is explicitly disapproved.
pub(crate) fn compute_is_denied(
    has_denied_result: bool,
    active_config: &Option<(OrchestrationConfig, OrchestrationConfigStatus)>,
) -> bool {
    has_denied_result
        || matches!(
            active_config,
            Some((_, status)) if status.is_disapproved()
        )
}

fn is_opencode_on_remote(request: &RunAgentsRequest) -> bool {
    matches!(
        request.execution_mode,
        RunAgentsExecutionMode::Remote { .. }
    ) && request.harness_type.eq_ignore_ascii_case("opencode")
}

impl RunAgentsCardView {
    pub fn new(
        action_id: AIAgentActionId,
        request: &RunAgentsRequest,
        active_config: Option<(OrchestrationConfig, OrchestrationConfigStatus)>,
        action_model: ModelHandle<BlocklistAIActionModel>,
        run_agents_executor: ModelHandle<RunAgentsExecutor>,
        block_model: Rc<dyn AIBlockModel<View = AIBlock>>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        // Detect an existing Denied result from history (e.g. restored
        // conversation where orchestration was disabled).
        let is_denied = if let Some(AIActionStatus::Finished(result)) =
            action_model.as_ref(ctx).get_action_status(&action_id)
        {
            matches!(
                &result.result,
                AIAgentActionResultType::RunAgents(RunAgentsResult::Denied { .. })
            )
        } else {
            false
        };

        // Auto-launch when the active config is approved and matches
        // the request — skip the confirmation card entirely.
        // The active_config is now conversation-scoped so cross-conversation
        // leakage is no longer possible.
        // Also treat the action as denied when the config is explicitly
        // disapproved — the card will auto-deny via the subscription
        // once the action becomes blocked.
        let is_denied = compute_is_denied(is_denied, &active_config);

        let state = RunAgentsEditState::from_request(request);
        let auto_launched = should_auto_launch(false, is_denied, false, &state, &active_config);

        let reject_keystroke = CTRL_C_KEYSTROKE.clone();
        let edit_keystroke =
            Keystroke::parse("cmdorctrl-e").expect("orchestrate edit keystroke literal must parse");
        let accept_keystroke = ENTER_KEYSTROKE.clone();

        let reject_button = CompactibleActionButton::new(
            "Reject".to_string(),
            Some(KeystrokeSource::Fixed(reject_keystroke)),
            ButtonSize::Small,
            RunAgentsCardViewAction::Reject,
            Icon::X,
            std::sync::Arc::new(NakedTheme),
            ctx,
        );
        let edit_button = CompactibleActionButton::new(
            "Edit".to_string(),
            Some(KeystrokeSource::Fixed(edit_keystroke)),
            ButtonSize::Small,
            RunAgentsCardViewAction::ToggleEdit,
            Icon::Pencil,
            std::sync::Arc::new(NakedTheme),
            ctx,
        );
        let position_id_prefix = format!("{action_id:?}");
        let accept_button = CompactibleSplitActionButton::new(
            "Accept".to_string(),
            Some(KeystrokeSource::Fixed(accept_keystroke)),
            ButtonSize::Small,
            RunAgentsCardViewAction::Accept,
            RunAgentsCardViewAction::ToggleAcceptMenu,
            Icon::Check,
            true,
            Some(Self::get_position_id_for_accept_split_button(
                &position_id_prefix,
            )),
            ctx,
        );

        let accept_menu = ctx.add_typed_action_view(|ctx| {
            let theme = Appearance::as_ref(ctx).theme();
            Menu::new()
                .with_menu_variant(MenuVariant::Fixed)
                .with_border(Border::all(1.).with_border_fill(theme.outline()))
                .prevent_interaction_with_other_elements()
        });
        ctx.subscribe_to_view(&accept_menu, |me, _menu, event, ctx| match event {
            MenuEvent::Close { .. } => {
                me.is_accept_menu_open = false;
                ctx.notify();
            }
            MenuEvent::ItemSelected | MenuEvent::ItemHovered => {}
        });

        let action_id_for_subscription = action_id.clone();
        ctx.subscribe_to_model(&run_agents_executor, move |me, _, event, ctx| match event {
            RunAgentsExecutorEvent::SpawningStarted {
                action_id,
                snapshot,
            } if action_id == &action_id_for_subscription => {
                me.spawning = Some(*snapshot);
                ctx.notify();
            }
            RunAgentsExecutorEvent::SpawningFinished { action_id }
                if action_id == &action_id_for_subscription =>
            {
                me.spawning = None;
                ctx.notify();
            }
            RunAgentsExecutorEvent::SpawningStarted { .. }
            | RunAgentsExecutorEvent::SpawningFinished { .. } => {}
        });

        // Re-render when this action finishes or becomes blocked.
        // When `auto_launched` is true and the action becomes blocked,
        // dispatch `execute_run_agents` — the deferred auto-launch
        // only sets the flag and shows the spawning UI; the actual
        // execution must wait until the action model has queued the
        // action.
        let action_id_for_action_events = action_id.clone();
        ctx.subscribe_to_model(&action_model, move |me, _, event, ctx| match event {
            BlocklistAIActionEvent::FinishedAction { action_id, .. }
                if action_id == &action_id_for_action_events =>
            {
                ctx.notify();
            }
            BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(action_id)
                if action_id == &action_id_for_action_events && me.is_denied =>
            {
                let action_id = me.action_id.clone();
                me.action_model.update(ctx, |action_model, action_ctx| {
                    action_model.deny_run_agents(&action_id, String::new(), action_ctx);
                });
            }
            BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(action_id)
                if action_id == &action_id_for_action_events && me.auto_launched =>
            {
                let request = me.state.to_request();
                let action_id = me.action_id.clone();
                me.action_model.update(ctx, |action_model, action_ctx| {
                    action_model.execute_run_agents(&action_id, request, action_ctx);
                });
            }
            _ => {}
        });

        // Repopulate the model picker when available LLMs change.
        // LLMPreferences loads asynchronously from the server; the
        // picker may have been created before models arrived.
        ctx.subscribe_to_model(&LLMPreferences::handle(ctx), |me, _, event, ctx| {
            if let LLMPreferencesEvent::UpdatedAvailableLLMs = event {
                if let Some(handle) = &me.handles.pickers.model_picker {
                    oc::populate_model_picker(handle, &me.state.orch.model_id, ctx);
                }
            }
        });

        // When auto_launched is true, execution is deferred to the
        // ActionBlockedOnUserConfirmation subscription above — the action
        // hasn't been queued in pending_actions yet at construction time.
        Self {
            action_id,
            state: RunAgentsEditState::from_request(request),
            original_request: request.clone(),
            handles: RunAgentsCardHandles {
                reject_button: Some(reject_button),
                edit_button: Some(edit_button),
                accept_button: Some(accept_button),
                ..Default::default()
            },
            spawning: None,
            auto_launched,
            is_denied,
            active_config,
            is_accept_menu_open: false,
            accept_menu,
            position_id_prefix,
            action_model,
            block_model,
        }
    }

    pub fn is_spawning(&self) -> bool {
        self.spawning.is_some()
    }

    /// Re-sync edit state from the latest streaming request.
    /// No-op when the editor is open (user edits take precedence).
    pub fn update_request(&mut self, request: &RunAgentsRequest, ctx: &mut ViewContext<Self>) {
        if self.state.is_editor_open
            || self.spawning.is_some()
            || self.auto_launched
            || self.is_denied
        {
            return;
        }
        let new_state = RunAgentsEditState::from_request(request);
        if self.state != new_state {
            self.state = new_state;
            self.original_request = request.clone();
            ctx.notify();
        }
    }

    /// Re-evaluate auto-launch after the output stream has finished and
    /// the request is fully populated.  Called from
    /// `AIBlock::handle_complete_output` so we don't act on partial
    /// streaming chunks that arrive with an empty `agent_run_configs`.
    pub fn try_auto_launch_on_stream_complete(&mut self, ctx: &mut ViewContext<Self>) {
        if should_auto_launch(
            self.auto_launched,
            self.is_denied,
            self.spawning.is_some(),
            &self.state,
            &self.active_config,
        ) {
            self.auto_launched = true;
            // Don't call execute_run_agents here — the action
            // hasn't been queued as Blocked yet. The subscription
            // on ActionBlockedOnUserConfirmation will dispatch it
            // once the action model is ready.
            ctx.notify();
        }
    }

    /// Validates and dispatches the resolved request.
    pub fn accept(&mut self, ctx: &mut ViewContext<Self>) {
        self.handle_accept(ctx);
    }

    fn handle_accept(&mut self, ctx: &mut ViewContext<Self>) {
        if self.spawning.is_some() {
            return;
        }
        let request = self.state.to_request();
        if is_opencode_on_remote(&request) {
            log::warn!(
                "RunAgentsCardView: refusing Accept for OpenCode+Cloud (unsupported per spec)"
            );
            return;
        }
        // Close the editor before dispatching.
        if self.state.is_editor_open {
            self.state.is_editor_open = false;
            self.sync_card_buttons(ctx);
        }
        let action_id = self.action_id.clone();
        self.action_model.update(ctx, |action_model, action_ctx| {
            action_model.execute_run_agents(&action_id, request, action_ctx);
        });
    }

    fn handle_toggle_edit(&mut self, ctx: &mut ViewContext<Self>) {
        self.state.is_editor_open = !self.state.is_editor_open;

        // Lazily build picker views on first editor open.
        if self.state.is_editor_open {
            self.ensure_pickers(ctx);
        }

        self.sync_card_buttons(ctx);
        ctx.notify();
    }

    /// Swap Edit ↔ "Discard edits" label/keystroke.
    fn sync_card_buttons(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(edit_button) = self.handles.edit_button.as_mut() else {
            return;
        };
        let (label, keystroke) = if self.state.is_editor_open {
            (
                "Discard edits".to_string(),
                Keystroke::parse("escape")
                    .expect("orchestrate discard-edits keystroke literal must parse"),
            )
        } else {
            (
                "Edit".to_string(),
                Keystroke::parse("cmdorctrl-e")
                    .expect("orchestrate edit keystroke literal must parse"),
            )
        };
        edit_button.set_label(label, ctx);
        edit_button.set_keybinding(Some(KeystrokeSource::Fixed(keystroke)), ctx);
    }

    /// Lazily construct the picker dropdown views (idempotent).
    fn ensure_pickers(&mut self, ctx: &mut ViewContext<Self>) {
        let appearance = Appearance::as_ref(ctx);
        let (styles, colors) = oc::picker_styles(appearance);

        let initial_model_id_default = self
            .block_model
            .base_model(ctx)
            .map(|id| id.to_string())
            .unwrap_or_default();
        let state = &self.state;

        if self.handles.pickers.model_picker.is_none() {
            let initial_model_id = if state.orch.model_id.trim().is_empty() {
                initial_model_id_default.clone()
            } else {
                state.orch.model_id.clone()
            };
            let handle = oc::new_standard_picker_dropdown(&colors, ctx);
            oc::populate_model_picker(&handle, &initial_model_id, ctx);
            Self::subscribe_picker_close(&handle, ctx);
            self.handles.pickers.model_picker = Some(handle);
        }

        if self.handles.pickers.harness_picker.is_none() {
            let handle = oc::new_standard_picker_dropdown(&colors, ctx);
            oc::populate_harness_picker(&handle, &state.orch.harness_type, ctx);
            Self::subscribe_picker_close(&handle, ctx);
            self.handles.pickers.harness_picker = Some(handle);
        }

        if self.handles.pickers.environment_picker.is_none() {
            let initial_env = match &state.orch.execution_mode {
                RunAgentsExecutionMode::Remote { environment_id, .. } => environment_id.as_str(),
                RunAgentsExecutionMode::Local => "",
            };
            let handle = oc::create_environment_picker(initial_env, &styles, ctx);
            ctx.subscribe_to_view(&handle, |me, _, event, ctx| {
                if let FilterableDropdownEvent::Close = event {
                    me.refocus_after_picker_close(ctx);
                }
            });
            self.handles.pickers.environment_picker = Some(handle);
        }

        if self.handles.pickers.host_picker.is_none() {
            let initial_host = match &state.orch.execution_mode {
                RunAgentsExecutionMode::Remote { worker_host, .. } => worker_host.as_str(),
                RunAgentsExecutionMode::Local => oc::ORCHESTRATION_WARP_WORKER_HOST,
            };
            let handle = oc::new_standard_picker_dropdown(&colors, ctx);
            oc::populate_host_picker(&handle, initial_host, ctx);
            Self::subscribe_picker_close(&handle, ctx);
            self.handles.pickers.host_picker = Some(handle);
        }

        self.sync_picker_selections(ctx);
    }

    fn subscribe_picker_close(
        dropdown_handle: &ViewHandle<
            crate::view_components::dropdown::Dropdown<RunAgentsCardViewAction>,
        >,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.subscribe_to_view(dropdown_handle, move |me, _, event, ctx| {
            if let DropdownEvent::Close = event {
                me.refocus_after_picker_close(ctx);
            }
        });
    }

    fn refocus_after_picker_close(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn sync_picker_selections(&mut self, ctx: &mut ViewContext<Self>) {
        oc::sync_picker_selections(&self.state.orch, &self.handles.pickers, ctx);
    }

    fn toggle_accept_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.is_accept_menu_open = !self.is_accept_menu_open;
        if self.is_accept_menu_open {
            let item = MenuItemFields::new_with_label("Accept w/o orchestration", "")
                .with_on_select_action(RunAgentsCardViewAction::AcceptWithoutOrchestration)
                .into_item();
            self.accept_menu.update(ctx, |menu, ctx| {
                menu.set_items(vec![item], ctx);
            });
            self.accept_menu
                .update(ctx, |menu, ctx| menu.set_selected_by_index(0, ctx));
            ctx.focus(&self.accept_menu);
        }
        ctx.notify();
    }

    fn get_position_id_for_accept_split_button(prefix: &str) -> String {
        format!("RunAgentsCardView-{prefix}-accept-split")
    }
}

impl Entity for RunAgentsCardView {
    type Event = RunAgentsCardViewEvent;
}

impl View for RunAgentsCardView {
    fn ui_name() -> &'static str {
        "RunAgentsCardView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let status = self
            .action_model
            .as_ref(app)
            .get_action_status(&self.action_id);

        if let Some(AIActionStatus::Finished(result)) = &status {
            if let AIAgentActionResultType::RunAgents(orchestrate_result) = &result.result {
                return render_terminal_state(orchestrate_result, appearance, app);
            }
            log::error!(
                "Unexpected action result type for orchestrate: {:?}",
                result.result
            );
            return Empty::new().finish();
        }

        // Denied at construction — render static disabled card.
        if self.is_denied {
            return render_status_only_card(
                "Orchestration is currently disabled. Re-enable on the plan card to launch."
                    .to_string(),
                appearance,
                StatusKind::Cancelled,
                app,
            );
        }

        // In-flight dispatch: check both spawning snapshot and action
        // status because the event arrives one tick after the status.
        if let Some(snapshot) = &self.spawning {
            return render_spawning_card(snapshot, appearance, app);
        }
        if matches!(status, Some(AIActionStatus::RunningAsync)) {
            let snapshot = RunAgentsSpawningSnapshot {
                agent_count: self.state.agent_run_configs.len(),
            };
            return render_spawning_card(&snapshot, appearance, app);
        }

        // Auto-launched: show spawning card while dispatch is in
        // flight (before the executor fires the SpawningStarted event).
        if self.auto_launched {
            let snapshot = RunAgentsSpawningSnapshot {
                agent_count: self.state.agent_run_configs.len(),
            };
            return render_spawning_card(&snapshot, appearance, app);
        }

        // Restored-from-history: dispatch state is lost, render as
        // Cancelled.
        if self.block_model.is_restored() {
            return render_status_only_card(
                "Spawn agents cancelled".to_string(),
                appearance,
                StatusKind::Cancelled,
                app,
            );
        }

        let is_blocked = matches!(status, Some(AIActionStatus::Blocked));
        let card = render_confirmation_card(&self.state, &self.handles, is_blocked, app);

        let mut root_stack = Stack::new();
        root_stack.add_child(card);

        if self.is_accept_menu_open {
            root_stack.add_positioned_child(
                ChildView::new(&self.accept_menu).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    Self::get_position_id_for_accept_split_button(&self.position_id_prefix),
                    vec2f(0., 8.),
                    warpui::elements::PositionedElementOffsetBounds::WindowByPosition,
                    warpui::elements::PositionedElementAnchor::BottomRight,
                    warpui::elements::ChildAnchor::TopRight,
                ),
            );
        }

        root_stack.finish()
    }

    fn keymap_context(&self, _app: &AppContext) -> warpui::keymap::Context {
        let mut context = Self::default_keymap_context();
        if self.state.is_editor_open {
            context.set.insert(RUN_AGENTS_EDITOR_OPEN);
        }
        context
    }
}

impl TypedActionView for RunAgentsCardView {
    type Action = RunAgentsCardViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            RunAgentsCardViewAction::Accept => {
                self.handle_accept(ctx);
            }
            RunAgentsCardViewAction::AcceptWithoutOrchestration => {
                let action_id = self.action_id.clone();
                self.action_model.update(ctx, |action_model, action_ctx| {
                    action_model.deny_run_agents(&action_id, String::new(), action_ctx);
                });
            }
            RunAgentsCardViewAction::ToggleAcceptMenu => {
                self.toggle_accept_menu(ctx);
            }
            RunAgentsCardViewAction::Reject => {
                ctx.emit(RunAgentsCardViewEvent::RejectRequested);
            }
            RunAgentsCardViewAction::ToggleEdit => {
                self.handle_toggle_edit(ctx);
            }
            RunAgentsCardViewAction::DiscardEdits => {
                if self.state.is_editor_open {
                    // Reset to the original tool-call values.
                    self.state = RunAgentsEditState::from_request(&self.original_request);
                    self.sync_card_buttons(ctx);
                    self.sync_picker_selections(ctx);
                    ctx.notify();
                }
            }
            RunAgentsCardViewAction::ExecutionModeToggled { is_remote } => {
                self.state.orch.toggle_execution_mode_to_remote(*is_remote);
                self.sync_picker_selections(ctx);
                ctx.notify();
            }
            RunAgentsCardViewAction::ModelChanged { model_id } => {
                self.state.orch.model_id = model_id.clone();
                ctx.notify();
            }
            RunAgentsCardViewAction::HarnessChanged { harness_type } => {
                self.state.orch.harness_type = harness_type.clone();
                ctx.notify();
            }
            RunAgentsCardViewAction::EnvironmentChanged { environment_id } => {
                self.state.orch.set_environment_id(environment_id.clone());
                ctx.notify();
            }
            RunAgentsCardViewAction::WorkerHostChanged { worker_host } => {
                self.state.orch.set_worker_host(worker_host.clone());
                ctx.notify();
            }
        }
    }
}

fn render_confirmation_card(
    state: &RunAgentsEditState,
    handles: &RunAgentsCardHandles,
    is_blocked: bool,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();

    let header = render_header(handles, app);
    let body = render_body(state, app);

    let mut content = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(header)
        .with_child(body);

    if state.is_editor_open {
        content.add_child(render_editor(state, handles, app));
    }

    let border_color = if is_blocked {
        theme.accent()
    } else {
        theme.surface_2()
    };

    Container::new(content.finish())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .with_border(Border::all(1.).with_border_fill(border_color))
        .finish()
        .with_content_item_spacing()
        .finish()
}

fn render_header(handles: &RunAgentsCardHandles, app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let mut config = HeaderConfig::new(RUN_AGENTS_CARD_TITLE, app)
        .with_icon(icons::yellow_stop_icon(appearance))
        .with_corner_radius_override(CornerRadius::with_top(Radius::Pixels(8.)));

    if let (Some(reject), Some(edit), Some(accept)) = (
        handles.reject_button.as_ref(),
        handles.edit_button.as_ref(),
        handles.accept_button.as_ref(),
    ) {
        let action_buttons: Vec<Rc<dyn RenderCompactibleActionButton>> = vec![
            Rc::new(reject.clone()),
            Rc::new(edit.clone()),
            Rc::new(accept.clone()),
        ];
        config = config.with_interaction_mode(InteractionMode::ActionButtons {
            action_buttons,
            size_switch_threshold: MEDIUM_SIZE_SWITCH_THRESHOLD,
        });
    }

    config.render(app)
}

fn render_body(state: &RunAgentsEditState, app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    column.add_child(render_summary(state, appearance));
    column.add_child(render_agents_section(state, app));

    Container::new(column.finish())
        .with_horizontal_padding(16.)
        .with_vertical_padding(12.)
        .with_background_color(theme.background().into_solid())
        .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(8.)))
        .finish()
}

fn render_summary(state: &RunAgentsEditState, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let summary = if state.summary.trim().is_empty() {
        format!(
            "Spawn {} agent(s) to address this task.",
            state.agent_run_configs.len()
        )
    } else {
        state.summary.clone()
    };
    let summary_text = Text::new(
        summary,
        appearance.ui_font_family(),
        appearance.monospace_font_size(),
    )
    .with_color(blended_colors::text_main(theme, theme.background()))
    .with_selectable(true)
    .finish();

    Container::new(summary_text)
        .with_margin_bottom(12.)
        .finish()
}

fn render_agents_section(state: &RunAgentsEditState, app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let label = Text::new(
        format!("Agents ({})", state.agent_run_configs.len()),
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(blended_colors::text_disabled(theme, theme.background()))
    .finish();

    let mut pills_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_spacing(4.);
    for cfg in &state.agent_run_configs {
        pills_row.add_child(render_static_agent_pill(&cfg.name, app));
    }

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(Container::new(label).with_margin_bottom(6.).finish())
        .with_child(pills_row.finish())
        .finish()
}

fn render_terminal_state(
    result: &RunAgentsResult,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let (label, kind) = format_terminal_state(result);
    render_status_only_card(label, appearance, kind, app)
}

pub(crate) fn format_terminal_state(result: &RunAgentsResult) -> (String, StatusKind) {
    match result {
        RunAgentsResult::Launched { agents, .. } => {
            let total = agents.len();
            let launched = agents
                .iter()
                .filter(|a| matches!(a.kind, RunAgentsAgentOutcomeKind::Launched { .. }))
                .count();
            let label = if launched == total {
                if total == 1 {
                    "Spawned 1 agent".to_string()
                } else {
                    format!("Spawned {total} agents")
                }
            } else {
                format!("Spawned {launched} of {total} agents")
            };
            let kind = if launched == total {
                StatusKind::Success
            } else {
                StatusKind::Mixed
            };
            (label, kind)
        }
        RunAgentsResult::Denied { reason } => {
            let body = if reason.is_empty() {
                "Orchestration is currently disabled. Re-enable on the plan card to launch."
                    .to_string()
            } else {
                format!(
                    "Orchestration is currently disabled. Re-enable on the plan card to launch. ({reason})"
                )
            };
            (body, StatusKind::Cancelled)
        }
        RunAgentsResult::Failure { error } => {
            let label = if error.is_empty() {
                "Failed to start orchestration".to_string()
            } else {
                format!("Failed to start orchestration: {error}")
            };
            (label, StatusKind::Failure)
        }
        RunAgentsResult::Cancelled => ("Spawn agents cancelled".to_string(), StatusKind::Cancelled),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum StatusKind {
    Spawning,
    Success,
    Mixed,
    Failure,
    Cancelled,
}

fn render_spawning_card(
    snapshot: &RunAgentsSpawningSnapshot,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let total = snapshot.agent_count;
    let label = if total == 1 {
        "Spawning 1 agent\u{2026}".to_string()
    } else {
        format!("Spawning {total} agents\u{2026}")
    };
    render_status_only_card(label, appearance, StatusKind::Spawning, app)
}

fn render_status_only_card(
    label: String,
    appearance: &Appearance,
    kind: StatusKind,
    app: &AppContext,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let icon = match kind {
        StatusKind::Spawning | StatusKind::Mixed => icons::yellow_running_icon(appearance).finish(),
        StatusKind::Success => inline_action_icons::green_check_icon(appearance).finish(),
        StatusKind::Failure => inline_action_icons::red_x_icon(appearance).finish(),
        StatusKind::Cancelled => inline_action_icons::cancelled_icon(appearance).finish(),
    };
    let row = render_requested_action_row_for_text(
        label.into(),
        appearance.ui_font_family(),
        Some(icon),
        None,
        false,
        false,
        app,
    );
    Container::new(row)
        .with_background_color(blended_colors::neutral_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
        .with_agent_output_item_spacing(app)
        .finish()
}

fn render_editor(
    state: &RunAgentsEditState,
    handles: &RunAgentsCardHandles,
    app: &AppContext,
) -> Box<dyn Element> {
    use warpui::elements::ConstrainedBox;
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    let divider = Container::new(
        ConstrainedBox::new(Empty::new().finish())
            .with_height(1.)
            .finish(),
    )
    .with_background_color(theme.surface_2().into_solid())
    .finish();
    column.add_child(divider);

    column.add_child(
        Container::new(oc::render_mode_toggle(
            state.orch.execution_mode.is_remote(),
            &handles.pickers,
            appearance,
            None,
            false,
        ))
        .with_margin_top(12.)
        .finish(),
    );
    column.add_child(oc::render_picker_row(
        &state.orch,
        &handles.pickers,
        appearance,
    ));

    if let Some(reason) = state.orch.accept_disabled_reason() {
        column.add_child(oc::render_validation_error(
            reason,
            theme.ui_error_color(),
            appearance,
        ));
    } else if let Some(message) =
        oc::empty_env_recommendation_message(&state.orch.execution_mode, app)
    {
        column.add_child(oc::render_validation_error(
            message,
            theme.ui_warning_color(),
            appearance,
        ));
    }

    Container::new(column.finish())
        .with_horizontal_padding(16.)
        .with_padding_bottom(12.)
        .with_background_color(theme.background().into_solid())
        .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(8.)))
        .finish()
}

#[cfg(test)]
#[path = "run_agents_card_view_tests.rs"]
mod tests;
