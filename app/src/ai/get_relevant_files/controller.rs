use ai::index::{
    full_source_code_embedding::{
        manager::{CodebaseIndexManager, CodebaseIndexManagerEvent},
        RetrievalID,
    },
    locations::CodeContextLocation,
};
use anyhow::anyhow;
use futures_util::stream::AbortHandle;
use instant::Instant;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use warp_core::features::FeatureFlag;

use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use crate::{
    ai::{
        agent::AIAgentActionId,
        get_relevant_files::api::{FileContext, GetRelevantFiles},
        outline::{OutlineStatus, RepoOutlines},
    },
    report_error, send_telemetry_from_ctx,
    server::server_api::{AIApiError, ServerApiProvider},
    TelemetryEvent,
};

#[derive(Debug)]
pub enum GetRelevantFilesControllerEvent {
    Success {
        action_id: AIAgentActionId,
        fragments: Arc<HashSet<CodeContextLocation>>,
    },
    Error {
        action_id: AIAgentActionId,
    },
}

impl GetRelevantFilesControllerEvent {
    pub fn action_id(&self) -> &AIAgentActionId {
        match self {
            GetRelevantFilesControllerEvent::Success { action_id, .. } => action_id,
            GetRelevantFilesControllerEvent::Error { action_id } => action_id,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetRelevantFilesError {
    #[error("Repo outline is still being computed.")]
    Pending,
    #[error("Failed to create outline.")]
    CreateFailed,
    #[error("Failed to create outline.")]
    Missing,
}

/// This enum allows us to use both the existing structure for outline-based indexing
/// and the new full source code indexing manager/model.
enum RequestHandle {
    /// Used with outline-based indexing.
    AbortHandle(AbortHandle),

    /// Used with full source code indexing.
    RetrievalID {
        repo_path: PathBuf,
        retrieval_id: RetrievalID,
        start_time: Instant,
    },
}

impl RequestHandle {
    fn abort(&mut self, ctx: &mut AppContext) {
        match self {
            RequestHandle::AbortHandle(abort_handle) => abort_handle.abort(),
            RequestHandle::RetrievalID {
                repo_path,
                retrieval_id,
                start_time: _,
            } => {
                CodebaseIndexManager::handle(ctx).update(ctx, |index_manager, ctx| {
                    if let Err(err) =
                        index_manager.abort_retrieval_request(repo_path, retrieval_id.clone(), ctx)
                    {
                        log::error!("Failed to abort file retrieval request: {err:?}");
                    }
                });
            }
        }
    }
}

/// Controller for GetRelevantFiles action. This is scoped per terminal session.
#[derive(Default)]
pub struct GetRelevantFilesController {
    /// Search requests currently in flight, keyed by the originating action ID.
    /// This allows several SearchCodebase actions to be active at once without newer requests
    /// cancelling unrelated older ones.
    pending_requests: std::collections::HashMap<AIAgentActionId, RequestHandle>,
}

impl GetRelevantFilesController {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let codebase_manager = CodebaseIndexManager::handle(ctx);
        ctx.subscribe_to_model(&codebase_manager, Self::handle_codebase_manager_event);
        Self::default()
    }

    fn pending_request_details_for_retrieval_id(
        &self,
        pending_retrieval_id: &RetrievalID,
    ) -> Option<(&AIAgentActionId, &Instant)> {
        // Full-source embedding completion events only carry the retrieval ID, so map them back to
        // the agent action that initiated the request before emitting results/telemetry.
        self.pending_requests
            .iter()
            .find_map(|(action_id, request_handle)| match request_handle {
                RequestHandle::AbortHandle(_) => None,
                RequestHandle::RetrievalID {
                    retrieval_id,
                    start_time,
                    ..
                } if retrieval_id == pending_retrieval_id => Some((action_id, start_time)),
                RequestHandle::RetrievalID { .. } => None,
            })
    }

    fn handle_codebase_manager_event(
        &mut self,
        codebase_manager_event: &CodebaseIndexManagerEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        match codebase_manager_event {
            CodebaseIndexManagerEvent::RetrievalRequestFailed {
                retrieval_id,
                error_message: error,
            } => {
                let Some((action_id, _search_start)) =
                    self.pending_request_details_for_retrieval_id(retrieval_id)
                else {
                    return;
                };
                send_telemetry_from_ctx!(
                    TelemetryEvent::FullEmbedCodebaseContextSearchFailed {
                        action_id: action_id.clone(),
                        error: error.to_string(),
                    },
                    ctx
                );

                self.handle_relevant_file_paths_result(
                    Err(anyhow!(error.to_owned())),
                    action_id.clone(),
                    ctx,
                );
            }
            CodebaseIndexManagerEvent::RetrievalRequestCompleted {
                retrieval_id,
                fragments,
                out_of_sync_delay,
            } => {
                let Some((action_id, search_start)) =
                    self.pending_request_details_for_retrieval_id(retrieval_id)
                else {
                    return;
                };
                send_telemetry_from_ctx!(
                    TelemetryEvent::FullEmbedCodebaseContextSearchSuccess {
                        action_id: action_id.clone(),
                        total_search_duration: search_start.elapsed(),
                        out_of_sync_delay: *out_of_sync_delay,
                    },
                    ctx
                );

                self.handle_relevant_file_paths_result(
                    Ok(fragments.clone()),
                    action_id.clone(),
                    ctx,
                );
            }
            _ => (),
        }
    }

    /// Start a new search query based on the repo outline.
    pub fn send_request(
        &mut self,
        directory: &Path,
        query: String,
        partial_path_segments: Option<&Vec<String>>,
        action_id: AIAgentActionId,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), GetRelevantFilesError> {
        const MINIMUM_FILE_COUNT_FOR_API_CALL: usize = 2;
        self.cancel_request_for_action(&action_id, ctx);

        if FeatureFlag::FullSourceCodeEmbedding.is_enabled() {
            let codebase_mgr = CodebaseIndexManager::handle(ctx);
            if let Some(base_path) = codebase_mgr.as_ref(ctx).root_path_for_codebase(directory) {
                match codebase_mgr.update(ctx, |index_manager, ctx| {
                    index_manager.retrieve_relevant_files(query.clone(), base_path.as_path(), ctx)
                }) {
                    Ok(retrieval_request_id) => {
                        log::info!("Using full source code embedding for search");
                        let search_start = Instant::now();
                        self.pending_requests.insert(
                            action_id,
                            RequestHandle::RetrievalID {
                                repo_path: base_path.clone(),
                                retrieval_id: retrieval_request_id,
                                start_time: search_start,
                            },
                        );

                        return Ok(());
                    }
                    Err(e) => {
                        log::info!(
                            "Failed to initiate full source code search: {e}, falling back to outline-based search"
                        );
                    }
                }
            }
        }

        match RepoOutlines::as_ref(ctx).get_outline(directory) {
            Some((OutlineStatus::Complete(outline), base_path)) => {
                let server_api = ServerApiProvider::as_ref(ctx).get();

                let file_outlines = outline.to_file_symbols(partial_path_segments);
                if file_outlines.len() < MINIMUM_FILE_COUNT_FOR_API_CALL {
                    ctx.emit(GetRelevantFilesControllerEvent::Success {
                        action_id,
                        fragments: Arc::new(
                            file_outlines
                                .into_iter()
                                .map(|file| {
                                    CodeContextLocation::WholeFile(PathBuf::from(file.path))
                                })
                                .collect(),
                        ),
                    });
                } else {
                    let outline_request = GetRelevantFiles {
                        query,
                        files: file_outlines
                            .into_iter()
                            .map(|outline| FileContext {
                                path: outline.path,
                                symbols: outline.symbols,
                            })
                            .collect(),
                    };
                    let action_id_clone = action_id.clone();
                    let request_abort_handle = ctx
                        .spawn(
                            async move {
                                let response =
                                    server_api.get_relevant_files(&outline_request).await?;
                                Ok(Arc::new(
                                    response
                                        .relevant_file_paths
                                        .into_iter()
                                        .filter_map(|path| {
                                            let file_path = base_path.join(path);
                                            // Validate the returned file paths.
                                            if file_path.exists() {
                                                Some(CodeContextLocation::WholeFile(file_path))
                                            } else {
                                                None
                                            }
                                        })
                                        .collect(),
                                ))
                            },
                            move |me,
                                  relevant_file_paths: Result<
                                Arc<HashSet<CodeContextLocation>>,
                                AIApiError,
                            >,
                                  ctx| {
                                me.handle_relevant_file_paths_result(
                                    relevant_file_paths.map_err(|e| anyhow!(e)),
                                    action_id_clone,
                                    ctx,
                                )
                            },
                        )
                        .abort_handle();
                    self.pending_requests
                        .insert(action_id, RequestHandle::AbortHandle(request_abort_handle));
                }
                Ok(())
            }
            Some((OutlineStatus::Pending, _)) => Err(GetRelevantFilesError::Pending),
            Some((OutlineStatus::Failed, _)) => Err(GetRelevantFilesError::CreateFailed),
            None => Err(GetRelevantFilesError::Missing),
        }
    }

    fn handle_relevant_file_paths_result(
        &mut self,
        relevant_file_locations: anyhow::Result<Arc<HashSet<CodeContextLocation>>>,
        action_id: AIAgentActionId,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.pending_requests.remove(&action_id).is_none() {
            return;
        }
        match relevant_file_locations {
            Ok(relevant_file_locations) => {
                ctx.emit(GetRelevantFilesControllerEvent::Success {
                    action_id,
                    fragments: relevant_file_locations,
                });
            }
            Err(e) => {
                report_error!(anyhow!(e).context("get_relevant_files failed"));
                ctx.emit(GetRelevantFilesControllerEvent::Error { action_id });
            }
        };
    }

    /// Returns the path to the root directory for a codebase search where pwd is `directory`.
    pub fn root_directory_for_search(&self, directory: &Path, app: &AppContext) -> Option<PathBuf> {
        let mut start = None;
        if FeatureFlag::FullSourceCodeEmbedding.is_enabled() {
            start = CodebaseIndexManager::as_ref(app).root_path_for_codebase(directory);
        }
        start.or_else(|| {
            RepoOutlines::as_ref(app)
                .get_outline(directory)
                .map(|(_, root)| root)
        })
    }

    pub fn cancel_request_for_action(
        &mut self,
        action_id: &AIAgentActionId,
        ctx: &mut ModelContext<Self>,
    ) {
        if let Some(mut request_handle) = self.pending_requests.remove(action_id) {
            request_handle.abort(ctx);
        }
    }
}

impl Entity for GetRelevantFilesController {
    type Event = GetRelevantFilesControllerEvent;
}
