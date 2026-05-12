//! Unified diff state module.
//!
//! [`DiffStateModel`] is an enum that provides a unified API over local and remote models.
//! It holds one of [`LocalDiffStateModel`] or [`RemoteDiffStateModel`] and dispatches
//! operations to whichever is active.
//! All consumers should use `DiffStateModel` rather than accessing sub-models directly.

use crate::code::buffer_location::FileLocation;
use crate::util::git::{Commit, PrInfo};
use warpui::{AppContext, ModelContext, ModelHandle};

// Re-export everything from the local model so existing `use
// crate::code_review::diff_state::{…}` paths continue to work.
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
mod local;
pub use local::*;

mod remote;
pub use remote::RemoteDiffStateModel;

// ── Unified model ────────────────────────────────────────────────────────

/// Unified diff state model that dispatches to a local or remote backend.
///
/// Only one variant is populated at a time, since a diff state belongs to
/// exactly one repository (either local or remote). All consumers should
/// interact with this enum rather than accessing sub-models directly.
pub enum DiffStateModel {
    Local(ModelHandle<LocalDiffStateModel>),
    Remote(ModelHandle<RemoteDiffStateModel>),
}

impl warpui::Entity for DiffStateModel {
    type Event = DiffStateModelEvent;
}

impl DiffStateModel {
    // ── Construction ─────────────────────────────────────────────────

    pub fn new(key: FileLocation, ctx: &mut ModelContext<Self>) -> Self {
        match key {
            FileLocation::Local(path) => {
                let repo_path = Some(path.display().to_string());
                let local = ctx.add_model(|ctx| LocalDiffStateModel::new(repo_path, ctx));
                ctx.subscribe_to_model(&local, Self::forward_event);
                Self::Local(local)
            }
            FileLocation::Remote(_remote_id) => {
                let remote = ctx.add_model(RemoteDiffStateModel::new);
                ctx.subscribe_to_model(&remote, Self::forward_event);
                Self::Remote(remote)
            }
        }
    }

    // ── Event forwarding ─────────────────────────────────────────────

    fn forward_event(&mut self, event: &DiffStateModelEvent, ctx: &mut ModelContext<Self>) {
        match event {
            DiffStateModelEvent::CurrentBranchChanged => {
                ctx.emit(DiffStateModelEvent::CurrentBranchChanged);
            }
            DiffStateModelEvent::NewDiffsComputed(diffs) => {
                ctx.emit(DiffStateModelEvent::NewDiffsComputed(diffs.clone()));
            }
            DiffStateModelEvent::SingleFileUpdated { path, diff } => {
                ctx.emit(DiffStateModelEvent::SingleFileUpdated {
                    path: path.clone(),
                    diff: diff.clone(),
                });
            }
            DiffStateModelEvent::MetadataRefreshed(metadata) => {
                ctx.emit(DiffStateModelEvent::MetadataRefreshed(metadata.clone()));
            }
        }
    }

    // ── Unified read API ─────────────────────────────────────────────

    pub(crate) fn get(&self, ctx: &AppContext) -> DiffState {
        match self {
            Self::Local(m) => m.as_ref(ctx).get(),
            Self::Remote(m) => m.as_ref(ctx).get(),
        }
    }

    pub(crate) fn diff_mode(&self, ctx: &AppContext) -> DiffMode {
        match self {
            Self::Local(m) => m.as_ref(ctx).diff_mode(),
            Self::Remote(m) => m.as_ref(ctx).diff_mode(),
        }
    }

    pub(crate) fn get_uncommitted_stats(&self, ctx: &AppContext) -> Option<DiffStats> {
        match self {
            Self::Local(m) => m.as_ref(ctx).get_uncommitted_stats(),
            Self::Remote(m) => m.as_ref(ctx).get_uncommitted_stats(),
        }
    }

    pub(crate) fn get_main_branch_name(&self, ctx: &AppContext) -> Option<String> {
        match self {
            Self::Local(m) => m.as_ref(ctx).get_main_branch_name(),
            Self::Remote(m) => m.as_ref(ctx).get_main_branch_name(),
        }
    }

    pub fn get_current_branch_name(&self, ctx: &AppContext) -> Option<String> {
        match self {
            Self::Local(m) => m.as_ref(ctx).get_current_branch_name(),
            Self::Remote(m) => m.as_ref(ctx).get_current_branch_name(),
        }
    }

    pub(crate) fn is_on_main_branch(&self, ctx: &AppContext) -> bool {
        match self {
            Self::Local(m) => m.as_ref(ctx).is_on_main_branch(),
            Self::Remote(m) => m.as_ref(ctx).is_on_main_branch(),
        }
    }

    pub(crate) fn unpushed_commits<'a>(&self, ctx: &'a AppContext) -> &'a [Commit] {
        match self {
            Self::Local(m) => m.as_ref(ctx).unpushed_commits(),
            Self::Remote(m) => m.as_ref(ctx).unpushed_commits(),
        }
    }

    pub(crate) fn upstream_ref<'a>(&self, ctx: &'a AppContext) -> Option<&'a str> {
        match self {
            Self::Local(m) => m.as_ref(ctx).upstream_ref(),
            Self::Remote(m) => m.as_ref(ctx).upstream_ref(),
        }
    }

    pub(crate) fn upstream_differs_from_main(&self, ctx: &AppContext) -> bool {
        match self {
            Self::Local(m) => m.as_ref(ctx).upstream_differs_from_main(),
            Self::Remote(m) => m.as_ref(ctx).upstream_differs_from_main(),
        }
    }

    pub(crate) fn pr_info<'a>(&self, ctx: &'a AppContext) -> Option<&'a PrInfo> {
        match self {
            Self::Local(m) => m.as_ref(ctx).pr_info(),
            Self::Remote(m) => m.as_ref(ctx).pr_info(),
        }
    }

    pub(crate) fn is_pr_info_refreshing(&self, ctx: &AppContext) -> bool {
        match self {
            Self::Local(m) => m.as_ref(ctx).is_pr_info_refreshing(),
            Self::Remote(m) => m.as_ref(ctx).is_pr_info_refreshing(),
        }
    }

    pub(crate) fn is_git_operation_blocked(&self, ctx: &AppContext) -> bool {
        match self {
            Self::Local(m) => m.as_ref(ctx).is_git_operation_blocked(ctx),
            Self::Remote(m) => m.as_ref(ctx).is_git_operation_blocked(ctx),
        }
    }

    pub(crate) fn has_head(&self, ctx: &AppContext) -> bool {
        match self {
            Self::Local(m) => m.as_ref(ctx).has_head(),
            Self::Remote(m) => m.as_ref(ctx).has_head(),
        }
    }

    // ── Unified write API ────────────────────────────────────────────

    pub(crate) fn set_diff_mode(
        &self,
        mode: DiffMode,
        should_fetch_base: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        match self {
            Self::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.set_diff_mode(mode, should_fetch_base, ctx);
                });
            }
            Self::Remote(_) => {}
        }
    }

    pub(crate) fn set_diff_mode_and_fetch_base(
        &self,
        mode: DiffMode,
        ctx: &mut ModelContext<Self>,
    ) {
        match self {
            Self::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.set_diff_mode_and_fetch_base(mode, ctx);
                });
            }
            Self::Remote(_) => {}
        }
    }

    pub(crate) fn load_diffs_for_current_repo(
        &self,
        should_fetch_base: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        match self {
            Self::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.load_diffs_for_current_repo(should_fetch_base, ctx);
                });
            }
            Self::Remote(_) => {}
        }
    }

    pub(crate) fn set_code_review_metadata_refresh_enabled(
        &self,
        enabled: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        match self {
            Self::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.set_code_review_metadata_refresh_enabled(enabled, ctx);
                });
            }
            Self::Remote(_) => {}
        }
    }

    pub(crate) fn refresh_metadata_and_pr_info(&self, ctx: &mut ModelContext<Self>) {
        match self {
            Self::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.refresh_metadata_and_pr_info(ctx);
                });
            }
            Self::Remote(_) => {}
        }
    }

    pub(crate) fn discard_files(
        &self,
        file_infos: Vec<FileStatusInfo>,
        should_stash: bool,
        branch_name: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        match self {
            Self::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.discard_files(file_infos, should_stash, branch_name, ctx);
                });
            }
            Self::Remote(_) => {}
        }
    }

    #[cfg(feature = "local_fs")]
    pub(crate) fn stop_active_watcher(&self, ctx: &mut ModelContext<Self>) {
        match self {
            Self::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.stop_active_watcher(ctx);
                });
            }
            Self::Remote(_) => {}
        }
    }
}

#[cfg(test)]
impl DiffStateModel {
    /// Test-only constructor that creates a local-backend model without a
    /// repository. All existing tests exercise local behavior; add a
    /// `new_for_test_remote` variant when remote-backend tests are needed.
    pub fn new_for_test(ctx: &mut ModelContext<Self>) -> Self {
        let local = ctx.add_model(LocalDiffStateModel::new_for_test);
        ctx.subscribe_to_model(&local, Self::forward_event);
        Self::Local(local)
    }
}
