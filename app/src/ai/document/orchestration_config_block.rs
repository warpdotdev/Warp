//! Inline config block rendered on plan cards when the conversation has
//! an active `OrchestrationConfigSnapshot`. Shows a "Use orchestration"
//! toggle, Cloud/Local picker, and run-wide config dropdowns.

use ai::agent::action::RunAgentsExecutionMode;
use ai::agent::orchestration_config::{OrchestrationConfig, OrchestrationConfigStatus};
use warpui::elements::{
    Container, CornerRadius, CrossAxisAlignment, Flex, ParentElement, Radius, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::{AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext};

use crate::ai::blocklist::inline_action::orchestration_controls::{
    self as oc, OrchestrationControlAction, OrchestrationEditState, OrchestrationPickerHandles,
};
use crate::ai::document::ai_document_model::{AIDocumentModel, AIDocumentModelEvent};
use crate::appearance::Appearance;
use crate::ui_components::blended_colors;

const CONFIG_BLOCK_HEADER: &str = "Use orchestration";
const CONFIG_BLOCK_DESCRIPTION: &str =
    "Break this work into coordinated streams handled by specialized agents. \
     Each agent focuses on a specific part of the plan\u{2014}design, instrumentation, \
     backend, testing, and rollout\u{2014}while sharing context to stay aligned. \
     This approach speeds up execution and reduces gaps between steps.";
const BASE_MODEL_HELPER: &str = "The primary model all agents will use.";

// ── Action type ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum OrchestrationConfigBlockAction {
    ToggleApproval,
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
    edit_state: OrchestrationEditState,
    pickers: OrchestrationPickerHandles<OrchestrationConfigBlockAction>,
    is_approved: bool,
    pickers_initialized: bool,
}

impl OrchestrationConfigBlockView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let doc_model = AIDocumentModel::as_ref(ctx);
        let (edit_state, is_approved) = match doc_model.active_orchestration_config() {
            Some(config) => (
                OrchestrationEditState::from_orchestration_config(config),
                doc_model.orchestration_status().is_approved(),
            ),
            None => (
                OrchestrationEditState::from_run_agents_fields("auto", "oz", &RunAgentsExecutionMode::Local),
                false,
            ),
        };

        ctx.subscribe_to_model(
            &AIDocumentModel::handle(ctx),
            |me, _, event, ctx| {
                if let AIDocumentModelEvent::OrchestrationConfigUpdated = event {
                    me.refresh_from_model(ctx);
                }
            },
        );

        Self {
            edit_state,
            pickers: OrchestrationPickerHandles::default(),
            is_approved,
            pickers_initialized: false,
        }
    }

    fn refresh_from_model(&mut self, ctx: &mut ViewContext<Self>) {
        let doc_model = AIDocumentModel::as_ref(ctx);
        if let Some(config) = doc_model.active_orchestration_config() {
            self.edit_state = OrchestrationEditState::from_orchestration_config(config);
            self.is_approved = doc_model.orchestration_status().is_approved();
            if self.pickers_initialized {
                oc::sync_picker_selections(&self.edit_state, &self.pickers, ctx);
            }
            ctx.notify();
        }
    }

    fn ensure_pickers(&mut self, ctx: &mut ViewContext<Self>) {
        if self.pickers_initialized {
            return;
        }

        let appearance = Appearance::as_ref(ctx);
        let (styles, colors) = oc::picker_styles(appearance);

        let model_handle = oc::new_standard_picker_dropdown(&colors, ctx);
        oc::populate_model_picker(&model_handle, &self.edit_state.model_id, ctx);
        self.pickers.model_picker = Some(model_handle);

        let harness_handle = oc::new_standard_picker_dropdown(&colors, ctx);
        oc::populate_harness_picker(&harness_handle, &self.edit_state.harness_type, ctx);
        self.pickers.harness_picker = Some(harness_handle);

        let initial_env = match &self.edit_state.execution_mode {
            RunAgentsExecutionMode::Remote { environment_id, .. } => environment_id.as_str(),
            RunAgentsExecutionMode::Local => "",
        };
        let env_handle = oc::create_environment_picker(initial_env, &styles, ctx);
        self.pickers.environment_picker = Some(env_handle);

        let initial_host = match &self.edit_state.execution_mode {
            RunAgentsExecutionMode::Remote { worker_host, .. } => worker_host.as_str(),
            RunAgentsExecutionMode::Local => oc::ORCHESTRATION_WARP_WORKER_HOST,
        };
        let host_handle = oc::new_standard_picker_dropdown(&colors, ctx);
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
        AIDocumentModel::handle(ctx).update(ctx, |model, ctx| {
            model.set_orchestration_config(config, status, None, ctx);
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

        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        // Header: "Use orchestration" + toggle
        let header_label = Text::new(
            CONFIG_BLOCK_HEADER.to_string(),
            appearance.ui_font_family(),
            16.,
        )
        .with_color(blended_colors::text_main(theme, theme.background()))
        .with_style(Properties::default().weight(Weight::Bold))
        .finish();

        let toggle_text = if self.is_approved { "On" } else { "Off" };
        let toggle_indicator = Text::new(
            toggle_text.to_string(),
            appearance.ui_font_family(),
            appearance.monospace_font_size(),
        )
        .with_color(if self.is_approved {
            theme.accent().into_solid()
        } else {
            blended_colors::text_disabled(theme, theme.background())
        })
        .finish();

        let header_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                warpui::elements::Expanded::new(1.0, header_label).finish(),
            )
            .with_child(
                warpui::elements::Hoverable::new(
                    warpui::elements::MouseStateHandle::default(),
                    move |_| toggle_indicator,
                )
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(OrchestrationConfigBlockAction::ToggleApproval);
                })
                .with_cursor(warpui::platform::Cursor::PointingHand)
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
        column.add_child(
            Container::new(description)
                .with_margin_top(8.)
                .finish(),
        );

        // Controls (only when approved)
        if self.is_approved {
            // Mode toggle
            column.add_child(
                Container::new(oc::render_mode_toggle(
                    self.edit_state.execution_mode.is_remote(),
                    &self.pickers,
                    appearance,
                ))
                .with_margin_top(12.)
                .finish(),
            );

            // Picker row
            column.add_child(oc::render_picker_row(&self.edit_state, &self.pickers, appearance));

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
            OrchestrationConfigBlockAction::ExecutionModeToggled { is_remote } => {
                self.edit_state.toggle_execution_mode_to_remote(*is_remote);
                oc::sync_picker_selections(&self.edit_state, &self.pickers, ctx);
                self.apply_field_change(ctx);
                ctx.notify();
            }
            OrchestrationConfigBlockAction::ModelChanged { model_id } => {
                self.edit_state.model_id = model_id.clone();
                self.apply_field_change(ctx);
                ctx.notify();
            }
            OrchestrationConfigBlockAction::HarnessChanged { harness_type } => {
                self.edit_state.harness_type = harness_type.clone();
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
