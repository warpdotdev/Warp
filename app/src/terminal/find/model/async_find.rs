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
use warpui::{Entity, EntityId, ModelContext};

use crate::terminal::block_list_element::GridType;
use crate::terminal::model::blocks::{
    BlockHeight, BlockHeightItem, BlockHeightSummary, BlockList, TotalIndex,
};
use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::{BlockIndex, BlockSortDirection};
use crate::terminal::model::TerminalModel;

use crate::view_components::find::FindDirection;

pub use super::block_list::BlockGridMatch;
use super::rich_content::{FindableRichContentHandle, RichContentMatchId};
use super::FindOptions;

use background_task::spawn_find_task;

/// Maximum time (in milliseconds) to hold the terminal model lock during a find chunk.
pub const MAX_LOCK_DURATION_MS: u64 = 100;

/// Number of rows to scan per chunk within a terminal block.
pub const ROWS_PER_CHUNK: usize = 1000;

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
        matches: Vec<RangeInclusive<Point>>,
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

/// Per-block find results, keyed by block index.
#[derive(Debug, Default)]
struct BlockFindResults {
    /// Matches for terminal blocks, keyed by (block_index, grid_type).
    terminal_matches: HashMap<(BlockIndex, GridType), Vec<RangeInclusive<Point>>>,
    /// Matches for AI blocks, keyed by view_id.
    ai_matches: HashMap<EntityId, Vec<RichContentMatchId>>,
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
    block_results: BlockFindResults,

    /// Current status of the find operation.
    status: AsyncFindStatus,

    /// Receiver for messages from background task.
    result_rx: Option<async_channel::Receiver<FindTaskMessage>>,

    /// Sender to cancel the current background task.
    cancel_tx: Option<async_channel::Sender<()>>,

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

    /// Returns the focused match as a BlockGridMatch if it's a terminal match.
    pub fn focused_terminal_match(&self) -> Option<BlockGridMatch> {
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
                        return Some(BlockGridMatch {
                            block_index: *block_index,
                            grid_type: *grid_type,
                            range: match_range.clone(),
                            is_filtered: false,
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
        spawn_find_task(
            config,
            self.terminal_model.clone(),
            result_tx,
            cancel_rx,
            ctx,
        );
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
                            let match_ids = view.run_find(&options, ctx);
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
        if let Some(cancel_tx) = self.cancel_tx.take() {
            // Send cancellation signal (ignore errors - task may have already finished).
            let _ = cancel_tx.try_send(());
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
    pub fn invalidate_block<E: Entity>(
        &mut self,
        block_index: BlockIndex,
        ctx: &mut ModelContext<E>,
    ) {
        self.block_results.remove_block(block_index);

        // If we have an active config, rescan just this block.
        if let Some(config) = &self.current_config {
            self.rescan_single_block(block_index, config.clone(), ctx);
        }

        // Note: The owning entity is responsible for emitting events to notify listeners.
    }

    /// Returns matches for a specific terminal block grid.
    pub fn matches_for_block_grid(
        &self,
        block_index: BlockIndex,
        grid_type: GridType,
    ) -> Option<&Vec<RangeInclusive<Point>>> {
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
        spawn_find_task(
            config,
            self.terminal_model.clone(),
            result_tx,
            cancel_rx,
            ctx,
        );
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
        spawn_find_task(
            single_block_config,
            self.terminal_model.clone(),
            result_tx,
            cancel_rx,
            ctx,
        );
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

    #[test]
    fn test_block_find_results_total_count() {
        let mut results = BlockFindResults::default();
        assert_eq!(results.total_match_count(), 0);

        // Add some terminal matches.
        let point = Point::new(0, 0);
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::Output))
            .or_default()
            .push(point..=point);
        assert_eq!(results.total_match_count(), 1);

        // Add more terminal matches.
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::Output))
            .or_default()
            .push(point..=point);
        results
            .terminal_matches
            .entry((BlockIndex(1), GridType::PromptAndCommand))
            .or_default()
            .push(point..=point);
        assert_eq!(results.total_match_count(), 3);
    }

    #[test]
    fn test_block_find_results_remove_block() {
        let mut results = BlockFindResults::default();
        let point = Point::new(0, 0);

        // Add matches for block 0 and block 1.
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::Output))
            .or_default()
            .push(point..=point);
        results
            .terminal_matches
            .entry((BlockIndex(0), GridType::PromptAndCommand))
            .or_default()
            .push(point..=point);
        results
            .terminal_matches
            .entry((BlockIndex(1), GridType::Output))
            .or_default()
            .push(point..=point);
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
}
