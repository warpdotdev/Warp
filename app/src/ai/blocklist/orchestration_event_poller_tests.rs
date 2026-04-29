use super::*;
use crate::ai::agent::conversation::{AIConversation, AIConversationId, ConversationStatus};
use crate::ai::agent_events::{
    agent_event_backoff, agent_event_failures_exceeded_threshold,
    DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS,
};
use crate::ai::ambient_agents::{AmbientAgentTask, AmbientAgentTaskState};
use crate::persistence::{model::AgentConversationData, ModelEvent};
use crate::server::server_api::ai::MockAIClient;
use crate::server::server_api::{AIApiError, ServerApiProvider};
use crate::test_util::settings::initialize_settings_for_tests;
use crate::{GlobalResourceHandles, GlobalResourceHandlesProvider};
use chrono::Utc;
use http::StatusCode;
use std::collections::HashSet;
use std::sync::Arc;
use warp_multi_agent_api as api;
use warpui::{App, EntityId};

#[test]
fn sse_backoff_escalates_then_caps() {
    assert_eq!(
        agent_event_backoff(1, DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS),
        Duration::from_secs(1)
    );
    assert_eq!(
        agent_event_backoff(2, DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS),
        Duration::from_secs(2)
    );
    assert_eq!(
        agent_event_backoff(3, DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS),
        Duration::from_secs(5)
    );
    assert_eq!(
        agent_event_backoff(4, DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS),
        Duration::from_secs(10)
    );
    // Caps at 10s for any higher failure count.
    assert_eq!(
        agent_event_backoff(5, DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS),
        Duration::from_secs(10)
    );
    assert_eq!(
        agent_event_backoff(100, DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS),
        Duration::from_secs(10)
    );
}

#[test]
fn sse_backoff_zero_failures_uses_first_step() {
    // Defensive: 0 failures should still return a valid backoff.
    assert_eq!(
        agent_event_backoff(0, DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS),
        Duration::from_secs(1)
    );
}

#[test]
fn threshold_not_exceeded_below_limit() {
    assert!(!agent_event_failures_exceeded_threshold(0, 5));
    assert!(!agent_event_failures_exceeded_threshold(1, 5));
    assert!(!agent_event_failures_exceeded_threshold(4, 5));
}

#[test]
fn threshold_exceeded_at_and_above_limit() {
    assert!(agent_event_failures_exceeded_threshold(5, 5));
    assert!(agent_event_failures_exceeded_threshold(6, 5));
    assert!(agent_event_failures_exceeded_threshold(100, 5));
}

fn make_run_event(event_type: &str, run_id: &str, ref_id: Option<&str>) -> AgentRunEvent {
    AgentRunEvent {
        event_type: event_type.to_string(),
        run_id: run_id.to_string(),
        ref_id: ref_id.map(|s| s.to_string()),
        execution_id: None,
        occurred_at: "2026-01-01T00:00:00Z".to_string(),
        sequence: 1,
    }
}

#[test]
fn convert_lifecycle_events_includes_run_blocked() {
    let events = vec![make_run_event("run_blocked", "child-run", None)];
    let result = convert_lifecycle_events(&events, "self-run");
    assert_eq!(result.len(), 1);
    let event = &result[0];
    let Some(api::agent_event::Event::LifecycleEvent(lifecycle)) = &event.event else {
        panic!("expected lifecycle event");
    };
    let Some(api::agent_event::lifecycle_event::Detail::Blocked(blocked)) = &lifecycle.detail
    else {
        panic!("expected blocked detail");
    };
    assert!(blocked.blocked_action.is_empty());
}

#[test]
fn convert_lifecycle_events_filters_self_run_blocked() {
    let events = vec![make_run_event("run_blocked", "self-run", None)];
    let result = convert_lifecycle_events(&events, "self-run");
    assert!(result.is_empty());
}

#[test]
fn convert_lifecycle_events_maps_run_restarted() {
    let events = vec![make_run_event("run_restarted", "child-run", None)];
    let result = convert_lifecycle_events(&events, "self-run");
    assert_eq!(result.len(), 1);
    let event = &result[0];
    let Some(api::agent_event::Event::LifecycleEvent(lifecycle)) = &event.event else {
        panic!("expected lifecycle event");
    };
    assert!(matches!(
        lifecycle.detail,
        Some(api::agent_event::lifecycle_event::Detail::InProgress(..))
    ));
}

fn restored_conversation(
    conversation_id: AIConversationId,
    run_id: String,
    last_event_sequence: Option<i64>,
) -> AIConversation {
    AIConversation::new_restored(
        conversation_id,
        vec![api::Task {
            id: "root-task".to_string(),
            messages: vec![],
            dependencies: None,
            description: String::new(),
            summary: String::new(),
            server_data: String::new(),
        }],
        Some(AgentConversationData {
            server_conversation_token: None,
            conversation_usage_metadata: None,
            reverted_action_ids: None,
            forked_from_server_conversation_token: None,
            artifacts_json: None,
            parent_agent_id: None,
            agent_name: None,
            parent_conversation_id: None,
            is_remote_child: false,
            run_id: Some(run_id),
            autoexecute_override: None,
            last_event_sequence,
        }),
    )
    .expect("restored conversation should build")
}

fn ambient_agent_task(
    task_id: crate::ai::ambient_agents::AmbientAgentTaskId,
    last_event_sequence: Option<i64>,
    children: Vec<String>,
) -> AmbientAgentTask {
    AmbientAgentTask {
        task_id,
        parent_run_id: None,
        title: "Task".to_string(),
        state: AmbientAgentTaskState::Succeeded,
        prompt: String::new(),
        created_at: Utc::now(),
        started_at: None,
        updated_at: Utc::now(),
        status_message: None,
        source: None,
        session_id: None,
        session_link: None,
        creator: None,
        conversation_id: None,
        request_usage: None,
        is_sandbox_running: false,
        agent_config_snapshot: None,
        artifacts: vec![],
        last_event_sequence,
        children,
    }
}

#[test]
fn finish_restore_fetch_merges_server_cursor_and_child_runs() {
    App::test((), |mut app| async move {
        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], &[]));
        let terminal_view_id = EntityId::new();
        let conversation_id = AIConversationId::new();
        let run_id = uuid::Uuid::new_v4().to_string();

        history_model.update(&mut app, |history_model, ctx| {
            history_model.restore_conversations(
                terminal_view_id,
                vec![restored_conversation(conversation_id, run_id.clone(), None)],
                ctx,
            );
            history_model.update_conversation_status(
                terminal_view_id,
                conversation_id,
                ConversationStatus::InProgress,
                ctx,
            );
        });

        let mut ai_client = MockAIClient::new();
        ai_client
            .expect_poll_agent_events()
            .returning(|_, _, _| Ok(vec![]));

        let server_api = ServerApiProvider::new_for_test().get();
        let poller = app.add_singleton_model(move |_| {
            OrchestrationEventPoller::new_with_clients_for_test(Arc::new(ai_client), server_api)
        });

        let child_run_id = uuid::Uuid::new_v4().to_string();
        let task_id = run_id.parse().expect("run_id should parse");
        let task = ambient_agent_task(task_id, Some(27), vec![child_run_id.clone()]);
        poller.update(&mut app, |poller, ctx| {
            poller
                .watched_run_ids
                .insert(conversation_id, HashSet::from([run_id.clone()]));
            poller.restore_fetch_failures.insert(conversation_id, 1);
            poller.finish_restore_fetch(conversation_id, 0, task, ctx);
        });
        poller.read(&app, |poller, _| {
            assert_eq!(poller.event_cursor.get(&conversation_id), Some(&27));
            assert!(!poller.restore_fetch_failures.contains_key(&conversation_id));

            let watched = poller
                .watched_run_ids
                .get(&conversation_id)
                .expect("watched runs should exist");
            assert!(watched.contains(&run_id));
            assert!(watched.contains(&child_run_id));
        });
    });
}

#[test]
fn non_retryable_restore_fetch_failure_falls_back_to_persisted_delivery_state() {
    App::test((), |mut app| async move {
        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], &[]));
        let terminal_view_id = EntityId::new();
        let conversation_id = AIConversationId::new();
        let run_id = uuid::Uuid::new_v4().to_string();

        history_model.update(&mut app, |history_model, ctx| {
            history_model.restore_conversations(
                terminal_view_id,
                vec![restored_conversation(
                    conversation_id,
                    run_id.clone(),
                    Some(17),
                )],
                ctx,
            );
            history_model.update_conversation_status(
                terminal_view_id,
                conversation_id,
                ConversationStatus::Success,
                ctx,
            );
        });

        let mut ai_client = MockAIClient::new();
        ai_client
            .expect_poll_agent_events()
            .returning(|_, _, _| Ok(vec![]));

        let server_api = ServerApiProvider::new_for_test().get();
        let poller = app.add_singleton_model(move |_| {
            OrchestrationEventPoller::new_with_clients_for_test(Arc::new(ai_client), server_api)
        });

        poller.update(&mut app, |poller, ctx| {
            poller
                .watched_run_ids
                .insert(conversation_id, HashSet::from([run_id.clone()]));
            poller.restore_fetch_failures.insert(conversation_id, 1);

            poller.handle_restore_fetch_error(
                conversation_id,
                run_id.clone(),
                anyhow::Error::new(AIApiError::ErrorStatus(
                    StatusCode::NOT_FOUND,
                    "missing task".to_string(),
                )),
                ctx,
            );

            assert!(!poller.restore_fetch_failures.contains_key(&conversation_id));
            assert!(
                poller.poll_in_flight.contains(&conversation_id)
                    || poller.sse_connections.contains_key(&conversation_id)
            );
        });
    });
}

#[test]
fn retryable_restore_fetch_failure_keeps_retry_state() {
    App::test((), |mut app| async move {
        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], &[]));
        let terminal_view_id = EntityId::new();
        let conversation_id = AIConversationId::new();
        let run_id = uuid::Uuid::new_v4().to_string();

        history_model.update(&mut app, |history_model, ctx| {
            history_model.restore_conversations(
                terminal_view_id,
                vec![restored_conversation(
                    conversation_id,
                    run_id.clone(),
                    Some(17),
                )],
                ctx,
            );
            history_model.update_conversation_status(
                terminal_view_id,
                conversation_id,
                ConversationStatus::Success,
                ctx,
            );
        });

        let server_api = ServerApiProvider::new_for_test().get();
        let poller = app.add_singleton_model(move |_| {
            OrchestrationEventPoller::new_with_clients_for_test(
                Arc::new(MockAIClient::new()),
                server_api,
            )
        });

        poller.update(&mut app, |poller, ctx| {
            poller
                .watched_run_ids
                .insert(conversation_id, HashSet::from([run_id.clone()]));
            poller.restore_fetch_failures.insert(conversation_id, 1);

            poller.handle_restore_fetch_error(
                conversation_id,
                run_id.clone(),
                anyhow::Error::new(AIApiError::ErrorStatus(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "server error".to_string(),
                )),
                ctx,
            );

            assert!(poller.restore_fetch_failures.contains_key(&conversation_id));
            assert!(!poller.poll_in_flight.contains(&conversation_id));
            assert!(!poller.sse_connections.contains_key(&conversation_id));
        });
    });
}

#[test]
fn build_pending_events_preserves_message_sequence_and_timestamp() {
    let pending = build_pending_events(
        &[AgentRunEvent {
            event_type: "new_message".to_string(),
            run_id: "recipient-run".to_string(),
            ref_id: Some("message-1".to_string()),
            execution_id: None,
            occurred_at: "2026-02-01T12:34:56Z".to_string(),
            sequence: 42,
        }],
        vec![crate::ai::agent::ReceivedMessageInput {
            message_id: "message-1".to_string(),
            sender_agent_id: "sender-run".to_string(),
            addresses: vec!["recipient-run".to_string()],
            subject: "subject".to_string(),
            message_body: "body".to_string(),
        }],
        vec![],
    );

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_id, "message-1");
    match &pending[0].detail {
        PendingEventDetail::Message {
            sequence,
            message_id,
            occurred_at,
            ..
        } => {
            assert_eq!(*sequence, 42);
            assert_eq!(message_id, "message-1");
            assert_eq!(occurred_at, "2026-02-01T12:34:56Z");
        }
        PendingEventDetail::Lifecycle { .. } => panic!("expected message event"),
    }
}

#[test]
fn handle_poll_result_updates_persisted_and_server_cursor() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);

        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let mut global_resource_handles = GlobalResourceHandles::mock(&mut app);
        global_resource_handles.model_event_sender = Some(sender);
        app.add_singleton_model(|_| GlobalResourceHandlesProvider::new(global_resource_handles));

        let history_model = app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], &[]));
        let terminal_view_id = EntityId::new();
        let conversation_id = AIConversationId::new();
        let run_id = uuid::Uuid::new_v4().to_string();

        history_model.update(&mut app, |history_model, ctx| {
            history_model.restore_conversations(
                terminal_view_id,
                vec![restored_conversation(conversation_id, run_id.clone(), None)],
                ctx,
            );
            history_model.update_conversation_status(
                terminal_view_id,
                conversation_id,
                ConversationStatus::InProgress,
                ctx,
            );
        });

        let expected_run_id = run_id.clone();
        let mut ai_client = MockAIClient::new();
        ai_client
            .expect_update_event_sequence_on_server()
            .withf(move |run_id, sequence| run_id == expected_run_id.as_str() && *sequence == 17)
            .times(1)
            .returning(|_, _| Ok(()));

        let server_api = ServerApiProvider::new_for_test().get();
        let poller = app.add_singleton_model(move |_| {
            OrchestrationEventPoller::new_with_clients_for_test(Arc::new(ai_client), server_api)
        });

        poller.update(&mut app, |poller, ctx| {
            poller.handle_poll_result(
                conversation_id,
                &run_id,
                0,
                vec![AgentRunEvent {
                    event_type: "new_message".to_string(),
                    run_id: run_id.clone(),
                    ref_id: Some("message-1".to_string()),
                    execution_id: None,
                    occurred_at: "2026-01-01T00:00:00Z".to_string(),
                    sequence: 17,
                }],
                vec![],
                ctx,
            );
        });

        let event = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let ModelEvent::UpdateMultiAgentConversation {
            conversation_id: persisted_conversation_id,
            conversation_data,
            ..
        } = event
        else {
            panic!("expected UpdateMultiAgentConversation event");
        };

        assert_eq!(persisted_conversation_id, conversation_id.to_string());
        assert_eq!(conversation_data.last_event_sequence, Some(17));
        poller.read(&app, |poller, _| {
            assert_eq!(poller.event_cursor.get(&conversation_id), Some(&17));
        });

        history_model.read(&app, |history_model, _| {
            let conversation = history_model
                .conversation(&conversation_id)
                .expect("conversation should exist");
            assert_eq!(conversation.last_event_sequence(), Some(17));
        });
    });
}
