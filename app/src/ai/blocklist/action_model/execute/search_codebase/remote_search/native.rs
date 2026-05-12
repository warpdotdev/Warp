use ::ai::index::full_source_code_embedding::{
    store_client::StoreClient, ContentHash, Fragment, RepoMetadata,
};
use futures::channel::oneshot;
use itertools::Itertools;
use remote_server::proto::{
    file_context_proto, FragmentMetadata, LineRange, ReadFileContextFile, ReadFileContextRequest,
    ReadFileContextResponse,
};
use std::{collections::HashMap, path::PathBuf, str::FromStr, sync::Arc};
use warpui::{ModelContext, SingletonEntity};

use crate::{
    ai::{
        agent::{
            AIAgentActionResultType, AnyFileContent, FileContext, SearchCodebaseFailureReason,
            SearchCodebaseResult,
        },
        blocklist::{action_model::execute::ActionExecution, SessionContext},
    },
    features::FeatureFlag,
    remote_server::codebase_index_model::{
        RemoteCodebaseIndexModel, RemoteCodebaseSearchAvailability, RemoteCodebaseSearchContext,
    },
    server::server_api::{ServerApi, ServerApiProvider},
};

use super::SearchCodebaseExecutor;

pub(super) fn execute_remote_search(
    query: String,
    partial_paths: Option<Vec<String>>,
    explicit_repo_path: Option<&str>,
    session_context: SessionContext,
    ctx: &mut ModelContext<SearchCodebaseExecutor>,
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
            ActionExecution::new_async(
                async move {
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
                },
                |res: Result<SearchCodebaseResult, oneshot::Canceled>, _ctx| {
                    let action_result = res.unwrap_or_else(|e| SearchCodebaseResult::Failed {
                        reason: SearchCodebaseFailureReason::ClientError,
                        message: e.to_string(),
                    });
                    AIAgentActionResultType::SearchCodebase(action_result)
                },
            )
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

async fn execute_remote_codebase_search(
    query: String,
    partial_paths: Option<Vec<String>>,
    search_context: RemoteCodebaseSearchContext,
    client: Arc<remote_server::client::RemoteServerClient>,
    store_client: Arc<ServerApi>,
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

    let (fragments, mut file_contexts_by_identity) =
        remote_fragments_and_file_contexts(response, &metadata)?;
    if fragments.is_empty() {
        return Ok(SearchCodebaseResult::Success { files: vec![] });
    }

    let reranked_fragments = store_client.rerank_fragments(query, fragments).await?;
    let files = reranked_fragments
        .into_iter()
        .filter_map(|fragment| {
            file_contexts_by_identity.remove(&RemoteFragmentIdentity::from_fragment(&fragment))
        })
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
                            end: fragment.end_line.saturating_add(1),
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct RemoteFragmentIdentity {
    content_hash: String,
    path: String,
    byte_start: u64,
    byte_end: u64,
}

impl RemoteFragmentIdentity {
    fn from_metadata(metadata: &FragmentMetadata) -> Self {
        Self {
            content_hash: metadata.content_hash.clone(),
            path: metadata.path.clone(),
            byte_start: metadata.byte_start,
            byte_end: metadata.byte_end,
        }
    }

    fn from_fragment(fragment: &Fragment) -> Self {
        let byte_range = fragment.byte_range();
        Self {
            content_hash: fragment.content_hash().to_string(),
            path: fragment.absolute_path().to_string_lossy().to_string(),
            byte_start: byte_range.start as u64,
            byte_end: byte_range.end as u64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct RemoteReadContextKey {
    path: String,
    line_range_start: Option<u32>,
    line_range_end: Option<u32>,
}

impl RemoteReadContextKey {
    fn from_metadata(metadata: &FragmentMetadata) -> Self {
        let line_range = (metadata.start_line > 0 && metadata.end_line >= metadata.start_line)
            .then_some((metadata.start_line, metadata.end_line.saturating_add(1)));
        Self {
            path: metadata.path.clone(),
            line_range_start: line_range.map(|(start, _)| start),
            line_range_end: line_range.map(|(_, end)| end),
        }
    }

    fn from_file_context(file_context: &remote_server::proto::FileContextProto) -> Self {
        Self {
            path: file_context.file_name.clone(),
            line_range_start: file_context.line_range_start,
            line_range_end: file_context.line_range_end,
        }
    }
}

fn remote_fragments_and_file_contexts(
    response: ReadFileContextResponse,
    metadata: &[FragmentMetadata],
) -> anyhow::Result<(Vec<Fragment>, HashMap<RemoteFragmentIdentity, FileContext>)> {
    let mut fragments = Vec::new();
    let mut file_contexts_by_read_key: HashMap<RemoteReadContextKey, Vec<FileContext>> =
        HashMap::new();
    for file_context in response.file_contexts {
        let read_key = RemoteReadContextKey::from_file_context(&file_context);
        let Some(file_context) = proto_file_context_to_file_context(file_context) else {
            continue;
        };
        file_contexts_by_read_key
            .entry(read_key)
            .or_default()
            .push(file_context);
    }

    let mut file_contexts_by_identity = HashMap::new();

    for fragment_metadata in metadata {
        let read_key = RemoteReadContextKey::from_metadata(fragment_metadata);
        let Some(file_context) = file_contexts_by_read_key
            .get(&read_key)
            .and_then(|file_contexts| file_contexts.last())
            .cloned()
        else {
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
        file_contexts_by_identity.insert(
            RemoteFragmentIdentity::from_metadata(fragment_metadata),
            file_context,
        );
    }

    Ok((fragments, file_contexts_by_identity))
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
    use remote_server::proto::FileContextProto;

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

    fn fragment_metadata(path: &str, content_hash: &str) -> FragmentMetadata {
        FragmentMetadata {
            content_hash: content_hash.to_string(),
            path: path.to_string(),
            start_line: 1,
            end_line: 2,
            byte_start: 0,
            byte_end: 4,
        }
    }

    fn text_file_context(path: &str, content: &str) -> FileContextProto {
        FileContextProto {
            file_name: path.to_string(),
            content: Some(file_context_proto::Content::TextContent(
                content.to_string(),
            )),
            line_range_start: Some(1),
            line_range_end: Some(3),
            last_modified_epoch_millis: None,
            line_count: 3,
        }
    }

    #[test]
    fn read_fragment_metadata_request_converts_end_line_to_exclusive() {
        let metadata = vec![fragment_metadata(
            "/repo/src/lib.rs",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )];

        let request = read_fragment_metadata_request(&metadata);

        assert_eq!(request.files.len(), 1);
        assert_eq!(request.files[0].line_ranges.len(), 1);
        assert_eq!(request.files[0].line_ranges[0].start, 1);
        assert_eq!(request.files[0].line_ranges[0].end, 3);
    }

    #[test]
    fn remote_fragments_match_file_contexts_by_identity() {
        let content_hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let first_metadata = fragment_metadata("/repo/src/first.rs", content_hash);
        let second_metadata = fragment_metadata("/repo/src/second.rs", content_hash);
        let metadata = vec![first_metadata.clone(), second_metadata.clone()];
        let response = ReadFileContextResponse {
            file_contexts: vec![
                text_file_context("/repo/src/second.rs", "two\n"),
                text_file_context("/repo/src/first.rs", "one\n"),
            ],
            failed_files: vec![],
        };

        let (fragments, file_contexts_by_identity) =
            remote_fragments_and_file_contexts(response, &metadata).unwrap();

        assert_eq!(fragments.len(), 2);
        assert_eq!(
            file_contexts_by_identity
                .get(&RemoteFragmentIdentity::from_metadata(&first_metadata))
                .map(|context| context.file_name.as_str()),
            Some("/repo/src/first.rs")
        );
        assert_eq!(
            file_contexts_by_identity
                .get(&RemoteFragmentIdentity::from_metadata(&second_metadata))
                .map(|context| context.file_name.as_str()),
            Some("/repo/src/second.rs")
        );
    }
}
