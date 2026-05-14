use warpui::ViewContext;

use super::{ResourceCenterMainView, Section};

pub fn sections(_ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<Section> {
    vec![Section::Changelog()]
}
