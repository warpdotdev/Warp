//! Inline config block rendered on plan cards when the conversation has
//! an active `OrchestrationConfigSnapshot`. Shows a "Use orchestration"
//! toggle, Cloud/Local picker, and run-wide config dropdowns.

use ai::agent::action::RunAgentsExecutionMode;
use ai::agent::orchestration_config::OrchestrationConfigStatus;
use pathfinder_color::ColorU;
use std::collections::HashMap;
use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Flex, Hoverable,
    MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement, Radius, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::platform::Cursor;
use warpui::{AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::blocklist::inline_action::orchestration_controls::{
    self as oc, OrchestrationControlAction, OrchestrationEditState, OrchestrationPickerHandles,
};
use crate::ai::blocklist::BlocklistAIHistoryEvent;
use crate::ai::document::ai_document_model::AIDocumentModel;
use crate::ai::harness_availability::{HarnessAvailabilityEvent, HarnessAvailabilityModel};
use crate::ai::llms::{LLMPreferences, LLMPreferencesEvent};
use crate::appearance::Appearance;
use crate::ui_components::blended_colors;
use crate::BlocklistAIHistoryModel;
use warp_core::ui::theme::WarpTheme;

/// Renders a pill-shaped toggle switch (36×18) matching the Figma mock.
fn render_pill_toggle(is_on: bool, theme: &WarpTheme) -> Box<dyn Element> {
    let thumb_size = 14.;
    let thumb = ConstrainedBox::new(
        Container::new(Empty::new().finish())
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
            .with_background_color(ColorU::white())
            .finish(),
    )
    .with_width(thumb_size)
    .with_height(thumb_size)
    .finish();

    let track_bg = if is_on {
        theme.accent().into_solid()
    } else {
        warp_core::ui::theme::color::internal_colors::fg_overlay_4(theme).into_solid()
    };
    let alignment = if is_on {
        MainAxisAlignment::End
    } else {
        MainAxisAlignment::Start
    };
    let switch_inner = Flex::row()
        .with_main_axis_alignment(alignment)
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(Container::new(thumb).with_uniform_padding(2.).finish())
        .finish();
    ConstrainedBox::new(
        Container::new(switch_inner)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(9.)))
            .with_background_color(track_bg)
            .finish(),
    )
    .with_width(36.)
    .with_height(18.)
    .finish()
}

const CONFIG_BLOCK_HEADER: &str = "Use orchestration";
const CONFIG_BLOCK_DESCRIPTION: &str =
    "Break this work into coordinated streams with multiple agents.";
const BASE_MODEL_HELPER: &str = "The primary model all agents will use.";

// ── Action type ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum OrchestrationConfigBlockAction {
    ToggleApproval,
    ToggleDetails,
    ExecutionModeToggled { is_remote: bool },
    ModelChanged { model_id: String },
    HarnessChanged { harness_type: String },
    EnvironmentChanged { environment_id: String },
    WorkerHostChanged { worker_host: String },
}

impl OrchestrationControlAction for OrchestrationConfigBlockAction {
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

// ── View ────────────────────────────────────────────────────────────

pub struct OrchestrationConfigBlockView {
    conversation_id: AIConversationId,
    edit_state: OrchestrationEditState,
    pickers: OrchestrationPickerHandles<OrchestrationConfigBlockAction>,
    is_approved: bool,
    details_expanded: bool,
    pickers_initialized: bool,
    toggle_mouse_state: MouseStateHandle,
    details_mouse_state: MouseStateHandle,
    /// UI-only per-harness model memory so switching harnesses preserves
    /// the user's previous model selection for each harness.
    saved_model_per_harness: HashMap<String, String>,
}

impl OrchestrationConfigBlockView {
    pub fn new_with_conversation_id(
        conversation_id: AIConversationId,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let history = BlocklistAIHistoryModel::as_ref(ctx);
        let (edit_state, is_approved) = history
            .conversation(&conversation_id)
            .and_then(|conv| {
                conv.orchestration_config().map(|config| {
                    (
                        OrchestrationEditState::from_orchestration_config(config),
                        conv.orchestration_status().is_approved(),
                    )
                })
            })
            .unwrap_or_else(|| {
                (
                    OrchestrationEditState::from_run_agents_fields(
                        "auto",
                        "oz",
                        &RunAgentsExecutionMode::Local,
                    ),
                    false,
                )
            });

        ctx.subscribe_to_model(
            &BlocklistAIHistoryModel::handle(ctx),
            move |me, _, event, ctx| {
                if let BlocklistAIHistoryEvent::OrchestrationConfigUpdated {
                    conversation_id: cid,
                } = event
                {
                    if *cid == me.conversation_id {
                        me.refresh_from_model(ctx);
                    }
                }
            },
        );

        // Repopulate the model picker when available LLMs change (Oz
        // harness only — non-Oz harnesses get their catalog from
        // HarnessAvailabilityModel, not LLMPreferences).
        ctx.subscribe_to_model(&LLMPreferences::handle(ctx), |me, _, event, ctx| {
            if let LLMPreferencesEvent::UpdatedAvailableLLMs = event {
                if let Some(handle) = &me.pickers.model_picker {
                    let is_local = !me.edit_state.execution_mode.is_remote();
                    oc::populate_model_picker_for_harness(
                        handle,
                        &me.edit_state.model_id,
                        &me.edit_state.harness_type,
                        is_local,
                        ctx,
                    );
                }
            }
        });

        // Repopulate harness and model pickers when the server-provided
        // harness list or harness model catalogs change.
        ctx.subscribe_to_model(
            &HarnessAvailabilityModel::handle(ctx),
            |me, _, event, ctx| {
                if let HarnessAvailabilityEvent::Changed = event {
                    if me.pickers_initialized {
                        oc::repopulate_all_pickers(&mut me.edit_state, &me.pickers, ctx);
                    }
                    ctx.notify();
                }
            },
        );

        let mut view = Self {
            conversation_id,
            edit_state,
            pickers: OrchestrationPickerHandles::default(),
            is_approved,
            details_expanded: false,
            pickers_initialized: false,
            toggle_mouse_state: MouseStateHandle::default(),
            details_mouse_state: MouseStateHandle::default(),
            saved_model_per_harness: HashMap::new(),
        };
        if view.is_approved {
            view.ensure_pickers(ctx);
        }
        view
    }

    fn refresh_from_model(&mut self, ctx: &mut ViewContext<Self>) {
        let history = BlocklistAIHistoryModel::as_ref(ctx);
        if let Some(conv) = history.conversation(&self.conversation_id) {
            if let Some(config) = conv.orchestration_config() {
                self.edit_state = OrchestrationEditState::from_orchestration_config(config);
                self.is_approved = conv.orchestration_status().is_approved();
                if self.pickers_initialized {
                    oc::repopulate_all_pickers(&mut self.edit_state, &self.pickers, ctx);
                }
                ctx.notify();
            }
        }
    }

    fn ensure_pickers(&mut self, ctx: &mut ViewContext<Self>) {
        if self.pickers_initialized {
            return;
        }

        let appearance = Appearance::as_ref(ctx);
        let (styles, colors) = oc::picker_styles(appearance);

        // When the agent didn't specify a model, fall back to the
        // conversation's current base model so the picker isn't blank.
        let display_model_id = if self.edit_state.model_id.trim().is_empty() {
            BlocklistAIHistoryModel::as_ref(ctx)
                .conversation(&self.conversation_id)
                .and_then(|conv| conv.latest_exchange())
                .map(|ex| ex.model_id.to_string())
                .unwrap_or_default()
        } else {
            self.edit_state.model_id.clone()
        };
        let is_local = !self.edit_state.execution_mode.is_remote();
        let model_handle = oc::new_standard_picker_dropdown(&colors, ctx);
        model_handle.update(ctx, |d, c| d.set_use_overlay_layer(true, c));
        oc::populate_model_picker_for_harness(
            &model_handle,
            &display_model_id,
            &self.edit_state.harness_type,
            is_local,
            ctx,
        );
        self.pickers.model_picker = Some(model_handle);

        let harness_handle = oc::new_standard_picker_dropdown(&colors, ctx);
        harness_handle.update(ctx, |d, c| d.set_use_overlay_layer(true, c));
        oc::populate_harness_picker(&harness_handle, &self.edit_state.harness_type, ctx);
        self.pickers.harness_picker = Some(harness_handle);

        let initial_env = match &self.edit_state.execution_mode {
            RunAgentsExecutionMode::Remote { environment_id, .. } => environment_id.as_str(),
            RunAgentsExecutionMode::Local => "",
        };
        let env_handle = oc::create_environment_picker(initial_env, &styles, ctx);
        env_handle.update(ctx, |d, c| d.set_use_overlay_layer(true, c));
        self.pickers.environment_picker = Some(env_handle);

        let initial_host = match &self.edit_state.execution_mode {
            RunAgentsExecutionMode::Remote { worker_host, .. } => worker_host.as_str(),
            RunAgentsExecutionMode::Local => oc::ORCHESTRATION_WARP_WORKER_HOST,
        };
        let host_handle = oc::new_standard_picker_dropdown(&colors, ctx);
        host_handle.update(ctx, |d, c| d.set_use_overlay_layer(true, c));
        oc::populate_host_picker(&host_handle, initial_host, ctx);
        self.pickers.host_picker = Some(host_handle);

        self.pickers_initialized = true;
        oc::sync_picker_selections(&self.edit_state, &self.pickers, ctx);
    }

    fn apply_field_change(&mut self, ctx: &mut ViewContext<Self>) {
        let config = self.edit_state.to_orchestration_config();
        let status = if self.is_approved {
            OrchestrationConfigStatus::Approved
        } else {
            OrchestrationConfigStatus::Disapproved
        };
        let conversation_id = self.conversation_id;
        // Preserve the existing plan_id from the conversation so we don't
        // clobber it when the user only edits config fields.
        let plan_id = BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&conversation_id)
            .and_then(|conv| conv.orchestration_plan_id().map(str::to_string));
        AIDocumentModel::handle(ctx).update(ctx, |model, ctx| {
            model.set_orchestration_config(conversation_id, config, status, plan_id, ctx);
        });
    }
}

impl Entity for OrchestrationConfigBlockView {
    type Event = ();
}

impl View for OrchestrationConfigBlockView {
    fn ui_name() -> &'static str {
        "OrchestrationConfigBlockView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        // Header row: "Use orchestration" + pill toggle switch
        let header_label = Text::new(
            CONFIG_BLOCK_HEADER.to_string(),
            appearance.ui_font_family(),
            16.,
        )
        .with_color(blended_colors::text_main(theme, theme.background()))
        .with_style(Properties::default().weight(Weight::Bold))
        .finish();

        let is_on = self.is_approved;

        let header_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(warpui::elements::Expanded::new(1.0, header_label).finish())
            .with_child(
                Hoverable::new(self.toggle_mouse_state.clone(), move |_| {
                    render_pill_toggle(is_on, theme)
                })
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(OrchestrationConfigBlockAction::ToggleApproval);
                })
                .with_cursor(Cursor::PointingHand)
                .finish(),
            )
            .finish();
        column.add_child(header_row);

        // Description
        let description = Text::new(
            CONFIG_BLOCK_DESCRIPTION.to_string(),
            appearance.ui_font_family(),
            appearance.monospace_font_size(),
        )
        .with_color(blended_colors::text_main(theme, theme.background()))
        .finish();
        column.add_child(Container::new(description).with_margin_top(8.).finish());

        // "View details" row + expandable controls (only when approved)
        if self.is_approved {
            // Divider
            let divider = Container::new(
                ConstrainedBox::new(Empty::new().finish())
                    .with_height(1.)
                    .finish(),
            )
            .with_background_color(theme.surface_2().into_solid())
            .finish();
            column.add_child(Container::new(divider).with_margin_top(8.).finish());

            // "View details" link row
            let chevron_icon = if self.details_expanded {
                warp_core::ui::Icon::ChevronDown
            } else {
                warp_core::ui::Icon::ChevronRight
            };
            let disabled_text_color = blended_colors::text_disabled(theme, theme.background());
            let details_text = Text::new(
                "View details".to_string(),
                appearance.ui_font_family(),
                appearance.monospace_font_size() + 1.,
            )
            .with_color(disabled_text_color)
            .finish();
            let chevron = ConstrainedBox::new(
                chevron_icon
                    .to_warpui_icon(warp_core::ui::theme::Fill::Solid(disabled_text_color))
                    .finish(),
            )
            .with_width(14.)
            .with_height(14.)
            .finish();
            let details_link = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(2.)
                .with_child(details_text)
                .with_child(chevron)
                .finish();
            let details_link_hoverable =
                Hoverable::new(self.details_mouse_state.clone(), move |_| details_link)
                    .on_click(|ctx, _, _| {
                        ctx.dispatch_typed_action(OrchestrationConfigBlockAction::ToggleDetails);
                    })
                    .with_cursor(Cursor::PointingHand)
                    .finish();
            let details_row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(details_link_hoverable)
                .finish();
            column.add_child(Container::new(details_row).with_margin_top(8.).finish());

            // Expanded controls
            if self.details_expanded {
                // Cloud / Local mode toggle (full width)
                let active_seg_bg =
                    warp_core::ui::theme::color::internal_colors::accent_overlay_2(theme);
                column.add_child(
                    Container::new(oc::render_mode_toggle(
                        self.edit_state.execution_mode.is_remote(),
                        &self.pickers,
                        appearance,
                        Some(active_seg_bg),
                        true,
                    ))
                    .with_margin_top(12.)
                    .finish(),
                );

                // Pickers stacked vertically
                column.add_child(oc::render_picker_row_with_layout(
                    &self.edit_state,
                    &self.pickers,
                    appearance,
                    true,
                ));

                // Helper text
                let helper = Text::new(
                    BASE_MODEL_HELPER.to_string(),
                    appearance.ui_font_family(),
                    appearance.monospace_font_size() - 1.,
                )
                .with_color(blended_colors::text_disabled(theme, theme.background()))
                .finish();
                column.add_child(Container::new(helper).with_margin_top(4.).finish());

                // Validation
                if let Some(reason) = self.edit_state.accept_disabled_reason() {
                    column.add_child(oc::render_validation_error(
                        reason,
                        theme.ui_error_color(),
                        appearance,
                    ));
                } else if let Some(message) =
                    oc::empty_env_recommendation_message(&self.edit_state.execution_mode, app)
                {
                    column.add_child(oc::render_validation_error(
                        message,
                        theme.ui_warning_color(),
                        appearance,
                    ));
                }
            }
        }

        // Outer container with accent styling per Figma
        Container::new(column.finish())
            .with_uniform_padding(12.)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .with_background(warp_core::ui::theme::color::internal_colors::accent_overlay_1(theme))
            .with_border(warpui::elements::Border::all(1.).with_border_fill(theme.accent()))
            .finish()
    }
}

impl TypedActionView for OrchestrationConfigBlockView {
    type Action = OrchestrationConfigBlockAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            OrchestrationConfigBlockAction::ToggleApproval => {
                self.is_approved = !self.is_approved;
                if self.is_approved && !self.pickers_initialized {
                    self.ensure_pickers(ctx);
                }
                self.apply_field_change(ctx);
                ctx.notify();
            }
            OrchestrationConfigBlockAction::ToggleDetails => {
                self.details_expanded = !self.details_expanded;
                if self.details_expanded && !self.pickers_initialized {
                    self.ensure_pickers(ctx);
                }
                ctx.notify();
            }
            OrchestrationConfigBlockAction::ExecutionModeToggled { is_remote } => {
                let conversation_id = self.conversation_id;
                oc::apply_execution_mode_change(
                    &mut self.edit_state,
                    &self.pickers,
                    *is_remote,
                    |ctx| {
                        BlocklistAIHistoryModel::as_ref(ctx)
                            .conversation(&conversation_id)
                            .and_then(|conv| conv.latest_exchange())
                            .map(|ex| ex.model_id.to_string())
                    },
                    ctx,
                );
                self.apply_field_change(ctx);
                ctx.notify();
            }
            OrchestrationConfigBlockAction::ModelChanged { model_id } => {
                self.edit_state.model_id = model_id.clone();
                self.apply_field_change(ctx);
                ctx.notify();
            }
            OrchestrationConfigBlockAction::HarnessChanged { harness_type } => {
                let conversation_id = self.conversation_id;
                oc::apply_harness_change(
                    &mut self.edit_state,
                    &mut self.saved_model_per_harness,
                    &self.pickers,
                    harness_type,
                    |ctx| {
                        BlocklistAIHistoryModel::as_ref(ctx)
                            .conversation(&conversation_id)
                            .and_then(|conv| conv.latest_exchange())
                            .map(|ex| ex.model_id.to_string())
                    },
                    ctx,
                );
                self.apply_field_change(ctx);
                ctx.notify();
            }
            OrchestrationConfigBlockAction::EnvironmentChanged { environment_id } => {
                self.edit_state.set_environment_id(environment_id.clone());
                self.apply_field_change(ctx);
                ctx.notify();
            }
            OrchestrationConfigBlockAction::WorkerHostChanged { worker_host } => {
                self.edit_state.set_worker_host(worker_host.clone());
                self.apply_field_change(ctx);
                ctx.notify();
            }
        }
    }
}
