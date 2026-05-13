use super::*;
use crate::ai::agent_events::AgentRunEvent;

fn make_run_event(
    sequence: i64,
    event_type: &str,
    run_id: &str,
    ref_id: Option<&str>,
) -> AgentRunEvent {
    AgentRunEvent {
        event_type: event_type.to_string(),
        run_id: run_id.to_string(),
        ref_id: ref_id.map(|value| value.to_string()),
        execution_id: None,
        occurred_at: "2026-01-01T00:00:00Z".to_string(),
        sequence,
    }
}

#[tokio::test]
async fn hydrator_does_not_fetch_cloud_message_for_matching_run() {
    let hydrator = MessageHydrator::new();
    let event = make_run_event(7, "new_message", "child-run", Some("msg-123"));

    assert!(hydrator
        .hydrate_event_for_recipient(&event, "child-run")
        .await
        .is_none());
}

#[tokio::test]
async fn hydrator_ignores_events_for_other_runs() {
    let hydrator = MessageHydrator::new();
    let event = make_run_event(7, "new_message", "other-run", Some("msg-123"));

    assert!(hydrator
        .hydrate_event_for_recipient(&event, "child-run")
        .await
        .is_none());
}
