use std::{collections::HashMap, sync::Arc};

use warp_core::features::FeatureFlag;
use warpui::{AppContext, ModelContext, SingletonEntity};

use crate::{
    ai::{
        agent::{
            conversation::AIConversationId, AIAgentAttachment, AIAgentContext, AIAgentInput,
            CloneRepositoryURL, EntrypointType, RequestMetadata, UserQueryMode,
        },
        blocklist::agent_view::AgentViewEntryOrigin,
    },
    search::slash_command_menu::static_commands::commands,
    terminal::input::slash_commands::SlashCommandTrigger,
    BlocklistAIHistoryModel,
};

use super::{
    input_context_for_request, parse_context_attachments, BlocklistAIController,
    BlocklistAIControllerEvent, RequestInput,
};

pub enum SlashCommandRequest {
    CreateNewProject {
        query: String,
    },
    CloneRepository {
        url: String,
    },
    InitProjectRules {
        arguments: Option<String>,
    },
    Summarize {
        prompt: Option<String>,
        /// OpenWarp BYOP 本地会话压缩:本次摘要是否由 token-overflow 自动触发。
        /// chat_stream::SummarizeConversation 分支据此决定 follow-up 文案
        /// (overflow 路径会拼一段 "previous request exceeded ..." 解释)。
        /// /compact /compact-and 手动触发时为 false;auto-trigger 路径为 true。
        overflow: bool,
    },
    FetchReviewComments {
        repo_path: String,
    },
    /// Invoke a skill.
    InvokeSkill {
        skill: ai::skills::ParsedSkill,
        user_query: Option<String>,
    },
}

impl SlashCommandRequest {
    /// Parses user input into a SlashCommandRequest for slash commands that are handled
    /// via the AI query flow (as opposed to action-based slash commands handled in input.rs).
    pub fn from_query(query: &str) -> Option<SlashCommandRequest> {
        if query == commands::INIT_NAME {
            return Some(Self::InitProjectRules { arguments: None });
        }
        if let Some(arguments) = query
            .strip_prefix(commands::INIT_NAME)
            .and_then(|query| query.strip_prefix(' '))
        {
            return Some(Self::InitProjectRules {
                arguments: Some(arguments.to_string()),
            });
        }

        // Check if query starts with /compact and route to summarize conversation
        if let Some(prompt) = query.strip_prefix(commands::COMPACT.name) {
            return Some(Self::Summarize {
                prompt: prompt.strip_prefix(' ').map(String::from),
                overflow: false, // 文本输入路径只用于手动 /compact,永不为自动 overflow
            });
        }

        None
    }

    pub(super) fn send_request(
        self,
        controller: &mut BlocklistAIController,
        is_queued_prompt: bool,
        ctx: &mut ModelContext<BlocklistAIController>,
    ) {
        let conversation_id = self.conversation_id(controller, ctx);
        // For skill invocations, include user-attached context (images, blocks, and selected
        // text) so the skill's agent sees the same attachments a non-slash-command user query
        // would. Other slash commands continue to pass `false` to preserve existing behavior.
        let is_invoke_skill = matches!(self, Self::InvokeSkill { .. });
        let context = input_context_for_request(
            is_invoke_skill,
            controller.context_model.as_ref(ctx),
            controller.active_session.as_ref(ctx),
            conversation_id,
            vec![],
            ctx,
        );
        let entrypoint = self.entrypoint();
        let is_summarize = matches!(self, Self::Summarize { .. });
        let inputs = self.input(context, controller.context_model.as_ref(ctx), ctx);
        if inputs.is_empty() {
            return;
        }

        // If no existing conversation, create a new one.
        // When AgentView is enabled, enter agent view which creates the conversation
        // and ensures AI blocks render correctly in the agent view.
        let Some(conversation_id) = conversation_id.or_else(|| {
            if FeatureFlag::AgentView.is_enabled() {
                controller.context_model.update(ctx, |context_model, ctx| {
                    context_model
                        .try_enter_agent_view_for_new_conversation(
                            AgentViewEntryOrigin::SlashCommand {
                                trigger: SlashCommandTrigger::input(),
                            },
                            ctx,
                        )
                        .ok()
                })
            } else {
                Some(controller.start_new_conversation_for_request(ctx).id())
            }
        }) else {
            log::error!("Failed to get conversation ID for slash command request");
            return;
        };

        let Some(conversation) =
            BlocklistAIHistoryModel::as_ref(ctx).conversation(&conversation_id)
        else {
            return;
        };

        let request_input = RequestInput::for_task(
            inputs,
            conversation.get_root_task_id().clone(),
            &controller.active_session,
            controller.get_current_response_initiator(),
            conversation_id,
            controller.terminal_view_id,
            ctx,
        );
        let model_id = request_input.model_id.clone();

        match controller.send_request_input(
            request_input,
            Some(RequestMetadata {
                is_autodetected_user_query: false,
                entrypoint,
                is_auto_resume_after_error: false,
            }),
            /*default_to_follow_up_on_success*/ true,
            /*can_attempt_resume_on_error*/ true,
            is_queued_prompt,
            ctx,
        ) {
            Ok((_, stream_id)) => {
                // Skill invocations now consume user-attached context (images, blocks, and
                // selected text) the same way regular user queries do. `send_request_input`
                // only clears that context for `AIAgentInput::UserQuery`, so we mirror its
                // reset here for `InvokeSkill` to avoid pending attachments sticking around
                // and getting re-sent on subsequent messages.
                if is_invoke_skill {
                    controller.context_model.update(ctx, |context_model, ctx| {
                        context_model.reset_context_to_default(ctx);
                    });
                }
                // Emit SentRequest event to trigger buffer clearing
                if is_summarize {
                    ctx.emit(BlocklistAIControllerEvent::SentRequest {
                        contains_user_query: true,
                        is_queued_prompt,
                        model_id,
                        stream_id,
                    });
                }
            }
            Err(e) => log::error!("Failed to send agent slash command request: {e:?}"),
        }
    }

    pub(super) fn conversation_id(
        &self,
        controller: &BlocklistAIController,
        app: &AppContext,
    ) -> Option<AIConversationId> {
        match self {
            Self::Summarize { .. }
            | Self::InvokeSkill { .. }
            | Self::FetchReviewComments { .. } => controller
                .context_model
                .as_ref(app)
                .selected_conversation_id(app),
            _ => None,
        }
    }

    fn input(
        self,
        context: Arc<[AIAgentContext]>,
        context_model: &crate::ai::blocklist::BlocklistAIContextModel,
        app: &AppContext,
    ) -> Vec<AIAgentInput> {
        match self {
            SlashCommandRequest::CreateNewProject { query } => {
                vec![AIAgentInput::CreateNewProject { query, context }]
            }
            SlashCommandRequest::CloneRepository { url } => {
                vec![AIAgentInput::CloneRepository {
                    clone_repo_url: CloneRepositoryURL::new(url),
                    context,
                }]
            }
            SlashCommandRequest::InitProjectRules { arguments } => vec![AIAgentInput::UserQuery {
                query: crate::ai::agent_providers::prompt_renderer::render_init_project_command(
                    arguments.as_deref(),
                ),
                context,
                static_query_type: None,
                referenced_attachments: HashMap::<String, AIAgentAttachment>::new(),
                user_query_mode: UserQueryMode::Normal,
                running_command: None,
                intended_agent: None,
            }],
            SlashCommandRequest::Summarize { prompt, overflow } => {
                vec![AIAgentInput::SummarizeConversation { prompt, overflow }]
            }
            SlashCommandRequest::FetchReviewComments { repo_path } => {
                vec![AIAgentInput::FetchReviewComments { repo_path, context }]
            }
            SlashCommandRequest::InvokeSkill { skill, user_query } => {
                let user_query = if FeatureFlag::SkillArguments.is_enabled() {
                    user_query
                        .map(|query| query.trim().to_string())
                        .filter(|query| !query.is_empty())
                        .map(|query| crate::ai::agent::InvokeSkillUserQuery {
                            referenced_attachments: parse_context_attachments(
                                &query,
                                context_model,
                                app,
                            ),
                            query,
                        })
                } else {
                    None
                };
                vec![AIAgentInput::InvokeSkill {
                    skill,
                    user_query,
                    context,
                }]
            }
        }
    }

    fn entrypoint(&self) -> EntrypointType {
        match self {
            SlashCommandRequest::CloneRepository { .. } => EntrypointType::CloneRepository,
            SlashCommandRequest::InitProjectRules { .. } => EntrypointType::InitProjectRules,
            SlashCommandRequest::CreateNewProject { .. }
            | SlashCommandRequest::Summarize { .. }
            | SlashCommandRequest::FetchReviewComments { .. }
            | SlashCommandRequest::InvokeSkill { .. } => EntrypointType::UserInitiated,
        }
    }
}
