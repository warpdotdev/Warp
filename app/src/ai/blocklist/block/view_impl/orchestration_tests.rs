use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::agent::{StartAgentExecutionMode, StartAgentResult};
use crate::BlocklistAIHistoryModel;
use ai::agent::action_result::StartAgentVersion;
use warp_core::ui::appearance::Appearance;
use warpui::elements::MouseStateHandle;
use warpui::{App, EntityId};

use super::{
    agent_display_name_from_id, child_conversation_card_data_for_result, compute_validation_errors,
    display_label_for_option, render_conversation_navigation_card_row,
    start_agent_cancelled_prefix, start_agent_error_prefix, start_agent_in_progress_prefix,
    start_agent_success_suffix, ChildConversationCardData, HARNESS_OPTIONS, MODEL_OPTIONS,
};

#[test]
fn child_conversation_card_data_for_success_result_returns_conversation_id_and_title() {
    App::test((), |mut app| async move {
        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
        let conversation_id = history_model.update(&mut app, |history_model, ctx| {
            let conversation_id =
                history_model.start_new_conversation(EntityId::new(), false, false, ctx);
            history_model.set_server_conversation_token_for_conversation(
                conversation_id,
                "child-agent-id".to_string(),
            );
            history_model
                .conversation_mut(&conversation_id)
                .expect("conversation should exist")
                .set_fallback_display_title("Generated child title".to_string());
            conversation_id
        });
        let result = StartAgentResult::Success {
            agent_id: "child-agent-id".to_string(),
            version: StartAgentVersion::V1,
        };
        let actual = app.read(|ctx| child_conversation_card_data_for_result(&result, ctx));
        assert_eq!(
            actual,
            Some(ChildConversationCardData {
                conversation_id,
                agent_name: "Agent".to_string(),
                title: "Generated child title".to_string(),
                status: ConversationStatus::InProgress,
            })
        );
    });
}

#[test]
fn start_agent_copy_uses_local_labels_for_local_children() {
    let execution_mode = StartAgentExecutionMode::local_harness("claude-code".to_string());

    assert_eq!(start_agent_success_suffix(&execution_mode), " locally.");
    assert_eq!(
        start_agent_error_prefix(&execution_mode),
        "Failed to start agent "
    );
    assert_eq!(
        start_agent_cancelled_prefix(&execution_mode),
        "Start agent "
    );
    assert_eq!(
        start_agent_in_progress_prefix(&execution_mode),
        "Starting agent "
    );
}

#[test]
fn start_agent_copy_uses_remote_labels_for_remote_children() {
    let execution_mode = StartAgentExecutionMode::Remote {
        environment_id: "env-123".to_string(),
        skill_references: vec![],
        model_id: String::new(),
        computer_use_enabled: false,
        worker_host: String::new(),
        harness_type: String::new(),
        title: String::new(),
    };

    assert_eq!(start_agent_success_suffix(&execution_mode), " remotely.");
    assert_eq!(
        start_agent_error_prefix(&execution_mode),
        "Failed to start remote agent "
    );
    assert_eq!(
        start_agent_cancelled_prefix(&execution_mode),
        "Start remote agent "
    );
    assert_eq!(
        start_agent_in_progress_prefix(&execution_mode),
        "Starting remote agent "
    );
}

#[test]
fn child_conversation_card_data_for_success_result_without_available_title_uses_placeholder() {
    App::test((), |mut app| async move {
        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
        let conversation_id = history_model.update(&mut app, |history_model, ctx| {
            let conversation_id =
                history_model.start_new_conversation(EntityId::new(), false, false, ctx);
            history_model.set_server_conversation_token_for_conversation(
                conversation_id,
                "child-agent-id".to_string(),
            );
            conversation_id
        });
        let result = StartAgentResult::Success {
            agent_id: "child-agent-id".to_string(),
            version: StartAgentVersion::V1,
        };
        let actual = app.read(|ctx| child_conversation_card_data_for_result(&result, ctx));
        assert_eq!(
            actual,
            Some(ChildConversationCardData {
                conversation_id,
                agent_name: "Agent".to_string(),
                title: "Generating title...".to_string(),
                status: ConversationStatus::InProgress,
            })
        );
    });
}

#[test]
fn child_conversation_card_data_for_non_success_result_returns_none() {
    App::test((), |app| async move {
        let error_result = StartAgentResult::Error {
            error: "boom".to_string(),
            version: StartAgentVersion::V1,
        };
        let error_actual =
            app.read(|ctx| child_conversation_card_data_for_result(&error_result, ctx));
        assert_eq!(error_actual, None);
        let cancelled_actual = app.read(|ctx| {
            child_conversation_card_data_for_result(
                &StartAgentResult::Cancelled {
                    version: StartAgentVersion::V1,
                },
                ctx,
            )
        });
        assert_eq!(cancelled_actual, None);
    });
}

#[test]
fn child_conversation_card_data_returns_none_for_unknown_agent_id() {
    App::test((), |app| async move {
        app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
        let result = StartAgentResult::Success {
            agent_id: "missing-agent-id".to_string(),
            version: StartAgentVersion::V1,
        };
        let actual = app.read(|ctx| child_conversation_card_data_for_result(&result, ctx));
        assert_eq!(actual, None);
    });
}

#[test]
fn agent_display_name_from_id_returns_child_agent_name() {
    App::test((), |mut app| async move {
        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
        history_model.update(&mut app, |history_model, ctx| {
            let conversation_id =
                history_model.start_new_conversation(EntityId::new(), false, false, ctx);
            history_model.set_server_conversation_token_for_conversation(
                conversation_id,
                "child-agent-id".to_string(),
            );
            history_model
                .conversation_mut(&conversation_id)
                .expect("conversation should exist")
                .set_agent_name("Agent 1".to_string());
        });

        let actual = app.read(|ctx| {
            agent_display_name_from_id("child-agent-id", Some("orchestrator-agent-id"), ctx)
        });
        assert_eq!(actual, "Agent 1");
    });
}

#[test]
fn agent_display_name_from_id_returns_orchestrator_label() {
    App::test((), |mut app| async move {
        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
        history_model.update(&mut app, |history_model, ctx| {
            let conversation_id =
                history_model.start_new_conversation(EntityId::new(), false, false, ctx);
            let conversation = history_model
                .conversation_mut(&conversation_id)
                .expect("conversation should exist");
            conversation.set_server_conversation_token("orchestrator-agent-id".to_string());
            conversation.set_agent_name("Agent 0".to_string());
        });

        let actual = app.read(|ctx| {
            agent_display_name_from_id("orchestrator-agent-id", Some("orchestrator-agent-id"), ctx)
        });
        assert_eq!(actual, "Orchestrator agent");
    });
}

#[test]
fn agent_display_name_from_id_returns_unknown_fallback() {
    App::test((), |app| async move {
        app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
        let actual =
            app.read(|ctx| agent_display_name_from_id("missing-agent-id", Some("other-id"), ctx));
        assert_eq!(actual, "Unknown agent");
    });
}

#[test]
fn compute_validation_errors_passes_for_valid_local_config() {
    // Local mode: no validation errors regardless of harness or env_id.
    assert!(compute_validation_errors("oz", false, "").is_empty());
    assert!(compute_validation_errors("opencode", false, "").is_empty());
    assert!(compute_validation_errors("claude", false, "some-env").is_empty());
}

#[test]
fn compute_validation_errors_passes_for_valid_remote_config() {
    // Remote with non-OpenCode harness and a non-empty env_id is valid.
    assert!(compute_validation_errors("oz", true, "env-123").is_empty());
    assert!(compute_validation_errors("claude", true, "env-123").is_empty());
    assert!(compute_validation_errors("gemini", true, "env-123").is_empty());
}

#[test]
fn compute_validation_errors_flags_remote_without_environment() {
    // PRODUCT.md §configuration-block: "If execution mode is Remote and the
    // Environment dropdown is unset after the rules above, Launch is
    // disabled with an inline error: 'Choose an environment before launching.'"
    let errors = compute_validation_errors("oz", true, "");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Choose an environment"));
}

#[test]
fn compute_validation_errors_flags_opencode_with_remote() {
    // PRODUCT.md §editing-across-mode-changes / TECH.md §6: "OpenCode is
    // not supported in remote mode." Disables Launch.
    let errors = compute_validation_errors("opencode", true, "env-123");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("OpenCode"));
    assert!(errors[0].contains("remote"));
}

#[test]
fn compute_validation_errors_reports_both_when_remote_opencode_and_no_env() {
    // Both errors fire simultaneously when the user lands on Remote +
    // OpenCode + no env. Each is rendered as its own inline row.
    let errors = compute_validation_errors("opencode", true, "");
    assert_eq!(errors.len(), 2);
}

#[test]
fn display_label_for_option_returns_label_for_known_value() {
    // The dropdown header shows the human-readable label rather than the
    // canonical value.
    assert_eq!(display_label_for_option("auto", MODEL_OPTIONS), "auto");
    assert_eq!(
        display_label_for_option("oz", HARNESS_OPTIONS),
        "Oz (Warp Agent)"
    );
    assert_eq!(
        display_label_for_option("opencode", HARNESS_OPTIONS),
        "OpenCode (local-only)"
    );
}

#[test]
fn display_label_for_option_falls_back_to_value_for_unknown_input() {
    // If the LLM proposes a model_id not in the static list, the dropdown
    // header still surfaces it (so the user can see what was requested) and
    // they can pick a known option to override.
    assert_eq!(
        display_label_for_option("some-future-model", MODEL_OPTIONS),
        "some-future-model"
    );
    assert_eq!(
        display_label_for_option("unknown-harness", HARNESS_OPTIONS),
        "unknown-harness"
    );
}

#[test]
fn conversation_navigation_card_row_renders_title_without_legacy_subtitle() {
    App::test((), |app| async move {
        app.add_singleton_model(|_| Appearance::mock());
        let element = app.read(|ctx| {
            render_conversation_navigation_card_row(
                "Child conversation",
                None,
                None,
                AIConversationId::new(),
                MouseStateHandle::default(),
                false,
                ctx,
            )
        });
        let text_content = element.debug_text_content().unwrap_or_default();
        assert!(
            text_content.contains("Child conversation"),
            "Expected child conversation title in rendered text: {text_content}",
        );
        assert!(
            !text_content.contains("Open in agent mode"),
            "Legacy subtitle should not appear in rendered card text: {text_content}",
        );
    });
}
