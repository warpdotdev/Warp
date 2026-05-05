use std::path::PathBuf;

use warpui::{
    elements::ChildView, AppContext, Element, Entity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::{
    tab_configs::PickerStyle,
    view_components::{DropdownItem, FilterableDropdown},
};

const DEFAULT_DROPDOWN_WIDTH: f32 = 380.;

/// A worktree entry that can be shown in the picker. The `display_name` is what the
/// user sees (e.g. `GHICRM_feature-wt`); `path` is the absolute filesystem path used
/// as the value when an item is selected; `is_main` flags the root worktree, which
/// gets a `(root)` suffix in its label.
#[derive(Clone, Debug)]
pub struct WorktreePickerEntry {
    pub display_name: String,
    pub path: PathBuf,
    pub is_main: bool,
}

/// Filterable dropdown listing the worktrees of a single repo. Used by the
/// `NewWorktreeModal` when triggered from the worktrees chip — replaces the
/// general-purpose `RepoPicker` (which lists all known repos in the workspace).
pub struct WorktreePicker {
    dropdown: ViewHandle<FilterableDropdown<WorktreePickerAction>>,
    selected: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorktreePickerAction {
    Select(String),
}

pub enum WorktreePickerEvent {
    Selected(PathBuf),
}

impl WorktreePicker {
    pub fn new(
        entries: Vec<WorktreePickerEntry>,
        default_selected: Option<PathBuf>,
        style: Option<PickerStyle>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let width = style.as_ref().map_or(DEFAULT_DROPDOWN_WIDTH, |s| s.width);
        let dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = FilterableDropdown::new(ctx);
            dropdown.set_top_bar_max_width(width);
            dropdown.set_menu_width(width, ctx);
            dropdown
        });

        let mut picker = Self {
            dropdown,
            selected: None,
        };

        picker.set_entries(entries, default_selected, ctx);
        picker
    }

    pub fn set_entries(
        &mut self,
        entries: Vec<WorktreePickerEntry>,
        default_selected: Option<PathBuf>,
        ctx: &mut ViewContext<Self>,
    ) {
        let items: Vec<DropdownItem<WorktreePickerAction>> = entries
            .iter()
            .map(|e| {
                let label = if e.is_main {
                    format!("{} (root)", e.display_name)
                } else {
                    e.display_name.clone()
                };
                let path_str = e.path.to_string_lossy().into_owned();
                DropdownItem::new(label, WorktreePickerAction::Select(path_str))
            })
            .collect();

        let label_to_select = default_selected.as_ref().and_then(|wanted| {
            entries.iter().find(|e| &e.path == wanted).map(|e| {
                if e.is_main {
                    format!("{} (root)", e.display_name)
                } else {
                    e.display_name.clone()
                }
            })
        });

        self.dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_items(items, ctx);
            if let Some(label) = label_to_select.as_deref() {
                dropdown.set_selected_by_name(label, ctx);
            }
        });
        self.selected = default_selected;
        ctx.notify();
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.selected.clone()
    }
}

impl Entity for WorktreePicker {
    type Event = WorktreePickerEvent;
}

impl View for WorktreePicker {
    fn ui_name() -> &'static str {
        "WorktreePicker"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        ChildView::new(&self.dropdown).finish()
    }
}

impl TypedActionView for WorktreePicker {
    type Action = WorktreePickerAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            WorktreePickerAction::Select(path_str) => {
                let path = PathBuf::from(path_str);
                self.selected = Some(path.clone());
                ctx.emit(WorktreePickerEvent::Selected(path));
            }
        }
    }
}
