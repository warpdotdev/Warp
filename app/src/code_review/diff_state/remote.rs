//! Remote diff state model.
//!
//! Client-side model for a single `(host_id, repo_path)` diff state subscription
//! received from the remote server. Presents the same read API as
//! `LocalDiffStateModel` and emits the same `DiffStateModelEvent` variants.
//!
//! The active [`DiffMode`] can change; the model handles this by unsubscribing
//! from the old `(repo_path, mode)` subscription and re-subscribing with the
//! new mode.

use std::path::PathBuf;
use std::sync::Arc;

use remote_server::manager::{RemoteServerManager, RemoteServerManagerEvent};
use remote_server::proto;
use remote_server::HostId;
use warp_core::SessionId;
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{ModelContext, SingletonEntity};

use crate::util::git::{Commit, PrInfo};

use super::{
    DiffMetadata, DiffMode, DiffState, DiffStateModelEvent, DiffStats, FileDiffAndContent,
    GitDiffData, GitDiffWithBaseContent,
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
    host_id: HostId,
    repo_path: StandardizedPath,
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
    pub fn new(
        host_id: HostId,
        repo_path: StandardizedPath,
        mode: DiffMode,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        // Subscribe to RemoteServerManager push events and filter by our
        // (host_id, repo_path, mode) triple.
        let mgr_handle = RemoteServerManager::handle(ctx);
        ctx.subscribe_to_model(&mgr_handle, Self::handle_manager_event);

        // Send the initial GetDiffState request through the manager.
        let session_id = Self::find_connected_session(&host_id, ctx);
        if let Some(session_id) = session_id {
            let proto_mode = remote_server::proto::DiffMode::from(&mode);
            mgr_handle.update(ctx, |mgr, ctx| {
                mgr.get_diff_state(session_id, repo_path.clone(), proto_mode, ctx);
            });
        } else {
            log::warn!(
                "RemoteDiffStateModel: no connected session for host={host_id:?}, \
                 will wait for push events"
            );
        }

        Self {
            host_id,
            repo_path,
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

    // ── Event handler ────────────────────────────────────────────────

    fn handle_manager_event(
        &mut self,
        event: &RemoteServerManagerEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let diff_subscription = match event {
            RemoteServerManagerEvent::DiffStateSnapshotReceived {
                host_id,
                repo_path,
                mode,
                ..
            }
            | RemoteServerManagerEvent::DiffStateMetadataUpdateReceived {
                host_id,
                repo_path,
                mode,
                ..
            }
            | RemoteServerManagerEvent::DiffStateFileDeltaReceived {
                host_id,
                repo_path,
                mode,
                ..
            } => Some((host_id, repo_path, mode)),
            _ => None,
        };
        if let Some((host_id, repo_path, mode)) = diff_subscription {
            if &self.host_id != host_id
                || &self.repo_path != repo_path
                || proto::DiffMode::from(&self.mode) != *mode
            {
                return;
            }
        }
        match event {
            RemoteServerManagerEvent::DiffStateSnapshotReceived { snapshot, .. } => {
                self.apply_snapshot(snapshot, ctx);
            }
            RemoteServerManagerEvent::DiffStateMetadataUpdateReceived { update, .. } => {
                if let Some(metadata) = &update.metadata {
                    self.apply_metadata_update(metadata, ctx);
                }
            }
            RemoteServerManagerEvent::DiffStateFileDeltaReceived { delta, .. } => {
                self.apply_file_delta(delta, ctx);
            }

            // ── Reconnection handling ─────────────────────────────────

            // The same transport session recovered with a fresh client
            // connection. The server-side subscription was tied to the old
            // connection, so re-send the repo/mode subscription through the
            // recovered session.
            RemoteServerManagerEvent::SessionReconnected {
                session_id,
                host_id,
                ..
            } if host_id == &self.host_id && self.session_id == Some(*session_id) => {
                self.resubscribe(*session_id, ctx);
            }

            // The transport session that established this subscription died.
            // Since diff state is host-scoped, another connected session to
            // the same host can safely re-establish the same repo/mode
            // subscription. If none exists, transition to Disconnected.
            RemoteServerManagerEvent::SessionDisconnected {
                session_id,
                host_id,
                ..
            } if host_id == &self.host_id && self.session_id == Some(*session_id) => {
                self.session_id = None;
                if let Some(new_session) = Self::find_connected_session(&self.host_id, ctx) {
                    // Another session to the same host is still alive —
                    // silently re-subscribe through it.
                    self.resubscribe(new_session, ctx);
                } else {
                    self.state = InternalRemoteDiffState::Disconnected;
                    ctx.emit(DiffStateModelEvent::ConnectionLost);
                }
            }

            // Reconnectable drops emit HostDisconnected before the reconnect
            // completes, but they do not emit SessionDisconnected unless all
            // reconnect attempts fail. Clear the stale subscription session so
            // HostConnected can re-establish it on the fresh connection.
            RemoteServerManagerEvent::HostDisconnected { host_id } if host_id == &self.host_id => {
                self.session_id = None;
                self.state = InternalRemoteDiffState::Disconnected;
                ctx.emit(DiffStateModelEvent::ConnectionLost);
            }
            // Host came back after being fully down. Re-subscribe.
            RemoteServerManagerEvent::HostConnected { host_id }
                if host_id == &self.host_id && self.session_id.is_none() =>
            {
                if let Some(session_id) = Self::find_connected_session(&self.host_id, ctx) {
                    self.resubscribe(session_id, ctx);
                }
            }

            _ => {}
        }
    }

    // ── Re-subscription ───────────────────────────────────────────────

    /// Re-sends `GetDiffState` through a new session and transitions to
    /// `Loading` while waiting for a fresh snapshot.
    fn resubscribe(&mut self, session_id: SessionId, ctx: &mut ModelContext<Self>) {
        let proto_mode = proto::DiffMode::from(&self.mode);
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.get_diff_state(session_id, self.repo_path.clone(), proto_mode, ctx);
        });
        self.session_id = Some(session_id);
        self.state = InternalRemoteDiffState::Loading;
        ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
    }

    // ── Apply methods (proto → domain conversion + event emission) ────

    fn apply_snapshot(
        &mut self,
        snapshot: &proto::DiffStateSnapshot,
        ctx: &mut ModelContext<Self>,
    ) {
        // Update metadata, detecting branch changes.
        if let Some(proto_meta) = &snapshot.metadata {
            self.apply_metadata_update(proto_meta, ctx);
        }

        // Update state.
        let state = DiffState::from(snapshot.state.as_ref());
        match state {
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
                let proto_diffs = match &snapshot.diffs {
                    Some(proto_diffs) => proto_diffs,
                    None => {
                        // The server reported Loaded but sent no diff data.
                        // This can happen if the async content reload on the
                        // server failed (e.g. repo deleted between the state
                        // check and the git commands). Transition to Error
                        // rather than showing a misleading empty file list.
                        log::warn!(
                            "RemoteDiffStateModel: snapshot has state=Loaded but \
                             diffs=None for repo={} mode={:?} — treating as error",
                            self.repo_path,
                            self.mode
                        );
                        self.state = InternalRemoteDiffState::Error(
                            "Server reported loaded state but no diff data was available"
                                .to_string(),
                        );
                        ctx.emit(DiffStateModelEvent::NewDiffsComputed(None));
                        return;
                    }
                };
                let base_content = GitDiffWithBaseContent::from(proto_diffs);
                let domain_diffs = GitDiffData::from(proto_diffs);
                self.state = InternalRemoteDiffState::Loaded(domain_diffs);
                ctx.emit(DiffStateModelEvent::NewDiffsComputed(Some(Arc::new(
                    base_content,
                ))));
            }
        }
    }

    fn apply_metadata_update(
        &mut self,
        proto_meta: &proto::DiffMetadata,
        ctx: &mut ModelContext<Self>,
    ) {
        let previous_branch = self
            .metadata
            .as_ref()
            .map(|m| m.current_branch_name.as_str());
        let domain_meta = DiffMetadata::from(proto_meta);
        let branch_changed =
            previous_branch.is_some_and(|prev| prev != domain_meta.current_branch_name);
        self.metadata = Some(domain_meta.clone());

        // Only emit CurrentBranchChanged when there was a previous branch to
        // compare against. On the first metadata update (initial snapshot)
        // previous_branch is None — that's initial population, not a switch.
        if branch_changed {
            ctx.emit(DiffStateModelEvent::CurrentBranchChanged);
        }
        ctx.emit(DiffStateModelEvent::MetadataRefreshed(domain_meta));
    }

    fn apply_file_delta(
        &mut self,
        delta: &remote_server::proto::DiffStateFileDelta,
        ctx: &mut ModelContext<Self>,
    ) {
        if let Some(proto_meta) = &delta.metadata {
            self.apply_metadata_update(proto_meta, ctx);
        }

        let InternalRemoteDiffState::Loaded(ref mut diffs) = self.state else {
            // Ignore file deltas until the initial snapshot has loaded.
            return;
        };

        let file_path = PathBuf::from(&delta.file_path);
        let domain_diff = delta.diff.as_ref().map(FileDiffAndContent::from);

        if let Some(ref new_diff) = domain_diff {
            if let Some(pos) = diffs.files.iter().position(|f| f.file_path == file_path) {
                diffs.files[pos] = new_diff.file_diff.clone();
            } else {
                diffs.files.push(new_diff.file_diff.clone());
            }
        } else {
            diffs.files.retain(|f| f.file_path != file_path);
        }
        diffs.total_additions = diffs.files.iter().map(|f| f.additions()).sum();
        diffs.total_deletions = diffs.files.iter().map(|f| f.deletions()).sum();
        diffs.files_changed = diffs.files.len();
        ctx.emit(DiffStateModelEvent::SingleFileUpdated {
            path: file_path,
            diff: domain_diff.map(Arc::new),
        });
    }

    // ── Cleanup ──────────────────────────────────────────────────────

    /// Sends `UnsubscribeDiffState` to the server. Call before dropping the
    /// model (the wrapper calls it during mode switch / pane close).
    pub fn unsubscribe(&self, ctx: &mut ModelContext<Self>) {
        let Some(session_id) = self.session_id else {
            log::debug!(
                "RemoteDiffStateModel::unsubscribe: no subscription session for host={:?}",
                self.host_id
            );
            return;
        };
        let proto_mode = remote_server::proto::DiffMode::from(&self.mode);
        let mgr_handle = RemoteServerManager::handle(ctx);
        let mgr = mgr_handle.as_ref(ctx);
        if mgr.client_for_session(session_id).is_none() {
            log::debug!(
                "RemoteDiffStateModel::unsubscribe: subscription session is no longer connected: session={session_id:?}"
            );
            return;
        }
        mgr.unsubscribe_diff_state(session_id, &self.repo_path, proto_mode);
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
        RemotePath::new(self.host_id.clone(), self.repo_path.clone())
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

        let session_id = Self::find_connected_session(&self.host_id, ctx);
        let Some(session_id) = session_id else {
            log::warn!(
                "RemoteDiffStateModel: no connected session for host={:?}, will wait for push events",
                self.host_id
            );
            return;
        };

        let proto_mode = proto::DiffMode::from(&self.mode);
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.get_diff_state(session_id, self.repo_path.clone(), proto_mode, ctx);
        });
        self.session_id = Some(session_id);
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
        let Some(session_id) = Self::find_connected_session(&self.host_id, ctx) else {
            log::warn!(
                "RemoteDiffStateModel::discard_files: no connected session for host={:?}",
                self.host_id
            );
            return;
        };

        let proto_files: Vec<_> = file_infos.iter().map(proto::FileStatusInfo::from).collect();
        let proto_mode = proto::DiffMode::from(&self.mode);

        let repo_path = self.repo_path.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.discard_files(
                session_id,
                repo_path,
                proto_files,
                should_stash,
                branch_name,
                proto_mode,
                ctx,
            );
        });
    }
}

#[cfg(test)]
impl RemoteDiffStateModel {
    fn new_for_test(
        mode: DiffMode,
        state: InternalRemoteDiffState,
        metadata: Option<DiffMetadata>,
    ) -> Self {
        Self {
            host_id: HostId::new("test-host".to_string()),
            repo_path: StandardizedPath::try_new("/test/repo")
                .expect("test repo path should be valid and absolute"),
            mode,
            state,
            metadata,
            session_id: None,
        }
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod remote_tests;
