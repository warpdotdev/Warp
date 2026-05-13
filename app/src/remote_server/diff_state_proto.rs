//! Conversion between diff state Rust types and proto-generated types.
//!
//! Rust types are canonical, proto types are the wire format.
//! Only the directions needed by the server are implemented here.
//!
//! This module lives in `app/` (rather than in the `remote_server` crate alongside
//! `repo_metadata_proto`) because it depends on app-level types
//! (`code_review::diff_state`, `util::git`) that are not available in the crate.
use std::path::Path;

use super::proto;
use warp_util::standardized_path::StandardizedPath;

use crate::code_review::diff_size_limits::DiffSize;
use crate::code_review::diff_state::{
    DiffHunk, DiffLine, DiffLineType, DiffMetadata, DiffMetadataAgainstBase, DiffMode, DiffState,
    DiffStats, FileDiff, FileDiffAndContent, FileStatusInfo, GitDiffWithBaseContent, GitFileStatus,
};
use crate::util::git::{Commit, PrInfo};

// ── Proto → Rust (for incoming client messages) ────────────────────

impl From<&proto::DiffMode> for DiffMode {
    fn from(proto_mode: &proto::DiffMode) -> Self {
        match &proto_mode.mode {
            Some(proto::diff_mode::Mode::Head(_)) | None => DiffMode::Head,
            Some(proto::diff_mode::Mode::MainBranch(_)) => DiffMode::MainBranch,
            Some(proto::diff_mode::Mode::OtherBranch(ob)) => {
                DiffMode::OtherBranch(ob.branch_name.clone())
            }
        }
    }
}

impl TryFrom<&proto::GitFileStatus> for GitFileStatus {
    type Error = String;

    fn try_from(proto_status: &proto::GitFileStatus) -> Result<Self, Self::Error> {
        match &proto_status.status {
            Some(proto::git_file_status::Status::NewFile(_)) => Ok(GitFileStatus::New),
            Some(proto::git_file_status::Status::Modified(_)) => Ok(GitFileStatus::Modified),
            Some(proto::git_file_status::Status::Deleted(_)) => Ok(GitFileStatus::Deleted),
            Some(proto::git_file_status::Status::Renamed(r)) => Ok(GitFileStatus::Renamed {
                old_path: r.old_path.clone(),
            }),
            Some(proto::git_file_status::Status::Copied(c)) => Ok(GitFileStatus::Copied {
                old_path: c.old_path.clone(),
            }),
            Some(proto::git_file_status::Status::Untracked(_)) => Ok(GitFileStatus::Untracked),
            Some(proto::git_file_status::Status::Conflicted(_)) => Ok(GitFileStatus::Conflicted),
            None => Err("missing status variant in GitFileStatus".to_string()),
        }
    }
}

impl TryFrom<&proto::FileStatusInfo> for FileStatusInfo {
    type Error = String;

    fn try_from(proto_info: &proto::FileStatusInfo) -> Result<Self, Self::Error> {
        let path = StandardizedPath::try_new(&proto_info.path).map_err(|e| e.to_string())?;

        let status: GitFileStatus = proto_info
            .status
            .as_ref()
            .ok_or_else(|| "missing status in FileStatusInfo".to_string())
            .and_then(GitFileStatus::try_from)?;

        // Validate old_path in Renamed/Copied variants — these also flow
        // into git restore/checkout commands during discard.
        match &status {
            GitFileStatus::Renamed { old_path } | GitFileStatus::Copied { old_path } => {
                StandardizedPath::try_new(old_path).map_err(|e| e.to_string())?;
            }
            _ => {}
        }

        Ok(FileStatusInfo { path, status })
    }
}

// ── Rust → Proto (for server pushes) ────────────────────────────────

impl From<&DiffMode> for proto::DiffMode {
    fn from(mode: &DiffMode) -> Self {
        let mode_oneof = match mode {
            DiffMode::Head => proto::diff_mode::Mode::Head(proto::DiffModeHead {}),
            DiffMode::MainBranch => {
                proto::diff_mode::Mode::MainBranch(proto::DiffModeMainBranch {})
            }
            DiffMode::OtherBranch(branch) => {
                proto::diff_mode::Mode::OtherBranch(proto::DiffModeOtherBranch {
                    branch_name: branch.clone(),
                })
            }
        };
        proto::DiffMode {
            mode: Some(mode_oneof),
        }
    }
}

impl From<&DiffStats> for proto::DiffStats {
    fn from(stats: &DiffStats) -> Self {
        proto::DiffStats {
            files_changed: stats.files_changed as u64,
            total_additions: stats.total_additions as u64,
            total_deletions: stats.total_deletions as u64,
        }
    }
}

impl From<&DiffMetadataAgainstBase> for proto::DiffMetadataAgainstBase {
    fn from(m: &DiffMetadataAgainstBase) -> Self {
        proto::DiffMetadataAgainstBase {
            aggregate_stats: Some((&m.aggregate_stats).into()),
        }
    }
}

impl From<&Commit> for proto::Commit {
    fn from(c: &Commit) -> Self {
        proto::Commit {
            hash: c.hash.clone(),
            subject: c.subject.clone(),
            files_changed: c.files_changed as u64,
            additions: c.additions as u64,
            deletions: c.deletions as u64,
        }
    }
}

impl From<&PrInfo> for proto::PrInfo {
    fn from(p: &PrInfo) -> Self {
        proto::PrInfo {
            number: p.number,
            url: p.url.clone(),
        }
    }
}

impl From<&DiffMetadata> for proto::DiffMetadata {
    fn from(m: &DiffMetadata) -> Self {
        proto::DiffMetadata {
            main_branch_name: m.main_branch_name.clone(),
            current_branch_name: m.current_branch_name.clone(),
            against_head: Some((&m.against_head).into()),
            against_base_branch: m.against_base_branch.as_ref().map(|b| b.into()),
            has_head_commit: m.has_head_commit,
            unpushed_commits: m.unpushed_commits.iter().map(proto::Commit::from).collect(),
            upstream_ref: m.upstream_ref.clone(),
            pr_info: m.pr_info.as_ref().map(proto::PrInfo::from),
        }
    }
}

impl From<&GitFileStatus> for proto::GitFileStatus {
    fn from(s: &GitFileStatus) -> Self {
        let status = match s {
            GitFileStatus::New => {
                proto::git_file_status::Status::NewFile(proto::GitFileStatusNew {})
            }
            GitFileStatus::Modified => {
                proto::git_file_status::Status::Modified(proto::GitFileStatusModified {})
            }
            GitFileStatus::Deleted => {
                proto::git_file_status::Status::Deleted(proto::GitFileStatusDeleted {})
            }
            GitFileStatus::Renamed { old_path } => {
                proto::git_file_status::Status::Renamed(proto::GitFileStatusRenamed {
                    old_path: old_path.clone(),
                })
            }
            GitFileStatus::Copied { old_path } => {
                proto::git_file_status::Status::Copied(proto::GitFileStatusCopied {
                    old_path: old_path.clone(),
                })
            }
            GitFileStatus::Untracked => {
                proto::git_file_status::Status::Untracked(proto::GitFileStatusUntracked {})
            }
            GitFileStatus::Conflicted => {
                proto::git_file_status::Status::Conflicted(proto::GitFileStatusConflicted {})
            }
        };
        proto::GitFileStatus {
            status: Some(status),
        }
    }
}

impl From<&DiffLineType> for proto::DiffLineType {
    fn from(t: &DiffLineType) -> Self {
        match t {
            DiffLineType::Context => proto::DiffLineType::Context,
            DiffLineType::Add => proto::DiffLineType::Add,
            DiffLineType::Delete => proto::DiffLineType::Delete,
            DiffLineType::HunkHeader => proto::DiffLineType::HunkHeader,
        }
    }
}

impl From<&DiffLine> for proto::DiffLine {
    fn from(l: &DiffLine) -> Self {
        proto::DiffLine {
            line_type: proto::DiffLineType::from(&l.line_type).into(),
            old_line_number: l.old_line_number.map(|n| n as u64),
            new_line_number: l.new_line_number.map(|n| n as u64),
            text: l.text.clone(),
            no_trailing_newline: l.no_trailing_newline,
        }
    }
}

impl From<&DiffHunk> for proto::DiffHunk {
    fn from(h: &DiffHunk) -> Self {
        proto::DiffHunk {
            old_start_line: h.old_start_line as u64,
            old_line_count: h.old_line_count as u64,
            new_start_line: h.new_start_line as u64,
            new_line_count: h.new_line_count as u64,
            lines: h.lines.iter().map(proto::DiffLine::from).collect(),
            unified_diff_start: h.unified_diff_start as u64,
            unified_diff_end: h.unified_diff_end as u64,
        }
    }
}

impl From<&DiffSize> for proto::DiffSize {
    fn from(s: &DiffSize) -> Self {
        match s {
            DiffSize::Normal => proto::DiffSize::Normal,
            DiffSize::Large => proto::DiffSize::Large,
            DiffSize::Unrenderable => proto::DiffSize::Unrenderable,
        }
    }
}

impl From<&DiffState> for proto::DiffState {
    fn from(state: &DiffState) -> Self {
        let state_oneof = match state {
            DiffState::NotInRepository => {
                proto::diff_state::State::NotInRepository(proto::DiffStateNotInRepository {})
            }
            DiffState::Loading => proto::diff_state::State::Loading(proto::DiffStateLoading {}),
            DiffState::Error(msg) => proto::diff_state::State::Error(proto::DiffStateErrorValue {
                message: msg.clone(),
            }),
            DiffState::Loaded => proto::diff_state::State::Loaded(proto::DiffStateLoaded {}),
        };
        proto::DiffState {
            state: Some(state_oneof),
        }
    }
}

/// Converts a `FileDiff` to proto with an optional `content_at_base`.
/// Cannot be a `From` impl because of the extra parameter.
pub fn file_diff_to_proto(f: &FileDiff, content_at_base: Option<&str>) -> proto::FileDiff {
    proto::FileDiff {
        file_path: f.file_path.to_string_lossy().to_string(),
        status: Some((&f.status).into()),
        hunks: f.hunks.iter().map(proto::DiffHunk::from).collect(),
        is_binary: f.is_binary,
        is_autogenerated: f.is_autogenerated,
        max_line_number: f.max_line_number as u64,
        has_hidden_bidi_chars: f.has_hidden_bidi_chars,
        size: proto::DiffSize::from(&f.size).into(),
        content_at_base: content_at_base.map(|s| s.to_string()),
    }
}

impl From<&FileDiffAndContent> for proto::FileDiff {
    fn from(f: &FileDiffAndContent) -> Self {
        file_diff_to_proto(&f.file_diff, f.content_at_head.as_deref())
    }
}

impl From<&GitDiffWithBaseContent> for proto::GitDiffData {
    fn from(d: &GitDiffWithBaseContent) -> Self {
        proto::GitDiffData {
            files: d.files.iter().map(proto::FileDiff::from).collect(),
            total_additions: d.total_additions as u64,
            total_deletions: d.total_deletions as u64,
            files_changed: d.files_changed as u64,
        }
    }
}

// ── Higher-level message builders ───────────────────────────────────

/// Builds a `DiffStateSnapshot` proto message.
///
/// Accepts an optional `GitDiffWithBaseContent` and converts it to proto
/// internally. Pass `None` for terminal states (Error, NotInRepository)
/// or when diffs are not yet available.
pub fn build_diff_state_snapshot(
    repo_path: &str,
    mode: &DiffMode,
    metadata: Option<&DiffMetadata>,
    state: &DiffState,
    diffs: Option<&GitDiffWithBaseContent>,
) -> proto::DiffStateSnapshot {
    proto::DiffStateSnapshot {
        repo_path: repo_path.to_string(),
        mode: Some(mode.into()),
        metadata: metadata.map(proto::DiffMetadata::from),
        state: Some(state.into()),
        diffs: diffs.map(proto::GitDiffData::from),
    }
}

/// Builds a `DiffStateMetadataUpdate` proto message.
pub fn build_diff_state_metadata_update(
    repo_path: &str,
    mode: &DiffMode,
    metadata: &DiffMetadata,
) -> proto::DiffStateMetadataUpdate {
    proto::DiffStateMetadataUpdate {
        repo_path: repo_path.to_string(),
        mode: Some(mode.into()),
        metadata: Some(metadata.into()),
    }
}

/// Builds a `DiffStateFileDelta` proto message.
pub fn build_diff_state_file_delta(
    repo_path: &str,
    mode: &DiffMode,
    file_path: &Path,
    diff: Option<&FileDiffAndContent>,
    metadata: Option<&DiffMetadata>,
) -> proto::DiffStateFileDelta {
    proto::DiffStateFileDelta {
        repo_path: repo_path.to_string(),
        mode: Some(mode.into()),
        file_path: file_path.to_string_lossy().to_string(),
        diff: diff.map(proto::FileDiff::from),
        metadata: metadata.map(proto::DiffMetadata::from),
    }
}

#[cfg(test)]
#[path = "diff_state_proto_tests.rs"]
mod tests;
