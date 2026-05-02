//! Integration tests for [`orchestrator::McpForwarder`].
//!
//! These tests verify the forwarding-target lifecycle: initial state, switching
//! between agents, no-op detection, clearing, and subscriber notifications.
//! All tests are synchronous except those that exercise the watch channel's
//! async `changed()` method.

use std::sync::Arc;

use orchestrator::{AgentId, ForwardingTarget, McpForwarder};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn agent(id: &str) -> AgentId {
    AgentId(id.to_string())
}

// ---------------------------------------------------------------------------
// Compile-time guarantees
// ---------------------------------------------------------------------------

/// [`McpForwarder`] must be shareable across async tasks.
fn _assert_send_sync<T: Send + Sync>() {}

#[test]
fn mcp_forwarder_is_send_sync() {
    _assert_send_sync::<McpForwarder>();
    _assert_send_sync::<ForwardingTarget>();
}

// ---------------------------------------------------------------------------
// Initial state
// ---------------------------------------------------------------------------

#[test]
fn new_forwarder_has_no_active_agent() {
    let fw = McpForwarder::new();
    assert_eq!(fw.active_agent_id(), None);
    assert_eq!(fw.current_target(), ForwardingTarget::None);
    assert!(!fw.current_target().is_active());
}

// ---------------------------------------------------------------------------
// set_active
// ---------------------------------------------------------------------------

#[test]
fn set_active_changes_target_and_returns_true() {
    let fw = McpForwarder::new();
    let changed = fw.set_active(agent("a"));
    assert!(changed, "first set_active should report a change");
    assert_eq!(fw.active_agent_id(), Some(agent("a")));
    assert!(fw.current_target().is_active());
    assert_eq!(fw.current_target().agent_id(), Some(&agent("a")));
}

#[test]
fn set_active_same_id_is_noop_returns_false() {
    let fw = McpForwarder::new();
    fw.set_active(agent("a"));

    let changed = fw.set_active(agent("a"));
    assert!(!changed, "setting the same agent should be a no-op");
    assert_eq!(fw.active_agent_id(), Some(agent("a")));
}

#[test]
fn set_active_different_id_returns_true() {
    let fw = McpForwarder::new();
    fw.set_active(agent("a"));

    let changed = fw.set_active(agent("b"));
    assert!(changed, "switching to a different agent should report a change");
    assert_eq!(fw.active_agent_id(), Some(agent("b")));
}

// ---------------------------------------------------------------------------
// clear_active
// ---------------------------------------------------------------------------

#[test]
fn clear_active_removes_target_returns_true() {
    let fw = McpForwarder::new();
    fw.set_active(agent("a"));

    let changed = fw.clear_active();
    assert!(changed, "clearing an active agent should report a change");
    assert_eq!(fw.active_agent_id(), None);
    assert_eq!(fw.current_target(), ForwardingTarget::None);
}

#[test]
fn clear_active_when_already_none_is_noop_returns_false() {
    let fw = McpForwarder::new();
    let changed = fw.clear_active();
    assert!(!changed, "clearing when already none should be a no-op");
}

#[test]
fn set_active_after_clear_works() {
    let fw = McpForwarder::new();
    fw.set_active(agent("a"));
    fw.clear_active();

    let changed = fw.set_active(agent("b"));
    assert!(changed);
    assert_eq!(fw.active_agent_id(), Some(agent("b")));
}

// ---------------------------------------------------------------------------
// Multiple switches
// ---------------------------------------------------------------------------

#[test]
fn multiple_switches_tracked_correctly() {
    let fw = McpForwarder::new();

    fw.set_active(agent("alpha"));
    assert_eq!(fw.active_agent_id(), Some(agent("alpha")));

    fw.set_active(agent("beta"));
    assert_eq!(fw.active_agent_id(), Some(agent("beta")));

    fw.set_active(agent("gamma"));
    assert_eq!(fw.active_agent_id(), Some(agent("gamma")));

    fw.clear_active();
    assert_eq!(fw.active_agent_id(), None);
}

// ---------------------------------------------------------------------------
// Arc sharing
// ---------------------------------------------------------------------------

#[test]
fn forwarder_shared_via_arc_stays_consistent() {
    let fw = Arc::new(McpForwarder::new());
    let fw2 = Arc::clone(&fw);

    fw.set_active(agent("a"));
    assert_eq!(fw2.active_agent_id(), Some(agent("a")));

    fw2.set_active(agent("b"));
    assert_eq!(fw.active_agent_id(), Some(agent("b")));
}

// ---------------------------------------------------------------------------
// Subscriber notifications (async)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscriber_notified_on_first_set_active() {
    let fw = McpForwarder::new();
    let mut rx = fw.subscribe();

    fw.set_active(agent("a"));

    // The watch channel marks the receiver as changed after a send.
    assert!(rx.has_changed().unwrap(), "subscriber should see the change");

    let target = rx.borrow_and_update().clone();
    assert_eq!(target, ForwardingTarget::Agent(agent("a")));
}

#[tokio::test]
async fn subscriber_not_notified_on_noop_set_active() {
    let fw = McpForwarder::new();
    fw.set_active(agent("a"));

    // Subscribe *after* the first set so the receiver is "up to date".
    let mut rx = fw.subscribe();
    let _ = rx.borrow_and_update(); // drain the initial mark

    // Setting the same agent again must not mark the receiver as changed.
    fw.set_active(agent("a"));
    assert!(
        !rx.has_changed().unwrap(),
        "no-op set_active must not notify subscribers"
    );
}

#[tokio::test]
async fn subscriber_notified_on_agent_switch() {
    let fw = McpForwarder::new();
    fw.set_active(agent("a"));

    let mut rx = fw.subscribe();
    let _ = rx.borrow_and_update(); // drain initial mark

    fw.set_active(agent("b"));

    assert!(rx.has_changed().unwrap());
    let target = rx.borrow_and_update().clone();
    assert_eq!(target, ForwardingTarget::Agent(agent("b")));
}

#[tokio::test]
async fn subscriber_notified_on_clear_active() {
    let fw = McpForwarder::new();
    fw.set_active(agent("a"));

    let mut rx = fw.subscribe();
    let _ = rx.borrow_and_update(); // drain initial mark

    fw.clear_active();

    assert!(rx.has_changed().unwrap());
    let target = rx.borrow_and_update().clone();
    assert_eq!(target, ForwardingTarget::None);
}

#[tokio::test]
async fn subscriber_sees_latest_target_after_rapid_switches() {
    let fw = McpForwarder::new();
    let mut rx = fw.subscribe();

    // Rapidly set multiple agents; the watch channel retains only the latest.
    for name in ["x", "y", "z"] {
        fw.set_active(agent(name));
    }

    // Even if notifications were coalesced, the final value must be "z".
    let target = rx.borrow_and_update().clone();
    assert_eq!(target, ForwardingTarget::Agent(agent("z")));
}

#[tokio::test]
async fn multiple_subscribers_all_notified() {
    let fw = McpForwarder::new();
    let mut rx1 = fw.subscribe();
    let mut rx2 = fw.subscribe();

    fw.set_active(agent("shared"));

    assert!(rx1.has_changed().unwrap());
    assert!(rx2.has_changed().unwrap());

    assert_eq!(
        rx1.borrow_and_update().clone(),
        ForwardingTarget::Agent(agent("shared"))
    );
    assert_eq!(
        rx2.borrow_and_update().clone(),
        ForwardingTarget::Agent(agent("shared"))
    );
}

#[tokio::test]
async fn changed_resolves_immediately_when_target_updated() {
    let fw = Arc::new(McpForwarder::new());
    let mut rx = fw.subscribe();
    let _ = rx.borrow_and_update(); // ensure we start "up to date"

    let fw2 = Arc::clone(&fw);
    tokio::spawn(async move {
        fw2.set_active(agent("async-agent"));
    })
    .await
    .unwrap();

    // `changed()` should resolve without a timeout because the task above
    // sent before we got here (joined via `.await`).
    rx.changed().await.expect("watch channel not closed");
    assert_eq!(
        rx.borrow_and_update().clone(),
        ForwardingTarget::Agent(agent("async-agent"))
    );
}
