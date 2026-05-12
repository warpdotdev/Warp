use futures::channel::oneshot;
use warpui::ModelContext;

use crate::ai::{
    agent::{AIAgentActionResultType, SearchCodebaseFailureReason, SearchCodebaseResult},
    blocklist::{action_model::execute::ActionExecution, SessionContext},
};

use super::SearchCodebaseExecutor;

pub(super) fn execute_remote_search(
    query: String,
    partial_paths: Option<Vec<String>>,
    explicit_repo_path: Option<&str>,
    session_context: SessionContext,
    ctx: &mut ModelContext<SearchCodebaseExecutor>,
) -> ActionExecution<Result<SearchCodebaseResult, oneshot::Canceled>> {
    let _ = (
        query,
        partial_paths,
        explicit_repo_path,
        session_context,
        ctx,
    );
    ActionExecution::Sync(AIAgentActionResultType::SearchCodebase(
        SearchCodebaseResult::Failed {
            reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
            message: "Remote codebase search is not available in this environment.".to_string(),
        },
    ))
}
