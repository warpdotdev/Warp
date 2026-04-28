mod alt_screen;
mod block_list;
#[allow(dead_code)]
mod rich_content;
#[cfg(any(test, feature = "integration_tests"))]
mod testing;

pub use block_list::{BlockGridMatch, BlockListFindRun, BlockListMatch};
pub use rich_content::{FindableRichContentView, RichContentMatchId};

use crate::terminal::block_list_viewport::InputMode;
use std::{collections::HashMap, sync::Arc};

use alt_screen::{run_find_on_alt_screen, AltScreenFindRun};
use parking_lot::FairMutex;
use settings::Setting as _;
use warpui::{AppContext, Entity, EntityId, ModelContext, SingletonEntity, ViewHandle};

use crate::{
    settings::InputModeSettings,
    terminal::model::{terminal_model::BlockIndex, TerminalModel},
    view_components::find::{FindEvent, FindModel},
};

use crate::view_components::find::FindDirection;

use block_list::run_find_on_block_list;
use rich_content::FindableRichContentHandle;

/// `TerminalView`-scoped model for the find bar.
pub struct TerminalFindModel {
    terminal_model: Arc<FairMutex<TerminalModel>>,

    rich_content_views: HashMap<EntityId, Box<dyn FindableRichContentHandle>>,

    /// The most recent find "run" on the alt screen, if any.
    alt_screen_find_run: Option<AltScreenFindRun>,

    /// The most recent find "run" on the block list, if any.
    block_list_find_run: Option<BlockListFindRun>,

    /// `true` if the find bar is open.
    is_find_bar_open: bool,
}

impl FindModel for TerminalFindModel {
    fn focused_match_index(&self) -> Option<usize> {
        if self.terminal_model.lock().is_alt_screen_active() {
            self.alt_screen_find_run
                .as_ref()
                .and_then(|run| run.focused_match_index())
        } else {
            self.block_list_find_run
                .as_ref()
                .and_then(|run| run.focused_match_index())
        }
    }

    fn match_count(&self) -> usize {
        if self.terminal_model.lock().is_alt_screen_active() {
            self.alt_screen_find_run
                .as_ref()
                .map(|run| run.matches().len())
                .unwrap_or(0)
        } else {
            self.block_list_find_run
                .as_ref()
                .map(|run| run.matches().count())
                .unwrap_or(0)
        }
    }

    fn default_find_direction(&self, app: &AppContext) -> FindDirection {
        let input_mode = *InputModeSettings::as_ref(app).input_mode.value();
        match input_mode {
            InputMode::PinnedToBottom | InputMode::Waterfall => FindDirection::Up,
            InputMode::PinnedToTop => FindDirection::Down,
        }
    }
}

impl TerminalFindModel {
    pub fn new(terminal_model: Arc<FairMutex<TerminalModel>>) -> Self {
        Self {
            terminal_model,
            rich_content_views: HashMap::new(),
            alt_screen_find_run: None,
            block_list_find_run: None,
            is_find_bar_open: false,
        }
    }
    pub fn register_findable_rich_content_view<T: FindableRichContentView>(
        &mut self,
        view_handle: ViewHandle<T>,
    ) {
        self.rich_content_views
            .insert(view_handle.id(), Box::new(view_handle));
    }

    /// Returns `true` if the find bar is currently open.
    pub(crate) fn is_find_bar_open(&self) -> bool {
        self.is_find_bar_open
    }

    /// Updates find bar visibility.
    pub(crate) fn set_is_find_bar_open(&mut self, is_open: bool) {
        self.is_find_bar_open = is_open;
    }

    /// Returns the last find run for the alt screen.
    pub(crate) fn alt_screen_find_run(&self) -> Option<&AltScreenFindRun> {
        self.alt_screen_find_run.as_ref()
    }

    /// Returns the last find run for the blocklist.
    pub(crate) fn block_list_find_run(&self) -> Option<&BlockListFindRun> {
        self.block_list_find_run.as_ref()
    }

    /// Returns `FindOptions` applied to the active find run, if any.
    pub fn active_find_options(&self) -> Option<&FindOptions> {
        if self.terminal_model.lock().is_alt_screen_active() {
            self.alt_screen_find_run.as_ref().map(|run| run.options())
        } else {
            self.block_list_find_run.as_ref().map(|run| run.options())
        }
    }

    /// Runs find with the given `options` on the alt screen or blocklist (depending on which is
    /// active).
    pub fn run_find(&mut self, options: FindOptions, ctx: &mut ModelContext<Self>) {
        if self.terminal_model.lock().is_alt_screen_active() {
            self.alt_screen_find_run = Some(run_find_on_alt_screen(
                options,
                self.terminal_model.lock().alt_screen(),
            ));
        } else {
            let _ = self.block_list_find_run.take();

            let block_sort_direction = InputModeSettings::as_ref(ctx)
                .input_mode
                .value()
                .block_sort_direction();

            self.block_list_find_run = Some(run_find_on_block_list(
                options,
                self.terminal_model.lock().block_list(),
                &self.rich_content_views,
                block_sort_direction,
                ctx,
            ));
        }
        ctx.emit(FindEvent::RanFind);
    }

    /// Reruns find with the same options applied to the current run, to be called if terminal
    /// contents have changed since the last find run.
    pub fn rerun_find_on_active_grid(&mut self, ctx: &mut ModelContext<Self>) {
        if self.terminal_model.lock().is_alt_screen_active() {
            if let Some(old_find_state) = self.alt_screen_find_run.take() {
                self.alt_screen_find_run =
                    Some(old_find_state.rerun(self.terminal_model.lock().alt_screen()));
                ctx.emit(FindEvent::RanFind);
            }
        } else {
            // Find the last block index. This is the only block whose state may change.
            let last_block_index = self
                .terminal_model
                .lock()
                .block_list()
                .last_non_hidden_block_by_index()
                .unwrap_or_default();

            // Call find on the the last block's command and output grids.
            // If the block is a new finished block, the matches are inserted at a new key, the block's index in the blocklist.
            // If the block is an active, running block, its matches are overwritten in the terminal's block_matches.
            if let Some(block) = self
                .terminal_model
                .lock()
                .block_list()
                .block_at(last_block_index)
            {
                let block_sort_direction = InputModeSettings::as_ref(ctx)
                    .input_mode
                    .value()
                    .block_sort_direction();

                if let Some(old_find_run) = self.block_list_find_run.take() {
                    self.block_list_find_run = Some(old_find_run.rerun_on_block(
                        block,
                        last_block_index,
                        block_sort_direction,
                    ));
                    ctx.emit(FindEvent::RanFind);
                }
            }
        }
    }

    /// Focus the "next" match, depending on the given [`FindDirection`], in the active find run's
    /// list of matches.
    ///
    /// If there is no focused match, focuses the first match in the list.
    pub fn focus_next_find_match(
        &mut self,
        find_direction: FindDirection,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.terminal_model.lock().is_alt_screen_active() {
            if let Some(alt_screen_find_run) = self.alt_screen_find_run.as_mut() {
                alt_screen_find_run.focus_next_match(find_direction);
            }
        } else if let Some(block_list_find_run) = self.block_list_find_run.as_mut() {
            let block_sort_direction = InputModeSettings::as_ref(ctx)
                .input_mode
                .value()
                .block_sort_direction();

            block_list_find_run.focus_next_match(find_direction, block_sort_direction);
        }
        ctx.emit(FindEvent::UpdatedFocusedMatch);
    }

    /// Clears matches in the active find run, if any.
    pub fn clear_matches(&mut self, ctx: &mut ModelContext<Self>) {
        if self.terminal_model.lock().is_alt_screen_active() {
            if let Some(run) = self.alt_screen_find_run.take() {
                self.alt_screen_find_run = Some(run.cleared());
            }
        } else if let Some(run) = self.block_list_find_run.take() {
            for (_, rich_content_view) in self.rich_content_views.iter() {
                rich_content_view.clear_matches(ctx);
            }
            self.block_list_find_run = Some(run.cleared());
        }
        ctx.emit(FindEvent::RanFind);
    }

    /// Updates matches in the active find run for the block at the `block_index`, which is
    /// presumed to be filtered.
    ///
    /// Under the hood, this does not result in a new find run, but updates state on the matches
    /// for the existing find run, which is used to determine if matches should be represented in
    /// the find bar UI (e.g. match count, focused match index).
    pub fn update_matches_for_filtered_block(
        &mut self,
        block_index: BlockIndex,
        ctx: &mut ModelContext<Self>,
    ) {
        let terminal_model = self.terminal_model.lock();
        if let (Some(block_list_find_run), Some(filtered_block)) = (
            self.block_list_find_run.as_mut(),
            terminal_model.block_list().block_at(block_index),
        ) {
            let block_sort_direction = InputModeSettings::as_ref(ctx)
                .input_mode
                .value()
                .block_sort_direction();

            block_list_find_run.update_matches_for_filtered_block(
                filtered_block,
                block_index,
                block_sort_direction,
            );
            ctx.emit(FindEvent::RanFind);
        }
    }
}

impl Entity for TerminalFindModel {
    type Event = FindEvent;
}

/// Parameters for a find "run".
#[derive(Debug, Clone, Default)]
pub struct FindOptions {
    /// The find query, if any.
    pub query: Option<Arc<String>>,

    /// `true` if the find should be case-sensitive.
    pub is_case_sensitive: bool,

    /// `true` if the query should be matched as a regex pattern.
    pub is_regex_enabled: bool,

    /// If `Some()`, the find run only surfaces matches that are in blocks with the provided
    /// indices. If `None`, the find run surfaces matches across the entire blocklist.
    ///
    /// This is ignored when applied to alt screen find runs.
    pub blocks_to_include_in_results: Option<Vec<BlockIndex>>,
}

impl FindOptions {
    pub fn with_is_case_sensitive(mut self, is_case_sensitive: bool) -> Self {
        self.is_case_sensitive = is_case_sensitive;
        self
    }

    pub fn with_is_regex_enabled(mut self, is_regex_enabled: bool) -> Self {
        self.is_regex_enabled = is_regex_enabled;
        self
    }

    pub fn with_query(mut self, query: Option<impl Into<Arc<String>>>) -> Self {
        self.query = query.map(Into::into);
        self
    }

    pub fn with_blocks_to_include_in_results(
        mut self,
        block_indices: Option<impl IntoIterator<Item = BlockIndex>>,
    ) -> Self {
        self.blocks_to_include_in_results =
            block_indices.map(|indices| indices.into_iter().collect());
        self
    }
}

impl std::fmt::Debug for TerminalFindModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FindModel")
            .field("terminal_model", &"<TerminalModel>")
            .field(
                "rich_content_views",
                &self.rich_content_views.keys().collect::<Vec<_>>(),
            )
            .field("alt_screen_find_run", &self.alt_screen_find_run)
            .field("block_list_find_run", &self.block_list_find_run)
            .field("is_find_bar_open", &self.is_find_bar_open)
            .finish()
    }
}
