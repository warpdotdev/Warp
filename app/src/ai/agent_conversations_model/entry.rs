use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::ambient_agents::{AgentSource, AmbientAgentTask, AmbientAgentTaskId};
use crate::ai::artifacts::Artifact;
use crate::ai::blocklist::history_model::{AIConversationMetadata, BlocklistAIHistoryModel};
use crate::ai::conversation_navigation::ConversationNavigationData;
use crate::auth::AuthStateProvider;
use chrono::{DateTime, Utc};
use session_sharing_protocol::common::SessionId;
use warp_cli::agent::Harness;
use warpui::{AppContext, SingletonEntity};

use super::{
    artifacts_match_filter, AgentManagementFilters, AgentRunDisplayStatus, ArtifactFilter,
    ConversationMetadata, ConversationOrTask, CreatedOnFilter, CreatorFilter, EnvironmentFilter,
    HarnessFilter, OwnerFilter, SourceFilter, StatusFilter,
};

/// Stable projection identity used by list and navigation surfaces.
///
/// Task-backed rows use the ambient run ID even when they are attached to a local
/// conversation, so task-specific affordances do not disappear when local data is present.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AgentConversationEntryId {
    AmbientRun(AmbientAgentTaskId),
    Conversation(AIConversationId),
}

/// Normalized row data for agent conversation list, management, and navigation surfaces.
///
/// The entry keeps local conversation identity, ambient run identity, cloud token identity,
/// display fields, and available actions together so callers do not recompute navigation
/// policy from stale partial sources.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentConversationEntry {
    pub id: AgentConversationEntryId,
    pub identity: AgentConversationIdentity,
    pub provenance: AgentConversationProvenance,
    pub display: AgentConversationDisplayData,
    pub backing: AgentConversationBackingData,
    pub capabilities: AgentConversationCapabilities,
}

/// Cross-system identifiers that may refer to the same underlying conversation/run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentConversationIdentity {
    pub local_conversation_id: Option<AIConversationId>,
    pub ambient_agent_task_id: Option<AmbientAgentTaskId>,
    pub server_conversation_token: Option<ServerConversationToken>,
    pub session_id: Option<SessionId>,
}

/// Display-only fields for rendering a conversation entry without consulting source models.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentConversationDisplayData {
    pub title: String,
    pub initial_query: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub status: AgentRunDisplayStatus,
    pub creator: AgentConversationCreator,
    pub request_usage: Option<f32>,
    pub run_time: Option<String>,
    pub source: Option<AgentSource>,
    pub working_directory: Option<String>,
    pub environment_id: Option<String>,
    pub harness: Option<Harness>,
    pub artifacts: Vec<Artifact>,
}

/// Creator information normalized across local conversations and ambient runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentConversationCreator {
    pub name: Option<String>,
    pub uid: Option<String>,
}

/// Source category that explains why an entry exists and which backing systems can refresh it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentConversationProvenance {
    LocalInteractive,
    AmbientRun,
    CloudSyncedConversation,
}

/// Availability flags for the source data that contributed to an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentConversationBackingData {
    pub has_loaded_conversation: bool,
    pub has_local_persisted_data: bool,
    pub has_cloud_data: bool,
    pub has_ambient_run: bool,
}

/// Actions that should be exposed for an entry after applying current navigation policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentConversationCapabilities {
    pub can_open: bool,
    pub can_copy_link: bool,
    pub can_share: bool,
    pub can_delete: bool,
    pub can_fork_locally: bool,
    pub can_cancel: bool,
}

impl AgentConversationEntry {
    pub(super) fn matches_filters(
        &self,
        filters: &AgentManagementFilters,
        app: &AppContext,
    ) -> bool {
        self.matches_owner_and_creator(&filters.owners, &filters.creator, app)
            && self.matches_status(&filters.status)
            && self.matches_source(&filters.source)
            && self.matches_created_on(&filters.created_on)
            && self.matches_artifact(&filters.artifact)
            && self.matches_environment(&filters.environment)
            && self.matches_harness(&filters.harness)
    }

    fn matches_owner_and_creator(
        &self,
        owner_filter: &OwnerFilter,
        creator_filter: &CreatorFilter,
        app: &AppContext,
    ) -> bool {
        let current_user_id = AuthStateProvider::as_ref(app)
            .get()
            .user_id()
            .map(|uid| uid.as_string());

        let passes_owner = match owner_filter {
            OwnerFilter::All => true,
            OwnerFilter::PersonalOnly => {
                if self.backing.has_ambient_run {
                    self.display.creator.uid == current_user_id
                } else {
                    true
                }
            }
        };

        if !passes_owner || matches!(owner_filter, OwnerFilter::PersonalOnly) {
            return passes_owner;
        }

        match creator_filter {
            CreatorFilter::All => true,
            CreatorFilter::Specific { name, .. } => {
                self.display.creator.name.as_ref() == Some(name)
            }
        }
    }

    fn matches_status(&self, status_filter: &StatusFilter) -> bool {
        match status_filter {
            StatusFilter::All => true,
            StatusFilter::Working | StatusFilter::Done | StatusFilter::Failed => {
                self.display.status.status_filter() == *status_filter
            }
        }
    }

    fn matches_source(&self, source_filter: &SourceFilter) -> bool {
        match source_filter {
            SourceFilter::All => true,
            SourceFilter::Specific(source) => self.display.source.as_ref() == Some(source),
        }
    }

    fn matches_created_on(&self, created_on_filter: &CreatedOnFilter) -> bool {
        let now = Utc::now();
        let created_cutoff = match created_on_filter {
            CreatedOnFilter::All => None,
            CreatedOnFilter::Last24Hours => Some(now - chrono::Duration::hours(24)),
            CreatedOnFilter::Past3Days => Some(now - chrono::Duration::days(3)),
            CreatedOnFilter::LastWeek => Some(now - chrono::Duration::days(7)),
        };
        match created_cutoff {
            Some(cutoff) => self.display.created_at >= cutoff,
            None => true,
        }
    }

    fn matches_artifact(&self, artifact_filter: &ArtifactFilter) -> bool {
        artifacts_match_filter(&self.display.artifacts, artifact_filter)
    }

    fn matches_environment(&self, environment_filter: &EnvironmentFilter) -> bool {
        match environment_filter {
            EnvironmentFilter::All => true,
            EnvironmentFilter::NoEnvironment => self.display.environment_id.is_none(),
            EnvironmentFilter::Specific(id) => self.display.environment_id.as_ref() == Some(id),
        }
    }

    fn matches_harness(&self, harness_filter: &HarnessFilter) -> bool {
        match harness_filter {
            HarnessFilter::All => true,
            HarnessFilter::Specific(harness) => self.display.harness == Some(*harness),
        }
    }
}

/// Returns the local conversation ID represented by the given task, if this task and a
/// conversation entry both point at the same underlying local run.
///
/// We first match using the orchestration agent ID (task ID / run ID under v2), and fall back
/// to the server conversation token for cases where the task only carries conversation identity
/// through `conversation_id`.
pub(super) fn conversation_id_shadowed_by_task(
    task: &AmbientAgentTask,
    history_model: &BlocklistAIHistoryModel,
) -> Option<AIConversationId> {
    history_model
        .conversation_id_for_agent_id(&task.run_id().to_string())
        .or_else(|| {
            task.conversation_id().and_then(|conversation_id| {
                history_model.find_conversation_id_by_server_token(&ServerConversationToken::new(
                    conversation_id.to_string(),
                ))
            })
        })
}

pub(super) fn entry_for_task(
    task: &AmbientAgentTask,
    history_model: &BlocklistAIHistoryModel,
    app: &AppContext,
) -> AgentConversationEntry {
    let item = ConversationOrTask::Task(task);
    let local_conversation_id = conversation_id_shadowed_by_task(task, history_model);
    let conversation_metadata =
        local_conversation_id.and_then(|id| history_model.get_conversation_metadata(&id));
    let server_conversation_token = task
        .conversation_id()
        .map(|id| ServerConversationToken::new(id.to_string()))
        .or_else(|| {
            local_conversation_id.and_then(|conversation_id| {
                server_conversation_token_for_conversation(conversation_id, None, history_model)
            })
        });
    let status = item.display_status(app);

    AgentConversationEntry {
        id: AgentConversationEntryId::AmbientRun(task.task_id),
        identity: AgentConversationIdentity {
            local_conversation_id,
            ambient_agent_task_id: Some(task.task_id),
            server_conversation_token,
            session_id: item.session_id(),
        },
        provenance: AgentConversationProvenance::AmbientRun,
        display: AgentConversationDisplayData {
            title: item.title(app),
            initial_query: Some(task.prompt.clone()),
            created_at: item.created_at(),
            last_updated: item.last_updated(),
            status: status.clone(),
            creator: AgentConversationCreator {
                name: item.creator_name(app),
                uid: item.creator_uid(app),
            },
            request_usage: item.request_usage(app),
            run_time: item.run_time(),
            source: item.source().cloned(),
            working_directory: None,
            environment_id: item.environment_id().map(ToString::to_string),
            harness: item.harness(app),
            artifacts: item.artifacts(app),
        },
        backing: AgentConversationBackingData {
            has_loaded_conversation: local_conversation_id
                .is_some_and(|id| history_model.conversation(&id).is_some()),
            has_local_persisted_data: conversation_metadata
                .is_some_and(|metadata| metadata.has_local_data),
            has_cloud_data: conversation_metadata.is_some_and(|metadata| metadata.has_cloud_data)
                || task.conversation_id().is_some(),
            has_ambient_run: true,
        },
        capabilities: AgentConversationCapabilities {
            can_open: item.get_open_action(None, app).is_some(),
            can_copy_link: item.session_or_conversation_link(app).is_some(),
            can_share: task.conversation_id().is_some()
                || local_conversation_id
                    .is_some_and(|id| history_model.can_conversation_be_shared(&id)),
            can_delete: false,
            can_fork_locally: local_conversation_id.is_some(),
            can_cancel: status.is_cancellable(),
        },
    }
}

pub(super) fn entry_for_conversation(
    metadata: &ConversationMetadata,
    history_model: &BlocklistAIHistoryModel,
    app: &AppContext,
) -> AgentConversationEntry {
    let conversation_metadata = history_model.get_conversation_metadata(&metadata.nav_data.id);
    entry_for_conversation_parts(
        metadata.nav_data.clone(),
        conversation_metadata,
        history_model,
        app,
    )
}

pub(super) fn entry_for_historical_metadata(
    metadata: &AIConversationMetadata,
    nav_data: ConversationNavigationData,
    history_model: &BlocklistAIHistoryModel,
    app: &AppContext,
) -> AgentConversationEntry {
    entry_for_conversation_parts(nav_data, Some(metadata), history_model, app)
}

fn entry_for_conversation_parts(
    nav_data: ConversationNavigationData,
    conversation_metadata: Option<&AIConversationMetadata>,
    history_model: &BlocklistAIHistoryModel,
    app: &AppContext,
) -> AgentConversationEntry {
    let metadata = ConversationMetadata { nav_data };
    let item = ConversationOrTask::Conversation(&metadata);
    let conversation_id = metadata.nav_data.id;
    let status = item.display_status(app);
    let has_loaded_conversation = history_model.conversation(&conversation_id).is_some();
    let has_local_persisted_data = conversation_metadata
        .is_some_and(|metadata| metadata.has_local_data)
        || has_loaded_conversation;
    let has_cloud_data = conversation_metadata.is_some_and(|metadata| metadata.has_cloud_data)
        || server_conversation_token_for_conversation(
            conversation_id,
            Some(&metadata.nav_data),
            history_model,
        )
        .is_some();
    let provenance = if has_cloud_data {
        AgentConversationProvenance::CloudSyncedConversation
    } else {
        AgentConversationProvenance::LocalInteractive
    };

    AgentConversationEntry {
        id: AgentConversationEntryId::Conversation(conversation_id),
        identity: AgentConversationIdentity {
            local_conversation_id: Some(conversation_id),
            ambient_agent_task_id: conversation_metadata
                .and_then(|metadata| metadata.server_conversation_metadata.as_ref())
                .and_then(|metadata| metadata.ambient_agent_task_id),
            server_conversation_token: server_conversation_token_for_conversation(
                conversation_id,
                Some(&metadata.nav_data),
                history_model,
            ),
            session_id: None,
        },
        provenance,
        display: AgentConversationDisplayData {
            title: item.title(app),
            initial_query: metadata.nav_data.initial_query.clone(),
            created_at: item.created_at(),
            last_updated: item.last_updated(),
            status: status.clone(),
            creator: AgentConversationCreator {
                name: item.creator_name(app),
                uid: item.creator_uid(app),
            },
            request_usage: item.request_usage(app),
            run_time: item.run_time(),
            source: item.source().cloned(),
            working_directory: metadata
                .nav_data
                .latest_working_directory
                .clone()
                .or_else(|| metadata.nav_data.initial_working_directory.clone()),
            environment_id: None,
            harness: conversation_metadata
                .and_then(|metadata| metadata.server_conversation_metadata.as_ref())
                .map(|metadata| Harness::from(metadata.harness))
                .or_else(|| item.harness(app)),
            artifacts: item.artifacts(app),
        },
        backing: AgentConversationBackingData {
            has_loaded_conversation,
            has_local_persisted_data,
            has_cloud_data,
            has_ambient_run: conversation_metadata
                .is_some_and(AIConversationMetadata::is_ambient_agent_conversation),
        },
        capabilities: AgentConversationCapabilities {
            can_open: item.get_open_action(None, app).is_some(),
            can_copy_link: item.session_or_conversation_link(app).is_some(),
            can_share: history_model.can_conversation_be_shared(&conversation_id),
            can_delete: has_local_persisted_data,
            can_fork_locally: has_local_persisted_data,
            can_cancel: status.is_cancellable(),
        },
    }
}

fn server_conversation_token_for_conversation(
    conversation_id: AIConversationId,
    nav_data: Option<&ConversationNavigationData>,
    history_model: &BlocklistAIHistoryModel,
) -> Option<ServerConversationToken> {
    history_model
        .conversation(&conversation_id)
        .and_then(|conversation| conversation.server_conversation_token())
        .cloned()
        .or_else(|| {
            history_model
                .get_conversation_metadata(&conversation_id)
                .and_then(|metadata| metadata.server_conversation_token.clone())
        })
        .or_else(|| nav_data.and_then(|nav_data| nav_data.server_conversation_token.clone()))
}
