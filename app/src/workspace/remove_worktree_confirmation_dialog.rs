use std::path::PathBuf;

use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::Fill;
use warpui::{
    elements::{
        Align, ChildAnchor, ChildView, Container, OffsetPositioning, ParentAnchor,
        ParentOffsetBounds, Stack,
    },
    keymap::{FixedBinding, Keystroke},
    ui_components::components::{UiComponent, UiComponentStyles},
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
    /// Names of tabs that will close after removal (their CWD is under the worktree path).
    pub tabs_to_close: Vec<String>,
    pub dirty_status: WorktreeDirtyStatus,
}

pub struct RemoveWorktreeConfirmationDialog {
    cancel_button: ViewHandle<ActionButton>,
    remove_button: ViewHandle<ActionButton>,
    source: Option<RemoveWorktreeDialogSource>,
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
        }
    }

    pub fn set_source(&mut self, source: RemoveWorktreeDialogSource) {
        self.source = Some(source);
    }

    fn build_body(&self) -> String {
        let Some(source) = self.source.as_ref() else {
            return "This worktree will be removed.".to_string();
        };

        let mut sections: Vec<String> = Vec::new();
        sections.push(format!("Path: {}", source.path.display()));

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

        let dialog = Dialog::new(
            title,
            Some(self.build_body()),
            UiComponentStyles {
                width: Some(DIALOG_WIDTH),
                ..dialog_styles(appearance)
            },
        )
        .with_bottom_row_child(cancel_button)
        .with_bottom_row_child(ChildView::new(&self.remove_button).finish())
        .build()
        .finish();

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
    },
    Cancel,
}

#[derive(Debug)]
pub enum RemoveWorktreeConfirmationAction {
    Confirm,
    Cancel,
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
                ctx.emit(RemoveWorktreeConfirmationEvent::Confirm { source });
            }
            RemoveWorktreeConfirmationAction::Cancel => {
                ctx.emit(RemoveWorktreeConfirmationEvent::Cancel);
            }
        }
    }
}
