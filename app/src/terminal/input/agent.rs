use super::{
    common::{
        add_command_xray_overlay, add_input_suggestions_overlays, add_voltron_overlay,
        add_workflow_info_overlay, wrap_input_with_terminal_padding_and_focus_handler,
    },
    Input, InputAction, InputDropTargetData,
};
use crate::{
    ai::blocklist::{
        agent_view::{
            agent_view_bg_fill,
            shortcuts::{render_agent_shortcuts_view, AgentShortcutsViewContext},
            AgentViewState,
        },
        InputType,
    },
    appearance::Appearance,
    context_chips::spacing::{self},
    features::FeatureFlag,
    settings::InputModeSettings,
    terminal::{settings::TerminalSettings, view::TerminalAction},
    BlocklistAIHistoryModel,
};
use warp_core::settings::Setting;
use warpui::{
    elements::{
        Border, Container, DropTarget, Element, Flex, Hoverable, ParentElement, SavePosition, Stack,
    },
    presenter::ChildView,
    AppContext, SingletonEntity as _,
};

impl Input {
    /// Renders the input when there is an active `AgentView`.
    ///
    /// Only used when `FeatureFlag::AgentView` is enabled.
    pub(super) fn render_agent_input(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let menu_positioning = self.menu_positioning(app);

        let _model = self.model.lock();

        // We should likely rework this stack to not need to use `with_constrain_absolute_children`,
        // by reworking the positioning of the children to not depend on this.
        let mut stack = Stack::new().with_constrain_absolute_children();

        let input_mode = *InputModeSettings::as_ref(app).input_mode.value();

        let mut column = Flex::column();

        if let Some(banner) =
            self.render_input_banner(appearance, app, input_mode, /*is_compact_mode=*/ false)
        {
            column.add_child(
                Container::new(banner)
                    .with_margin_top(spacing::UDI_CHIP_MARGIN)
                    .finish(),
            );
        }

        let ai_input_model = self.ai_input_model.as_ref(app);

        if FeatureFlag::ImageAsContext.is_enabled()
            && matches!(ai_input_model.input_type(), InputType::AI)
        {
            if let Some(images) = self.render_attachment_chips(appearance) {
                column.add_child(
                    Container::new(images)
                        .with_margin_top(spacing::UDI_CHIP_MARGIN)
                        .finish(),
                );
            }
        }

        let terminal_spacing = TerminalSettings::as_ref(app)
            .terminal_input_spacing(appearance.line_height_ratio(), app);
        column.add_child(
            Container::new(self.render_input_box(/*show_vim_status=*/ false, appearance, app))
                .with_margin_top(
                    terminal_spacing.prompt_to_editor_padding
                        * spacing::UDI_PROMPT_BOTTOM_PADDING_FACTOR,
                )
                .finish(),
        );
        column.add_child(
            SavePosition::new(
                ChildView::new(&self.agent_input_footer).finish(),
                &self.prompt_save_position_id(),
            )
            .finish(),
        );

        stack.add_child(wrap_input_with_terminal_padding_and_focus_handler(
            self.is_active_session(app),
            column.finish(),
            false,
        ));

        if let Some(selected_workflow_state) = self.workflows_state.selected_workflow_state.as_ref()
        {
            if selected_workflow_state.should_show_more_info_view {
                add_workflow_info_overlay(
                    &mut stack,
                    selected_workflow_state,
                    self.size_info(app).pane_height_px().as_f32(),
                    menu_positioning,
                );
            }
        }

        if self.is_voltron_open && self.is_pane_focused(app) {
            add_voltron_overlay(&mut stack, &self.voltron_view, menu_positioning);
        }

        if self.is_pane_focused(app) {
            add_input_suggestions_overlays(self, &mut stack, appearance, menu_positioning, app);
        }

        if let Some(token_description) = &self.command_x_ray_description {
            add_command_xray_overlay(
                self,
                &mut stack,
                token_description,
                appearance,
                menu_positioning,
                app,
            );
        }

        let drop_target = DropTarget::new(
            SavePosition::new(stack.finish(), &self.status_free_input_save_position_id()).finish(),
            InputDropTargetData::new(self.weak_view_handle.clone()),
        )
        .finish();

        let border_color = if !self.ai_input_model.as_ref(app).is_ai_input_enabled()
            && !self.suggestions_mode_model.as_ref(app).is_slash_commands()
            && !self.slash_command_model.as_ref(app).state().is_detected_command()
            // If NLD, don't color the border if the input is empty, because the current
            // classification is necessarily stale (intentionally inherited from the last
            // classification prior to clearing the input)
            && (!self.editor.as_ref(app).is_empty(app)
                || self.ai_input_model.as_ref(app).is_input_type_locked())
        {
            appearance.theme().ansi_fg_blue()
        } else {
            styles::default_border_color(appearance.theme())
        };

        let mut input = Container::new(
            Hoverable::new(self.hoverable_handle.clone(), |_| drop_target)
                .on_hover(|is_hovered, ctx, _app, _position| {
                    ctx.dispatch_typed_action(InputAction::SetUDIHovered(is_hovered));
                })
                .on_middle_click(|ctx, _app, _position| {
                    ctx.dispatch_typed_action(TerminalAction::MiddleClickOnInput)
                })
                .finish(),
        )
        .with_border(Border::top(1.).with_border_color(border_color))
        .with_padding_bottom(4.);

        if self.agent_view_controller.as_ref(app).is_inline() {
            input = input.with_background(agent_view_bg_fill(app));
        }

        let input = input.finish();

        let mut column = Flex::column();

        if self
            .suggestions_mode_model
            .as_ref(app)
            .is_inline_model_selector()
        {
            column.add_child(ChildView::new(&self.inline_model_selector_view).finish());
        } else if FeatureFlag::InlineProfileSelector.is_enabled()
            && self
                .suggestions_mode_model
                .as_ref(app)
                .is_profile_selector()
        {
            column.add_child(ChildView::new(&self.inline_profile_selector_view).finish());
        } else if self.suggestions_mode_model.as_ref(app).is_slash_commands() {
            column.add_child(ChildView::new(&self.inline_slash_commands_view).finish());
        } else if self.suggestions_mode_model.as_ref(app).is_prompts_menu() {
            column.add_child(ChildView::new(&self.inline_prompts_menu_view).finish());
        } else if self
            .suggestions_mode_model
            .as_ref(app)
            .is_conversation_menu()
        {
            column.add_child(ChildView::new(&self.inline_conversation_menu_view).finish());
        } else if FeatureFlag::ListSkills.is_enabled()
            && self.suggestions_mode_model.as_ref(app).is_skill_menu()
        {
            column.add_child(ChildView::new(&self.inline_skill_selector_view).finish());
        } else if self.suggestions_mode_model.as_ref(app).is_user_query_menu() {
            column.add_child(ChildView::new(&self.user_query_menu_view).finish());
        } else if self.suggestions_mode_model.as_ref(app).is_rewind_menu() {
            column.add_child(ChildView::new(&self.rewind_menu_view).finish());
        } else if self
            .suggestions_mode_model
            .as_ref(app)
            .is_inline_history_menu()
        {
            column.add_child(ChildView::new(&self.inline_history_menu_view).finish());
        } else if self.suggestions_mode_model.as_ref(app).is_repos_menu() {
            column.add_child(ChildView::new(&self.inline_repos_menu_view).finish());
        } else if self.suggestions_mode_model.as_ref(app).is_plan_menu() {
            column.add_child(ChildView::new(&self.inline_plan_menu_view).finish());
        }

        if self
            .agent_shortcut_view_model
            .as_ref(app)
            .is_shortcut_view_open()
        {
            let agent_view_controller = self.agent_view_controller.as_ref(app);
            let (is_ambient_agent, has_submitted_first_prompt) =
                match agent_view_controller.agent_view_state() {
                    AgentViewState::Active {
                        conversation_id,
                        origin,
                        ..
                    } => {
                        let is_ambient_agent = origin.is_ambient_agent();
                        let has_submitted_first_prompt = if is_ambient_agent {
                            BlocklistAIHistoryModel::as_ref(app)
                                .conversation(conversation_id)
                                .is_some_and(|c| c.initial_user_query().is_some())
                        } else {
                            true
                        };
                        (is_ambient_agent, has_submitted_first_prompt)
                    }
                    // When inactive, show all shortcuts (treat as not-cloud and not in the zero-state).
                    AgentViewState::Inactive => (false, true),
                };

            column.add_child(render_agent_shortcuts_view(
                AgentShortcutsViewContext {
                    is_ambient_agent,
                    has_submitted_first_prompt,
                },
                app,
            ));
        }
        column.add_children([ChildView::new(&self.agent_status_view).finish(), input]);

        let mut outer_stack = Stack::new().with_constrain_absolute_children();
        outer_stack.add_child(column.finish());

        SavePosition::new(outer_stack.finish(), &self.save_position_id()).finish()
    }
}

pub mod styles {
    use pathfinder_color::ColorU;
    use warp_core::ui::theme::WarpTheme;

    use crate::ui_components::blended_colors;

    pub fn default_border_color(theme: &WarpTheme) -> ColorU {
        blended_colors::neutral_2(theme)
    }
}
