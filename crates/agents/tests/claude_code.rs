//! Integration tests for [`agents::ClaudeCodeAgent`].
//!
//! These tests focus on construction, capability advertisement and health
//! defaults. The actual `execute()` path that spawns `claude` lives behind
//! `#[ignore]` because it depends on a logged-in CLI and would otherwise
//! make CI runs flaky and expensive.

use std::env;
use std::sync::Mutex;

use agents::{ClaudeCodeAgent, ClaudeModel};
use orchestrator::{Agent, AgentError, AgentId, Role};

// `std::env::set_var` is process-global. cargo runs tests in parallel within
// the same process, so any test that mutates PATH must hold this lock.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn new_returns_err_when_claude_not_on_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let saved_path = env::var_os("PATH");
    // SAFETY: synchronized via `ENV_LOCK`; restored before the lock drops.
    unsafe {
        env::set_var("PATH", "");
    }

    let result = ClaudeCodeAgent::new(AgentId("test".to_string()), ClaudeModel::Sonnet46);

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
                msg.contains("claude CLI not on PATH"),
                "unexpected error message: {msg}"
            );
            assert!(
                msg.contains("npm i -g @anthropic-ai/claude-code"),
                "error should hint at the install command: {msg}"
            );
        }
        Err(other) => panic!("expected AgentError::Other, got {other:?}"),
        Ok(_) => panic!("expected error when PATH is empty"),
    }
}

#[test]
fn capabilities_per_model() {
    // Skip if `claude` is genuinely not on PATH — there's nothing meaningful
    // to assert about a constructor that always fails.
    if which::which("claude").is_err() {
        eprintln!("skipping capabilities_per_model: claude CLI not on PATH");
        return;
    }

    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let opus = ClaudeCodeAgent::new(AgentId("opus".into()), ClaudeModel::Opus47).unwrap();
    let opus_caps = opus.capabilities();
    assert_eq!(opus_caps.max_context_tokens, 200_000);
    assert!(opus_caps.supports_tools);
    assert!(opus_caps.supports_vision);
    assert!(opus_caps.roles.contains(&Role::Planner));
    assert!(opus_caps.roles.contains(&Role::Reviewer));
    assert!(opus_caps.roles.contains(&Role::Worker));
    assert!(opus_caps.roles.contains(&Role::BulkRefactor));

    let sonnet = ClaudeCodeAgent::new(AgentId("sonnet".into()), ClaudeModel::Sonnet46).unwrap();
    let sonnet_caps = sonnet.capabilities();
    assert_eq!(sonnet_caps.max_context_tokens, 200_000);
    assert!(sonnet_caps.supports_tools);
    assert!(sonnet_caps.supports_vision);
    assert!(sonnet_caps.roles.contains(&Role::Worker));
    assert!(sonnet_caps.roles.contains(&Role::BulkRefactor));
    assert!(sonnet_caps.roles.contains(&Role::Reviewer));
    assert!(!sonnet_caps.roles.contains(&Role::Planner));

    let haiku = ClaudeCodeAgent::new(AgentId("haiku".into()), ClaudeModel::Haiku45).unwrap();
    let haiku_caps = haiku.capabilities();
    assert_eq!(haiku_caps.max_context_tokens, 200_000);
    assert!(haiku_caps.supports_tools);
    assert!(!haiku_caps.supports_vision);
    assert!(haiku_caps.roles.contains(&Role::Summarize));
    assert!(haiku_caps.roles.contains(&Role::Inline));
    assert!(haiku_caps.roles.contains(&Role::Worker));
}

#[test]
fn health_starts_healthy() {
    if which::which("claude").is_err() {
        eprintln!("skipping health_starts_healthy: claude CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let agent =
        ClaudeCodeAgent::new(AgentId("h".into()), ClaudeModel::Haiku45).expect("construct agent");
    let health = agent.health();
    assert!(health.healthy);
    assert_eq!(health.error_rate, 0.0);
}

#[test]
fn id_round_trips() {
    if which::which("claude").is_err() {
        eprintln!("skipping id_round_trips: claude CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let agent = ClaudeCodeAgent::new(AgentId("hello-1".into()), ClaudeModel::Sonnet46)
        .expect("construct agent");
    assert_eq!(agent.id(), AgentId("hello-1".to_string()));
}

// ----------------------------------------------------------------------------
// Manual / opt-in tests below: these spawn the real `claude` CLI and require
// the user to be authenticated. Run with:
//
//     cargo test -p agents -- --ignored
//
// They are intentionally not part of the default `cargo test` pass.
// ----------------------------------------------------------------------------

#[tokio::test]
#[ignore = "spawns the real `claude` CLI; requires login + network"]
async fn execute_emits_started_and_completed() {
    use futures_core::Stream;
    use orchestrator::{AgentEvent, Task, TaskContext, TaskId};
    use std::pin::Pin;

    let agent = ClaudeCodeAgent::new(AgentId("integration".into()), ClaudeModel::Haiku45)
        .expect("claude on PATH");
    let task = Task {
        id: TaskId::new(),
        role: Role::Inline,
        prompt: "Reply with the single word: pong".to_string(),
        context: TaskContext::default(),
        budget_hint: None,
    };

    let mut stream: Pin<Box<dyn Stream<Item = AgentEvent> + Send>> =
        agent.execute(task).await.expect("spawn claude");

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
