use std::collections::HashMap;
use std::str::FromStr;

use ai::index::full_source_code_embedding::{EmbeddingConfig, NodeHash};
use remote_server::codebase_index_proto::{RemoteCodebaseIndexState, RemoteCodebaseIndexStatus};
use warp_core::HostId;
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::blocklist::SessionContext;
use crate::features::FeatureFlag;
use crate::settings::CodeSettings;
use crate::workspaces::user_workspaces::UserWorkspaces;

use super::manager::{
    RemoteCodebaseIndexStatusWithPath, RemoteServerManager, RemoteServerManagerEvent,
};

#[derive(Clone, Debug)]
pub struct RemoteCodebaseSearchContext {
    pub remote_path: RemotePath,
    pub root_hash: NodeHash,
    pub embedding_config: EmbeddingConfig,
}

#[derive(Clone, Debug)]
pub enum RemoteCodebaseSearchAvailability {
    NoConnectedHost,
    NoActiveRepo,
    NotIndexed {
        remote_path: RemotePath,
    },
    Indexing {
        remote_path: RemotePath,
    },
    Unavailable {
        remote_path: RemotePath,
        message: String,
    },
    Ready(RemoteCodebaseSearchContext),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteCodebaseContextEntry {
    pub name: String,
    pub path: String,
}

impl RemoteCodebaseSearchAvailability {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    fn repo_path(&self) -> Option<&str> {
        match self {
            Self::NoConnectedHost | Self::NoActiveRepo => None,
            Self::NotIndexed { remote_path }
            | Self::Indexing { remote_path }
            | Self::Unavailable { remote_path, .. } => Some(remote_path.path.as_str()),
            Self::Ready(context) => Some(context.remote_path.path.as_str()),
        }
    }
}

fn remote_path_from_repo_path(host_id: &HostId, repo_path: &str) -> Option<RemotePath> {
    StandardizedPath::try_new(repo_path)
        .ok()
        .map(|path| RemotePath::new(host_id.clone(), path))
}

fn remote_codebase_name(repo_path: &str) -> String {
    repo_path
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(repo_path)
        .to_string()
}

#[derive(Default)]
pub struct RemoteCodebaseIndexModel {
    statuses: HashMap<RemotePath, RemoteCodebaseIndexStatus>,
    active_repos_by_host: HashMap<HostId, RemotePath>,
}

impl RemoteCodebaseIndexModel {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let manager = RemoteServerManager::handle(ctx);
        ctx.subscribe_to_model(&manager, |me, event, ctx| {
            me.handle_remote_server_manager_event(event, ctx);
        });
        Self::default()
    }

    pub fn active_repo_availability(
        &self,
        session_context: &SessionContext,
        requested_codebase_path: Option<&str>,
    ) -> RemoteCodebaseSearchAvailability {
        let Some(host_id) = session_context.host_id() else {
            return RemoteCodebaseSearchAvailability::NoConnectedHost;
        };

        self.availability_for_remote(
            host_id,
            session_context.current_working_directory().as_deref(),
            requested_codebase_path,
        )
    }

    pub fn active_repo_path(
        &self,
        session_context: &SessionContext,
        requested_codebase_path: Option<&str>,
    ) -> Option<String> {
        self.active_repo_availability(session_context, requested_codebase_path)
            .repo_path()
            .map(ToOwned::to_owned)
    }

    pub fn request_active_repo_index(
        &self,
        session_context: &SessionContext,
        requested_codebase_path: Option<&str>,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let Some(host_id) = session_context.host_id() else {
            return false;
        };
        let Some(remote_path) = self.resolve_remote_repo_path(
            host_id,
            session_context.current_working_directory().as_deref(),
            requested_codebase_path,
        ) else {
            return false;
        };

        RemoteServerManager::handle(ctx).update(ctx, |manager, ctx| {
            manager.ensure_codebase_indexed(remote_path, ctx);
        });
        true
    }

    pub fn codebases_for_agent_context(&self) -> Vec<RemoteCodebaseContextEntry> {
        let mut entries = self
            .statuses
            .iter()
            .filter(|&(remote_path, status)| {
                search_availability_for_status(status, remote_path.clone()).is_ready()
            })
            .map(|(remote_path, _)| {
                let path = remote_path.path.as_str().to_string();
                RemoteCodebaseContextEntry {
                    name: remote_codebase_name(&path),
                    path,
                }
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries
    }

    fn handle_remote_server_manager_event(
        &mut self,
        event: &RemoteServerManagerEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        match event {
            RemoteServerManagerEvent::CodebaseIndexStatusesSnapshot { host_id, statuses } => {
                self.apply_statuses_snapshot(host_id, statuses);
            }
            RemoteServerManagerEvent::CodebaseIndexStatusUpdated {
                remote_path,
                status,
            } => {
                self.apply_status_update(remote_path.clone(), status.clone());
            }
            RemoteServerManagerEvent::NavigatedToDirectory {
                session_id: _,
                remote_path,
                is_git,
            } => {
                self.record_navigated_directory(remote_path);
                if *is_git
                    && should_auto_index_remote_codebase(ctx)
                    && self.should_request_auto_index_for_navigated_git_repo(remote_path)
                {
                    // Mirrors local auto-indexing for the thin remote E2E path. TODO(APP-3792):
                    // route remote indexing through the speedbump/consent flow instead of
                    // requesting immediately on navigation.
                    let remote_path = remote_path.clone();
                    RemoteServerManager::handle(ctx).update(ctx, |manager, ctx| {
                        manager.ensure_codebase_indexed(remote_path, ctx);
                    });
                }
            }
            RemoteServerManagerEvent::HostDisconnected { host_id } => {
                self.remove_host(host_id);
            }
            RemoteServerManagerEvent::SessionConnecting { .. }
            | RemoteServerManagerEvent::SessionConnected { .. }
            | RemoteServerManagerEvent::SessionConnectionFailed { .. }
            | RemoteServerManagerEvent::SessionDisconnected { .. }
            | RemoteServerManagerEvent::SessionReconnected { .. }
            | RemoteServerManagerEvent::SessionDeregistered { .. }
            | RemoteServerManagerEvent::HostConnected { .. }
            | RemoteServerManagerEvent::RepoMetadataSnapshot { .. }
            | RemoteServerManagerEvent::RepoMetadataUpdated { .. }
            | RemoteServerManagerEvent::RepoMetadataDirectoryLoaded { .. }
            | RemoteServerManagerEvent::BufferUpdated { .. }
            | RemoteServerManagerEvent::BufferConflictDetected { .. }
            | RemoteServerManagerEvent::DiffStateSnapshotReceived { .. }
            | RemoteServerManagerEvent::DiffStateMetadataUpdateReceived { .. }
            | RemoteServerManagerEvent::DiffStateFileDeltaReceived { .. }
            | RemoteServerManagerEvent::GetBranchesResponse { .. }
            | RemoteServerManagerEvent::SetupStateChanged { .. }
            | RemoteServerManagerEvent::BinaryCheckComplete { .. }
            | RemoteServerManagerEvent::BinaryInstallComplete { .. }
            | RemoteServerManagerEvent::ClientRequestFailed { .. }
            | RemoteServerManagerEvent::ServerMessageDecodingError { .. } => {}
        }
    }
    fn should_request_auto_index_for_navigated_git_repo(&self, remote_path: &RemotePath) -> bool {
        let Some(status) = self.status_for_repo(remote_path) else {
            return true;
        };

        match search_availability_for_status(status, remote_path.clone()) {
            RemoteCodebaseSearchAvailability::Ready(_)
            | RemoteCodebaseSearchAvailability::Indexing { .. } => false,
            RemoteCodebaseSearchAvailability::NoConnectedHost
            | RemoteCodebaseSearchAvailability::NoActiveRepo
            | RemoteCodebaseSearchAvailability::NotIndexed { .. }
            | RemoteCodebaseSearchAvailability::Unavailable { .. } => true,
        }
    }

    fn apply_statuses_snapshot(
        &mut self,
        host_id: &HostId,
        statuses: &[RemoteCodebaseIndexStatusWithPath],
    ) {
        self.statuses.retain(|key, _| key.host_id != *host_id);
        for status_with_path in statuses {
            self.apply_status_update(
                status_with_path.remote_path.clone(),
                status_with_path.status.clone(),
            );
        }
    }

    fn apply_status_update(&mut self, remote_path: RemotePath, status: RemoteCodebaseIndexStatus) {
        self.statuses.insert(remote_path, status);
    }

    fn record_navigated_directory(&mut self, remote_path: &RemotePath) {
        self.active_repos_by_host
            .insert(remote_path.host_id.clone(), remote_path.clone());
    }

    fn remove_host(&mut self, host_id: &HostId) {
        self.statuses.retain(|key, _| key.host_id != *host_id);
        self.active_repos_by_host.remove(host_id);
    }

    fn availability_for_remote(
        &self,
        host_id: &HostId,
        current_working_directory: Option<&str>,
        requested_codebase_path: Option<&str>,
    ) -> RemoteCodebaseSearchAvailability {
        let remote_path = self.resolve_remote_repo_path(
            host_id,
            current_working_directory,
            requested_codebase_path,
        );

        let Some(remote_path) = remote_path else {
            return RemoteCodebaseSearchAvailability::NoActiveRepo;
        };
        let Some(status) = self.status_for_repo(&remote_path) else {
            return RemoteCodebaseSearchAvailability::NotIndexed { remote_path };
        };
        search_availability_for_status(status, remote_path)
    }

    fn resolve_remote_repo_path(
        &self,
        host_id: &HostId,
        current_working_directory: Option<&str>,
        requested_codebase_path: Option<&str>,
    ) -> Option<RemotePath> {
        if let Some(requested_codebase_path) = requested_codebase_path {
            let requested_remote_path =
                remote_path_from_repo_path(host_id, requested_codebase_path);
            // `requested_codebase_path` comes from `SearchCodebase.codebase_path`. Only honor it
            // when it matches daemon-reported remote index state, so a model can't accidentally
            // send a local path to the remote daemon for indexing.
            if let Some(remote_path) = requested_remote_path
                .as_ref()
                .filter(|remote_path| self.status_for_repo(remote_path).is_some())
            {
                return Some(remote_path.clone());
            }
            if let Some((remote_path, _)) =
                self.best_status_for_path(host_id, requested_codebase_path)
            {
                return Some(remote_path.clone());
            }
            return None;
        }

        if let Some(remote_path) = self.active_repos_by_host.get(host_id) {
            // Remote branch: only implicit searches (no `codebase_path`) fall back to the active
            // repo recorded by daemon navigation events.
            return Some(remote_path.clone());
        }

        if let Some((remote_path, _)) =
            current_working_directory.and_then(|cwd| self.best_status_for_path(host_id, cwd))
        {
            // Remote branch: if the remote cwd is inside a known indexed repo, use the indexed root
            // rather than re-indexing the nested directory.
            return Some(remote_path.clone());
        }

        current_working_directory.and_then(|cwd| {
            // Remote branch: only when we have no indexed/active remote repo do we fall back to the
            // remote session cwd as the candidate to index. Local sessions never use this path; they
            // resolve search roots in the local `SearchCodebase` executor branch instead.
            remote_path_from_repo_path(host_id, cwd)
        })
    }

    fn status_for_repo(&self, remote_path: &RemotePath) -> Option<&RemoteCodebaseIndexStatus> {
        self.statuses.get(remote_path)
    }

    fn best_status_for_path(
        &self,
        host_id: &HostId,
        path: &str,
    ) -> Option<(&RemotePath, &RemoteCodebaseIndexStatus)> {
        let path = StandardizedPath::try_new(path).ok()?;
        self.statuses
            .iter()
            .filter(|(key, _)| key.host_id == *host_id && path.starts_with(&key.path))
            .max_by_key(|(remote_path, _)| remote_path.path.as_str().len())
    }
}

impl Entity for RemoteCodebaseIndexModel {
    type Event = ();
}

impl SingletonEntity for RemoteCodebaseIndexModel {}

fn search_availability_for_status(
    status: &RemoteCodebaseIndexStatus,
    remote_path: RemotePath,
) -> RemoteCodebaseSearchAvailability {
    match status.state {
        RemoteCodebaseIndexState::Ready | RemoteCodebaseIndexState::Stale => {
            let Some(root_hash) = status
                .root_hash
                .as_deref()
                .and_then(|hash| NodeHash::from_str(hash).ok())
            else {
                return RemoteCodebaseSearchAvailability::Unavailable {
                    remote_path,
                    message: "The remote codebase index is missing its root hash.".to_string(),
                };
            };
            RemoteCodebaseSearchAvailability::Ready(RemoteCodebaseSearchContext {
                remote_path,
                root_hash,
                embedding_config: EmbeddingConfig::default(),
            })
        }
        RemoteCodebaseIndexState::Queued | RemoteCodebaseIndexState::Indexing => {
            RemoteCodebaseSearchAvailability::Indexing { remote_path }
        }
        RemoteCodebaseIndexState::Failed
        | RemoteCodebaseIndexState::NotEnabled
        | RemoteCodebaseIndexState::Unavailable
        | RemoteCodebaseIndexState::Disabled => RemoteCodebaseSearchAvailability::Unavailable {
            remote_path,
            message: status
                .failure_message
                .clone()
                .unwrap_or_else(|| "Remote codebase search is not available.".to_string()),
        },
    }
}

fn should_auto_index_remote_codebase(ctx: &mut ModelContext<RemoteCodebaseIndexModel>) -> bool {
    remote_auto_indexing_enabled(
        UserWorkspaces::as_ref(ctx).is_codebase_context_enabled(ctx),
        *CodeSettings::as_ref(ctx).auto_indexing_enabled,
    )
}

fn remote_auto_indexing_enabled(
    codebase_context_enabled: bool,
    auto_indexing_enabled: bool,
) -> bool {
    FeatureFlag::RemoteCodebaseIndexing.is_enabled()
        && FeatureFlag::FullSourceCodeEmbedding.is_enabled()
        && codebase_context_enabled
        && auto_indexing_enabled
}

#[cfg(test)]
#[path = "codebase_index_model_tests.rs"]
mod tests;
