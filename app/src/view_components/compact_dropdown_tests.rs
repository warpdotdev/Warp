use warp_core::ui::appearance::Appearance;
use warpui::{platform::WindowStyle, App, View};

use crate::{menu::MenuVariant, ui_components::icons::Icon};

use super::{CompactDropdown, CompactDropdownItem};

#[derive(Debug, Clone)]
struct TestAction;

/// Baseline test that the view can render.
#[test]
fn test_render() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| Appearance::mock());
        let (_, view) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
            CompactDropdown::<TestAction>::new(MenuVariant::Fixed, ctx)
        });

        // This should not panic.
        view.read(&app, |view, ctx| view.render(ctx));

        // After adding some items, rendering should still not panic.
        view.update(&mut app, |view, ctx| {
            view.set_items(
                [
                    CompactDropdownItem::new(Icon::Folder, "Folder", TestAction),
                    CompactDropdownItem::new(Icon::Gear, "Gear", TestAction),
                ],
                ctx,
            );
        });
        view.read(&app, |view, ctx| view.render(ctx));
    })
}
