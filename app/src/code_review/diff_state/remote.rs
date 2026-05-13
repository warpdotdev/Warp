//! Remote diff state model.
//!
//! Client-side model for a single remote repository diff state subscription
//! received from the remote server. Presents the same read API as
//! `LocalDiffStateModel` and emits the same `DiffStateModelEvent` variants.
//!
//! The active [`DiffMode`] can change; the model handles this by unsubscribing
//! from the old `(repo_path, mode)` subscription and re-subscribing with the
//! new mode.

use std::sync::Arc;

use crate::remote_server::proto;
use crate::util::git::{Commit, PrInfo};
use remote_server::manager::{RemoteServerManager, RemoteServerManagerEvent};
use warp_core::{HostId, SessionId};
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{ModelContext, SingletonEntity};

use super::{
    DiffMetadata, DiffMode, DiffState, DiffStateFileDelta, DiffStateMetadataUpdate,
    DiffStateModelEvent, DiffStateSnapshot, DiffStats, GitDiffData,
};

// ── Internal state ───────────────────────────────────────────────────

#[derive(Default)]
enum InternalRemoteDiffState {
    #[default]
    Loading,
    NotInRepository,
    Loaded(GitDiffData),
    Error(String),
    /// The remote connection was lost. Preserves stale data until the model
    /// can re-establish the server-side subscription.
    Disconnected,
}

// ── Model ────────────────────────────────────────────────────────────

pub struct RemoteDiffStateModel {
    remote_path: RemotePath,
    mode: DiffMode,
    state: InternalRemoteDiffState,
    metadata: Option<DiffMetadata>,
    /// The session through which the current server-side subscription
    /// was established. Used to detect subscription loss when a specific
    /// session disconnects while other sessions to the same host survive.
    session_id: Option<SessionId>,
}

impl warpui::Entity for RemoteDiffStateModel {
    type Event = DiffStateModelEvent;
}

impl RemoteDiffStateModel {
    /// Creates a new remote diff state model and initiates the `GetDiffState`
    /// request. The model starts in `Loading` state.
    pub fn new(remote_path: RemotePath, mode: DiffMode, ctx: &mut ModelContext<Self>) -> Self {
        // Subscribe to RemoteServerManager push events and filter by remote_path and diff_mode
        let mgr_handle = RemoteServerManager::handle(ctx);
        ctx.subscribe_to_model(&mgr_handle, Self::handle_manager_event);

        // Send the initial GetDiffState request through any currently-
        // connected session for this host. If none exists, wait for a
        // HostConnected event to resubscribe.
        let session_id = Self::find_connected_session(&remote_path.host_id, ctx);
        if let Some(session_id) = session_id {
            let remote_path = remote_path.clone();
            let mode = mode.clone();
            mgr_handle.update(ctx, |mgr, ctx| {
                mgr.get_diff_state(session_id, remote_path, proto::DiffMode::from(&mode), ctx);
            });
        } else {
            log::warn!(
                "RemoteDiffStateModel: no connected session for host={:?}, \
                 will wait for push events",
                remote_path.host_id
            );
        }

        Self {
            remote_path,
            mode,
            state: InternalRemoteDiffState::Loading,
            metadata: None,
            session_id,
        }
    }

    /// Resolves a connected session for `host_id` at call time.
    /// Returns `None` if no session is currently connected.
    fn find_connected_session(host_id: &HostId, ctx: &ModelContext<Self>) -> Option<SessionId> {
        let mgr = RemoteServerManager::handle(ctx);
        let mgr_ref = mgr.as_ref(ctx);
        mgr_ref.sessions_for_host(host_id).and_then(|sessions| {
            sessions
                .iter()
                .copied()
                .find(|sid| mgr_ref.client_for_session(*sid).is_some())
        })
    }

    // ── Event handler ───────────────────────────────────────────

    fn matches_remote_path_and_mode(
        &self,
        host_id: &HostId,
        repo_path: &StandardizedPath,
        mode: &proto::DiffMode,
    ) -> bool {
        let remote_mode = proto::DiffMode::from(&self.mode);
        host_id == &self.remote_path.host_id
            && repo_path == &self.remote_path.path
            && mode == &remote_mode
    }

    fn handle_manager_event(
        &mut self,
        event: &RemoteServerManagerEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        match event {
            RemoteServerManagerEvent::DiffStateSnapshotReceived {
                host_id,
                repo_path,
                mode,
                snapshot,
            } => {
                if !self.matches_remote_path_and_mode(host_id, repo_path, mode) {
                    return;
                }
                match DiffStateSnapshot::try_from(snapshot) {
                    Ok(snapshot) => self.apply_snapshot(snapshot, ctx),
                    Err(error) => {
                        log::warn!("RemoteDiffStateModel: invalid diff state snapshot: {error}");
                    }
                }
            }
            RemoteServerManagerEvent::DiffStateMetadataUpdateReceived {
                host_id,
                repo_path,
                mode,
                update,
            } => {
                if !self.matches_remote_path_and_mode(host_id, repo_path, mode) {
                    return;
                }
                match DiffStateMetadataUpdate::try_from(update) {
                    Ok(update) => {
                        if let Some(metadata) = &update.metadata {
                            self.apply_metadata_update(metadata, ctx);
                        }
                    }
                    Err(error) => {
                        log::warn!(
                            "RemoteDiffStateModel: invalid diff state metadata update: {error}"
                        );
                    }
                }
            }
            RemoteServerManagerEvent::DiffStateFileDeltaReceived {
                host_id,
                repo_path,
                mode,
                delta,
            } => {
                if !self.matches_remote_path_and_mode(host_id, repo_path, mode) {
                    return;
                }
                match DiffStateFileDelta::try_from(delta) {
                    Ok(delta) => self.apply_file_delta(delta, ctx),
                    Err(error) => {
                        log::warn!("RemoteDiffStateModel: invalid diff state file delta: {error}");
                    }
                }
            }
            RemoteServerManagerEvent::HostDisconnected { host_id }
                if host_id == &self.remote_path.host_id =>
            {
                self.session_id = None;
                self.state = InternalRemoteDiffState::Disconnected;
                ctx.emit(DiffStateModelEvent::ConnectionLost);
            }
            // Host came back after being fully down. Only re-subscribe if
            // we don't already have a live subscription session — otherwise
            // we'd issue a redundant GetDiffState and force a transient
            // Loading state.
            RemoteServerManagerEvent::HostConnected { host_id }
                if host_id == &self.remote_path.host_id && self.session_id.is_none() =>
            {
                self.resubscribe(ctx);
            }
            // The same transport session recovered with a fresh client
            // connection. The server-side subscription was tied to the old
            // connection, so re-send the repo/mode subscription through the
            // recovered session.
            RemoteServerManagerEvent::SessionReconnected {
                session_id,
                host_id,
                ..
            } if Some(*session_id) == self.session_id && host_id == &self.remote_path.host_id => {
                self.resubscribe(ctx);
            }
            RemoteServerManagerEvent::SessionDisconnected {
                session_id,
                host_id,
                ..
            } if Some(*session_id) == self.session_id && host_id == &self.remote_path.host_id => {
                self.resubscribe(ctx);
            }
            _ => {}
        }
    }

    // ── Re-subscription ───────────────────────────────────────────────

    /// Re-sends `GetDiffState` through a connected session and transitions
    /// to `Loading` while waiting for a fresh snapshot.
    fn resubscribe(&mut self, ctx: &mut ModelContext<Self>) {
        let Some(session_id) = Self::find_connected_session(&self.remote_path.host_id, ctx) else {
            log::warn!(
                "RemoteDiffStateModel::resubscribe: no connected session for host={:?}",
                self.remote_path.host_id
            );
            self.session_id = None;
            return;
        };
        let remote_path = self.remote_path.clone();
        let mode = self.mode.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.get_diff_state(session_id, remote_path, proto::DiffMode::from(&mode), ctx);
        });
        self.session_id = Some(session_id);
        self.state = InternalRemoteDiffState::Loading;
        ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
    }

    // ── Apply methods ────────────────────────────────────────────────

    fn apply_snapshot(&mut self, snapshot: DiffStateSnapshot, ctx: &mut ModelContext<Self>) {
        // Update metadata, detecting branch changes.
        if let Some(metadata) = &snapshot.metadata {
            self.apply_metadata_update(metadata, ctx);
        }

        // Update state.
        match snapshot.state {
            // Disconnected is never produced by proto deserialization.
            DiffState::Disconnected => {}
            DiffState::NotInRepository => {
                self.state = InternalRemoteDiffState::NotInRepository;
                ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
            }
            DiffState::Loading => {
                self.state = InternalRemoteDiffState::Loading;
                ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
            }
            DiffState::Error(msg) => {
                self.state = InternalRemoteDiffState::Error(msg);
                ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
            }
            DiffState::Loaded => {
                let Some(base_content) = snapshot.diffs else {
                    self.state = InternalRemoteDiffState::Error(
                        "Server reported loaded state but no diff data was available".to_string(),
                    );
                    ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
                    return;
                };
                let diffs = GitDiffData::from(&base_content);
                self.state = InternalRemoteDiffState::Loaded(diffs);
                ctx.emit(DiffStateModelEvent::NewDiffsComputed(Some(Arc::new(
                    base_content,
                ))));
            }
        }
    }

    fn apply_metadata_update(&mut self, metadata: &DiffMetadata, ctx: &mut ModelContext<Self>) {
        let previous_branch = self
            .metadata
            .as_ref()
            .map(|m| m.current_branch_name.as_str());
        let branch_changed =
            previous_branch.is_some_and(|prev| prev != metadata.current_branch_name.as_str());
        self.metadata = Some(metadata.clone());

        // Only emit CurrentBranchChanged when there was a previous branch to
        // compare against. On the first metadata update (initial snapshot)
        // previous_branch is None — that's initial population, not a switch.
        if branch_changed {
            ctx.emit(DiffStateModelEvent::CurrentBranchChanged);
        }
        ctx.emit(DiffStateModelEvent::MetadataRefreshed(metadata.clone()));
    }

    fn apply_file_delta(&mut self, delta: DiffStateFileDelta, ctx: &mut ModelContext<Self>) {
        if let Some(metadata) = &delta.metadata {
            self.apply_metadata_update(metadata, ctx);
        }

        let InternalRemoteDiffState::Loaded(ref mut diffs) = self.state else {
            // Ignore file deltas until the initial snapshot has loaded.
            return;
        };

        let file_path = delta.file_path;
        let event_path = file_path.to_local_path_lossy();
        let diff = delta.diff;

        if let Some(ref new_diff) = diff {
            if let Some(pos) = diffs
                .files
                .iter()
                .position(|f| f.file_path.to_string_lossy() == file_path.as_str())
            {
                diffs.files[pos] = new_diff.file_diff.clone();
            } else {
                diffs.files.push(new_diff.file_diff.clone());
            }
        } else {
            diffs
                .files
                .retain(|f| f.file_path.to_string_lossy() != file_path.as_str());
        }
        diffs.total_additions = diffs.files.iter().map(|f| f.additions()).sum();
        diffs.total_deletions = diffs.files.iter().map(|f| f.deletions()).sum();
        diffs.files_changed = diffs.files.len();
        ctx.emit(DiffStateModelEvent::SingleFileUpdated {
            path: event_path,
            diff: diff.map(Arc::new),
        });
    }

    // ── Cleanup ──────────────────────────────────────────────────────

    /// Sends `UnsubscribeDiffState` to the server. Call before dropping the
    /// model (the wrapper calls it during mode switch / pane close).
    pub fn unsubscribe(&self, ctx: &mut ModelContext<Self>) {
        let Some(session_id) = self.session_id else {
            log::debug!(
                "RemoteDiffStateModel::unsubscribe: no subscription session for host={:?}",
                self.remote_path.host_id
            );
            return;
        };
        let mgr_handle = RemoteServerManager::handle(ctx);
        let mgr = mgr_handle.as_ref(ctx);
        if mgr.client_for_session(session_id).is_none() {
            log::debug!(
                "RemoteDiffStateModel::unsubscribe: subscription session is no longer connected: session={session_id:?}"
            );
            return;
        }
        mgr.unsubscribe_diff_state(
            session_id,
            &self.remote_path,
            proto::DiffMode::from(&self.mode),
        );
    }

    // ── Read API (matching LocalDiffStateModel interface) ────────────

    pub fn get(&self) -> DiffState {
        match &self.state {
            InternalRemoteDiffState::NotInRepository => DiffState::NotInRepository,
            InternalRemoteDiffState::Loading => DiffState::Loading,
            InternalRemoteDiffState::Loaded(_) => DiffState::Loaded,
            InternalRemoteDiffState::Error(msg) => DiffState::Error(msg.clone()),
            InternalRemoteDiffState::Disconnected => DiffState::Disconnected,
        }
    }

    pub fn diff_mode(&self) -> DiffMode {
        self.mode.clone()
    }

    pub fn get_uncommitted_stats(&self) -> Option<DiffStats> {
        self.metadata
            .as_ref()
            .map(|m| m.against_head.aggregate_stats)
    }

    pub fn get_main_branch_name(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .map(|m| m.main_branch_name.clone())
            .filter(|s| !s.is_empty())
    }

    pub fn get_current_branch_name(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .map(|m| m.current_branch_name.clone())
            .filter(|s| !s.is_empty())
    }

    pub fn is_on_main_branch(&self) -> bool {
        self.metadata.as_ref().is_some_and(|m| {
            !m.current_branch_name.is_empty() && m.current_branch_name == m.main_branch_name
        })
    }

    pub fn unpushed_commits(&self) -> &[Commit] {
        self.metadata
            .as_ref()
            .map(|m| m.unpushed_commits.as_slice())
            .unwrap_or(&[])
    }

    pub fn upstream_ref(&self) -> Option<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.upstream_ref.as_deref())
    }

    pub fn upstream_differs_from_main(&self) -> bool {
        match (self.upstream_ref(), self.get_main_branch_name().as_deref()) {
            (Some(upstream), Some(main)) => upstream != main,
            _ => false,
        }
    }

    pub fn pr_info(&self) -> Option<&PrInfo> {
        self.metadata.as_ref().and_then(|m| m.pr_info.as_ref())
    }

    pub fn is_pr_info_refreshing(&self) -> bool {
        false
    }

    pub fn is_git_operation_blocked(&self, _ctx: &warpui::AppContext) -> bool {
        false
    }

    pub fn has_head(&self) -> bool {
        self.metadata.as_ref().is_some_and(|m| m.has_head_commit)
    }

    pub fn remote_path(&self) -> RemotePath {
        self.remote_path.clone()
    }

    // ── Write API ────────────────────────────────────────────────────

    pub fn set_diff_mode(&mut self, mode: DiffMode, ctx: &mut ModelContext<Self>) {
        if self.mode == mode {
            return;
        }

        // Unsubscribe from the old mode before switching.
        self.unsubscribe(ctx);
        self.session_id = None;

        self.mode = mode;
        self.state = InternalRemoteDiffState::Loading;
        ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
        self.resubscribe(ctx);
    }

    /// Sends a `DiscardFiles` request to the remote server. The server's
    /// watcher will push updated diff snapshots on success.
    pub fn discard_files(
        &self,
        file_infos: Vec<super::FileStatusInfo>,
        should_stash: bool,
        branch_name: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(session_id) = Self::find_connected_session(&self.remote_path.host_id, ctx) else {
            log::warn!(
                "RemoteDiffStateModel::discard_files: no connected session for host={:?}",
                self.remote_path.host_id
            );
            return;
        };
        let remote_path = self.remote_path.clone();
        let mode = self.mode.clone();
        let proto_files = file_infos.iter().map(proto::FileStatusInfo::from).collect();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.discard_files(
                session_id,
                remote_path,
                proto_files,
                should_stash,
                branch_name,
                proto::DiffMode::from(&mode),
                ctx,
            );
        });
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod remote_tests;
