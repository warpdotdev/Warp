//! Unified diff state wrapper module.
//!
//! [`DiffStateModel`] is a model that provides a unified API over local and remote models.
//! It holds one of [`LocalDiffStateModel`] or [`RemoteDiffStateModel`] behind a
//! [`DiffStateBackend`] enum and dispatches operations to whichever is active.
//! All consumers should use `DiffStateModel` rather than accessing sub-models directly.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::util::git::{Commit, PrInfo};
use repo_metadata::repository_identifier::RemoteRepositoryIdentifier;
use warpui::{AppContext, ModelContext, ModelHandle};

// Re-export everything from the local model so existing `use
// crate::code_review::diff_state::{…}` paths continue to work.
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
mod local;
pub use local::*;

mod remote;
pub use remote::RemoteDiffStateModel;

// ── Enums ────────────────────────────────────────────────────────────────

/// The active backend — only one variant is populated at a time, since a diff
/// state belongs to exactly one repository (either local or remote).
enum DiffStateBackend {
    Local(ModelHandle<LocalDiffStateModel>),
    Remote(ModelHandle<RemoteDiffStateModel>),
}

/// Key for the per-repository `DiffStateModel` cache in
/// [`WorkingDirectoriesModel`]. Uses `PathBuf` for local repos (avoiding
/// `PathBuf → StandardizedPath` conversion) and `RemoteRepositoryIdentifier`
/// for remote repos.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UniversalPath {
    Local(PathBuf),
    Remote(RemoteRepositoryIdentifier),
}

// ── Wrapper ──────────────────────────────────────────────────────────────

/// Wrapper that provides a unified API over local and remote diff state models.
///
/// All consumers should interact with this type rather than accessing
/// [`LocalDiffStateModel`] or [`RemoteDiffStateModel`] directly.
pub struct DiffStateModel {
    inner: DiffStateBackend,
}

impl warpui::Entity for DiffStateModel {
    type Event = DiffStateModelEvent;
}

impl DiffStateModel {
    // ── Construction ─────────────────────────────────────────────────

    pub fn new(key: UniversalPath, ctx: &mut ModelContext<Self>) -> Self {
        let inner = match key {
            UniversalPath::Local(path) => {
                let repo_path = Some(path.display().to_string());
                let local = ctx.add_model(|ctx| LocalDiffStateModel::new(repo_path, ctx));
                ctx.subscribe_to_model(&local, Self::forward_event);
                DiffStateBackend::Local(local)
            }
            UniversalPath::Remote(_remote_id) => {
                let remote = ctx.add_model(RemoteDiffStateModel::new);
                ctx.subscribe_to_model(&remote, Self::forward_event);
                DiffStateBackend::Remote(remote)
            }
        };
        Self { inner }
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

    /// Returns `true` when the backend is a local diff state model.
    pub(crate) fn is_local(&self) -> bool {
        matches!(self.inner, DiffStateBackend::Local(_))
    }

    pub(crate) fn get(&self, ctx: &AppContext) -> DiffState {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).get(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).get(),
        }
    }

    pub(crate) fn diff_mode(&self, ctx: &AppContext) -> DiffMode {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).diff_mode(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).diff_mode(),
        }
    }

    pub(crate) fn get_uncommitted_stats(&self, ctx: &AppContext) -> Option<DiffStats> {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).get_uncommitted_stats(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).get_uncommitted_stats(),
        }
    }

    pub(crate) fn get_main_branch_name(&self, ctx: &AppContext) -> Option<String> {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).get_main_branch_name(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).get_main_branch_name(),
        }
    }

    pub fn get_current_branch_name(&self, ctx: &AppContext) -> Option<String> {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).get_current_branch_name(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).get_current_branch_name(),
        }
    }

    pub(crate) fn is_on_main_branch(&self, ctx: &AppContext) -> bool {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).is_on_main_branch(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).is_on_main_branch(),
        }
    }

    pub(crate) fn unpushed_commits<'a>(&self, ctx: &'a AppContext) -> &'a [Commit] {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).unpushed_commits(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).unpushed_commits(),
        }
    }

    pub(crate) fn upstream_ref<'a>(&self, ctx: &'a AppContext) -> Option<&'a str> {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).upstream_ref(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).upstream_ref(),
        }
    }

    pub(crate) fn upstream_differs_from_main(&self, ctx: &AppContext) -> bool {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).upstream_differs_from_main(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).upstream_differs_from_main(),
        }
    }

    pub(crate) fn pr_info<'a>(&self, ctx: &'a AppContext) -> Option<&'a PrInfo> {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).pr_info(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).pr_info(),
        }
    }

    pub(crate) fn is_pr_info_refreshing(&self, ctx: &AppContext) -> bool {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).is_pr_info_refreshing(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).is_pr_info_refreshing(),
        }
    }

    pub(crate) fn is_git_operation_blocked(&self, ctx: &AppContext) -> bool {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).is_git_operation_blocked(ctx),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).is_git_operation_blocked(ctx),
        }
    }

    pub(crate) fn has_head(&self, ctx: &AppContext) -> bool {
        match &self.inner {
            DiffStateBackend::Local(m) => m.as_ref(ctx).has_head(),
            DiffStateBackend::Remote(m) => m.as_ref(ctx).has_head(),
        }
    }

    // ── Unified write API ────────────────────────────────────────────

    pub(crate) fn set_diff_mode(
        &self,
        mode: DiffMode,
        should_fetch_base: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        match &self.inner {
            DiffStateBackend::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.set_diff_mode(mode, should_fetch_base, ctx);
                });
            }
            DiffStateBackend::Remote(_) => {}
        }
    }

    pub(crate) fn set_diff_mode_and_fetch_base(
        &self,
        mode: DiffMode,
        ctx: &mut ModelContext<Self>,
    ) {
        match &self.inner {
            DiffStateBackend::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.set_diff_mode_and_fetch_base(mode, ctx);
                });
            }
            DiffStateBackend::Remote(_) => {}
        }
    }

    pub(crate) fn load_diffs_for_current_repo(
        &self,
        should_fetch_base: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        match &self.inner {
            DiffStateBackend::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.load_diffs_for_current_repo(should_fetch_base, ctx);
                });
            }
            DiffStateBackend::Remote(_) => {}
        }
    }

    pub(crate) fn set_code_review_metadata_refresh_enabled(
        &self,
        enabled: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        match &self.inner {
            DiffStateBackend::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.set_code_review_metadata_refresh_enabled(enabled, ctx);
                });
            }
            DiffStateBackend::Remote(_) => {}
        }
    }

    pub(crate) fn refresh_metadata_and_pr_info(&self, ctx: &mut ModelContext<Self>) {
        match &self.inner {
            DiffStateBackend::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.refresh_metadata_and_pr_info(ctx);
                });
            }
            DiffStateBackend::Remote(_) => {}
        }
    }

    pub(crate) fn discard_files(
        &self,
        file_infos: Vec<FileStatusInfo>,
        should_stash: bool,
        branch_name: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        match &self.inner {
            DiffStateBackend::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.discard_files(file_infos, should_stash, branch_name, ctx);
                });
            }
            DiffStateBackend::Remote(_) => {}
        }
    }

    // ── Local-only operations ─────────────────────────────────────────
    // These are inherently local and delegate directly to `LocalDiffStateModel`
    // without dispatching through the backend enum. They operate on local repo
    // paths, run git CLI commands, or parse raw git output — none of which
    // apply to remote repositories. Remote equivalents will use server-side
    // APIs instead.

    /// Lists branches by running `git branch` against a local repo path.
    pub async fn get_all_branches(
        repo_path: &Path,
        max_branch_count: Option<usize>,
        include_remotes: bool,
    ) -> Result<Vec<(String, bool)>> {
        LocalDiffStateModel::get_all_branches(repo_path, max_branch_count, include_remotes).await
    }

    /// Lists branches with a known main branch by running `git branch`
    /// against a local repo path.
    pub async fn get_all_branches_with_known_main(
        repo_path: &Path,
        main_branch: &str,
        max_branch_count: Option<usize>,
        include_remotes: bool,
    ) -> Result<Vec<(String, bool)>> {
        LocalDiffStateModel::get_all_branches_with_known_main(
            repo_path,
            main_branch,
            max_branch_count,
            include_remotes,
        )
        .await
    }

    /// Sorts a branch list so the main branch appears first. Pure helper
    /// over data returned by the local git branch listing above.
    pub fn sort_branches_main_first(
        branches: &[(String, bool)],
    ) -> impl Iterator<Item = &(String, bool)> {
        LocalDiffStateModel::sort_branches_main_first(branches)
    }

    /// Parses a `@@ -a,b +c,d @@` unified diff header line. This is a
    /// format-level utility tied to local `git diff` output.
    pub(crate) fn parse_unified_diff_header(header_line: &str) -> Result<UnifiedDiffHeader> {
        LocalDiffStateModel::parse_unified_diff_header(header_line)
    }

    #[cfg(feature = "local_fs")]
    pub(crate) fn stop_active_watcher(&self, ctx: &mut ModelContext<Self>) {
        match &self.inner {
            DiffStateBackend::Local(local) => {
                local.update(ctx, |local, ctx| {
                    local.stop_active_watcher(ctx);
                });
            }
            DiffStateBackend::Remote(_) => {}
        }
    }

    #[cfg(feature = "local_fs")]
    pub async fn load_diff_data_for_mode(
        mode: DiffMode,
        repo_path: PathBuf,
    ) -> Option<GitDiffData> {
        LocalDiffStateModel::load_diff_data_for_mode(mode, repo_path).await
    }

    pub(crate) async fn diff_metadata_against_head(
        repo_path: &Path,
    ) -> Result<DiffMetadataAgainstBase> {
        LocalDiffStateModel::diff_metadata_against_head(repo_path).await
    }

    pub(crate) async fn retrieve_diff_state(
        repo_path: &Path,
        file: &Path,
        mode: &DiffMode,
        merge_base: Option<&str>,
    ) -> Result<(PathBuf, Option<Arc<FileDiffAndContent>>)> {
        LocalDiffStateModel::retrieve_diff_state(repo_path, file, mode, merge_base).await
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
        Self {
            inner: DiffStateBackend::Local(local),
        }
    }

    pub(crate) async fn compute_merge_base(repo_path: &Path, mode: &DiffMode) -> Result<String> {
        LocalDiffStateModel::compute_merge_base(repo_path, mode).await
    }

    pub(crate) fn parse_diff_hunks(diff_output: &str) -> Result<Vec<DiffHunk>> {
        LocalDiffStateModel::parse_diff_hunks(diff_output)
    }

    pub(crate) fn parse_range(range_str: &str) -> Result<(usize, usize)> {
        LocalDiffStateModel::parse_range(range_str)
    }
}
