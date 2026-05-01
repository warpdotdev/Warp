//! Integration tests for the [`agents::RemoteAgent`] stub.

use agents::RemoteAgent;
use orchestrator::{Agent, AgentEvent, AgentId, Role, Task, TaskContext, TaskId};

use futures_util::StreamExt;

#[test]
fn new_with_none_url_is_unhealthy() {
    let agent = RemoteAgent::new(AgentId("r".into()), None);
    let h = agent.health();
    assert!(!h.healthy, "no cloud URL ⇒ unhealthy");
    assert_eq!(agent.cloud_url(), None);
}

#[tokio::test]
async fn new_with_some_url_is_healthy_but_execute_still_fails() {
    let agent = RemoteAgent::new(
        AgentId("r".into()),
        Some("wss://cloud.example/agent".to_string()),
    );
    assert!(agent.health().healthy, "configured URL ⇒ healthy stub");
    assert_eq!(agent.cloud_url(), Some("wss://cloud.example/agent"));

    let task = Task {
        id: TaskId::new(),
        role: Role::Worker,
        prompt: "hi".to_string(),
        context: TaskContext::default(),
        budget_hint: None,
    };

    let mut stream = agent.execute(task).await.expect("execute returns stream");
    let ev = stream.next().await.expect("at least one event");
    assert!(
        matches!(ev, AgentEvent::Failed { .. }),
        "stub must always fail"
    );
}

#[tokio::test]
async fn execute_returns_failed_event_with_pdx_c2_hint() {
    let agent = RemoteAgent::new(AgentId("r".into()), None);
    let task = Task {
        id: TaskId::new(),
        role: Role::Inline,
        prompt: "hi".to_string(),
        context: TaskContext::default(),
        budget_hint: None,
    };
    let mut stream = agent.execute(task).await.expect("execute returns stream");
    let ev = stream.next().await.expect("at least one event");
    match ev {
        AgentEvent::Failed { error, .. } => {
            assert!(
                error.contains("PDX-C2"),
                "expected PDX-C2 hint in error: {error}"
            );
            assert!(
                error.to_lowercase().contains("cloud"),
                "expected cloud-backend mention: {error}"
            );
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn capabilities_cover_every_role() {
    let agent = RemoteAgent::new(AgentId("r".into()), None);
    let caps = agent.capabilities();
    for role in [
        Role::Planner,
        Role::Reviewer,
        Role::Worker,
        Role::BulkRefactor,
        Role::Summarize,
        Role::ToolRouter,
        Role::Inline,
    ] {
        assert!(caps.roles.contains(&role), "missing role: {role:?}");
    }
    assert!(caps.supports_tools);
    assert!(caps.supports_vision);
    assert_eq!(caps.max_context_tokens, 200_000);
}

#[test]
fn id_round_trips() {
    let agent = RemoteAgent::new(AgentId("r-9".into()), None);
    assert_eq!(agent.id(), AgentId("r-9".to_string()));
}
