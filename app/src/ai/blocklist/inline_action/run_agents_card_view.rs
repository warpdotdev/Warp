//! Inline view for the orchestrate (`RunAgents`) confirmation card.
//!
//! Each card is a `View` keyed by `AIAgentActionId`, embedded by
//! `AIBlock` via `ChildView`. Keybindings and Accept dispatch live on
//! the view; only `RejectRequested` flows back to the parent.
use ai::agent::action::{RunAgentsAgentRunConfig, RunAgentsExecutionMode, RunAgentsRequest};
use ai::agent::action_result::{RunAgentsAgentOutcomeKind, RunAgentsResult};
use ai::skills::SkillReference;
use pathfinder_color::ColorU;
use std::rc::Rc;
use warpui::elements::{
    Border, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
    Expanded, Flex, Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement,
    Radius, Text,
};
use warpui::keymap::{FixedBinding, Keystroke};
use warpui::platform::Cursor;
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponentStyles};
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use warp_cli::agent::Harness;
use warp_core::channel::{Channel, ChannelState};
use warp_core::ui::color::blend::Blend;
use warp_core::ui::theme::Fill;

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
use crate::ai::blocklist::inline_action::requested_action::{
    render_requested_action_row_for_text, CTRL_C_KEYSTROKE, ENTER_KEYSTROKE,
};
use crate::ai::cloud_environments::CloudAmbientAgentEnvironment;
use crate::ai::execution_profiles::model_menu_items::available_model_menu_items;
use crate::ai::harness_display;
use crate::appearance::Appearance;
use crate::menu::{MenuItem, MenuItemFields};
use crate::ui_components::blended_colors;
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{ButtonSize, KeystrokeSource, NakedTheme};
use crate::view_components::compactible_action_button::{
    CompactibleActionButton, RenderCompactibleActionButton, MEDIUM_SIZE_SWITCH_THRESHOLD,
};
use crate::view_components::compactible_split_action_button::CompactibleSplitActionButton;
use crate::view_components::dropdown::{Dropdown, DropdownAction, DropdownEvent, DropdownStyle};
use crate::view_components::{FilterableDropdown, FilterableDropdownEvent};
use crate::LLMPreferences;

const RUN_AGENTS_WARP_WORKER_HOST: &str = "warp";

const RUN_AGENTS_CARD_TITLE: &str = "Can I add additional agents to this task?";

const RUN_AGENTS_ENV_NONE_LABEL: &str = "(no environment)";

const RUN_AGENTS_EDITOR_OPEN: &str = "RunAgentsEditorOpen";

const RUN_AGENTS_PICKER_HEIGHT: f32 = 36.;
const RUN_AGENTS_PICKER_BORDER_WIDTH: f32 = 1.;
const RUN_AGENTS_PICKER_FONT_SIZE: f32 = 14.;
const ORCHESTRATE_PICKER_RADIUS: f32 = 4.;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAgentsEditState {
    pub is_editor_open: bool,
    pub model_id: String,
    pub harness_type: String,
    pub execution_mode: RunAgentsExecutionMode,
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
            model_id: req.model_id.clone(),
            harness_type: req.harness_type.clone(),
            execution_mode: req.execution_mode.clone(),
            agent_run_configs: req.agent_run_configs.clone(),
            base_prompt: req.base_prompt.clone(),
            summary: req.summary.clone(),
            skills: req.skills.clone(),
        }
    }

    /// Toggle Local <-> Cloud. Resets OpenCode to Oz when switching
    /// to Cloud (unsupported combination).
    pub fn toggle_execution_mode_to_remote(&mut self, is_remote: bool) {
        if is_remote {
            if self.harness_type.eq_ignore_ascii_case("opencode") {
                self.harness_type = "oz".to_string();
            }
            // TODO(QUALITY-569): expose worker_host as an editable picker.
            if !self.execution_mode.is_remote() {
                self.execution_mode = RunAgentsExecutionMode::Remote {
                    environment_id: String::new(),
                    worker_host: "warp".to_string(),
                    computer_use_enabled: false,
                };
            }
        } else {
            self.execution_mode = RunAgentsExecutionMode::Local;
        }
    }

    pub fn set_environment_id(&mut self, environment_id: String) {
        if let RunAgentsExecutionMode::Remote {
            environment_id: id, ..
        } = &mut self.execution_mode
        {
            *id = environment_id;
        }
    }

    pub fn set_worker_host(&mut self, worker_host: String) {
        if let RunAgentsExecutionMode::Remote {
            worker_host: wh, ..
        } = &mut self.execution_mode
        {
            *wh = worker_host;
        }
    }

    /// Returns `Some(reason)` if Accept must be disabled.
    /// Only hard block: OpenCode+Cloud.
    pub fn accept_disabled_reason(&self) -> Option<&'static str> {
        match &self.execution_mode {
            RunAgentsExecutionMode::Remote { .. }
                if self.harness_type.eq_ignore_ascii_case("opencode") =>
            {
                Some(
                    "OpenCode is not supported on Cloud yet. Switch to Local or pick a different harness.",
                )
            }
            RunAgentsExecutionMode::Local | RunAgentsExecutionMode::Remote { .. } => None,
        }
    }

    pub fn to_request(&self) -> RunAgentsRequest {
        RunAgentsRequest {
            summary: self.summary.clone(),
            base_prompt: self.base_prompt.clone(),
            skills: self.skills.clone(),
            model_id: self.model_id.clone(),
            harness_type: self.harness_type.clone(),
            execution_mode: self.execution_mode.clone(),
            agent_run_configs: self.agent_run_configs.clone(),
        }
    }
}

/// Per-action UI handles. Picker views are lazily created on first
/// editor open.
#[derive(Default, Clone)]
struct RunAgentsCardHandles {
    reject_button: Option<CompactibleActionButton>,
    edit_button: Option<CompactibleActionButton>,
    accept_button: Option<CompactibleSplitActionButton>,
    local_toggle: MouseStateHandle,
    cloud_toggle: MouseStateHandle,
    model_picker: Option<ViewHandle<Dropdown<RunAgentsCardViewAction>>>,
    harness_picker: Option<ViewHandle<Dropdown<RunAgentsCardViewAction>>>,
    environment_picker: Option<ViewHandle<FilterableDropdown<RunAgentsCardViewAction>>>,
    host_picker: Option<ViewHandle<Dropdown<RunAgentsCardViewAction>>>,
}

#[derive(Clone, Debug)]
pub enum RunAgentsCardViewAction {
    Accept,
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

    action_model: ModelHandle<BlocklistAIActionModel>,
    block_model: Rc<dyn AIBlockModel<View = AIBlock>>,
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
        action_model: ModelHandle<BlocklistAIActionModel>,
        run_agents_executor: ModelHandle<RunAgentsExecutor>,
        block_model: Rc<dyn AIBlockModel<View = AIBlock>>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
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
        // Both primary and chevron click route to Accept.
        let accept_button = CompactibleSplitActionButton::new(
            "Accept".to_string(),
            Some(KeystrokeSource::Fixed(accept_keystroke)),
            ButtonSize::Small,
            RunAgentsCardViewAction::Accept,
            RunAgentsCardViewAction::Accept,
            Icon::Check,
            true,
            None,
            ctx,
        );

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

        // Re-render when this action finishes (e.g. cancelled via
        // Ctrl+C at the terminal level) so render() picks up the
        // Finished status from the action model.
        let action_id_for_finished = action_id.clone();
        ctx.subscribe_to_model(&action_model, move |_, _, event, ctx| {
            if let BlocklistAIActionEvent::FinishedAction { action_id, .. } = event {
                if action_id == &action_id_for_finished {
                    ctx.notify();
                }
            }
        });

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
        if self.state.is_editor_open || self.spawning.is_some() {
            return;
        }
        let new_state = RunAgentsEditState::from_request(request);
        if self.state != new_state {
            self.state = new_state;
            self.original_request = request.clone();
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
        // Shared picker styling.
        let picker_padding = Coords {
            top: 8.,
            bottom: 8.,
            left: 12.,
            right: 12.,
        };
        let picker_corner_radius =
            CornerRadius::with_all(Radius::Pixels(ORCHESTRATE_PICKER_RADIUS));
        let theme = Appearance::as_ref(ctx).theme();
        // The picker bg is a translucent overlay (surface_overlay_1 =
        // fg at 5%). Composite it against the card background to derive
        // opaque border and text colors that work on any theme.
        let picker_background_theme: Fill = theme.surface_overlay_1();
        let composited_bg = theme
            .background()
            .blend(&picker_background_theme)
            .into_solid();
        let picker_border_color_warpui: warpui::elements::Fill = theme.surface_2().into();
        let picker_font_color = blended_colors::text_main(theme, composited_bg);
        let picker_background_warpui: warpui::elements::Fill = picker_background_theme.into();
        let picker_styles = UiComponentStyles {
            height: Some(RUN_AGENTS_PICKER_HEIGHT),
            background: Some(picker_background_warpui),
            border_color: Some(picker_border_color_warpui),
            border_width: Some(RUN_AGENTS_PICKER_BORDER_WIDTH),
            border_radius: Some(picker_corner_radius),
            font_size: Some(RUN_AGENTS_PICKER_FONT_SIZE),
            font_color: Some(picker_font_color),
            padding: Some(picker_padding),
            ..Default::default()
        };

        let initial_model_id_default = self
            .block_model
            .base_model(ctx)
            .map(|id| id.to_string())
            .unwrap_or_default();
        let state_snapshot = self.state.clone();

        if self.handles.model_picker.is_none() {
            let initial_model_id = if state_snapshot.model_id.trim().is_empty() {
                initial_model_id_default.clone()
            } else {
                state_snapshot.model_id.clone()
            };
            let dropdown_handle = Self::new_standard_picker_dropdown(
                picker_padding,
                picker_corner_radius,
                picker_background_warpui,
                picker_border_color_warpui,
                picker_font_color,
                ctx,
            );
            dropdown_handle.update(ctx, |dropdown, ctx_dropdown| {
                let llm_prefs = LLMPreferences::as_ref(ctx_dropdown);
                let choices: Vec<_> = llm_prefs.get_base_llm_choices_for_agent_mode().collect();
                let initial_index = choices
                    .iter()
                    .position(|llm| llm.id.to_string() == initial_model_id);
                let items = available_model_menu_items(
                    choices,
                    move |llm| {
                        DropdownAction::SelectActionAndClose(
                            RunAgentsCardViewAction::ModelChanged {
                                model_id: llm.id.to_string(),
                            },
                        )
                    },
                    None,
                    None,
                    false,
                    false,
                    ctx_dropdown,
                );
                dropdown.set_rich_items(items, ctx_dropdown);
                if let Some(idx) = initial_index {
                    dropdown.set_selected_by_index(idx, ctx_dropdown);
                }
            });
            Self::subscribe_picker_close(&dropdown_handle, ctx);
            self.handles.model_picker = Some(dropdown_handle);
        }

        if self.handles.harness_picker.is_none() {
            let initial_harness = state_snapshot.harness_type.clone();
            let dropdown_handle = Self::new_standard_picker_dropdown(
                picker_padding,
                picker_corner_radius,
                picker_background_warpui,
                picker_border_color_warpui,
                picker_font_color,
                ctx,
            );
            dropdown_handle.update(ctx, |dropdown, ctx_dropdown| {
                let mut items: Vec<MenuItem<DropdownAction<RunAgentsCardViewAction>>> = Vec::new();
                let mut selected_idx = None;
                // TODO: Re-enable Harness::Gemini once it is supported as
                // a multi-agent harness (currently causes an infinite
                // "Spawning agents" hang).
                for (idx, harness) in [Harness::Oz, Harness::Claude, Harness::Codex]
                    .into_iter()
                    .enumerate()
                {
                    let mut fields = MenuItemFields::new(harness_display::display_name(harness))
                        .with_icon(harness_display::icon_for(harness));
                    if let Some(color) = harness_display::brand_color(harness) {
                        fields = fields.with_override_icon_color(Fill::from(color));
                    }
                    let harness_str = harness.to_string();
                    fields = fields.with_on_select_action(DropdownAction::SelectActionAndClose(
                        RunAgentsCardViewAction::HarnessChanged {
                            harness_type: harness_str.clone(),
                        },
                    ));
                    if harness_str.eq_ignore_ascii_case(&initial_harness) {
                        selected_idx = Some(idx);
                    }
                    items.push(MenuItem::Item(fields));
                }
                dropdown.set_rich_items(items, ctx_dropdown);
                if let Some(idx) = selected_idx {
                    dropdown.set_selected_by_index(idx, ctx_dropdown);
                }
            });
            Self::subscribe_picker_close(&dropdown_handle, ctx);
            self.handles.harness_picker = Some(dropdown_handle);
        }

        if self.handles.environment_picker.is_none() {
            let initial_env = match &state_snapshot.execution_mode {
                RunAgentsExecutionMode::Remote { environment_id, .. } => environment_id.clone(),
                RunAgentsExecutionMode::Local => String::new(),
            };
            let picker_styles_clone = picker_styles;
            let dropdown_handle = ctx.add_typed_action_view(move |ctx_dropdown| {
                let mut dropdown = FilterableDropdown::<RunAgentsCardViewAction>::new(ctx_dropdown);
                dropdown.set_use_overlay_layer(false, ctx_dropdown);
                dropdown.set_main_axis_size(MainAxisSize::Max, ctx_dropdown);
                dropdown.set_button_variant(ButtonVariant::Secondary);
                dropdown.set_style(picker_styles_clone);
                dropdown.set_top_bar_height(RUN_AGENTS_PICKER_HEIGHT, ctx_dropdown);
                dropdown
            });
            dropdown_handle.update(ctx, |dropdown, ctx_dropdown| {
                dropdown.set_menu_width(280.0, ctx_dropdown);
                let all_envs = CloudAmbientAgentEnvironment::get_all(ctx_dropdown);
                let mut sorted_envs: Vec<(String, String)> = all_envs
                    .iter()
                    .map(|env| (env.id.uid(), env.model().string_model.name.clone()))
                    .collect();
                sorted_envs.sort_by(|a, b| a.1.cmp(&b.1));

                let mut items: Vec<MenuItem<DropdownAction<RunAgentsCardViewAction>>> = Vec::new();
                let mut selected_name: Option<String> = None;
                items.push(MenuItem::Item(
                    MenuItemFields::new(RUN_AGENTS_ENV_NONE_LABEL).with_on_select_action(
                        DropdownAction::SelectActionAndClose(
                            RunAgentsCardViewAction::EnvironmentChanged {
                                environment_id: String::new(),
                            },
                        ),
                    ),
                ));
                if initial_env.is_empty() {
                    selected_name = Some(RUN_AGENTS_ENV_NONE_LABEL.to_string());
                }
                for (env_id, env_name) in &sorted_envs {
                    if env_id == &initial_env {
                        selected_name = Some(env_name.clone());
                    }
                    let env_id_for_item = env_id.clone();
                    items.push(MenuItem::Item(
                        MenuItemFields::new(env_name).with_on_select_action(
                            DropdownAction::SelectActionAndClose(
                                RunAgentsCardViewAction::EnvironmentChanged {
                                    environment_id: env_id_for_item,
                                },
                            ),
                        ),
                    ));
                }
                dropdown.set_rich_items(items, ctx_dropdown);
                if let Some(name) = selected_name {
                    dropdown.set_selected_by_name(&name, ctx_dropdown);
                }
            });
            ctx.subscribe_to_view(&dropdown_handle, |me, _, event, ctx| {
                if let FilterableDropdownEvent::Close = event {
                    me.refocus_after_picker_close(ctx);
                }
            });
            self.handles.environment_picker = Some(dropdown_handle);
        }

        if self.handles.host_picker.is_none() {
            let initial_host = match &state_snapshot.execution_mode {
                RunAgentsExecutionMode::Remote { worker_host, .. } => worker_host.clone(),
                RunAgentsExecutionMode::Local => RUN_AGENTS_WARP_WORKER_HOST.to_string(),
            };
            let dropdown_handle = Self::new_standard_picker_dropdown(
                picker_padding,
                picker_corner_radius,
                picker_background_warpui,
                picker_border_color_warpui,
                picker_font_color,
                ctx,
            );
            dropdown_handle.update(ctx, |dropdown, ctx_dropdown| {
                let hosts: &[&str] = if matches!(ChannelState::channel(), Channel::Local) {
                    &["warp", "local-dev"]
                } else {
                    &["warp"]
                };
                let mut items: Vec<MenuItem<DropdownAction<RunAgentsCardViewAction>>> = Vec::new();
                let mut selected_idx = None;
                for (idx, &host) in hosts.iter().enumerate() {
                    let fields = MenuItemFields::new(host).with_on_select_action(
                        DropdownAction::SelectActionAndClose(
                            RunAgentsCardViewAction::WorkerHostChanged {
                                worker_host: host.to_string(),
                            },
                        ),
                    );
                    if host.eq_ignore_ascii_case(&initial_host) {
                        selected_idx = Some(idx);
                    }
                    items.push(MenuItem::Item(fields));
                }
                dropdown.set_rich_items(items, ctx_dropdown);
                if let Some(idx) = selected_idx {
                    dropdown.set_selected_by_index(idx, ctx_dropdown);
                }
            });
            Self::subscribe_picker_close(&dropdown_handle, ctx);
            self.handles.host_picker = Some(dropdown_handle);
        }

        // Dropdown's internal selection display is unreliable in this
        // view tree, so we explicitly drive it.
        self.sync_picker_selections(ctx);
    }

    /// Shared dropdown construction with the standard orchestrate-card
    /// styling (border, radius, background, font). Both the model and
    /// harness pickers use identical chrome; only their item lists differ.
    fn new_standard_picker_dropdown(
        padding: Coords,
        corner_radius: CornerRadius,
        background: warpui::elements::Fill,
        border_color: warpui::elements::Fill,
        font_color: ColorU,
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<Dropdown<RunAgentsCardViewAction>> {
        ctx.add_typed_action_view(move |ctx_dropdown| {
            let mut dropdown = Dropdown::<RunAgentsCardViewAction>::new(ctx_dropdown);
            dropdown.set_use_overlay_layer(false, ctx_dropdown);
            dropdown.set_main_axis_size(MainAxisSize::Max, ctx_dropdown);
            dropdown.set_style(DropdownStyle::ActionButtonSecondary, ctx_dropdown);
            dropdown.set_top_bar_height(RUN_AGENTS_PICKER_HEIGHT, ctx_dropdown);
            dropdown.set_padding(padding, ctx_dropdown);
            dropdown.set_border_radius(corner_radius, ctx_dropdown);
            dropdown.set_background(background, ctx_dropdown);
            dropdown.set_border_color(border_color, ctx_dropdown);
            dropdown.set_border_width(RUN_AGENTS_PICKER_BORDER_WIDTH, ctx_dropdown);
            dropdown.set_font_size(RUN_AGENTS_PICKER_FONT_SIZE, ctx_dropdown);
            dropdown.set_font_color(font_color, ctx_dropdown);
            dropdown
        })
    }

    fn subscribe_picker_close(
        dropdown_handle: &ViewHandle<Dropdown<RunAgentsCardViewAction>>,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.subscribe_to_view(dropdown_handle, move |me, _, event, ctx| {
            if let DropdownEvent::Close = event {
                me.refocus_after_picker_close(ctx);
            }
        });
    }

    /// Restore focus after a picker dropdown closes.
    fn refocus_after_picker_close(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn sync_picker_selections(&mut self, ctx: &mut ViewContext<Self>) {
        let state = self.state.clone();
        if let Some(model_picker) = self.handles.model_picker.clone() {
            let target_model_id = state.model_id.clone();
            model_picker.update(ctx, |dropdown, ctx_dropdown| {
                let llm_prefs = LLMPreferences::as_ref(ctx_dropdown);
                let choices: Vec<_> = llm_prefs.get_base_llm_choices_for_agent_mode().collect();
                if let Some(idx) = choices
                    .iter()
                    .position(|llm| llm.id.to_string() == target_model_id)
                {
                    dropdown.set_selected_by_index(idx, ctx_dropdown);
                }
            });
        }
        if let Some(harness_picker) = self.handles.harness_picker.clone() {
            let target =
                Harness::parse_orchestration_harness(&state.harness_type).unwrap_or(Harness::Oz);
            let display = harness_display::display_name(target).to_string();
            harness_picker.update(ctx, |dropdown, ctx_dropdown| {
                dropdown.set_selected_by_name(&display, ctx_dropdown);
            });
        }
        if let Some(environment_picker) = self.handles.environment_picker.clone() {
            let env_id = match &state.execution_mode {
                RunAgentsExecutionMode::Remote { environment_id, .. } => environment_id.clone(),
                RunAgentsExecutionMode::Local => String::new(),
            };
            environment_picker.update(ctx, |dropdown, ctx_dropdown| {
                if env_id.is_empty() {
                    dropdown.set_selected_by_name(RUN_AGENTS_ENV_NONE_LABEL, ctx_dropdown);
                    return;
                }
                let all_envs = CloudAmbientAgentEnvironment::get_all(ctx_dropdown);
                if let Some(env) = all_envs.iter().find(|e| e.id.uid() == env_id) {
                    dropdown.set_selected_by_name(&env.model().string_model.name, ctx_dropdown);
                }
            });
        }
        if let Some(host_picker) = self.handles.host_picker.clone() {
            let worker_host = match &state.execution_mode {
                RunAgentsExecutionMode::Remote { worker_host, .. } => worker_host.clone(),
                RunAgentsExecutionMode::Local => RUN_AGENTS_WARP_WORKER_HOST.to_string(),
            };
            host_picker.update(ctx, |dropdown, ctx_dropdown| {
                dropdown.set_selected_by_name(&worker_host, ctx_dropdown);
            });
        }
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
        render_confirmation_card(&self.state, &self.handles, is_blocked, app)
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
                self.state.toggle_execution_mode_to_remote(*is_remote);
                // Local→Cloud may reset OpenCode→Oz; sync pickers.
                self.sync_picker_selections(ctx);
                ctx.notify();
            }
            RunAgentsCardViewAction::ModelChanged { model_id } => {
                self.state.model_id = model_id.clone();
                // Do NOT call sync_picker_selections here — this runs
                // mid-update and would cause a circular view update.
                ctx.notify();
            }
            RunAgentsCardViewAction::HarnessChanged { harness_type } => {
                self.state.harness_type = harness_type.clone();
                // See ModelChanged note.
                ctx.notify();
            }
            RunAgentsCardViewAction::EnvironmentChanged { environment_id } => {
                self.state.set_environment_id(environment_id.clone());
                // See ModelChanged note.
                ctx.notify();
            }
            RunAgentsCardViewAction::WorkerHostChanged { worker_host } => {
                self.state.set_worker_host(worker_host.clone());
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
    Container::new(
        Container::new(row)
            .with_background_color(blended_colors::neutral_2(theme))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .finish(),
    )
    .with_margin_left(16.)
    .with_margin_right(16.)
    .finish()
    .with_agent_output_item_spacing(app)
    .finish()
}

fn render_editor(
    state: &RunAgentsEditState,
    handles: &RunAgentsCardHandles,
    app: &AppContext,
) -> Box<dyn Element> {
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
        Container::new(render_mode_toggle(state, handles, appearance))
            .with_margin_top(12.)
            .finish(),
    );
    column.add_child(render_picker_row_quad(state, handles, appearance));

    if let Some(reason) = state.accept_disabled_reason() {
        column.add_child(render_validation_error(
            reason,
            theme.ui_error_color(),
            appearance,
        ));
    } else if let Some(message) = empty_env_recommendation_message(state, app) {
        column.add_child(render_validation_error(
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

fn render_picker_row_quad(
    state: &RunAgentsEditState,
    handles: &RunAgentsCardHandles,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let is_remote = state.execution_mode.is_remote();
    let main_axis_size = if is_remote {
        MainAxisSize::Max
    } else {
        MainAxisSize::Min
    };
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(main_axis_size)
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_spacing(12.);

    const LOCAL_PICKER_WIDTH: f32 = 220.;
    let add_picker = |row: &mut Flex, label: &str, picker: Option<Box<dyn Element>>| {
        let column = render_picker_column(label, picker, appearance);
        if is_remote {
            row.add_child(Expanded::new(1.0, column).finish());
        } else {
            row.add_child(
                ConstrainedBox::new(column)
                    .with_width(LOCAL_PICKER_WIDTH)
                    .finish(),
            );
        }
    };

    add_picker(
        &mut row,
        "Agent harness",
        handles
            .harness_picker
            .as_ref()
            .map(|p| ChildView::new(p).finish()),
    );
    if is_remote {
        add_picker(
            &mut row,
            "Host",
            handles
                .host_picker
                .as_ref()
                .map(|p| ChildView::new(p).finish()),
        );
        add_picker(
            &mut row,
            "Environment",
            handles
                .environment_picker
                .as_ref()
                .map(|p| ChildView::new(p).finish()),
        );
    }
    add_picker(
        &mut row,
        "Base model",
        handles
            .model_picker
            .as_ref()
            .map(|p| ChildView::new(p).finish()),
    );

    Container::new(row.finish()).with_margin_top(12.).finish()
}

fn render_picker_column(
    label: &str,
    picker: Option<Box<dyn Element>>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label_el = Text::new(
        label.to_string(),
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(blended_colors::text_disabled(theme, theme.surface_1()))
    .finish();

    let body: Box<dyn Element> = picker.unwrap_or_else(|| Empty::new().finish());
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(label_el)
        .with_child(body)
        .finish()
}

fn render_mode_toggle(
    state: &RunAgentsEditState,
    handles: &RunAgentsCardHandles,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let is_remote = state.execution_mode.is_remote();
    let label = Text::new(
        "Agent location".to_string(),
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(blended_colors::text_disabled(theme, theme.surface_1()))
    .finish();

    let local_segment = render_segment_button(
        "Local",
        !is_remote,
        RunAgentsCardViewAction::ExecutionModeToggled { is_remote: false },
        handles.local_toggle.clone(),
        appearance,
    );
    let cloud_segment = render_segment_button(
        "Cloud",
        is_remote,
        RunAgentsCardViewAction::ExecutionModeToggled { is_remote: true },
        handles.cloud_toggle.clone(),
        appearance,
    );

    let segment_outer_bg = warp_core::ui::theme::color::internal_colors::fg_overlay_2(theme);
    let segments_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(Expanded::new(1.0, cloud_segment).finish())
        .with_child(Expanded::new(1.0, local_segment).finish())
        .finish();
    let segmented_control = Container::new(segments_row)
        .with_padding_top(4.)
        .with_padding_bottom(4.)
        .with_padding_left(4.)
        .with_padding_right(4.)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
        .with_background(segment_outer_bg)
        .finish();
    let segmented_control = ConstrainedBox::new(segmented_control)
        .with_width(205.)
        .finish();

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(Container::new(label).with_margin_bottom(6.).finish())
        .with_child(segmented_control)
        .finish()
}

fn render_segment_button(
    label: &str,
    is_active: bool,
    on_click: RunAgentsCardViewAction,
    mouse_state: MouseStateHandle,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label_owned = label.to_string();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.monospace_font_size() + 1.;
    let active_text_color = blended_colors::text_main(theme, theme.surface_1());
    let inactive_text_color = blended_colors::text_disabled(theme, theme.surface_1());
    let segment_active_bg = warp_core::ui::theme::color::internal_colors::fg_overlay_4(theme);
    Hoverable::new(mouse_state, move |_| {
        let text = Text::new(label_owned.clone(), font_family, font_size)
            .with_color(if is_active {
                active_text_color
            } else {
                inactive_text_color
            })
            .finish();
        let centered = warpui::elements::Align::new(text).finish();
        let mut container = Container::new(centered)
            .with_vertical_padding(6.)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));
        if is_active {
            container = container.with_background(segment_active_bg);
        }
        container.finish()
    })
    .on_click(move |ctx, _, _| {
        ctx.dispatch_typed_action(on_click.clone());
    })
    .with_cursor(Cursor::PointingHand)
    .finish()
}

fn render_validation_error(
    reason: impl Into<String>,
    color: ColorU,
    appearance: &Appearance,
) -> Box<dyn Element> {
    Container::new(
        Text::new(
            reason.into(),
            appearance.ui_font_family(),
            appearance.monospace_font_size() - 1.,
        )
        .with_color(color)
        .finish(),
    )
    .with_margin_bottom(8.)
    .finish()
}

fn empty_env_recommendation_message(
    state: &RunAgentsEditState,
    app: &AppContext,
) -> Option<String> {
    let RunAgentsExecutionMode::Remote {
        environment_id,
        worker_host,
        ..
    } = &state.execution_mode
    else {
        return None;
    };
    if !environment_id.trim().is_empty() {
        return None;
    }
    if !worker_host.eq_ignore_ascii_case(RUN_AGENTS_WARP_WORKER_HOST) {
        return None;
    }
    let env_count = CloudAmbientAgentEnvironment::get_all(app).len();
    Some(if env_count > 0 {
        "We recommend selecting an environment for cloud agents.".to_string()
    } else {
        "We recommend creating an environment for cloud agents.".to_string()
    })
}

#[cfg(test)]
#[path = "run_agents_card_view_tests.rs"]
mod tests;
