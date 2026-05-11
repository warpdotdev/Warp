use std::collections::HashMap;
use std::str::FromStr;

use ai::index::full_source_code_embedding::{EmbeddingConfig, NodeHash};
use remote_server::codebase_index_proto::{RemoteCodebaseIndexState, RemoteCodebaseIndexStatus};
use warp_core::{HostId, SessionId};
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::blocklist::SessionContext;
use crate::features::FeatureFlag;
use crate::settings::CodeSettings;
use crate::workspaces::user_workspaces::UserWorkspaces;

use super::manager::{RemoteServerManager, RemoteServerManagerEvent};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RemoteCodebaseIndexKey {
    remote_identity_key: String,
    host_id: HostId,
    repo_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRemoteRepo {
    host_id: HostId,
    repo_path: String,
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

#[derive(Default)]
pub struct RemoteCodebaseIndexModel {
    statuses: HashMap<RemoteCodebaseIndexKey, RemoteCodebaseIndexStatus>,
    active_repos_by_session: HashMap<SessionId, ActiveRemoteRepo>,
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
            session_context.active_session_id(),
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
                remote_identity_key,
                host_id,
                statuses,
            } => {
                self.apply_statuses_snapshot(remote_identity_key, host_id, statuses);
                ctx.notify();
            }
            RemoteServerManagerEvent::CodebaseIndexStatusUpdated {
                remote_identity_key,
                host_id,
                status,
            } => {
                self.apply_status_update(remote_identity_key, host_id, status.clone());
                ctx.notify();
            }
            RemoteServerManagerEvent::NavigatedToDirectory {
                session_id,
                host_id,
                indexed_path,
                is_git,
            } => {
                self.record_navigated_directory(
                    *session_id,
                    host_id.clone(),
                    indexed_path.clone(),
                    *is_git,
                );
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
            RemoteServerManagerEvent::SessionDeregistered { session_id } => {
                self.active_repos_by_session.remove(session_id);
                ctx.notify();
            }
            RemoteServerManagerEvent::SessionConnecting { .. }
            | RemoteServerManagerEvent::SessionConnected { .. }
            | RemoteServerManagerEvent::SessionConnectionFailed { .. }
            | RemoteServerManagerEvent::SessionDisconnected { .. }
            | RemoteServerManagerEvent::SessionReconnected { .. }
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
        remote_identity_key: &str,
        host_id: &HostId,
        statuses: &[RemoteCodebaseIndexStatus],
    ) {
        self.statuses.retain(|key, _| {
            key.remote_identity_key != remote_identity_key || key.host_id != *host_id
        });
        for status in statuses {
            self.apply_status_update(remote_identity_key, host_id, status.clone());
        }
    }

    fn apply_status_update(
        &mut self,
        remote_identity_key: &str,
        host_id: &HostId,
        status: RemoteCodebaseIndexStatus,
    ) {
        self.statuses.insert(
            RemoteCodebaseIndexKey {
                remote_identity_key: remote_identity_key.to_string(),
                host_id: host_id.clone(),
                repo_path: status.repo_path.clone(),
            },
            status,
        );
    }

    fn record_navigated_directory(
        &mut self,
        session_id: SessionId,
        host_id: HostId,
        repo_path: String,
        is_git: bool,
    ) {
        self.active_repos_by_session.insert(
            session_id,
            ActiveRemoteRepo {
                host_id,
                repo_path,
                is_git,
            },
        );
    }

    fn remove_host(&mut self, host_id: &HostId) {
        self.statuses.retain(|key, _| key.host_id != *host_id);
        self.active_repos_by_session
            .retain(|_, repo| repo.host_id != *host_id);
    }

    fn availability_for_remote(
        &self,
        session_id: Option<SessionId>,
        host_id: &HostId,
        current_working_directory: Option<&str>,
        explicit_repo_path: Option<&str>,
    ) -> RemoteCodebaseSearchAvailability {
        let repo_path = explicit_repo_path
            .map(ToOwned::to_owned)
            .or_else(|| {
                session_id.and_then(|session_id| {
                    self.active_repos_by_session
                        .get(&session_id)
                        .filter(|repo| repo.host_id == *host_id && repo.is_git)
                        .map(|repo| repo.repo_path.clone())
                })
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

        availability_from_status(host_id.clone(), status)
    }

    fn status_for_repo(
        &self,
        host_id: &HostId,
        repo_path: &str,
    ) -> Option<&RemoteCodebaseIndexStatus> {
        self.statuses
            .iter()
            .filter(|(key, _)| key.host_id == *host_id && key.repo_path == repo_path)
            .map(|(_, status)| status)
            .max_by_key(|status| status.last_updated_epoch_millis.unwrap_or_default())
    }

    fn best_status_for_path(
        &self,
        host_id: &HostId,
        path: &str,
    ) -> Option<&RemoteCodebaseIndexStatus> {
        self.statuses
            .iter()
            .filter(|(key, _)| key.host_id == *host_id && path_is_within_repo(path, &key.repo_path))
            .map(|(_, status)| status)
            .max_by_key(|status| status.repo_path.len())
    }
}

impl Entity for RemoteCodebaseIndexModel {
    type Event = ();
}

impl SingletonEntity for RemoteCodebaseIndexModel {}

fn availability_from_status(
    host_id: HostId,
    status: &RemoteCodebaseIndexStatus,
) -> RemoteCodebaseSearchAvailability {
    let repo_path = status.repo_path.clone();
    match status.state {
        RemoteCodebaseIndexState::Ready | RemoteCodebaseIndexState::Stale => {
            let Some(root_hash) = status
                .root_hash
                .as_deref()
                .and_then(|hash| NodeHash::from_str(hash).ok())
            else {
                return RemoteCodebaseSearchAvailability::Unavailable {
                    repo_path,
                    message: "The remote codebase index is missing its root hash.".to_string(),
                };
            };
            let Some(embedding_config) = embedding_config_from_status(status) else {
                return RemoteCodebaseSearchAvailability::Unavailable {
                    repo_path,
                    message: "The remote codebase index is missing its embedding config."
                        .to_string(),
                };
            };
            RemoteCodebaseSearchAvailability::Ready(RemoteCodebaseSearchContext {
                host_id,
                repo_path,
                root_hash,
                embedding_config,
            })
        }
        RemoteCodebaseIndexState::Queued | RemoteCodebaseIndexState::Indexing => {
            RemoteCodebaseSearchAvailability::Indexing { repo_path }
        }
        RemoteCodebaseIndexState::Failed => RemoteCodebaseSearchAvailability::Failed {
            repo_path,
            message: status
                .failure_message
                .clone()
                .unwrap_or_else(|| "The remote codebase index failed.".to_string()),
        },
        RemoteCodebaseIndexState::NotEnabled
        | RemoteCodebaseIndexState::Unavailable
        | RemoteCodebaseIndexState::Disabled => RemoteCodebaseSearchAvailability::Unavailable {
            repo_path,
            message: status
                .failure_message
                .clone()
                .unwrap_or_else(|| "Remote codebase search is not available.".to_string()),
        },
    }
}

fn embedding_config_from_status(status: &RemoteCodebaseIndexStatus) -> Option<EmbeddingConfig> {
    let model = status.embedding_model.as_deref()?.to_ascii_lowercase();
    let dimensions = status.embedding_dimensions;
    match (model.as_str(), dimensions) {
        ("openaitextsmall3_256", Some(256))
        | ("openai_text_small_3", Some(256))
        | ("openai-text-small-3", Some(256))
        | ("text-embedding-3-small", Some(256))
        | ("openai_text_small_3_256", _)
        | ("openai-text-small-3-256", _)
        | ("openaitextsmall3256", _) => Some(EmbeddingConfig::OpenAiTextSmall3_256),
        ("voyagecode3_512", Some(512))
        | ("voyage_code_3", Some(512))
        | ("voyage-code-3", Some(512))
        | ("voyage_code_3_512", _)
        | ("voyage-code-3-512", _)
        | ("voyagecode3512", _) => Some(EmbeddingConfig::VoyageCode3_512),
        ("voyage3_5_lite_512", Some(512))
        | ("voyage_3_5_lite", Some(512))
        | ("voyage-3.5-lite", Some(512))
        | ("voyage35lite512", _) => Some(EmbeddingConfig::Voyage3_5_Lite_512),
        ("voyage3_5_512", Some(512))
        | ("voyage_3_5", Some(512))
        | ("voyage-3.5", Some(512))
        | ("voyage35512", _) => Some(EmbeddingConfig::Voyage3_5_512),
        _ => None,
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

fn path_is_within_repo(path: &str, repo_path: &str) -> bool {
    path == repo_path
        || path
            .strip_prefix(repo_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> HostId {
        HostId::new("host".to_string())
    }

    fn ready_status(repo_path: &str) -> RemoteCodebaseIndexStatus {
        RemoteCodebaseIndexStatus {
            repo_path: repo_path.to_string(),
            state: RemoteCodebaseIndexState::Ready,
            last_updated_epoch_millis: Some(1),
            progress_completed: None,
            progress_total: None,
            failure_message: None,
            root_hash: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            embedding_model: Some("voyage-3.5".to_string()),
            embedding_dimensions: Some(512),
        }
    }

    #[test]
    fn snapshot_replaces_statuses_for_identity_and_host() {
        let mut model = RemoteCodebaseIndexModel::default();
        let host = host();
        model.apply_status_update("identity", &host, ready_status("/old"));
        model.apply_statuses_snapshot("identity", &host, &[ready_status("/new")]);

        assert!(model.status_for_repo(&host, "/old").is_none());
        assert!(model.status_for_repo(&host, "/new").is_some());
    }

    #[test]
    fn availability_uses_active_navigated_repo() {
        let mut model = RemoteCodebaseIndexModel::default();
        let host = host();
        let session_id = SessionId::from(1);
        model.record_navigated_directory(session_id, host.clone(), "/repo".to_string(), true);
        model.apply_status_update("identity", &host, ready_status("/repo"));

        let availability =
            model.availability_for_remote(Some(session_id), &host, Some("/repo/src"), None);

        assert!(availability.is_ready());
        assert_eq!(availability.repo_path(), Some("/repo"));
    }

    #[test]
    fn availability_falls_back_to_longest_status_prefix() {
        let mut model = RemoteCodebaseIndexModel::default();
        let host = host();
        model.apply_status_update("identity", &host, ready_status("/repo"));
        model.apply_status_update("identity", &host, ready_status("/repo/nested"));

        let availability =
            model.availability_for_remote(None, &host, Some("/repo/nested/src"), None);

        assert!(availability.is_ready());
        assert_eq!(availability.repo_path(), Some("/repo/nested"));
    }

    #[test]
    fn indexing_state_is_not_ready() {
        let mut status = ready_status("/repo");
        status.state = RemoteCodebaseIndexState::Indexing;

        let availability = availability_from_status(host(), &status);

        assert!(matches!(
            availability,
            RemoteCodebaseSearchAvailability::Indexing { .. }
        ));
    }

    #[test]
    fn missing_embedding_config_is_unavailable() {
        let mut status = ready_status("/repo");
        status.embedding_model = None;

        let availability = availability_from_status(host(), &status);

        assert!(matches!(
            availability,
            RemoteCodebaseSearchAvailability::Unavailable { .. }
        ));
    }

    #[test]
    fn remote_auto_indexing_requires_feature_codebase_context_and_auto_indexing() {
        {
            let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
            let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(false);
            assert!(!remote_auto_indexing_enabled(true, true));
        }
        {
            let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
            let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
            assert!(remote_auto_indexing_enabled(true, true));
            assert!(!remote_auto_indexing_enabled(false, true));
            assert!(!remote_auto_indexing_enabled(true, false));
            assert!(!remote_auto_indexing_enabled(false, false));
        }
        {
            let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(false);
            let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
            assert!(!remote_auto_indexing_enabled(true, true));
        }
    }
}
