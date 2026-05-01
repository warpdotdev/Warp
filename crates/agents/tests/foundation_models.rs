//! Integration tests for the [`agents::FoundationModelsAgent`] stub.

use agents::FoundationModelsAgent;
use orchestrator::{Agent, AgentEvent, AgentId, Role, Task, TaskContext, TaskId};

use futures_util::StreamExt;

#[test]
fn new_succeeds_unconditionally() {
    // The stub never probes the host, so this should never panic regardless
    // of platform or environment.
    let _ = FoundationModelsAgent::new(AgentId("fm".into()));
    let _ = FoundationModelsAgent::new(AgentId("another".into()));
}

#[test]
fn health_is_unhealthy_so_router_skips() {
    let agent = FoundationModelsAgent::new(AgentId("fm".into()));
    let h = agent.health();
    assert!(!h.healthy, "stub must report unhealthy until PDX-13 lands");
    assert_eq!(h.error_rate, 0.0);
}

#[tokio::test]
async fn execute_returns_failed_event() {
    let agent = FoundationModelsAgent::new(AgentId("fm".into()));
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
                error.contains("stub") && error.contains("PDX-13"),
                "unexpected error message: {error}"
            );
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn capabilities_match_master_plan() {
    let agent = FoundationModelsAgent::new(AgentId("fm".into()));
    let caps = agent.capabilities();
    assert!(caps.roles.contains(&Role::Inline));
    assert!(caps.roles.contains(&Role::ToolRouter));
    assert!(caps.roles.contains(&Role::Summarize));
    // Foundation Models is a tiny on-device tier; nothing else.
    assert!(!caps.roles.contains(&Role::Planner));
    assert!(!caps.roles.contains(&Role::Reviewer));
    assert!(!caps.roles.contains(&Role::Worker));
    assert!(!caps.roles.contains(&Role::BulkRefactor));

    assert!(!caps.supports_tools);
    assert!(!caps.supports_vision);
    assert_eq!(caps.max_context_tokens, 4_096);
}

#[test]
fn id_round_trips() {
    let agent = FoundationModelsAgent::new(AgentId("fm-7".into()));
    assert_eq!(agent.id(), AgentId("fm-7".to_string()));
}
