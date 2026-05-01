//! Integration tests for [`agents::OllamaAgent`].
//!
//! Covers the constructor's PATH handling, capability defaults when probing
//! fails, and role gating. The real `ollama run` smoke test lives behind
//! `#[ignore]` because it depends on a running daemon and a pulled model.

mod common;

use std::env;

use agents::OllamaAgent;
use orchestrator::{Agent, AgentError, AgentId, Role};

use common::ENV_LOCK;

#[test]
fn new_returns_err_when_ollama_not_on_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let saved_path = env::var_os("PATH");
    // SAFETY: synchronized via `ENV_LOCK`; restored before the lock drops.
    unsafe {
        env::set_var("PATH", "");
    }

    let result = OllamaAgent::new(
        AgentId("test".to_string()),
        "qwen2.5-coder:latest".to_string(),
    );

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
                msg.contains("ollama CLI not on PATH"),
                "unexpected error message: {msg}"
            );
            assert!(
                msg.contains("ollama.com"),
                "error should hint at the install URL: {msg}"
            );
        }
        Err(other) => panic!("expected AgentError::Other, got {other:?}"),
        Ok(_) => panic!("expected error when PATH is empty"),
    }
}

#[test]
fn capabilities_excludes_planner_and_reviewer() {
    if which::which("ollama").is_err() {
        eprintln!("skipping capabilities_excludes_planner_and_reviewer: ollama CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    // Use a model name that almost certainly isn't pulled so the probe path
    // exercises its fallback. The constructor should still succeed.
    let agent = OllamaAgent::new(
        AgentId("o".into()),
        "definitely-not-a-real-model:nope".to_string(),
    )
    .expect("construct ollama agent");

    let caps = agent.capabilities();
    assert!(!caps.roles.contains(&Role::Planner));
    assert!(!caps.roles.contains(&Role::Reviewer));
    assert!(caps.roles.contains(&Role::Worker));
    assert!(caps.roles.contains(&Role::BulkRefactor));
    assert!(caps.roles.contains(&Role::Summarize));
    assert!(caps.roles.contains(&Role::Inline));
}

#[test]
fn capability_detection_falls_back_to_safe_defaults_when_show_fails() {
    if which::which("ollama").is_err() {
        eprintln!(
            "skipping capability_detection_falls_back_to_safe_defaults_when_show_fails: \
             ollama CLI not on PATH"
        );
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let agent = OllamaAgent::new(
        AgentId("o".into()),
        "definitely-not-a-real-model:nope".to_string(),
    )
    .expect("construct ollama agent");
    let caps = agent.capabilities();
    // The default fallback context window from the source.
    assert_eq!(caps.max_context_tokens, 8_192);
    // Without a successful probe we must not lie about tool/vision support.
    assert!(!caps.supports_tools);
    assert!(!caps.supports_vision);
}

#[tokio::test]
async fn execute_with_planner_role_emits_failed_event() {
    if which::which("ollama").is_err() {
        eprintln!("skipping execute_with_planner_role_emits_failed_event: ollama CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let agent = OllamaAgent::new(
        AgentId("o".into()),
        "definitely-not-a-real-model:nope".to_string(),
    )
    .expect("construct ollama agent");

    use futures_util::StreamExt;
    use orchestrator::{AgentEvent, Task, TaskContext, TaskId};

    let task = Task {
        id: TaskId::new(),
        role: Role::Planner,
        prompt: "irrelevant".to_string(),
        context: TaskContext::default(),
        budget_hint: None,
    };

    let mut stream = agent.execute(task).await.expect("execute returns stream");
    let mut saw_failed = false;
    while let Some(ev) = stream.next().await {
        if let AgentEvent::Failed { error, .. } = ev {
            assert!(
                error.contains("planning/review"),
                "unexpected error message: {error}"
            );
            saw_failed = true;
            break;
        }
    }
    assert!(saw_failed, "expected a Failed event for Planner role");
}

#[test]
fn id_round_trips() {
    if which::which("ollama").is_err() {
        eprintln!("skipping id_round_trips: ollama CLI not on PATH");
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let agent = OllamaAgent::new(
        AgentId("hello-1".into()),
        "definitely-not-a-real-model:nope".to_string(),
    )
    .expect("construct ollama agent");
    assert_eq!(agent.id(), AgentId("hello-1".to_string()));
}

// ----------------------------------------------------------------------------
// Manual / opt-in tests below: these spawn the real `ollama` CLI and require
// a running daemon plus a pulled model. Run with:
//
//     cargo test -p agents -- --ignored
// ----------------------------------------------------------------------------

#[tokio::test]
#[ignore = "spawns the real `ollama` CLI; requires daemon + pulled model"]
async fn execute_emits_started_and_terminal() {
    use futures_util::StreamExt;
    use orchestrator::{AgentEvent, Task, TaskContext, TaskId};

    let model =
        std::env::var("OLLAMA_TEST_MODEL").unwrap_or_else(|_| "qwen2.5-coder:latest".to_string());
    let agent =
        OllamaAgent::new(AgentId("integration".into()), model).expect("construct ollama agent");
    let task = Task {
        id: TaskId::new(),
        role: Role::Worker,
        prompt: "Reply with the single word: pong".to_string(),
        context: TaskContext::default(),
        budget_hint: None,
    };

    let mut stream = agent.execute(task).await.expect("spawn ollama");
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
