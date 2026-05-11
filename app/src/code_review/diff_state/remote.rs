//! Remote diff state model (stub).
//!
//! This is a no-op placeholder with the same read interface as
//! [`LocalDiffStateModel`]. A real implementation will be added when the
//! remote client ↔ server sync layer supports git diff data.

use warpui::ModelContext;

use crate::util::git::{Commit, PrInfo};

use super::{DiffMode, DiffState, DiffStateModelEvent, DiffStats};

pub struct RemoteDiffStateModel;

impl RemoteDiffStateModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }
}

impl warpui::Entity for RemoteDiffStateModel {
    type Event = DiffStateModelEvent;
}

/// Read methods matching the same interface as `LocalDiffStateModel`.
/// All return no-op/default values since the remote implementation is a stub.
impl RemoteDiffStateModel {
    pub fn get(&self) -> DiffState {
        DiffState::NotInRepository
    }

    pub fn diff_mode(&self) -> DiffMode {
        DiffMode::default()
    }

    pub fn get_uncommitted_stats(&self) -> Option<DiffStats> {
        None
    }

    pub fn get_main_branch_name(&self) -> Option<String> {
        None
    }

    pub fn get_current_branch_name(&self) -> Option<String> {
        None
    }

    pub fn is_on_main_branch(&self) -> bool {
        false
    }

    pub fn unpushed_commits(&self) -> &[Commit] {
        &[]
    }

    pub fn upstream_ref(&self) -> Option<&str> {
        None
    }

    pub fn upstream_differs_from_main(&self) -> bool {
        false
    }

    pub fn pr_info(&self) -> Option<&PrInfo> {
        None
    }

    pub fn is_pr_info_refreshing(&self) -> bool {
        false
    }

    pub fn is_git_operation_blocked(&self, _ctx: &warpui::AppContext) -> bool {
        false
    }

    pub fn has_head(&self) -> bool {
        false
    }
}
