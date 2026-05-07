use std::path::PathBuf;

use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::Fill;
use warpui::{
    elements::{
        Align, ChildAnchor, ChildView, Container, MouseStateHandle, OffsetPositioning,
        ParentAnchor, ParentOffsetBounds, Stack,
    },
    keymap::{FixedBinding, Keystroke},
    platform::Cursor,
    ui_components::{
        components::{UiComponent, UiComponentStyles},
        text::Span,
    },
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::{
    appearance::Appearance,
    ui_components::dialog::{dialog_styles, Dialog},
    view_components::action_button::{
        ActionButton, DangerPrimaryTheme, KeystrokeSource, NakedTheme,
    },
};

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([
        FixedBinding::new(
            "escape",
            RemoveWorktreeConfirmationAction::Cancel,
            id!(RemoveWorktreeConfirmationDialog::ui_name()),
        ),
        FixedBinding::new(
            "enter",
            RemoveWorktreeConfirmationAction::Confirm,
            id!(RemoveWorktreeConfirmationDialog::ui_name()),
        ),
    ]);
}

const DIALOG_WIDTH: f32 = 480.;

#[derive(Clone, Debug, Default)]
pub struct WorktreeDirtyStatus {
    /// Files modified or staged.
    pub has_uncommitted_changes: bool,
    /// Files not yet tracked by git.
    pub has_untracked_files: bool,
    /// Local commits not yet pushed to upstream.
    pub has_unpushed_commits: bool,
}

impl WorktreeDirtyStatus {
    pub fn is_clean(&self) -> bool {
        !self.has_uncommitted_changes
            && !self.has_untracked_files
            && !self.has_unpushed_commits
    }

    fn warning_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.has_uncommitted_changes {
            out.push("Uncommitted changes will be lost.".to_string());
        }
        if self.has_untracked_files {
            out.push("Untracked files will be lost.".to_string());
        }
        if self.has_unpushed_commits {
            out.push("Local commits not yet pushed to remote will be lost.".to_string());
        }
        out
    }
}

#[derive(Clone)]
pub struct RemoveWorktreeDialogSource {
    pub path: PathBuf,
    pub worktree_name: String,
    /// The branch this worktree currently has checked out, when known. Used by the
    /// "Also delete branch" checkbox in the dialog so the user can wipe the branch
    /// at the same time they wipe the worktree (`git branch -D <branch>`).
    pub branch_name: Option<String>,
    /// The shared repo root (parent of `.git` common dir). Required to run
    /// `git branch -D` after the worktree itself is gone — running it from the
    /// removed worktree's parent dir fails because that path is not a git repo.
    pub repo_root: Option<PathBuf>,
    /// Names of tabs that will close after removal (their CWD is under the worktree path).
    pub tabs_to_close: Vec<String>,
    pub dirty_status: WorktreeDirtyStatus,
}

pub struct RemoveWorktreeConfirmationDialog {
    cancel_button: ViewHandle<ActionButton>,
    remove_button: ViewHandle<ActionButton>,
    source: Option<RemoveWorktreeDialogSource>,
    /// Whether the "Also delete branch '<name>'" checkbox is checked.
    also_delete_branch: bool,
    also_delete_branch_mouse_state: MouseStateHandle,
}

impl RemoveWorktreeConfirmationDialog {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let cancel_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Cancel", NakedTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(RemoveWorktreeConfirmationAction::Cancel);
            })
        });

        let enter_keystroke = Keystroke::parse("enter").expect("Valid keystroke");
        let remove_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new("Remove", DangerPrimaryTheme)
                .with_keybinding(KeystrokeSource::Fixed(enter_keystroke), ctx)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(RemoveWorktreeConfirmationAction::Confirm);
                })
        });

        Self {
            cancel_button,
            remove_button,
            source: None,
            also_delete_branch: true,
            also_delete_branch_mouse_state: Default::default(),
        }
    }

    pub fn set_source(&mut self, source: RemoveWorktreeDialogSource) {
        self.source = Some(source);
        // Default the "Also delete branch" checkbox to ON: the typical flow is to
        // delete the branch alongside the worktree (otherwise the branch lingers as
        // dead state). User can uncheck it to keep the branch.
        self.also_delete_branch = true;
    }

    fn build_body(&self) -> String {
        let Some(source) = self.source.as_ref() else {
            return "This worktree will be removed.".to_string();
        };

        let mut sections: Vec<String> = Vec::new();
        sections.push(format!("Worktree: {}", source.worktree_name));
        sections.push(format!("Path: {}", source.path.display()));
        if let Some(branch) = source.branch_name.as_deref() {
            sections.push(format!("Branch: {branch}"));
        }

        if !source.tabs_to_close.is_empty() {
            let count = source.tabs_to_close.len();
            let label = if count == 1 { "tab" } else { "tabs" };
            sections.push(format!(
                "{count} open {label} will close: {}",
                source.tabs_to_close.join(", ")
            ));
        }

        let warnings = source.dirty_status.warning_lines();
        if !warnings.is_empty() {
            sections.push(format!("⚠  {}", warnings.join(" ")));
        } else {
            sections.push("Worktree is clean.".to_string());
        }

        sections.join("\n\n")
    }
}

impl Entity for RemoveWorktreeConfirmationDialog {
    type Event = RemoveWorktreeConfirmationEvent;
}

impl View for RemoveWorktreeConfirmationDialog {
    fn ui_name() -> &'static str {
        "RemoveWorktreeConfirmationDialog"
    }

    fn on_focus(&mut self, _focus_ctx: &warpui::FocusContext, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let cancel_button = Container::new(ChildView::new(&self.cancel_button).finish())
            .with_margin_right(12.)
            .finish();

        let title = self
            .source
            .as_ref()
            .map(|s| format!("Remove worktree '{}'?", s.worktree_name))
            .unwrap_or_else(|| "Remove worktree?".into());

        // "Also delete branch '<name>'" checkbox in the footer-left slot. Only
        // rendered when we know the branch name; if the branch is detached or the
        // porcelain didn't surface it, we skip the option entirely.
        let branch_name_opt = self
            .source
            .as_ref()
            .and_then(|s| s.branch_name.clone());
        let dialog_base_styles = dialog_styles(appearance);
        // Override checkbox label color: by default the checkbox builder uses
        // `nonactive_ui_text_color` for the label, which renders dimmed on the
        // dialog's dark surface. We apply the dialog's main text color instead so
        // the label reads at the same contrast as the body text above.
        let checkbox_label_override = UiComponentStyles {
            font_color: dialog_base_styles.font_color,
            font_family_id: dialog_base_styles.font_family_id,
            font_size: Some(13.),
            font_weight: Some(warpui::fonts::Weight::Thin),
            ..Default::default()
        };
        let also_delete_checkbox: Option<Box<dyn Element>> = branch_name_opt.map(|branch| {
            let label = format!("Also delete branch '{branch}'");
            appearance
                .ui_builder()
                .checkbox(self.also_delete_branch_mouse_state.clone(), Some(14.))
                .with_style(checkbox_label_override)
                .with_label(Span::new(label, Default::default()))
                .check(self.also_delete_branch)
                .build()
                .with_cursor(Cursor::PointingHand)
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(
                        RemoveWorktreeConfirmationAction::ToggleAlsoDeleteBranch,
                    );
                })
                .finish()
        });

        let mut dialog_builder = Dialog::new(
            title,
            Some(self.build_body()),
            UiComponentStyles {
                width: Some(DIALOG_WIDTH),
                ..dialog_styles(appearance)
            },
        )
        .with_bottom_row_child(cancel_button)
        .with_bottom_row_child(ChildView::new(&self.remove_button).finish());
        if let Some(checkbox) = also_delete_checkbox {
            dialog_builder = dialog_builder.with_bottom_row_left_child(checkbox);
        }
        let dialog = dialog_builder.build().finish();

        let mut stack = Stack::new();
        stack.add_positioned_child(
            dialog,
            OffsetPositioning::offset_from_parent(
                vec2f(0., 0.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::Center,
                ChildAnchor::Center,
            ),
        );

        Container::new(Align::new(stack.finish()).finish())
            .with_background_color(Fill::blur().into())
            .with_corner_radius(app.windows().window_corner_radius())
            .finish()
    }
}

pub enum RemoveWorktreeConfirmationEvent {
    Confirm {
        source: RemoveWorktreeDialogSource,
        also_delete_branch: bool,
    },
    Cancel,
}

#[derive(Debug)]
pub enum RemoveWorktreeConfirmationAction {
    Confirm,
    Cancel,
    ToggleAlsoDeleteBranch,
}

impl TypedActionView for RemoveWorktreeConfirmationDialog {
    type Action = RemoveWorktreeConfirmationAction;

    fn handle_action(
        &mut self,
        action: &RemoveWorktreeConfirmationAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            RemoveWorktreeConfirmationAction::Confirm => {
                let Some(source) = self.source.clone() else {
                    log::error!("Remove worktree confirm pressed with no source");
                    return;
                };
                ctx.emit(RemoveWorktreeConfirmationEvent::Confirm {
                    source,
                    also_delete_branch: self.also_delete_branch,
                });
            }
            RemoveWorktreeConfirmationAction::Cancel => {
                ctx.emit(RemoveWorktreeConfirmationEvent::Cancel);
            }
            RemoveWorktreeConfirmationAction::ToggleAlsoDeleteBranch => {
                self.also_delete_branch = !self.also_delete_branch;
                ctx.notify();
            }
        }
    }
}
