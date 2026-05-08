use warp_core::ui::appearance::Appearance;
use warpui::elements::{Element, Icon};

pub const WARPER_LOADER_FRAME_COUNT: usize = 14;

const WARPER_LOADER_FRAMES: [&str; WARPER_LOADER_FRAME_COUNT] = [
    "bundled/svg/warper-loading-000000.svg",
    "bundled/svg/warper-loading-000001.svg",
    "bundled/svg/warper-loading-000002.svg",
    "bundled/svg/warper-loading-000003.svg",
    "bundled/svg/warper-loading-000004.svg",
    "bundled/svg/warper-loading-000005.svg",
    "bundled/svg/warper-loading-000006.svg",
    "bundled/svg/warper-loading-000007.svg",
    "bundled/svg/warper-loading-000008.svg",
    "bundled/svg/warper-loading-000009.svg",
    "bundled/svg/warper-loading-000010.svg",
    "bundled/svg/warper-loading-000011.svg",
    "bundled/svg/warper-loading-000012.svg",
    "bundled/svg/warper-loading-000013.svg",
];

pub fn warper_loader_icon(frame: usize, appearance: &Appearance) -> Box<dyn Element> {
    Icon::new(
        WARPER_LOADER_FRAMES[frame % WARPER_LOADER_FRAME_COUNT],
        appearance.theme().foreground(),
    )
    .finish()
}
