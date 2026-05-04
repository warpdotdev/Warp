mod apply_diff_model;
mod diff_application;
mod telemetry;

use warp_util::file::FileSaveError;

use std::collections::HashMap;
use std::path::PathBuf;

use ai::diff_validation::{AIRequestedCodeDiff, ParsedDiff};
use futures::{channel::oneshot, future::BoxFuture, FutureExt};
use itertools::Itertools;
use vec1::{vec1, Vec1};
use warp_core::send_telemetry_from_ctx;
use warpui::{Entity, EntityId, ModelContext, ModelHandle, SingletonEntity as _, ViewHandle};

use apply_diff_model::ApplyDiffModel;
pub(crate) use diff_application::apply_edits;
use diff_application::DiffApplicationError;
pub(crate) use diff_application::FileReadResult;
pub(crate) use telemetry::MalformedFinalLineProxyEvent;
#[allow(unused_imports)]
pub use telemetry::{EditAcceptAndContinueClickedEvent, EditAcceptClickedEvent};
pub use telemetry::{
    EditReceivedEvent, EditResolvedEvent, EditStats, RequestFileEditsFormatKind,
    RequestFileEditsTelemetryEvent,
};

use crate::{
    ai::{
        agent::{
            conversation::AIConversationId, AIAgentAction, AIAgentActionId,
            AIAgentActionResultType, AIAgentActionType, AIAgentOutputMessage,
            AIAgentOutputMessageType, AIIdentifiers, FileEdit, RequestFileEditsResult,
            UpdatedFileContext,
        },
        blocklist::{
            inline_action::code_diff_view::{
                CodeDiffView, CodeDiffViewEvent, DiffSessionType, FileDiff,
            },
            BlocklistAIPermissions, RequestedEditResolution,
        },
        paths::host_native_absolute_path,
    },
    safe_warn,
    terminal::{
        model::session::{active_session::ActiveSession, SessionType},
        ShellLaunchData,
    },
    BlocklistAIHistoryModel,
};

use super::{ActionExecution, AnyActionExecution, ExecuteActionInput, PreprocessActionInput};

pub struct RequestFileEditsExecutor {
    active_session: ModelHandle<ActiveSession>,
    apply_diff_model: ModelHandle<ApplyDiffModel>,
    diff_views: HashMap<AIAgentActionId, ViewHandle<CodeDiffView>>,
    /// Failed diff applications scoped to the exact action payload that was preprocessed.
    diff_application_failures: HashMap<AIAgentActionId, FileEditPreprocessFailure>,
    preprocessed_actions: HashMap<AIAgentActionId, FileEditPreprocessFingerprint>,
    terminal_view_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileEditPreprocessFingerprint {
    conversation_id: AIConversationId,
    action_payload: String,
    session_context: String,
}

#[derive(Debug, Clone)]
struct FileEditPreprocessContext {
    session_type: Option<SessionType>,
    shell: Option<ShellLaunchData>,
    current_working_directory: Option<String>,
}

impl FileEditPreprocessContext {
    fn fingerprint_key(&self) -> String {
        format!(
            "session_type={:?};shell={:?};cwd={:?}",
            self.session_type, self.shell, self.current_working_directory
        )
    }
}

struct FileEditPreprocessFailure {
    fingerprint: FileEditPreprocessFingerprint,
    errors: Vec1<DiffApplicationError>,
}

fn file_edit_preprocess_failure_matches(
    failure: &FileEditPreprocessFailure,
    fingerprint: &FileEditPreprocessFingerprint,
) -> bool {
    &failure.fingerprint == fingerprint
}

fn file_edit_preprocess_fingerprint(
    conversation_id: AIConversationId,
    action: &AIAgentAction,
    session_context: &FileEditPreprocessContext,
) -> FileEditPreprocessFingerprint {
    FileEditPreprocessFingerprint {
        conversation_id,
        action_payload: format!("{:?}", action.action),
        session_context: session_context.fingerprint_key(),
    }
}

#[cfg(test)]
fn file_edit_preprocess_context_for_test(
    current_working_directory: Option<&str>,
) -> FileEditPreprocessContext {
    FileEditPreprocessContext {
        session_type: None,
        shell: None,
        current_working_directory: current_working_directory.map(str::to_string),
    }
}

fn file_edit_paths_for_permissions(file_edits: &[FileEdit]) -> Vec<&str> {
    file_edits
        .iter()
        .flat_map(|edit| match edit {
            FileEdit::Edit(ParsedDiff::V4AEdit { file, move_to, .. }) => {
                [file.as_deref(), move_to.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            }
            _ => edit.file().into_iter().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent::task::TaskId;

    fn file_edit_action(action_id: &str, file: &str) -> AIAgentAction {
        AIAgentAction {
            id: AIAgentActionId::from(action_id.to_string()),
            task_id: TaskId::new("task_1".to_string()),
            action: AIAgentActionType::RequestFileEdits {
                file_edits: vec![crate::ai::agent::FileEdit::Create {
                    file: Some(file.to_string()),
                    content: Some("content\n".to_string()),
                }],
                title: None,
            },
            requires_result: true,
        }
    }

    #[test]
    fn file_edit_preprocess_fingerprint_scopes_conversation_and_payload() {
        let conversation_one = AIConversationId::new();
        let conversation_two = AIConversationId::new();
        let old_action = file_edit_action("action_1", "src/old.rs");
        let new_action = file_edit_action("action_1", "src/new.rs");
        let context = file_edit_preprocess_context_for_test(Some("/repo"));

        assert_ne!(
            file_edit_preprocess_fingerprint(conversation_one, &old_action, &context),
            file_edit_preprocess_fingerprint(conversation_two, &old_action, &context)
        );
        assert_ne!(
            file_edit_preprocess_fingerprint(conversation_one, &old_action, &context),
            file_edit_preprocess_fingerprint(conversation_one, &new_action, &context)
        );
    }

    #[test]
    fn file_edit_preprocess_fingerprint_scopes_working_directory() {
        let conversation_id = AIConversationId::new();
        let action = file_edit_action("action_1", "src/config.rs");
        let old_context = file_edit_preprocess_context_for_test(Some("/repo-a"));
        let new_context = file_edit_preprocess_context_for_test(Some("/repo-b"));

        assert_ne!(
            file_edit_preprocess_fingerprint(conversation_id, &action, &old_context),
            file_edit_preprocess_fingerprint(conversation_id, &action, &new_context)
        );
    }

    #[test]
    fn file_edit_preprocess_failure_is_fingerprinted() {
        let conversation_id = AIConversationId::new();
        let old_action = file_edit_action("action_1", "src/old.rs");
        let new_action = file_edit_action("action_1", "src/new.rs");
        let context = file_edit_preprocess_context_for_test(Some("/repo"));
        let old_fingerprint =
            file_edit_preprocess_fingerprint(conversation_id, &old_action, &context);
        let new_fingerprint =
            file_edit_preprocess_fingerprint(conversation_id, &new_action, &context);
        let failure = FileEditPreprocessFailure {
            fingerprint: old_fingerprint.clone(),
            errors: vec1![DiffApplicationError::EmptyDiff],
        };

        assert!(file_edit_preprocess_failure_matches(
            &failure,
            &old_fingerprint
        ));
        assert!(!file_edit_preprocess_failure_matches(
            &failure,
            &new_fingerprint
        ));
    }
}

impl RequestFileEditsExecutor {
    pub fn new(
        active_session: ModelHandle<ActiveSession>,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let apply_diff_model = ctx.add_model(|_| ApplyDiffModel::new(active_session.clone()));
        Self {
            active_session,
            apply_diff_model,
            diff_views: HashMap::new(),
            diff_application_failures: HashMap::new(),
            preprocessed_actions: HashMap::new(),
            terminal_view_id,
        }
    }

    pub(super) fn should_autoexecute(
        &self,
        input: ExecuteActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let ExecuteActionInput {
            action,
            conversation_id,
        } = input;
        let AIAgentActionType::RequestFileEdits { file_edits, .. } = &action.action else {
            return false;
        };

        let paths = file_edit_paths_for_permissions(file_edits)
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();

        // Don't allow autoexecution if the diff was generated passively.
        let Some(latest_exchange) = BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&conversation_id)
            .and_then(|c| c.latest_exchange())
        else {
            return false;
        };
        if latest_exchange.has_passive_request() {
            return false;
        }

        // Allow "autoexecution" if the diff application failed so that we can continue execution.
        // This is a terrible hack--but allows us to continue execution and let the LLM potentially recover
        // from the LLM.
        // If we don't do this, a failed diff application will block execution of the entire AI conversation
        // without any possibility of recovery.
        let session_context = self.file_edit_preprocess_context(ctx);
        let fingerprint =
            file_edit_preprocess_fingerprint(conversation_id, action, &session_context);
        if self
            .diff_application_failures
            .get(&action.id)
            .is_some_and(|failure| file_edit_preprocess_failure_matches(failure, &fingerprint))
        {
            return true;
        }

        BlocklistAIPermissions::as_ref(ctx)
            .can_write_files(&conversation_id, &paths, Some(self.terminal_view_id), ctx)
            .is_allowed()
    }

    /// Registers a diff view to handle a RequestFileEdits action.
    /// Note this MUST be called before `execute` or `preprocess_action` is invoked in
    /// order for the necessary state to be set to handle the action.
    pub fn register_requested_edits(
        &mut self,
        action_id: &AIAgentActionId,
        view: &ViewHandle<CodeDiffView>,
    ) {
        self.diff_views.insert(action_id.clone(), view.clone());
    }

    pub(super) fn has_preprocessed_action(
        &self,
        conversation_id: AIConversationId,
        action: &AIAgentAction,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let session_context = self.file_edit_preprocess_context(ctx);
        let fingerprint =
            file_edit_preprocess_fingerprint(conversation_id, action, &session_context);
        self.preprocessed_actions
            .get(&action.id)
            .is_some_and(|stored| stored == &fingerprint)
            && (self.diff_views.contains_key(&action.id)
                || self
                    .diff_application_failures
                    .get(&action.id)
                    .is_some_and(|failure| {
                        file_edit_preprocess_failure_matches(failure, &fingerprint)
                    }))
    }

    pub(super) fn execute(
        &mut self,
        input: ExecuteActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> impl Into<AnyActionExecution> {
        let ExecuteActionInput {
            action,
            conversation_id,
        } = input;
        let AIAgentAction {
            id,
            action: AIAgentActionType::RequestFileEdits { .. },
            ..
        } = action
        else {
            return ActionExecution::InvalidAction;
        };

        let Some(diff_view) = self.diff_views.get(id) else {
            log::warn!("Tried to execute a RequestFileEdits action without a diff view");
            return ActionExecution::NotReady;
        };

        // If diff application failed, early exit.
        let session_context = self.file_edit_preprocess_context(ctx);
        let fingerprint =
            file_edit_preprocess_fingerprint(conversation_id, action, &session_context);
        let matching_failure = self
            .diff_application_failures
            .get(id)
            .is_some_and(|failure| file_edit_preprocess_failure_matches(failure, &fingerprint));
        if matching_failure {
            let errors = self
                .diff_application_failures
                .remove(id)
                .expect("matching diff failure should exist")
                .errors;
            return ActionExecution::Sync(AIAgentActionResultType::RequestFileEdits(
                RequestFileEditsResult::DiffApplicationFailed {
                    error: DiffApplicationError::error_for_conversation(&errors),
                },
            ));
        }

        let identifiers = self
            .generate_ai_identifiers(&conversation_id, id, ctx)
            .unwrap_or_else(|| AIIdentifiers {
                client_conversation_id: Some(conversation_id),
                ..Default::default()
            });

        let (result_tx, result_rx) = oneshot::channel();
        let mut result_tx = Some(result_tx);

        ctx.subscribe_to_view(diff_view, move |_me, event, ctx| match event {
            CodeDiffViewEvent::Rejected => {
                let Some(result_tx) = result_tx.take() else {
                    return;
                };
                let _ = result_tx.send(RequestFileEditsResult::Cancelled);
            }
            CodeDiffViewEvent::SavedAcceptedDiffs {
                diff,
                updated_files,
                file_contents,
                deleted_files,
                save_errors,
            } => {
                let Some(result_tx) = result_tx.take() else {
                    return;
                };

                // If saving any file failed, report it as an error to the LLM. Other files may
                // have saved successfully, but we're ignoring this edge case for now.
                if !save_errors.is_empty() {
                    let error = save_errors
                        .iter()
                        .filter_map(|err| match err.as_ref() {
                            FileSaveError::IOError { error, path } => {
                                Some(format!("Failed to save file {path:?}: {error}"))
                            }
                            _ => None,
                        })
                        .join("\n");

                    let _ = result_tx.send(RequestFileEditsResult::DiffApplicationFailed { error });
                    return;
                }

                let passive_diff = BlocklistAIHistoryModel::as_ref(ctx)
                    .is_entirely_passive_conversation(&conversation_id);
                send_telemetry_from_ctx!(
                    RequestFileEditsTelemetryEvent::EditResolved(EditResolvedEvent {
                        identifiers: identifiers.clone(),
                        response: RequestedEditResolution::Accept,
                        stats: EditStats {
                            files_edited: updated_files.len(),
                            lines_added: diff.lines_added,
                            lines_removed: diff.lines_removed,
                        },
                        passive_diff,
                    },),
                    ctx
                );

                // Build a map of file path → content from the editor buffers.
                // This avoids re-reading files from disk or the remote server.
                let content_map: HashMap<String, String> = file_contents.iter().cloned().collect();

                let mut file_edited_map = HashMap::new();
                for (file_location, was_edited) in updated_files.iter() {
                    file_edited_map.insert(file_location.name.clone(), *was_edited);
                }

                let _ = result_tx.send(RequestFileEditsResult::Success {
                    diff: diff.unified_diff.clone(),
                    updated_files: updated_files
                        .iter()
                        .map(|(file_location, was_edited)| {
                            let content = content_map
                                .get(&file_location.name)
                                .cloned()
                                .unwrap_or_default();
                            let line_count = content.lines().count();
                            UpdatedFileContext {
                                was_edited_by_user: *was_edited,
                                file_context: crate::ai::agent::FileContext {
                                    file_name: file_location.name.clone(),
                                    content: crate::ai::agent::AnyFileContent::StringContent(
                                        content,
                                    ),
                                    line_range: None,
                                    last_modified: None,
                                    line_count,
                                },
                            }
                        })
                        .collect(),
                    deleted_files: deleted_files.clone(),
                    lines_added: diff.lines_added,
                    lines_removed: diff.lines_removed,
                });
            }
            _ => (),
        });
        diff_view.update(ctx, |diff_view, ctx| {
            diff_view.accept_and_save(ctx);
        });

        ActionExecution::new_async(result_rx, |result, _ctx| match result {
            Ok(result) => AIAgentActionResultType::RequestFileEdits(result),
            Err(oneshot::Canceled) => {
                AIAgentActionResultType::RequestFileEdits(RequestFileEditsResult::Cancelled)
            }
        })
    }

    pub(super) fn preprocess_action(
        &mut self,
        input: PreprocessActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> BoxFuture<'static, ()> {
        let AIAgentAction {
            id,
            action: AIAgentActionType::RequestFileEdits { file_edits, .. },
            ..
        } = input.action
        else {
            return futures::future::ready(()).boxed();
        };

        let ai_identifiers = self
            .generate_ai_identifiers(&input.conversation_id, id, ctx)
            .unwrap_or_else(|| AIIdentifiers {
                client_conversation_id: Some(input.conversation_id),
                ..Default::default()
            });

        let passive_diff = BlocklistAIHistoryModel::as_ref(ctx)
            .is_entirely_passive_conversation(&input.conversation_id);

        send_telemetry_from_ctx!(
            RequestFileEditsTelemetryEvent::EditReceived(EditReceivedEvent {
                identifiers: ai_identifiers.clone(),
                unique_files: file_edits.iter().map(|file| file.file()).unique().count(),
                diffs: file_edits.len(),
                passive_diff,
            }),
            ctx
        );

        let (tx, rx) = oneshot::channel();
        let files = file_edits.clone();
        let id = id.clone();
        let session_context = self.file_edit_preprocess_context(ctx);
        let fingerprint =
            file_edit_preprocess_fingerprint(input.conversation_id, input.action, &session_context);

        let apply_future = self.apply_diff_model.update(ctx, |model, ctx| {
            model.apply_diffs(files, &ai_identifiers, passive_diff, ctx)
        });

        ctx.spawn(
            async move {
                let applied_diffs = apply_future.await;
                (applied_diffs, id, fingerprint, session_context, tx)
            },
            |me, (diffs, id, fingerprint, session_context, tx), ctx| {
                me.on_diffs_applied(diffs, id, fingerprint, session_context, tx, ctx);
            },
        );

        async {
            rx.await.ok();
        }
        .boxed()
    }

    fn on_diffs_applied(
        &mut self,
        applied_diffs: Result<Vec<AIRequestedCodeDiff>, Vec1<DiffApplicationError>>,
        id: AIAgentActionId,
        fingerprint: FileEditPreprocessFingerprint,
        session_context: FileEditPreprocessContext,
        tx: oneshot::Sender<()>,
        ctx: &mut ModelContext<Self>,
    ) {
        tx.send(()).ok();
        self.preprocessed_actions
            .insert(id.clone(), fingerprint.clone());

        let Some(diff_view) = self.diff_views.get(&id) else {
            log::warn!(
                "Tried to apply diffs for a RequestFileEdits action without a corresponding diff view"
            );
            return;
        };

        let applied_diffs = match applied_diffs {
            Ok(diffs) if !diffs.is_empty() => diffs,
            Ok(_) => {
                // We didn't generate any diffs--consider this a failure.
                log::warn!("No diffs generated");
                self.diff_application_failures.insert(
                    id,
                    FileEditPreprocessFailure {
                        fingerprint,
                        errors: vec1![DiffApplicationError::EmptyDiff],
                    },
                );
                return;
            }
            Err(err) => {
                safe_warn!(
                    safe: ("Failed to generate diffs"),
                    full: ("Failed to generate diffs {err:?}")
                );
                self.diff_application_failures.insert(
                    id,
                    FileEditPreprocessFailure {
                        fingerprint,
                        errors: err,
                    },
                );
                return;
            }
        };
        self.diff_application_failures.remove(&id);

        let mut diffs = Vec::with_capacity(applied_diffs.len());
        for diff in applied_diffs {
            let path = host_native_absolute_path(
                diff.file_name.as_str(),
                &session_context.shell,
                &session_context.current_working_directory,
            );
            let file_diff = FileDiff::new(diff.original_content, path, diff.diff_type);
            diffs.push(file_diff);
        }

        // Set the session type on the diff view so save/delete/create routes
        // through the correct FileModel backend.
        let diff_session_type = match &session_context.session_type {
            Some(SessionType::WarpifiedRemote {
                host_id: Some(host_id),
            }) => DiffSessionType::Remote(host_id.clone()),
            _ => DiffSessionType::Local,
        };

        diff_view.update(ctx, |diff_view, ctx| {
            diff_view.set_diff_session_type(diff_session_type);
            diff_view.set_candidate_diffs(diffs, ctx);
        });
    }

    fn file_edit_preprocess_context(
        &self,
        ctx: &mut ModelContext<Self>,
    ) -> FileEditPreprocessContext {
        let active_session = self.active_session.as_ref(ctx);
        FileEditPreprocessContext {
            session_type: active_session.session_type(ctx),
            shell: active_session.shell_launch_data(ctx),
            current_working_directory: active_session.current_working_directory().cloned(),
        }
    }

    fn generate_ai_identifiers(
        &self,
        conversation_id: &AIConversationId,
        action_id: &AIAgentActionId,
        ctx: &mut ModelContext<Self>,
    ) -> Option<AIIdentifiers> {
        let history_model = BlocklistAIHistoryModel::as_ref(ctx);
        let conversation = history_model.conversation(conversation_id)?;

        // Find the `AIAgentExchange` and its corresponding `AIAgentOutput` for this given action.
        let (exchange, output) = conversation.all_exchanges().into_iter().find_map(|exchange| {
            let output = exchange.output_status.output()?;
            let contains_action = output.get().messages.iter().any(|step| {
                matches!(step, AIAgentOutputMessage{ message: AIAgentOutputMessageType::Action(AIAgentAction { id, .. }), .. } if id == action_id)
            });

            contains_action.then_some((exchange, output))
        })?;

        let server_output_id = output.get().server_output_id.clone();
        let model_id = output.get().model_info.as_ref().map(|m| m.model_id.clone());
        Some(AIIdentifiers {
            client_conversation_id: Some(*conversation_id),
            client_exchange_id: Some(exchange.id),
            server_output_id,
            server_conversation_id: conversation
                .server_conversation_token()
                .cloned()
                .map(Into::into),
            model_id,
        })
    }
}

impl Entity for RequestFileEditsExecutor {
    type Event = ();
}
