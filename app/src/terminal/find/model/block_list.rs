//! This module implements terminal find functionality for the blocklist.
use std::{collections::HashMap, iter, ops::RangeInclusive};

use itertools::Itertools;
use warpui::{units::Lines, AppContext, EntityId};

use crate::terminal::{
    model::{
        block::Block,
        blocks::{
            BlockHeight, BlockHeightItem, BlockHeightSummary, BlockList, RichContentItem,
            TotalIndex,
        },
        find::{FindConfig, RegexDFAs},
        index::Point,
        terminal_model::{BlockIndex, BlockSortDirection},
    },
    GridType,
};
use crate::view_components::find::FindDirection;

use super::{
    rich_content::{FindableRichContentHandle, RichContentMatchId},
    FindOptions,
};

const MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT: usize = 1000;

/// Runs a find operation on the blocklist using the given `options` and returns a
/// `BlockListFindRun` with the results.
///
/// If the given `options` does not contain a query, short-circuits and returns a find run with no
/// matches.
pub(super) fn run_find_on_block_list(
    mut options: FindOptions,
    block_list: &BlockList,
    findable_rich_content_views: &HashMap<EntityId, Box<dyn FindableRichContentHandle>>,
    block_sort_direction: BlockSortDirection,
    ctx: &mut AppContext,
) -> BlockListFindRun {
    let Some(dfas) = options.query.as_ref().and_then(|query| {
        RegexDFAs::new_with_config(
            query.as_str(),
            FindConfig {
                is_regex_enabled: options.is_regex_enabled,
                is_case_sensitive: options.is_case_sensitive,
            },
        )
        .ok()
    }) else {
        // Clear rich content matches.
        for rich_content_view in findable_rich_content_views.values() {
            rich_content_view.clear_matches(ctx);
        }
        return BlockListFindRun {
            options,
            block_sort_direction,
            dfas: None,
            matches: vec![],
            match_count: 0,
            is_match_count_exact: true,
            block_match_counts: HashMap::new(),
            raw_focused_match_index: None,
        };
    };

    // If find in block is enabled, find matches in selected blocks only
    if let Some(blocks_to_include_in_results) = options.blocks_to_include_in_results.as_mut() {
        // Sort blocks in descending order so that the most recent block is last, which is the order we expect.
        blocks_to_include_in_results.sort_by(|i, j| j.cmp(i));
    }

    let matches = find_next_match(
        &options,
        &dfas,
        block_list,
        findable_rich_content_views,
        block_sort_direction,
        None,
        ctx,
    )
    .into_iter()
    .collect_vec();
    let count = count_matches(
        &options,
        &dfas,
        block_list,
        findable_rich_content_views,
        block_sort_direction,
        ctx,
    );

    let raw_focused_match_index = (!matches.is_empty()).then_some(0);
    BlockListFindRun {
        dfas: Some(dfas),
        matches,
        match_count: count.count,
        is_match_count_exact: count.is_exact,
        block_match_counts: count.block_match_counts,
        raw_focused_match_index,
        options,
        block_sort_direction,
    }
}

fn find_next_match(
    options: &FindOptions,
    dfas: &RegexDFAs,
    block_list: &BlockList,
    rich_content_views: &HashMap<EntityId, Box<dyn FindableRichContentHandle>>,
    block_sort_direction: BlockSortDirection,
    current_match: Option<&BlockListMatch>,
    ctx: &mut AppContext,
) -> Option<BlockListMatch> {
    let mut found_current_match = current_match.is_none();

    if let Some(blocks_to_include_in_results) = options.blocks_to_include_in_results.as_ref() {
        for block_index in blocks_to_include_in_results {
            let agent_view_state = block_list.agent_view_state();
            let Some(block) = block_list
                .block_at(*block_index)
                .filter(|block| !block.is_empty(agent_view_state))
            else {
                continue;
            };
            if block.height(agent_view_state) == Lines::zero() {
                continue;
            }

            for find_match in run_find_on_block(dfas, block, *block_index, block_sort_direction) {
                if let Some(find_match) =
                    next_match_after(find_match, current_match, &mut found_current_match)
                {
                    return Some(find_match);
                }
            }
        }
        return None;
    }

    let mut cursor = block_list
        .block_heights()
        .cursor::<BlockHeight, BlockHeightSummary>();
    cursor.descend_to_last_item(block_list.block_heights());

    while let Some(item) = cursor.item() {
        match item {
            BlockHeightItem::Block(height) if height.into_lines() > Lines::zero() => {
                let block_index = cursor.start().block_count;
                if let Some(block) = block_list.block_at(block_index.into()) {
                    for find_match in
                        run_find_on_block(dfas, block, block_index.into(), block_sort_direction)
                    {
                        if let Some(find_match) =
                            next_match_after(find_match, current_match, &mut found_current_match)
                        {
                            return Some(find_match);
                        }
                    }
                }
            }
            BlockHeightItem::RichContent(RichContentItem {
                view_id,
                last_laid_out_height,
                ..
            }) if last_laid_out_height.into_lines() > Lines::zero() => {
                if let Some(findable_view) = rich_content_views.get(view_id) {
                    let mut rich_content_matches = findable_view.run_find(options, ctx);
                    if matches!(block_sort_direction, BlockSortDirection::MostRecentLast) {
                        rich_content_matches.reverse();
                    }

                    for find_match in rich_content_matches.into_iter().map(|match_id| {
                        BlockListMatch::RichContent {
                            match_id,
                            view_id: *view_id,
                            index: cursor.start().total_count.into(),
                        }
                    }) {
                        if let Some(find_match) =
                            next_match_after(find_match, current_match, &mut found_current_match)
                        {
                            return Some(find_match);
                        }
                    }
                }
            }
            _ => (),
        }
        cursor.prev();
    }
    None
}

struct LimitedMatchCount {
    count: usize,
    is_exact: bool,
}

struct BlockListMatchCount {
    count: usize,
    is_exact: bool,
    block_match_counts: HashMap<BlockIndex, usize>,
}

fn next_match_after(
    find_match: BlockListMatch,
    current_match: Option<&BlockListMatch>,
    found_current_match: &mut bool,
) -> Option<BlockListMatch> {
    if !*found_current_match {
        if current_match.is_some_and(|current_match| find_match.same_span(current_match)) {
            *found_current_match = true;
        }
        return None;
    }

    (!find_match.is_filtered()).then_some(find_match)
}

fn count_matches(
    options: &FindOptions,
    dfas: &RegexDFAs,
    block_list: &BlockList,
    rich_content_views: &HashMap<EntityId, Box<dyn FindableRichContentHandle>>,
    block_sort_direction: BlockSortDirection,
    ctx: &mut AppContext,
) -> BlockListMatchCount {
    let mut match_count = 0;
    let mut is_exact = true;
    let mut block_match_counts = HashMap::new();

    if let Some(blocks_to_include_in_results) = options.blocks_to_include_in_results.as_ref() {
        for block_index in blocks_to_include_in_results {
            let agent_view_state = block_list.agent_view_state();
            let Some(block) = block_list
                .block_at(*block_index)
                .filter(|block| !block.is_empty(agent_view_state))
            else {
                continue;
            };
            if block.height(agent_view_state) == Lines::zero() {
                continue;
            }

            let remaining_count = MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT - match_count;
            let block_count = count_block_matches(
                dfas,
                block,
                *block_index,
                block_sort_direction,
                remaining_count,
            );
            match_count += block_count.count;
            block_match_counts.insert(*block_index, block_count.count);
            if !block_count.is_exact {
                is_exact = false;
                break;
            }
        }
        return BlockListMatchCount {
            count: match_count,
            is_exact,
            block_match_counts,
        };
    }

    let mut cursor = block_list
        .block_heights()
        .cursor::<BlockHeight, BlockHeightSummary>();
    cursor.descend_to_last_item(block_list.block_heights());

    while let Some(item) = cursor.item() {
        match item {
            BlockHeightItem::Block(height) if height.into_lines() > Lines::zero() => {
                let block_index = cursor.start().block_count;
                if let Some(block) = block_list.block_at(block_index.into()) {
                    let remaining_count = MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT - match_count;
                    let block_count = count_block_matches(
                        dfas,
                        block,
                        block_index.into(),
                        block_sort_direction,
                        remaining_count,
                    );
                    match_count += block_count.count;
                    block_match_counts.insert(block_index.into(), block_count.count);
                    if !block_count.is_exact {
                        is_exact = false;
                        break;
                    }
                }
            }
            BlockHeightItem::RichContent(RichContentItem {
                view_id,
                last_laid_out_height,
                ..
            }) if last_laid_out_height.into_lines() > Lines::zero() => {
                if let Some(findable_view) = rich_content_views.get(view_id) {
                    let remaining_count = MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT - match_count;
                    let rich_content_match_count = findable_view.run_find(options, ctx).len();
                    if rich_content_match_count > remaining_count {
                        match_count = MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT;
                        is_exact = false;
                        break;
                    }
                    match_count += rich_content_match_count;
                }
            }
            _ => (),
        }
        cursor.prev();
    }

    BlockListMatchCount {
        count: match_count,
        is_exact,
        block_match_counts,
    }
}

fn count_block_matches(
    dfas: &RegexDFAs,
    block: &Block,
    block_index: BlockIndex,
    block_sort_direction: BlockSortDirection,
    limit: usize,
) -> LimitedMatchCount {
    if limit == 0 {
        return LimitedMatchCount {
            count: 0,
            is_exact: false,
        };
    }

    let mut count = 0;
    for _find_match in run_find_on_block(dfas, block, block_index, block_sort_direction)
        .filter(|find_match| !find_match.is_filtered())
    {
        if count == limit {
            return LimitedMatchCount {
                count,
                is_exact: false,
            };
        }
        count += 1;
    }

    LimitedMatchCount {
        count,
        is_exact: true,
    }
}

/// Runs a find operation on the given block and returns resulting matches lazily.
fn run_find_on_block<'a>(
    dfas: &'a RegexDFAs,
    block: &'a Block,
    block_index: BlockIndex,
    block_sort_direction: BlockSortDirection,
) -> Box<dyn Iterator<Item = BlockListMatch> + 'a> {
    let grid_order = match block_sort_direction {
        BlockSortDirection::MostRecentFirst => &[GridType::PromptAndCommand, GridType::Output],
        BlockSortDirection::MostRecentLast => &[GridType::Output, GridType::PromptAndCommand],
    };

    let mut grid_matches = Vec::with_capacity(grid_order.len());
    for grid_type in grid_order {
        let matches_for_grid: Box<dyn Iterator<Item = BlockListMatch>> = match grid_type {
            GridType::PromptAndCommand => Box::new(block.prompt_and_command_grid().find(dfas).map(
                move |range| {
                    BlockListMatch::CommandBlock(BlockGridMatch {
                        grid_type: GridType::PromptAndCommand,
                        range,
                        block_index,
                        is_filtered: false,
                    })
                },
            )),
            GridType::Output => Box::new(block.output_grid().find(dfas).map(move |range| {
                let mut find_match = BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: GridType::Output,
                    range,
                    block_index,
                    is_filtered: false,
                });
                update_matches_for_filtered_block(
                    iter::once(&mut find_match),
                    block,
                    block_sort_direction,
                );
                find_match
            })),
            _ => continue,
        };
        grid_matches.push(matches_for_grid);
    }

    Box::new(grid_matches.into_iter().flatten())
}

/// Represents a single find match in a grid-based block in the blocklist..
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockGridMatch {
    /// The type of grid in which the match was found.
    pub grid_type: GridType,

    /// The character index range of the match.
    pub range: RangeInclusive<Point>,

    /// The index of the containing block.
    pub block_index: BlockIndex,

    /// `true` if the match should be filtered out from displayed results (e.g. if the containing
    /// row has been filtered out via block filtering).
    pub is_filtered: bool,
}

/// Represents a single find match in the blocklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockListMatch {
    CommandBlock(BlockGridMatch),
    RichContent {
        match_id: RichContentMatchId,
        view_id: EntityId,
        index: TotalIndex,
    },
}

impl BlockListMatch {
    pub fn is_filtered(&self) -> bool {
        match self {
            BlockListMatch::CommandBlock(BlockGridMatch { is_filtered, .. }) => *is_filtered,
            _ => false,
        }
    }

    fn grid_type(&self) -> Option<GridType> {
        match self {
            BlockListMatch::CommandBlock(BlockGridMatch { grid_type, .. }) => Some(*grid_type),
            _ => None,
        }
    }

    pub fn matches_block(&self, block_index: BlockIndex) -> bool {
        match self {
            BlockListMatch::CommandBlock(BlockGridMatch {
                block_index: match_block_index,
                ..
            }) => block_index == *match_block_index,
            _ => false,
        }
    }

    fn matches_blockgrid(&self, block_index: BlockIndex, grid_type: GridType) -> bool {
        match self {
            BlockListMatch::CommandBlock(BlockGridMatch {
                block_index: match_block_index,
                grid_type: match_grid_type,
                ..
            }) => block_index == *match_block_index && grid_type == *match_grid_type,
            _ => false,
        }
    }

    /// Returns `true` if `self` and `other` refer to the same matched span, ignoring transient
    /// state like `is_filtered`.
    fn same_span(&self, other: &BlockListMatch) -> bool {
        match (self, other) {
            (
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: g1,
                    range: r1,
                    block_index: b1,
                    ..
                }),
                BlockListMatch::CommandBlock(BlockGridMatch {
                    grid_type: g2,
                    range: r2,
                    block_index: b2,
                    ..
                }),
            ) => g1 == g2 && r1 == r2 && b1 == b2,
            (
                BlockListMatch::RichContent {
                    match_id: id1,
                    view_id: v1,
                    index: i1,
                },
                BlockListMatch::RichContent {
                    match_id: id2,
                    view_id: v2,
                    index: i2,
                },
            ) => id1 == id2 && v1 == v2 && i1 == i2,
            _ => false,
        }
    }
}

/// Represents the result of a find "run" on the blocklist.
#[derive(Debug)]
pub struct BlockListFindRun {
    /// Compiled [`RegexDFAs`] for the find query.
    ///
    /// If the query in `options` is Some(), this is guaranteed to be `Some()`.
    dfas: Option<RegexDFAs>,

    /// Matches found in the blocklist.
    ///
    /// Matches in this vector are ordered by block index in order of decreasing recency. Within
    /// the slice for a given block, matches ordering depends on the `block_sort_direction`. For
    /// `BlockSortDirection::MostRecentLast`, matches are ordered from "bottom" to "top". For
    /// `BlockSortDirection::MostRecentFirst`, matches are ordered from "top" to "bottom". This
    /// ensures that iterating over matches occurs in the order that is expected in the UI.
    ///
    /// The match at index 0 is the first match to be focused after a fresh find run - for
    /// pin-to-bottom and waterfall input modes, this is the match closest to the bottom of the
    /// most recent block. For pin-to-top, this is the match closest to the top of the most recent
    /// block.
    matches: Vec<BlockListMatch>,

    /// Total number of visible matches for the current query, capped at
    /// [`MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT`] when there are too many matches to count
    /// synchronously without blocking the UI thread.
    match_count: usize,

    /// Whether `match_count` is exact. If false, the real count is at least `match_count`.
    is_match_count_exact: bool,

    /// Visible match counts by command block, used to update the total when the active block changes.
    /// These counts are complete only while `is_match_count_exact` is true.
    block_match_counts: HashMap<BlockIndex, usize>,

    /// The index of the currently focused match in the `matches` vector.
    ///
    /// Note that this may differ from the focused match index displayed in the find bar UI, since
    /// the number of visible matches may be affected by block filtering.  The focused match index
    /// in the UI thus is relative to visible matches, while this field is relative to all matches.
    raw_focused_match_index: Option<usize>,

    /// The `FindOptions` used to configure the find run.
    options: FindOptions,

    /// The block sort direction, reflected in the ordering of intra-block matches in `matches`.
    block_sort_direction: BlockSortDirection,
}

impl BlockListFindRun {
    /// Returns the UI focused match index, relative to the list of visible matches.
    pub fn focused_match_index(&self) -> Option<usize> {
        self.raw_focused_match_index.map(|focused_index| {
            focused_index
                - self.matches[..focused_index]
                    .iter()
                    .fold(0, |count, find_match| {
                        if find_match.is_filtered() {
                            count + 1
                        } else {
                            count
                        }
                    })
        })
    }

    /// Returns an iterator over visible `BlockListMatch`es.
    pub fn matches(&self) -> impl Iterator<Item = &BlockListMatch> {
        self.matches.iter().filter(|m| !m.is_filtered())
    }

    pub fn match_count(&self) -> usize {
        self.match_count
    }

    pub fn is_match_count_exact(&self) -> bool {
        self.is_match_count_exact
    }

    /// Returns an iterator over matches for the given block index and grid type, in "ascending"
    /// order.
    ///
    /// This logic relies on the ordering of `self.matches` explained in the field declaration.
    pub fn matches_for_block_grid(
        &self,
        _block_index: BlockIndex,
        _grid_type: GridType,
    ) -> Box<dyn Iterator<Item = &RangeInclusive<Point>> + '_> {
        Box::new(iter::empty())
    }

    pub fn focused_match(&self) -> Option<&BlockListMatch> {
        self.raw_focused_match_index
            .and_then(|i| self.matches.get(i))
    }

    pub fn options(&self) -> &FindOptions {
        &self.options
    }

    /// Focuses the next match in `matches` based on the given `direction` and `block_sort_direction`.
    pub(super) fn focus_next_match(
        &mut self,
        direction: FindDirection,
        block_sort_direction: BlockSortDirection,
        block_list: &BlockList,
        rich_content_views: &HashMap<EntityId, Box<dyn FindableRichContentHandle>>,
        ctx: &mut AppContext,
    ) {
        let should_advance = matches!(
            (direction, block_sort_direction),
            (FindDirection::Up, BlockSortDirection::MostRecentLast)
                | (FindDirection::Down, BlockSortDirection::MostRecentFirst)
        );

        let new_focused_index = match (self.raw_focused_match_index, self.matches.is_empty()) {
            (_, true) => None,
            (Some(current_index), false) => Some(if should_advance {
                if current_index + 1 == self.matches.len() {
                    let current_match = self.matches.get(current_index).cloned();
                    if let Some(next_match) = self.find_next_visible_match(
                        current_match.as_ref(),
                        block_list,
                        rich_content_views,
                        ctx,
                    ) {
                        self.matches.push(next_match);
                    }
                }

                if current_index + 1 < self.matches.len() {
                    current_index + 1
                } else {
                    0
                }
            } else if current_index > 0 {
                current_index - 1
            } else {
                current_index
            }),
            (None, false) => Some(0),
        };
        self.raw_focused_match_index = new_focused_index;
    }

    fn find_next_visible_match(
        &self,
        current_match: Option<&BlockListMatch>,
        block_list: &BlockList,
        rich_content_views: &HashMap<EntityId, Box<dyn FindableRichContentHandle>>,
        ctx: &mut AppContext,
    ) -> Option<BlockListMatch> {
        let dfas = self.dfas.as_ref()?;
        find_next_match(
            &self.options,
            dfas,
            block_list,
            rich_content_views,
            self.block_sort_direction,
            current_match,
            ctx,
        )
    }

    /// Reruns the find operation on the block at the given index, updates the matches with the new results.
    ///
    /// Note that matches for a given block are expected to appear in a contiguous slice (per the
    /// expected ordering of `self.matches`).
    pub(super) fn rerun_on_block(
        mut self,
        block: &Block,
        block_index: BlockIndex,
        block_sort_direction: BlockSortDirection,
    ) -> Self {
        let Some(dfas) = self.dfas.as_ref() else {
            return self;
        };

        // Remember the currently focused match so we can relocate it after splicing.
        let old_focused_match = self
            .raw_focused_match_index
            .and_then(|i| self.matches.get(i).cloned());

        let old_block_matches_start_index = self
            .matches
            .iter()
            .position(|find_match| find_match.matches_block(block_index));
        if self.is_match_count_exact {
            let old_block_match_count = self
                .block_match_counts
                .get(&block_index)
                .copied()
                .unwrap_or_default();
            let base_match_count = self.match_count.saturating_sub(old_block_match_count);
            let remaining_count =
                MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT.saturating_sub(base_match_count);
            let new_block_match_count = count_block_matches(
                dfas,
                block,
                block_index,
                block_sort_direction,
                remaining_count,
            );

            self.match_count = base_match_count + new_block_match_count.count;
            self.block_match_counts
                .insert(block_index, new_block_match_count.count);
            if !new_block_match_count.is_exact {
                self.match_count = MAX_SYNC_BLOCK_LIST_FIND_MATCH_COUNT;
                self.is_match_count_exact = false;
                self.block_match_counts.clear();
            }
        }

        let new_matches = run_find_on_block(dfas, block, block_index, block_sort_direction)
            .find(|find_match| !find_match.is_filtered())
            .into_iter()
            .collect_vec();
        let new_stored_block_match_count = new_matches.len();
        if let Some(start_index) = old_block_matches_start_index {
            let end_index = old_block_matches_start_index
                .and_then(|i| {
                    self.matches[(i + 1)..]
                        .iter()
                        .position(|find_match| !find_match.matches_block(block_index))
                        .map(|j| i + j + 1)
                })
                .unwrap_or(self.matches.len());

            let old_stored_block_match_count = end_index - start_index;

            // Splice in the new matches where the old block matches used to exist.
            self.matches.splice(start_index..end_index, new_matches);

            // Adjust the focused match index so it still points to the same match.
            if let Some(focused_index) = self.raw_focused_match_index {
                if focused_index >= start_index && focused_index < end_index {
                    // The focused match was inside the rerun block. Try to find the same
                    // match (by span identity) in the new results.
                    self.raw_focused_match_index = old_focused_match
                        .as_ref()
                        .and_then(|old_match| {
                            self.matches[start_index..(start_index + new_stored_block_match_count)]
                                .iter()
                                .position(|m| m.same_span(old_match))
                                .map(|p| start_index + p)
                        })
                        .or_else(|| {
                            // The old match no longer exists; clamp to a valid index.
                            if self.matches.is_empty() {
                                None
                            } else {
                                Some(focused_index.min(self.matches.len() - 1))
                            }
                        });
                } else if focused_index >= end_index {
                    // The focused match was after the rerun block. Shift by the change in
                    // match count so it continues to point at the same match.
                    let new_index =
                        focused_index + new_stored_block_match_count - old_stored_block_match_count;
                    self.raw_focused_match_index =
                        Some(new_index.min(self.matches.len().saturating_sub(1)));
                }
                // If focused_index < start_index the match is before the rerun block and
                // needs no adjustment.
            }
        } else {
            let mut new_matches = new_matches;
            new_matches.append(&mut self.matches);
            self.matches = new_matches;

            // All previous indices shifted forward by the number of newly prepended matches.
            if let Some(focused_index) = self.raw_focused_match_index {
                self.raw_focused_match_index = Some(focused_index + new_stored_block_match_count);
            }
        }

        if self.matches.is_empty() {
            self.raw_focused_match_index = None;
        } else if let Some(focused_match_index) = self.raw_focused_match_index {
            // Final bounds check.
            if focused_match_index >= self.matches.len() {
                self.raw_focused_match_index = Some(self.matches.len() - 1);
            }
        }

        self
    }

    pub(super) fn update_matches_for_filtered_block(
        &mut self,
        block: &Block,
        block_index: BlockIndex,
        block_sort_direction: BlockSortDirection,
    ) {
        update_matches_for_filtered_block(
            self.matches.iter_mut().filter(|find_match| {
                find_match.matches_block(block_index)
                    && matches!(find_match.grid_type(), Some(GridType::Output))
            }),
            block,
            block_sort_direction,
        );
    }

    pub(super) fn cleared(mut self) -> Self {
        let new_dfas =
            self.options.query.as_ref().and_then(|query| {
                match RegexDFAs::new_with_config(
                    query.as_str(),
                    FindConfig {
                        is_regex_enabled: self.options.is_regex_enabled,
                        is_case_sensitive: self.options.is_case_sensitive,
                    },
                ) {
                    Ok(dfas) => Some(dfas),
                    Err(e) => {
                        log::warn!(
                            "Failed to construct new RegexDFAs for cleared BlockListFindRun: {e:?}"
                        );
                        None
                    }
                }
            });
        self.dfas = new_dfas;
        self.matches = vec![];
        self.match_count = 0;
        self.is_match_count_exact = true;
        self.block_match_counts.clear();
        self.raw_focused_match_index = None;
        self
    }
}

fn update_matches_for_filtered_block<'a>(
    mut matches: impl Iterator<Item = &'a mut BlockListMatch>,
    block: &Block,
    block_sort_direction: BlockSortDirection,
) {
    let Some(displayed_rows) = block.displayed_output_row_ranges() else {
        matches.for_each(|find_match| {
            if let BlockListMatch::CommandBlock(BlockGridMatch {
                ref mut is_filtered,
                ..
            }) = find_match
            {
                *is_filtered = false;
            }
        });
        return;
    };

    match block_sort_direction {
        BlockSortDirection::MostRecentLast => {
            let mut current_find_match = matches.next();

            let mut displayed_row_ranges = displayed_rows.rev();
            let mut current_row_range = displayed_row_ranges.next();

            loop {
                match (current_find_match.take(), current_row_range.take()) {
                    (Some(find_match), Some(row_range)) => {
                        match find_match {
                            BlockListMatch::CommandBlock(BlockGridMatch {
                                range,
                                is_filtered,
                                ..
                            }) if range.end().row > *row_range.end() => {
                                *is_filtered = true;
                                current_find_match = matches.next();
                                current_row_range = Some(row_range);
                            }
                            BlockListMatch::CommandBlock(BlockGridMatch {
                                range,
                                is_filtered,
                                ..
                            }) if range.start().row >= *row_range.start() => {
                                *is_filtered = false;
                                current_find_match = matches.next();
                                current_row_range = Some(row_range);
                            }
                            _ => {
                                current_find_match = Some(find_match);
                                current_row_range = displayed_row_ranges.next();
                            }
                        };
                    }
                    (Some(find_match), None) => {
                        if let BlockListMatch::CommandBlock(BlockGridMatch {
                            is_filtered, ..
                        }) = find_match
                        {
                            *is_filtered = true;
                        }
                        current_find_match = matches.next();
                    }
                    (None, _) => break,
                }
            }
        }
        BlockSortDirection::MostRecentFirst => {
            let mut current_find_match = matches.next();

            let mut displayed_row_ranges = displayed_rows;
            let mut current_row_range = displayed_row_ranges.next();

            loop {
                match (current_find_match.take(), current_row_range.take()) {
                    (Some(find_match), Some(row_range)) => {
                        match find_match {
                            BlockListMatch::CommandBlock(BlockGridMatch {
                                range,
                                is_filtered,
                                ..
                            }) if range.start().row < *row_range.start() => {
                                *is_filtered = true;
                                current_find_match = matches.next();
                            }
                            BlockListMatch::CommandBlock(BlockGridMatch {
                                range,
                                is_filtered,
                                ..
                            }) if range.end().row <= *row_range.end() => {
                                *is_filtered = false;
                                current_find_match = matches.next();
                            }
                            _ => {
                                current_find_match = Some(find_match);
                                current_row_range = displayed_row_ranges.next();
                            }
                        };
                    }
                    (Some(find_match), None) => {
                        if let BlockListMatch::CommandBlock(BlockGridMatch {
                            is_filtered, ..
                        }) = find_match
                        {
                            *is_filtered = true;
                        }
                        current_find_match = matches.next();
                    }
                    (None, _) => break,
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "block_list_tests.rs"]
mod tests;
