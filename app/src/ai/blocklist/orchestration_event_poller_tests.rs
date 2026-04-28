use super::*;
use crate::ai::agent_events::{
    agent_event_backoff, agent_event_failures_exceeded_threshold,
    DEFAULT_AGENT_EVENT_RECONNECT_BACKOFF_STEPS,
};

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
