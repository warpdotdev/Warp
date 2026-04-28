use std::sync::Arc;

use mockall::predicate::eq;

use super::*;
use crate::server::server_api::ai::{
    AIClient, AgentRunEvent, MockAIClient, ReadAgentMessageResponse,
};

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
async fn hydrator_reads_new_message_for_matching_run() {
    let mut ai_client = MockAIClient::new();
    ai_client
        .expect_read_agent_message()
        .with(eq("msg-123"))
        .times(1)
        .returning(|_| {
            Ok(ReadAgentMessageResponse {
                message_id: "msg-123".to_string(),
                sender_run_id: "parent-run".to_string(),
                subject: "Need a redirect".to_string(),
                body: "Switch to the failing test first.".to_string(),
                sent_at: "2026-01-01T00:00:00Z".to_string(),
                delivered_at: None,
                read_at: Some("2026-01-01T00:00:01Z".to_string()),
            })
        });

    let ai_client: Arc<dyn AIClient> = Arc::new(ai_client);
    let hydrator = MessageHydrator::new(ai_client);
    let event = make_run_event(7, "new_message", "child-run", Some("msg-123"));

    let hydrated = hydrator
        .hydrate_event_for_recipient(&event, "child-run")
        .await
        .expect("expected hydrated message");

    assert_eq!(hydrated.message_id, "msg-123");
    assert_eq!(hydrated.sender_agent_id, "parent-run");
    assert_eq!(hydrated.subject, "Need a redirect");
    assert_eq!(hydrated.message_body, "Switch to the failing test first.");
    assert_eq!(hydrated.addresses, vec!["child-run".to_string()]);
}

#[tokio::test]
async fn hydrator_ignores_events_for_other_runs() {
    let ai_client: Arc<dyn AIClient> = Arc::new(MockAIClient::new());
    let hydrator = MessageHydrator::new(ai_client);
    let event = make_run_event(7, "new_message", "other-run", Some("msg-123"));

    assert!(hydrator
        .hydrate_event_for_recipient(&event, "child-run")
        .await
        .is_none());
}
