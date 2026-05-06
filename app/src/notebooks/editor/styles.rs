use warp_core::ui::appearance::Appearance;
use warp_editor::render::model::{BlockSpacing, IndentableBlockSpacing, PlaceholderVisibility, RichTextStyles};
use warpui::elements::{Margin, Padding};
use warpui::text_layout::DEFAULT_TOP_BOTTOM_RATIO;
use crate::notebooks::editor::{NOTEBOOK_BASELINE_RATIO, NOTEBOOK_LINE_HEIGHT_RATIO};
use crate::settings::FontSettings;

pub trait RichTextStylesExt {
    fn new_for_notebook(appearance: &Appearance, font_settings: &FontSettings) -> Self;

    fn new_with_default_line_height(appearance: &Appearance, font_settings: &FontSettings) -> Self;
}

impl RichTextStylesExt for RichTextStyles {
    fn new_for_notebook(appearance: &Appearance, font_settings: &FontSettings) -> Self {
        super::rich_text_styles_internal(appearance, font_settings, NOTEBOOK_LINE_HEIGHT_RATIO, NOTEBOOK_BASELINE_RATIO)
    }

    fn new_with_default_line_height(appearance: &Appearance, font_settings: &FontSettings) -> Self {
        // Bump the line height ratio slightly so soft-wrapped and hard-wrapped lines
        // have consistent, comfortable spacing.
        let line_height_ratio = appearance.line_height_ratio() + 0.15;
        let compact_text_spacing = BlockSpacing {
            margin: Margin::uniform(0.),
            padding: Padding::uniform(0.),
        };
        let compact_indentable = IndentableBlockSpacing::new(Margin::uniform(0.), 20.);
        let mut styles = super::rich_text_styles_internal(
            appearance,
            font_settings,
            line_height_ratio,
            DEFAULT_TOP_BOTTOM_RATIO,
        );
        styles.minimum_paragraph_height = None;
        styles.cursor_width = 3.;
        styles.placeholder_visibility = PlaceholderVisibility::WhenBufferEmpty;
        styles.placeholder_text = Some("Leave a comment".into());
        // Only compact text block spacings; keep the default code block spacing.
        styles.block_spacings.text = compact_text_spacing;
        styles.block_spacings.header = compact_text_spacing;
        styles.block_spacings.task_list = compact_indentable.clone();
        styles.block_spacings.ordered_list = compact_indentable.clone();
        styles.block_spacings.unordered_list = compact_indentable;
        styles
    }
}
