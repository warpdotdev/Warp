use itertools::Itertools;
use serde::{Deserialize, Serialize};
use warpui::ViewContext;

use crate::{
    ai::agent_providers::active_ai::workflow_metadata,
    send_telemetry_from_ctx,
    server::telemetry::TelemetryEvent,
    workflows::workflow::{Argument, Workflow},
};

use super::{
    arguments::ArgumentsState,
    modal::{AiAssistState, WorkflowModal, WorkflowModalEvent},
};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum GeneratedCommandMetadataError {
    /// OpenAI failed to generate a parsable response.
    BadCommand,
    /// Request to OpenAI failed
    AiProviderError,
    /// User is over rate limit.
    RateLimited,
    Other,
}

impl GeneratedCommandMetadataError {
    pub fn user_facing_message(&self) -> String {
        match self {
            Self::BadCommand => crate::t!("workflow-ai-assist-error-bad-command"),
            Self::AiProviderError => crate::t!("workflow-ai-assist-error-generic"),
            Self::RateLimited => crate::t!("workflow-ai-assist-error-rate-limited"),
            Self::Other => crate::t!("workflow-ai-assist-error-generic"),
        }
    }
}

impl WorkflowModal {
    /// 通过 BYOP one-shot completion 为命令生成 metadata,并把 AI 反馈
    /// 直接落到 modal 编辑器对应字段。无 BYOP 配置 → 直接 emit 错误事件。
    pub(super) fn issue_request(&mut self, ctx: &mut ViewContext<Self>) {
        let content = self.content_editor.as_ref(ctx).buffer_text(ctx);
        let raw_request = content.trim().to_string();

        let Some(rendered) = workflow_metadata::dispatch(
            ctx,
            None,
            workflow_metadata::Input {
                command: raw_request,
            },
        ) else {
            ctx.emit(WorkflowModalEvent::AiAssistError(crate::t!(
                "workflow-ai-assist-error-byop-required"
            )));
            return;
        };

        ctx.spawn(
            async move { workflow_metadata::run(rendered).await },
            move |modal, response, ctx| match response {
                Some(metadata) => {
                    modal.ai_metadata_assist_state = AiAssistState::Generated;
                    modal.enable_editors(ctx);

                    let arguments = metadata
                        .arguments
                        .into_iter()
                        .map(|parameter| Argument {
                            name: parameter.name,
                            description: Some(parameter.description),
                            default_value: Some(parameter.default_value),
                            arg_type: Default::default(),
                        })
                        .collect_vec();

                    let workflow = Workflow::Command {
                        name: metadata.title,
                        description: Some(metadata.description),
                        command: metadata.command,
                        arguments,
                        tags: vec![],
                        source_url: None,
                        author: None,
                        author_url: None,
                        shells: vec![],
                        environment_variables: None,
                    };

                    send_telemetry_from_ctx!(TelemetryEvent::AutoGenerateMetadataSuccess, ctx);

                    modal.populate_missing_field_with_suggestion(workflow, ctx);
                    ctx.notify();
                }
                None => {
                    let message = GeneratedCommandMetadataError::BadCommand.user_facing_message();
                    ctx.emit(WorkflowModalEvent::AiAssistError(message));

                    send_telemetry_from_ctx!(
                        TelemetryEvent::AutoGenerateMetadataError {
                            error_payload: serde_json::json!(
                                GeneratedCommandMetadataError::BadCommand
                            )
                        },
                        ctx
                    );

                    modal.ai_metadata_assist_state = AiAssistState::PreRequest;
                    modal.enable_editors(ctx);
                    ctx.notify();
                }
            },
        );

        self.ai_metadata_assist_state = AiAssistState::RequestInFlight;
        self.disable_editors(ctx);
        ctx.notify();
    }

    // Populate only the missing field in the workflow editor with the generated suggestion from AI.
    pub(super) fn populate_missing_field_with_suggestion(
        &mut self,
        workflow: Workflow,
        ctx: &mut ViewContext<Self>,
    ) {
        self.title_editor.update(ctx, |editor, ctx| {
            if editor.is_empty(ctx) {
                editor.set_buffer_text(workflow.name(), ctx);
            }
        });

        self.description_editor.update(ctx, |editor, ctx| {
            if editor.is_empty(ctx) {
                editor.set_buffer_text(
                    workflow
                        .description()
                        .map(String::as_str)
                        .unwrap_or_default(),
                    ctx,
                );
            }
        });

        let content_parsed = !self.arguments_state.arguments.is_empty();
        if !content_parsed {
            self.content_editor.update(ctx, |editor, ctx| {
                editor.set_buffer_text(workflow.content(), ctx);
            });

            // note: normally, we wouldn't have to do this, since editing the command
            // editor's text will trigger the event that does this automatically.
            // however, that happens in a callback, yet we need to know what the args
            // are right away to populate the description/default value editors.
            self.arguments_state = ArgumentsState::for_command_workflow(
                &self.arguments_state,
                workflow.content().to_string(),
            );
            self.update_arguments_rows(ctx);

            workflow
                .arguments()
                .iter()
                .enumerate()
                .for_each(|(index, argument)| {
                    // Since suggestion generated by AI is non-deterministic, we should make sure to handle each
                    // operation safely.
                    if index >= self.arguments_rows.len() {
                        return;
                    }

                    if let Some(description) = &argument.description {
                        self.arguments_rows[index]
                            .description_editor
                            .update(ctx, |editor, ctx| {
                                editor.set_buffer_text(description.as_str(), ctx);
                            });
                    }

                    if let Some(default_value) = &argument.default_value {
                        self.arguments_rows[index].default_value_editor.update(
                            ctx,
                            |editor, ctx| {
                                editor.set_buffer_text(default_value.as_str(), ctx);
                            },
                        );
                    }
                });
        }
    }
}
