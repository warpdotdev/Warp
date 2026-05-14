use warp::integration_testing::{
    step::new_step_with_default_assertions,
    tab::assert_pane_title,
    terminal::{
        assert_open_in_warp_banner_open, execute_command_for_single_terminal_in_tab,
        util::ExpectedExitStatus, wait_until_bootstrapped_single_pane_for_tab,
    },
    view_getters::terminal_view,
};

use super::{new_builder, Builder};

pub fn test_open_in_warp_banner() -> Builder {
    new_builder()
        .with_setup(|utils| {
            std::fs::write(utils.test_dir().join("README.md"), "# Hello, world!")
                .expect("Couldn't create README.md");
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            execute_command_for_single_terminal_in_tab(
                0,
                "cat README.md".to_string(),
                ExpectedExitStatus::Success,
                (),
            )
            .add_assertion(assert_open_in_warp_banner_open(0, 0)),
        )
        .with_step(
            new_step_with_default_assertions("Click Open in Warp banner")
                .with_click_on_saved_position_fn(|app, window_id| {
                    let view = terminal_view(app, window_id, 0, 0);
                    format!("open_in_warp_banner_button_{}", view.id())
                }),
        )
        .with_step(
            new_step_with_default_assertions("Wait for Markdown file to open")
                .add_assertion(assert_pane_title(0, 1, "README.md")),
        )
}
