/// In-app log viewer sidebar panel.
///
/// Docks on the right side of the workspace and tails the live Warp log file
/// (path resolved via `warp_logging::log_file_path()`).
///
/// Features
/// - Streams new lines from the log file in real time via a background task
/// - Shows up to 256 KB of existing content on open
/// - Case-insensitive substring filter
/// - Level filter chips (All / INFO / WARN / ERROR)
/// - Auto-scroll to bottom as new lines arrive
/// - Resizable via drag handle on the left edge
/// - Toggled from Help > "View Warp logs" or the header toolbar Logs button

#[cfg(not(target_family = "wasm"))]
use {
    std::io::{BufRead as _, Seek, SeekFrom},
    tokio::io::AsyncBufReadExt as _,
};

use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::Fill as ThemeFill;
use warpui::clipboard::ClipboardContent;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DragBarSide,
    Element, Fill, Flex, Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, Padding,
    ParentElement, Radius, Resizable, ResizableStateHandle, ScrollStateHandle, Scrollable,
    ScrollableElement, ScrollbarWidth, Shrinkable, Text, UniformList, UniformListState,
};
use warpui::fonts::{Properties, Weight};
use warpui::keymap::FixedBinding;
use warpui::platform::Cursor;
use warpui::ui_components::components::{UiComponent as _, UiComponentStyles};
use warpui::ui_components::text_input::TextInput;
use warpui::{AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle};
use pathfinder_color::ColorU;

use crate::drive::panel::{MAX_SIDEBAR_WIDTH_RATIO, MIN_SIDEBAR_WIDTH};
use crate::editor::{EditorView, Event as EditorEvent, SingleLineEditorOptions};
use crate::pane_group::pane::view::header::PANE_HEADER_HEIGHT;
use crate::terminal::resizable_data::{ModalType, ResizableData};
use crate::ui_components::{buttons::icon_button_with_color, icons};
use crate::util::bindings::keybinding_name_to_display_string;
use crate::workspace::WorkspaceAction;

/// Maximum lines kept in memory.
const MAX_LINES: usize = 10_000;
/// How many bytes to seek back from EOF when opening, to show recent context.
const TAIL_INITIAL_BYTES: u64 = 256 * 1024;
/// Poll interval for new lines in milliseconds.
const POLL_INTERVAL_MS: u64 = 500;

/// Keybinding name used to toggle the log viewer sidebar.
pub const TOGGLE_LOG_VIEWER_BINDING_NAME: &str = "workspace:toggle_log_viewer";

// ---------------------------------------------------------------------------
// Level filter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LevelFilter {
    #[default]
    All,
    Info,
    Warn,
    Error,
}

impl LevelFilter {
    fn label(self) -> &'static str {
        match self {
            LevelFilter::All => "All",
            LevelFilter::Info => "INFO",
            LevelFilter::Warn => "WARN",
            LevelFilter::Error => "ERROR",
        }
    }

    fn matches(self, line: &str) -> bool {
        match self {
            LevelFilter::All => true,
            _ => extract_level(line) == self.label(),
        }
    }
}

/// Pull the bracketed level token out of a log line like:
/// `2024-01-01T12:00:00.000 [INFO] message`
fn extract_level(line: &str) -> &str {
    let start = line.find('[').map(|i| i + 1).unwrap_or(0);
    let end = line[start..]
        .find(']')
        .map(|j| start + j)
        .unwrap_or(start);
    line[start..end].trim()
}

// ---------------------------------------------------------------------------
// Actions / Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum LogViewerAction {
    Close,
    SetLevelFilter(LevelFilter),
    FilterChanged,
    ClearBuffer,
    CopyAll,
}

#[derive(Copy, Clone, Debug)]
pub enum LogViewerEvent {
    Close,
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub struct LogViewerPanel {
    /// All lines loaded (capped at MAX_LINES).
    lines: Vec<String>,
    /// Indices into `lines` that pass the current filter.
    filtered_indices: Vec<usize>,
    level_filter: LevelFilter,
    filter_editor: ViewHandle<EditorView>,
    filter_text: String,
    list_state: UniformListState,
    scroll_state: ScrollStateHandle,
    close_button_mouse_state: MouseStateHandle,
    clear_button_mouse_state: MouseStateHandle,
    copy_all_button_mouse_state: MouseStateHandle,
    chip_states: [MouseStateHandle; 4],
    resizable_state_handle: ResizableStateHandle,
}

impl LogViewerPanel {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let filter_editor = ctx.add_typed_action_view(|ctx| {
            let options = SingleLineEditorOptions {
                ..Default::default()
            };
            let mut editor = EditorView::new(options.into(), ctx);
            editor.set_placeholder_text("Filter logs...", ctx);
            editor
        });

        ctx.subscribe_to_view(&filter_editor, |_me, _, event, ctx| {
            if matches!(event, EditorEvent::Edited(_)) {
                ctx.dispatch_typed_action(&LogViewerAction::FilterChanged);
            }
        });

        let resizable_state_handle = {
            let resizable_data_handle = ResizableData::handle(ctx);
            match resizable_data_handle
                .as_ref(ctx)
                .get_handle(ctx.window_id(), ModalType::LogViewerWidth)
            {
                Some(handle) => handle,
                None => {
                    log::error!("Couldn't retrieve log viewer resizable state handle.");
                    warpui::elements::resizable_state_handle(480.0)
                }
            }
        };

        let mut panel = Self {
            lines: Vec::new(),
            filtered_indices: Vec::new(),
            level_filter: LevelFilter::default(),
            filter_editor,
            filter_text: String::new(),
            list_state: UniformListState::new(),
            scroll_state: Default::default(),
            close_button_mouse_state: Default::default(),
            clear_button_mouse_state: Default::default(),
            copy_all_button_mouse_state: Default::default(),
            chip_states: Default::default(),
            resizable_state_handle,
        };

        #[cfg(not(target_family = "wasm"))]
        panel.start_tail(ctx);

        panel
    }

    #[cfg(not(target_family = "wasm"))]
    fn start_tail(&mut self, ctx: &mut ViewContext<Self>) {
        let log_path = match warp_logging::log_file_path() {
            Ok(p) => p,
            Err(err) => {
                log::warn!("LogViewerPanel: could not get log file path: {err}");
                return;
            }
        };

        let (tx, rx) = async_channel::unbounded::<Vec<String>>();

        // Spawn the background reader on the tokio-based background executor.
        ctx.background_executor()
            .spawn(async move {
                // --- Read initial tail from existing file content ---
                let initial: Vec<String> = {
                    match std::fs::File::open(&log_path) {
                        Err(e) => {
                            log::warn!(
                                "LogViewerPanel: cannot open {}: {e}",
                                log_path.display()
                            );
                            return;
                        }
                        Ok(f) => {
                            let mut reader = std::io::BufReader::new(f);
                            let len = reader.seek(SeekFrom::End(0)).unwrap_or(0);
                            let start = len.saturating_sub(TAIL_INITIAL_BYTES);
                            let _ = reader.seek(SeekFrom::Start(start));
                            // Skip the first (possibly partial) line.
                            if start > 0 {
                                let mut skip = String::new();
                                let _ = reader.read_line(&mut skip);
                            }
                            let mut buf = Vec::new();
                            for line in reader.lines().filter_map(Result::ok) {
                                buf.push(line);
                            }
                            buf
                        }
                    }
                };

                if !initial.is_empty() && tx.send(initial).await.is_err() {
                    return;
                }

                // --- Tail new lines as they appear ---
                match tokio::fs::File::open(&log_path).await {
                    Err(e) => {
                        log::warn!(
                            "LogViewerPanel: cannot open for tail {}: {e}",
                            log_path.display()
                        );
                        return;
                    }
                    Ok(f) => {
                        use tokio::io::AsyncSeekExt;
                        let mut reader = tokio::io::BufReader::new(f);
                        if reader.seek(SeekFrom::End(0)).await.is_err() {
                            return;
                        }
                        let mut line_buf = String::new();
                        loop {
                            line_buf.clear();
                            match reader.read_line(&mut line_buf).await {
                                Err(_) => break,
                                Ok(0) => {
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        POLL_INTERVAL_MS,
                                    ))
                                    .await;
                                }
                                Ok(_) => {
                                    let line = line_buf
                                        .trim_end_matches('\n')
                                        .trim_end_matches('\r')
                                        .to_string();
                                    if tx.send(vec![line]).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .detach();

        // Receive lines on the main thread and update the view.
        ctx.spawn_stream_local(
            rx,
            |me: &mut LogViewerPanel, batch: Vec<String>, ctx: &mut ViewContext<LogViewerPanel>| {
                for line in batch {
                    if me.lines.len() >= MAX_LINES {
                        me.lines.remove(0);
                    }
                    me.lines.push(line);
                }
                me.rebuild_filtered();
                me.scroll_to_bottom();
                ctx.notify();
            },
            |_, _| {},
        );
    }

    fn rebuild_filtered(&mut self) {
        let filter = self.filter_text.to_lowercase();
        self.filtered_indices = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                self.level_filter.matches(line)
                    && (filter.is_empty() || line.to_lowercase().contains(&filter))
            })
            .map(|(i, _)| i)
            .collect();
    }

    fn scroll_to_bottom(&mut self) {
        let last = self.filtered_indices.len().saturating_sub(1);
        self.list_state.scroll_to(last);
    }

    fn render_header(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let theme = appearance.theme();
        let sub_text_color = theme.sub_text_color(theme.background());
        let border_color = theme.outline().into_solid();

        let title = Text::new_inline("Warp Logs", appearance.ui_font_family(), 12.)
            .with_style(Properties::default().weight(Weight::Bold))
            .with_color(sub_text_color.into())
            .finish();

        let tooltip_keybinding =
            keybinding_name_to_display_string(TOGGLE_LOG_VIEWER_BINDING_NAME, app);
        let ui_builder = appearance.ui_builder().clone();
        let tooltip = if let Some(keybinding) = tooltip_keybinding {
            ui_builder
                .tool_tip_with_sublabel("Close panel".to_string(), keybinding)
                .build()
                .finish()
        } else {
            ui_builder
                .tool_tip("Close panel".to_string())
                .build()
                .finish()
        };

        let icon_color = theme.sub_text_color(theme.background());
        let action_ui_builder = appearance.ui_builder().clone();

        let clear_tooltip_builder = action_ui_builder.clone();
        let clear_btn = icon_button_with_color(
            appearance,
            icons::Icon::Trash,
            false,
            self.clear_button_mouse_state.clone(),
            icon_color,
        )
        .with_tooltip(move || {
            clear_tooltip_builder
                .tool_tip("Clear log buffer".to_string())
                .build()
                .finish()
        })
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(LogViewerAction::ClearBuffer);
        })
        .with_cursor(Cursor::PointingHand)
        .finish();

        let copy_tooltip_builder = action_ui_builder;
        let copy_all_btn = icon_button_with_color(
            appearance,
            icons::Icon::Copy,
            false,
            self.copy_all_button_mouse_state.clone(),
            icon_color,
        )
        .with_tooltip(move || {
            copy_tooltip_builder
                .tool_tip("Copy all visible lines".to_string())
                .build()
                .finish()
        })
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(LogViewerAction::CopyAll);
        })
        .with_cursor(Cursor::PointingHand)
        .finish();

        let close_btn = icon_button_with_color(
            appearance,
            icons::Icon::X,
            false,
            self.close_button_mouse_state.clone(),
            icon_color,
        )
        .with_tooltip(move || tooltip)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::ToggleLogViewer);
        })
        .with_cursor(Cursor::PointingHand)
        .finish();

        Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(Shrinkable::new(1., title).finish())
                    .with_child(
                        Flex::row()
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_spacing(4.)
                            .with_child(clear_btn)
                            .with_child(copy_all_btn)
                            .with_child(close_btn)
                            .finish(),
                    )
                    .finish(),
            )
            .with_height(PANE_HEADER_HEIGHT)
            .finish(),
        )
        .with_padding_left(16.)
        .with_padding_right(8.)
        .with_border(Border::bottom(1.).with_border_color(border_color))
        .finish()
    }

    fn render_filter_bar(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let text_color = theme.sub_text_color(theme.background()).into_solid();
        let border_color = theme.outline().into_solid();
        let surface_1 = theme.surface_1().into_solid();

        // Filter text input
        let text_input = TextInput::new(
            self.filter_editor.clone(),
            UiComponentStyles::default()
                .set_background(warpui::elements::Fill::None)
                .set_border_radius(CornerRadius::with_all(Radius::Pixels(0.)))
                .set_border_width(0.),
        )
        .build()
        .finish();

        let filter_box = Container::new(Shrinkable::new(1., text_input).finish())
            .with_background(ThemeFill::Solid(surface_1))
            .with_border(Border::all(1.).with_border_color(border_color))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .with_uniform_padding(6.)
            .finish();

        // Level filter chips
        let level_variants = [
            LevelFilter::All,
            LevelFilter::Info,
            LevelFilter::Warn,
            LevelFilter::Error,
        ];
        let active_color = theme.active_ui_detail().into_solid();
        let inactive_color = theme.surface_1().into_solid();
        let level_filter = self.level_filter;

        let font_family = appearance.ui_font_family();
        let font_size_chips = 11.;

        let mut chips = Flex::row().with_spacing(4.);
        for (i, &variant) in level_variants.iter().enumerate() {
            let is_active = level_filter == variant;
            let bg = if is_active { active_color } else { inactive_color };
            let fg = if is_active {
                ColorU::white()
            } else {
                text_color
            };
            let chip = Hoverable::new(self.chip_states[i].clone(), move |_| {
                Container::new(
                    Text::new(variant.label(), font_family, font_size_chips)
                        .with_color(fg)
                        .finish(),
                )
                .with_background(ThemeFill::Solid(bg))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .with_horizontal_padding(8.)
                .with_vertical_padding(3.)
                .finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _app, _pos| {
                ctx.dispatch_typed_action(LogViewerAction::SetLevelFilter(variant));
            })
            .finish();
            chips.add_child(chip);
        }

        let count_str = format!("{} lines", self.filtered_indices.len());
        let count_label = Text::new(
            count_str,
            appearance.ui_font_family(),
            11.,
        )
        .with_color(text_color)
        .finish();

        Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(8.)
                .with_child(Shrinkable::new(1., filter_box).finish())
                .with_child(chips.finish())
                .with_child(count_label)
                .finish(),
        )
        .with_padding(Padding::uniform(8.))
        .with_border(Border::bottom(1.).with_border_color(theme.outline().into_solid()))
        .finish()
    }

    fn render_log_list(&self, appearance: &Appearance) -> Box<dyn Element> {
        let filtered = self.filtered_indices.clone();
        let lines = self.lines.clone();
        let font_family = appearance.ui_font_family();
        let font_size = (appearance.ui_font_size() - 1.).max(10.);

        // Extract colors so the closure is 'static (no Appearance reference captured).
        let theme = appearance.theme();
        let default_color = theme.main_text_color(theme.background()).into_solid();
        let error_color: ColorU = theme.terminal_colors().normal.red.into();
        let warn_color: ColorU = theme.terminal_colors().normal.yellow.into();

        let count = filtered.len();
        let list = UniformList::new(
            self.list_state.clone(),
            count,
            move |range: std::ops::Range<usize>, _app: &AppContext| {
                range
                    .filter_map(|i| {
                        let line_idx = *filtered.get(i)?;
                        let line = lines.get(line_idx)?;
                        let color = match extract_level(line) {
                            "ERROR" => error_color,
                            "WARN" => warn_color,
                            _ => default_color,
                        };
                        Some(
                            Container::new(
                                Text::new(line.clone(), font_family, font_size)
                                    .with_color(color)
                                    .soft_wrap(false)
                                    .finish(),
                            )
                            .with_horizontal_padding(12.)
                            .with_vertical_padding(1.)
                            .finish() as Box<dyn Element>,
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
            },
        )
        .finish_scrollable();

        let theme = appearance.theme();
        Scrollable::vertical(
            self.scroll_state.clone(),
            list,
            ScrollbarWidth::Auto,
            theme.nonactive_ui_detail().into_solid().into(),
            theme.active_ui_detail().into_solid().into(),
            Fill::None,
        )
        .with_overlayed_scrollbar()
        .finish()
    }
}

impl Entity for LogViewerPanel {
    type Event = LogViewerEvent;
}

impl TypedActionView for LogViewerPanel {
    type Action = LogViewerAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            LogViewerAction::Close => ctx.emit(LogViewerEvent::Close),
            LogViewerAction::SetLevelFilter(level) => {
                self.level_filter = *level;
                self.rebuild_filtered();
                self.scroll_to_bottom();
                ctx.notify();
            }
            LogViewerAction::FilterChanged => {
                self.filter_text = self
                    .filter_editor
                    .read(ctx, |editor, ctx| editor.buffer_text(ctx));
                self.rebuild_filtered();
                self.scroll_to_bottom();
                ctx.notify();
            }
            LogViewerAction::ClearBuffer => {
                self.lines.clear();
                self.filtered_indices.clear();
                ctx.notify();
            }
            LogViewerAction::CopyAll => {
                let lines = self
                    .filtered_indices
                    .iter()
                    .filter_map(|&i| self.lines.get(i))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                ctx.clipboard().write(ClipboardContent {
                    plain_text: lines,
                    ..Default::default()
                });
            }
        }
    }
}

impl View for LogViewerPanel {
    fn ui_name() -> &'static str {
        "LogViewerPanel"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let header = self.render_header(appearance, app);
        let filter_bar = self.render_filter_bar(appearance);
        let log_list = self.render_log_list(appearance);

        let body = Flex::column()
            .with_child(header)
            .with_child(filter_bar)
            .with_child(Shrinkable::new(1., log_list).finish())
            .finish();

        Resizable::new(self.resizable_state_handle.clone(), body)
            .with_dragbar_side(DragBarSide::Left)
            .on_resize(move |ctx, _| {
                ctx.notify();
            })
            .with_bounds_callback(Box::new(|window_size| {
                let min_width = MIN_SIDEBAR_WIDTH;
                let max_width = window_size.x() * MAX_SIDEBAR_WIDTH_RATIO;
                (min_width, max_width.max(min_width))
            }))
            .finish()
    }
}

/// Register keyboard shortcuts for `LogViewerPanel`.
pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;
    // Escape closes the panel when focus is inside it.
    app.register_fixed_bindings([FixedBinding::new(
        "escape",
        LogViewerAction::Close,
        id!("LogViewerPanel"),
    )]);
}
