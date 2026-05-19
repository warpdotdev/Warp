use std::collections::HashMap;

use ai::agent::action::{RunAgentsAgentRunConfig, RunAgentsExecutionMode, RunAgentsRequest};
use settings::Setting;
use warp_core::execution_mode::ExecutionMode;
use warpui::{App, EntityId, ModelHandle};

use super::*;
use crate::ai::active_agent_views_model::ActiveAgentViewsModel;
use crate::ai::agent::task::TaskId;
use crate::ai::blocklist::{BlocklistAIHistoryModel, BlocklistAIPermissions};
use crate::ai::cloud_agent_settings::CloudAgentSettings;
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::mcp::templatable_manager::TemplatableMCPServerManager;
use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;
use crate::{
    auth::AuthStateProvider,
    cloud_object::model::persistence::CloudModel,
    network::NetworkStatus,
    server::{cloud_objects::update_manager::UpdateManager, sync_queue::SyncQueue},
    settings::PrivacySettings,
    test_util::settings::initialize_settings_for_tests_with_mode,
    workspaces::{team_tester::TeamTesterStatus, user_workspaces::UserWorkspaces},
    AgentNotificationsModel, GlobalResourceHandles, GlobalResourceHandlesProvider, LaunchMode,
};

struct RunAgentsTestState {
    conversation_id: AIConversationId,
    executor: ModelHandle<RunAgentsExecutor>,
}

fn initialize_run_agents_test(app: &mut App, mode: ExecutionMode) -> RunAgentsTestState {
    initialize_settings_for_tests_with_mode(app, mode, false);
    let global_resource_handles = GlobalResourceHandles::mock(app);
    app.add_singleton_model(|_| GlobalResourceHandlesProvider::new(global_resource_handles));
    let history = app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], &[]));
    app.add_singleton_model(|_| CLIAgentSessionsModel::new());
    app.add_singleton_model(|_| ActiveAgentViewsModel::new());
    app.add_singleton_model(AgentNotificationsModel::new);
    app.add_singleton_model(BlocklistAIPermissions::new);
    let terminal_view_id = EntityId::new();
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(CloudModel::mock);
    app.add_singleton_model(|_| TemplatableMCPServerManager::default());
    app.add_singleton_model(|ctx| {
        AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
    });
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(UserWorkspaces::default_mock);
    let conversation_id = history.update(app, |history_model, ctx| {
        history_model.start_new_conversation(terminal_view_id, false, false, false, ctx)
    });
    let start_agent_executor = app.add_model(StartAgentExecutor::new);
    let executor =
        app.add_model(|_| RunAgentsExecutor::new(start_agent_executor, terminal_view_id));

    RunAgentsTestState {
        conversation_id,
        executor,
    }
}

fn remote_run_agents_action(harness_type: &str) -> AIAgentAction {
    AIAgentAction {
        id: AIAgentActionId::from("run-agents-action".to_string()),
        task_id: TaskId::new("run-agents-task".to_string()),
        requires_result: true,
        action: AIAgentActionType::RunAgents(RunAgentsRequest {
            summary: "Run child agent".to_string(),
            base_prompt: "Help".to_string(),
            skills: vec![],
            model_id: String::new(),
            harness_type: harness_type.to_string(),
            execution_mode: RunAgentsExecutionMode::Remote {
                environment_id: "env-1".to_string(),
                worker_host: "warp".to_string(),
                computer_use_enabled: false,
            },
            agent_run_configs: vec![RunAgentsAgentRunConfig {
                name: "child".to_string(),
                prompt: "Help".to_string(),
                title: String::new(),
            }],
            plan_id: String::new(),
            harness_auth_secret_name: None,
        }),
    }
}

fn persist_default_auth_secret(app: &mut App, harness_config_name: &str, secret_name: &str) {
    CloudAgentSettings::handle(app).update(app, |settings, ctx| {
        let mut secrets = settings.last_selected_auth_secret.value().clone();
        secrets.insert(harness_config_name.to_string(), secret_name.to_string());
        settings
            .last_selected_auth_secret
            .set_value(secrets, ctx)
            .unwrap();
        settings
            .inherit_auth_secret_harnesses
            .set_value(HashMap::new(), ctx)
            .unwrap();
    });
}

#[test]
fn should_not_autoexecute_remote_non_warp_harness_without_default_auth_secret() {
    App::test((), |mut app| async move {
        let state = initialize_run_agents_test(&mut app, ExecutionMode::Sdk);
        let action = remote_run_agents_action("codex");

        let should_autoexecute = state.executor.update(&mut app, |executor, ctx| {
            executor.should_autoexecute(
                ExecuteActionInput {
                    action: &action,
                    conversation_id: state.conversation_id,
                },
                ctx,
            )
        });

        assert!(!should_autoexecute);
    });
}

#[test]
fn should_autoexecute_remote_non_warp_harness_with_default_auth_secret() {
    App::test((), |mut app| async move {
        let state = initialize_run_agents_test(&mut app, ExecutionMode::Sdk);
        persist_default_auth_secret(&mut app, "codex", "default-openai-key");
        let action = remote_run_agents_action("codex");

        let should_autoexecute = state.executor.update(&mut app, |executor, ctx| {
            executor.should_autoexecute(
                ExecuteActionInput {
                    action: &action,
                    conversation_id: state.conversation_id,
                },
                ctx,
            )
        });

        assert!(should_autoexecute);
    });
}

#[test]
fn should_autoexecute_remote_warp_harness_without_default_auth_secret() {
    App::test((), |mut app| async move {
        let state = initialize_run_agents_test(&mut app, ExecutionMode::Sdk);
        let action = remote_run_agents_action("oz");

        let should_autoexecute = state.executor.update(&mut app, |executor, ctx| {
            executor.should_autoexecute(
                ExecuteActionInput {
                    action: &action,
                    conversation_id: state.conversation_id,
                },
                ctx,
            )
        });

        assert!(should_autoexecute);
    });
}

#[test]
fn populate_default_auth_secret_for_autoexecute_uses_persisted_secret() {
    App::test((), |mut app| async move {
        let state = initialize_run_agents_test(&mut app, ExecutionMode::Sdk);
        persist_default_auth_secret(&mut app, "claude", "default-anthropic-key");
        let AIAgentActionType::RunAgents(mut request) = remote_run_agents_action("claude").action
        else {
            panic!("expected run_agents action");
        };

        state.executor.update(&mut app, |_, ctx| {
            populate_default_auth_secret_for_autoexecute(&mut request, ctx);
        });

        assert_eq!(
            request.harness_auth_secret_name.as_deref(),
            Some("default-anthropic-key")
        );
    });
}
