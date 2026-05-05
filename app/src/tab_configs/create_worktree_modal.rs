//! Chip-triggered worktree creation modal. Built fresh per the open/closed principle
//! so the existing `NewWorktreeModal` (APP-3679) stays untouched for its legacy entry
//! points (tab menu items). This variant is opened from the worktrees chip footer
//! and tailored to the chip's context: the repo is already known and we have the
//! worktree list in hand.
//!
//! Layout:
//!   - Source worktree picker (lists worktrees of the current repo, root marked with
//!     a `(root)` suffix). Default = current worktree.
//!   - Branch picker (reused from the existing modal). Default = source's HEAD.
//!   - Destination directory text field. Default = `~/.warp/worktrees/{repo}/`.
//!   - Worktree name text field. Required.
//!   - Cancel / Create buttons in the footer.

use std::path::PathBuf;

use warpui::{
    elements::{
        Border, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Element,
        Empty, Flex, MainAxisAlignment, MainAxisSize, MouseStateHandle, Padding, ParentElement,
        Radius, Text,
    },
    fonts::{Properties, Weight},
    keymap::FixedBinding,
    platform::Cursor,
    ui_components::button::ButtonVariant,
    AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use warp_core::ui::theme::color::internal_colors;

use crate::{
    appearance::Appearance,
    editor::{EditorView, Event as EditorEvent, SingleLineEditorOptions},
    tab_configs::branch_picker::BranchPicker,
};

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;
    app.register_fixed_bindings(vec![FixedBinding::new(
        "escape",
        CreateWorktreeModalAction::Escape,
        id!("CreateWorktreeModal"),
    )]);
}

const SECTION_GAP: f32 = 16.;
const LABEL_BOTTOM_MARGIN: f32 = 4.;
const CONTENT_HORIZONTAL_PADDING: f32 = 24.;
const HEADER_PADDING_TOP: f32 = 24.;
const HEADER_PADDING_BOTTOM: f32 = 12.;
const HEADER_TITLE_FONT_SIZE: f32 = 16.;
const BODY_BOTTOM_PADDING: f32 = 16.;
const FOOTER_VERTICAL_PADDING: f32 = 12.;
const FOOTER_BUTTON_HEIGHT: f32 = 32.;
const FOOTER_BUTTON_HORIZONTAL_PADDING: f32 = 12.;
const FOOTER_BUTTON_GAP: f32 = 8.;
const FOOTER_BUTTON_RADIUS: Radius = Radius::Pixels(4.);

const ERROR_FONT_SIZE: f32 = 12.;
const INVALID_NAME_ERROR: &str =
    "Name can only contain letters, numbers, hyphens, and underscores";

fn is_valid_worktree_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Configuration the workspace passes to seed the modal when opening it from the
/// worktrees chip. The modal silently uses `current_worktree_path` as the cwd for
/// the eventual `git worktree add` (any worktree of the repo works) and pre-fills
/// the destination input with `default_destination_dir`.
#[derive(Clone, Debug)]
pub struct CreateWorktreeModalSeed {
    pub current_worktree_path: Option<PathBuf>,
    pub default_destination_dir: PathBuf,
}

pub struct CreateWorktreeModal {
    branch_picker: ViewHandle<BranchPicker>,
    destination_editor: ViewHandle<EditorView>,
    name_editor: ViewHandle<EditorView>,
    /// New branch the worktree will check out (`git worktree add -b <branch>`).
    /// Auto-mirrors the worktree name char-by-char; once the user manually edits
    /// it the mirror stops (`branch_name_overridden = true`).
    branch_name_editor: ViewHandle<EditorView>,
    branch_name_overridden: bool,
    /// Last value we set into `branch_name_editor` programmatically — used to tell
    /// our auto-mirror updates from real user edits in the editor's `Edited` event.
    last_programmatic_branch_value: String,
    cancel_button_mouse_state: MouseStateHandle,
    create_button_mouse_state: MouseStateHandle,
    /// Worktree path used as cwd for the eventual `git worktree add` command.
    /// Any worktree of the repo works — git resolves up to the repo root — so we
    /// just keep whichever one the chip handed us at open time.
    cwd: Option<PathBuf>,
    /// Seed data; kept around for re-rendering and submit.
    seed: Option<CreateWorktreeModalSeed>,
}

#[derive(Debug, Clone)]
pub enum CreateWorktreeModalEvent {
    Close,
    Submit {
        /// Worktree path to use as `cwd` for the `git worktree add` command. Any
        /// worktree of the repo works because git resolves up to the repo root.
        source_worktree: PathBuf,
        /// Branch the new worktree should be based on (or detached HEAD when None).
        branch: String,
        /// Destination path for the new worktree (passed to `git worktree add`).
        destination: PathBuf,
        /// User-supplied worktree name, validated for filename safety.
        worktree_name: String,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum CreateWorktreeModalAction {
    Cancel,
    Create,
    Escape,
}

impl CreateWorktreeModal {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let branch_picker = ctx.add_typed_action_view(|ctx| BranchPicker::new(None, None, ctx));
        let destination_editor = ctx.add_typed_action_view(|ctx| {
            EditorView::single_line(SingleLineEditorOptions::default(), ctx)
        });
        let name_editor = ctx.add_typed_action_view(|ctx| {
            EditorView::single_line(SingleLineEditorOptions::default(), ctx)
        });
        let branch_name_editor = ctx.add_typed_action_view(|ctx| {
            EditorView::single_line(SingleLineEditorOptions::default(), ctx)
        });

        // Trigger re-render when text fields edit so the validation message and
        // submit-button enablement react in real time.
        // Worktree name: also mirrors into the branch name field unless the user
        // has manually overridden it.
        ctx.subscribe_to_view(&name_editor, |me, editor, event, ctx| {
            if matches!(event, EditorEvent::Edited(_)) {
                if !me.branch_name_overridden {
                    let new_name = editor.read(ctx, |e, app| e.buffer_text(app).to_string());
                    me.last_programmatic_branch_value = new_name.clone();
                    me.branch_name_editor.update(ctx, |b, ctx| {
                        b.set_buffer_text(&new_name, ctx);
                    });
                }
                ctx.notify();
            }
        });
        ctx.subscribe_to_view(&destination_editor, |_, _, event, ctx| {
            if matches!(event, EditorEvent::Edited(_)) {
                ctx.notify();
            }
        });
        // Branch name: mark as overridden whenever the user edits it to a value
        // different from our last programmatic mirror update.
        ctx.subscribe_to_view(&branch_name_editor, |me, editor, event, ctx| {
            if matches!(event, EditorEvent::Edited(_)) {
                let current = editor.read(ctx, |e, app| e.buffer_text(app).to_string());
                if current != me.last_programmatic_branch_value {
                    me.branch_name_overridden = true;
                }
                ctx.notify();
            }
        });

        Self {
            branch_picker,
            destination_editor,
            name_editor,
            branch_name_editor,
            branch_name_overridden: false,
            last_programmatic_branch_value: String::new(),
            cancel_button_mouse_state: Default::default(),
            create_button_mouse_state: Default::default(),
            cwd: None,
            seed: None,
        }
    }

    pub fn on_open(&mut self, seed: CreateWorktreeModalSeed, ctx: &mut ViewContext<Self>) {
        self.cwd = seed.current_worktree_path.clone();
        self.seed = Some(seed);
        self.refresh_branch_picker(ctx);
        self.update_default_destination(ctx);
        self.name_editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text("", ctx);
        });
        // Reset branch name + override flag so a fresh dialog session starts the
        // mirror behavior from scratch.
        self.branch_name_overridden = false;
        self.last_programmatic_branch_value = String::new();
        self.branch_name_editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text("", ctx);
        });
    }

    fn refresh_branch_picker(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(cwd) = self.cwd.clone() {
            self.branch_picker.update(ctx, |picker, ctx| {
                picker.refetch_branches(cwd, ctx);
            });
        }
    }

    fn update_default_destination(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(seed) = &self.seed else { return };
        let dest = seed
            .default_destination_dir
            .display()
            .to_string();
        self.destination_editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text(&dest, ctx);
        });
    }

    fn current_name(&self, app: &AppContext) -> String {
        self.name_editor
            .read(app, |editor, app| editor.buffer_text(app).to_string())
    }

    fn current_destination(&self, app: &AppContext) -> String {
        self.destination_editor
            .read(app, |editor, app| editor.buffer_text(app).to_string())
    }

    fn current_branch_name(&self, app: &AppContext) -> String {
        self.branch_name_editor
            .read(app, |editor, app| editor.buffer_text(app).to_string())
    }

    fn try_submit(&mut self, ctx: &mut ViewContext<Self>) {
        let name = self.current_name(ctx);
        if !is_valid_worktree_name(&name) {
            return;
        }
        // Branch name defaults to the worktree name when not overridden; if the
        // user overrode and emptied it, fall back to worktree name silently.
        let branch_name = {
            let typed = self.current_branch_name(ctx).trim().to_string();
            if typed.is_empty() {
                name.trim().to_string()
            } else {
                typed
            }
        };
        let Some(cwd) = self.cwd.clone() else {
            return;
        };
        let Some(source_branch) = self
            .branch_picker
            .read(ctx, |p, ctx| p.selected_value(ctx))
        else {
            return;
        };
        let dest_str = self.current_destination(ctx);
        if dest_str.trim().is_empty() {
            return;
        }
        let mut destination = PathBuf::from(dest_str.trim());
        destination.push(name.trim());

        ctx.emit(CreateWorktreeModalEvent::Submit {
            source_worktree: cwd,
            branch: source_branch,
            destination,
            worktree_name: branch_name,
        });
    }
}

impl Entity for CreateWorktreeModal {
    type Event = CreateWorktreeModalEvent;
}

impl TypedActionView for CreateWorktreeModal {
    type Action = CreateWorktreeModalAction;

    fn handle_action(
        &mut self,
        action: &CreateWorktreeModalAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            CreateWorktreeModalAction::Cancel | CreateWorktreeModalAction::Escape => {
                ctx.emit(CreateWorktreeModalEvent::Close);
            }
            CreateWorktreeModalAction::Create => {
                self.try_submit(ctx);
            }
        }
    }
}

impl View for CreateWorktreeModal {
    fn ui_name() -> &'static str {
        "CreateWorktreeModal"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let ui_font_family = appearance.ui_font_family();
        let label_color = theme.sub_text_color(theme.surface_2()).into_solid();

        let label = |text: &'static str| {
            Container::new(
                Text::new_inline(text, ui_font_family, appearance.ui_font_size())
                    .with_color(warp_core::ui::theme::Fill::Solid(label_color).into())
                    .with_style(Properties::default().weight(Weight::Semibold))
                    .finish(),
            )
            .with_margin_bottom(LABEL_BOTTOM_MARGIN)
            .finish()
        };

        let name = self.current_name(app);
        let name_valid = is_valid_worktree_name(&name);
        // Show a validation hint whenever the field is invalid OR empty so the user
        // gets feedback before clicking Create — silent button is bad UX.
        let validation_message: Option<Box<dyn Element>> = if name.trim().is_empty() {
            Some(
                Container::new(
                    Text::new_inline(
                        "Worktree name is required.",
                        ui_font_family,
                        ERROR_FONT_SIZE,
                    )
                    .with_color(theme.ui_error_color())
                    .finish(),
                )
                .with_margin_top(4.)
                .finish(),
            )
        } else if !name_valid {
            Some(
                Container::new(
                    Text::new_inline(INVALID_NAME_ERROR, ui_font_family, ERROR_FONT_SIZE)
                        .with_color(theme.ui_error_color())
                        .finish(),
                )
                .with_margin_top(4.)
                .finish(),
            )
        } else {
            None
        };

        // Live preview of the final worktree path. Combines the destination directory
        // (from the Destination editor) with the worktree name as `<dest>/<name>`.
        // Empty when name is empty so the user knows nothing will be created yet.
        let dest_input = self.current_destination(app);
        let preview_path = if name.trim().is_empty() {
            String::new()
        } else {
            let mut p = std::path::PathBuf::from(dest_input.trim());
            p.push(name.trim());
            p.display().to_string()
        };
        let preview_element: Box<dyn Element> = if preview_path.is_empty() {
            Empty::new().finish()
        } else {
            Container::new(
                Text::new_inline(preview_path, ui_font_family, appearance.ui_font_size() - 1.)
                    .with_color(
                        warp_core::ui::theme::Fill::Solid(label_color).into(),
                    )
                    .finish(),
            )
            .with_margin_top(LABEL_BOTTOM_MARGIN)
            .finish()
        };

        let body = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(label("Source branch"))
            .with_child(ChildView::new(&self.branch_picker).finish())
            .with_child(
                Container::new(label("Destination directory"))
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            )
            .with_child(input_frame(
                ChildView::new(&self.destination_editor).finish(),
                theme,
            ))
            .with_child(
                Container::new(label("Worktree name for feature"))
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            )
            .with_child(input_frame(
                ChildView::new(&self.name_editor).finish(),
                theme,
            ))
            .with_child(
                Container::new(label("Branch name"))
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            )
            .with_child(input_frame(
                ChildView::new(&self.branch_name_editor).finish(),
                theme,
            ))
            .with_child(preview_element);
        let body = match validation_message {
            Some(msg) => body.with_child(msg).finish(),
            None => body.finish(),
        };

        let header_text = Text::new_inline(
            "Create new worktree",
            ui_font_family,
            HEADER_TITLE_FONT_SIZE,
        )
        .with_style(Properties::default().weight(Weight::Bold))
        .with_color(
            warp_core::ui::theme::Fill::Solid(theme.main_text_color(theme.surface_2()).into_solid())
                .into(),
        )
        .finish();
        let header = Container::new(header_text)
            .with_padding(
                Padding::uniform(0.)
                    .with_top(HEADER_PADDING_TOP)
                    .with_bottom(HEADER_PADDING_BOTTOM)
                    .with_left(CONTENT_HORIZONTAL_PADDING)
                    .with_right(CONTENT_HORIZONTAL_PADDING),
            )
            .finish();

        let body_container = Container::new(body)
            .with_padding(
                Padding::uniform(0.)
                    .with_bottom(BODY_BOTTOM_PADDING)
                    .with_left(CONTENT_HORIZONTAL_PADDING)
                    .with_right(CONTENT_HORIZONTAL_PADDING),
            )
            .finish();

        let cancel_button = footer_button(
            "Cancel",
            ButtonVariant::Secondary,
            self.cancel_button_mouse_state.clone(),
            theme,
            ui_font_family,
            appearance.ui_font_size(),
            CreateWorktreeModalAction::Cancel,
        );
        // Disable Create when name is invalid: render in subdued style and skip
        // the click handler so the button visibly communicates "fix the form first".
        let create_enabled = name_valid && !name.trim().is_empty();
        let create_button = footer_button_maybe_disabled(
            "Create",
            ButtonVariant::Accent,
            self.create_button_mouse_state.clone(),
            theme,
            ui_font_family,
            appearance.ui_font_size(),
            CreateWorktreeModalAction::Create,
            create_enabled,
        );

        let footer = Container::new(
            Flex::row()
                .with_main_axis_alignment(MainAxisAlignment::End)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(cancel_button)
                .with_child(
                    Container::new(create_button)
                        .with_margin_left(FOOTER_BUTTON_GAP)
                        .finish(),
                )
                .finish(),
        )
        .with_padding(
            Padding::uniform(0.)
                .with_vertical(FOOTER_VERTICAL_PADDING)
                .with_left(CONTENT_HORIZONTAL_PADDING)
                .with_right(CONTENT_HORIZONTAL_PADDING),
        )
        .with_border(Border::top(1.).with_border_fill(internal_colors::neutral_4(theme)))
        .finish();

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(header)
                .with_child(body_container)
                .with_child(footer)
                .finish(),
        )
        .with_background_color(theme.surface_2().into())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
    }
}

/// Wrap a single-line `EditorView` in a styled container so it visually matches
/// the dropdowns above (border, background, padding, rounded corners). Without
/// this the editor renders as bare text with no visible boundary.
fn input_frame(
    editor_view: Box<dyn Element>,
    theme: &warp_core::ui::theme::WarpTheme,
) -> Box<dyn Element> {
    Container::new(editor_view)
        .with_horizontal_padding(10.)
        .with_vertical_padding(8.)
        .with_background_color(theme.surface_3().into())
        .with_border(Border::all(1.).with_border_fill(internal_colors::neutral_4(theme)))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
}

fn footer_button(
    label_text: &str,
    variant: ButtonVariant,
    mouse_state: MouseStateHandle,
    theme: &warp_core::ui::theme::WarpTheme,
    font_family: warpui::fonts::FamilyId,
    font_size: f32,
    action: CreateWorktreeModalAction,
) -> Box<dyn Element> {
    footer_button_maybe_disabled(
        label_text,
        variant,
        mouse_state,
        theme,
        font_family,
        font_size,
        action,
        true,
    )
}

fn footer_button_maybe_disabled(
    label_text: &str,
    variant: ButtonVariant,
    mouse_state: MouseStateHandle,
    theme: &warp_core::ui::theme::WarpTheme,
    font_family: warpui::fonts::FamilyId,
    font_size: f32,
    action: CreateWorktreeModalAction,
    enabled: bool,
) -> Box<dyn Element> {
    use warpui::elements::Hoverable;
    let bg = if enabled {
        match variant {
            ButtonVariant::Accent => theme.accent(),
            _ => theme.surface_2(),
        }
    } else {
        // Disabled: subdued background regardless of variant.
        internal_colors::neutral_4(theme).into()
    };
    let text_color = if enabled {
        theme.main_text_color(bg).into_solid()
    } else {
        theme.sub_text_color(theme.surface_2()).into_solid()
    };
    let border_fill = internal_colors::neutral_4(theme);
    let label_owned = label_text.to_string();
    let hover = Hoverable::new(mouse_state, move |_| {
        ConstrainedBox::new(
            Container::new(
                Text::new_inline(label_owned.clone(), font_family, font_size)
                    .with_color(warp_core::ui::theme::Fill::Solid(text_color).into())
                    .finish(),
            )
            .with_horizontal_padding(FOOTER_BUTTON_HORIZONTAL_PADDING)
            .with_background(bg)
            .with_border(Border::all(1.).with_border_fill(border_fill))
            .with_corner_radius(CornerRadius::with_all(FOOTER_BUTTON_RADIUS))
            .finish(),
        )
        .with_height(FOOTER_BUTTON_HEIGHT)
        .finish()
    });
    if enabled {
        hover
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(action);
            })
            .with_cursor(Cursor::PointingHand)
            .finish()
    } else {
        // No on_click → button is visually present but does nothing on click.
        hover.finish()
    }
}
