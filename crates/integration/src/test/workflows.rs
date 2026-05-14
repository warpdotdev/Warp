use warp::{
    integration_testing::{
        self,
        step::new_step_with_default_assertions,
        terminal::{
            execute_command_for_single_terminal_in_tab, util::ExpectedExitStatus,
            wait_until_bootstrapped_single_pane_for_tab,
        },
        view_of_type,
    },
    workflows::CategoriesView,
};
use warpui::{async_assert_eq, integration::TestStep, ViewHandle};

use super::{new_builder, Builder, TEST_ONLY_ASSETS};

/// Adds a workflow file, containing two workflows, to a `.warp/workflows`
/// directory under a git repository and verifies that the workflows appear
/// in the workflow menu.
pub fn test_loading_project_workflows() -> Builder {
    new_builder()
        .with_setup(move |utils| {
            utils.set_env("WARP_CONFIG_WATCHER_DELAY_MS", Some((10).to_string()));
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            TestStep::new("Should have no project workflows").add_named_assertion(
                "Should have no project workflows",
                |app, window_id| {
                    let workflows: ViewHandle<CategoriesView> = view_of_type(app, window_id, 0);

                    workflows.read(app, |workflows, _| {
                        // Note that this can be a synchronous assertion because unlike the next assertion,
                        // we don't have concurrency with a WarpConfig watcher thread
                        async_assert_eq!(
                            workflows.project_workflows().count(),
                            0,
                            "There should not be any project workflows"
                        )
                    })
                },
            ),
        )
        // Create a git repository in the `repo` subdirectory.
        .with_step(execute_command_for_single_terminal_in_tab(
            0,
            "git init repo && cd repo".into(),
            ExpectedExitStatus::Success,
            (),
        ))
        .with_step(
            TestStep::new("Write a new file containing two workflows").with_setup(|utils| {
                integration_testing::create_file_from_assets(
                    TEST_ONLY_ASSETS,
                    "test_workflow.yaml",
                    &utils
                        .test_dir()
                        .join("repo/.warp/workflows/test_workflow.yaml"),
                );
            }),
        )
        .with_step(
            new_step_with_default_assertions(
                "Open the workflows browser to refresh the list of project workflows",
            )
            .with_keystrokes(&["ctrl-shift-R"]),
        )
        .with_step(
            TestStep::new("Verify the workflows were loaded successfully").add_named_assertion(
                "The two added workflows should be in the view",
                |app, window_id| {
                    let workflows: ViewHandle<CategoriesView> = view_of_type(app, window_id, 0);

                    let num_workflows =
                        workflows.read(app, |workflows, _| workflows.project_workflows().count());
                    async_assert_eq!(num_workflows, 2)
                },
            ),
        )
}
