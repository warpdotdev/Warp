use super::*;
use crate::ai::block_context::BlockContext;
use crate::ai::blocklist::SessionContext;
use crate::ai::llms::LLMId;
use crate::terminal::model::{block::BlockId, terminal_model::BlockIndex};
use ai::skills::{ParsedSkill, SkillProvider, SkillScope};
use futures_lite::{future::block_on, StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use warp_core::command::ExitCode;

fn request_params_for_test() -> RequestParams {
    let model = LLMId::from("test-model");

    RequestParams {
        input: vec![],
        primary_task_id: "test-task".to_owned(),
        conversation_token: None,
        forked_from_conversation_token: None,
        local_agent_run_id: None,
        tasks: vec![],
        existing_suggestions: None,
        metadata: None,
        session_context: SessionContext::new_for_test(),
        model: model.clone(),
        coding_model: model.clone(),
        cli_agent_model: model.clone(),
        computer_use_model: model,
        is_memory_enabled: false,
        mcp_context: None,
        planning_enabled: true,
        should_redact_secrets: false,
        api_keys: None,
        open_router_model: None,
        allow_use_of_warp_credits_with_byok: false,
        autonomy_level: api::AutonomyLevel::Supervised,
        isolation_level: api::IsolationLevel::None,
        web_search_enabled: false,
        computer_use_enabled: false,
        ask_user_question_enabled: false,
        research_agent_enabled: false,
        orchestration_enabled: false,
        supported_tools_override: None,
        parent_agent_id: None,
        agent_name: None,
    }
}

fn user_query_input(query: &str) -> AIAgentInput {
    AIAgentInput::UserQuery {
        query: query.to_owned(),
        context: Default::default(),
        static_query_type: None,
        referenced_attachments: Default::default(),
        user_query_mode: UserQueryMode::Normal,
        running_command: None,
        intended_agent: None,
    }
}

fn user_query_input_with_mode(query: &str, user_query_mode: UserQueryMode) -> AIAgentInput {
    AIAgentInput::UserQuery {
        query: query.to_owned(),
        context: Default::default(),
        static_query_type: None,
        referenced_attachments: Default::default(),
        user_query_mode,
        running_command: None,
        intended_agent: None,
    }
}

fn user_query_input_with_context(query: &str, context: Vec<AIAgentContext>) -> AIAgentInput {
    AIAgentInput::UserQuery {
        query: query.to_owned(),
        context: Arc::from(context),
        static_query_type: None,
        referenced_attachments: Default::default(),
        user_query_mode: UserQueryMode::Normal,
        running_command: None,
        intended_agent: None,
    }
}

fn shell_output_context(output: &str) -> AIAgentContext {
    AIAgentContext::Block(Box::new(BlockContext {
        id: BlockId::from("test-block".to_owned()),
        index: BlockIndex::zero(),
        command: String::new(),
        output: output.to_owned(),
        exit_code: ExitCode::from(127),
        is_auto_attached: false,
        started_ts: None,
        finished_ts: None,
        pwd: None,
        shell: None,
        username: None,
        hostname: None,
        git_branch: None,
        os: None,
        session_id: None,
    }))
}

fn user_query_message(query: &str, request_id: &str) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: "test-task".to_owned(),
        request_id: request_id.to_owned(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::UserQuery(message::UserQuery {
            query: query.to_owned(),
            context: None,
            referenced_attachments: Default::default(),
            mode: None,
            intended_agent: Default::default(),
        })),
    }
}

fn user_query_message_with_mode(
    query: &str,
    request_id: &str,
    mode: UserQueryMode,
) -> api::Message {
    let mut message = user_query_message(query, request_id);
    let Some(message::Message::UserQuery(user_query)) = message.message.as_mut() else {
        panic!("expected user query message");
    };
    user_query.mode = Some(api_user_query_mode(mode));
    message
}

#[allow(deprecated)]
fn user_query_message_with_context(query: &str, request_id: &str) -> api::Message {
    let mut message = user_query_message(query, request_id);
    let Some(message::Message::UserQuery(user_query)) = message.message.as_mut() else {
        panic!("expected user query message");
    };
    user_query.context = Some(api::InputContext {
        executed_shell_commands: vec![api::ExecutedShellCommand {
            command: "asdf".to_owned(),
            output: "zsh: command not found: asdf".to_owned(),
            exit_code: 127,
            command_id: "test-block".to_owned(),
            started_ts: None,
            finished_ts: None,
            is_auto_attached: false,
        }],
        ..Default::default()
    });
    message
}

fn agent_output_message(text: &str, request_id: &str) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: "test-task".to_owned(),
        request_id: request_id.to_owned(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::AgentOutput(message::AgentOutput {
            text: text.to_owned(),
        })),
    }
}

fn summarization_message(summary: &str, request_id: &str) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: "test-task".to_owned(),
        request_id: request_id.to_owned(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::Summarization(message::Summarization {
            finished_duration: None,
            summary_type: Some(message::summarization::SummaryType::ConversationSummary(
                message::summarization::ConversationSummary {
                    summary: summary.to_owned(),
                    token_count: 0,
                },
            )),
        })),
    }
}

fn test_skill(name: &str, content: &str) -> ParsedSkill {
    ParsedSkill {
        path: PathBuf::from(format!("/tmp/{name}/SKILL.md")),
        name: name.to_owned(),
        description: format!("{name} description"),
        content: content.to_owned(),
        line_range: None,
        provider: SkillProvider::Warp,
        scope: SkillScope::Bundled,
    }
}

fn invoke_skill_input(name: &str, query: &str, content: &str) -> AIAgentInput {
    AIAgentInput::InvokeSkill {
        context: Default::default(),
        skill: test_skill(name, content),
        user_query: Some(crate::ai::agent::InvokeSkillUserQuery {
            query: query.to_owned(),
            referenced_attachments: Default::default(),
        }),
    }
}

fn invoke_skill_message(name: &str, query: &str, content: &str) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: "test-task".to_owned(),
        request_id: "request-1".to_owned(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::InvokeSkill(message::InvokeSkill {
            skill: Some(test_skill(name, content).into()),
            user_query: Some(message::UserQuery {
                query: query.to_owned(),
                context: None,
                referenced_attachments: Default::default(),
                mode: None,
                intended_agent: Default::default(),
            }),
        })),
    }
}

fn tool_call(name: &str, arguments: serde_json::Value) -> OpenRouterToolCall {
    OpenRouterToolCall {
        id: format!("{name}-call"),
        function: OpenRouterFunctionCall {
            name: name.to_owned(),
            arguments: arguments.to_string(),
        },
    }
}

#[test]
fn invoke_skill_prompt_includes_skill_content_and_user_query() {
    let input = invoke_skill_input(
        "update-tab-config",
        "Update /tmp/tab.toml",
        "# update-tab-config\nRead before editing.",
    );

    let prompt = input_to_prompt_text(&input);

    assert!(prompt.contains("Invoked skill: /update-tab-config"));
    assert!(prompt.contains("Update /tmp/tab.toml"));
    assert!(prompt.contains("Read before editing."));
    assert_ne!(prompt.trim(), "/update-tab-config Update /tmp/tab.toml");
}

#[test]
fn plan_mode_prompt_preserves_slash_command_intent() {
    let input = user_query_input_with_mode("make granular commits", UserQueryMode::Plan);

    let prompt = input_to_prompt_text(&input);

    assert!(prompt.contains("Original user message: /plan make granular commits"));
    assert!(prompt.contains("Plan mode instructions"));
    assert!(prompt.contains("Do not modify files"));
    assert_ne!(prompt.trim(), "make granular commits");
}

#[test]
fn orchestrate_mode_prompt_preserves_slash_command_intent() {
    let input = user_query_input_with_mode("ship the release", UserQueryMode::Orchestrate);

    let prompt = input_to_prompt_text(&input);

    assert!(prompt.contains("Original user message: /orchestrate ship the release"));
    assert!(prompt.contains("Orchestrate mode instructions"));
    assert_ne!(prompt.trim(), "ship the release");
}

#[test]
fn input_messages_for_task_persists_invoke_skill() {
    let input = invoke_skill_input(
        "update-tab-config",
        "Update /tmp/tab.toml",
        "# update-tab-config",
    );

    let messages = input_messages_for_task(&[input], "test-task", "request-1", None);

    assert_eq!(messages.len(), 1);
    let Some(message::Message::InvokeSkill(invoke_skill)) = messages[0].message.as_ref() else {
        panic!("expected invoke skill message");
    };
    assert_eq!(
        invoke_skill
            .skill
            .as_ref()
            .and_then(|skill| skill.descriptor.as_ref())
            .map(|descriptor| descriptor.name.as_str()),
        Some("update-tab-config")
    );
    assert_eq!(
        invoke_skill
            .user_query
            .as_ref()
            .map(|query| query.query.as_str()),
        Some("Update /tmp/tab.toml")
    );
}

#[test]
fn request_messages_include_prior_invoke_skill_history() {
    let mut params = request_params_for_test();
    params.tasks.push(api::Task {
        id: "test-task".to_owned(),
        messages: vec![
            invoke_skill_message(
                "update-tab-config",
                "Update /tmp/tab.toml",
                "# update-tab-config",
            ),
            agent_output_message("Done", "request-1"),
        ],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    });
    params.input = vec![user_query_input("what was the first message i sent you?")];

    let messages = build_openrouter_messages(&params);

    assert_eq!(messages[1].role, "user");
    assert!(messages[1]
        .content
        .contains("Invoked skill: /update-tab-config"));
    assert!(messages[1].content.contains("Update /tmp/tab.toml"));
    assert!(messages[1].content.contains("# update-tab-config"));
    assert_eq!(messages[2].role, "assistant");
    assert_eq!(
        messages[3].content,
        "what was the first message i sent you?"
    );
}

#[test]
fn request_messages_include_current_and_prior_plan_mode() {
    let current = user_query_input_with_mode("make granular commits", UserQueryMode::Plan);
    let mut params = request_params_for_test();
    params.input = vec![current];

    let messages = build_openrouter_messages(&params);

    assert!(messages
        .last()
        .expect("request should include current user message")
        .content
        .contains("Original user message: /plan make granular commits"));

    params.tasks.push(api::Task {
        id: "test-task".to_owned(),
        messages: vec![
            user_query_message_with_mode("make granular commits", "request-1", UserQueryMode::Plan),
            agent_output_message("Plan: inspect changes, group commits.", "request-1"),
        ],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    });
    params.input = vec![user_query_input("what was the first message i sent you?")];

    let messages = build_openrouter_messages(&params);

    assert!(messages[1]
        .content
        .contains("Original user message: /plan make granular commits"));
    assert_eq!(
        messages.last().expect("follow-up message").content,
        "what was the first message i sent you?"
    );
}

#[test]
fn request_messages_include_current_and_prior_orchestrate_mode() {
    let current = user_query_input_with_mode("ship the release", UserQueryMode::Orchestrate);
    let mut params = request_params_for_test();
    params.input = vec![current];

    let messages = build_openrouter_messages(&params);

    assert!(messages
        .last()
        .expect("request should include current user message")
        .content
        .contains("Original user message: /orchestrate ship the release"));

    params.tasks.push(api::Task {
        id: "test-task".to_owned(),
        messages: vec![
            user_query_message_with_mode(
                "ship the release",
                "request-1",
                UserQueryMode::Orchestrate,
            ),
            agent_output_message("Plan the release work across local steps.", "request-1"),
        ],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    });
    params.input = vec![user_query_input("what was the first message i sent you?")];

    let messages = build_openrouter_messages(&params);

    assert!(messages[1]
        .content
        .contains("Original user message: /orchestrate ship the release"));
    assert_eq!(
        messages.last().expect("follow-up message").content,
        "what was the first message i sent you?"
    );
}

#[test]
fn static_agent_request_prompts_include_structured_details() {
    let create = AIAgentInput::CreateNewProject {
        query: "a todo app in Rust".to_owned(),
        context: Default::default(),
    };
    let fetch_comments = AIAgentInput::FetchReviewComments {
        repo_path: "/repo/project".to_owned(),
        context: Default::default(),
    };
    let init = AIAgentInput::InitProjectRules {
        context: Default::default(),
        display_query: Some(commands::INIT.name.to_owned()),
    };

    assert!(input_to_prompt_text(&create)
        .contains("Original user message: /create-new-project a todo app in Rust"));
    let fetch_prompt = input_to_prompt_text(&fetch_comments);
    assert!(fetch_prompt.contains("Original user message: /pr-comments"));
    assert!(fetch_prompt.contains("/repo/project"));
    assert!(input_to_prompt_text(&init).contains("Original user message: /init"));
}

#[test]
fn input_messages_for_task_persists_static_agent_requests() {
    let inputs = vec![
        AIAgentInput::CreateNewProject {
            query: "a todo app in Rust".to_owned(),
            context: Default::default(),
        },
        AIAgentInput::CloneRepository {
            clone_repo_url: crate::ai::agent::CloneRepositoryURL::new(
                "https://example.com/repo.git".to_owned(),
            ),
            context: Default::default(),
        },
        AIAgentInput::FetchReviewComments {
            repo_path: "/repo/project".to_owned(),
            context: Default::default(),
        },
        AIAgentInput::SummarizeConversation {
            prompt: Some("focus on decisions".to_owned()),
        },
    ];

    let messages = input_messages_for_task(&inputs, "test-task", "request-1", None);

    assert_eq!(messages.len(), 4);
    let expected = [
        "Original user message: /create-new-project a todo app in Rust",
        "Clone repository:\nhttps://example.com/repo.git",
        "Original user message: /pr-comments",
        "Original user message: /compact focus on decisions",
    ];
    for (message, expected) in messages.iter().zip(expected) {
        let replayed = api_message_to_openrouter_message(message).expect("message should replay");
        assert!(
            replayed.content.contains(expected),
            "replayed content did not contain {expected:?}: {}",
            replayed.content
        );
    }
}

#[test]
fn openrouter_tools_include_local_first_tool_subset() {
    let tool_names = openrouter_tools()
        .into_iter()
        .map(|tool| tool.function.name)
        .collect::<Vec<_>>();

    assert!(tool_names.contains(&"run_shell_command"));
    assert!(tool_names.contains(&"read_files"));
    assert!(tool_names.contains(&"grep"));
    assert!(tool_names.contains(&"file_glob_v2"));
    assert!(tool_names.contains(&"search_codebase"));
    assert!(tool_names.contains(&"apply_file_diffs"));
    assert!(tool_names.contains(&"read_skill"));
    assert!(tool_names.contains(&"ask_user_question"));
}

#[test]
fn run_shell_command_conversion_respects_explicit_uses_pager() {
    let explicit_tool = openrouter_tool_call_to_api_tool(&tool_call(
        "run_shell_command",
        json!({ "command": "git diff --stat", "uses_pager": false }),
    ))
    .expect("tool should convert");
    let message::tool_call::Tool::RunShellCommand(command) = explicit_tool else {
        panic!("expected run_shell_command");
    };
    assert!(!command.uses_pager);
}

#[test]
fn run_shell_command_conversion_does_not_guess_pager_usage() {
    let tool = openrouter_tool_call_to_api_tool(&tool_call(
        "run_shell_command",
        json!({ "command": "git diff --stat" }),
    ))
    .expect("tool should convert");
    let message::tool_call::Tool::RunShellCommand(command) = tool else {
        panic!("expected run_shell_command");
    };
    assert!(!command.uses_pager);
}

#[test]
fn openrouter_tool_calls_convert_to_local_tool_messages() {
    let calls = vec![
        tool_call("read_files", json!({ "files": [{ "path": "Cargo.toml" }] })),
        tool_call(
            "grep",
            json!({ "queries": ["InvokeSkill"], "path": "app/src" }),
        ),
        tool_call(
            "file_glob_v2",
            json!({ "patterns": ["*.rs"], "search_dir": "app/src" }),
        ),
        tool_call(
            "search_codebase",
            json!({ "query": "OpenRouter skill history" }),
        ),
        tool_call(
            "apply_file_diffs",
            json!({
                "summary": "Update file",
                "diffs": [{
                    "file_path": "test.txt",
                    "search": "old",
                    "replace": "new"
                }]
            }),
        ),
        tool_call("read_skill", json!({ "name": "tab-configs" })),
        tool_call(
            "ask_user_question",
            json!({
                "questions": [{
                    "question": "Which path?",
                    "options": [{ "label": "A" }, { "label": "B" }]
                }]
            }),
        ),
    ];

    let converted = calls
        .into_iter()
        .map(|call| openrouter_tool_call_to_api_tool(&call).expect("tool should convert"))
        .collect::<Vec<_>>();

    assert!(matches!(
        converted[0],
        message::tool_call::Tool::ReadFiles(_)
    ));
    assert!(matches!(converted[1], message::tool_call::Tool::Grep(_)));
    assert!(matches!(
        converted[2],
        message::tool_call::Tool::FileGlobV2(_)
    ));
    assert!(matches!(
        converted[3],
        message::tool_call::Tool::SearchCodebase(_)
    ));
    assert!(matches!(
        converted[4],
        message::tool_call::Tool::ApplyFileDiffs(_)
    ));
    assert!(matches!(
        converted[5],
        message::tool_call::Tool::ReadSkill(_)
    ));
    let message::tool_call::Tool::ReadSkill(read_skill) = &converted[5] else {
        panic!("expected read_skill tool call");
    };
    assert_eq!(read_skill.name, "tab-configs");
    assert_eq!(
        read_skill.skill_reference.as_ref(),
        Some(
            &api::message::tool_call::read_skill::SkillReference::BundledSkillId(
                "tab-configs".to_owned()
            )
        )
    );
    assert!(matches!(
        converted[6],
        message::tool_call::Tool::AskUserQuestion(_)
    ));
}

fn openrouter_tool_result_message(result: message::tool_call_result::Result) -> OpenRouterMessage {
    let message = api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: "test-task".to_owned(),
        request_id: "request-1".to_owned(),
        timestamp: None,
        server_message_data: String::new(),
        citations: vec![],
        message: Some(message::Message::ToolCallResult(message::ToolCallResult {
            tool_call_id: "tool-call".to_owned(),
            context: None,
            result: Some(result),
        })),
    };

    api_message_to_openrouter_message(&message).expect("message should replay")
}

#[test]
fn read_files_tool_history_replays_to_openrouter_messages() {
    let openrouter_message = openrouter_tool_result_message(
        message::tool_call_result::Result::ReadFiles(api::ReadFilesResult {
            result: Some(api::read_files_result::Result::TextFilesSuccess(
                api::read_files_result::TextFilesSuccess {
                    files: vec![api::FileContent {
                        file_path: "Cargo.toml".to_owned(),
                        content: "[package]\nname = \"warper\"".to_owned(),
                        line_range: None,
                    }],
                },
            )),
        }),
    );

    assert_eq!(openrouter_message.role, "user");
    assert!(openrouter_message.content.contains("Tool result:"));
    assert!(openrouter_message.content.contains("File: Cargo.toml"));
    assert!(openrouter_message.content.contains("name = \"warper\""));
}

#[test]
fn local_search_and_skill_tool_history_replays_to_openrouter_messages() {
    let grep =
        openrouter_tool_result_message(message::tool_call_result::Result::Grep(api::GrepResult {
            result: Some(api::grep_result::Result::Success(
                api::grep_result::Success {
                    matched_files: vec![api::grep_result::success::GrepFileMatch {
                        file_path: "app/src/ai/agent/api/openrouter.rs".to_owned(),
                        matched_lines: vec![
                            api::grep_result::success::grep_file_match::GrepLineMatch {
                                line_number: 791,
                            },
                        ],
                    }],
                },
            )),
        }));
    assert!(grep.content.contains("openrouter.rs: lines [791]"));

    let glob = openrouter_tool_result_message(message::tool_call_result::Result::FileGlobV2(
        api::FileGlobV2Result {
            result: Some(api::file_glob_v2_result::Result::Success(
                api::file_glob_v2_result::Success {
                    matched_files: vec![api::file_glob_v2_result::success::FileGlobMatch {
                        file_path: "app/src/ai/agent/api/openrouter.rs".to_owned(),
                    }],
                    warnings: "skipped unreadable directory".to_owned(),
                },
            )),
        },
    ));
    assert!(glob.content.contains("app/src/ai/agent/api/openrouter.rs"));
    assert!(glob.content.contains("Warnings:"));

    let read_skill = openrouter_tool_result_message(message::tool_call_result::Result::ReadSkill(
        api::ReadSkillResult {
            result: Some(api::read_skill_result::Result::Success(
                api::read_skill_result::Success {
                    content: Some(api::FileContent {
                        file_path: "skills/update-tab-config/SKILL.md".to_owned(),
                        content: "# update-tab-config\nUse tab-configs.".to_owned(),
                        line_range: None,
                    }),
                },
            )),
        },
    ));
    assert!(read_skill
        .content
        .contains("File: skills/update-tab-config/SKILL.md"));
    assert!(read_skill.content.contains("Use tab-configs."));
}

#[test]
fn edit_and_user_question_tool_history_replays_to_openrouter_messages() {
    let edits = openrouter_tool_result_message(message::tool_call_result::Result::ApplyFileDiffs(
        api::ApplyFileDiffsResult {
            result: Some(api::apply_file_diffs_result::Result::Success(
                api::apply_file_diffs_result::Success {
                    #[allow(deprecated)]
                    updated_files: vec![],
                    updated_files_v2: vec![
                        api::apply_file_diffs_result::success::UpdatedFileContent {
                            file: Some(api::FileContent {
                                file_path: "README.md".to_owned(),
                                content: "updated".to_owned(),
                                line_range: None,
                            }),
                            was_edited_by_user: false,
                        },
                    ],
                    deleted_files: vec![api::apply_file_diffs_result::success::DeletedFile {
                        file_path: "old.md".to_owned(),
                    }],
                },
            )),
        },
    ));
    assert!(edits.content.contains("File edits applied."));
    assert!(edits.content.contains("Updated: README.md"));
    assert!(edits.content.contains("Deleted: old.md"));

    let answer = openrouter_tool_result_message(
        message::tool_call_result::Result::AskUserQuestion(api::AskUserQuestionResult {
            result: Some(api::ask_user_question_result::Result::Success(
                api::ask_user_question_result::Success {
                    answers: vec![api::ask_user_question_result::AnswerItem {
                        question_id: "target_path".to_owned(),
                        answer: Some(
                            api::ask_user_question_result::answer_item::Answer::MultipleChoice(
                                api::ask_user_question_result::answer_item::MultipleChoiceAnswer {
                                    selected_options: vec!["Use current file".to_owned()],
                                    other_text: "and keep local-only".to_owned(),
                                },
                            ),
                        ),
                    }],
                },
            )),
        }),
    );
    assert!(answer
        .content
        .contains("target_path: Use current file, and keep local-only"));
}

#[test]
fn output_creates_task_before_adding_messages_for_new_conversation() {
    let params = request_params_for_test();
    let events = block_on(generate_openrouter_output(params).collect::<Vec<_>>());

    assert!(matches!(
        events[0].as_ref().unwrap().r#type.as_ref().unwrap(),
        response_event::Type::Init(_)
    ));

    let response_event::Type::ClientActions(create_actions) =
        events[1].as_ref().unwrap().r#type.as_ref().unwrap()
    else {
        panic!("expected CreateTask client action");
    };
    let Some(client_action::Action::CreateTask(create)) = create_actions.actions[0].action.as_ref()
    else {
        panic!("expected CreateTask action");
    };
    assert_eq!(create.task.as_ref().unwrap().id, "test-task");

    let response_event::Type::ClientActions(add_actions) =
        events[2].as_ref().unwrap().r#type.as_ref().unwrap()
    else {
        panic!("expected AddMessagesToTask client action");
    };
    let Some(client_action::Action::AddMessagesToTask(add)) =
        add_actions.actions[0].action.as_ref()
    else {
        panic!("expected AddMessagesToTask action");
    };
    assert_eq!(add.task_id, "test-task");
    assert!(add
        .messages
        .iter()
        .any(|message| matches!(message.message, Some(message::Message::AgentOutput(_)))));
}

#[test]
#[allow(deprecated)]
fn output_persists_current_user_query_message() {
    let mut params = request_params_for_test();
    params.input = vec![user_query_input_with_context(
        "remember this",
        vec![shell_output_context("zsh: command not found: asdf\n")],
    )];

    let events = block_on(generate_openrouter_output(params).collect::<Vec<_>>());

    let response_event::Type::ClientActions(add_actions) =
        events[2].as_ref().unwrap().r#type.as_ref().unwrap()
    else {
        panic!("expected AddMessagesToTask client action");
    };
    let Some(client_action::Action::AddMessagesToTask(add)) =
        add_actions.actions[0].action.as_ref()
    else {
        panic!("expected AddMessagesToTask action");
    };

    let persisted_query = add.messages.iter().find_map(|message| {
        let Some(message::Message::UserQuery(query)) = message.message.as_ref() else {
            return None;
        };
        (query.query == "remember this").then_some(query)
    });
    assert!(persisted_query.is_some());
    assert!(persisted_query
        .and_then(|query| query.context.as_ref())
        .is_some_and(|context| !context.executed_shell_commands.is_empty()));
}

#[test]
fn request_messages_include_current_user_query_context() {
    let mut params = request_params_for_test();
    params.input = vec![user_query_input_with_context(
        "what is the error i'm attaching?",
        vec![shell_output_context("zsh: command not found: asdf\n")],
    )];

    let messages = build_openrouter_messages(&params);

    let user_message = messages.last().expect("expected current user message");
    assert_eq!(user_message.role, "user");
    assert!(user_message
        .content
        .contains("what is the error i'm attaching?"));
    assert!(user_message.content.contains("Attached terminal output"));
    assert!(user_message
        .content
        .contains("zsh: command not found: asdf"));
}

#[test]
fn request_messages_include_prior_task_history() {
    let mut params = request_params_for_test();
    params.tasks.push(api::Task {
        id: "test-task".to_owned(),
        messages: vec![
            user_query_message("first question", "request-1"),
            agent_output_message("first answer", "request-1"),
        ],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    });
    params.input = vec![user_query_input("follow up")];

    let messages = build_openrouter_messages(&params);

    assert_eq!(messages[0].role, "system");
    assert_eq!(messages[1].role, "user");
    assert_eq!(messages[1].content, "first question");
    assert_eq!(messages[2].role, "assistant");
    assert_eq!(messages[2].content, "first answer");
    assert_eq!(messages[3].role, "user");
    assert_eq!(messages[3].content, "follow up");
}

#[test]
fn compact_history_replays_from_latest_summary_boundary() {
    let mut params = request_params_for_test();
    params.tasks.push(api::Task {
        id: "test-task".to_owned(),
        messages: vec![
            user_query_message("first question", "request-1"),
            agent_output_message("first answer", "request-1"),
            summarization_message("summary of the compacted conversation", "request-2"),
        ],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    });
    params.input = vec![user_query_input("follow up after compact")];

    let messages = build_openrouter_messages(&params);

    assert_eq!(messages[0].role, "system");
    assert_eq!(messages[1].role, "user");
    assert_eq!(
        messages[1].content,
        "Compacted conversation summary:\nsummary of the compacted conversation"
    );
    assert_eq!(messages[2].role, "user");
    assert_eq!(messages[2].content, "follow up after compact");

    let replay = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!replay.contains("first question"));
    assert!(!replay.contains("first answer"));
}

#[test]
fn compact_output_persists_summarization_message() {
    let input = vec![AIAgentInput::SummarizeConversation { prompt: None }];
    let event = add_messages_event(
        "test-task".to_owned(),
        "request-1".to_owned(),
        "test-model".to_owned(),
        &input,
        Some("summary of the conversation".to_owned()),
        vec![],
    );

    let response_event::Type::ClientActions(add_actions) = event.r#type.as_ref().unwrap() else {
        panic!("expected ClientActions event");
    };
    let Some(client_action::Action::AddMessagesToTask(add)) =
        add_actions.actions[0].action.as_ref()
    else {
        panic!("expected AddMessagesToTask action");
    };

    let persisted_summary = add.messages.iter().find_map(|message| {
        let Some(message::Message::Summarization(summarization)) = message.message.as_ref() else {
            return None;
        };
        conversation_summary_text_from_summarization(summarization)
    });

    assert_eq!(persisted_summary, Some("summary of the conversation"));
    assert!(!add
        .messages
        .iter()
        .any(|message| matches!(message.message, Some(message::Message::AgentOutput(_)))));
}

#[test]
fn request_messages_include_prior_user_query_context() {
    let mut params = request_params_for_test();
    params.tasks.push(api::Task {
        id: "test-task".to_owned(),
        messages: vec![user_query_message_with_context(
            "what is the error i'm attaching?",
            "request-1",
        )],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    });

    let messages = build_openrouter_messages(&params);

    assert_eq!(messages[1].role, "user");
    assert!(messages[1]
        .content
        .contains("what is the error i'm attaching?"));
    assert!(messages[1].content.contains("Attached terminal command"));
    assert!(messages[1].content.contains("asdf"));
    assert!(messages[1].content.contains("zsh: command not found: asdf"));
}

#[test]
fn output_does_not_recreate_existing_task() {
    let mut params = request_params_for_test();
    params.tasks.push(api::Task {
        id: "test-task".to_owned(),
        messages: vec![],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    });

    let events = block_on(generate_openrouter_output(params).collect::<Vec<_>>());

    assert!(matches!(
        events[0].as_ref().unwrap().r#type.as_ref().unwrap(),
        response_event::Type::Init(_)
    ));

    let response_event::Type::ClientActions(add_actions) =
        events[1].as_ref().unwrap().r#type.as_ref().unwrap()
    else {
        panic!("expected AddMessagesToTask client action");
    };
    assert!(matches!(
        add_actions.actions[0].action.as_ref().unwrap(),
        client_action::Action::AddMessagesToTask(_)
    ));
}
