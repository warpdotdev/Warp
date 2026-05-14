use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::{
    AIAgentHarness, AIConversationId, ServerAIConversationMetadata,
};
use crate::ai::agent_conversations_model::AgentConversationsModel;
use crate::ai::ambient_agents::{
    conversation_output_status_from_conversation, AmbientAgentTaskId, AmbientConversationStatus,
};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::auth::AuthStateProvider;
use crate::cloud_object::{Owner, ServerGuestSubject};
use crate::drive::sharing::SharingAccessLevel;
use crate::workspaces::user_workspaces::UserWorkspaces;
use warpui::{AppContext, EntityId, SingletonEntity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TombstoneCta {
    ContinueLocally { conversation_id: AIConversationId },
    ContinueInCloud { task_id: AmbientAgentTaskId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::terminal::view) enum CloudConversationContinuationUiState {
    FollowupInput,
    Tombstone { cta: Option<TombstoneCta> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::terminal::view) enum CloudConversationContinuationError {
    MissingTask,
    ActiveTaskExecution,
    MissingConversationToken,
    MissingServerConversationMetadata,
    UnknownHarness,
    UnknownConversationAccess,
}

impl CloudConversationContinuationError {
    pub(in crate::terminal::view) fn should_fallback_to_tombstone(self) -> bool {
        !matches!(self, Self::ActiveTaskExecution)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConversationAccess {
    Edit,
    ViewOnly,
    Unknown,
}

pub(in crate::terminal::view) fn resolve_cloud_conversation_continuation_ui_state(
    terminal_view_id: EntityId,
    task_id: AmbientAgentTaskId,
    app: &AppContext,
) -> Result<CloudConversationContinuationUiState, CloudConversationContinuationError> {
    let Some(task) = AgentConversationsModel::as_ref(app).get_task_data(&task_id) else {
        return Err(CloudConversationContinuationError::MissingTask);
    };
    if task.has_active_execution() {
        return Err(CloudConversationContinuationError::ActiveTaskExecution);
    }
    if task.state.is_failure_like()
        && task
            .status_message
            .as_ref()
            .is_some_and(|status_message| status_message.is_environment_setup_failure())
    {
        return Ok(CloudConversationContinuationUiState::Tombstone { cta: None });
    }

    let Some(conversation_token) = task
        .conversation_id()
        .map(|token| ServerConversationToken::new(token.to_string()))
    else {
        return Err(CloudConversationContinuationError::MissingConversationToken);
    };
    let history_model = BlocklistAIHistoryModel::as_ref(app);
    let Some(metadata) =
        history_model.get_server_conversation_metadata_by_server_token(&conversation_token)
    else {
        return Err(CloudConversationContinuationError::MissingServerConversationMetadata);
    };

    let access = conversation_access(metadata, app);
    match (metadata.harness, access) {
        (AIAgentHarness::Oz, ConversationAccess::Edit) => {
            Ok(CloudConversationContinuationUiState::FollowupInput)
        }
        (AIAgentHarness::Oz, ConversationAccess::ViewOnly) => {
            let local_conversation_id = local_conversation_id_for_local_continuation(
                terminal_view_id,
                &conversation_token,
                history_model,
            );
            Ok(CloudConversationContinuationUiState::Tombstone {
                cta: local_conversation_id
                    .map(|conversation_id| TombstoneCta::ContinueLocally { conversation_id }),
            })
        }
        (
            AIAgentHarness::ClaudeCode | AIAgentHarness::Gemini | AIAgentHarness::Codex,
            ConversationAccess::Edit,
        ) => Ok(CloudConversationContinuationUiState::Tombstone {
            cta: Some(TombstoneCta::ContinueInCloud { task_id }),
        }),
        (
            AIAgentHarness::ClaudeCode | AIAgentHarness::Gemini | AIAgentHarness::Codex,
            ConversationAccess::ViewOnly,
        ) => Ok(CloudConversationContinuationUiState::Tombstone { cta: None }),
        (AIAgentHarness::Unknown, _) => Err(CloudConversationContinuationError::UnknownHarness),
        (_, ConversationAccess::Unknown) => {
            Err(CloudConversationContinuationError::UnknownConversationAccess)
        }
    }
}

fn conversation_access(
    metadata: &ServerAIConversationMetadata,
    app: &AppContext,
) -> ConversationAccess {
    let Some(current_user_uid) = AuthStateProvider::as_ref(app).get().user_id() else {
        return ConversationAccess::Unknown;
    };

    let mut access_level = SharingAccessLevel::View;
    match metadata.permissions.space {
        Owner::User { user_uid } => {
            let is_current_user_owner = user_uid == current_user_uid;
            if is_current_user_owner {
                access_level = access_level.max(SharingAccessLevel::Full);
            }
        }
        Owner::Team { team_uid } => {
            let is_current_team_owner = UserWorkspaces::as_ref(app)
                .team_from_uid_across_all_workspaces(team_uid)
                .is_some();
            if is_current_team_owner {
                access_level = access_level.max(SharingAccessLevel::Full);
            }
        }
    }

    if let Some(link_sharing) = &metadata.permissions.anyone_link_sharing {
        access_level = access_level.max(link_sharing.access_level.into());
    }

    // Direct user and team ACLs can both apply, so use the highest matching grant.
    for guest in &metadata.permissions.guests {
        match &guest.subject {
            ServerGuestSubject::User { firebase_uid } => {
                let matches_current_user = firebase_uid == current_user_uid.as_str();
                if matches_current_user {
                    access_level = access_level.max(guest.access_level.into());
                }
            }
            ServerGuestSubject::Team { team_uid } => {
                let matches_current_team = UserWorkspaces::as_ref(app)
                    .team_from_uid_across_all_workspaces(*team_uid)
                    .is_some();
                if matches_current_team {
                    access_level = access_level.max(guest.access_level.into());
                }
            }
            ServerGuestSubject::PendingUser { .. } => {}
        }
    }
    let is_creator = metadata
        .metadata
        .creator_uid
        .as_ref()
        .is_some_and(|creator_uid| creator_uid == current_user_uid.as_str());
    if is_creator {
        access_level = access_level.max(SharingAccessLevel::Edit);
    }
    if access_level >= SharingAccessLevel::Edit {
        ConversationAccess::Edit
    } else {
        ConversationAccess::ViewOnly
    }
}

pub(in crate::terminal::view) fn conversation_failed_before_task_creation(
    terminal_view_id: EntityId,
    history_model: &BlocklistAIHistoryModel,
) -> bool {
    if history_model.is_terminal_view_conversation_transcript_viewer(terminal_view_id) {
        return false;
    }
    history_model
        .all_live_conversations_for_terminal_view(terminal_view_id)
        .next()
        .and_then(conversation_output_status_from_conversation)
        .is_some_and(|status| matches!(status, AmbientConversationStatus::Error { .. }))
}

fn local_conversation_id_for_local_continuation(
    terminal_view_id: EntityId,
    conversation_token: &ServerConversationToken,
    history_model: &BlocklistAIHistoryModel,
) -> Option<AIConversationId> {
    history_model
        .all_live_conversations_for_terminal_view(terminal_view_id)
        .find(|conversation| conversation.server_conversation_token() == Some(conversation_token))
        .map(|conversation| conversation.id())
        .or_else(|| history_model.find_conversation_id_by_server_token(conversation_token))
}

#[cfg(test)]
#[path = "cloud_conversation_continuation_tests.rs"]
mod tests;
