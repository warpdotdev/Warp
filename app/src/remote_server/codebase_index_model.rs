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

use super::manager::{RemoteServerManager, RemoteServerManagerEvent};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRemoteRepo {
    remote_path: RemotePath,
    is_git: bool,
}

#[derive(Clone, Debug)]
pub struct RemoteCodebaseSearchContext {
    pub host_id: HostId,
    pub repo_path: String,
    pub root_hash: NodeHash,
    pub embedding_config: EmbeddingConfig,
}

#[derive(Clone, Debug)]
pub enum RemoteCodebaseSearchAvailability {
    NotRemote,
    NoConnectedHost,
    NoActiveRepo,
    NotIndexed { repo_path: String },
    Indexing { repo_path: String },
    Failed { repo_path: String, message: String },
    Unavailable { repo_path: String, message: String },
    Ready(RemoteCodebaseSearchContext),
}

impl RemoteCodebaseSearchAvailability {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    fn repo_path(&self) -> Option<&str> {
        match self {
            Self::NotRemote | Self::NoConnectedHost | Self::NoActiveRepo => None,
            Self::NotIndexed { repo_path }
            | Self::Indexing { repo_path }
            | Self::Failed { repo_path, .. }
            | Self::Unavailable { repo_path, .. } => Some(repo_path),
            Self::Ready(context) => Some(context.repo_path.as_str()),
        }
    }
}

fn remote_path_for_status(
    host_id: &HostId,
    status: &RemoteCodebaseIndexStatus,
) -> Option<RemotePath> {
    remote_path_from_repo_path(host_id, &status.repo_path)
}

fn remote_path_from_repo_path(host_id: &HostId, repo_path: &str) -> Option<RemotePath> {
    StandardizedPath::try_new(repo_path)
        .ok()
        .map(|path| RemotePath::new(host_id.clone(), path))
}

#[derive(Default)]
pub struct RemoteCodebaseIndexModel {
    statuses: HashMap<RemotePath, RemoteCodebaseIndexStatus>,
    active_repos_by_host: HashMap<HostId, ActiveRemoteRepo>,
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
        explicit_repo_path: Option<&str>,
    ) -> RemoteCodebaseSearchAvailability {
        let Some(host_id) = session_context.host_id().cloned() else {
            if session_context.is_remote() {
                return RemoteCodebaseSearchAvailability::NoConnectedHost;
            }
            return RemoteCodebaseSearchAvailability::NotRemote;
        };

        self.availability_for_remote(
            &host_id,
            session_context.current_working_directory().as_deref(),
            explicit_repo_path,
        )
    }

    fn active_repo_path(
        &self,
        session_context: &SessionContext,
        explicit_repo_path: Option<&str>,
    ) -> Option<String> {
        self.active_repo_availability(session_context, explicit_repo_path)
            .repo_path()
            .map(ToOwned::to_owned)
    }

    pub fn request_active_repo_index(
        &self,
        session_context: &SessionContext,
        explicit_repo_path: Option<&str>,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let Some(host_id) = session_context.host_id().cloned() else {
            return false;
        };
        let Some(repo_path) = self
            .active_repo_path(session_context, explicit_repo_path)
            .or_else(|| session_context.current_working_directory().clone())
        else {
            return false;
        };

        RemoteServerManager::handle(ctx).update(ctx, |manager, ctx| {
            manager.index_codebase(host_id, repo_path, ctx);
        });
        true
    }

    fn handle_remote_server_manager_event(
        &mut self,
        event: &RemoteServerManagerEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        match event {
            RemoteServerManagerEvent::CodebaseIndexStatusesSnapshot {
                remote_identity_key: _,
                host_id,
                statuses,
            } => {
                self.apply_statuses_snapshot(host_id, statuses);
                ctx.notify();
            }
            RemoteServerManagerEvent::CodebaseIndexStatusUpdated {
                remote_identity_key: _,
                host_id,
                status,
            } => {
                self.apply_status_update(host_id, status.clone());
                ctx.notify();
            }
            RemoteServerManagerEvent::NavigatedToDirectory {
                session_id: _,
                host_id,
                indexed_path,
                is_git,
            } => {
                self.record_navigated_directory(host_id.clone(), indexed_path.clone(), *is_git);
                if *is_git && should_auto_index_remote_codebase(ctx) {
                    // Mirrors local auto-indexing for the thin remote E2E path. TODO(APP-3792):
                    // route remote indexing through the speedbump/consent flow instead of
                    // requesting immediately on navigation.
                    RemoteServerManager::handle(ctx).update(ctx, |manager, ctx| {
                        manager.index_codebase(host_id.clone(), indexed_path.clone(), ctx);
                    });
                }
                ctx.notify();
            }
            RemoteServerManagerEvent::HostDisconnected { host_id } => {
                self.remove_host(host_id);
                ctx.notify();
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
            | RemoteServerManagerEvent::SetupStateChanged { .. }
            | RemoteServerManagerEvent::BinaryCheckComplete { .. }
            | RemoteServerManagerEvent::BinaryInstallComplete { .. }
            | RemoteServerManagerEvent::ClientRequestFailed { .. }
            | RemoteServerManagerEvent::ServerMessageDecodingError { .. } => {}
        }
    }

    fn apply_statuses_snapshot(
        &mut self,
        host_id: &HostId,
        statuses: &[RemoteCodebaseIndexStatus],
    ) {
        self.statuses.retain(|key, _| key.host_id != *host_id);
        for status in statuses {
            self.apply_status_update(host_id, status.clone());
        }
    }

    fn apply_status_update(&mut self, host_id: &HostId, status: RemoteCodebaseIndexStatus) {
        let Some(remote_path) = remote_path_for_status(host_id, &status) else {
            return;
        };
        self.statuses.insert(remote_path, status);
    }

    fn record_navigated_directory(&mut self, host_id: HostId, repo_path: String, is_git: bool) {
        let Some(remote_path) = remote_path_from_repo_path(&host_id, &repo_path) else {
            return;
        };
        self.active_repos_by_host.insert(
            host_id,
            ActiveRemoteRepo {
                remote_path,
                is_git,
            },
        );
    }

    fn remove_host(&mut self, host_id: &HostId) {
        self.statuses.retain(|key, _| key.host_id != *host_id);
        self.active_repos_by_host.remove(host_id);
    }

    fn availability_for_remote(
        &self,
        host_id: &HostId,
        current_working_directory: Option<&str>,
        explicit_repo_path: Option<&str>,
    ) -> RemoteCodebaseSearchAvailability {
        let repo_path = explicit_repo_path
            .map(ToOwned::to_owned)
            .or_else(|| {
                self.active_repos_by_host
                    .get(host_id)
                    .filter(|repo| repo.is_git)
                    .map(|repo| repo.remote_path.path.as_str().to_string())
            })
            .or_else(|| {
                current_working_directory.and_then(|cwd| {
                    self.best_status_for_path(host_id, cwd)
                        .map(|status| status.repo_path.clone())
                })
            });

        let Some(repo_path) = repo_path else {
            return RemoteCodebaseSearchAvailability::NoActiveRepo;
        };

        let Some(status) = self.status_for_repo(host_id, &repo_path) else {
            return RemoteCodebaseSearchAvailability::NotIndexed { repo_path };
        };
        status.search_availability(host_id.clone())
    }

    fn status_for_repo(
        &self,
        host_id: &HostId,
        repo_path: &str,
    ) -> Option<&RemoteCodebaseIndexStatus> {
        remote_path_from_repo_path(host_id, repo_path)
            .and_then(|remote_path| self.statuses.get(&remote_path))
    }

    fn best_status_for_path(
        &self,
        host_id: &HostId,
        path: &str,
    ) -> Option<&RemoteCodebaseIndexStatus> {
        let path = StandardizedPath::try_new(path).ok()?;
        self.statuses
            .iter()
            .filter(|(key, _)| key.host_id == *host_id && path.starts_with(&key.path))
            .map(|(_, status)| status)
            .max_by_key(|status| status.repo_path.len())
    }
}

impl Entity for RemoteCodebaseIndexModel {
    type Event = ();
}

impl SingletonEntity for RemoteCodebaseIndexModel {}

trait RemoteCodebaseIndexStatusExt {
    fn search_availability(&self, host_id: HostId) -> RemoteCodebaseSearchAvailability;
}

impl RemoteCodebaseIndexStatusExt for RemoteCodebaseIndexStatus {
    fn search_availability(&self, host_id: HostId) -> RemoteCodebaseSearchAvailability {
        let repo_path = self.repo_path.clone();
        match self.state {
            RemoteCodebaseIndexState::Ready | RemoteCodebaseIndexState::Stale => {
                let Some(root_hash) = self
                    .root_hash
                    .as_deref()
                    .and_then(|hash| NodeHash::from_str(hash).ok())
                else {
                    return RemoteCodebaseSearchAvailability::Unavailable {
                        repo_path,
                        message: "The remote codebase index is missing its root hash.".to_string(),
                    };
                };
                RemoteCodebaseSearchAvailability::Ready(RemoteCodebaseSearchContext {
                    host_id,
                    repo_path,
                    root_hash,
                    embedding_config: EmbeddingConfig::default(),
                })
            }
            RemoteCodebaseIndexState::Queued | RemoteCodebaseIndexState::Indexing => {
                RemoteCodebaseSearchAvailability::Indexing { repo_path }
            }
            RemoteCodebaseIndexState::Failed => RemoteCodebaseSearchAvailability::Failed {
                repo_path,
                message: self
                    .failure_message
                    .clone()
                    .unwrap_or_else(|| "The remote codebase index failed.".to_string()),
            },
            RemoteCodebaseIndexState::NotEnabled
            | RemoteCodebaseIndexState::Unavailable
            | RemoteCodebaseIndexState::Disabled => RemoteCodebaseSearchAvailability::Unavailable {
                repo_path,
                message: self
                    .failure_message
                    .clone()
                    .unwrap_or_else(|| "Remote codebase search is not available.".to_string()),
            },
        }
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
