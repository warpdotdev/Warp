//! Cloud-mode V2 slash command menu.
//!
//! A floating, cursor-anchored alternative to `InlineSlashCommandView` that is
//! gated behind `FeatureFlag::CloudModeInputV2`. The legacy view is left
//! untouched everywhere else.
//!
//! Rendering is driven by a single `MenuState` enum so that the two visible
//! shapes (`NoSearchActive` sectioned, `SearchActive` flat) are mutually
//! exclusive and never coexist.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use pathfinder_color::ColorU;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::Fill;
use warpui::elements::{
    Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, DropShadow, EventHandler, Flex, Hoverable,
    MainAxisSize, MouseInBehavior, MouseStateHandle, ParentElement, Radius, ScrollbarWidth, Text,
};
use warpui::platform::Cursor;
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    WeakViewHandle,
};

use crate::search::data_source::QueryFilter;
use crate::search::mixer::{AddAsyncSourceOptions, SearchMixer, SearchMixerEvent};
use crate::search::result_renderer::{QueryResultRenderer, QueryResultRendererStyles};
use crate::terminal::input::buffer_model::{InputBufferModel, InputBufferUpdateEvent};
use crate::terminal::input::inline_menu::styles as inline_styles;
use crate::terminal::input::slash_command_model::{SlashCommandEntryState, SlashCommandModel};
use crate::terminal::input::slash_commands::view::{slash_command_query, CloseReason};
use crate::terminal::input::slash_commands::{
    saved_prompts_data_source, AcceptSlashCommandOrSavedPrompt, SlashCommandDataSource,
    SlashCommandsEvent, UpdatedActiveCommands, ZeroStateDataSource,
};
use crate::terminal::input::suggestions_mode_model::{
    InputSuggestionsModeEvent, InputSuggestionsModeModel,
};

const MENU_WIDTH: f32 = 320.;

/// Figma frame is 380px tall; 400px leaves a few pixels of breathing room and
/// the `Scrollable` clamps to whatever vertical space the parent grants when
/// the window is narrow.
const MENU_MAX_HEIGHT: f32 = 400.;

/// Number of items shown per section in the `NoSearchActive` state before the
/// "Show N more" affordance appears.
const ITEMS_PER_SECTION_COLLAPSED: usize = 3;

const SECTION_HEADER_FONT_SIZE: f32 = 12.;

const ITEM_FONT_SIZE: f32 = 14.;

const MENU_HORIZONTAL_PADDING: f32 = 16.;

const MENU_VERTICAL_PADDING: f32 = 4.;

const MENU_CORNER_RADIUS: f32 = 6.;

const ROW_VERTICAL_PADDING: f32 = 4.;

const ICON_SIZE: f32 = 16.;

const ICON_TO_TEXT_GAP: f32 = 8.;

const DIVIDER_HEIGHT: f32 = 1.;

const DIVIDER_VERTICAL_PADDING: f32 = 4.;

/// Drop shadow color: Figma `rgba(0, 0, 0, 0.3)`. Sourced once here rather than
/// inline so the magic alpha is greppable.
const DROP_SHADOW_COLOR: ColorU = ColorU {
    r: 0,
    g: 0,
    b: 0,
    a: 77, // 0.3 * 255 = 76.5
};

const DROP_SHADOW_OFFSET_Y: f32 = 7.;

const DROP_SHADOW_BLUR_RADIUS: f32 = 7.;

/// Shared renderer styles for the V2 menu rows. Mirrors the subset of
/// `InlineMenuView::QUERY_RESULT_RENDERER_STYLES` we need; we don't reuse that
/// constant because it is private to `inline_menu::view`.
static QUERY_RESULT_RENDERER_STYLES: LazyLock<QueryResultRendererStyles> =
    LazyLock::new(|| QueryResultRendererStyles {
        result_item_height_fn: |appearance| appearance.monospace_font_size() + 8.,
        panel_corner_radius: CornerRadius::with_all(Radius::Pixels(0.)),
        result_vertical_padding: ROW_VERTICAL_PADDING,
        ..Default::default()
    });

/// Section identifier. The mapping from `AcceptSlashCommandOrSavedPrompt`
/// variant to section is deterministic; see `Section::for_action`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Section {
    Commands,
    Skills,
    Prompts,
}

impl Section {
    /// Order in which sections render in the `NoSearchActive` state.
    const RENDER_ORDER: [Self; 3] = [Self::Commands, Self::Skills, Self::Prompts];

    fn header(self) -> &'static str {
        match self {
            Self::Commands => "Commands",
            Self::Skills => "Skills",
            Self::Prompts => "Prompts",
        }
    }

    fn for_action(action: &AcceptSlashCommandOrSavedPrompt) -> Self {
        match action {
            AcceptSlashCommandOrSavedPrompt::SlashCommand { .. } => Self::Commands,
            AcceptSlashCommandOrSavedPrompt::Skill { .. } => Self::Skills,
            AcceptSlashCommandOrSavedPrompt::SavedPrompt { .. } => Self::Prompts,
        }
    }
}

/// Result renderers grouped under a section. Items keep score order within the
/// section (mixer returns ascending priority).
struct RenderedSection {
    section: Section,
    items: Vec<QueryResultRenderer<AcceptSlashCommandOrSavedPrompt>>,
}

/// Visible-row representation used only by the `NoSearchActive` state.
/// Computed on demand from `(sections, expanded_sections)` for both rendering
/// and keyboard navigation; not stored on the struct.
#[derive(Clone, Copy)]
enum NoSearchActiveRow {
    SectionHeader(Section),
    Item {
        section: Section,
        item_idx: usize,
    },
    /// Show the remaining items in `section`. `hidden_count` is the number of
    /// items currently hidden behind the truncation; used in the row label.
    ShowMore {
        section: Section,
        hidden_count: usize,
    },
    Divider,
}

impl NoSearchActiveRow {
    fn is_selectable(self) -> bool {
        matches!(
            self,
            NoSearchActiveRow::Item { .. } | NoSearchActiveRow::ShowMore { .. }
        )
    }
}

/// Top-level state for the V2 menu. Exactly one variant is live at a time.
enum MenuState {
    /// User has not entered a query (just typed `/`). Results are grouped into
    /// the three sections, each truncated to `ITEMS_PER_SECTION_COLLAPSED`
    /// until the user activates the section's `Show N more` row.
    NoSearchActive {
        sections: Vec<RenderedSection>,
        expanded_sections: HashSet<Section>,
        /// Index into the visible-row sequence (`browsing_rows`). Headers and
        /// dividers are skipped during navigation; items and `ShowMore` rows
        /// are selectable.
        selected_idx: Option<usize>,
        /// Mouse-state handles for `ShowMore` rows, keyed by section.
        show_more_mouse_states: HashMap<Section, MouseStateHandle>,
    },
    /// User has typed a query. Results are pulled from across all sections
    /// into a single flat list sorted by match score, with fuzzy match indices
    /// highlighted.
    SearchActive {
        results: Vec<QueryResultRenderer<AcceptSlashCommandOrSavedPrompt>>,
        /// Index directly into `results`. Disabled items are skipped during
        /// navigation.
        selected_idx: Option<usize>,
    },
}

impl MenuState {
    fn empty() -> Self {
        MenuState::NoSearchActive {
            sections: Vec::new(),
            expanded_sections: HashSet::new(),
            selected_idx: None,
            show_more_mouse_states: HashMap::new(),
        }
    }
}

/// Internal action used to wire mouse hover/click events back into the view.
#[derive(Debug, Clone)]
pub enum CloudModeV2SlashCommandAction {
    /// Accept the given item (Enter or click).
    Accept {
        item: AcceptSlashCommandOrSavedPrompt,
        cmd_or_ctrl_enter: bool,
    },
    /// Move the keyboard selection to the result at `idx` (mouse hover).
    HoverIdx(usize),
    /// Toggle expansion for `section` (Show N more clicked).
    ToggleSection(Section),
    /// User dismissed the menu (clicked outside, escape).
    Dismiss,
}

pub struct CloudModeV2SlashCommandView {
    mixer: ModelHandle<SearchMixer<AcceptSlashCommandOrSavedPrompt>>,
    suggestions_mode_model: ModelHandle<InputSuggestionsModeModel>,
    input_buffer_model: ModelHandle<InputBufferModel>,
    weak_handle: WeakViewHandle<Self>,
    scroll_state: ClippedScrollStateHandle,
    /// Mutually exclusive: at any moment the menu is either `NoSearchActive`
    /// (no query, sectioned with expand controls) or `SearchActive` (query,
    /// flat ranked list). Replacing the previous `sections` + `flat_rows`
    /// field pair with this enum means we never carry a stale empty copy of
    /// the unused shape.
    menu_state: MenuState,
}

impl CloudModeV2SlashCommandView {
    pub fn new(
        slash_command_model: &ModelHandle<SlashCommandModel>,
        slash_commands_source: ModelHandle<SlashCommandDataSource>,
        suggestions_mode_model: ModelHandle<InputSuggestionsModeModel>,
        input_buffer_model: ModelHandle<InputBufferModel>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        // Re-run the active query whenever the set of active commands changes
        // (e.g. CWD update, AI toggle). Mirrors `InlineSlashCommandView::new`.
        ctx.subscribe_to_model(
            &slash_commands_source,
            |me, _, _: &UpdatedActiveCommands, ctx| {
                me.mixer.update(ctx, |mixer, ctx| {
                    if let Some(query) = mixer.current_query().cloned() {
                        mixer.run_query(query, ctx);
                    }
                });
            },
        );

        let zero_state_source =
            ctx.add_model(|_| ZeroStateDataSource::for_cloud_mode_v2(&slash_commands_source));
        let saved_prompts_source = saved_prompts_data_source();

        let mixer = ctx.add_model(|ctx| {
            let mut mixer = SearchMixer::<AcceptSlashCommandOrSavedPrompt>::new();
            mixer.add_sync_source(
                slash_commands_source.clone(),
                [QueryFilter::StaticSlashCommands],
            );
            // V2 keeps the saved-prompts async source but configures it
            // identically to the legacy view; saved prompts in zero state are
            // sourced from the V2 zero-state extension instead so the legacy
            // mixer config doesn't need to change.
            mixer.add_async_source(
                saved_prompts_source,
                [QueryFilter::StaticSlashCommands],
                AddAsyncSourceOptions {
                    debounce_interval: None,
                    run_in_zero_state: false,
                    run_when_unfiltered: false,
                },
                ctx,
            );
            mixer.add_sync_source(
                zero_state_source.clone(),
                [QueryFilter::StaticSlashCommands],
            );
            mixer.run_query(slash_command_query(""), ctx);
            mixer
        });

        ctx.subscribe_to_model(&mixer, |me, _, event, ctx| match event {
            SearchMixerEvent::ResultsChanged => {
                if me.mixer.as_ref(ctx).is_loading() {
                    // Keep stale results visible while async sources are
                    // pending to avoid flicker. Mirrors `InlineMenuView`.
                    return;
                }
                me.rebuild_from_results(ctx);
                ctx.notify();
            }
        });

        // Re-run query when the slash command model state changes (the user
        // typed after the leading `/`). Same gating as the legacy view: only
        // re-run while the menu is open so we don't burn cycles on saved
        // prompt searches after the menu has been closed.
        ctx.subscribe_to_model(slash_command_model, |me, model, _, ctx| {
            if !me.suggestions_mode_model.as_ref(ctx).is_slash_commands() {
                return;
            }
            match model.as_ref(ctx).state().clone() {
                SlashCommandEntryState::None
                | SlashCommandEntryState::Composing { .. }
                | SlashCommandEntryState::SlashCommand(_) => {
                    me.run_query_for_current_slash_filter(ctx);
                }
                _ => (),
            }
        });

        // Buffer subscription so we transition between `NoSearchActive` and
        // `SearchActive` immediately as the user adds/removes characters
        // after the `/`, even if the slash command model didn't move state.
        ctx.subscribe_to_model(
            &input_buffer_model,
            |me, _, _: &InputBufferUpdateEvent, ctx| {
                if !me.suggestions_mode_model.as_ref(ctx).is_slash_commands() {
                    return;
                }
                me.run_query_for_current_slash_filter(ctx);
            },
        );

        ctx.subscribe_to_model(&suggestions_mode_model, |me, _, event, ctx| {
            let InputSuggestionsModeEvent::ModeChanged { .. } = event;
            if me.suggestions_mode_model.as_ref(ctx).is_closed() {
                me.mixer.update(ctx, |mixer, ctx| {
                    mixer.reset_results(ctx);
                });
                me.menu_state = MenuState::empty();
                return;
            }
            // If the menu reopened with a slash query already in the buffer,
            // re-run the query so we don't show stale results.
            if me.suggestions_mode_model.as_ref(ctx).is_slash_commands() {
                me.run_query_for_current_slash_filter(ctx);
            }
        });

        Self {
            mixer,
            suggestions_mode_model,
            input_buffer_model,
            weak_handle: ctx.handle(),
            scroll_state: Default::default(),
            menu_state: MenuState::empty(),
        }
    }

    pub fn select_up(&mut self, ctx: &mut ViewContext<Self>) {
        self.move_selection(SelectionDirection::Up, ctx);
    }

    pub fn select_down(&mut self, ctx: &mut ViewContext<Self>) {
        self.move_selection(SelectionDirection::Down, ctx);
    }

    pub fn accept_selected_item(&mut self, cmd_or_ctrl_enter: bool, ctx: &mut ViewContext<Self>) {
        match &self.menu_state {
            MenuState::NoSearchActive {
                sections,
                expanded_sections,
                selected_idx,
                ..
            } => {
                let Some(selected_idx) = *selected_idx else {
                    return;
                };
                let rows = browsing_rows(sections, expanded_sections);
                let Some(row) = rows.get(selected_idx) else {
                    return;
                };
                match *row {
                    NoSearchActiveRow::Item { section, item_idx } => {
                        let Some(rendered) = sections.iter().find(|s| s.section == section) else {
                            return;
                        };
                        let Some(item) = rendered.items.get(item_idx) else {
                            return;
                        };
                        if item.search_result.is_disabled() {
                            return;
                        }
                        let action = item.search_result.accept_result();
                        self.emit_selection(&action, cmd_or_ctrl_enter, ctx);
                    }
                    NoSearchActiveRow::ShowMore { section, .. } => {
                        self.toggle_section(section, ctx);
                    }
                    NoSearchActiveRow::SectionHeader(_) | NoSearchActiveRow::Divider => {}
                }
            }
            MenuState::SearchActive {
                results,
                selected_idx,
            } => {
                let Some(idx) = *selected_idx else {
                    return;
                };
                let Some(item) = results.get(idx) else {
                    return;
                };
                if item.search_result.is_disabled() {
                    return;
                }
                let action = item.search_result.accept_result();
                self.emit_selection(&action, cmd_or_ctrl_enter, ctx);
            }
        }
    }

    pub fn dismiss(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(SlashCommandsEvent::Close(CloseReason::ManualDismissal));
    }

    /// Returns the number of currently visible result items (for callers that
    /// gate on the count, e.g. `Input::handle_slash_command_model_event`).
    pub fn result_count(&self, app: &AppContext) -> usize {
        self.mixer.as_ref(app).results().len()
    }

    fn current_query_text(&self, app: &AppContext) -> String {
        self.input_buffer_model
            .as_ref(app)
            .current_value()
            .strip_prefix('/')
            .map(ToOwned::to_owned)
            .unwrap_or_default()
    }

    fn run_query_for_current_slash_filter(&mut self, ctx: &mut ViewContext<Self>) {
        let filter = self.current_query_text(ctx);
        self.mixer.update(ctx, move |mixer, ctx| {
            if mixer.current_query().is_some_and(|q| q.text == filter) {
                return;
            }
            mixer.run_query(slash_command_query(&filter), ctx);
        });
    }

    fn rebuild_from_results(&mut self, ctx: &mut ViewContext<Self>) {
        let weak_handle = self.weak_handle.clone();
        let on_click_fn = move |_idx: usize,
                                item: AcceptSlashCommandOrSavedPrompt,
                                evt_ctx: &mut warpui::EventContext| {
            // Forward clicks through the typed action so the view receives
            // them in `handle_action` regardless of which row dispatched.
            evt_ctx.dispatch_typed_action(CloudModeV2SlashCommandAction::Accept {
                item,
                cmd_or_ctrl_enter: false,
            });
            let _ = weak_handle; // keep the closure self-contained.
        };

        // The mixer sorts results ascending by `(priority_tier, score,
        // source_order)`, so the highest-priority item is at the *end* of
        // the vec. Our menu renders top-to-bottom, so we reverse here to put
        // the best match (or, for zero state, the alphabetically-first item
        // since data sources emit in name-descending order) at the top.
        let renderers: Vec<QueryResultRenderer<AcceptSlashCommandOrSavedPrompt>> = self
            .mixer
            .as_ref(ctx)
            .results()
            .iter()
            .rev()
            .enumerate()
            .map(|(idx, result)| {
                QueryResultRenderer::new(
                    result.clone(),
                    format!("v2_slash:{idx}"),
                    on_click_fn.clone(),
                    *QUERY_RESULT_RENDERER_STYLES,
                )
            })
            .collect();

        let query_is_empty = self.current_query_text(ctx).is_empty();

        self.menu_state = if query_is_empty {
            let mut by_section: HashMap<Section, Vec<QueryResultRenderer<_>>> = HashMap::new();
            for renderer in renderers {
                let section = Section::for_action(&renderer.search_result.accept_result());
                by_section.entry(section).or_default().push(renderer);
            }
            let sections: Vec<RenderedSection> = Section::RENDER_ORDER
                .into_iter()
                .map(|s| RenderedSection {
                    section: s,
                    items: by_section.remove(&s).unwrap_or_default(),
                })
                .collect();

            // Carry expansion across rebuilds while staying in
            // `NoSearchActive`; reset on transition from `SearchActive`.
            let expanded_sections = match &self.menu_state {
                MenuState::NoSearchActive {
                    expanded_sections, ..
                } => expanded_sections.clone(),
                MenuState::SearchActive { .. } => HashSet::new(),
            };

            // Allocate a stable mouse-state handle per section's `Show More`
            // row so hover state survives rebuilds.
            let mut show_more_mouse_states = match &self.menu_state {
                MenuState::NoSearchActive {
                    show_more_mouse_states,
                    ..
                } => show_more_mouse_states.clone(),
                MenuState::SearchActive { .. } => HashMap::new(),
            };
            for section in Section::RENDER_ORDER {
                show_more_mouse_states
                    .entry(section)
                    .or_insert_with(MouseStateHandle::default);
            }

            let mut state = MenuState::NoSearchActive {
                sections,
                expanded_sections,
                selected_idx: None,
                show_more_mouse_states,
            };
            initialize_browsing_selection(&mut state);
            state
        } else {
            let mut state = MenuState::SearchActive {
                results: renderers,
                selected_idx: None,
            };
            initialize_search_selection(&mut state);
            state
        };
    }

    fn toggle_section(&mut self, section: Section, ctx: &mut ViewContext<Self>) {
        if let MenuState::NoSearchActive {
            expanded_sections, ..
        } = &mut self.menu_state
        {
            if !expanded_sections.insert(section) {
                expanded_sections.remove(&section);
            }
        }
        // Selection may now point past the end of the new visible row list.
        clamp_browsing_selection(&mut self.menu_state);
        ctx.notify();
    }

    fn emit_selection(
        &self,
        action: &AcceptSlashCommandOrSavedPrompt,
        cmd_or_ctrl_enter: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            AcceptSlashCommandOrSavedPrompt::SlashCommand { id } => {
                ctx.emit(SlashCommandsEvent::SelectedStaticCommand {
                    id: *id,
                    cmd_or_ctrl_enter,
                });
            }
            AcceptSlashCommandOrSavedPrompt::SavedPrompt { id } => {
                ctx.emit(SlashCommandsEvent::SelectedSavedPrompt { id: *id });
            }
            AcceptSlashCommandOrSavedPrompt::Skill { name, reference } => {
                ctx.emit(SlashCommandsEvent::SelectedSkill {
                    reference: reference.clone(),
                    name: name.clone(),
                });
            }
        }
    }

    fn move_selection(&mut self, direction: SelectionDirection, ctx: &mut ViewContext<Self>) {
        match &mut self.menu_state {
            MenuState::NoSearchActive {
                sections,
                expanded_sections,
                selected_idx,
                ..
            } => {
                let rows = browsing_rows(sections, expanded_sections);
                if rows.is_empty() {
                    return;
                }
                let next = next_selectable_browsing_idx(&rows, *selected_idx, direction);
                if let Some(next) = next {
                    *selected_idx = Some(next);
                }
            }
            MenuState::SearchActive {
                results,
                selected_idx,
            } => {
                if results.is_empty() {
                    return;
                }
                let next = next_selectable_search_idx(results, *selected_idx, direction);
                if let Some(next) = next {
                    *selected_idx = Some(next);
                }
            }
        }
        ctx.notify();
    }

    fn set_browsing_selection(&mut self, idx: usize, ctx: &mut ViewContext<Self>) {
        if let MenuState::NoSearchActive {
            sections,
            expanded_sections,
            selected_idx,
            ..
        } = &mut self.menu_state
        {
            let rows = browsing_rows(sections, expanded_sections);
            if rows.get(idx).is_some_and(|r| r.is_selectable()) {
                *selected_idx = Some(idx);
                ctx.notify();
            }
        }
    }

    fn set_search_selection(&mut self, idx: usize, ctx: &mut ViewContext<Self>) {
        if let MenuState::SearchActive {
            results,
            selected_idx,
        } = &mut self.menu_state
        {
            if let Some(item) = results.get(idx) {
                if !item.search_result.is_disabled() {
                    *selected_idx = Some(idx);
                    ctx.notify();
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum SelectionDirection {
    Up,
    Down,
}

/// Builds the visible-row sequence for the `NoSearchActive` state.
fn browsing_rows(
    sections: &[RenderedSection],
    expanded_sections: &HashSet<Section>,
) -> Vec<NoSearchActiveRow> {
    let mut rows = Vec::new();
    let non_empty_sections: Vec<&RenderedSection> =
        sections.iter().filter(|s| !s.items.is_empty()).collect();

    for (idx, rendered) in non_empty_sections.iter().enumerate() {
        rows.push(NoSearchActiveRow::SectionHeader(rendered.section));
        let visible_count = if expanded_sections.contains(&rendered.section) {
            rendered.items.len()
        } else {
            rendered.items.len().min(ITEMS_PER_SECTION_COLLAPSED)
        };
        for item_idx in 0..visible_count {
            rows.push(NoSearchActiveRow::Item {
                section: rendered.section,
                item_idx,
            });
        }
        let hidden_count = rendered.items.len().saturating_sub(visible_count);
        if hidden_count > 0 {
            rows.push(NoSearchActiveRow::ShowMore {
                section: rendered.section,
                hidden_count,
            });
        }
        let is_last = idx + 1 == non_empty_sections.len();
        if !is_last {
            rows.push(NoSearchActiveRow::Divider);
        }
    }
    rows
}

fn initialize_browsing_selection(state: &mut MenuState) {
    if let MenuState::NoSearchActive {
        sections,
        expanded_sections,
        selected_idx,
        ..
    } = state
    {
        let rows = browsing_rows(sections, expanded_sections);
        *selected_idx = rows.iter().position(|r| r.is_selectable());
    }
}

fn initialize_search_selection(state: &mut MenuState) {
    if let MenuState::SearchActive {
        results,
        selected_idx,
    } = state
    {
        *selected_idx = results.iter().position(|r| !r.search_result.is_disabled());
    }
}

fn clamp_browsing_selection(state: &mut MenuState) {
    if let MenuState::NoSearchActive {
        sections,
        expanded_sections,
        selected_idx,
        ..
    } = state
    {
        let rows = browsing_rows(sections, expanded_sections);
        let in_range = selected_idx
            .as_ref()
            .is_some_and(|idx| rows.get(*idx).is_some_and(|r| r.is_selectable()));
        if !in_range {
            *selected_idx = rows.iter().position(|r| r.is_selectable());
        }
    }
}

fn next_selectable_browsing_idx(
    rows: &[NoSearchActiveRow],
    current: Option<usize>,
    direction: SelectionDirection,
) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let count = rows.len();
    let start = match (current, direction) {
        (Some(idx), SelectionDirection::Down) if idx + 1 < count => idx + 1,
        (Some(_), SelectionDirection::Down) => 0,
        (Some(idx), SelectionDirection::Up) if idx > 0 => idx - 1,
        (Some(_), SelectionDirection::Up) => count - 1,
        (None, SelectionDirection::Down) => 0,
        (None, SelectionDirection::Up) => count - 1,
    };
    for offset in 0..count {
        let candidate = match direction {
            SelectionDirection::Down => (start + offset) % count,
            SelectionDirection::Up => (start + count - offset) % count,
        };
        if rows[candidate].is_selectable() {
            return Some(candidate);
        }
    }
    None
}

fn next_selectable_search_idx(
    results: &[QueryResultRenderer<AcceptSlashCommandOrSavedPrompt>],
    current: Option<usize>,
    direction: SelectionDirection,
) -> Option<usize> {
    if results.is_empty() {
        return None;
    }
    let count = results.len();
    let start = match (current, direction) {
        (Some(idx), SelectionDirection::Down) if idx + 1 < count => idx + 1,
        (Some(_), SelectionDirection::Down) => 0,
        (Some(idx), SelectionDirection::Up) if idx > 0 => idx - 1,
        (Some(_), SelectionDirection::Up) => count - 1,
        (None, SelectionDirection::Down) => 0,
        (None, SelectionDirection::Up) => count - 1,
    };
    for offset in 0..count {
        let candidate = match direction {
            SelectionDirection::Down => (start + offset) % count,
            SelectionDirection::Up => (start + count - offset) % count,
        };
        if !results[candidate].search_result.is_disabled() {
            return Some(candidate);
        }
    }
    None
}

impl CloudModeV2SlashCommandView {
    fn render_no_search_active(
        &self,
        sections: &[RenderedSection],
        expanded_sections: &HashSet<Section>,
        selected_idx: Option<usize>,
        show_more_mouse_states: &HashMap<Section, MouseStateHandle>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let rows = browsing_rows(sections, expanded_sections);
        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);

        for (visible_idx, row) in rows.iter().enumerate() {
            let is_selected = selected_idx == Some(visible_idx);
            match *row {
                NoSearchActiveRow::SectionHeader(section) => {
                    column.add_child(render_section_header(section, app));
                }
                NoSearchActiveRow::Item { section, item_idx } => {
                    let Some(rendered) = sections.iter().find(|s| s.section == section) else {
                        continue;
                    };
                    let Some(renderer) = rendered.items.get(item_idx) else {
                        continue;
                    };
                    column.add_child(self.wrap_with_hover(
                        renderer.render_inline(visible_idx, is_selected, app),
                        visible_idx,
                        /*is_browsing=*/ true,
                    ));
                }
                NoSearchActiveRow::ShowMore {
                    section,
                    hidden_count,
                } => {
                    let mouse_state = show_more_mouse_states
                        .get(&section)
                        .cloned()
                        .unwrap_or_default();
                    column.add_child(render_show_more_row(
                        section,
                        hidden_count,
                        is_selected,
                        mouse_state,
                        visible_idx,
                        app,
                    ));
                }
                NoSearchActiveRow::Divider => {
                    column.add_child(render_divider(app));
                }
            }
        }

        column.finish()
    }

    fn render_search_active(
        &self,
        results: &[QueryResultRenderer<AcceptSlashCommandOrSavedPrompt>],
        selected_idx: Option<usize>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        for (idx, renderer) in results.iter().enumerate() {
            let is_selected = selected_idx == Some(idx);
            column.add_child(self.wrap_with_hover(
                renderer.render_inline(idx, is_selected, app),
                idx,
                /*is_browsing=*/ false,
            ));
        }
        column.finish()
    }

    /// Wraps an item-row element with a `MouseInBehavior` so hover updates the
    /// keyboard selection — same UX as `InlineMenuView` rows.
    fn wrap_with_hover(
        &self,
        element: Box<dyn Element>,
        idx: usize,
        _is_browsing: bool,
    ) -> Box<dyn Element> {
        EventHandler::new(element)
            .on_mouse_in(
                move |ctx, _, _| {
                    ctx.dispatch_typed_action(CloudModeV2SlashCommandAction::HoverIdx(idx));
                    DispatchEventResult::PropagateToParent
                },
                Some(MouseInBehavior {
                    fire_on_synthetic_events: false,
                    fire_when_covered: true,
                }),
            )
            .finish()
    }

    fn render_no_results_state(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let menu_bg = inline_styles::menu_background_color(app);
        let label = if self.mixer.as_ref(app).is_loading() {
            "Loading..."
        } else {
            "No results"
        };
        Container::new(
            Text::new(
                label.to_owned(),
                appearance.ui_font_family(),
                ITEM_FONT_SIZE,
            )
            .with_color(theme.disabled_text_color(Fill::Solid(menu_bg)).into_solid())
            .finish(),
        )
        .with_horizontal_padding(MENU_HORIZONTAL_PADDING)
        .with_vertical_padding(ROW_VERTICAL_PADDING)
        .finish()
    }
}

impl Entity for CloudModeV2SlashCommandView {
    type Event = SlashCommandsEvent;
}

impl TypedActionView for CloudModeV2SlashCommandView {
    type Action = CloudModeV2SlashCommandAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            CloudModeV2SlashCommandAction::Accept {
                item,
                cmd_or_ctrl_enter,
            } => {
                self.emit_selection(item, *cmd_or_ctrl_enter, ctx);
            }
            CloudModeV2SlashCommandAction::HoverIdx(idx) => {
                let idx = *idx;
                match &self.menu_state {
                    MenuState::NoSearchActive { .. } => self.set_browsing_selection(idx, ctx),
                    MenuState::SearchActive { .. } => self.set_search_selection(idx, ctx),
                }
            }
            CloudModeV2SlashCommandAction::ToggleSection(section) => {
                self.toggle_section(*section, ctx);
            }
            CloudModeV2SlashCommandAction::Dismiss => {
                self.dismiss(ctx);
            }
        }
    }
}

impl View for CloudModeV2SlashCommandView {
    fn ui_name() -> &'static str {
        "CloudModeV2SlashCommandView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let menu_bg = inline_styles::menu_background_color(app);

        let content: Box<dyn Element> = match &self.menu_state {
            MenuState::NoSearchActive {
                sections,
                expanded_sections,
                selected_idx,
                show_more_mouse_states,
            } => {
                let any_items = sections.iter().any(|s| !s.items.is_empty());
                if !any_items {
                    self.render_no_results_state(app)
                } else {
                    self.render_no_search_active(
                        sections,
                        expanded_sections,
                        *selected_idx,
                        show_more_mouse_states,
                        app,
                    )
                }
            }
            MenuState::SearchActive {
                results,
                selected_idx,
            } => {
                if results.is_empty() {
                    self.render_no_results_state(app)
                } else {
                    self.render_search_active(results, *selected_idx, app)
                }
            }
        };

        let scrollable = ClippedScrollable::vertical(
            self.scroll_state.clone(),
            content,
            ScrollbarWidth::Auto,
            theme.nonactive_ui_detail().into(),
            theme.active_ui_detail().into(),
            warpui::elements::Fill::None,
        )
        .with_overlayed_scrollbar()
        .finish();

        Container::new(
            ConstrainedBox::new(scrollable)
                .with_max_height(MENU_MAX_HEIGHT)
                .with_max_width(MENU_WIDTH)
                .finish(),
        )
        .with_background(Fill::Solid(menu_bg))
        .with_border(Border::all(1.).with_border_fill(Fill::Solid(theme.outline().into_solid())))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(MENU_CORNER_RADIUS)))
        .with_padding_top(MENU_VERTICAL_PADDING)
        .with_padding_bottom(MENU_VERTICAL_PADDING)
        .with_drop_shadow(DropShadow {
            color: DROP_SHADOW_COLOR,
            offset: pathfinder_geometry::vector::vec2f(0., DROP_SHADOW_OFFSET_Y),
            blur_radius: DROP_SHADOW_BLUR_RADIUS,
            spread_radius: 0.,
        })
        .finish()
    }
}

fn render_section_header(section: Section, app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let menu_bg = inline_styles::menu_background_color(app);
    let header_color = theme.sub_text_color(Fill::Solid(menu_bg)).into_solid();

    Container::new(
        Text::new(
            section.header().to_owned(),
            appearance.ui_font_family(),
            SECTION_HEADER_FONT_SIZE,
        )
        .with_color(header_color)
        .finish(),
    )
    .with_horizontal_padding(MENU_HORIZONTAL_PADDING)
    .with_vertical_padding(ROW_VERTICAL_PADDING)
    .finish()
}

fn render_show_more_row(
    section: Section,
    hidden_count: usize,
    is_selected: bool,
    mouse_state: MouseStateHandle,
    _row_idx: usize,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let menu_bg = inline_styles::menu_background_color(app);
    let secondary_color = theme.sub_text_color(Fill::Solid(menu_bg)).into_solid();

    let label = format!("Show {hidden_count} more");

    let row = Hoverable::new(mouse_state, move |mouse_state| {
        let bg = if is_selected || mouse_state.is_hovered() {
            Some(theme.surface_overlay_2())
        } else {
            None
        };
        let mut container = Container::new(
            Text::new(label.clone(), appearance.ui_font_family(), ITEM_FONT_SIZE)
                .with_color(secondary_color)
                .finish(),
        )
        .with_horizontal_padding(MENU_HORIZONTAL_PADDING)
        .with_vertical_padding(ROW_VERTICAL_PADDING);
        if let Some(bg) = bg {
            container = container.with_background(bg);
        }
        container.finish()
    })
    .with_cursor(Cursor::PointingHand)
    .finish();

    EventHandler::new(row)
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .on_left_mouse_up(move |evt_ctx, _, _| {
            evt_ctx.dispatch_typed_action(CloudModeV2SlashCommandAction::ToggleSection(section));
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn render_divider(app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    Container::new(
        ConstrainedBox::new(warpui::elements::Empty::new().finish())
            .with_height(DIVIDER_HEIGHT)
            .finish(),
    )
    .with_background(theme.surface_overlay_2())
    .with_vertical_padding(DIVIDER_VERTICAL_PADDING)
    .finish()
}

// ICON_SIZE / ICON_TO_TEXT_GAP are referenced from the design specs; surface
// them via small helpers so the constants don't appear unused if the per-row
// element layout shifts.
#[allow(dead_code)]
fn _icon_size() -> f32 {
    ICON_SIZE
}

#[allow(dead_code)]
fn _icon_to_text_gap() -> f32 {
    ICON_TO_TEXT_GAP
}
