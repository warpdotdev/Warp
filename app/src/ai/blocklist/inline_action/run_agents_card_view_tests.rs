use ai::agent::action::{RunAgentsAgentRunConfig, RunAgentsExecutionMode, RunAgentsRequest};
use ai::agent::action_result::{
    RunAgentsAgentOutcome, RunAgentsAgentOutcomeKind, RunAgentsLaunchedExecutionMode,
    RunAgentsResult,
};
use ai::skills::SkillReference;
use std::path::PathBuf;

use super::RunAgentsEditState;

fn make_request(harness: &str, mode: RunAgentsExecutionMode) -> RunAgentsRequest {
    make_request_with_skills(harness, mode, Vec::new())
}

fn make_request_with_skills(
    harness: &str,
    mode: RunAgentsExecutionMode,
    skills: Vec<SkillReference>,
) -> RunAgentsRequest {
    RunAgentsRequest {
        summary: "summary".to_string(),
        base_prompt: "base".to_string(),
        skills,
        model_id: "auto".to_string(),
        harness_type: harness.to_string(),
        execution_mode: mode,
        agent_run_configs: vec![RunAgentsAgentRunConfig {
            name: "child".to_string(),
            prompt: "do work".to_string(),
            title: "Child agent".to_string(),
        }],
        plan_id: String::new(),
    }
}

#[test]
fn local_to_cloud_initializes_remote_with_empty_environment() {
    let mut state =
        RunAgentsEditState::from_request(&make_request("oz", RunAgentsExecutionMode::Local));
    assert!(matches!(
        state.orch.execution_mode,
        RunAgentsExecutionMode::Local
    ));

    state.orch.toggle_execution_mode_to_remote(true);
    let RunAgentsExecutionMode::Remote {
        environment_id,
        worker_host,
        computer_use_enabled,
    } = state.orch.execution_mode
    else {
        panic!("expected Remote after toggle");
    };
    assert_eq!(environment_id, "");
    assert_eq!(worker_host, "warp");
    assert!(!computer_use_enabled);
}

#[test]
fn cloud_to_local_drops_environment() {
    let mut state = RunAgentsEditState::from_request(&make_request(
        "oz",
        RunAgentsExecutionMode::Remote {
            environment_id: "env-1".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: false,
        },
    ));
    state.orch.toggle_execution_mode_to_remote(false);
    assert!(matches!(
        state.orch.execution_mode,
        RunAgentsExecutionMode::Local
    ));
}

#[test]
fn local_to_cloud_resets_opencode_to_oz() {
    let mut state =
        RunAgentsEditState::from_request(&make_request("opencode", RunAgentsExecutionMode::Local));
    state.orch.toggle_execution_mode_to_remote(true);
    assert_eq!(state.orch.harness_type, "oz");
}

#[test]
fn cloud_without_env_no_longer_disables_accept() {
    let state = RunAgentsEditState::from_request(&make_request(
        "oz",
        RunAgentsExecutionMode::Remote {
            environment_id: String::new(),
            worker_host: "warp".to_string(),
            computer_use_enabled: false,
        },
    ));
    assert!(
        state.orch.accept_disabled_reason().is_none(),
        "Cloud without env should NOT disable Accept (soft recommendation only)"
    );
}

#[test]
fn cloud_with_opencode_disables_accept() {
    // Bypass the toggle helper to test the validation gate directly.
    let state = RunAgentsEditState::from_request(&make_request(
        "opencode",
        RunAgentsExecutionMode::Remote {
            environment_id: "env-1".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: false,
        },
    ));
    let reason = state.orch.accept_disabled_reason();
    assert!(reason.is_some(), "Cloud + OpenCode should disable Accept");
    assert!(reason.unwrap().contains("OpenCode"));
}

#[test]
fn local_with_any_harness_does_not_disable_accept() {
    for harness in ["oz", "claude", "gemini", "opencode"] {
        let state =
            RunAgentsEditState::from_request(&make_request(harness, RunAgentsExecutionMode::Local));
        assert!(
            state.orch.accept_disabled_reason().is_none(),
            "Local + {harness} should allow Accept"
        );
    }
}

#[test]
fn cloud_with_env_and_non_opencode_harness_allows_accept() {
    for harness in ["oz", "claude", "gemini"] {
        let state = RunAgentsEditState::from_request(&make_request(
            harness,
            RunAgentsExecutionMode::Remote {
                environment_id: "env-1".to_string(),
                worker_host: "warp".to_string(),
                computer_use_enabled: false,
            },
        ));
        assert!(
            state.orch.accept_disabled_reason().is_none(),
            "Cloud + env + {harness} should allow Accept"
        );
    }
}

#[test]
fn set_environment_id_no_op_in_local_mode() {
    let mut state =
        RunAgentsEditState::from_request(&make_request("oz", RunAgentsExecutionMode::Local));
    state.orch.set_environment_id("env-1".to_string());
    assert!(matches!(
        state.orch.execution_mode,
        RunAgentsExecutionMode::Local
    ));
}

#[test]
fn set_environment_id_updates_remote() {
    let mut state = RunAgentsEditState::from_request(&make_request(
        "oz",
        RunAgentsExecutionMode::Remote {
            environment_id: "old".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: false,
        },
    ));
    state.orch.set_environment_id("new-env".to_string());
    let RunAgentsExecutionMode::Remote { environment_id, .. } = state.orch.execution_mode else {
        panic!("expected Remote");
    };
    assert_eq!(environment_id, "new-env");
}

#[test]
fn to_request_round_trips_request_fields() {
    let req = make_request_with_skills(
        "claude",
        RunAgentsExecutionMode::Remote {
            environment_id: "env-2".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: true,
        },
        vec![
            SkillReference::BundledSkillId("writing-pr-descriptions".to_string()),
            SkillReference::Path(PathBuf::from("/tmp/skill/SKILL.md")),
        ],
    );
    let state = RunAgentsEditState::from_request(&req);
    let round_tripped = state.to_request();
    assert_eq!(round_tripped.summary, req.summary);
    assert_eq!(round_tripped.base_prompt, req.base_prompt);
    assert_eq!(round_tripped.model_id, req.model_id);
    assert_eq!(round_tripped.harness_type, req.harness_type);
    assert_eq!(round_tripped.execution_mode, req.execution_mode);
    assert_eq!(round_tripped.agent_run_configs, req.agent_run_configs);
    assert_eq!(round_tripped.skills, req.skills);
}

mod format_terminal_state_tests {
    use super::super::{format_terminal_state, StatusKind};
    use super::*;

    fn launched(name: &str, agent_id: &str) -> RunAgentsAgentOutcome {
        RunAgentsAgentOutcome {
            name: name.to_string(),
            kind: RunAgentsAgentOutcomeKind::Launched {
                agent_id: agent_id.to_string(),
            },
        }
    }

    fn failed(name: &str, error: &str) -> RunAgentsAgentOutcome {
        RunAgentsAgentOutcome {
            name: name.to_string(),
            kind: RunAgentsAgentOutcomeKind::Failed {
                error: error.to_string(),
            },
        }
    }

    fn launched_result(agents: Vec<RunAgentsAgentOutcome>) -> RunAgentsResult {
        RunAgentsResult::Launched {
            model_id: "auto".to_string(),
            harness_type: "oz".to_string(),
            execution_mode: RunAgentsLaunchedExecutionMode::Local,
            agents,
        }
    }

    #[test]
    fn launched_singular_uses_singular_label() {
        let result = launched_result(vec![launched("child", "a-1")]);
        let (label, kind) = format_terminal_state(&result);
        assert_eq!(label, "Spawned 1 agent");
        assert!(matches!(kind, StatusKind::Success));
    }

    #[test]
    fn launched_plural_uses_plural_label() {
        let result = launched_result(vec![
            launched("a", "a-1"),
            launched("b", "a-2"),
            launched("c", "a-3"),
        ]);
        let (label, kind) = format_terminal_state(&result);
        assert_eq!(label, "Spawned 3 agents");
        assert!(matches!(kind, StatusKind::Success));
    }

    #[test]
    fn launched_partial_uses_x_of_y_label_and_mixed_status() {
        let result = launched_result(vec![
            launched("a", "a-1"),
            failed("b", "boom"),
            launched("c", "a-3"),
        ]);
        let (label, kind) = format_terminal_state(&result);
        assert_eq!(label, "Spawned 2 of 3 agents");
        assert!(matches!(kind, StatusKind::Mixed));
    }

    #[test]
    fn failure_with_error_includes_error_text() {
        let (label, kind) = format_terminal_state(&RunAgentsResult::Failure {
            error: "server rejected request".to_string(),
        });
        assert_eq!(
            label,
            "Failed to start orchestration: server rejected request"
        );
        assert!(matches!(kind, StatusKind::Failure));
    }

    #[test]
    fn failure_with_empty_error_uses_short_label() {
        let (label, kind) = format_terminal_state(&RunAgentsResult::Failure {
            error: String::new(),
        });
        assert_eq!(label, "Failed to start orchestration");
        assert!(matches!(kind, StatusKind::Failure));
    }

    #[test]
    fn denied_with_reason_appends_reason() {
        let (label, kind) = format_terminal_state(&RunAgentsResult::Denied {
            reason: "disapproved".to_string(),
        });
        assert!(label.contains("disapproved"));
        assert!(matches!(kind, StatusKind::Cancelled));
    }

    #[test]
    fn denied_without_reason_uses_short_label() {
        let (label, kind) = format_terminal_state(&RunAgentsResult::Denied {
            reason: String::new(),
        });
        assert!(!label.contains("()"));
        assert!(matches!(kind, StatusKind::Cancelled));
    }

    #[test]
    fn cancelled_uses_cancelled_status() {
        let (label, kind) = format_terminal_state(&RunAgentsResult::Cancelled);
        assert_eq!(label, "Spawn agents cancelled");
        assert!(matches!(kind, StatusKind::Cancelled));
    }
}

mod should_auto_launch_tests {
    use super::super::{should_auto_launch, RunAgentsEditState};
    use super::*;
    use ai::agent::orchestration_config::{
        OrchestrationConfig, OrchestrationConfigStatus, OrchestrationExecutionMode,
    };

    fn matching_config() -> (OrchestrationConfig, OrchestrationConfigStatus) {
        (
            OrchestrationConfig {
                model_id: "auto".to_string(),
                harness_type: "oz".to_string(),
                execution_mode: OrchestrationExecutionMode::Local,
            },
            OrchestrationConfigStatus::Approved,
        )
    }

    fn default_state() -> RunAgentsEditState {
        RunAgentsEditState::from_request(&make_request("oz", RunAgentsExecutionMode::Local))
    }

    #[test]
    fn returns_true_when_all_conditions_met() {
        let state = default_state();
        let config = Some(matching_config());
        assert!(should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn returns_false_when_already_auto_launched() {
        let state = default_state();
        let config = Some(matching_config());
        assert!(!should_auto_launch(true, false, false, &state, &config));
    }

    #[test]
    fn returns_false_when_denied() {
        let state = default_state();
        let config = Some(matching_config());
        assert!(!should_auto_launch(false, true, false, &state, &config));
    }

    #[test]
    fn returns_false_when_spawning() {
        let state = default_state();
        let config = Some(matching_config());
        assert!(!should_auto_launch(false, false, true, &state, &config));
    }

    #[test]
    fn returns_false_when_agent_run_configs_empty() {
        let mut state = default_state();
        state.agent_run_configs.clear();
        let config = Some(matching_config());
        assert!(!should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn returns_false_when_no_active_config() {
        let state = default_state();
        assert!(!should_auto_launch(false, false, false, &state, &None));
    }

    #[test]
    fn returns_false_when_config_not_approved() {
        let state = default_state();
        let config = Some((
            OrchestrationConfig {
                model_id: "auto".to_string(),
                harness_type: "oz".to_string(),
                execution_mode: OrchestrationExecutionMode::Local,
            },
            OrchestrationConfigStatus::None,
        ));
        assert!(!should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn returns_false_when_config_disapproved() {
        let state = default_state();
        let config = Some((
            OrchestrationConfig {
                model_id: "auto".to_string(),
                harness_type: "oz".to_string(),
                execution_mode: OrchestrationExecutionMode::Local,
            },
            OrchestrationConfigStatus::Disapproved,
        ));
        assert!(!should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn returns_false_when_model_id_mismatches() {
        let state = default_state();
        let config = Some((
            OrchestrationConfig {
                model_id: "claude-4".to_string(),
                harness_type: "oz".to_string(),
                execution_mode: OrchestrationExecutionMode::Local,
            },
            OrchestrationConfigStatus::Approved,
        ));
        assert!(!should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn returns_false_when_harness_type_mismatches() {
        let state = default_state();
        let config = Some((
            OrchestrationConfig {
                model_id: "auto".to_string(),
                harness_type: "claude".to_string(),
                execution_mode: OrchestrationExecutionMode::Local,
            },
            OrchestrationConfigStatus::Approved,
        ));
        assert!(!should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn returns_false_when_execution_mode_mismatches() {
        let state = default_state();
        let config = Some((
            OrchestrationConfig {
                model_id: "auto".to_string(),
                harness_type: "oz".to_string(),
                execution_mode: OrchestrationExecutionMode::Remote {
                    environment_id: "env-1".to_string(),
                    worker_host: "warp".to_string(),
                },
            },
            OrchestrationConfigStatus::Approved,
        ));
        assert!(!should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn matches_remote_config_when_fields_agree() {
        let state = RunAgentsEditState::from_request(&make_request(
            "oz",
            RunAgentsExecutionMode::Remote {
                environment_id: "env-1".to_string(),
                worker_host: "warp".to_string(),
                computer_use_enabled: false,
            },
        ));
        let config = Some((
            OrchestrationConfig {
                model_id: "auto".to_string(),
                harness_type: "oz".to_string(),
                execution_mode: OrchestrationExecutionMode::Remote {
                    environment_id: "env-1".to_string(),
                    worker_host: "warp".to_string(),
                },
            },
            OrchestrationConfigStatus::Approved,
        ));
        assert!(should_auto_launch(false, false, false, &state, &config));
    }

    #[test]
    fn empty_model_id_inherits_from_config() {
        let mut req = make_request("oz", RunAgentsExecutionMode::Local);
        req.model_id = String::new();
        let state = RunAgentsEditState::from_request(&req);
        let config = Some(matching_config());
        assert!(
            should_auto_launch(false, false, false, &state, &config),
            "Empty model_id on request should inherit from config and match"
        );
    }
}

mod compute_is_denied_tests {
    use super::super::compute_is_denied;
    use ai::agent::orchestration_config::{
        OrchestrationConfig, OrchestrationConfigStatus, OrchestrationExecutionMode,
    };

    fn some_config(
        status: OrchestrationConfigStatus,
    ) -> Option<(OrchestrationConfig, OrchestrationConfigStatus)> {
        Some((
            OrchestrationConfig {
                model_id: "auto".to_string(),
                harness_type: "oz".to_string(),
                execution_mode: OrchestrationExecutionMode::Local,
            },
            status,
        ))
    }

    #[test]
    fn false_when_no_denied_result_and_no_config() {
        assert!(!compute_is_denied(false, &None));
    }

    #[test]
    fn true_when_has_denied_result_from_history() {
        assert!(compute_is_denied(true, &None));
    }

    #[test]
    fn true_when_config_is_disapproved() {
        let config = some_config(OrchestrationConfigStatus::Disapproved);
        assert!(compute_is_denied(false, &config));
    }

    #[test]
    fn true_when_both_denied_and_disapproved() {
        let config = some_config(OrchestrationConfigStatus::Disapproved);
        assert!(compute_is_denied(true, &config));
    }

    #[test]
    fn false_when_config_is_approved() {
        let config = some_config(OrchestrationConfigStatus::Approved);
        assert!(!compute_is_denied(false, &config));
    }

    #[test]
    fn false_when_config_status_is_none() {
        let config = some_config(OrchestrationConfigStatus::None);
        assert!(!compute_is_denied(false, &config));
    }

    #[test]
    fn denied_result_overrides_approved_config() {
        let config = some_config(OrchestrationConfigStatus::Approved);
        assert!(
            compute_is_denied(true, &config),
            "History denied result should take precedence over approved config"
        );
    }
}

#[test]
fn local_to_cloud_idempotent_when_already_remote() {
    let mut state = RunAgentsEditState::from_request(&make_request(
        "oz",
        RunAgentsExecutionMode::Remote {
            environment_id: "env-1".to_string(),
            worker_host: "warp".to_string(),
            computer_use_enabled: true,
        },
    ));
    state.orch.toggle_execution_mode_to_remote(true);
    let RunAgentsExecutionMode::Remote {
        environment_id,
        computer_use_enabled,
        ..
    } = state.orch.execution_mode
    else {
        panic!("expected Remote");
    };
    assert_eq!(
        environment_id, "env-1",
        "toggle to Remote when already Remote should not clobber env"
    );
    assert!(
        computer_use_enabled,
        "toggle to Remote when already Remote should not clobber computer_use"
    );
}
