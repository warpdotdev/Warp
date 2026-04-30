//! Integration tests for workspace-level behavior.

use std::fs;

use settings::Setting as _;
use warp::integration_testing::terminal::assert_long_running_block_executing_for_single_terminal_in_tab;
use warp::integration_testing::view_getters::terminal_view;
use warp::integration_testing::workspace::{assert_tab_count, press_native_modal_button};
use warp::{
    cmd_or_ctrl_shift,
    integration_testing::{
        pane_group::assert_focused_pane_index,
        step::new_step_with_default_assertions,
        terminal::{
            assert_active_session_local_path, execute_command, util::ExpectedExitStatus,
            wait_until_bootstrapped_pane, wait_until_bootstrapped_single_pane_for_tab,
        },
    },
    settings::PaneSettings,
    workspace::NEW_TAB_BUTTON_POSITION_ID,
};
use warpui::{async_assert, integration::TestStep, SingletonEntity};

use crate::{util::skip_if_powershell_core_2303, Builder};

use super::new_builder;

pub fn test_active_session_follows_focus() -> Builder {
    new_builder()
        // TODO(CORE-2732): Flakey on Powershell (Linux)
        .set_should_run_test(skip_if_powershell_core_2303)
        .with_setup(|utils| {
            fs::create_dir(utils.test_dir().join("dir1")).expect("Couldn't create subdirectory");
            fs::create_dir(utils.test_dir().join("dir2")).expect("Couldn't create subdirectory");
        })
        .with_step(
            new_step_with_default_assertions("Ensure initial active session is set")
                .add_assertion(assert_active_session_local_path("~")),
        )
        .with_step(
            new_step_with_default_assertions("Create another session in the same tab")
                .with_keystrokes(&[cmd_or_ctrl_shift("d")]),
        )
        .with_step(wait_until_bootstrapped_pane(0, 1))
        .with_step(
            execute_command(0, 1, "cd dir1".to_string(), ExpectedExitStatus::Success, ())
                .add_assertion(assert_active_session_local_path("~/dir1")),
        )
        .with_step(
            new_step_with_default_assertions("Switch to the first session")
                .with_keystrokes(&["cmdorctrl-meta-left"])
                .add_assertion(assert_active_session_local_path("~")),
        )
        .with_step(
            new_step_with_default_assertions("Open a new tab")
                .with_keystrokes(&[cmd_or_ctrl_shift("t")]),
        )
        .with_step(wait_until_bootstrapped_pane(1, 0))
        .with_step(
            execute_command(1, 0, "cd dir2".to_string(), ExpectedExitStatus::Success, ())
                .add_assertion(assert_active_session_local_path("~/dir2")),
        )
        .with_step(
            new_step_with_default_assertions("Switch to the first tab")
                .with_keystrokes(&["cmdorctrl-1"])
                .add_assertion(assert_active_session_local_path("~")),
        )
        .with_step(
            new_step_with_default_assertions("Close the tab")
                .with_keystrokes(&[cmd_or_ctrl_shift("w"), cmd_or_ctrl_shift("w")])
                .add_assertion(assert_active_session_local_path("~/dir2")),
        )
}

pub fn test_focus_panes_on_hover() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Create a new session in a split pane")
                .with_keystrokes(&[cmd_or_ctrl_shift("d")])
                .add_assertion(assert_focused_pane_index(0, 1)),
        )
        .with_step(wait_until_bootstrapped_pane(0, 1))
        .with_step(
            new_step_with_default_assertions("Enable focus pane on hover").add_assertion(
                |app, _| {
                    PaneSettings::handle(app).update(app, |settings, ctx| {
                        settings
                            .focus_panes_on_hover
                            .set_value(true, ctx)
                            .expect("error updating setting");
                        async_assert!(*settings.focus_panes_on_hover)
                    })
                },
            ),
        )
        .with_step(
            new_step_with_default_assertions("Hover over the initial pane's terminal")
                .with_hover_on_saved_position_fn(|app, window_id| {
                    let terminal_view = terminal_view(app, window_id, 0, 0);
                    terminal_view.read(app, |terminal, _| terminal.terminal_position_id())
                })
                .add_assertion(assert_focused_pane_index(0, 0)),
        )
        .with_step(
            new_step_with_default_assertions("Hover back over the second pane's terminal")
                .with_hover_on_saved_position_fn(|app, window_id| {
                    let terminal_view = terminal_view(app, window_id, 0, 1);
                    terminal_view.read(app, |terminal, _| terminal.terminal_position_id())
                })
                .add_assertion(assert_focused_pane_index(0, 1)),
        )
        .with_step(
            new_step_with_default_assertions("Create another new session in a split pane")
                .with_keystrokes(&[cmd_or_ctrl_shift("d")]),
        )
        .with_step(wait_until_bootstrapped_pane(0, 2))
        .with_step(
            new_step_with_default_assertions(
                "Make sure the pane is focused even though the mouse is over the first pane",
            )
            .add_assertion(assert_focused_pane_index(0, 2)),
        )
        .with_step(
            new_step_with_default_assertions("Disable focus pane on hover").add_assertion(
                |app, _| {
                    PaneSettings::handle(app).update(app, |settings, ctx| {
                        settings
                            .focus_panes_on_hover
                            .set_value(false, ctx)
                            .expect("error updating setting");
                        async_assert!(!*settings.focus_panes_on_hover)
                    })
                },
            ),
        )
        .with_step(
            new_step_with_default_assertions(
                "Hover over the initial pane's terminal and make sure it's not focused",
            )
            .with_hover_on_saved_position_fn(|app, window_id| {
                let terminal_view = terminal_view(app, window_id, 0, 0);
                terminal_view.read(app, |terminal, _| terminal.terminal_position_id())
            })
            .add_assertion(assert_focused_pane_index(0, 2)),
        )
}

pub fn test_close_tab_with_long_running_process() -> Builder {
    new_builder()
        .set_should_run_test(|| cfg!(any(target_os = "linux", target_os = "freebsd")))
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Open a new tab")
                .with_click_on_saved_position(NEW_TAB_BUTTON_POSITION_ID),
        )
        .with_step(wait_until_bootstrapped_single_pane_for_tab(1))
        .with_step(
            TestStep::new("Execute long-running command")
                .with_typed_characters(&["python3"])
                .with_keystrokes(&["enter"])
                .add_assertion(
                    assert_long_running_block_executing_for_single_terminal_in_tab(true, 1),
                ),
        )
        .with_step(
            new_step_with_default_assertions("Close the tab with a long-running command")
                .with_hover_over_saved_position("close_tab_button:1")
                .with_click_on_saved_position("close_tab_button:1")
                .add_assertion(assert_tab_count(2))
                .add_assertion(
                    // The tab should not yet be closed.
                    assert_long_running_block_executing_for_single_terminal_in_tab(true, 1),
                ),
        )
        // Press the confirm button in the modal.
        .with_step(press_native_modal_button(0))
        .with_step(TestStep::new("Wait for tab to close").add_assertion(assert_tab_count(1)))
}
