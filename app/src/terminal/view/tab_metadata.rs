use crate::context_chips::display_chip::GitLineChanges;
use crate::context_chips::{
    git_line_changes_from_chips, github_pr_number_from_url, ContextChipKind,
};
use crate::terminal::TerminalView;
use crate::util::git::PrInfo;
use warpui::AppContext;

#[cfg(feature = "local_fs")]
use crate::code::buffer_location::BufferLocation;
#[cfg(feature = "local_fs")]
use crate::pane_group::WorkingDirectoriesModel;

impl TerminalView {
    fn prompt_chip_value(&self, chip_kind: &ContextChipKind, ctx: &AppContext) -> Option<String> {
        self.current_prompt
            .as_ref(ctx)
            .latest_chip_value(chip_kind, ctx)
            .map(|v| v.to_string())
            .filter(|value| !value.trim().is_empty())
    }

    pub fn display_working_directory(&self, ctx: &AppContext) -> Option<String> {
        let raw = self
            .prompt_chip_value(&ContextChipKind::WorkingDirectory, ctx)
            .or_else(|| self.pwd())?;
        let home_dir = self
            .active_block_session_id()
            .and_then(|session_id| self.sessions.as_ref(ctx).get(session_id))
            .and_then(|session| session.home_dir().map(str::to_owned));
        Some(warp_util::path::user_friendly_path(&raw, home_dir.as_deref()).to_string())
    }

    pub fn terminal_title_from_shell(&self) -> String {
        let model = self.model.lock();
        let fallback_title = model.shell_launch_state().display_name().to_owned();
        model
            .terminal_title()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or(fallback_title)
    }

    #[cfg_attr(not(feature = "local_fs"), allow(clippy::unnecessary_lazy_evaluations))]
    pub fn current_git_branch(&self, ctx: &AppContext) -> Option<String> {
        self.prompt_chip_value(&ContextChipKind::ShellGitBranch, ctx)
            .or_else(|| {
                #[cfg(feature = "local_fs")]
                {
                    self.git_status_metadata(ctx)
                        .map(|metadata| metadata.current_branch_name.clone())
                        .filter(|branch| !branch.trim().is_empty())
                }
                #[cfg(not(feature = "local_fs"))]
                {
                    None
                }
            })
    }

    pub fn last_completed_command_text(&self) -> Option<String> {
        let model = self.model.lock();
        model.block_list().blocks().iter().rev().find_map(|block| {
            if block.finished()
                && !block.is_background()
                && !block.is_static()
                && (block.bootstrap_stage().is_done() || block.is_restored())
            {
                let cmd = block.command_to_string();
                if cmd.trim().is_empty() {
                    None
                } else {
                    Some(cmd)
                }
            } else {
                None
            }
        })
    }

    pub fn terminal_title_text(&self) -> String {
        if !self.terminal_title.trim().is_empty() {
            return self.terminal_title.clone();
        }
        self.terminal_title_from_shell()
    }

    pub fn current_pull_request_url(&self, ctx: &AppContext) -> Option<String> {
        self.current_prompt
            .as_ref(ctx)
            .latest_chip_value(&ContextChipKind::GithubPullRequest, ctx)
            .map(|v| v.to_string())
            .filter(|value| !value.trim().is_empty())
    }

    /// PR info for this terminal's current branch.
    ///
    /// Under `local_fs`, the canonical substrate is the `DiffStateModel` for
    /// the terminal's repo — when one has been created (i.e. code review has
    /// been opened for that repo at some point in the session), its cached
    /// `pr_info` provides title/state/reviewers.
    ///
    /// The PR-chip URL from the prompt is used as the identity check: the
    /// cached info is only trusted when its URL matches the chip's URL. If
    /// they disagree (e.g. the active code-review repo differs from this
    /// terminal's repo), or no cached info exists, the result degrades to a
    /// number+URL-only `PrInfo`.
    ///
    /// Under `cfg(not(feature = "local_fs"))` (wasm builds) the chip URL is
    /// the only substrate.
    #[cfg(feature = "local_fs")]
    pub fn current_pull_request_info(
        &self,
        working_directories: &WorkingDirectoriesModel,
        ctx: &AppContext,
    ) -> Option<PrInfo> {
        let url = self.current_pull_request_url(ctx)?;
        let number = github_pr_number_from_url(&url)
            .and_then(|n| n.parse::<u64>().ok())?;

        let cached = self.current_repo_path().and_then(|repo_path| {
            let key = BufferLocation::Local(repo_path.clone());
            working_directories
                .diff_state_model_for(&key)
                .and_then(|handle| handle.as_ref(ctx).pr_info(ctx).cloned())
        });

        if let Some(info) = cached {
            if info.url == url {
                return Some(info);
            }
        }

        Some(PrInfo::minimal(number, url))
    }

    #[cfg(not(feature = "local_fs"))]
    pub fn current_pull_request_info(&self, ctx: &AppContext) -> Option<PrInfo> {
        let url = self.current_pull_request_url(ctx)?;
        let number = github_pr_number_from_url(&url)
            .and_then(|n| n.parse::<u64>().ok())?;
        Some(PrInfo::minimal(number, url))
    }

    #[cfg_attr(not(feature = "local_fs"), allow(clippy::unnecessary_lazy_evaluations))]
    pub fn current_diff_line_changes(&self, ctx: &AppContext) -> Option<GitLineChanges> {
        // Prefer the filesystem-event-based GitRepoStatusModel (which includes
        // untracked files) over parsing the raw shell chip output. This matches
        // the preference order used by the prompt chip display (display.rs) and
        // agent footer (chips.rs).
        #[cfg(feature = "local_fs")]
        let from_model = self
            .git_status_metadata(ctx)
            .map(|metadata| GitLineChanges::from_diff_stats(&metadata.stats_against_head));
        #[cfg(not(feature = "local_fs"))]
        let from_model: Option<GitLineChanges> = None;

        from_model
            .or_else(|| {
                git_line_changes_from_chips(&self.current_prompt.as_ref(ctx).agent_view_chips(ctx))
            })
            .filter(|line_changes| {
                line_changes.files_changed > 0
                    || line_changes.lines_added > 0
                    || line_changes.lines_removed > 0
            })
    }
}
