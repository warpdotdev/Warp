use crate::ai::blocklist::usage::render_context_window_usage_icon;
use crate::ai::blocklist::view_util::format_credits;
use crate::appearance::Appearance;
use crate::persistence::model::{
    token_usage_category_display_name, ModelTokenUsage, FULL_TERMINAL_USE_CATEGORY,
    PRIMARY_AGENT_CATEGORY,
};
use crate::ui_components::blended_colors;
use std::cmp::Ordering;
use std::collections::HashMap;
use warp_core::ui::theme::color::internal_colors;
use warp_core::ui::Icon;
use warpui::elements::ConstrainedBox;
use warpui::{
    elements::{
        Border, Container, CornerRadius, CrossAxisAlignment, Empty, Flex, MainAxisSize,
        MouseStateHandle, ParentElement, Radius, Text,
    },
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Settings,
    Footer,
}

pub struct ConversationUsageInfo {
    pub credits_spent: f32,
    // Credits spent over the last block, where the block comprises
    // all agent outputs since the most recent user input.
    pub credits_spent_for_last_block: Option<f32>,
    pub tool_calls: i32,
    pub models: Vec<ModelTokenUsage>,
    pub context_window_usage: f32,
    pub files_changed: i32,
    pub lines_added: i32,
    pub lines_removed: i32,
    pub commands_executed: i32,
}

/// Timing information for the last set of agent responses
/// (all blocks since the last user input, as this is the granularity
/// at which we show the usage footer)
pub struct TimingInfo {
    /// Time to first token for the last block (in milliseconds)
    pub time_to_first_token_ms: i64,
    /// Total response time for the last block (in milliseconds)
    pub total_agent_response_time_ms: i64,
    /// Wall-to-wall response time (in milliseconds) from sending the user query
    /// to the last token in the last set of responses.
    pub wall_to_wall_response_time_ms: Option<i64>,
}

/// View to hold a conversation usage info block.
/// This is used for both the usage footer and the usage history page in settings.
pub struct ConversationUsageView {
    pub usage_info: ConversationUsageInfo,
    /// The display mode for this view.
    pub display_mode: DisplayMode,
    /// Optional timing information for the last set of responses (only shown in the footer version of this view).
    pub timing_info: Option<TimingInfo>,
    full_terminal_use_tooltip_mouse_state: MouseStateHandle,
}

impl ConversationUsageView {
    pub fn new(
        usage_info: ConversationUsageInfo,
        display_mode: DisplayMode,
        timing_info: Option<TimingInfo>,
        full_terminal_use_tooltip_mouse_state: MouseStateHandle,
    ) -> Self {
        Self {
            usage_info,
            display_mode,
            timing_info,
            full_terminal_use_tooltip_mouse_state,
        }
    }
    /// Helper to collect models grouped by category.
    /// Returns a HashMap mapping category name to list of (model_id, is_byok) tuples.
    /// Handles both category-based fields and legacy warp_tokens/byok_tokens fields.
    fn collect_models_by_category(&self) -> HashMap<String, Vec<(String, bool)>> {
        let mut entries_by_category: HashMap<String, Vec<(String, bool)>> = HashMap::new();

        // Collect from category-based fields
        for model in &self.usage_info.models {
            for (category, &tokens) in &model.warp_token_usage_by_category {
                if tokens > 0 {
                    entries_by_category
                        .entry(category.clone())
                        .or_default()
                        .push((model.model_id.clone(), false));
                }
            }
            for (category, &tokens) in &model.byok_token_usage_by_category {
                if tokens > 0 {
                    entries_by_category
                        .entry(category.clone())
                        .or_default()
                        .push((model.model_id.clone(), true));
                }
            }
        }

        // Fallback to legacy fields for backwards compatibility
        if entries_by_category.is_empty() {
            for model in &self.usage_info.models {
                if model.warp_tokens > 0 {
                    entries_by_category
                        .entry(PRIMARY_AGENT_CATEGORY.to_string())
                        .or_default()
                        .push((model.model_id.clone(), false));
                }
                if model.byok_tokens > 0 {
                    entries_by_category
                        .entry(PRIMARY_AGENT_CATEGORY.to_string())
                        .or_default()
                        .push((model.model_id.clone(), true));
                }
            }
        }

        entries_by_category
    }

    fn render_unified_layout(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let font_size = appearance.ui_font_size() + 2.;
        let text_color = blended_colors::text_main(theme, theme.surface_2());

        let mut labels: Vec<Box<dyn Element>> = vec![];
        let mut values: Vec<Box<dyn Element>> = vec![];

        // Usage summary
        labels.push(render_section_header(
            "USAGE SUMMARY".to_string(),
            appearance,
        ));
        values.push(render_section_header("".to_string(), appearance));

        if self.display_mode == DisplayMode::Footer
            && self.usage_info.credits_spent_for_last_block.is_some()
        {
            let last_block_credits = self.usage_info.credits_spent_for_last_block.unwrap();
            labels.push(render_label_text(
                "Credits spent (last response)",
                appearance,
            ));
            values.push(render_value_text(
                format_credits(last_block_credits),
                appearance,
            ));

            labels.push(render_label_text("Credits spent (total)", appearance));
            values.push(render_value_text(
                format_credits(self.usage_info.credits_spent),
                appearance,
            ));
        } else {
            labels.push(render_label_text("Credits spent", appearance));
            values.push(render_value_text(
                format_credits(self.usage_info.credits_spent),
                appearance,
            ));
        }

        labels.push(render_label_text("Tool calls", appearance));
        values.push(render_value_text(
            format_value_text(self.usage_info.tool_calls, "call"),
            appearance,
        ));

        let entries_by_category = self.collect_models_by_category();
        let mut categories: Vec<_> = entries_by_category.keys().cloned().collect();
        categories.sort_by(|a, b| match (a.as_str(), b.as_str()) {
            (PRIMARY_AGENT_CATEGORY, _) => Ordering::Less,
            (_, PRIMARY_AGENT_CATEGORY) => Ordering::Greater,
            _ => a.cmp(b),
        });

        for category in categories {
            let models = entries_by_category.get(&category).unwrap();
            if models.is_empty() {
                break;
            }

            let label_text = if category == PRIMARY_AGENT_CATEGORY && entries_by_category.len() == 1
            {
                "Models".to_string()
            } else {
                format!("Models ({})", token_usage_category_display_name(&category))
            };

            // For FULL_TERMINAL_USE_CATEGORY, add an info icon with tooltip
            if category == FULL_TERMINAL_USE_CATEGORY {
                let label_element = render_label_text(&label_text, appearance);

                let hoverable_icon = appearance
                    .ui_builder()
                    .info_button_with_tooltip(
                        font_size * 0.85,
                        "You can change which model is used for full terminal use in the AI settings page",
                        self.full_terminal_use_tooltip_mouse_state.clone(),
                    )
                    .finish();

                labels.push(
                    Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(label_element)
                        .with_child(Container::new(hoverable_icon).with_margin_left(4.).finish())
                        .finish(),
                );
            } else {
                labels.push(render_label_text(&label_text, appearance));
            }

            // Build comma-separated list of models, with BYOK indicator using Icon::Key
            let mut model_elements: Vec<Box<dyn Element>> = vec![];
            let mut sorted_models: Vec<_> = models.iter().collect();
            sorted_models.sort_by(|a, b| a.0.cmp(&b.0));

            for (i, (model_id, is_byok)) in sorted_models.iter().enumerate() {
                if i > 0 {
                    model_elements.push(
                        Text::new(", ".to_string(), appearance.ui_font_family(), font_size)
                            .with_color(text_color)
                            .finish(),
                    );
                }

                if *is_byok {
                    model_elements.push(
                        ConstrainedBox::new(Icon::Key.to_warpui_icon(text_color.into()).finish())
                            .with_width(font_size)
                            .with_height(font_size)
                            .finish(),
                    );
                    model_elements.push(
                        Container::new(
                            Text::new((*model_id).clone(), appearance.ui_font_family(), font_size)
                                .with_color(text_color)
                                .finish(),
                        )
                        .with_margin_left(4.)
                        .finish(),
                    );
                } else {
                    model_elements.push(
                        Text::new((*model_id).clone(), appearance.ui_font_family(), font_size)
                            .with_color(text_color)
                            .finish(),
                    );
                }
            }

            values.push(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_children(model_elements)
                    .finish(),
            );
        }

        labels.push(render_label_text("Context window used", appearance));
        let context_usage_str =
            format!("{}%", (self.usage_info.context_window_usage * 100.).round());
        let context_window_element = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(4.)
            .with_child(
                Text::new(context_usage_str, appearance.ui_font_family(), font_size)
                    .with_color(text_color)
                    .finish(),
            )
            .with_child(
                ConstrainedBox::new(render_context_window_usage_icon(
                    self.usage_info.context_window_usage,
                    theme,
                    None,
                ))
                .with_width(font_size)
                .with_height(font_size)
                .finish(),
            )
            .finish();
        values.push(context_window_element);

        // Space between sections
        labels.push(
            Container::new(Empty::new().finish())
                .with_margin_top(12.)
                .finish(),
        );
        values.push(
            Container::new(Empty::new().finish())
                .with_margin_top(12.)
                .finish(),
        );

        // Tool call summary
        labels.push(render_section_header(
            "TOOL CALL SUMMARY".to_string(),
            appearance,
        ));
        values.push(render_section_header("".to_string(), appearance));

        labels.push(render_label_text("Files changed", appearance));
        values.push(render_value_text(
            format_value_text(self.usage_info.files_changed, "file"),
            appearance,
        ));

        labels.push(render_label_text("Diffs applied", appearance));
        let diffs_element = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Text::new(
                    format!("+ {}", self.usage_info.lines_added),
                    appearance.ui_font_family(),
                    font_size,
                )
                .with_color(theme.ansi_fg_green())
                .finish(),
            )
            .with_child(
                Container::new(
                    ConstrainedBox::new(Empty::new().finish())
                        .with_width(4.)
                        .with_height(4.)
                        .finish(),
                )
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                .with_background(internal_colors::neutral_6(theme))
                .with_margin_left(8.)
                .with_margin_right(8.)
                .finish(),
            )
            .with_child(
                Text::new(
                    format!("- {}", self.usage_info.lines_removed),
                    appearance.ui_font_family(),
                    font_size,
                )
                .with_color(theme.ansi_fg_red())
                .finish(),
            )
            .finish();
        values.push(diffs_element);

        labels.push(render_label_text("Commands executed", appearance));
        values.push(render_value_text(
            format_value_text(self.usage_info.commands_executed, "command"),
            appearance,
        ));

        // Last response time
        if self.display_mode == DisplayMode::Footer {
            if let Some(timing) = &self.timing_info {
                if timing.time_to_first_token_ms != 0
                    || timing.total_agent_response_time_ms != 0
                    || timing.wall_to_wall_response_time_ms.is_some()
                {
                    // Space between sections
                    labels.push(
                        Container::new(Empty::new().finish())
                            .with_margin_top(12.)
                            .finish(),
                    );
                    values.push(
                        Container::new(Empty::new().finish())
                            .with_margin_top(12.)
                            .finish(),
                    );

                    // Section header
                    labels.push(render_section_header(
                        "LAST RESPONSE TIME".to_string(),
                        appearance,
                    ));
                    values.push(render_section_header("".to_string(), appearance));

                    labels.push(render_label_text("Time to first token", appearance));
                    values.push(render_value_text(
                        format!(
                            "{:.1} seconds",
                            timing.time_to_first_token_ms as f64 / 1000.0
                        ),
                        appearance,
                    ));

                    labels.push(render_label_text("Total agent response time", appearance));
                    values.push(render_value_text(
                        format!(
                            "{:.1} seconds",
                            timing.total_agent_response_time_ms as f64 / 1000.0
                        ),
                        appearance,
                    ));

                    if let Some(wall_ms) = timing.wall_to_wall_response_time_ms {
                        if wall_ms != 0 {
                            labels.push(render_label_text(
                                "Total time (including tool calls)",
                                appearance,
                            ));
                            values.push(render_value_text(
                                format!("{:.1} seconds", wall_ms as f64 / 1000.0),
                                appearance,
                            ));
                        }
                    }
                }
            }
        }

        Container::new(
            Flex::row()
                .with_spacing(8.)
                .with_child(Flex::column().with_children(labels).finish())
                .with_child(Flex::column().with_children(values).finish())
                .finish(),
        )
        .with_uniform_margin(16.)
        .finish()
    }

    /// Render the card container with display mode-specific styling.
    fn render_card_container(
        &self,
        content: Box<dyn Element>,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let mut card_container = Container::new(content).with_background(theme.surface_2());

        if let DisplayMode::Footer = self.display_mode {
            card_container = card_container
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                .with_border(Border::all(1.0).with_border_fill(theme.outline()))
                .with_uniform_margin(16.);
        } else {
            card_container =
                card_container.with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(6.)));
        }

        let mut res = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        if let DisplayMode::Footer = self.display_mode {
            res = res.with_child(
                // Top divider
                Container::new(Empty::new().finish())
                    .with_border(Border::top(2.0).with_border_fill(theme.outline()))
                    .with_overdraw_bottom(0.)
                    .finish(),
            );
        }

        res.with_child(card_container.finish()).finish()
    }
}

impl View for ConversationUsageView {
    fn ui_name() -> &'static str {
        "ConversationUsageView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        self.render_card_container(self.render_unified_layout(appearance), appearance)
    }
}

impl Entity for ConversationUsageView {
    type Event = ();
}

impl TypedActionView for ConversationUsageView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}

/// Render the main header for a usage section.
fn render_section_header(header_label: String, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let background = theme.surface_2();

    Container::new(
        Text::new(
            header_label,
            appearance.overline_font_family(),
            appearance.overline_font_size(),
        )
        .with_color(blended_colors::text_disabled(theme, background))
        .finish(),
    )
    .with_margin_bottom(4.)
    .finish()
}

/// Format a value and a label into one usage string,
/// making the label plural if the value is not 1.
fn format_value_text(value: i32, label: &str) -> String {
    format!("{} {}{}", value, label, if value == 1 { "" } else { "s" })
}

/// Helper to build a text element with consistent styling for labels.
fn render_label_text(text: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let font_size = appearance.ui_font_size() + 2.;

    Text::new(text.to_string(), appearance.ui_font_family(), font_size)
        .with_color(blended_colors::text_sub(theme, theme.surface_2()))
        .finish()
}

/// Helper to build a text element with consistent styling for values.
fn render_value_text(text: String, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let font_size = appearance.ui_font_size() + 2.;
    let text_color = blended_colors::text_main(theme, theme.surface_2());

    Text::new(text, appearance.ui_font_family(), font_size)
        .with_color(text_color)
        .finish()
}
