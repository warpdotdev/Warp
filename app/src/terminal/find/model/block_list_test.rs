use std::collections::HashMap;

use itertools::Itertools;
use warpui::App;

use crate::terminal::{
    block_filter::BlockFilterQuery,
    find::{
        model::{block_list::run_find_on_block_list, FindOptions},
        BlockGridMatch,
    },
    model::{index::Point, terminal_model::BlockSortDirection},
    GridType, TerminalModel,
};

use super::{BlockListFindRun, BlockListMatch};

impl BlockListFindRun {
    fn all_matches(&self) -> &[BlockListMatch] {
        &self.matches[..]
    }
}

#[test]
fn test_run_find_on_block_list() {
    App::test((), |mut app| async move {
        let mut mock_terminal_model = TerminalModel::mock(None, None);
        mock_terminal_model.simulate_block("foobar", "foo\r\nbar\r\n");
        mock_terminal_model.simulate_block("barbaz", "bar baz\r\n");

        let run = app.update(|ctx| {
            run_find_on_block_list(
                FindOptions {
                    query: Some("bar".to_owned().into()),
                    is_regex_enabled: false,
                    is_case_sensitive: false,
                    ..Default::default()
                },
                mock_terminal_model.block_list(),
                &HashMap::new(),
                BlockSortDirection::MostRecentLast,
                ctx,
            )
        });

        // Matches should be sorted by recency at the block level, then "bottom to top" within each
        // block.
        assert_eq!(
            run.matches().collect_vec(),
            vec![
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 1, col: 0 }..=Point { row: 1, col: 2 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 3 }..=Point { row: 0, col: 5 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
            ]
        );

        assert_eq!(run.focused_match_index(), Some(0));
    });
}

#[test]
fn test_run_find_on_block_list_pin_to_top() {
    App::test((), |mut app| async move {
        let mut mock_terminal_model = TerminalModel::mock(None, None);
        mock_terminal_model.simulate_block("foobar", "foo\r\nbar\r\n");
        mock_terminal_model.simulate_block("barbaz", "bar baz\r\n");

        let run = app.update(|ctx| {
            run_find_on_block_list(
                FindOptions {
                    query: Some("bar".to_owned().into()),
                    is_regex_enabled: false,
                    is_case_sensitive: false,
                    ..Default::default()
                },
                mock_terminal_model.block_list(),
                &HashMap::new(),
                BlockSortDirection::MostRecentFirst,
                ctx,
            )
        });

        // Matches should be sorted by recency at the block level, then "top to bottom" within each
        // block.
        assert_eq!(
            run.matches().collect_vec(),
            vec![
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 3 }..=Point { row: 0, col: 5 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 1, col: 0 }..=Point { row: 1, col: 2 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
            ]
        );
    });
}

#[test]
fn test_run_find_on_block_list_case_sensitive() {
    App::test((), |mut app| async move {
        let mut mock_terminal_model = TerminalModel::mock(None, None);
        mock_terminal_model.simulate_block("foobar", "foo\r\nbar\r\n");
        mock_terminal_model.simulate_block("Barbaz", "Bar baz\r\n");

        let run = app.update(|ctx| {
            run_find_on_block_list(
                FindOptions {
                    query: Some("Bar".to_owned().into()),
                    is_regex_enabled: false,
                    is_case_sensitive: true,
                    ..Default::default()
                },
                mock_terminal_model.block_list(),
                &HashMap::new(),
                BlockSortDirection::MostRecentLast,
                ctx,
            )
        });

        // Matches should be sorted by recency at the block level, then "bottom to top" within each
        // block.
        assert_eq!(
            run.matches().collect_vec(),
            vec![
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
            ]
        );
        assert_eq!(run.focused_match_index(), Some(0));
    });
}

#[test]
fn test_run_find_on_block_list_regex_enabled() {
    App::test((), |mut app| async move {
        let mut mock_terminal_model = TerminalModel::mock(None, None);
        mock_terminal_model.simulate_block("foobar", "foo\r\nbar\r\n");
        mock_terminal_model.simulate_block("barbaz", "bar baz\r\n");

        let run = app.update(|ctx| {
            run_find_on_block_list(
                FindOptions {
                    query: Some("ba[rz]".to_owned().into()),
                    is_regex_enabled: true,
                    is_case_sensitive: false,
                    ..Default::default()
                },
                mock_terminal_model.block_list(),
                &HashMap::new(),
                BlockSortDirection::MostRecentLast,
                ctx,
            )
        });

        // Matches should be sorted by recency at the block level, then "bottom to top" within each
        // block.
        assert_eq!(
            run.matches().collect_vec(),
            vec![
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 0, col: 4 }..=Point { row: 0, col: 6 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 3 }..=Point { row: 0, col: 5 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 1, col: 0 }..=Point { row: 1, col: 2 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 3 }..=Point { row: 0, col: 5 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
            ]
        );
        assert_eq!(run.focused_match_index(), Some(0));
    });
}

#[test]
fn test_run_find_on_block_list_with_filtered_block() {
    App::test((), |mut app| async move {
        let mut mock_terminal_model = TerminalModel::mock(None, None);
        mock_terminal_model.simulate_block("foobar", "foo\r\nbar\r\n");
        mock_terminal_model.simulate_block("barbaz filter", "bar baz\r\nbar bat\r\nbar baz\r\n");

        // Filter the second block to only show lines containing "bat".
        mock_terminal_model
            .update_filter_on_block(2.into(), BlockFilterQuery::new_for_test("bat".to_owned()));

        let run = app.update(|ctx| {
            run_find_on_block_list(
                FindOptions {
                    query: Some("bar".to_owned().into()),
                    is_regex_enabled: false,
                    is_case_sensitive: false,
                    ..Default::default()
                },
                mock_terminal_model.block_list(),
                &HashMap::new(),
                BlockSortDirection::MostRecentLast,
                ctx,
            )
        });

        // Matches should be sorted by recency at the block level, then "bottom to top" within each
        // block.
        //
        // The first and third rows of the most recent block contain baz, and should be filtered.
        assert_eq!(
            run.matches().collect_vec(),
            vec![
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 1, col: 0 }..=Point { row: 1, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 1, col: 0 }..=Point { row: 1, col: 2 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
                &BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 3 }..=Point { row: 0, col: 5 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
            ]
        );

        // Check `all_matches` so we can ensure the filtered matches exist, but were filtered.
        assert_eq!(
            run.all_matches(),
            &[
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 2, col: 0 }..=Point { row: 2, col: 2 },
                    block_index: 2.into(),
                    is_filtered: true,
                }),
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 1, col: 0 }..=Point { row: 1, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: true,
                }),
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 0 }..=Point { row: 0, col: 2 },
                    block_index: 2.into(),
                    is_filtered: false,
                }),
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range: Point { row: 1, col: 0 }..=Point { row: 1, col: 2 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::PromptAndCommand,
                    range: Point { row: 0, col: 3 }..=Point { row: 0, col: 5 },
                    block_index: 1.into(),
                    is_filtered: false,
                }),
            ]
        );
    });
}
