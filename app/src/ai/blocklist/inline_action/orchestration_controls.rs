//! Shared orchestration controls reused by the `RunAgentsCardView`
//! confirmation card editor and the plan-card
//! `OrchestrationConfigBlockView`.
//!
//! The generic parameter `A` is the parent view's typed action — both
//! consumers impl [`OrchestrationControlAction`] to provide the mapping
//! from field-change events to their own action enum.

use ai::agent::action::RunAgentsExecutionMode;
use ai::agent::orchestration_config::{OrchestrationConfig, OrchestrationExecutionMode};
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::{vec2f, Vector2F};
use std::collections::HashMap;
use std::fmt::Debug;
use warpui::elements::{
    ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Expanded, Flex,
    Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement, Point, Radius,
    Text,
};
use warpui::event::DispatchedEvent;
use warpui::platform::Cursor;
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponentStyles};
use warpui::{
    AfterLayoutContext, AppContext, Element, EventContext, LayoutContext, PaintContext,
    SingletonEntity, SizeConstraint, View, ViewContext, ViewHandle,
};

use warp_cli::agent::Harness;
use warp_core::channel::{Channel, ChannelState};
use warp_core::ui::theme::Fill;

use crate::ai::cloud_environments::CloudAmbientAgentEnvironment;
use crate::ai::execution_profiles::model_menu_items::available_model_menu_items;
use crate::ai::harness_availability::HarnessAvailabilityModel;
use crate::ai::harness_display;
use crate::appearance::Appearance;
use crate::menu::{MenuItem, MenuItemFields};
use crate::ui_components::blended_colors;
use crate::view_components::dropdown::{Dropdown, DropdownAction, DropdownStyle};
use crate::view_components::FilterableDropdown;
use crate::LLMPreferences;

// ── Shared constants ────────────────────────────────────────────────

pub const ORCHESTRATION_WARP_WORKER_HOST: &str = "warp";
pub const ORCHESTRATION_ENV_NONE_LABEL: &str = "(no environment)";

pub const ORCHESTRATION_PICKER_HEIGHT: f32 = 36.;
pub const ORCHESTRATION_PICKER_BORDER_WIDTH: f32 = 1.;
pub const ORCHESTRATION_PICKER_FONT_SIZE: f32 = 14.;
pub const ORCHESTRATION_PICKER_RADIUS: f32 = 4.;
pub const ORCHESTRATION_PICKER_MAX_WIDTH: f32 = 205.;

const DEFAULT_MODEL_LABEL: &str = "Default model";

// ── Action trait ────────────────────────────────────────────────────

/// Trait that both `RunAgentsCardViewAction` and
/// `OrchestrationConfigBlockAction` implement so the shared picker
/// creation and render helpers can produce the correct action variant.
pub trait OrchestrationControlAction: Clone + Debug + Send + Sync + 'static {
    fn execution_mode_toggled(is_remote: bool) -> Self;
    fn model_changed(model_id: String) -> Self;
    fn harness_changed(harness_type: String) -> Self;
    fn environment_changed(environment_id: String) -> Self;
    fn worker_host_changed(worker_host: String) -> Self;
}

// ── Shared edit state ───────────────────────────────────────────────

/// Run-wide configuration fields shared between the confirmation card
/// editor and the plan-card config block. Card-specific fields
/// (agent_run_configs, base_prompt, summary, skills)
/// remain on the per-view state structs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestrationEditState {
    pub model_id: String,
    pub harness_type: String,
    pub execution_mode: RunAgentsExecutionMode,
}

impl OrchestrationEditState {
    pub fn from_run_agents_fields(
        model_id: &str,
        harness_type: &str,
        execution_mode: &RunAgentsExecutionMode,
    ) -> Self {
        Self {
            model_id: model_id.to_string(),
            harness_type: harness_type.to_string(),
            execution_mode: execution_mode.clone(),
        }
    }

    pub fn from_orchestration_config(config: &OrchestrationConfig) -> Self {
        let execution_mode = match &config.execution_mode {
            OrchestrationExecutionMode::Local => RunAgentsExecutionMode::Local,
            OrchestrationExecutionMode::Remote {
                environment_id,
                worker_host,
            } => RunAgentsExecutionMode::Remote {
                environment_id: environment_id.clone(),
                worker_host: worker_host.clone(),
                computer_use_enabled: false,
            },
        };
        Self {
            model_id: config.model_id.clone(),
            harness_type: config.harness_type.clone(),
            execution_mode,
        }
    }

    /// Toggle Local ↔ Cloud. Resets OpenCode to Oz when switching
    /// to Cloud (unsupported combination).
    pub fn toggle_execution_mode_to_remote(&mut self, is_remote: bool) {
        if is_remote {
            if self.harness_type.eq_ignore_ascii_case("opencode") {
                self.harness_type = "oz".to_string();
            }
            if !self.execution_mode.is_remote() {
                self.execution_mode = RunAgentsExecutionMode::Remote {
                    environment_id: String::new(),
                    worker_host: ORCHESTRATION_WARP_WORKER_HOST.to_string(),
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

    /// Returns `Some(reason)` if Accept / Apply must be disabled.
    /// Only hard block: OpenCode + Cloud.
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

    /// Fills in empty fields from the approved orchestration config.
    /// When the LLM omits harness/model/execution_mode to inherit from
    /// the active config, the raw request arrives with defaults (empty
    /// harness, empty model, Local mode). This resolves those to the
    /// config values so the UI shows the intended settings.
    pub fn resolve_from_config(&mut self, config: &OrchestrationConfig) {
        if self.harness_type.is_empty() && !config.harness_type.is_empty() {
            self.harness_type = config.harness_type.clone();
        }
        if self.model_id.is_empty() && !config.model_id.is_empty() {
            self.model_id = config.model_id.clone();
        }
        if !self.execution_mode.is_remote() && config.execution_mode.is_remote() {
            self.execution_mode = Self::from_orchestration_config(config).execution_mode;
        }
    }

    /// Converts to a native `OrchestrationConfig` for storage / match.
    pub fn to_orchestration_config(&self) -> OrchestrationConfig {
        let execution_mode = match &self.execution_mode {
            RunAgentsExecutionMode::Local => OrchestrationExecutionMode::Local,
            RunAgentsExecutionMode::Remote {
                environment_id,
                worker_host,
                ..
            } => OrchestrationExecutionMode::Remote {
                environment_id: environment_id.clone(),
                worker_host: worker_host.clone(),
            },
        };
        OrchestrationConfig {
            model_id: self.model_id.clone(),
            harness_type: self.harness_type.clone(),
            execution_mode,
        }
    }
}

// ── Picker handles ──────────────────────────────────────────────────

/// Picker view handles shared between card editor and plan-card config
/// block. Generic over the action type `A`.
#[derive(Clone)]
pub struct OrchestrationPickerHandles<A: OrchestrationControlAction> {
    pub model_picker: Option<ViewHandle<Dropdown<A>>>,
    pub harness_picker: Option<ViewHandle<Dropdown<A>>>,
    pub environment_picker: Option<ViewHandle<FilterableDropdown<A>>>,
    pub host_picker: Option<ViewHandle<Dropdown<A>>>,
    pub local_toggle: MouseStateHandle,
    pub cloud_toggle: MouseStateHandle,
}

impl<A: OrchestrationControlAction> Default for OrchestrationPickerHandles<A> {
    fn default() -> Self {
        Self {
            model_picker: None,
            harness_picker: None,
            environment_picker: None,
            host_picker: None,
            local_toggle: MouseStateHandle::default(),
            cloud_toggle: MouseStateHandle::default(),
        }
    }
}

// ── Picker styling ──────────────────────────────────────────────────

/// Constructs the shared `UiComponentStyles` for orchestration pickers.
pub fn picker_styles(appearance: &Appearance) -> (UiComponentStyles, PickerColors) {
    let theme = appearance.theme();
    let padding = Coords {
        top: 8.,
        bottom: 8.,
        left: 12.,
        right: 12.,
    };
    let corner_radius = CornerRadius::with_all(Radius::Pixels(ORCHESTRATION_PICKER_RADIUS));
    // The picker bg is a translucent overlay (surface_overlay_1 =
    // fg at 5%). It must stay translucent so that the accent-tinted
    // card background in the config block shows through, and so that
    // gradient-background themes render correctly.
    let background_fill: Fill = theme.surface_overlay_1();
    let background: warpui::elements::Fill = background_fill.into();
    // Border and font colors are intentionally left to the dropdown's
    // default ButtonVariant::Secondary styling, which uses
    // theme.outline() and theme.main_text_color() — both are
    // contrast-aware and adapt correctly to all themes.

    let styles = UiComponentStyles {
        height: Some(ORCHESTRATION_PICKER_HEIGHT),
        background: Some(background),
        border_width: Some(ORCHESTRATION_PICKER_BORDER_WIDTH),
        border_radius: Some(corner_radius),
        font_size: Some(ORCHESTRATION_PICKER_FONT_SIZE),
        padding: Some(padding),
        ..Default::default()
    };
    let colors = PickerColors {
        padding,
        corner_radius,
        background,
    };
    (styles, colors)
}

#[derive(Clone)]
pub struct PickerColors {
    pub padding: Coords,
    pub corner_radius: CornerRadius,
    pub background: warpui::elements::Fill,
}

// ── Picker creation (generic over action type) ──────────────────────

/// Creates a standard dropdown with the shared orchestration picker
/// chrome (border, radius, background, font).
pub fn new_standard_picker_dropdown<A: OrchestrationControlAction, V: View>(
    colors: &PickerColors,
    ctx: &mut ViewContext<V>,
) -> ViewHandle<Dropdown<A>> {
    let padding = colors.padding;
    let corner_radius = colors.corner_radius;
    let background = colors.background;
    ctx.add_typed_action_view(move |ctx_dropdown| {
        let mut dropdown = Dropdown::<A>::new(ctx_dropdown);
        dropdown.set_use_overlay_layer(false, ctx_dropdown);
        dropdown.set_main_axis_size(MainAxisSize::Max, ctx_dropdown);
        dropdown.set_style(DropdownStyle::ActionButtonSecondary, ctx_dropdown);
        dropdown.set_top_bar_height(ORCHESTRATION_PICKER_HEIGHT, ctx_dropdown);
        dropdown.set_top_bar_max_width(f32::INFINITY);
        dropdown.set_padding(padding, ctx_dropdown);
        dropdown.set_border_radius(corner_radius, ctx_dropdown);
        dropdown.set_background(background, ctx_dropdown);
        dropdown.set_border_width(ORCHESTRATION_PICKER_BORDER_WIDTH, ctx_dropdown);
        dropdown.set_font_size(ORCHESTRATION_PICKER_FONT_SIZE, ctx_dropdown);
        dropdown
    })
}

/// Populates the model picker based on the active harness.
///
/// - **Oz / empty**: shows the Warp LLM catalog (existing behavior).
/// - **Local Codex**: shows only a "Default model" entry (no model delivery
///   possible for local Codex children).
/// - **Other non-Oz harnesses**: shows "Default model" at the top, followed
///   by the server-provided harness model catalog from
///   `HarnessAvailabilityModel::models_for()`.
pub fn populate_model_picker_for_harness<A: OrchestrationControlAction, V: View>(
    dropdown: &ViewHandle<Dropdown<A>>,
    initial_model_id: &str,
    harness_type: &str,
    is_local: bool,
    ctx: &mut ViewContext<V>,
) {
    let initial_model_id = initial_model_id.to_string();
    let harness_type = harness_type.to_string();
    dropdown.update(ctx, |dropdown, ctx_dropdown| {
        let harness = Harness::parse_orchestration_harness(&harness_type);
        match harness {
            Some(Harness::Oz) | None => {
                // Oz / unset: current behavior — Warp LLM catalog.
                let llm_prefs = LLMPreferences::as_ref(ctx_dropdown);
                let choices: Vec<_> = llm_prefs.get_base_llm_choices_for_agent_mode().collect();
                let selected_display_name = choices
                    .iter()
                    .find(|llm| llm.id.to_string() == initial_model_id)
                    .map(|llm| llm.menu_display_name());
                let items = available_model_menu_items(
                    choices,
                    move |llm| {
                        DropdownAction::SelectActionAndClose(A::model_changed(llm.id.to_string()))
                    },
                    None,
                    None,
                    false,
                    false,
                    ctx_dropdown,
                );
                dropdown.set_rich_items(items, ctx_dropdown);
                if let Some(name) = &selected_display_name {
                    dropdown.set_selected_by_name(name, ctx_dropdown);
                }
            }
            Some(Harness::Codex) if is_local => {
                // Local Codex: only "Default model" entry.
                let items = vec![default_model_menu_item::<A>()];
                dropdown.set_rich_items(items, ctx_dropdown);
                dropdown.set_selected_by_name(DEFAULT_MODEL_LABEL, ctx_dropdown);
            }
            Some(harness) => {
                // Non-Oz harness: "Default model" at top, then server-provided
                // harness models.
                let mut items: Vec<MenuItem<DropdownAction<A>>> =
                    vec![default_model_menu_item::<A>()];
                let availability = HarnessAvailabilityModel::as_ref(ctx_dropdown);
                if let Some(models) = availability.models_for(harness) {
                    for model in models {
                        let model_id = model.id.clone();
                        let fields = MenuItemFields::new(&model.display_name)
                            .with_on_select_action(DropdownAction::SelectActionAndClose(
                                A::model_changed(model_id),
                            ));
                        items.push(MenuItem::Item(fields));
                    }
                }
                // Find display name before set_rich_items borrows ctx_dropdown mutably.
                let selected_display_name = if initial_model_id.is_empty() {
                    Some(DEFAULT_MODEL_LABEL.to_string())
                } else {
                    availability
                        .models_for(harness)
                        .and_then(|models| {
                            models
                                .iter()
                                .find(|m| m.id == initial_model_id)
                                .map(|m| m.display_name.clone())
                        })
                        .or_else(|| Some(DEFAULT_MODEL_LABEL.to_string()))
                };
                dropdown.set_rich_items(items, ctx_dropdown);
                if let Some(name) = &selected_display_name {
                    dropdown.set_selected_by_name(name, ctx_dropdown);
                }
            }
        }
    });
}

/// Creates a "Default model" menu item that emits an empty model_id.
fn default_model_menu_item<A: OrchestrationControlAction>() -> MenuItem<DropdownAction<A>> {
    MenuItem::Item(
        MenuItemFields::new(DEFAULT_MODEL_LABEL).with_on_select_action(
            DropdownAction::SelectActionAndClose(A::model_changed(String::new())),
        ),
    )
}

/// Returns whether the given model_id is present in the harness-filtered
/// model choices. Used to detect when a harness change invalidates the
/// current model selection.
pub fn is_model_in_filtered_choices<V: View>(
    model_id: &str,
    harness_type: &str,
    is_local: bool,
    ctx: &mut ViewContext<V>,
) -> bool {
    let harness = Harness::parse_orchestration_harness(harness_type);
    match harness {
        Some(Harness::Oz) | None => {
            let llm_prefs = LLMPreferences::as_ref(ctx);
            llm_prefs
                .get_base_llm_choices_for_agent_mode()
                .any(|llm| llm.id.to_string() == model_id)
        }
        Some(Harness::Codex) if is_local => model_id.is_empty(),
        Some(harness) => {
            // Empty string is always valid (the "Default model" entry).
            if model_id.is_empty() {
                return true;
            }
            let availability = HarnessAvailabilityModel::as_ref(ctx);
            availability
                .models_for(harness)
                .is_some_and(|models| models.iter().any(|m| m.id == model_id))
        }
    }
}

/// Returns the default model_id for the given harness.
///
/// For Oz this is the first Warp LLM; for non-Oz harnesses it is an empty
/// string (the "Default model" entry).
pub fn first_filtered_model_id<V: View>(
    harness_type: &str,
    ctx: &mut ViewContext<V>,
) -> Option<String> {
    let harness = Harness::parse_orchestration_harness(harness_type);
    match harness {
        Some(Harness::Oz) | None => {
            let llm_prefs = LLMPreferences::as_ref(ctx);
            llm_prefs
                .get_base_llm_choices_for_agent_mode()
                .next()
                .map(|llm| llm.id.to_string())
        }
        Some(_) => Some(String::new()),
    }
}

pub fn populate_harness_picker<A: OrchestrationControlAction, V: View>(
    dropdown: &ViewHandle<Dropdown<A>>,
    initial_harness: &str,
    ctx: &mut ViewContext<V>,
) {
    let initial_harness = initial_harness.to_string();
    dropdown.update(ctx, |dropdown, ctx_dropdown| {
        let availability = HarnessAvailabilityModel::as_ref(ctx_dropdown);
        let harnesses = availability.available_harnesses();

        // Sort enabled harnesses before disabled ones, preserving
        // relative order within each group.
        // Filter out Gemini — it is not yet supported as a multi-agent
        // harness and causes an infinite "Spawning agents" hang.
        let mut sorted: Vec<_> = harnesses
            .iter()
            .filter(|entry| entry.harness != Harness::Gemini)
            .collect();
        sorted.sort_by_key(|entry| !entry.enabled);

        // Resolve the target harness so we can match by enum variant
        // even when the `initial_harness` string is "claude" but the
        // cached entry.harness deserialized as Unknown.
        let target_harness = Harness::parse_orchestration_harness(&initial_harness);

        let mut items: Vec<MenuItem<DropdownAction<A>>> = Vec::new();
        let mut selected_idx = None;
        for (idx, entry) in sorted.iter().enumerate() {
            let harness = entry.harness;
            // Use the server-provided display_name for the label so stale
            // cache entries (where harness deserializes as Unknown) still
            // show the correct name.
            let mut fields = MenuItemFields::new(&entry.display_name)
                .with_icon(harness_display::icon_for(harness));
            if let Some(color) = harness_display::brand_color(harness) {
                fields = fields.with_override_icon_color(Fill::from(color));
            }
            let harness_str = harness.to_string();
            if entry.enabled {
                fields = fields.with_on_select_action(DropdownAction::SelectActionAndClose(
                    A::harness_changed(harness_str.clone()),
                ));
            } else {
                fields = fields.with_disabled(true);
            }
            // Match by harness string first, then fall back to matching
            // the display_name against the client-side name for the target
            // harness. This handles stale cache entries where entry.harness
            // is Unknown but entry.display_name is still correct.
            if harness_str.eq_ignore_ascii_case(&initial_harness) {
                selected_idx = Some(idx);
            } else if selected_idx.is_none() {
                if let Some(target) = target_harness {
                    let target_display = harness_display::display_name(target);
                    if entry.display_name == target_display {
                        selected_idx = Some(idx);
                    }
                }
            }
            items.push(MenuItem::Item(fields));
        }
        dropdown.set_rich_items(items, ctx_dropdown);
        if let Some(idx) = selected_idx {
            dropdown.set_selected_by_index(idx, ctx_dropdown);
        }
    });
}

pub fn create_environment_picker<A: OrchestrationControlAction, V: View>(
    initial_env_id: &str,
    styles: &UiComponentStyles,
    ctx: &mut ViewContext<V>,
) -> ViewHandle<FilterableDropdown<A>> {
    let initial_env = initial_env_id.to_string();
    let styles = *styles;
    let dropdown_handle = ctx.add_typed_action_view(move |ctx_dropdown| {
        let mut dropdown = FilterableDropdown::<A>::new(ctx_dropdown);
        dropdown.set_use_overlay_layer(false, ctx_dropdown);
        dropdown.set_main_axis_size(MainAxisSize::Max, ctx_dropdown);
        dropdown.set_button_variant(ButtonVariant::Secondary);
        dropdown.set_style(styles);
        dropdown.set_top_bar_height(ORCHESTRATION_PICKER_HEIGHT, ctx_dropdown);
        dropdown.set_top_bar_max_width(f32::INFINITY);
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

        let mut items: Vec<MenuItem<DropdownAction<A>>> = Vec::new();
        let mut selected_name: Option<String> = None;
        items.push(MenuItem::Item(
            MenuItemFields::new(ORCHESTRATION_ENV_NONE_LABEL).with_on_select_action(
                DropdownAction::SelectActionAndClose(A::environment_changed(String::new())),
            ),
        ));
        if initial_env.is_empty() {
            selected_name = Some(ORCHESTRATION_ENV_NONE_LABEL.to_string());
        }
        for (env_id, env_name) in &sorted_envs {
            if env_id == &initial_env {
                selected_name = Some(env_name.clone());
            }
            let env_id_for_item = env_id.clone();
            items.push(MenuItem::Item(
                MenuItemFields::new(env_name).with_on_select_action(
                    DropdownAction::SelectActionAndClose(A::environment_changed(env_id_for_item)),
                ),
            ));
        }
        dropdown.set_rich_items(items, ctx_dropdown);
        if let Some(name) = selected_name {
            dropdown.set_selected_by_name(&name, ctx_dropdown);
        }
    });
    dropdown_handle
}

pub fn populate_host_picker<A: OrchestrationControlAction, V: View>(
    dropdown: &ViewHandle<Dropdown<A>>,
    initial_host: &str,
    ctx: &mut ViewContext<V>,
) {
    let initial_host = if initial_host.is_empty() {
        ORCHESTRATION_WARP_WORKER_HOST.to_string()
    } else {
        initial_host.to_string()
    };
    dropdown.update(ctx, |dropdown, ctx_dropdown| {
        let hosts: &[&str] = if matches!(ChannelState::channel(), Channel::Local) {
            &["warp", "local-dev"]
        } else {
            &["warp"]
        };
        let mut items: Vec<MenuItem<DropdownAction<A>>> = Vec::new();
        let mut selected_idx = None;
        for (idx, &host) in hosts.iter().enumerate() {
            let fields = MenuItemFields::new(host).with_on_select_action(
                DropdownAction::SelectActionAndClose(A::worker_host_changed(host.to_string())),
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
}

/// Normalizes a harness_type string for use as a HashMap key in
/// per-harness model memory. Empty string (the wire representation
/// of Oz) is mapped to "oz" so saves and lookups are consistent.
pub fn harness_save_key(harness_type: &str) -> &str {
    if harness_type.is_empty() {
        "oz"
    } else {
        harness_type
    }
}

// ── Shared action helpers ───────────────────────────────────────────

/// Handles a harness change for both card views: saves the current
/// model for the old harness, restores a previously saved model for
/// the new harness (if still valid), falls back to a caller-provided
/// base model id or the first available model, and repopulates the
/// model picker.
///
/// Does NOT call `sync_picker_selections` — the harness picker
/// dispatched this action and must not be re-entered.
pub fn apply_harness_change<A: OrchestrationControlAction, V: View>(
    state: &mut OrchestrationEditState,
    memory: &mut HashMap<String, String>,
    handles: &OrchestrationPickerHandles<A>,
    new_harness_type: &str,
    fallback_base_model_id: impl FnOnce(&mut ViewContext<V>) -> Option<String>,
    ctx: &mut ViewContext<V>,
) {
    // Save current model for the old harness.
    let old_key = harness_save_key(&state.harness_type).to_string();
    memory.insert(old_key, state.model_id.clone());
    state.harness_type = new_harness_type.to_string();

    let is_local = !state.execution_mode.is_remote();
    // Try to restore a previously saved model for this harness.
    let new_key = harness_save_key(new_harness_type);
    let restored = memory
        .get(new_key)
        .filter(|id| is_model_in_filtered_choices(id, new_harness_type, is_local, ctx))
        .cloned();
    if let Some(saved_id) = restored {
        state.model_id = saved_id;
    } else if !is_model_in_filtered_choices(&state.model_id, new_harness_type, is_local, ctx) {
        // No saved model — fall back to conversation base model
        // for Oz, or default for non-Oz.
        let reset_id = fallback_base_model_id(ctx)
            .filter(|id| is_model_in_filtered_choices(id, new_harness_type, is_local, ctx))
            .or_else(|| first_filtered_model_id(new_harness_type, ctx))
            .unwrap_or_default();
        state.model_id = reset_id;
    }
    if let Some(handle) = &handles.model_picker {
        populate_model_picker_for_harness(handle, &state.model_id, new_harness_type, is_local, ctx);
    }
}

/// Handles an execution-mode toggle for both card views: toggles the
/// mode, revalidates/resets the model_id if invalid for the new mode,
/// repopulates the model picker, and syncs all picker selections.
pub fn apply_execution_mode_change<A: OrchestrationControlAction, V: View>(
    state: &mut OrchestrationEditState,
    handles: &OrchestrationPickerHandles<A>,
    is_remote: bool,
    fallback_base_model_id: impl FnOnce(&mut ViewContext<V>) -> Option<String>,
    ctx: &mut ViewContext<V>,
) {
    state.toggle_execution_mode_to_remote(is_remote);
    let is_local = !state.execution_mode.is_remote();
    if !is_model_in_filtered_choices(&state.model_id, &state.harness_type, is_local, ctx) {
        let reset_id = fallback_base_model_id(ctx)
            .filter(|id| is_model_in_filtered_choices(id, &state.harness_type, is_local, ctx))
            .or_else(|| first_filtered_model_id(&state.harness_type, ctx))
            .unwrap_or_default();
        state.model_id = reset_id;
    }
    if let Some(handle) = &handles.model_picker {
        populate_model_picker_for_harness(
            handle,
            &state.model_id,
            &state.harness_type,
            is_local,
            ctx,
        );
    }
    sync_picker_selections(state, handles, ctx);
}

// ── Picker repopulation + selection sync ──

/// Repopulates both the harness and model pickers from the current
/// server-provided data, revalidates `state.model_id` against the
/// updated catalog (resetting to default if the model disappeared),
/// then re-syncs dropdown selections.
pub fn repopulate_all_pickers<A: OrchestrationControlAction, V: View>(
    state: &mut OrchestrationEditState,
    handles: &OrchestrationPickerHandles<A>,
    ctx: &mut ViewContext<V>,
) {
    if let Some(handle) = &handles.harness_picker {
        populate_harness_picker(handle, &state.harness_type, ctx);
    }
    // Revalidate model_id: if the previously selected model is no longer
    // in the catalog (e.g. server removed it), reset to default.
    let is_local = !state.execution_mode.is_remote();
    if !is_model_in_filtered_choices(&state.model_id, &state.harness_type, is_local, ctx) {
        if let Some(first_id) = first_filtered_model_id(&state.harness_type, ctx) {
            state.model_id = first_id;
        }
    }
    if let Some(handle) = &handles.model_picker {
        populate_model_picker_for_harness(
            handle,
            &state.model_id,
            &state.harness_type,
            is_local,
            ctx,
        );
    }
    sync_picker_selections(state, handles, ctx);
}

pub fn sync_picker_selections<A: OrchestrationControlAction, V: View>(
    state: &OrchestrationEditState,
    handles: &OrchestrationPickerHandles<A>,
    ctx: &mut ViewContext<V>,
) {
    if let Some(model_picker) = handles.model_picker.clone() {
        let target_model_id = state.model_id.clone();
        let harness_type = state.harness_type.clone();
        model_picker.update(ctx, |dropdown, ctx_dropdown| {
            let harness = Harness::parse_orchestration_harness(&harness_type);
            let display_name = match harness {
                Some(Harness::Oz) | None => {
                    let llm_prefs = LLMPreferences::as_ref(ctx_dropdown);
                    llm_prefs
                        .get_base_llm_choices_for_agent_mode()
                        .find(|llm| llm.id.to_string() == target_model_id)
                        .map(|llm| llm.menu_display_name())
                }
                Some(harness) => {
                    if target_model_id.is_empty() {
                        Some(DEFAULT_MODEL_LABEL.to_string())
                    } else {
                        let availability = HarnessAvailabilityModel::as_ref(ctx_dropdown);
                        availability.models_for(harness).and_then(|models| {
                            models
                                .iter()
                                .find(|m| m.id == target_model_id)
                                .map(|m| m.display_name.clone())
                        })
                    }
                }
            };
            if let Some(name) = &display_name {
                dropdown.set_selected_by_name(name, ctx_dropdown);
            }
        });
    }
    if let Some(harness_picker) = handles.harness_picker.clone() {
        let harness_type = state.harness_type.clone();
        harness_picker.update(ctx, |dropdown, ctx_dropdown| {
            let target = Harness::parse_orchestration_harness(&harness_type).unwrap_or(Harness::Oz);
            // Use the server-provided display_name from HarnessAvailabilityModel
            // so the selection matches the labels (which also use display_name).
            let display = HarnessAvailabilityModel::as_ref(ctx_dropdown)
                .display_name_for(target)
                .to_string();
            dropdown.set_selected_by_name(&display, ctx_dropdown);
        });
    }
    if let Some(environment_picker) = handles.environment_picker.clone() {
        let env_id = match &state.execution_mode {
            RunAgentsExecutionMode::Remote { environment_id, .. } => environment_id.clone(),
            RunAgentsExecutionMode::Local => String::new(),
        };
        environment_picker.update(ctx, |dropdown, ctx_dropdown| {
            if env_id.is_empty() {
                dropdown.set_selected_by_name(ORCHESTRATION_ENV_NONE_LABEL, ctx_dropdown);
                return;
            }
            let all_envs = CloudAmbientAgentEnvironment::get_all(ctx_dropdown);
            if let Some(env) = all_envs.iter().find(|e| e.id.uid() == env_id) {
                dropdown.set_selected_by_name(&env.model().string_model.name, ctx_dropdown);
            }
        });
    }
    if let Some(host_picker) = handles.host_picker.clone() {
        let worker_host = match &state.execution_mode {
            RunAgentsExecutionMode::Remote { worker_host, .. } => worker_host.clone(),
            RunAgentsExecutionMode::Local => ORCHESTRATION_WARP_WORKER_HOST.to_string(),
        };
        host_picker.update(ctx, |dropdown, ctx_dropdown| {
            dropdown.set_selected_by_name(&worker_host, ctx_dropdown);
        });
    }
}

// ── Adaptive picker layout ──────────────────────────────────────────

/// Lays out children horizontally at a fixed width when they all fit,
/// otherwise stacks them vertically at full available width.
///
/// Switches to vertical when `n * picker_width + (n-1) * spacing` exceeds
/// the available width from the incoming size constraint.
struct AdaptivePickerRow {
    children: Vec<Box<dyn Element>>,
    picker_width: f32,
    spacing: f32,
    is_vertical: bool,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl AdaptivePickerRow {
    fn new(picker_width: f32, spacing: f32) -> Self {
        Self {
            children: Vec::new(),
            picker_width,
            spacing,
            is_vertical: false,
            size: None,
            origin: None,
        }
    }

    fn add_child(&mut self, child: Box<dyn Element>) {
        self.children.push(child);
    }

    fn finish(self) -> Box<dyn Element> {
        Box::new(self)
    }
}

impl Element for AdaptivePickerRow {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let n = self.children.len();
        if n == 0 {
            self.size = Some(Vector2F::zero());
            return Vector2F::zero();
        }

        let total_horizontal =
            self.picker_width * n as f32 + self.spacing * n.saturating_sub(1) as f32;

        self.is_vertical = total_horizontal > constraint.max.x();

        if self.is_vertical {
            let width = constraint.max.x();
            let mut total_height = 0.0f32;
            for (i, child) in self.children.iter_mut().enumerate() {
                if i > 0 {
                    total_height += self.spacing;
                }
                let child_constraint =
                    SizeConstraint::new(vec2f(width, 0.), vec2f(width, f32::INFINITY));
                let child_size = child.layout(child_constraint, ctx, app);
                total_height += child_size.y();
            }
            let size = vec2f(width, total_height);
            self.size = Some(size);
            size
        } else {
            let mut max_height = 0.0f32;
            for child in self.children.iter_mut() {
                let child_constraint = SizeConstraint::new(
                    vec2f(self.picker_width, 0.),
                    vec2f(self.picker_width, f32::INFINITY),
                );
                let child_size = child.layout(child_constraint, ctx, app);
                max_height = max_height.max(child_size.y());
            }
            let size = vec2f(total_horizontal, max_height);
            self.size = Some(size);
            size
        }
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        for child in &mut self.children {
            child.after_layout(ctx, app);
        }
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let mut current = origin;
        if self.is_vertical {
            for (i, child) in self.children.iter_mut().enumerate() {
                if i > 0 {
                    current += vec2f(0., self.spacing);
                }
                child.paint(current, ctx, app);
                if let Some(size) = child.size() {
                    current += vec2f(0., size.y());
                }
            }
        } else {
            for (i, child) in self.children.iter_mut().enumerate() {
                if i > 0 {
                    current += vec2f(self.spacing, 0.);
                }
                child.paint(current, ctx, app);
                let advance = child.size().map_or(self.picker_width, |s| s.x());
                current += vec2f(advance, 0.);
            }
        }
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        let mut handled = false;
        for child in &mut self.children {
            handled |= child.dispatch_event(event, ctx, app);
        }
        handled
    }
}

// ── Render helpers ──────────────────────────────────────────────────

pub fn render_mode_toggle<A: OrchestrationControlAction>(
    is_remote: bool,
    handles: &OrchestrationPickerHandles<A>,
    appearance: &Appearance,
    active_segment_bg: Option<Fill>,
    full_width: bool,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label = Text::new(
        "Agent location".to_string(),
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(blended_colors::text_disabled(theme, theme.surface_1()))
    .finish();

    let local_segment = render_segment_button::<A>(
        "Local",
        !is_remote,
        A::execution_mode_toggled(false),
        handles.local_toggle.clone(),
        appearance,
        active_segment_bg,
    );
    let cloud_segment = render_segment_button::<A>(
        "Cloud",
        is_remote,
        A::execution_mode_toggled(true),
        handles.cloud_toggle.clone(),
        appearance,
        active_segment_bg,
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
    let segmented_control = if full_width {
        segmented_control
    } else {
        ConstrainedBox::new(segmented_control)
            .with_width(ORCHESTRATION_PICKER_MAX_WIDTH)
            .finish()
    };

    let cross_axis = if full_width {
        CrossAxisAlignment::Stretch
    } else {
        CrossAxisAlignment::Start
    };
    Flex::column()
        .with_cross_axis_alignment(cross_axis)
        .with_child(Container::new(label).with_margin_bottom(6.).finish())
        .with_child(segmented_control)
        .finish()
}

fn render_segment_button<A: OrchestrationControlAction>(
    label: &str,
    is_active: bool,
    on_click: A,
    mouse_state: MouseStateHandle,
    appearance: &Appearance,
    active_bg_override: Option<Fill>,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label_owned = label.to_string();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.monospace_font_size() + 1.;
    let active_text_color = blended_colors::text_main(theme, theme.surface_1());
    let inactive_text_color = blended_colors::text_disabled(theme, theme.surface_1());
    let segment_active_bg = active_bg_override
        .unwrap_or_else(|| warp_core::ui::theme::color::internal_colors::fg_overlay_4(theme));
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

pub fn render_picker_row<A: OrchestrationControlAction>(
    state: &OrchestrationEditState,
    handles: &OrchestrationPickerHandles<A>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    render_picker_row_with_layout(state, handles, appearance, false)
}

/// Renders pickers vertically at full width when `vertical` is true,
/// or in the original horizontal layout when false.
pub fn render_picker_row_with_layout<A: OrchestrationControlAction>(
    state: &OrchestrationEditState,
    handles: &OrchestrationPickerHandles<A>,
    appearance: &Appearance,
    vertical: bool,
) -> Box<dyn Element> {
    let is_remote = state.execution_mode.is_remote();

    if vertical {
        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(12.);

        let add = |col: &mut Flex, label: &str, picker: Option<Box<dyn Element>>| {
            col.add_child(render_picker_column(label, picker, appearance));
        };

        add(
            &mut column,
            "Agent harness",
            handles
                .harness_picker
                .as_ref()
                .map(|p| ChildView::new(p).finish()),
        );
        if is_remote {
            add(
                &mut column,
                "Host",
                handles
                    .host_picker
                    .as_ref()
                    .map(|p| ChildView::new(p).finish()),
            );
            add(
                &mut column,
                "Environment",
                handles
                    .environment_picker
                    .as_ref()
                    .map(|p| ChildView::new(p).finish()),
            );
        }
        add(
            &mut column,
            "Base model",
            handles
                .model_picker
                .as_ref()
                .map(|p| ChildView::new(p).finish()),
        );

        Container::new(column.finish())
            .with_margin_top(12.)
            .finish()
    } else {
        let mut row = AdaptivePickerRow::new(ORCHESTRATION_PICKER_MAX_WIDTH, 12.);

        let add_picker =
            |row: &mut AdaptivePickerRow, label: &str, picker: Option<Box<dyn Element>>| {
                let col = render_picker_column(label, picker, appearance);
                row.add_child(col);
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
}

pub fn render_picker_column(
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

pub fn render_validation_error(
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

pub fn empty_env_recommendation_message(
    execution_mode: &RunAgentsExecutionMode,
    app: &AppContext,
) -> Option<String> {
    let RunAgentsExecutionMode::Remote {
        environment_id,
        worker_host,
        ..
    } = execution_mode
    else {
        return None;
    };
    if !environment_id.trim().is_empty() {
        return None;
    }
    if !worker_host.eq_ignore_ascii_case(ORCHESTRATION_WARP_WORKER_HOST) {
        return None;
    }
    let env_count = CloudAmbientAgentEnvironment::get_all(app).len();
    Some(if env_count > 0 {
        "We recommend selecting an environment for cloud agents.".to_string()
    } else {
        "We recommend creating an environment for cloud agents.".to_string()
    })
}
