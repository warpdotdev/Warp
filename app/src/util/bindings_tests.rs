use warpui::platform::OperatingSystem;
use warpui::{
    keymap::{EditableBinding, Keystroke, Trigger},
    App,
};

use crate::{
    util::bindings::{
        custom_tag_to_keystroke, keybinding_name_to_display_string, trigger_to_keystroke,
        CustomAction,
    },
    workspace::WorkspaceAction,
};

#[test]
fn test_keybinding_name_to_display_string() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            ctx.register_editable_bindings([
                EditableBinding::new(
                    "workspace:show_settings",
                    "Open settings",
                    WorkspaceAction::ShowSettings,
                )
                .with_key_binding("cmd-,"),
                EditableBinding::new(
                    "workspace:toggle_resource_center",
                    "Toggle Resource Center",
                    WorkspaceAction::ToggleResourceCenter,
                ),
            ]);

            let displayed_keybinding = if OperatingSystem::get().is_mac() {
                "⌘,"
            } else {
                "Logo ,"
            };
            assert_eq!(
                Some(displayed_keybinding),
                keybinding_name_to_display_string("workspace:show_settings", ctx).as_deref()
            );

            assert_eq!(
                None,
                keybinding_name_to_display_string("workspace:toggle_resource_center", ctx)
            );

            ctx.set_custom_trigger(
                "workspace:show_settings".to_owned(),
                Trigger::Keystrokes(vec![Keystroke::parse("cmd-shift-<").unwrap()]),
            );

            let displayed_keybinding = if OperatingSystem::get().is_mac() {
                "⇧⌘<"
            } else {
                "Shift Logo <"
            };
            assert_eq!(
                Some(displayed_keybinding),
                keybinding_name_to_display_string("workspace:show_settings", ctx).as_deref()
            );

            ctx.set_custom_trigger(
                "workspace:toggle_resource_center".to_owned(),
                Trigger::Keystrokes(vec![Keystroke::parse("cmd-alt-/").unwrap()]),
            );

            let expected_keybinding = if OperatingSystem::get().is_mac() {
                "⌥⌘/"
            } else {
                "Alt Logo /"
            };
            assert_eq!(
                Some(expected_keybinding),
                keybinding_name_to_display_string("workspace:toggle_resource_center", ctx)
                    .as_deref()
            );
        });
    });
}

#[cfg(target_os = "macos")]
#[test]
fn test_cmd_w_defaults_to_close_window_on_macos() {
    use warpui::actions::StandardAction;

    assert_eq!(
        Some("cmd-w"),
        custom_tag_to_keystroke(CustomAction::CloseWindow.into())
            .as_ref()
            .map(|keystroke| keystroke.normalized())
            .as_deref()
    );
    assert_eq!(
        None,
        custom_tag_to_keystroke(CustomAction::CloseCurrentSession.into())
    );
    assert_eq!(
        Some("cmd-w"),
        trigger_to_keystroke(&Trigger::Standard(StandardAction::Close))
            .as_ref()
            .map(|keystroke| keystroke.normalized())
            .as_deref()
    );
}
