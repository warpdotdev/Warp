//! Async find implementation for terminal content.
//!
//! This module provides asynchronous find functionality that runs on a background thread,
//! streaming results back to the main thread to avoid blocking the UI.

mod background_task;

#[cfg(test)]
mod async_find_tests;

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

/// Maximum time (in milliseconds) to hold the terminal model lock during a find chunk.
pub const MAX_LOCK_DURATION_MS: u64 = 5;

/// Number of rows to scan per chunk within a terminal block.
pub const ROWS_PER_CHUNK: usize = 1000;

/// Maximum number of dirty rows to scan synchronously.
/// If the dirty range exceeds this, we fall back to a full async rescan.
const MAX_SYNC_DIRTY_ROWS: usize = 500;

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
    /// Find operation completed.
    Done,
    /// Find operation cancelled.
    Cancelled,
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
    pub fn from_options(options: &FindOptions, block_sort_direction: BlockSortDirection) -> Option<Self> {
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
            matches.windows(2).all(|w| w[0].end_row() <= w[1].start_row()),
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
    Terminal {
        block_index: BlockIndex,
    },
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

    /// Sender to cancel the current background task.
    /// Dropping the sender closes the channel, which signals cancellation.
    #[cfg(not(test))]
    cancel_tx: Option<async_channel::Sender<()>>,
    #[cfg(test)]
    pub(crate) cancel_tx: Option<async_channel::Sender<()>>,

    /// Handle to abort the background task's future.
    task_handle: Option<SpawnedFutureHandle>,

    /// Rich content views for AI block searching.
    rich_content_views: HashMap<EntityId, Box<dyn FindableRichContentHandle>>,

    /// The block sort direction for the current/last find run.
    block_sort_direction: BlockSortDirection,

    /// The currently focused match index (0-based), if any.
    focused_match_index: Option<usize>,
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
            cancel_tx: None,
            task_handle: None,
            rich_content_views: HashMap::new(),
            block_sort_direction: BlockSortDirection::MostRecentLast,
            focused_match_index: None,
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
    }

    /// Returns the focused match as an AsyncBlockGridMatch if it's a terminal match.
    ///
    /// The returned match uses absolute coordinates. Callers should use
    /// `AbsoluteMatch::to_range()` to convert to relative coordinates for rendering.
    pub fn focused_terminal_match(&self) -> Option<AsyncBlockGridMatch> {
        let focused_idx = self.focused_match_index?;

        // Iterate through terminal matches in order to find the one at focused_idx.
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
            if let Some(matches) = self.block_results.terminal_matches.get(&(*block_index, *grid_type)) {
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
                // New query is a refinement of the old query - filter existing results.
                self.filter_results_for_refinement(options, block_sort_direction, ctx);
                return;
            }
        }

        // Create new config.
        let Some(config) = AsyncFindConfig::from_options(options, block_sort_direction) else {
            // No query - clear results and return.
            self.clear_results(ctx);
            return;
        };

        self.current_config = Some(config.clone());
        self.block_sort_direction = block_sort_direction;
        self.block_results.clear();
        self.focused_match_index = None;
        self.status = AsyncFindStatus::Scanning {
            blocks_scanned: 0,
            total_blocks: 0,
        };

        // Create channels for communication with background task.
        let (result_tx, result_rx) = async_channel::unbounded();
        let (cancel_tx, cancel_rx) = async_channel::bounded(1);

        self.result_rx = Some(result_rx);
        self.cancel_tx = Some(cancel_tx);

        // Spawn background task.
        self.task_handle = Some(spawn_find_task(
            config,
            self.terminal_model.clone(),
            result_tx,
            cancel_rx,
            ctx,
        ));
    }

    /// Processes pending messages from the background find task.
    ///
    /// This should be called periodically (e.g., on a timer or in response to a wakeup).
    /// The `ctx` parameter is generic so this can be called from any owning entity.
    pub fn process_messages<E: Entity>(&mut self, ctx: &mut ModelContext<E>) {
        let Some(rx) = &self.result_rx else {
            eprintln!("[async_find] process_messages: no result_rx channel");
            return;
        };

        // Collect all messages first to avoid borrow issues.
        let messages: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        eprintln!("[async_find] process_messages: received {} messages", messages.len());

        let mut had_matches = false;
        let mut should_clear_channels = false;

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
                            log::info!(
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
                    should_clear_channels = true;
                }
                FindTaskMessage::Cancelled => {
                    should_clear_channels = true;
                }
            }
        }

        if should_clear_channels {
            self.result_rx = None;
            self.cancel_tx = None;
        }

        // Note: The owning entity is responsible for emitting events to notify listeners.
        // We don't emit events here because we're a plain struct, not an Entity.
        let _ = (had_matches, ctx);
    }

    /// Cancels the current find operation, if any.
    pub fn cancel_current_find(&mut self) {
        // Dropping the sender closes the channel, which the background task
        // detects via `cancel_rx.is_closed()`.
        self.cancel_tx.take();

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
        self.status = AsyncFindStatus::Idle;

        // Clear matches in AI blocks.
        for view in self.rich_content_views.values() {
            view.clear_matches(ctx);
        }

        // Note: The owning entity is responsible for emitting events to notify listeners.
    }

    /// Invalidates results for a specific block and rescans it.
    ///
    /// This should be called when a block's content changes.
    /// The `ctx` parameter is generic so this can be called from any owning entity.
    ///
    /// # Arguments
    /// * `block_index` - The index of the block that changed.
    /// * `dirty_row_range` - Optional range of rows (in relative coordinates) that changed.
    ///   If provided and small enough, only those rows will be rescanned and merged.
    /// * `num_lines_truncated` - The current number of truncated lines in the grid.
    /// * `ctx` - The model context.
    pub fn invalidate_block<E: Entity>(
        &mut self,
        block_index: BlockIndex,
        dirty_row_range: Option<RangeInclusive<usize>>,
        num_lines_truncated: u64,
        ctx: &mut ModelContext<E>,
    ) {
        // If we have an active config, determine whether to do incremental or full rescan.
        let Some(config) = self.current_config.clone() else {
            return;
        };

                // Check if we should do an incremental update.
        if let Some(dirty_range) = dirty_row_range {
            let dirty_row_count = dirty_range.end().saturating_sub(*dirty_range.start()) + 1;

            if dirty_row_count <= MAX_SYNC_DIRTY_ROWS {
                // Do incremental scanning for the dirty range.
                eprintln!("[async_find] invalidate_block: sync scan for block {:?}, dirty_range={:?}", block_index, dirty_range);
                self.scan_dirty_range_sync(
                    block_index,
                    dirty_range,
                    num_lines_truncated,
                    &config,
                );
                return;
            }
        }

        // Fall back to full block rescan.
        eprintln!("[async_find] invalidate_block: full rescan for block {:?}", block_index);
        self.block_results.remove_block(block_index);
        self.rescan_single_block(block_index, config, ctx);

        // Note: The owning entity is responsible for emitting events to notify listeners.
    }

    /// Scans a dirty range synchronously and merges results with existing matches.
    fn scan_dirty_range_sync(
        &mut self,
        block_index: BlockIndex,
        dirty_range: RangeInclusive<usize>,
        num_lines_truncated: u64,
        config: &AsyncFindConfig,
    ) {
        use warp_terminal::model::grid::Dimensions;
        use crate::terminal::model::find::{FindConfig, RegexDFAs};

        // Build RegexDFAs from config.
        let Ok(dfas) = RegexDFAs::new_with_config(
            config.query.as_str(),
            FindConfig {
                is_regex_enabled: config.is_regex_enabled,
                is_case_sensitive: config.is_case_sensitive,
            },
        ) else {
            return;
        };

        // Lock the terminal model to access the block.
        let model = self.terminal_model.lock();
        let Some(block) = model.block_list().block_at(block_index) else {
            return;
        };

        // Scan both grids for the dirty range.
        for grid_type in [GridType::PromptAndCommand, GridType::Output] {
            let grid = match grid_type {
                GridType::Output => block.output_grid().grid_handler(),
                GridType::PromptAndCommand => block.prompt_and_command_grid().grid_handler(),
                _ => continue,
            };

            let total_rows = Dimensions::total_rows(grid);
            let columns = Dimensions::columns(grid);

            // Clamp dirty range to grid bounds.
            let start_row = *dirty_range.start().min(&total_rows.saturating_sub(1));
            let end_row = *dirty_range.end().min(&total_rows.saturating_sub(1));

            if start_row > end_row {
                continue;
            }

            // Create start and end points for the range scan.
            let start_point = Point::new(start_row, 0);
            let end_point = Point::new(end_row, columns.saturating_sub(1));

            // Scan the range for matches.
            let iter = grid.find_in_range(&dfas, start_point, end_point);
            let mut matches: Vec<AbsoluteMatch> = iter
                .map(|range| AbsoluteMatch::from_range(&range, grid))
                .collect();

            // The find iterator returns matches in descending order; reverse to ascending.
            matches.reverse();

            // Convert dirty range to absolute row indices.
            let absolute_start = start_row as u64 + num_lines_truncated;
            let absolute_end = end_row as u64 + num_lines_truncated;
            let absolute_dirty_range = absolute_start..=absolute_end;

            // Update matches using the merge logic.
            self.block_results.update_dirty_matches(
                block_index,
                grid_type,
                absolute_dirty_range,
                matches,
            );
        }
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
        // For now, we'll do a full rescan when the query is refined.
        // A future optimization could filter existing matches without rescanning.
        // This requires extracting text from each match range and re-matching,
        // which needs terminal model access.

        // Create new config.
        let Some(config) = AsyncFindConfig::from_options(options, block_sort_direction) else {
            self.clear_results(ctx);
            return;
        };

        self.current_config = Some(config.clone());
        self.block_sort_direction = block_sort_direction;
        self.block_results.clear();
        self.status = AsyncFindStatus::Scanning {
            blocks_scanned: 0,
            total_blocks: 0,
        };

        // Create channels for communication with background task.
        let (result_tx, result_rx) = async_channel::unbounded();
        let (cancel_tx, cancel_rx) = async_channel::bounded(1);

        self.result_rx = Some(result_rx);
        self.cancel_tx = Some(cancel_tx);

        // Spawn background task.
        self.task_handle = Some(spawn_find_task(
            config,
            self.terminal_model.clone(),
            result_tx,
            cancel_rx,
            ctx,
        ));
    }

    /// Rescans a single block.
    fn rescan_single_block<E: Entity>(
        &mut self,
        block_index: BlockIndex,
        config: AsyncFindConfig,
        ctx: &mut ModelContext<E>,
    ) {
        // Create a config that only scans the specified block.
        let single_block_config = AsyncFindConfig {
            blocks_to_include: Some(vec![block_index]),
            ..config
        };

        // Create channels for communication with background task.
        let (result_tx, result_rx) = async_channel::unbounded();
        let (cancel_tx, cancel_rx) = async_channel::bounded(1);

        // Note: We don't update status here since this is a partial rescan.
        // We also keep any existing result_rx - messages will be processed together.
        // For simplicity, we cancel any existing task first.
        self.cancel_current_find();

        self.result_rx = Some(result_rx);
        self.cancel_tx = Some(cancel_tx);

        // Spawn background task.
        self.task_handle = Some(spawn_find_task(
            single_block_config,
            self.terminal_model.clone(),
            result_tx,
            cancel_rx,
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
    !old_query.is_empty()
        && new_query.starts_with(old_query)
        && new_query.len() > old_query.len()
}

/// Collects information about blocks to search.
pub fn collect_block_info(
    block_list: &BlockList,
    config: &AsyncFindConfig,
) -> Vec<BlockInfo> {
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
            BlockHeightItem::Block(height) if height.into_lines() > warpui::units::Lines::zero() => {
                let block_index = cursor.start().block_count;
                block_info.push(BlockInfo::Terminal {
                    block_index: block_index.into(),
                });
            }
            BlockHeightItem::RichContent(rich_content_item)
                if rich_content_item.last_laid_out_height.into_lines() > warpui::units::Lines::zero() =>
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
mod tests {
    use super::*;

    #[test]
    fn test_is_query_refinement() {
        assert!(is_query_refinement("hel", "hello"));
        assert!(is_query_refinement("foo", "foobar"));
        assert!(!is_query_refinement("hello", "hel"));
        assert!(!is_query_refinement("hello", "hello"));
        assert!(!is_query_refinement("bar", "foo"));
        assert!(!is_query_refinement("", "hello"));
    }

    #[test]
    fn test_async_find_config_from_options() {
        // Empty query should return None.
        let options = FindOptions::default();
        assert!(AsyncFindConfig::from_options(
            &options,
            BlockSortDirection::MostRecentLast
        )
        .is_none());

        // Query with only whitespace should return None.
        let options = FindOptions {
            query: Some(Arc::new("   ".to_string())),
            ..Default::default()
        };
        assert!(AsyncFindConfig::from_options(
            &options,
            BlockSortDirection::MostRecentLast
        )
        .is_none());

        // Valid query should return Some config.
        let options = FindOptions {
            query: Some(Arc::new("hello".to_string())),
            is_case_sensitive: true,
            is_regex_enabled: false,
            blocks_to_include_in_results: Some(vec![BlockIndex(0), BlockIndex(1)]),
        };
        let config = AsyncFindConfig::from_options(&options, BlockSortDirection::MostRecentFirst);
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.query.as_str(), "hello");
        assert!(config.is_case_sensitive);
        assert!(!config.is_regex_enabled);
        assert_eq!(config.blocks_to_include, Some(vec![BlockIndex(0), BlockIndex(1)]));
    }

    /// Helper to create an AbsoluteMatch at a given row.
    fn make_match(row: u64) -> AbsoluteMatch {
        AbsoluteMatch {
            start: AbsolutePoint { row, col: 0 },
            end: AbsolutePoint { row, col: 5 },
        }
    }

    #[test]
    fn test_block_find_results_total_count() {
        let mut results = BlockFindResults::default();
        assert_eq!(results.total_match_count(), 0);

        // Add some terminal matches.
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::Output))
            .or_default()
            .push(make_match(0));
        assert_eq!(results.total_match_count(), 1);

        // Add more terminal matches.
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::Output))
            .or_default()
            .push(make_match(1));
        results
            .terminal_matches
            .entry((BlockIndex(1), GridType::PromptAndCommand))
            .or_default()
            .push(make_match(0));
        assert_eq!(results.total_match_count(), 3);
    }

    #[test]
    fn test_block_find_results_remove_block() {
        let mut results = BlockFindResults::default();

        // Add matches for block 0 and block 1.
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::Output))
            .or_default()
            .push(make_match(0));
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::PromptAndCommand))
            .or_default()
            .push(make_match(0));
        results
            .terminal_matches
            .entry((BlockIndex(1), GridType::Output))
            .or_default()
            .push(make_match(0));
        assert_eq!(results.total_match_count(), 3);

        // Remove block 0.
        results.remove_block(BlockIndex(0));
        assert_eq!(results.total_match_count(), 1);

        // Block 1 should still have its matches.
        assert!(results
            .terminal_matches
            .contains_key(&(BlockIndex(1), GridType::Output)));
    }

    #[test]
    fn test_async_find_status_display() {
        assert_eq!(format!("{}", AsyncFindStatus::Idle), "Idle");
        assert_eq!(format!("{}", AsyncFindStatus::Complete), "Complete");
        assert_eq!(
            format!(
                "{}",
                AsyncFindStatus::Scanning {
                    blocks_scanned: 5,
                    total_blocks: 10
                }
            ),
            "Scanning (5/10)"
        );
    }

    #[test]
    fn test_absolute_match_is_truncated() {
        let match_at_row_5 = make_match(5);
        // Not truncated when num_lines_truncated <= start row.
        assert!(!match_at_row_5.is_truncated(0));
        assert!(!match_at_row_5.is_truncated(5));
        // Truncated when num_lines_truncated > start row.
        assert!(match_at_row_5.is_truncated(6));
        assert!(match_at_row_5.is_truncated(100));
    }

    #[test]
    fn test_update_dirty_matches_empty_existing() {
        let mut results = BlockFindResults::default();
        let block_index = BlockIndex(0);
        let grid_type = GridType::Output;

        // Update with new matches when there are no existing matches.
        let new_matches = vec![make_match(5), make_match(10), make_match(15)];
        results.update_dirty_matches(block_index, grid_type, 5..=15, new_matches.clone());

        let stored = results.terminal_matches.get(&(block_index, grid_type)).unwrap();
        assert_eq!(stored.len(), 3);
        assert_eq!(stored[0].start_row(), 5);
        assert_eq!(stored[1].start_row(), 10);
        assert_eq!(stored[2].start_row(), 15);
    }

    #[test]
    fn test_update_dirty_matches_prepend() {
        let mut results = BlockFindResults::default();
        let block_index = BlockIndex(0);
        let grid_type = GridType::Output;

        // Seed with matches at rows 20, 30.
        results
            .terminal_matches
            .insert((block_index, grid_type), vec![make_match(20), make_match(30)]);

        // Update with dirty range before all existing matches.
        let new_matches = vec![make_match(5), make_match(10)];
        results.update_dirty_matches(block_index, grid_type, 5..=10, new_matches);

        let stored = results.terminal_matches.get(&(block_index, grid_type)).unwrap();
        assert_eq!(stored.len(), 4);
        assert_eq!(stored[0].start_row(), 5);
        assert_eq!(stored[1].start_row(), 10);
        assert_eq!(stored[2].start_row(), 20);
        assert_eq!(stored[3].start_row(), 30);
    }

    #[test]
    fn test_update_dirty_matches_append() {
        let mut results = BlockFindResults::default();
        let block_index = BlockIndex(0);
        let grid_type = GridType::Output;

        // Seed with matches at rows 5, 10.
        results
            .terminal_matches
            .insert((block_index, grid_type), vec![make_match(5), make_match(10)]);

        // Update with dirty range after all existing matches.
        let new_matches = vec![make_match(20), make_match(30)];
        results.update_dirty_matches(block_index, grid_type, 20..=30, new_matches);

        let stored = results.terminal_matches.get(&(block_index, grid_type)).unwrap();
        assert_eq!(stored.len(), 4);
        assert_eq!(stored[0].start_row(), 5);
        assert_eq!(stored[1].start_row(), 10);
        assert_eq!(stored[2].start_row(), 20);
        assert_eq!(stored[3].start_row(), 30);
    }

    #[test]
    fn test_update_dirty_matches_replace_middle() {
        let mut results = BlockFindResults::default();
        let block_index = BlockIndex(0);
        let grid_type = GridType::Output;

        // Seed with matches at rows 5, 15, 25.
        results
            .terminal_matches
            .insert((block_index, grid_type), vec![make_match(5), make_match(15), make_match(25)]);

        // Update dirty range 10..=20, which overlaps with the match at row 15.
        // Replace it with matches at rows 12 and 18.
        let new_matches = vec![make_match(12), make_match(18)];
        results.update_dirty_matches(block_index, grid_type, 10..=20, new_matches);

        let stored = results.terminal_matches.get(&(block_index, grid_type)).unwrap();
        assert_eq!(stored.len(), 4);
        assert_eq!(stored[0].start_row(), 5);
        assert_eq!(stored[1].start_row(), 12);
        assert_eq!(stored[2].start_row(), 18);
        assert_eq!(stored[3].start_row(), 25);
    }

    #[test]
    fn test_update_dirty_matches_clear_range() {
        let mut results = BlockFindResults::default();
        let block_index = BlockIndex(0);
        let grid_type = GridType::Output;

        // Seed with matches at rows 5, 15, 25.
        results
            .terminal_matches
            .insert((block_index, grid_type), vec![make_match(5), make_match(15), make_match(25)]);

        // Update dirty range 10..=20 with no new matches (clears the match at row 15).
        results.update_dirty_matches(block_index, grid_type, 10..=20, vec![]);

        let stored = results.terminal_matches.get(&(block_index, grid_type)).unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].start_row(), 5);
        assert_eq!(stored[1].start_row(), 25);
    }
}
