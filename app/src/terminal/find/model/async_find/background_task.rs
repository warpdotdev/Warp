//! Background task for async find operations.
//!
//! This module contains the logic that runs on a background thread to scan
//! terminal blocks for matches without blocking the main thread.

use std::ops::RangeInclusive;
use std::sync::Arc;

use instant::Instant;
use parking_lot::FairMutex;
use warp_terminal::model::grid::Dimensions;
use warpui::ModelContext;

use warpui::Entity;

use crate::terminal::block_list_element::GridType;
use crate::terminal::model::find::{FindConfig, RegexDFAs};
use crate::terminal::model::grid::grid_handler::GridHandler;
use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::BlockIndex;
use crate::terminal::model::TerminalModel;

use super::{
    collect_block_info, AbsoluteMatch, AsyncFindConfig, BlockInfo, FindTaskMessage,
    MAX_LOCK_DURATION_MS, ROWS_PER_CHUNK,
};

/// Spawns a background find task.
///
/// This is generic over the entity type so that it can be called from
/// any model that owns an `AsyncFindController`.
///
/// Returns a handle that can be used to abort the spawned future.
pub fn spawn_find_task<E: Entity>(
    config: AsyncFindConfig,
    terminal_model: Arc<FairMutex<TerminalModel>>,
    result_tx: async_channel::Sender<FindTaskMessage>,
    cancel_rx: async_channel::Receiver<()>,
    ctx: &mut ModelContext<E>,
) -> warpui::r#async::SpawnedFutureHandle {
    ctx.spawn(
        async move {
            run_find_task(config, terminal_model, result_tx, cancel_rx).await;
        },
        |_me, (), _ctx| {
            // Task completed - nothing to do here as results are sent via channel.
        },
    )
}

/// Runs the main find task loop.
async fn run_find_task(
    config: AsyncFindConfig,
    terminal_model: Arc<FairMutex<TerminalModel>>,
    result_tx: async_channel::Sender<FindTaskMessage>,
    cancel_rx: async_channel::Receiver<()>,
) {
    // Build RegexDFAs from config.
    let Ok(dfas) = RegexDFAs::new_with_config(
        config.query.as_str(),
        FindConfig {
            is_regex_enabled: config.is_regex_enabled,
            is_case_sensitive: config.is_case_sensitive,
        },
    ) else {
        // Invalid regex - signal completion with no matches.
        let _ = result_tx.send(FindTaskMessage::Done).await;
        return;
    };

    // Get block list snapshot (indices, types, heights).
    let block_info = {
        let model = terminal_model.lock();
        collect_block_info(model.block_list(), &config)
    };

    let total_blocks = block_info.len();
    log::trace!("[async_find] background task: scanning {} blocks", total_blocks);

    // Send initial progress.
    let _ = result_tx
        .send(FindTaskMessage::Progress {
            blocks_scanned: 0,
            total_blocks,
        })
        .await;

    // Process blocks newest-first.
    for (i, info) in block_info.iter().enumerate() {
        // Check for cancellation.
        if cancel_rx.is_closed() {
            let _ = result_tx.send(FindTaskMessage::Cancelled).await;
            return;
        }

        match info {
            BlockInfo::Terminal { block_index } => {
                scan_terminal_block_chunked(
                    *block_index,
                    &terminal_model,
                    &dfas,
                    &result_tx,
                    &cancel_rx,
                    config.block_sort_direction,
                )
                .await;

                // Notify the main thread that this block has been fully scanned.
                let _ = result_tx
                    .send(FindTaskMessage::BlockScanned {
                        block_index: *block_index,
                    })
                    .await;
            }
            BlockInfo::RichContent {
                view_id,
                total_index,
            } => {
                // Request main thread to scan AI block.
                let _ = result_tx
                    .send(FindTaskMessage::ScanAIBlock {
                        view_id: *view_id,
                        total_index: *total_index,
                    })
                    .await;
            }
        }

        // Send progress update after each block.
        let _ = result_tx
            .send(FindTaskMessage::Progress {
                blocks_scanned: i + 1,
                total_blocks,
            })
            .await;
    }

    let _ = result_tx.send(FindTaskMessage::Done).await;
}

/// Scans a terminal block in chunks, streaming results back to the main thread.
async fn scan_terminal_block_chunked(
    block_index: BlockIndex,
    terminal_model: &Arc<FairMutex<TerminalModel>>,
    dfas: &RegexDFAs,
    result_tx: &async_channel::Sender<FindTaskMessage>,
    cancel_rx: &async_channel::Receiver<()>,
    block_sort_direction: crate::terminal::model::terminal_model::BlockSortDirection,
) {
    // Determine grid order based on sort direction.
    let grid_order = match block_sort_direction {
        crate::terminal::model::terminal_model::BlockSortDirection::MostRecentFirst => {
            &[GridType::PromptAndCommand, GridType::Output]
        }
        crate::terminal::model::terminal_model::BlockSortDirection::MostRecentLast => {
            &[GridType::Output, GridType::PromptAndCommand]
        }
    };

    for grid_type in grid_order.iter() {
        if cancel_rx.is_closed() {
            return;
        }

        scan_grid_chunked(
            block_index,
            *grid_type,
            terminal_model,
            dfas,
            result_tx,
            cancel_rx,
        )
        .await;
    }
}

/// Scans a single grid in chunks, releasing the lock between chunks.
async fn scan_grid_chunked(
    block_index: BlockIndex,
    grid_type: GridType,
    terminal_model: &Arc<FairMutex<TerminalModel>>,
    dfas: &RegexDFAs,
    result_tx: &async_channel::Sender<FindTaskMessage>,
    cancel_rx: &async_channel::Receiver<()>,
) {
    let mut start_row = 0;

    loop {
        // Check for cancellation before each chunk.
        if cancel_rx.is_closed() {
            return;
        }

        let chunk_result = {
            let lock_start = Instant::now();
            let model = terminal_model.lock();

            let Some(block) = model.block_list().block_at(block_index) else {
                // Block no longer exists.
                return;
            };

            let grid = match grid_type {
                GridType::Output => block.output_grid(),
                GridType::PromptAndCommand => block.prompt_and_command_grid(),
                _ => return,
            };

            let grid_handler = grid.grid_handler();
            let total_rows = grid_handler.total_rows();
            log::trace!("[async_find] scan_grid_chunked: block {:?} grid {:?}, total_rows={}, start_row={}", block_index, grid_type, total_rows, start_row);
            if start_row >= total_rows {
                // Finished scanning this grid.
                log::trace!("[async_find] scan_grid_chunked: block {:?} grid {:?} is empty or fully scanned", block_index, grid_type);
                return;
            }

            // Calculate the end row for this chunk.
            let end_row = (start_row + ROWS_PER_CHUNK).min(total_rows);

            // Scan this chunk using the existing find implementation.
            // We scan the range and collect matches, converting to AbsoluteMatch for storage.
            let point_matches = scan_grid_range(grid_handler, dfas, start_row, end_row);
            let matches: Vec<AbsoluteMatch> = point_matches
                .iter()
                .map(|range| AbsoluteMatch::from_range(range, grid_handler))
                .collect();

            let elapsed = lock_start.elapsed();
            (matches, end_row, total_rows, elapsed)
        };

        let (mut matches, end_row, total_rows, elapsed) = chunk_result;

        // The `find_in_range` function returns matches in descending order (it searches from
        // end to start going left). We always reverse each chunk to ascending order so that
        // when chunks are concatenated via `.extend()`, the final Vec remains in ascending
        // order. The renderer expects matches in ascending order for `active_or_next_match`
        // to work correctly.
        matches.reverse();

        // Stream results if we found any matches in this chunk.
        if !matches.is_empty() {
            log::trace!(
                "[async_find] background task: found {} matches in block {:?} grid {:?}",
                matches.len(),
                block_index,
                grid_type
            );
            let _ = result_tx
                .send(FindTaskMessage::BlockGridMatches {
                    block_index,
                    grid_type,
                    matches,
                })
                .await;
        }

        if end_row >= total_rows {
            // Finished scanning this grid.
            break;
        }

        start_row = end_row;

        // Yield to let other tasks run if we held the lock for a while.
        if elapsed.as_millis() > MAX_LOCK_DURATION_MS as u128 / 2 {
            // Use a small delay to release the lock and let other tasks run.
            yield_now().await;
        }
    }
}

/// Scans a range of rows in a grid for matches.
fn scan_grid_range(
    grid: &GridHandler,
    dfas: &RegexDFAs,
    start_row: usize,
    end_row: usize,
) -> Vec<RangeInclusive<Point>> {
    let mut matches: Vec<RangeInclusive<Point>> = Vec::new();

    // Use the grid's find functionality to scan the specified range.
    // We need to create start and end points for the range.
    let columns = Dimensions::columns(grid);
    let start_point = Point::new(start_row, 0);
    let end_point = Point::new(end_row.saturating_sub(1), columns.saturating_sub(1));

    // Use the existing regex iterator to find matches in this range.
    let iter = grid.find_in_range(dfas, start_point, end_point);

    for m in iter {
        matches.push(m);
    }

    matches
}

/// Yields control to other tasks.
async fn yield_now() {
    // Create a future that yields once.
    struct YieldNow {
        yielded: bool,
    }

    impl std::future::Future for YieldNow {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            if self.yielded {
                std::task::Poll::Ready(())
            } else {
                self.yielded = true;
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
    }

    YieldNow { yielded: false }.await
}
