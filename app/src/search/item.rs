use ordered_float::OrderedFloat;
use warp_core::ui::theme::Fill;
use warpui::fonts::FamilyId;
use warpui::{Action, AppContext, Element};

use crate::appearance::Appearance;

use super::result_renderer::ItemHighlightState;

/// Compact, type-erased metadata used by detail/sidecar views (e.g. the
/// V2 cloud-mode slash command menu sidecar) to render an expanded preview
/// of a search item without re-running the row layout. Items that don't
/// have a meaningful title/description pair (e.g. action shortcuts, static
/// separators) should return `None` from `SearchItem::detail_data`.
#[derive(Clone)]
pub struct SearchItemDetail {
    /// Display title shown at the top of the sidecar.
    pub title: String,
    /// Optional description body shown beneath the title. `None` for items
    /// like saved prompts that only have a title.
    pub description: Option<String>,
    /// Font family used to render the title. Typically matches the row's
    /// title font (monospace for slash commands and skills, UI font for
    /// saved prompts).
    pub title_font_family: FamilyId,
}

/// Location where icon should be rendered relative to the [`SearchItem`].
pub enum IconLocation {
    /// Icon should be centered within the element.
    Centered,
    /// Icon should be rendered at the top of the element, offset by `margin_top`.
    Top { margin_top: f32 },
}

/// A trait representing a result from searching for a command.
pub trait SearchItem: Send + Sync {
    /// The action that is dispatched when an item is accepted.
    type Action: Action + Clone;

    /// Returns whether this item should be treated as a multiline row.
    ///
    /// This is used for styling decisions in renderers (e.g. applying extra vertical padding).
    fn is_multiline(&self) -> bool {
        false
    }

    /// Returns an [`Icon`] element to be rendered in a location determined by
    /// [`SearchItem::icon_location`]
    fn render_icon(
        &self,
        highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element>;

    /// Returns the location in which the icon should be rendered relative to the search item.
    fn icon_location(&self, _appearance: &Appearance) -> IconLocation {
        IconLocation::Centered
    }

    /// Returns an element to be rendered as the "body" of the item in the results list.
    fn render_item(
        &self,
        highlight_state: ItemHighlightState,
        app: &AppContext,
    ) -> Box<dyn Element>;

    fn item_background(
        &self,
        highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Option<Fill> {
        highlight_state.container_background_fill(appearance)
    }

    /// Optionally returns an [`Element`] to be rendered within a floating details panel when the
    /// item is highlighted in the results list.
    ///
    /// If this returns `None`, no details panel is shown for the item.
    fn render_details(&self, _: &AppContext) -> Option<Box<dyn Element>> {
        None
    }

    /// Returns a priority tier used to group result types.
    ///
    /// Results are primarily ordered by this tier (higher tier wins). Scores are only compared
    /// within the same tier.
    fn priority_tier(&self) -> u8 {
        0
    }

    /// Returns the "score" of the item used to rank the item in the results list.
    fn score(&self) -> OrderedFloat<f64>;

    /// Returns the [`CommandSearchItemAction`] to be emitted when the result is "accepted".
    fn accept_result(&self) -> Self::Action;

    /// Returns the [`CommandSearchItemAction`] to be emitted when the result is "executed".
    fn execute_result(&self) -> Self::Action;

    /// Returns the text that describes this item for accessibility purposes.
    fn accessibility_label(&self) -> String;

    /// Returns the a11y help message, if any, that describes this item.
    fn accessibility_help_message(&self) -> Option<String> {
        None
    }

    /// Returns an optional deduplication key for this item.
    /// Items with the same deduplication key will be considered duplicates.
    fn dedup_key(&self) -> Option<String> {
        None
    }

    /// Returns whether this item is a static separator,
    /// meaning it is a non-interactible item that should act as a simple UI element.
    fn is_static_separator(&self) -> bool {
        false
    }

    /// Returns whether this item is disabled.
    /// Disabled items cannot be accepted or selected.
    fn is_disabled(&self) -> bool {
        false
    }

    /// Returns an optional tooltip string to display when hovering over this item.
    fn tooltip(&self) -> Option<String> {
        None
    }

    /// Optional structured metadata used by detail/sidecar views to render a
    /// richer preview of this item. Default returns `None`; items that
    /// participate in sidecar rendering (currently only the V2 cloud-mode
    /// slash command menu) override this.
    fn detail_data(&self) -> Option<SearchItemDetail> {
        None
    }
}
