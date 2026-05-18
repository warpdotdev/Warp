use ai::index::full_source_code_embedding::EmbeddingConfig;
use instant::Instant;
use std::{path::PathBuf, time::Duration};

use warpui::{AppContext, ModelContext};

use crate::ai::{
    agent::{AIAgentActionId, SearchCodebaseFailureReason, SearchCodebaseResult},
    blocklist::SessionContext,
};
use crate::server::telemetry::{
    RemoteCodebaseSearchFailureStage, RemoteCodebaseSearchTelemetryResult,
};

use crate::ai::get_relevant_files::controller::GetRelevantFilesController;

pub(super) enum RemoteSearchRequest {
    Ready(RemoteSearchResponse),
}

pub(super) struct RemoteSearchResponse {
    pub result: SearchCodebaseResult,
    pub telemetry: RemoteSearchTelemetry,
}

pub(super) struct RemoteSearchTelemetry {
    pub result: RemoteCodebaseSearchTelemetryResult,
    pub total_search_duration: Duration,
    pub candidate_hash_count: Option<usize>,
    pub returned_file_count: Option<usize>,
    pub embedding_config: Option<EmbeddingConfig>,
    pub failure_stage: Option<RemoteCodebaseSearchFailureStage>,
}

pub(super) fn root_directory_for_search(
    _session_context: &SessionContext,
    _requested_codebase_path: Option<&str>,
    _app: &AppContext,
) -> Option<PathBuf> {
    None
}

pub(super) fn send_request(
    _query: String,
    _partial_paths: Option<Vec<String>>,
    _session_context: SessionContext,
    _requested_codebase_path: Option<String>,
    _action_id: AIAgentActionId,
    _ctx: &mut ModelContext<GetRelevantFilesController>,
) -> RemoteSearchRequest {
    let search_start = Instant::now();
    RemoteSearchRequest::Ready(RemoteSearchResponse {
        result: SearchCodebaseResult::Failed {
            reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
            message: "Remote codebase search is not available in this environment.".to_string(),
        },
        telemetry: RemoteSearchTelemetry {
            result: RemoteCodebaseSearchTelemetryResult::CodebaseNotIndexed,
            total_search_duration: search_start.elapsed(),
            candidate_hash_count: None,
            returned_file_count: None,
            embedding_config: None,
            failure_stage: None,
        },
    })
}
