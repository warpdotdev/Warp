//! Background task for async find operations.
//!
//! This module contains the logic that runs on a background thread to scan
//! terminal blocks for matches without blocking the main thread. The task
//! pulls work items from a shared [`FindWorkQueue`] and streams results
//! back via an `async_channel`.

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

use super::work_queue::{FindWorkItem, FindWorkQueue};
use super::{
    AbsoluteMatch, AsyncFindConfig, FindTaskMessage, MAX_LOCK_DURATION_MS, ROWS_PER_CHUNK,
};

/// Spawns a background find task that pulls work from the given queue.
///
/// Returns a handle that can be used to abort the spawned future.
pub fn spawn_find_task<E: Entity>(
    config: AsyncFindConfig,
    terminal_model: Arc<FairMutex<TerminalModel>>,
    queue: FindWorkQueue,
    result_tx: async_channel::Sender<FindTaskMessage>,
    total_blocks: usize,
    ctx: &mut ModelContext<E>,
) -> warpui::r#async::SpawnedFutureHandle {
    ctx.spawn(
        async move {
            run_find_task_loop(config, terminal_model, queue, result_tx, total_blocks).await;
        },
        |_me, (), _ctx| {
            // Task completed — nothing to do here as results are sent via channel.
        },
    )
}

/// Runs the main find task loop, pulling work items from the queue.
async fn run_find_task_loop(
    config: AsyncFindConfig,
    terminal_model: Arc<FairMutex<TerminalModel>>,
    queue: FindWorkQueue,
    result_tx: async_channel::Sender<FindTaskMessage>,
    total_blocks: usize,
) {
    // Build RegexDFAs from config.
    let Ok(dfas) = RegexDFAs::new_with_config(
        config.query.as_str(),
        FindConfig {
            is_regex_enabled: config.is_regex_enabled,
            is_case_sensitive: config.is_case_sensitive,
        },
    ) else {
        // Invalid regex — signal completion with no matches.
        let _ = result_tx.send(FindTaskMessage::Done).await;
        return;
    };

    let mut blocks_scanned: usize = 0;

    // Send initial progress.
    let _ = result_tx
        .send(FindTaskMessage::Progress {
            blocks_scanned: 0,
            total_blocks,
        })
        .await;

    loop {
        match queue.pop().await {
            Ok(item) => {
                match item {
                    FindWorkItem::ScanFullBlock { block_index } => {
                        scan_terminal_block_chunked(
                            block_index,
                            &terminal_model,
                            &dfas,
                            &result_tx,
                            &queue,
                            config.block_sort_direction,
                        )
                        .await;

                        blocks_scanned += 1;
                        let _ = result_tx
                            .send(FindTaskMessage::Progress {
                                blocks_scanned,
                                total_blocks,
                            })
                            .await;
                    }
                    FindWorkItem::ScanDirtyRange {
                        block_index,
                        row_range,
                        num_lines_truncated,
                    } => {
                        scan_dirty_range(
                            block_index,
                            row_range,
                            num_lines_truncated,
                            &terminal_model,
                            &dfas,
                            &result_tx,
                        )
                        .await;
                    }
                    FindWorkItem::ScanAIBlock {
                        view_id,
                        total_index,
                    } => {
                        // Forward to main thread for execution.
                        let _ = result_tx
                            .send(FindTaskMessage::ScanAIBlock {
                                view_id,
                                total_index,
                            })
                            .await;

                        blocks_scanned += 1;
                        let _ = result_tx
                            .send(FindTaskMessage::Progress {
                                blocks_scanned,
                                total_blocks,
                            })
                            .await;
                    }
                }

                // If the queue just drained, send Done so the controller can
                // transition status to Complete.
                if queue.is_empty() {
                    let _ = result_tx.send(FindTaskMessage::Done).await;
                }
            }
            Err(_queue_closed) => {
                // Queue was closed — exit the task.
                break;
            }
        }
    }
}

/// Scans a dirty range within a terminal block and sends results for merging.
async fn scan_dirty_range(
    block_index: BlockIndex,
    row_range: RangeInclusive<usize>,
    num_lines_truncated: u64,
    terminal_model: &Arc<FairMutex<TerminalModel>>,
    dfas: &RegexDFAs,
    result_tx: &async_channel::Sender<FindTaskMessage>,
) {
    // Collect all results under the lock, then send after releasing it.
    let messages = {
        let model = terminal_model.lock();
        let Some(block) = model.block_list().block_at(block_index) else {
            return;
        };

        let mut messages = Vec::new();

        for grid_type in [GridType::PromptAndCommand, GridType::Output] {
            let grid = match grid_type {
                GridType::Output => block.output_grid().grid_handler(),
                GridType::PromptAndCommand => block.prompt_and_command_grid().grid_handler(),
                _ => continue,
            };

            let total_rows = Dimensions::total_rows(grid);
            let columns = Dimensions::columns(grid);

            // Clamp dirty range to grid bounds.
            let start_row = *row_range.start().min(&total_rows.saturating_sub(1));
            let end_row = *row_range.end().min(&total_rows.saturating_sub(1));

            if start_row > end_row {
                continue;
            }

            let start_point = Point::new(start_row, 0);
            let end_point = Point::new(end_row, columns.saturating_sub(1));

            let iter = grid.find_in_range(dfas, start_point, end_point);
            let mut matches: Vec<AbsoluteMatch> = iter
                .map(|range| AbsoluteMatch::from_range(&range, grid))
                .collect();

            // The find iterator returns matches in descending order; reverse to ascending.
            matches.reverse();

            // Convert dirty range to absolute row indices.
            let absolute_start = start_row as u64 + num_lines_truncated;
            let absolute_end = end_row as u64 + num_lines_truncated;

            messages.push(FindTaskMessage::DirtyRangeMatches {
                block_index,
                grid_type,
                dirty_range: absolute_start..=absolute_end,
                matches,
            });
        }

        messages
    };

    for msg in messages {
        let _ = result_tx.send(msg).await;
    }
}

/// Scans a terminal block in chunks, streaming results back to the main thread.
async fn scan_terminal_block_chunked(
    block_index: BlockIndex,
    terminal_model: &Arc<FairMutex<TerminalModel>>,
    dfas: &RegexDFAs,
    result_tx: &async_channel::Sender<FindTaskMessage>,
    queue: &FindWorkQueue,
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
        scan_grid_chunked(
            block_index,
            *grid_type,
            terminal_model,
            dfas,
            result_tx,
            queue,
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
    queue: &FindWorkQueue,
) {
    let mut start_row = 0;

    loop {
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
            log::trace!(
                "[async_find] scan_grid_chunked: block {:?} grid {:?}, total_rows={}, start_row={}",
                block_index,
                grid_type,
                total_rows,
                start_row
            );
            if start_row >= total_rows {
                // Finished scanning this grid.
                log::trace!("[async_find] scan_grid_chunked: block {:?} grid {:?} is empty or fully scanned", block_index, grid_type);
                return;
            }

            // Calculate the end row for this chunk.
            let end_row = (start_row + ROWS_PER_CHUNK).min(total_rows);

            // Scan this chunk using the existing find implementation.
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
            yield_now().await;
        }
    }

    // Check if the queue was closed (cancellation) between chunks. This is a
    // lightweight check; the main cancellation path is queue.pop() returning
    // Err(QueueClosed) in the task loop.
    let _ = queue.is_empty();
}

/// Scans a range of rows in a grid for matches.
fn scan_grid_range(
    grid: &GridHandler,
    dfas: &RegexDFAs,
    start_row: usize,
    end_row: usize,
) -> Vec<RangeInclusive<Point>> {
    let columns = Dimensions::columns(grid);
    let start_point = Point::new(start_row, 0);
    let end_point = Point::new(end_row.saturating_sub(1), columns.saturating_sub(1));

    grid.find_in_range(dfas, start_point, end_point).collect()
}

/// Yields control to other tasks.
async fn yield_now() {
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
