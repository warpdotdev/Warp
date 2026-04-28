/// Returns the backing scale factor of the main display.
///
/// This is used to convert between pixel coordinates (as returned by screenshot tools)
/// and point coordinates (as used by CGEvent and screencapture).
pub fn main_display_scale_factor() -> f64 {
    use dispatch2::run_on_main;
    use objc2_app_kit::NSScreen;

    run_on_main(|mtm| {
        NSScreen::mainScreen(mtm)
            .map(|screen| screen.backingScaleFactor())
            .unwrap_or(1.0)
    })
}
