//! Integration tests for [`agents::CodexAgent`].
//!
//! Like the `claude_code` integration tests, these focus on construction,
//! capability advertisement and warning behaviour. The real `codex` spawn
//! path lives behind `#[ignore]` because it depends on a logged-in CLI and
//! would otherwise make CI runs flaky and expensive.

mod common;

use std::env;
use std::sync::{Arc, Mutex};

use agents::{CodexAgent, ReasoningEffort, ServiceTier};
use orchestrator::{Agent, AgentError, AgentId, Role};
use tracing::subscriber::with_default;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::Registry;
use tracing_subscriber::Layer;

use common::ENV_LOCK;

#[test]
fn new_returns_err_when_codex_not_on_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let saved_path = env::var_os("PATH");
    // SAFETY: synchronized via `ENV_LOCK`; restored before the lock drops.
    unsafe {
        env::set_var("PATH", "");
    }

    let result = CodexAgent::worker(AgentId("test".to_string()));

    // SAFETY: see above.
    unsafe {
        match saved_path {
            Some(p) => env::set_var("PATH", p),
            None => env::remove_var("PATH"),
        }
    }

    match result {
        Err(AgentError::Other(msg)) => {
            assert!(
                msg.contains("codex CLI not on PATH"),
                "unexpected error message: {msg}"
            );
            assert!(
                msg.contains("npm i -g @openai/codex"),
                "error should hint at the install command: {msg}"
            );
        }
        Err(other) => panic!("expected AgentError::Other, got {other:?}"),
        Ok(_) => panic!("expected error when PATH is empty"),
    }
}

#[test]
fn capabilities_per_profile() {
    if which::which("codex").is_err() {
        eprintln!("skipping capabilities_per_profile: codex CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let planner = CodexAgent::planner(AgentId("p".into())).expect("construct planner");
    let pcaps = planner.capabilities();
    assert!(pcaps.roles.contains(&Role::Planner));
    assert!(pcaps.roles.contains(&Role::Reviewer));
    assert!(!pcaps.roles.contains(&Role::Worker));
    assert!(pcaps.supports_tools);
    assert_eq!(planner.service_tier(), ServiceTier::Fast);
    assert_eq!(planner.reasoning_effort(), ReasoningEffort::High);

    let worker = CodexAgent::worker(AgentId("w".into())).expect("construct worker");
    let wcaps = worker.capabilities();
    assert!(wcaps.roles.contains(&Role::Worker));
    assert_eq!(wcaps.roles.len(), 1);
    assert!(wcaps.supports_tools);
    assert_eq!(worker.service_tier(), ServiceTier::Standard);
    assert_eq!(worker.reasoning_effort(), ReasoningEffort::Medium);

    let bulk =
        CodexAgent::bulk_refactor(AgentId("b".into())).expect("construct bulk_refactor");
    let bcaps = bulk.capabilities();
    assert!(bcaps.roles.contains(&Role::BulkRefactor));
    assert_eq!(bcaps.roles.len(), 1);
    assert!(bcaps.supports_tools);
    assert_eq!(bulk.service_tier(), ServiceTier::Standard);
    assert_eq!(bulk.reasoning_effort(), ReasoningEffort::Low);

    let summ = CodexAgent::summarizer(AgentId("s".into())).expect("construct summarizer");
    let scaps = summ.capabilities();
    assert!(scaps.roles.contains(&Role::Summarize));
    assert!(scaps.roles.contains(&Role::Inline));
    assert!(!scaps.supports_tools);
    assert_eq!(summ.service_tier(), ServiceTier::Standard);
    assert_eq!(summ.reasoning_effort(), ReasoningEffort::None);
}

#[test]
fn health_starts_healthy() {
    if which::which("codex").is_err() {
        eprintln!("skipping health_starts_healthy: codex CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let agent = CodexAgent::worker(AgentId("h".into())).expect("construct agent");
    let health = agent.health();
    assert!(health.healthy);
    assert_eq!(health.error_rate, 0.0);
}

#[test]
fn id_round_trips() {
    if which::which("codex").is_err() {
        eprintln!("skipping id_round_trips: codex CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let agent = CodexAgent::worker(AgentId("hello-1".into())).expect("construct agent");
    assert_eq!(agent.id(), AgentId("hello-1".to_string()));
}

/// Minimal `tracing` layer that records every event's formatted message into
/// a shared `Vec<String>`. Sufficient for asserting that
/// `CodexAgent::custom` emits a warning for non-canonical profile combos.
struct CapturingLayer {
    messages: Arc<Mutex<Vec<String>>>,
}

impl<S: Subscriber> Layer<S> for CapturingLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        struct MessageVisitor<'a>(&'a mut String);
        impl<'a> tracing::field::Visit for MessageVisitor<'a> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    use std::fmt::Write;
                    let _ = write!(self.0, "{value:?}");
                }
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    self.0.push_str(value);
                }
            }
        }
        let mut buf = String::new();
        event.record(&mut MessageVisitor(&mut buf));
        if !buf.is_empty() {
            self.messages.lock().unwrap().push(buf);
        }
    }
}

#[test]
fn custom_warns_for_non_canonical_combo() {
    if which::which("codex").is_err() {
        eprintln!("skipping custom_warns_for_non_canonical_combo: codex CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let messages = Arc::new(Mutex::new(Vec::<String>::new()));
    let layer = CapturingLayer {
        messages: Arc::clone(&messages),
    };
    let subscriber = Registry::default().with(layer);

    with_default(subscriber, || {
        // Fast + Low is *not* one of the four canonical profiles.
        let agent = CodexAgent::custom(
            AgentId("c".into()),
            ServiceTier::Fast,
            ReasoningEffort::Low,
        )
        .expect("construct custom");
        // Sanity: custom advertises every Role variant.
        let caps = agent.capabilities();
        assert!(caps.roles.contains(&Role::Planner));
        assert!(caps.roles.contains(&Role::Reviewer));
        assert!(caps.roles.contains(&Role::Worker));
        assert!(caps.roles.contains(&Role::BulkRefactor));
        assert!(caps.roles.contains(&Role::Summarize));
        assert!(caps.roles.contains(&Role::ToolRouter));
        assert!(caps.roles.contains(&Role::Inline));
    });

    let captured = messages.lock().unwrap();
    let joined = captured.join("\n");
    assert!(
        joined.contains("not in the standard profile map"),
        "expected warning about non-canonical profile, got: {joined}"
    );
}

#[test]
fn custom_does_not_warn_for_canonical_combo() {
    if which::which("codex").is_err() {
        eprintln!("skipping custom_does_not_warn_for_canonical_combo: codex CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let messages = Arc::new(Mutex::new(Vec::<String>::new()));
    let layer = CapturingLayer {
        messages: Arc::clone(&messages),
    };
    let subscriber = Registry::default().with(layer);

    with_default(subscriber, || {
        let _agent = CodexAgent::custom(
            AgentId("c".into()),
            ServiceTier::Standard,
            ReasoningEffort::Medium,
        )
        .expect("construct custom");
    });

    let captured = messages.lock().unwrap();
    assert!(
        captured
            .iter()
            .all(|m| !m.contains("not in the standard profile map")),
        "did not expect a non-canonical warning, got: {captured:?}"
    );
}

// ----------------------------------------------------------------------------
// Manual / opt-in tests below: these spawn the real `codex` CLI and require
// the user to be authenticated. Run with:
//
//     cargo test -p agents -- --ignored
// ----------------------------------------------------------------------------

#[tokio::test]
#[ignore = "spawns the real `codex` CLI; requires login + network"]
async fn execute_emits_started_and_terminal() {
    use futures_core::Stream;
    use orchestrator::{AgentEvent, Task, TaskContext, TaskId};
    use std::pin::Pin;

    let agent = CodexAgent::worker(AgentId("integration".into())).expect("codex on PATH");
    let task = Task {
        id: TaskId::new(),
        role: Role::Worker,
        prompt: "Reply with the single word: pong".to_string(),
        context: TaskContext::default(),
        budget_hint: None,
    };

    let mut stream: Pin<Box<dyn Stream<Item = AgentEvent> + Send>> =
        agent.execute(task).await.expect("spawn codex");

    use futures_util::StreamExt;
    let mut saw_started = false;
    let mut saw_terminal = false;
    while let Some(ev) = stream.next().await {
        match ev {
            AgentEvent::Started { .. } => saw_started = true,
            AgentEvent::Completed { .. } | AgentEvent::Failed { .. } => {
                saw_terminal = true;
                break;
            }
            _ => {}
        }
    }
    assert!(saw_started, "expected a Started event");
    assert!(saw_terminal, "expected a terminal event");
}
