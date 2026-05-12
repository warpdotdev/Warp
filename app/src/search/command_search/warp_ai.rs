use crate::{
    ai_assistant::AI_ASSISTANT_LOGO_COLOR,
    appearance::Appearance,
    features::FeatureFlag,
    search::{
        command_search::searcher::CommandSearchItemAction,
        data_source::{Query, QueryResult},
        item::SearchItem,
        mixer::{DataSourceRunErrorWrapper, SyncDataSource},
        result_renderer::ItemHighlightState,
    },
    themes::theme::Blend,
    ui_components::icons::Icon as UIIcon,
    util::color::{ContrastingColor, MinimumAllowedContrast},
};

use ordered_float::OrderedFloat;
use warp_core::ui::builder;
use warpui::{
    elements::{ConstrainedBox, Container, Text},
    AppContext, Element, SingletonEntity,
};

const OPEN_WARP_AI_ITEM_BODY_TEXT: &str = "Ask Warp AI for command suggestions";
const TRANSLATE_WITH_WARP_AI_ITEM_BODY_TEXT: &str = "Translate into shell command using Warp AI";

#[derive(Clone, Debug)]
pub enum WarpAISearchItem {
    /// Translates the query within command search.
    Translate,

    /// Opens WarpAI with the query.
    Open,
}

impl WarpAISearchItem {
    fn item_body_text(&self) -> &'static str {
        match self {
            WarpAISearchItem::Translate => TRANSLATE_WITH_WARP_AI_ITEM_BODY_TEXT,
            WarpAISearchItem::Open => OPEN_WARP_AI_ITEM_BODY_TEXT,
        }
    }
}

impl SearchItem for WarpAISearchItem {
    type Action = CommandSearchItemAction;

    fn render_icon(
        &self,
        highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        // Since the Warp AI logo color is hardcoded, let's find the best
        // contrasting color depending on the user's theme and the item's selected state.
        let command_search_background = appearance.theme().surface_1();
        let item_background_color = match highlight_state.container_background_fill(appearance) {
            None => command_search_background,
            Some(highlight) => command_search_background.blend(&highlight),
        };

        let icon = if FeatureFlag::AgentMode.is_enabled() {
            UIIcon::Oz
                .to_warpui_icon(
                    appearance
                        .theme()
                        .main_text_color(appearance.theme().accent()),
                )
                .finish()
        } else {
            let color = (AI_ASSISTANT_LOGO_COLOR).on_background(
                item_background_color.into_solid(),
                MinimumAllowedContrast::NonText,
            );
            UIIcon::AiAssistant.to_warpui_icon(color.into()).finish()
        };

        Container::new(
            ConstrainedBox::new(icon)
                .with_width(styles::icon_size(appearance))
                .with_height(styles::icon_size(appearance))
                .finish(),
        )
        .with_margin_right(8.)
        .finish()
    }

    fn render_item(
        &self,
        highlight_state: ItemHighlightState,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        Text::new_inline(
            self.item_body_text(),
            appearance.monospace_font_family(),
            appearance.monospace_font_size(),
        )
        .autosize_text(builder::MIN_FONT_SIZE)
        .with_color(highlight_state.main_text_fill(appearance).into_solid())
        .finish()
    }

    fn render_details(&self, _: &AppContext) -> Option<Box<dyn Element>> {
        None
    }

    fn score(&self) -> OrderedFloat<f64> {
        // Decided to try using a score of 0 instead of a score of -f32::MAX.
        // This means it's not necessarily the lowest-ranked item, but often is.
        OrderedFloat(0.)
    }

    fn accept_result(&self) -> CommandSearchItemAction {
        match self {
            WarpAISearchItem::Translate => CommandSearchItemAction::TranslateUsingWarpAI,
            WarpAISearchItem::Open => CommandSearchItemAction::OpenWarpAI,
        }
    }

    fn execute_result(&self) -> CommandSearchItemAction {
        match self {
            WarpAISearchItem::Translate => CommandSearchItemAction::TranslateUsingWarpAI,
            WarpAISearchItem::Open => CommandSearchItemAction::OpenWarpAI,
        }
    }

    fn accessibility_label(&self) -> String {
        format!("Warp AI: {}", self.item_body_text())
    }
}

/// OpenWarp 只保留同步入口:打开 BYOP Agent 或把自然语言写回输入框。
/// 云端“自然语言生成命令”异步源已删除。
pub struct WarpAIDataSource;

impl WarpAIDataSource {
    pub fn new() -> Self {
        Self
    }
}

impl SyncDataSource for WarpAIDataSource {
    type Action = CommandSearchItemAction;

    fn run_query(
        &self,
        query: &Query,
        _app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        if query.filters.is_empty() {
            Ok(vec![WarpAISearchItem::Translate.into()])
        } else {
            // Since the query matched, the `#` filter must be applied in this case.
            Ok(vec![WarpAISearchItem::Open.into()])
        }
    }
}

mod styles {
    use crate::appearance::Appearance;

    /// Returns the icon size to be used for the 'sparkle' icon in the AI command search result.
    /// The icon appeaars smaller than its size would indicate, so make a bit larger than icons
    /// used for other search result types.
    pub(super) fn icon_size(appearance: &Appearance) -> f32 {
        appearance.monospace_font_size() + 2.
    }
}
