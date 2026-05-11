use ::ai::index::full_source_code_embedding::{
    store_client::StoreClient, ContentHash, Fragment, RepoMetadata,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use futures::{channel::oneshot, future::BoxFuture, FutureExt};
use itertools::Itertools;
use remote_server::proto::{
    file_context_proto, FragmentMetadata, LineRange, ReadFileContextFile, ReadFileContextRequest,
    ReadFileContextResponse,
};
use warpui::{AppContext, Entity, EntityId, ModelContext, ModelHandle, SingletonEntity};

use crate::{
    ai::{
        agent::{
            AIAgentAction, AIAgentActionId, AIAgentActionResultType, AIAgentActionType,
            AnyFileContent, FileContext, SearchCodebaseFailureReason, SearchCodebaseRequest,
            SearchCodebaseResult,
        },
        blocklist::SessionContext,
        blocklist::{action_model::execute::get_server_output_id, BlocklistAIPermissions},
        get_relevant_files::controller::{
            GetRelevantFilesController, GetRelevantFilesControllerEvent, GetRelevantFilesError,
        },
    },
    features::FeatureFlag,
    remote_server::codebase_index_model::{
        RemoteCodebaseIndexModel, RemoteCodebaseSearchAvailability, RemoteCodebaseSearchContext,
    },
    send_telemetry_from_ctx,
    server::server_api::ServerApiProvider,
    terminal::model::session::active_session::ActiveSession,
    TelemetryEvent,
};

use super::{
    read_local_file_context, ActionExecution, AnyActionExecution, ExecuteActionInput,
    PreprocessActionInput,
};

pub struct SearchCodebaseExecutor {
    active_session: ModelHandle<ActiveSession>,
    get_relevant_files_controller: ModelHandle<GetRelevantFilesController>,
    /// Per-action response channels for searches that are still waiting on
    /// `GetRelevantFilesController`.
    active_searches: HashMap<AIAgentActionId, oneshot::Sender<SearchCodebaseResult>>,
    /// Cached repo roots derived during preprocessing so permission checks and execution can agree
    /// on which repository the action actually targets.
    root_repo_paths: HashMap<AIAgentActionId, PathBuf>,
    terminal_view_id: EntityId,
}

impl SearchCodebaseExecutor {
    pub fn new(
        active_session: ModelHandle<ActiveSession>,
        get_relevant_files_controller: ModelHandle<GetRelevantFilesController>,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&get_relevant_files_controller, |me, event, ctx| {
            if !me.active_searches.contains_key(event.action_id()) {
                return;
            }

            match event {
                GetRelevantFilesControllerEvent::Success { fragments, .. } => {
                    let action_id = event.action_id().clone();
                    let locations = fragments
                        .iter()
                        .map(|location| location.into())
                        .collect_vec();
                    let current_working_directory = me
                        .active_session
                        .as_ref(ctx)
                        .current_working_directory()
                        .cloned();
                    let shell = me.active_session.as_ref(ctx).shell_launch_data(ctx);
                    ctx.spawn(
                        async move {
                            match read_local_file_context(
                                &locations,
                                current_working_directory,
                                shell,
                                None,
                                None,
                            )
                            .await
                            {
                                Ok(result) => {
                                    if !result.missing_files.is_empty() {
                                        let missing_files = result.missing_files.join(", ");
                                        SearchCodebaseResult::Failed {
                                            message: format!(
                                                "These files do not exist: {missing_files}"
                                            ),
                                            reason: SearchCodebaseFailureReason::InvalidFilePaths,
                                        }
                                    } else {
                                        SearchCodebaseResult::Success {
                                            files: result.file_contexts,
                                        }
                                    }
                                }
                                Err(e) => SearchCodebaseResult::Failed {
                                    reason: SearchCodebaseFailureReason::ClientError,
                                    message: e.to_string(),
                                },
                            }
                        },
                        move |me, result, _| {
                            let Some(result_tx) = me.active_searches.remove(&action_id) else {
                                return;
                            };
                            if let Err(e) = result_tx.send(result) {
                                log::warn!(
                                    "Failed to send search codebase results to receiver {e:?}."
                                );
                            }
                        },
                    );
                }
                GetRelevantFilesControllerEvent::Error { action_id } => {
                    let Some(result_tx) = me.active_searches.remove(action_id) else {
                        return;
                    };
                    if let Err(e) = result_tx.send(SearchCodebaseResult::Failed {
                        message: "The search failed. Try another way to locate the relevant files."
                            .to_owned(),
                        reason: SearchCodebaseFailureReason::GetRelevantFilesError,
                    }) {
                        log::warn!("Failed to send search codebase results to receiver {e:?}.");
                    }
                }
            }
        });

        Self {
            active_session,
            get_relevant_files_controller,
            active_searches: HashMap::new(),
            root_repo_paths: HashMap::new(),
            terminal_view_id,
        }
    }

    pub(super) fn should_autoexecute(
        &self,
        input: ExecuteActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let ExecuteActionInput {
            action:
                AIAgentAction {
                    id,
                    action: AIAgentActionType::SearchCodebase(..),
                    ..
                },
            conversation_id,
        } = input
        else {
            return false;
        };

        self.root_repo_paths.get(id).is_none_or(|root_repo_path| {
            // If we have access to read the repo, we can auto-execute the search.
            BlocklistAIPermissions::as_ref(ctx)
                .can_read_files_with_conversation(
                    &conversation_id,
                    vec![root_repo_path.to_owned()],
                    Some(self.terminal_view_id),
                    ctx,
                )
                .is_allowed()
        })
    }

    pub(super) fn execute(
        &mut self,
        input: ExecuteActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> impl Into<AnyActionExecution> {
        let ExecuteActionInput {
            action,
            conversation_id,
            ..
        } = input;
        let AIAgentAction {
            id,
            action:
                AIAgentActionType::SearchCodebase(SearchCodebaseRequest {
                    query,
                    partial_paths,
                    codebase_path,
                }),
            ..
        } = action
        else {
            return ActionExecution::InvalidAction;
        };

        let session_context = SessionContext::from_session(self.active_session.as_ref(ctx), ctx);
        if session_context.is_remote() {
            let explicit_repo_path = codebase_path
                .as_deref()
                .filter(|path| !path.is_empty() && *path != ".");
            let server_output_id = get_server_output_id(input.conversation_id, ctx);
            send_telemetry_from_ctx!(
                TelemetryEvent::SearchCodebaseRequested {
                    action_id: id.clone(),
                    server_output_id,
                    is_cross_repo: explicit_repo_path.is_some(),
                },
                ctx
            );
            return self.execute_remote_search(
                query.clone(),
                partial_paths.clone(),
                explicit_repo_path,
                session_context,
                ctx,
            );
        }
        let codebase_path = codebase_path.as_ref().map(PathBuf::from);

        let Some(current_working_directory) = self
            .active_session
            .as_ref(ctx)
            .current_working_directory()
            .map(PathBuf::from)
        else {
            // This should really never happen; it implies that we don't know what the
            // current working directory is, which is never the case.
            return ActionExecution::Sync(AIAgentActionResultType::SearchCodebase(
                SearchCodebaseResult::Failed {
                    reason: SearchCodebaseFailureReason::MissingCurrentWorkingDirectory,
                    message: "The search failed. Try another way to locate the relevant files."
                        .to_string(),
                },
            ));
        };

        let search_dir;
        let is_cross_repo;
        if FeatureFlag::CrossRepoContext.is_enabled() {
            is_cross_repo = codebase_path
                .as_ref()
                .is_some_and(|path| !current_working_directory.starts_with(path));
            search_dir = codebase_path.unwrap_or(current_working_directory);
        } else {
            is_cross_repo = false;
            search_dir = current_working_directory;
        }
        let server_output_id = get_server_output_id(input.conversation_id, ctx);
        send_telemetry_from_ctx!(
            TelemetryEvent::SearchCodebaseRequested {
                action_id: id.clone(),
                server_output_id,
                is_cross_repo,
            },
            ctx
        );

        let Some(root_dir_for_search) = self.root_repo_paths.get(id) else {
            let action_id = id.clone();

            // Check if directory exists on background thread since its a sys call; no need to block
            // main thread since its just for telemetry.
            let _ = ctx.spawn(async move { search_dir.exists() }, |_, exists, ctx| {
                let error = if exists {
                    "The codebase isn't indexed".to_string()
                } else {
                    "The codebase doesn't exist".to_string()
                };
                send_telemetry_from_ctx!(
                    TelemetryEvent::SearchCodebaseRepoUnavailable { action_id, error },
                    ctx
                );
            });
            return ActionExecution::Sync(AIAgentActionResultType::SearchCodebase(SearchCodebaseResult::Failed {
                message: "The search failed because the codebase is not available. Try another way to locate the relevant files.".to_owned(),
                reason: SearchCodebaseFailureReason::CodebaseNotIndexed
            }));
        };

        // Add the repo root as a temporary permission; if the user gave us permission to
        // search the repo, we can certainly search files within it for the rest of the convo.
        BlocklistAIPermissions::handle(ctx).update(ctx, |model, _ctx| {
            model.add_temporary_file_read_permissions(
                conversation_id,
                vec![root_dir_for_search.to_owned()],
            );
        });

        let (result_tx, result_rx) = oneshot::channel();
        self.active_searches.insert(id.clone(), result_tx);

        // Start the actual search.
        match self
            .get_relevant_files_controller
            .update(ctx, |controller, ctx| {
                controller.send_request(
                    root_dir_for_search,
                    query.clone(),
                    partial_paths.as_ref(),
                    id.clone(),
                    ctx,
                )
            }) {
            Ok(_) => ActionExecution::Async {
                execute_future: Box::pin(result_rx),
                on_complete: Box::new(
                    |res: Result<SearchCodebaseResult, oneshot::Canceled>, _ctx| {
                        let action_result = res.unwrap_or_else(|e| SearchCodebaseResult::Failed {
                            message: e.to_string(),
                            reason: SearchCodebaseFailureReason::ClientError,
                        });
                        AIAgentActionResultType::SearchCodebase(action_result)
                    },
                ),
            },
            Err(e) => {
                log::warn!("Failed to send get_relevant_files request for directory: {e:?}");

                let error_message = match e {
                            GetRelevantFilesError::Pending => {
                                "The current git repository is still being indexed, so search is unavailable right now. You can try again later".to_owned()
                            }
                            GetRelevantFilesError::CreateFailed => {
                                "Relevant file search in the current directory is not available".to_owned()
                            }
                            GetRelevantFilesError::Missing => {
                                "The current directory isn't within a git repository, which is necessary to search for relevant files.".to_owned()
                            }
                        };
                ActionExecution::Sync(AIAgentActionResultType::SearchCodebase(
                    SearchCodebaseResult::Failed {
                        reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
                        message: error_message,
                    },
                ))
            }
        }
    }

    fn execute_remote_search(
        &self,
        query: String,
        partial_paths: Option<Vec<String>>,
        explicit_repo_path: Option<&str>,
        session_context: SessionContext,
        ctx: &mut ModelContext<Self>,
    ) -> ActionExecution<Result<SearchCodebaseResult, oneshot::Canceled>> {
        let availability = RemoteCodebaseIndexModel::as_ref(ctx)
            .active_repo_availability(&session_context, explicit_repo_path);
        if !FeatureFlag::RemoteCodebaseIndexing.is_enabled() {
            return ActionExecution::Sync(AIAgentActionResultType::SearchCodebase(
                SearchCodebaseResult::Failed {
                    reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
                    message: "Remote codebase search is not enabled.".to_string(),
                },
            ));
        }

        match availability {
            RemoteCodebaseSearchAvailability::Ready(search_context) => {
                let Some(client) = remote_server::manager::RemoteServerManager::as_ref(ctx)
                    .client_for_host(&search_context.host_id)
                    .cloned()
                else {
                    return ActionExecution::Sync(AIAgentActionResultType::SearchCodebase(
                        SearchCodebaseResult::Failed {
                            reason: SearchCodebaseFailureReason::ClientError,
                            message: "Remote codebase search is unavailable because the remote server is not connected.".to_string(),
                        },
                    ));
                };
                let store_client = ServerApiProvider::as_ref(ctx).get();
                ActionExecution::Async {
                    execute_future: Box::pin(async move {
                        let result = execute_remote_codebase_search(
                            query,
                            partial_paths,
                            search_context,
                            client,
                            store_client,
                        )
                        .await
                        .unwrap_or_else(|e| SearchCodebaseResult::Failed {
                            reason: SearchCodebaseFailureReason::ClientError,
                            message: e.to_string(),
                        });
                        Ok(result)
                    }),
                    on_complete: Box::new(
                        |res: Result<SearchCodebaseResult, oneshot::Canceled>, _ctx| {
                            let action_result =
                                res.unwrap_or_else(|e| SearchCodebaseResult::Failed {
                                    reason: SearchCodebaseFailureReason::ClientError,
                                    message: e.to_string(),
                                });
                            AIAgentActionResultType::SearchCodebase(action_result)
                        },
                    ),
                }
            }
            availability @ RemoteCodebaseSearchAvailability::NotIndexed { .. } => {
                let explicit_repo_path = explicit_repo_path.map(ToOwned::to_owned);
                RemoteCodebaseIndexModel::handle(ctx).update(ctx, |model, ctx| {
                    model.request_active_repo_index(
                        &session_context,
                        explicit_repo_path.as_deref(),
                        ctx,
                    );
                });
                ActionExecution::Sync(AIAgentActionResultType::SearchCodebase(
                    remote_availability_failure(availability),
                ))
            }
            RemoteCodebaseSearchAvailability::NotRemote
            | RemoteCodebaseSearchAvailability::NoConnectedHost
            | RemoteCodebaseSearchAvailability::NoActiveRepo
            | RemoteCodebaseSearchAvailability::Indexing { .. }
            | RemoteCodebaseSearchAvailability::Failed { .. }
            | RemoteCodebaseSearchAvailability::Unavailable { .. } => ActionExecution::Sync(
                AIAgentActionResultType::SearchCodebase(remote_availability_failure(availability)),
            ),
        }
    }

    pub fn root_repo_for_action(&self, id: &AIAgentActionId) -> Option<&Path> {
        self.root_repo_paths.get(id).map(|path| path.as_path())
    }

    pub(super) fn cancel_execution(
        &mut self,
        action_id: &AIAgentActionId,
        ctx: &mut ModelContext<Self>,
    ) {
        // Drop the waiting sender first so any late completion from the controller becomes a no-op.
        self.active_searches.remove(action_id);
        self.get_relevant_files_controller
            .update(ctx, |controller, ctx| {
                controller.cancel_request_for_action(action_id, ctx)
            });
    }

    fn get_root_repo_path_for_request(
        &self,
        request: &SearchCodebaseRequest,
        app: &AppContext,
    ) -> Option<PathBuf> {
        let SearchCodebaseRequest { codebase_path, .. } = request;
        let codebase_path = codebase_path.as_deref().map(PathBuf::from);
        let Some(pwd) = self
            .active_session
            .as_ref(app)
            .current_working_directory()
            .map(PathBuf::from)
        else {
            // This should never really happen, since we should always have a pwd.
            log::warn!("No pwd found for search codebase request");
            return None;
        };

        let search_dir = if FeatureFlag::CrossRepoContext.is_enabled() {
            match codebase_path {
                Some(codebase_path) if codebase_path == Path::new(".") => pwd,
                Some(codebase_path) => codebase_path,
                None => pwd,
            }
        } else {
            pwd
        };

        self.get_relevant_files_controller
            .as_ref(app)
            .root_directory_for_search(&search_dir, app)
    }

    /// In the preprocessing step, we determine the root of the repo path for the codebase to be
    /// searched, and cache it. This is used downstream to render UI thats derived from the root
    /// repo path, which isn't really trivially computable from the `action` itself.
    pub(super) fn preprocess_action(
        &mut self,
        input: PreprocessActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> BoxFuture<'static, ()> {
        let AIAgentAction {
            id,
            action: AIAgentActionType::SearchCodebase(request),
            ..
        } = input.action
        else {
            log::error!("Expected a SearchCodebase action when preprocessing action");
            return futures::future::ready(()).boxed();
        };

        if let Some(root_repo_path) = self.get_root_repo_path_for_request(request, ctx) {
            self.root_repo_paths.insert(id.clone(), root_repo_path);
        }
        futures::future::ready(()).boxed()
    }
}

async fn execute_remote_codebase_search(
    query: String,
    partial_paths: Option<Vec<String>>,
    search_context: RemoteCodebaseSearchContext,
    client: std::sync::Arc<remote_server::client::RemoteServerClient>,
    store_client: std::sync::Arc<crate::server::server_api::ServerApi>,
) -> Result<SearchCodebaseResult, anyhow::Error> {
    let root_hash = search_context.root_hash;
    let root_hash_string = root_hash.to_string();
    let candidate_hashes = store_client
        .get_relevant_fragments(
            search_context.embedding_config,
            query.clone(),
            root_hash,
            RepoMetadata {
                path: Some(search_context.repo_path.clone()),
            },
        )
        .await?;
    if candidate_hashes.is_empty() {
        return Ok(SearchCodebaseResult::Success { files: vec![] });
    }

    let candidate_hash_strings = candidate_hashes
        .iter()
        .map(ToString::to_string)
        .collect_vec();
    let metadata_response = client
        .get_fragment_metadata_from_hash(
            search_context.repo_path.clone(),
            root_hash_string,
            candidate_hash_strings,
        )
        .await?;
    if !metadata_response.missing_hashes.is_empty() {
        log::warn!(
            "Remote codebase search metadata lookup missed {} hashes for repo {}",
            metadata_response.missing_hashes.len(),
            search_context.repo_path
        );
    }
    let mut metadata = metadata_response.fragments;
    if let Some(partial_paths) = partial_paths {
        metadata.retain(|fragment| {
            partial_paths
                .iter()
                .any(|partial_path| fragment.path.contains(partial_path))
        });
    }
    if metadata.is_empty() {
        return Ok(SearchCodebaseResult::Success { files: vec![] });
    }

    let response = client
        .read_file_context(read_fragment_metadata_request(&metadata))
        .await?;
    if !response.failed_files.is_empty() && response.file_contexts.is_empty() {
        let failed = response
            .failed_files
            .iter()
            .map(|file| {
                let reason = file
                    .error
                    .as_ref()
                    .map(|error| error.message.as_str())
                    .unwrap_or("unknown error");
                format!("{}: {reason}", file.path)
            })
            .join(", ");
        return Ok(SearchCodebaseResult::Failed {
            reason: SearchCodebaseFailureReason::InvalidFilePaths,
            message: format!("Failed to read remote search result files: {failed}"),
        });
    }

    let (fragments, mut file_contexts_by_hash) =
        remote_fragments_and_file_contexts(response, &metadata)?;
    if fragments.is_empty() {
        return Ok(SearchCodebaseResult::Success { files: vec![] });
    }

    let reranked_fragments = store_client.rerank_fragments(query, fragments).await?;
    let files = reranked_fragments
        .into_iter()
        .filter_map(|fragment| file_contexts_by_hash.remove(&fragment.content_hash().to_string()))
        .collect_vec();

    Ok(SearchCodebaseResult::Success { files })
}

fn read_fragment_metadata_request(metadata: &[FragmentMetadata]) -> ReadFileContextRequest {
    ReadFileContextRequest {
        files: metadata
            .iter()
            .map(|fragment| {
                let line_ranges =
                    if fragment.start_line > 0 && fragment.end_line >= fragment.start_line {
                        vec![LineRange {
                            start: fragment.start_line,
                            end: fragment.end_line,
                        }]
                    } else {
                        vec![]
                    };
                ReadFileContextFile {
                    path: fragment.path.clone(),
                    line_ranges,
                }
            })
            .collect(),
        max_file_bytes: None,
        max_batch_bytes: None,
    }
}

fn remote_fragments_and_file_contexts(
    response: ReadFileContextResponse,
    metadata: &[FragmentMetadata],
) -> anyhow::Result<(Vec<Fragment>, HashMap<String, FileContext>)> {
    let mut fragments = Vec::new();
    let mut file_contexts_by_hash = HashMap::new();

    for (file_context, fragment_metadata) in response.file_contexts.into_iter().zip(metadata) {
        let Some(file_context) = proto_file_context_to_file_context(file_context) else {
            continue;
        };
        let AnyFileContent::StringContent(content) = &file_context.content else {
            continue;
        };
        let content_hash = ContentHash::from_str(&fragment_metadata.content_hash)?;
        fragments.push(Fragment::from_byte_range(
            content.clone(),
            content_hash,
            PathBuf::from(fragment_metadata.path.clone()),
            fragment_metadata.byte_start as usize..fragment_metadata.byte_end as usize,
        ));
        file_contexts_by_hash.insert(fragment_metadata.content_hash.clone(), file_context);
    }

    Ok((fragments, file_contexts_by_hash))
}

fn proto_file_context_to_file_context(
    file_context: remote_server::proto::FileContextProto,
) -> Option<FileContext> {
    let content = match file_context.content? {
        file_context_proto::Content::TextContent(text) => AnyFileContent::StringContent(text),
        file_context_proto::Content::BinaryContent(bytes) => AnyFileContent::BinaryContent(bytes),
    };
    let line_range = match (file_context.line_range_start, file_context.line_range_end) {
        (Some(start), Some(end)) => Some(start as usize..end as usize),
        (Some(_), None) | (None, Some(_)) | (None, None) => None,
    };
    let last_modified = file_context
        .last_modified_epoch_millis
        .map(|ms| std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms));
    Some(FileContext {
        file_name: file_context.file_name,
        content,
        line_range,
        last_modified,
        line_count: file_context.line_count as usize,
    })
}

fn remote_availability_failure(
    availability: RemoteCodebaseSearchAvailability,
) -> SearchCodebaseResult {
    match availability {
        RemoteCodebaseSearchAvailability::NotRemote => SearchCodebaseResult::Failed {
            reason: SearchCodebaseFailureReason::ClientError,
            message: "Codebase search was routed to a remote search path for a local session."
                .to_string(),
        },
        RemoteCodebaseSearchAvailability::NoConnectedHost => SearchCodebaseResult::Failed {
            reason: SearchCodebaseFailureReason::ClientError,
            message: "Remote codebase search is unavailable because the remote host is not connected."
                .to_string(),
        },
        RemoteCodebaseSearchAvailability::NoActiveRepo => SearchCodebaseResult::Failed {
            reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
            message: "The current remote directory is not in a known git repository.".to_string(),
        },
        RemoteCodebaseSearchAvailability::NotIndexed { repo_path } => {
            SearchCodebaseResult::Failed {
                reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
                message: format!(
                    "The remote codebase at {repo_path} is not indexed yet. Indexing has been requested; try again after it finishes."
                ),
            }
        }
        RemoteCodebaseSearchAvailability::Indexing { repo_path } => {
            SearchCodebaseResult::Failed {
                reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
                message: format!(
                    "The remote codebase at {repo_path} is still being indexed. Try again later."
                ),
            }
        }
        RemoteCodebaseSearchAvailability::Failed { repo_path, message } => {
            SearchCodebaseResult::Failed {
                reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
                message: format!("The remote codebase index for {repo_path} failed: {message}"),
            }
        }
        RemoteCodebaseSearchAvailability::Unavailable { repo_path, message } => {
            SearchCodebaseResult::Failed {
                reason: SearchCodebaseFailureReason::CodebaseNotIndexed,
                message: format!(
                    "Remote codebase search is unavailable for {repo_path}: {message}"
                ),
            }
        }
        RemoteCodebaseSearchAvailability::Ready(_) => SearchCodebaseResult::Failed {
            reason: SearchCodebaseFailureReason::ClientError,
            message: "Remote codebase search was unexpectedly unavailable.".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure_reason(result: SearchCodebaseResult) -> SearchCodebaseFailureReason {
        match result {
            SearchCodebaseResult::Failed { reason, .. } => reason,
            SearchCodebaseResult::Success { .. } => {
                panic!("expected remote availability failure")
            }
            SearchCodebaseResult::Cancelled => {
                panic!("expected remote availability failure")
            }
        }
    }

    #[test]
    fn remote_not_indexed_failure_maps_to_codebase_not_indexed() {
        let reason = failure_reason(remote_availability_failure(
            RemoteCodebaseSearchAvailability::NotIndexed {
                repo_path: "/repo".to_string(),
            },
        ));

        assert_eq!(reason, SearchCodebaseFailureReason::CodebaseNotIndexed);
    }

    #[test]
    fn remote_indexing_failure_maps_to_codebase_not_indexed() {
        let reason = failure_reason(remote_availability_failure(
            RemoteCodebaseSearchAvailability::Indexing {
                repo_path: "/repo".to_string(),
            },
        ));

        assert_eq!(reason, SearchCodebaseFailureReason::CodebaseNotIndexed);
    }

    #[test]
    fn remote_unavailable_failure_maps_to_codebase_not_indexed() {
        let reason = failure_reason(remote_availability_failure(
            RemoteCodebaseSearchAvailability::Unavailable {
                repo_path: "/repo".to_string(),
                message: "missing root hash".to_string(),
            },
        ));

        assert_eq!(reason, SearchCodebaseFailureReason::CodebaseNotIndexed);
    }

    #[test]
    fn remote_disconnected_failure_maps_to_client_error() {
        let reason = failure_reason(remote_availability_failure(
            RemoteCodebaseSearchAvailability::NoConnectedHost,
        ));

        assert_eq!(reason, SearchCodebaseFailureReason::ClientError);
    }
}
impl Entity for SearchCodebaseExecutor {
    type Event = ();
}
