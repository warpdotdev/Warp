//! SearchItem implementation for conversation menu items.

use fuzzy_match::FuzzyMatchResult;
use ordered_float::OrderedFloat;
use warp_core::ui::color::coloru_with_opacity;
use warp_core::ui::theme::Fill;
use warp_core::ui::Icon;
use warpui::elements::{ConstrainedBox, Container, Highlight, ParentElement, Shrinkable, Text};
use warpui::fonts::{Properties, Style, Weight};
use warpui::prelude::{Align, CrossAxisAlignment, Flex, MainAxisAlignment, MainAxisSize};
use warpui::scene::{CornerRadius, Radius};
use warpui::text_layout::ClipConfig;
use warpui::{AppContext, Element, SingletonEntity};

use crate::ai::active_agent_views_model::{ActiveAgentViewsModel, ConversationOrTaskId};
use crate::ai::agent::conversation::ConversationStatus;
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::conversation_navigation::ConversationNavigationData;
use crate::ai::conversation_status_ui::{render_status_element, STATUS_ELEMENT_PADDING};
use crate::appearance::Appearance;
use crate::search::{ItemHighlightState, SearchItem};
use crate::terminal::input::conversations::AcceptConversation;
use crate::terminal::input::inline_menu::styles as inline_styles;
use crate::util::time_format::format_approx_duration_from_now_utc;

/// Search item for rendering a conversation in the inline conversation menu.
#[derive(Debug, Clone)]
pub(super) struct ConversationSearchItem {
    navigation_data: ConversationNavigationData,
    name_match_result: Option<FuzzyMatchResult>,
    score: OrderedFloat<f64>,
    conversation_status: Option<ConversationStatus>,
}

impl ConversationSearchItem {
    pub fn new(navigation_data: ConversationNavigationData, app: &AppContext) -> Self {
        let history_model = BlocklistAIHistoryModel::as_ref(app);
        let conversation_status = history_model
            .conversation(&navigation_data.id)
            .map(|conversation| conversation.status().clone());

        Self {
            navigation_data,
            name_match_result: None,
            score: OrderedFloat(f64::MIN),
            conversation_status,
        }
    }

    pub fn with_name_match_result(mut self, result: Option<FuzzyMatchResult>) -> Self {
        self.name_match_result = result;
        self
    }

    pub fn with_score(mut self, score: OrderedFloat<f64>) -> Self {
        self.score = score;
        self
    }
}

impl SearchItem for ConversationSearchItem {
    type Action = AcceptConversation;

    fn render_icon(
        &self,
        _highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let icon_size = inline_styles::font_size(appearance);
        let icon = match &self.conversation_status {
            Some(conversation_status) => {
                render_status_element(conversation_status, icon_size, appearance)
            }
            None => {
                let icon_color = appearance
                    .theme()
                    .sub_text_color(appearance.theme().background());
                Container::new(
                    ConstrainedBox::new(Icon::History.to_warpui_icon(icon_color).finish())
                        .with_width(icon_size)
                        .with_height(icon_size)
                        .finish(),
                )
                .with_uniform_padding(STATUS_ELEMENT_PADDING)
                .with_background(coloru_with_opacity(icon_color.into(), 10))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .finish()
            }
        };

        Container::new(icon)
            .with_margin_right(inline_styles::ICON_MARGIN)
            .finish()
    }

    fn render_item(
        &self,
        _highlight_state: ItemHighlightState,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let font_size = inline_styles::font_size(appearance);
        let background_color = inline_styles::menu_background_color(app);

        let primary_text_color = inline_styles::primary_text_color(theme, background_color.into());
        let secondary_text_color = theme.disabled_text_color(background_color.into());

        let open_conversation_ids =
            ActiveAgentViewsModel::as_ref(app).get_all_open_conversation_ids(app);
        let is_active = open_conversation_ids.contains(&ConversationOrTaskId::ConversationId(
            self.navigation_data.id,
        ));

        let secondary_suffix = " open in different pane";
        let title = &self.navigation_data.title;
        let should_show_suffix = is_active && !self.navigation_data.is_in_active_pane;
        let full_text = if should_show_suffix {
            format!("{title}{secondary_suffix}")
        } else {
            title.clone()
        };

        let mut name_text = Text::new_inline(full_text, appearance.ui_font_family(), font_size)
            .with_color(primary_text_color.into())
            .with_clip(ClipConfig::ellipsis());

        if let Some(name_match) = &self.name_match_result {
            if !name_match.matched_indices.is_empty() {
                name_text = name_text.with_single_highlight(
                    Highlight::new().with_properties(Properties::default().weight(Weight::Bold)),
                    name_match.matched_indices.clone(),
                );
            }
        }

        if should_show_suffix {
            let secondary_range = title.len()..(title.len() + secondary_suffix.len());
            name_text = name_text.with_single_highlight(
                Highlight::new()
                    .with_properties(Properties {
                        style: Style::Italic,
                        ..Default::default()
                    })
                    .with_foreground_color(secondary_text_color.into()),
                secondary_range.collect(),
            );
        }

        let mut primary_row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Shrinkable::new(1., name_text.finish()).finish());

        // We want the timestamp 'column' to have fixed width so clipping is consistent,
        // limit the timestamp width to about 10 chars.
        let timestamp = Text::new_inline(
            format_approx_duration_from_now_utc(self.navigation_data.last_updated.to_utc()),
            appearance.ui_font_family(),
            font_size,
        )
        .with_color(secondary_text_color.into())
        .finish();

        let max_timestamp_width = app
            .font_cache()
            .em_width(appearance.ui_font_family(), font_size)
            * 10.;
        primary_row.add_child(
            ConstrainedBox::new(Align::new(timestamp).right().finish())
                .with_width(max_timestamp_width)
                .finish(),
        );

        primary_row.finish()
    }

    fn item_background(
        &self,
        highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Option<Fill> {
        inline_styles::item_background(highlight_state, appearance)
    }

    fn score(&self) -> OrderedFloat<f64> {
        self.score
    }

    fn accept_result(&self) -> Self::Action {
        AcceptConversation {
            navigation_data: self.navigation_data.clone(),
        }
    }

    fn execute_result(&self) -> Self::Action {
        self.accept_result()
    }

    fn accessibility_label(&self) -> String {
        format!("Conversation: {}", self.navigation_data.title)
    }
}
