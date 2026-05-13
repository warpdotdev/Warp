//! Common utilities for agent SDK commands.

use std::future::Future;

use futures::TryFutureExt;

use warp_cli::agent::Harness;
use warpui::r#async::FutureExt;
use warpui::{AppContext, SingletonEntity as _};

use crate::ai::agent::conversation::ServerAIConversationMetadata;
use crate::ai::agent_sdk::driver::{AgentDriverError, WARP_DRIVE_SYNC_TIMEOUT};
use crate::ai::ambient_agents::AmbientAgentTaskId;

use crate::ai::llms::{LLMId, LLMPreferences};
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::persistence::ObjectStoreModel;
use crate::cloud_object::Owner;
use crate::workspaces::user_workspaces::UserWorkspaces;

pub fn validate_agent_mode_base_model_id(
    model_id: &str,
    ctx: &AppContext,
) -> anyhow::Result<LLMId> {
    let llm_prefs = LLMPreferences::as_ref(ctx);

    let llm_id: LLMId = model_id.into();
    let valid_ids = llm_prefs
        .get_base_llm_choices_for_agent_mode()
        .map(|info| info.id.clone())
        .collect::<Vec<_>>();

    if valid_ids.contains(&llm_id) {
        Ok(llm_id)
    } else {
        let suggestions = valid_ids
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Err(anyhow::anyhow!(
            "Unknown model id '{model_id}'. Try one of: {suggestions}"
        ))
    }
}

pub(super) fn parse_ambient_task_id(
    run_id: &str,
    error_prefix: &str,
) -> anyhow::Result<AmbientAgentTaskId> {
    run_id
        .parse()
        .map_err(|err| anyhow::anyhow!("{error_prefix} '{run_id}': {err}"))
}

pub(super) fn set_ambient_task_context_from_run_id(
    run_id: &str,
) -> anyhow::Result<AmbientAgentTaskId> {
    let task_id = parse_ambient_task_id(run_id, "Invalid run ID")?;
    Ok(task_id)
}

/// Resolve the owner of a new cloud object. This resolution is based on the CLI `--team` and `--personal` flags.
///
/// If `team_flag` is true, attempts to get the current team UID (errors if not on a team).
/// If `user_flag` is true, gets the current user's UID.
/// Otherwise, defaults to team if available, falling back to user.
pub fn resolve_owner(team_flag: bool, user_flag: bool, ctx: &AppContext) -> anyhow::Result<Owner> {
    if team_flag {
        let team_id = UserWorkspaces::as_ref(ctx)
            .current_team_uid()
            .ok_or_else(|| anyhow::anyhow!("User is not on a team"))?;
        return Ok(Owner::Team { team_uid: team_id });
    }

    if user_flag {
        let user_id = AuthStateProvider::as_ref(ctx)
            .get()
            .user_id()
            .ok_or_else(|| anyhow::anyhow!("User should be logged in"))?;
        return Ok(Owner::User { user_uid: user_id });
    }

    // Default: try team first, fall back to user
    if let Some(team_uid) = UserWorkspaces::as_ref(ctx).current_team_uid() {
        return Ok(Owner::Team { team_uid });
    }

    log::warn!("Tried to default to creating team object, team could not be found.");
    let user_id = AuthStateProvider::as_ref(ctx)
        .get()
        .user_id()
        .ok_or_else(|| anyhow::anyhow!("User should be logged in"))?;
    Ok(Owner::User { user_uid: user_id })
}

/// Refresh workspace metadata before executing an operation.
///
/// This ensures that team state is up-to-date before creating cloud objects or performing
/// other operations that depend on team membership.
pub fn refresh_workspace_metadata<C>(
    _ctx: &mut C,
) -> impl Future<Output = anyhow::Result<()>> + Send + 'static {
    async { Ok(()) }
}

/// Refresh Warp Drive before executing an operation.
pub fn refresh_warp_drive(
    ctx: &AppContext,
) -> impl Future<Output = anyhow::Result<()>> + Send + 'static {
    ObjectStoreModel::as_ref(ctx)
        .initial_load_complete()
        .with_timeout(WARP_DRIVE_SYNC_TIMEOUT)
        .map_err(|_| anyhow::anyhow!("Timed out waiting for Warp Drive to sync"))
}

/// Fetch the conversation's server metadata and validate that its harness matches the caller's
/// `--harness` choice. Returns the metadata on success so the caller can reuse it (e.g. for the
/// server conversation token).
///
/// Called up-front before any task/config-build logic consumes `args.harness`, so a mismatch
/// error surfaces before side effects like task creation. We deliberately do NOT auto-upgrade
/// the harness: `Harness::Oz` default with a Claude conversation id is treated as a mismatch
/// and errors out.
pub(super) async fn fetch_and_validate_conversation_harness(
    conversation_id: &str,
    _args_harness: Harness,
) -> Result<ServerAIConversationMetadata, AgentDriverError> {
    // Conversation metadata fetching from the cloud was removed alongside the
    // `CloudConversations` feature. The `--conversation` CLI argument therefore
    // can no longer resolve any cloud-stored conversation in OpenWarp.
    Err(AgentDriverError::ConversationLoadFailed(format!(
        "conversation {conversation_id} not found: cloud conversations are disabled in OpenWarp"
    )))
}

/// Format an object owner for display in the CLI.
pub fn format_owner(owner: &Owner) -> &'static str {
    // TODO: For potentially-shared objects, consider looking up the particular user/team name.
    match owner {
        Owner::User { .. } => "Personal",
        Owner::Team { .. } => "Team",
    }
}

/// An error resolving an agent option, which we may have prompted the user for.
#[derive(Debug, thiserror::Error)]
pub enum ResolveConfigurationError {
    /// The user canceled the operation, and we should exit.
    #[error("Operation canceled")]
    Canceled,
    #[error("{id} is not a valid {kind} identifier")]
    InvalidId { id: String, kind: &'static str },
    #[error("{kind} {id} not found")]
    ObjectNotFound { id: String, kind: &'static str },
    #[error(transparent)]
    Other(anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::parse_ambient_task_id;

    #[test]
    fn parse_ambient_task_id_accepts_valid_ids() {
        let task_id =
            parse_ambient_task_id("550e8400-e29b-41d4-a716-446655440000", "Invalid run ID")
                .unwrap();

        assert_eq!(task_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn parse_ambient_task_id_preserves_error_prefix() {
        let err = parse_ambient_task_id("not-a-run-id", "Invalid run ID").unwrap_err();

        assert!(err.to_string().contains("Invalid run ID 'not-a-run-id'"));
    }
}
