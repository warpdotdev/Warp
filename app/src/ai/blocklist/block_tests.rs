use super::{received_message_collapsible_id, CollapsibleElementState, CollapsibleExpansionState};
use crate::ai::agent::StartAgentExecutionMode;
use crate::ai::blocklist::action_model::{
    compose_run_agents_child_prompt, run_agents_to_start_agent_mode,
};
use crate::settings::AISettings;
use crate::test_util::settings::initialize_settings_for_tests;
use ai::agent::action::{RunAgentsAgentRunConfig, RunAgentsExecutionMode};
use ai::skills::SkillReference;
use settings::Setting;
use std::path::PathBuf;
use warpui::{App, SingletonEntity};

#[test]
fn reasoning_auto_collapses_when_user_has_not_manually_toggled() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let mut state = CollapsibleElementState::default();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Collapsed
        ));
    });
}

#[test]
fn always_show_thinking_stays_expanded_after_finish() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        AISettings::handle(&app).update(&mut app, |settings, ctx| {
            settings
                .thinking_display_mode
                .set_value(crate::settings::ThinkingDisplayMode::AlwaysShow, ctx)
                .unwrap();
        });

        let mut state = CollapsibleElementState::default();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Expanded {
                is_finished: true,
                scroll_pinned_to_bottom: false
            }
        ));
    });
}

#[test]
fn manual_collapse_while_streaming_stays_collapsed_after_finish() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let mut state = CollapsibleElementState::default();

        state.toggle_expansion();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Collapsed
        ));
    });
}

#[test]
fn manual_reexpand_while_streaming_stays_expanded_after_finish() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let mut state = CollapsibleElementState::default();

        state.toggle_expansion();
        state.toggle_expansion();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Expanded {
                is_finished: true,
                scroll_pinned_to_bottom: false
            }
        ));
    });
}

#[test]
fn received_message_collapsible_id_prefixes_row_ids() {
    let first = received_message_collapsible_id("message-1");
    let second = received_message_collapsible_id("message-2");

    assert_eq!(&*first, "received-message:message-1");
    assert_eq!(&*second, "received-message:message-2");
    assert_ne!(first, second);
}

#[test]
fn compose_child_prompt_concatenates_when_both_non_empty() {
    let composed = compose_run_agents_child_prompt("base", "do X");
    assert_eq!(composed, "base\n\ndo X");
}

#[test]
fn compose_child_prompt_uses_base_only_when_per_agent_empty() {
    let composed = compose_run_agents_child_prompt("base", "");
    assert_eq!(composed, "base");
}

#[test]
fn compose_child_prompt_uses_per_agent_only_when_base_empty() {
    let composed = compose_run_agents_child_prompt("", "do X");
    assert_eq!(composed, "do X");
}

#[test]
fn compose_child_prompt_returns_empty_when_both_empty() {
    let composed = compose_run_agents_child_prompt("", "");
    assert_eq!(composed, "");
}

#[test]
fn compose_child_prompt_treats_whitespace_only_base_as_empty() {
    let composed = compose_run_agents_child_prompt("   \n", "do X");
    assert_eq!(composed, "do X");
}

fn agent_cfg() -> RunAgentsAgentRunConfig {
    RunAgentsAgentRunConfig {
        name: "child".to_string(),
        prompt: "do X".to_string(),
        title: "Child".to_string(),
    }
}

#[test]
fn remote_arm_propagates_skills_into_skill_references() {
    let skills = vec![
        SkillReference::BundledSkillId("writing-pr-descriptions".to_string()),
        SkillReference::Path(PathBuf::from("/tmp/skill/SKILL.md")),
    ];
    let mode = run_agents_to_start_agent_mode(
        &RunAgentsExecutionMode::Remote {
            environment_id: "env-1".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: true,
        },
        "oz",
        "auto",
        &skills,
        &agent_cfg(),
    )
    .expect("Remote+oz must convert");
    let StartAgentExecutionMode::Remote {
        skill_references,
        environment_id,
        worker_host,
        harness_type,
        model_id,
        computer_use_enabled,
        title,
    } = mode
    else {
        panic!("expected Remote start-agent mode");
    };
    assert_eq!(skill_references, skills);
    assert_eq!(environment_id, "env-1");
    assert_eq!(worker_host, "warp");
    assert_eq!(harness_type, "oz");
    assert_eq!(model_id, "auto");
    assert!(computer_use_enabled);
    assert_eq!(title, "Child");
}

#[test]
fn remote_arm_with_empty_skills_propagates_empty_vec() {
    let mode = run_agents_to_start_agent_mode(
        &RunAgentsExecutionMode::Remote {
            environment_id: "env-1".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: false,
        },
        "claude",
        "auto",
        &[],
        &agent_cfg(),
    )
    .expect("Remote+claude must convert");
    let StartAgentExecutionMode::Remote {
        skill_references, ..
    } = mode
    else {
        panic!("expected Remote start-agent mode");
    };
    assert!(skill_references.is_empty());
}

#[test]
fn remote_arm_rejects_opencode() {
    let err = run_agents_to_start_agent_mode(
        &RunAgentsExecutionMode::Remote {
            environment_id: "env-1".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: false,
        },
        "opencode",
        "auto",
        &[],
        &agent_cfg(),
    )
    .expect_err("Remote+opencode must be rejected");
    assert!(err.to_lowercase().contains("opencode"));
}

#[test]
fn should_show_agent_mode_ask_user_question_speedbump_defaults_to_true() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        AISettings::handle(&app).read(&app, |settings, _ctx| {
            assert!(*settings.should_show_agent_mode_ask_user_question_speedbump);
        });
    });
}

#[test]
fn should_show_agent_mode_ask_user_question_speedbump_round_trips_to_false() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        AISettings::handle(&app).update(&mut app, |settings, ctx| {
            settings
                .should_show_agent_mode_ask_user_question_speedbump
                .set_value(false, ctx)
                .unwrap();
        });
        AISettings::handle(&app).read(&app, |settings, _ctx| {
            assert!(!*settings.should_show_agent_mode_ask_user_question_speedbump);
        });
    });
}
