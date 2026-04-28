mod entry;
mod query;
mod setup_command;
mod setup_command_text;

pub use entry::*;
pub use query::*;
pub use setup_command::*;
pub use setup_command_text::*;

use warpui::prelude::Container;
use warpui::{AppContext, Element, ModelHandle};

use crate::ai::blocklist::block::view_impl::{
    WithContentItemSpacing, CONTENT_ITEM_VERTICAL_MARGIN,
};
use crate::terminal::view::PADDING_LEFT;

use super::AmbientAgentViewModel;

/// Wraps a cloud-mode setup row with spacing appropriate for the run's harness: non-oz
/// runs use terminal `PADDING_LEFT` so the row lines up with the harness CLI's command
/// block once it takes over; Oz runs use the standard agent-output indent.
pub(super) fn cloud_mode_setup_row_spacing(
    element: Box<dyn Element>,
    ambient_agent_view_model: &ModelHandle<AmbientAgentViewModel>,
    app: &AppContext,
) -> Container {
    if ambient_agent_view_model
        .as_ref(app)
        .is_third_party_harness()
    {
        Container::new(element)
            .with_margin_left(*PADDING_LEFT)
            .with_margin_right(*PADDING_LEFT)
            .with_margin_bottom(CONTENT_ITEM_VERTICAL_MARGIN)
    } else {
        element.with_agent_output_item_spacing(app)
    }
}
