use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use chrono::Utc;
use session_sharing_protocol::common::SessionId;

use crate::ai::agent::UserQueryMode;
use crate::ai::ambient_agents::{AmbientAgentTask, AmbientAgentTaskState};
use crate::server::server_api::ai::{MockAIClient, SpawnAgentResponse, TaskStatusMessage};
use crate::terminal::shared_session;

use super::{spawn_task, submit_run_followup, AmbientAgentEvent, SessionJoinInfo};

fn task_with(
    state: AmbientAgentTaskState,
    session_id: Option<String>,
    session_link: Option<String>,
) -> AmbientAgentTask {
    AmbientAgentTask {
        task_id: "550e8400-e29b-41d4-a716-446655440000".parse().unwrap(),
        parent_run_id: None,
        title: "title".to_string(),
        state,
        prompt: "prompt".to_string(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        updated_at: Utc::now(),
        status_message: None,
        source: None,
        session_id,
        session_link,
        creator: None,
        conversation_id: None,
        request_usage: None,
        agent_config_snapshot: None,
        artifacts: vec![],
        is_sandbox_running: true,
        last_event_sequence: None,
        children: vec![],
    }
}

#[tokio::test]
async fn followup_submits_before_polling_and_ignores_previous_session_id() {
    use futures::StreamExt;

    let previous_session_id = SessionId::new();
    let new_session_id = SessionId::new();
    let submitted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let call_count = Arc::new(AtomicUsize::new(0));
    let mut mock = MockAIClient::new();

    mock.expect_submit_run_followup().times(1).returning({
        let submitted = submitted.clone();
        move |observed_run_id, request| {
            assert_eq!(observed_run_id.to_string(), run_id().to_string());
            assert_eq!(request.message, "continue from here");
            submitted.store(true, Ordering::SeqCst);
            Ok(())
        }
    });

    mock.expect_get_ambient_agent_task().returning({
        let submitted = submitted.clone();
        let call_count = call_count.clone();
        move |observed_run_id| {
            assert!(submitted.load(Ordering::SeqCst));
            assert_eq!(observed_run_id.to_string(), run_id().to_string());
            let idx = call_count.fetch_add(1, Ordering::SeqCst);
            let (session_id, session_link) = if idx == 0 {
                (
                    previous_session_id.to_string(),
                    "https://example.com/session/previous".to_string(),
                )
            } else {
                (
                    new_session_id.to_string(),
                    "https://example.com/session/new".to_string(),
                )
            };

            Ok(task_with(
                AmbientAgentTaskState::InProgress,
                Some(session_id),
                Some(session_link),
            ))
        }
    });

    let ai_client = Arc::new(mock);
    let mut stream = Box::pin(submit_run_followup(
        "continue from here".to_string(),
        run_id(),
        Some(previous_session_id),
        ai_client,
        None,
    ));

    let event = stream
        .next()
        .await
        .expect("expected state changed")
        .expect("expected ok");
    assert!(matches!(
        event,
        AmbientAgentEvent::StateChanged {
            state: AmbientAgentTaskState::InProgress,
            ..
        }
    ));

    let event = stream
        .next()
        .await
        .expect("expected session started")
        .expect("expected ok");
    let AmbientAgentEvent::SessionStarted { session_join_info } = event else {
        panic!("Expected SessionStarted event");
    };
    assert_eq!(session_join_info.session_id, Some(new_session_id));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn followup_api_error_does_not_poll() {
    use futures::StreamExt;

    let mut mock = MockAIClient::new();
    mock.expect_submit_run_followup()
        .times(1)
        .returning(|_, _| Err(anyhow::anyhow!("follow-up rejected")));
    mock.expect_get_ambient_agent_task().times(0);

    let ai_client = Arc::new(mock);
    let mut stream = Box::pin(submit_run_followup(
        "continue".to_string(),
        run_id(),
        Some(SessionId::new()),
        ai_client,
        None,
    ));

    let err = stream
        .next()
        .await
        .expect("expected error")
        .expect_err("expected follow-up error");
    assert_eq!(err.to_string(), "follow-up rejected");
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn followup_terminal_failure_surfaces_status_message() {
    use futures::StreamExt;

    let mut mock = MockAIClient::new();
    mock.expect_submit_run_followup()
        .times(1)
        .returning(|_, _| Ok(()));
    mock.expect_get_ambient_agent_task()
        .times(1)
        .returning(|_| {
            let mut task = task_with(AmbientAgentTaskState::Error, None, None);
            task.status_message = Some(TaskStatusMessage {
                message: "failed to provision runtime".to_string(),
            });
            Ok(task)
        });

    let ai_client = Arc::new(mock);
    let mut stream = Box::pin(submit_run_followup(
        "continue".to_string(),
        run_id(),
        Some(SessionId::new()),
        ai_client,
        None,
    ));

    let event = stream
        .next()
        .await
        .expect("expected state changed")
        .expect("expected ok");
    assert!(matches!(
        event,
        AmbientAgentEvent::StateChanged {
            state: AmbientAgentTaskState::Error,
            ..
        }
    ));

    let err = stream
        .next()
        .await
        .expect("expected terminal error")
        .expect_err("expected terminal error");
    assert_eq!(err.to_string(), "failed to provision runtime");
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn followup_without_previous_session_id_accepts_joinable_session() {
    use futures::StreamExt;

    let session_id = SessionId::new();
    let expected_session_id = session_id;
    let mut mock = MockAIClient::new();

    mock.expect_submit_run_followup()
        .times(1)
        .returning(|_, _| Ok(()));
    mock.expect_get_ambient_agent_task()
        .times(1)
        .returning(move |_| {
            Ok(task_with(
                AmbientAgentTaskState::InProgress,
                Some(session_id.to_string()),
                Some("https://example.com/session/joinable".to_string()),
            ))
        });

    let ai_client = Arc::new(mock);
    let mut stream = Box::pin(submit_run_followup(
        "continue".to_string(),
        run_id(),
        None,
        ai_client,
        None,
    ));

    let event = stream
        .next()
        .await
        .expect("expected state changed")
        .expect("expected ok");
    assert!(matches!(
        event,
        AmbientAgentEvent::StateChanged {
            state: AmbientAgentTaskState::InProgress,
            ..
        }
    ));

    let event = stream
        .next()
        .await
        .expect("expected session started")
        .expect("expected ok");
    let AmbientAgentEvent::SessionStarted { session_join_info } = event else {
        panic!("Expected SessionStarted event");
    };
    assert_eq!(session_join_info.session_id, Some(expected_session_id));
    assert_eq!(
        session_join_info.session_link,
        "https://example.com/session/joinable"
    );
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn followup_without_previous_session_id_errors_if_run_finishes_before_session() {
    use futures::StreamExt;

    let mut mock = MockAIClient::new();

    mock.expect_submit_run_followup()
        .times(1)
        .returning(|_, _| Ok(()));
    mock.expect_get_ambient_agent_task()
        .times(1)
        .returning(|_| Ok(task_with(AmbientAgentTaskState::Succeeded, None, None)));

    let ai_client = Arc::new(mock);
    let mut stream = Box::pin(submit_run_followup(
        "continue".to_string(),
        run_id(),
        None,
        ai_client,
        None,
    ));

    let event = stream
        .next()
        .await
        .expect("expected state changed")
        .expect("expected ok");
    assert!(matches!(
        event,
        AmbientAgentEvent::StateChanged {
            state: AmbientAgentTaskState::Succeeded,
            ..
        }
    ));

    let err = stream
        .next()
        .await
        .expect("expected terminal error")
        .expect_err("expected terminal error");
    assert_eq!(
        err.to_string(),
        "Cloud follow-up finished before a new session became available"
    );
    assert!(stream.next().await.is_none());
}

fn run_id() -> crate::ai::ambient_agents::AmbientAgentTaskId {
    "550e8400-e29b-41d4-a716-446655440000".parse().unwrap()
}

#[tokio::test]
async fn poll_stops_on_terminal_failure_like_state() {
    use futures::StreamExt;

    let mut mock = MockAIClient::new();

    mock.expect_spawn_agent().returning(|_| {
        Ok(SpawnAgentResponse {
            task_id: "550e8400-e29b-41d4-a716-446655440000".parse().unwrap(),
            run_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            at_capacity: false,
        })
    });

    mock.expect_get_ambient_agent_task()
        .times(1)
        .returning(|_task_id| Ok(task_with(AmbientAgentTaskState::Error, None, None)));

    let ai_client = Arc::new(mock);
    let request = crate::server::server_api::ai::SpawnAgentRequest {
        prompt: "test".to_string(),
        mode: UserQueryMode::Normal,
        config: None,
        title: None,
        team: None,
        skill: None,
        attachments: vec![],
        interactive: None,
        parent_run_id: None,
        runtime_skills: vec![],
        referenced_attachments: vec![],
        conversation_id: None,
        initial_snapshot_token: None,
    };

    let mut stream = Box::pin(spawn_task(request, ai_client, None));

    let event = stream
        .next()
        .await
        .expect("expected event")
        .expect("expected ok");
    assert!(matches!(event, AmbientAgentEvent::TaskSpawned { .. }));

    let event = stream
        .next()
        .await
        .expect("expected event")
        .expect("expected ok");
    assert!(matches!(
        event,
        AmbientAgentEvent::StateChanged {
            state: AmbientAgentTaskState::Error,
            ..
        }
    ));

    assert!(stream.next().await.is_none());
}

#[test]
fn session_join_info_requires_session_id() {
    // session_link without session_id is not actionable for the cloud-mode pane
    // (the GET task handler may overwrite session_link with a conversation link
    // for tasks with synced conversation data, e.g. local-to-cloud handoff forks).
    let task = task_with(
        AmbientAgentTaskState::InProgress,
        None,
        Some("https://example.com/session/abc".to_string()),
    );
    assert!(SessionJoinInfo::from_task(&task).is_none());
}

#[test]
fn session_join_info_prefers_server_session_link_when_session_id_is_present() {
    let task = task_with(
        AmbientAgentTaskState::InProgress,
        Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
        Some("https://example.com/session/abc".to_string()),
    );

    let join_info = SessionJoinInfo::from_task(&task).expect("expected join info");
    assert_eq!(join_info.session_link, "https://example.com/session/abc");
    assert!(join_info.session_id.is_some());
}

#[test]
fn session_join_info_constructs_link_from_session_id_when_link_missing() {
    let task = task_with(
        AmbientAgentTaskState::InProgress,
        Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
        None,
    );

    let join_info = SessionJoinInfo::from_task(&task).expect("expected join info");
    assert!(!join_info.session_link.is_empty());
    assert!(join_info.session_id.is_some());
}

#[test]
fn session_join_info_falls_back_to_session_id() {
    let session_id = SessionId::new();
    let task = task_with(
        AmbientAgentTaskState::InProgress,
        Some(session_id.to_string()),
        None,
    );

    let join_info = SessionJoinInfo::from_task(&task).expect("expected join info");

    assert_eq!(join_info.session_id, Some(session_id));
    assert_eq!(
        join_info.session_link,
        shared_session::join_link(&session_id)
    );
}

#[test]
fn session_join_info_ignores_empty_link_and_invalid_session_id() {
    let task = task_with(
        AmbientAgentTaskState::InProgress,
        Some("not-a-session-id".to_string()),
        Some(String::new()),
    );

    assert_eq!(SessionJoinInfo::from_task(&task), None);
}

#[tokio::test]
async fn poll_for_session_join_info_waits_until_link_is_available() {
    use futures::StreamExt;

    let mut mock = MockAIClient::new();

    let call_count = Arc::new(AtomicUsize::new(0));

    mock.expect_spawn_agent().returning(|_| {
        Ok(SpawnAgentResponse {
            task_id: "550e8400-e29b-41d4-a716-446655440000".parse().unwrap(),
            run_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            at_capacity: false,
        })
    });

    mock.expect_get_ambient_agent_task().returning({
        let call_count = call_count.clone();
        move |_task_id| {
            let idx = call_count.fetch_add(1, Ordering::SeqCst);
            let task = if idx == 0 {
                task_with(AmbientAgentTaskState::InProgress, None, None)
            } else {
                task_with(
                    AmbientAgentTaskState::InProgress,
                    Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
                    Some("https://example.com/session/ready".to_string()),
                )
            };

            Ok(task)
        }
    });

    let ai_client = Arc::new(mock);
    let request = crate::server::server_api::ai::SpawnAgentRequest {
        prompt: "test".to_string(),
        mode: UserQueryMode::Normal,
        config: None,
        title: None,
        team: None,
        skill: None,
        attachments: vec![],
        interactive: None,
        parent_run_id: None,
        runtime_skills: vec![],
        referenced_attachments: vec![],
        conversation_id: None,
        initial_snapshot_token: None,
    };

    let mut stream = Box::pin(spawn_task(request, ai_client, None));

    // First event should be TaskSpawned
    let event = stream
        .next()
        .await
        .expect("expected event")
        .expect("expected ok");
    assert!(matches!(event, AmbientAgentEvent::TaskSpawned { .. }));

    // Second event should be StateChanged
    let event = stream
        .next()
        .await
        .expect("expected event")
        .expect("expected ok");
    assert!(matches!(event, AmbientAgentEvent::StateChanged { .. }));

    // Third event should be SessionStarted
    let event = stream
        .next()
        .await
        .expect("expected event")
        .expect("expected ok");
    if let AmbientAgentEvent::SessionStarted { session_join_info } = event {
        assert_eq!(
            session_join_info.session_link,
            "https://example.com/session/ready"
        );
    } else {
        panic!("Expected SessionStarted event");
    }
}
