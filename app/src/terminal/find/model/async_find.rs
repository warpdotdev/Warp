//! Async find implementation for terminal content.
//!
//! This module provides asynchronous find functionality that runs on a background thread,
//! streaming results back to the main thread to avoid blocking the UI.

mod background_task;
mod work_queue;

use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

use parking_lot::FairMutex;
use warpui::r#async::SpawnedFutureHandle;
use warpui::{Entity, EntityId, ModelContext};

use crate::terminal::block_list_element::GridType;
use crate::terminal::model::blocks::{
    BlockHeight, BlockHeightItem, BlockHeightSummary, BlockList, TotalIndex,
};
use crate::terminal::model::grid::grid_handler::{AbsolutePoint, GridHandler};
use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::{BlockIndex, BlockSortDirection};
use crate::terminal::model::TerminalModel;

use crate::view_components::find::FindDirection;

use super::rich_content::{FindableRichContentHandle, RichContentMatchId};
use super::FindOptions;

use background_task::spawn_find_task;
use work_queue::FindWorkQueue;

/// Status of an async find operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncFindStatus {
    /// No find operation in progress.
    Idle,
    /// Find operation is running.
    Scanning {
        /// Number of blocks that have been fully scanned.
        blocks_scanned: usize,
        /// Total number of blocks to scan.
        total_blocks: usize,
    },
    /// Find operation completed.
    Complete,
}

impl Default for AsyncFindStatus {
    fn default() -> Self {
        Self::Idle
    }
}

impl std::fmt::Display for AsyncFindStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Scanning {
                blocks_scanned,
                total_blocks,
            } => write!(f, "Scanning ({}/{})", blocks_scanned, total_blocks),
            Self::Complete => write!(f, "Complete"),
        }
    }
}

/// Message streamed from background find task to main thread.
#[derive(Debug)]
pub enum FindTaskMessage {
    /// Matches found in a terminal block's grid.
    BlockGridMatches {
        block_index: BlockIndex,
        grid_type: GridType,
        matches: Vec<AbsoluteMatch>,
    },
    /// Matches found in a dirty range within a terminal block's grid.
    ///
    /// Unlike `BlockGridMatches` (which extends results), these matches are
    /// merged into existing results using `update_dirty_matches`.
    DirtyRangeMatches {
        block_index: BlockIndex,
        grid_type: GridType,
        /// The dirty range in absolute row coordinates, used for merging.
        dirty_range: RangeInclusive<u64>,
        matches: Vec<AbsoluteMatch>,
    },
    /// Request to scan an AI block on the main thread.
    ScanAIBlock {
        view_id: EntityId,
        total_index: TotalIndex,
    },
    /// Progress update.
    Progress {
        blocks_scanned: usize,
        total_blocks: usize,
    },
    /// The work queue has drained (current batch of work is complete).
    Done,
}

/// Configuration for an async find run.
#[derive(Debug, Clone)]
pub struct AsyncFindConfig {
    /// The search query.
    pub query: Arc<String>,
    /// Whether the search is case-sensitive.
    pub is_case_sensitive: bool,
    /// Whether the query should be interpreted as a regex.
    pub is_regex_enabled: bool,
    /// If `Some`, only search within these block indices.
    pub blocks_to_include: Option<Vec<BlockIndex>>,
    /// The sort direction of blocks in the view.
    pub block_sort_direction: BlockSortDirection,
}

impl AsyncFindConfig {
    /// Creates a new config from FindOptions and a block sort direction.
    ///
    /// Returns `None` if there is no query or if the query is empty/whitespace.
    pub fn from_options(
        options: &FindOptions,
        block_sort_direction: BlockSortDirection,
    ) -> Option<Self> {
        let query = options.query.clone()?;
        if query.trim().is_empty() {
            return None;
        }
        Some(Self {
            query,
            is_case_sensitive: options.is_case_sensitive,
            is_regex_enabled: options.is_regex_enabled,
            blocks_to_include: options.blocks_to_include_in_results.clone(),
            block_sort_direction,
        })
    }
}

/// A match stored with absolute row indices to handle scrollback truncation.
///
/// Using absolute indices (offset from original row 0) allows us to:
/// 1. Avoid updating all match indices when rows are truncated
/// 2. Efficiently filter out truncated matches at query time
/// 3. Support incremental dirty-range scanning without full rescans
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteMatch {
    /// Start point with absolute row index.
    pub start: AbsolutePoint,
    /// End point with absolute row index.
    pub end: AbsolutePoint,
}

impl AbsoluteMatch {
    /// Creates an AbsoluteMatch from a relative Point range.
    pub fn from_range(range: &RangeInclusive<Point>, grid: &GridHandler) -> Self {
        Self {
            start: AbsolutePoint::from_point(*range.start(), grid),
            end: AbsolutePoint::from_point(*range.end(), grid),
        }
    }

    /// Converts back to a relative Point range.
    ///
    /// Returns `None` if either point has been truncated from scrollback.
    pub fn to_range(&self, grid: &GridHandler) -> Option<RangeInclusive<Point>> {
        let start = self.start.to_point(grid)?;
        let end = self.end.to_point(grid)?;
        Some(start..=end)
    }

    /// Returns true if this match has been truncated from scrollback.
    pub fn is_truncated(&self, num_lines_truncated: u64) -> bool {
        // A match is truncated if its start point is truncated.
        self.start.is_truncated(num_lines_truncated)
    }

    /// Returns the absolute start row.
    pub fn start_row(&self) -> u64 {
        self.start.row
    }

    /// Returns the absolute end row.
    pub fn end_row(&self) -> u64 {
        self.end.row
    }
}

impl PartialOrd for AbsoluteMatch {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AbsoluteMatch {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by end point (for ascending order iteration during rendering).
        self.end.cmp(&other.end)
    }
}

/// A focused match from the async find controller.
///
/// Similar to `BlockGridMatch` but uses `AbsoluteMatch` for the range, allowing
/// the caller to convert to relative coordinates when needed.
#[derive(Debug, Clone)]
pub struct AsyncBlockGridMatch {
    /// The type of grid in which the match was found.
    pub grid_type: GridType,
    /// The match range in absolute coordinates.
    pub range: AbsoluteMatch,
    /// The index of the containing block.
    pub block_index: BlockIndex,
}

/// Per-block find results, keyed by block index.
#[derive(Debug, Default)]
pub(crate) struct BlockFindResults {
    /// Matches for terminal blocks, keyed by (block_index, grid_type).
    /// Matches are stored in ascending order by end point.
    pub(crate) terminal_matches: HashMap<(BlockIndex, GridType), Vec<AbsoluteMatch>>,
    /// Matches for AI blocks, keyed by view_id.
    pub(crate) ai_matches: HashMap<EntityId, Vec<RichContentMatchId>>,
}

impl BlockFindResults {
    /// Returns the total number of matches across all blocks.
    fn total_match_count(&self) -> usize {
        let terminal_count: usize = self.terminal_matches.values().map(|v| v.len()).sum();
        let ai_count: usize = self.ai_matches.values().map(|v| v.len()).sum();
        terminal_count + ai_count
    }

    /// Clears all results.
    fn clear(&mut self) {
        self.terminal_matches.clear();
        self.ai_matches.clear();
    }

    /// Removes all results for a specific block index.
    fn remove_block(&mut self, block_index: BlockIndex) {
        self.terminal_matches
            .retain(|(idx, _), _| *idx != block_index);
    }

    /// Updates matches for a dirty range within a specific block grid.
    ///
    /// This removes all existing matches that overlap with the dirty range and
    /// inserts the new matches in their place, maintaining ascending order.
    ///
    /// Adapted from `FilterState::update_dirty_matches` in filtering.rs.
    fn update_dirty_matches(
        &mut self,
        block_index: BlockIndex,
        grid_type: GridType,
        dirty_range: RangeInclusive<u64>,
        new_matches: Vec<AbsoluteMatch>,
    ) {
        let matches = self
            .terminal_matches
            .entry((block_index, grid_type))
            .or_default();

        // If there are no current matches, just insert the new ones.
        if matches.is_empty() {
            *matches = new_matches;
            return;
        }

        // If the dirty range is before all existing matches, insert at the start.
        if matches
            .first()
            .is_some_and(|first_match| *dirty_range.end() < first_match.start_row())
        {
            matches.splice(0..0, new_matches);
            return;
        }

        // If the dirty range is after all existing matches, append at the end.
        if matches
            .last()
            .is_some_and(|last_match| last_match.end_row() < *dirty_range.start())
        {
            matches.extend(new_matches);
            return;
        }

        // Find the range of matches that overlap with the dirty range.
        // A match overlaps if: match.start <= dirty.end AND match.end >= dirty.start
        let replace_start = matches
            .iter()
            .position(|m| m.end_row() >= *dirty_range.start())
            .unwrap_or(matches.len());

        let replace_end = matches
            .iter()
            .rposition(|m| m.start_row() <= *dirty_range.end())
            .map(|pos| pos + 1)
            .unwrap_or(replace_start);

        let replace_range = if replace_start <= replace_end {
            replace_start..replace_end
        } else {
            // Dirty range lies between two adjacent matches; insert without replacing.
            replace_start..replace_start
        };

        matches.splice(replace_range, new_matches);

        // Assert that matches are still in ascending order.
        debug_assert!(
            matches
                .windows(2)
                .all(|w| w[0].end_row() <= w[1].start_row()),
            "Matches should be in ascending order after update_dirty_matches"
        );
    }
}

/// Events emitted by the AsyncFindController.
#[derive(Debug, Clone)]
pub enum AsyncFindEvent {
    /// New matches have arrived.
    MatchesUpdated,
    /// Status changed (scanning/complete).
    StatusChanged,
}

/// Information about a block to be searched.
#[derive(Debug, Clone)]
pub enum BlockInfo {
    /// A terminal command block.
    Terminal { block_index: BlockIndex },
    /// A rich content block (e.g., AI block).
    RichContent {
        view_id: EntityId,
        total_index: TotalIndex,
    },
}

/// Controller for async find operations.
///
/// This model manages background find tasks, processes results, and maintains
/// find state for the terminal view.
pub struct AsyncFindController {
    /// Reference to the terminal model.
    terminal_model: Arc<FairMutex<TerminalModel>>,

    /// Current find configuration, if any.
    current_config: Option<AsyncFindConfig>,

    /// Per-block results for the current find run.
    #[cfg(not(test))]
    block_results: BlockFindResults,
    #[cfg(test)]
    pub(crate) block_results: BlockFindResults,

    /// Current status of the find operation.
    #[cfg(not(test))]
    status: AsyncFindStatus,
    #[cfg(test)]
    pub(crate) status: AsyncFindStatus,

    /// Receiver for messages from background task.
    #[cfg(not(test))]
    result_rx: Option<async_channel::Receiver<FindTaskMessage>>,
    #[cfg(test)]
    pub(crate) result_rx: Option<async_channel::Receiver<FindTaskMessage>>,

    /// Handle to abort the background task's future.
    task_handle: Option<SpawnedFutureHandle>,

    /// Shared work queue for the background task.
    work_queue: Option<FindWorkQueue>,

    /// Rich content views for AI block searching.
    rich_content_views: HashMap<EntityId, Box<dyn FindableRichContentHandle>>,

    /// The block sort direction for the current/last find run.
    block_sort_direction: BlockSortDirection,

    /// The currently focused match index (0-based), if any.
    focused_match_index: Option<usize>,

    /// The FindOptions for the current find run, stored for `active_find_options()` access.
    current_find_options: Option<FindOptions>,

    /// Cached result of `focused_terminal_match()`, updated when focus or matches change.
    /// Avoids re-sorting HashMap keys and iterating on every call.
    cached_focused_match: Option<AsyncBlockGridMatch>,
}

impl AsyncFindController {
    /// Creates a new AsyncFindController.
    pub fn new(terminal_model: Arc<FairMutex<TerminalModel>>) -> Self {
        Self {
            terminal_model,
            current_config: None,
            block_results: BlockFindResults::default(),
            status: AsyncFindStatus::Idle,
            result_rx: None,
            task_handle: None,
            work_queue: None,
            rich_content_views: HashMap::new(),
            block_sort_direction: BlockSortDirection::MostRecentLast,
            focused_match_index: None,
            current_find_options: None,
            cached_focused_match: None,
        }
    }

    /// Returns the current find status.
    pub fn status(&self) -> &AsyncFindStatus {
        &self.status
    }

    /// Returns the current config, if any.
    pub fn current_config(&self) -> Option<&AsyncFindConfig> {
        self.current_config.as_ref()
    }

    /// Returns the FindOptions for the current find run, if any.
    pub fn find_options(&self) -> Option<&FindOptions> {
        self.current_find_options.as_ref()
    }

    /// Returns the total number of matches found so far.
    pub fn match_count(&self) -> usize {
        self.block_results.total_match_count()
    }

    /// Returns true if a find operation is currently in progress.
    pub fn is_scanning(&self) -> bool {
        matches!(self.status, AsyncFindStatus::Scanning { .. })
    }

    /// Returns true if there is an active find configuration.
    ///
    /// This indicates that find is active and new blocks should be scanned.
    pub fn has_active_find(&self) -> bool {
        self.current_config.is_some()
    }

    /// Returns true if there are pending results from a background task.
    ///
    /// This can be true even when `is_scanning()` is false, e.g., when a single
    /// block is being rescanned after the initial scan completed.
    pub fn has_pending_results(&self) -> bool {
        self.result_rx.is_some()
    }

    /// Returns the currently focused match index (0-based), if any.
    pub fn focused_match_index(&self) -> Option<usize> {
        self.focused_match_index
    }

    /// Focuses the next or previous match based on the given direction.
    pub fn focus_next_match(&mut self, direction: FindDirection) {
        let total = self.match_count();
        if total == 0 {
            self.focused_match_index = None;
            return;
        }

        let new_index = match (self.focused_match_index, direction) {
            (None, FindDirection::Down) => 0,
            (None, FindDirection::Up) => total.saturating_sub(1),
            (Some(current), FindDirection::Down) => {
                if current + 1 >= total {
                    0 // Wrap around.
                } else {
                    current + 1
                }
            }
            (Some(current), FindDirection::Up) => {
                if current == 0 {
                    total.saturating_sub(1) // Wrap around.
                } else {
                    current - 1
                }
            }
        };

        self.focused_match_index = Some(new_index);
        self.update_cached_focused_match();
    }

    /// Returns the focused match as an AsyncBlockGridMatch if it's a terminal match.
    ///
    /// Returns a cached value that is updated when focus or matches change,
    /// avoiding the cost of sorting and iterating on every call.
    pub fn focused_terminal_match(&self) -> Option<AsyncBlockGridMatch> {
        self.cached_focused_match.clone()
    }

    /// Recomputes the cached focused match from the current matches and focus index.
    fn update_cached_focused_match(&mut self) {
        self.cached_focused_match = self.compute_focused_terminal_match();
    }

    /// Computes the focused terminal match by iterating through all terminal matches
    /// in deterministic order to find the one at the focused index.
    fn compute_focused_terminal_match(&self) -> Option<AsyncBlockGridMatch> {
        let focused_idx = self.focused_match_index?;

        let mut current_idx = 0;

        // Sort keys for deterministic iteration order.
        let mut keys: Vec<_> = self.block_results.terminal_matches.keys().collect();
        keys.sort_by(|a, b| {
            // Sort by block index, then grid type (PromptAndCommand before Output).
            match a.0.cmp(&b.0) {
                std::cmp::Ordering::Equal => {
                    let grid_order = |g: &GridType| match g {
                        GridType::PromptAndCommand => 0,
                        GridType::Output => 1,
                        _ => 2,
                    };
                    grid_order(&a.1).cmp(&grid_order(&b.1))
                }
                other => other,
            }
        });

        for (block_index, grid_type) in keys {
            if let Some(matches) = self
                .block_results
                .terminal_matches
                .get(&(*block_index, *grid_type))
            {
                for match_range in matches {
                    if current_idx == focused_idx {
                        return Some(AsyncBlockGridMatch {
                            block_index: *block_index,
                            grid_type: *grid_type,
                            range: match_range.clone(),
                        });
                    }
                    current_idx += 1;
                }
            }
        }

        None
    }

    /// Registers a rich content view for AI block searching.
    pub(crate) fn register_rich_content_view(
        &mut self,
        view_id: EntityId,
        handle: Box<dyn FindableRichContentHandle>,
    ) {
        self.rich_content_views.insert(view_id, handle);
    }

    /// Unregisters a rich content view.
    pub(crate) fn unregister_rich_content_view(&mut self, view_id: EntityId) {
        self.rich_content_views.remove(&view_id);
    }

    /// Starts a new find operation with the given options.
    ///
    /// If a find operation is already in progress, it will be cancelled first.
    /// The `ctx` parameter is generic so this can be called from any owning entity.
    pub fn start_find<E: Entity>(
        &mut self,
        options: &FindOptions,
        block_sort_direction: BlockSortDirection,
        ctx: &mut ModelContext<E>,
    ) {
        // Cancel any existing find operation.
        self.cancel_current_find();

        // Check for query refinement optimization.
        if let (Some(current_config), Some(new_query)) =
            (&self.current_config, options.query.as_ref())
        {
            if !options.is_regex_enabled
                && !current_config.is_regex_enabled
                && options.is_case_sensitive == current_config.is_case_sensitive
                && is_query_refinement(&current_config.query, new_query)
            {
                // New query is a refinement of the old query — filter existing results.
                self.filter_results_for_refinement(options, block_sort_direction, ctx);
                return;
            }
        }

        // Create new config.
        let Some(config) = AsyncFindConfig::from_options(options, block_sort_direction) else {
            // No query — clear results and return.
            self.clear_results(ctx);
            return;
        };

        self.current_config = Some(config.clone());
        self.block_sort_direction = block_sort_direction;
        self.block_results.clear();
        self.focused_match_index = None;
        self.cached_focused_match = None;
        self.current_find_options = Some(options.clone());
        self.status = AsyncFindStatus::Scanning {
            blocks_scanned: 0,
            total_blocks: 0,
        };

        // Build the work queue from the current block list.
        let queue = FindWorkQueue::new();
        let block_info = {
            let model = self.terminal_model.lock();
            collect_block_info(model.block_list(), &config)
        };
        let total_blocks = block_info.len();
        queue.enqueue_full_scan(&block_info);
        self.work_queue = Some(queue.clone());

        // Create result channel.
        let (result_tx, result_rx) = async_channel::unbounded();
        self.result_rx = Some(result_rx);

        // Spawn background task.
        self.task_handle = Some(spawn_find_task(
            config,
            self.terminal_model.clone(),
            queue,
            result_tx,
            total_blocks,
            ctx,
        ));
    }

    /// Processes pending messages from the background find task.
    ///
    /// This should be called periodically (e.g., on a timer or in response to a wakeup).
    /// The `ctx` parameter is generic so this can be called from any owning entity.
    pub fn process_messages<E: Entity>(&mut self, ctx: &mut ModelContext<E>) {
        let Some(rx) = &self.result_rx else {
            log::trace!("[async_find] process_messages: no result_rx channel");
            return;
        };

        // Collect all messages first to avoid borrow issues.
        let messages: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        log::trace!(
            "[async_find] process_messages: received {} messages",
            messages.len()
        );

        let mut had_matches = false;

        for msg in messages {
            match msg {
                FindTaskMessage::BlockGridMatches {
                    block_index,
                    grid_type,
                    matches,
                } => {
                    if !matches.is_empty() {
                        self.block_results
                            .terminal_matches
                            .entry((block_index, grid_type))
                            .or_default()
                            .extend(matches);
                        had_matches = true;

                        // Auto-select the first match when results first arrive.
                        if self.focused_match_index.is_none() {
                            self.focused_match_index = Some(0);
                        }
                    }
                }
                FindTaskMessage::DirtyRangeMatches {
                    block_index,
                    grid_type,
                    dirty_range,
                    matches,
                } => {
                    self.block_results.update_dirty_matches(
                        block_index,
                        grid_type,
                        dirty_range,
                        matches,
                    );
                    had_matches = true;
                }
                FindTaskMessage::ScanAIBlock {
                    view_id,
                    total_index: _,
                } => {
                    // Scan AI block on main thread.
                    if let Some(view) = self.rich_content_views.get(&view_id) {
                        if let Some(config) = &self.current_config {
                            let options = FindOptions {
                                query: Some(config.query.clone()),
                                is_case_sensitive: config.is_case_sensitive,
                                is_regex_enabled: config.is_regex_enabled,
                                blocks_to_include_in_results: None,
                            };
                            let start = instant::Instant::now();
                            let match_ids = view.run_find(&options, ctx);
                            let elapsed = start.elapsed();
                            log::trace!(
                                "[async_find] AI block scan took {}ms for view_id={:?}",
                                elapsed.as_millis(),
                                view_id
                            );
                            if !match_ids.is_empty() {
                                self.block_results.ai_matches.insert(view_id, match_ids);
                                had_matches = true;
                            }
                        }
                    }
                }
                FindTaskMessage::Progress {
                    blocks_scanned,
                    total_blocks,
                } => {
                    self.status = AsyncFindStatus::Scanning {
                        blocks_scanned,
                        total_blocks,
                    };
                }
                FindTaskMessage::Done => {
                    self.status = AsyncFindStatus::Complete;
                }
            }
        }

        if had_matches {
            self.update_cached_focused_match();
        }
    }

    /// Cancels the current find operation, if any.
    pub fn cancel_current_find(&mut self) {
        // Close the work queue, which causes the background task's pop() to
        // return Err(QueueClosed) and exit.
        if let Some(queue) = self.work_queue.take() {
            queue.close();
        }

        // Abort the background future so it stops being polled.
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }

        self.result_rx = None;
    }

    /// Clears all find results and resets state.
    /// The `ctx` parameter is generic so this can be called from any owning entity.
    pub fn clear_results<E: Entity>(&mut self, ctx: &mut ModelContext<E>) {
        self.cancel_current_find();
        self.current_config = None;
        self.block_results.clear();
        self.focused_match_index = None;
        self.cached_focused_match = None;
        self.status = AsyncFindStatus::Idle;
        self.current_find_options = None;

        // Clear matches in AI blocks.
        for view in self.rich_content_views.values() {
            view.clear_matches(ctx);
        }

        // Note: The owning entity is responsible for emitting events to notify listeners.
    }

    /// Invalidates results for a specific block and rescans it.
    ///
    /// This enqueues work into the shared queue so the background task handles
    /// it asynchronously. No scanning happens on the main thread.
    ///
    /// # Arguments
    /// * `block_index` - The index of the block that changed.
    /// * `dirty_info` - If provided, a `(row_range, grid_type, num_lines_truncated)`
    ///   tuple describing the dirty region. If `None`, a full block rescan is enqueued.
    pub fn invalidate_block(
        &mut self,
        block_index: BlockIndex,
        dirty_info: Option<(RangeInclusive<usize>, GridType, u64)>,
    ) {
        if self.current_config.is_none() {
            return;
        }

        let Some(queue) = self.work_queue.clone() else {
            return;
        };

        // For a full block rescan (no dirty range), clear existing results now
        // so stale matches are not shown while the rescan is pending.
        if dirty_info.is_none() {
            self.block_results.remove_block(block_index);
            self.update_cached_focused_match();
        }

        log::trace!(
            "[async_find] invalidate_block: enqueuing work for block {:?}, dirty={:?}",
            block_index,
            dirty_info.as_ref().map(|(r, _, _)| r),
        );

        queue.invalidate_block(block_index, dirty_info);
    }

    /// Returns matches for a specific terminal block grid as AbsoluteMatch references.
    ///
    /// Callers should convert to relative `Point` ranges using `AbsoluteMatch::to_range()`.
    pub fn matches_for_block_grid(
        &self,
        block_index: BlockIndex,
        grid_type: GridType,
    ) -> Option<&Vec<AbsoluteMatch>> {
        self.block_results
            .terminal_matches
            .get(&(block_index, grid_type))
    }

    /// Returns matches for a specific AI block.
    pub fn matches_for_ai_block(&self, view_id: EntityId) -> Option<&Vec<RichContentMatchId>> {
        self.block_results.ai_matches.get(&view_id)
    }

    /// Filters existing results for a query refinement.
    fn filter_results_for_refinement<E: Entity>(
        &mut self,
        options: &FindOptions,
        block_sort_direction: BlockSortDirection,
        ctx: &mut ModelContext<E>,
    ) {
        // For now, we do a full rescan when the query is refined.
        // A future optimization could filter existing matches without rescanning,
        // and update the query for pending queue items.

        // Create new config.
        let Some(config) = AsyncFindConfig::from_options(options, block_sort_direction) else {
            self.clear_results(ctx);
            return;
        };

        // Cancel existing operation and clear results, but keep the task alive
        // by re-using start_find which handles everything.
        self.cancel_current_find();

        self.current_config = Some(config.clone());
        self.block_sort_direction = block_sort_direction;
        self.block_results.clear();
        self.focused_match_index = None;
        self.cached_focused_match = None;
        self.current_find_options = Some(options.clone());
        self.status = AsyncFindStatus::Scanning {
            blocks_scanned: 0,
            total_blocks: 0,
        };

        // Build the work queue from the current block list.
        let queue = FindWorkQueue::new();
        let block_info = {
            let model = self.terminal_model.lock();
            collect_block_info(model.block_list(), &config)
        };
        let total_blocks = block_info.len();
        queue.enqueue_full_scan(&block_info);
        self.work_queue = Some(queue.clone());

        // Create result channel.
        let (result_tx, result_rx) = async_channel::unbounded();
        self.result_rx = Some(result_rx);

        // Spawn background task.
        self.task_handle = Some(spawn_find_task(
            config,
            self.terminal_model.clone(),
            queue,
            result_tx,
            total_blocks,
            ctx,
        ));
    }
}

/// Returns true if `new_query` is a refinement of `old_query`.
///
/// A query is considered a refinement if it starts with the old query,
/// meaning any match of the new query must also be a match of the old query.
/// An empty old_query is not considered a valid refinement base.
fn is_query_refinement(old_query: &str, new_query: &str) -> bool {
    !old_query.is_empty() && new_query.starts_with(old_query) && new_query.len() > old_query.len()
}

/// Collects information about blocks to search.
pub fn collect_block_info(block_list: &BlockList, config: &AsyncFindConfig) -> Vec<BlockInfo> {
    let mut block_info = Vec::new();

    // If specific blocks are requested, only collect those.
    if let Some(blocks_to_include) = &config.blocks_to_include {
        for &block_index in blocks_to_include {
            if block_list.block_at(block_index).is_some() {
                block_info.push(BlockInfo::Terminal { block_index });
            }
        }
        // Sort by recency (newest first).
        block_info.sort_by(|a, b| {
            let idx_a = match a {
                BlockInfo::Terminal { block_index } => block_index.0,
                BlockInfo::RichContent { .. } => 0,
            };
            let idx_b = match b {
                BlockInfo::Terminal { block_index } => block_index.0,
                BlockInfo::RichContent { .. } => 0,
            };
            idx_b.cmp(&idx_a)
        });
        return block_info;
    }

    // Otherwise, iterate through all blocks via the height cursor.
    let mut cursor = block_list
        .block_heights()
        .cursor::<BlockHeight, BlockHeightSummary>();
    cursor.descend_to_last_item(block_list.block_heights());

    while let Some(item) = cursor.item() {
        match item {
            BlockHeightItem::Block(height)
                if height.into_lines() > warpui::units::Lines::zero() =>
            {
                let block_index = cursor.start().block_count;
                block_info.push(BlockInfo::Terminal {
                    block_index: block_index.into(),
                });
            }
            BlockHeightItem::RichContent(rich_content_item)
                if rich_content_item.last_laid_out_height.into_lines()
                    > warpui::units::Lines::zero() =>
            {
                block_info.push(BlockInfo::RichContent {
                    view_id: rich_content_item.view_id,
                    total_index: cursor.start().total_count.into(),
                });
            }
            _ => {}
        }
        cursor.prev();
    }

    block_info
}

#[cfg(test)]
#[path = "async_find_tests.rs"]
mod tests;
