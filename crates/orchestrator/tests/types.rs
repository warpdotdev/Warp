//! Unit tests for the foundation types defined in [`orchestrator`].
//!
//! These tests deliberately stay at the type / serde layer — exercising the
//! [`Agent`] trait belongs in the dispatcher and router crates that build
//! on this foundation.

use std::collections::HashMap;
use std::path::PathBuf;

use orchestrator::{AgentEvent, Role, Task, TaskContext, TaskId};
use serde_json::json;

fn sample_task() -> Task {
    let mut env = HashMap::new();
    env.insert("FOO".to_string(), "bar".to_string());

    let mut metadata = HashMap::new();
    metadata.insert("trace_id".to_string(), json!("abc-123"));
    metadata.insert("priority".to_string(), json!(7));

    Task {
        id: TaskId::new(),
        role: Role::Worker,
        prompt: "summarize this paragraph".to_string(),
        context: TaskContext {
            cwd: PathBuf::from("/tmp/work"),
            env,
            metadata,
        },
        budget_hint: Some(2048),
    }
}

#[test]
fn task_serde_round_trip() {
    let task = sample_task();
    let json = serde_json::to_string(&task).expect("serialize task");
    let decoded: Task = serde_json::from_str(&json).expect("deserialize task");

    assert_eq!(decoded.id, task.id);
    assert_eq!(decoded.role, task.role);
    assert_eq!(decoded.prompt, task.prompt);
    assert_eq!(decoded.context.cwd, task.context.cwd);
    assert_eq!(decoded.context.env, task.context.env);
    assert_eq!(decoded.context.metadata, task.context.metadata);
    assert_eq!(decoded.budget_hint, task.budget_hint);
}

fn round_trip_event(event: &AgentEvent) -> AgentEvent {
    let json = serde_json::to_string(event).expect("serialize event");
    serde_json::from_str(&json).expect("deserialize event")
}

#[test]
fn agent_event_started_round_trips() {
    let event = AgentEvent::Started {
        task_id: TaskId::new(),
    };
    let decoded = round_trip_event(&event);
    match (event, decoded) {
        (AgentEvent::Started { task_id: a }, AgentEvent::Started { task_id: b }) => {
            assert_eq!(a, b);
        }
        _ => panic!("variant mismatch after round trip"),
    }
}

#[test]
fn agent_event_output_chunk_round_trips() {
    let event = AgentEvent::OutputChunk {
        text: "hello world".to_string(),
    };
    let decoded = round_trip_event(&event);
    match decoded {
        AgentEvent::OutputChunk { text } => assert_eq!(text, "hello world"),
        _ => panic!("variant mismatch"),
    }
}

#[test]
fn agent_event_tool_call_round_trips() {
    let event = AgentEvent::ToolCall {
        name: "fs.read".to_string(),
        args: json!({ "path": "/etc/hosts" }),
    };
    let decoded = round_trip_event(&event);
    match decoded {
        AgentEvent::ToolCall { name, args } => {
            assert_eq!(name, "fs.read");
            assert_eq!(args, json!({ "path": "/etc/hosts" }));
        }
        _ => panic!("variant mismatch"),
    }
}

#[test]
fn agent_event_tool_result_round_trips() {
    let event = AgentEvent::ToolResult {
        name: "fs.read".to_string(),
        result: json!({ "bytes": 42 }),
    };
    let decoded = round_trip_event(&event);
    match decoded {
        AgentEvent::ToolResult { name, result } => {
            assert_eq!(name, "fs.read");
            assert_eq!(result, json!({ "bytes": 42 }));
        }
        _ => panic!("variant mismatch"),
    }
}

#[test]
fn agent_event_completed_round_trips() {
    let id = TaskId::new();
    let event = AgentEvent::Completed {
        task_id: id,
        summary: Some("ok".to_string()),
    };
    let decoded = round_trip_event(&event);
    match decoded {
        AgentEvent::Completed { task_id, summary } => {
            assert_eq!(task_id, id);
            assert_eq!(summary.as_deref(), Some("ok"));
        }
        _ => panic!("variant mismatch"),
    }
}

#[test]
fn agent_event_failed_round_trips() {
    let id = TaskId::new();
    let event = AgentEvent::Failed {
        task_id: id,
        error: "boom".to_string(),
    };
    let decoded = round_trip_event(&event);
    match decoded {
        AgentEvent::Failed { task_id, error } => {
            assert_eq!(task_id, id);
            assert_eq!(error, "boom");
        }
        _ => panic!("variant mismatch"),
    }
}

/// Compile-time guarantee that every [`Role`] variant is handled. If a new
/// variant is added without updating consumers, this match will fail to
/// compile.
#[test]
fn role_exhaustive_match() {
    fn describe(role: Role) -> &'static str {
        match role {
            Role::Planner => "planner",
            Role::Reviewer => "reviewer",
            Role::Worker => "worker",
            Role::BulkRefactor => "bulk_refactor",
            Role::Summarize => "summarize",
            Role::ToolRouter => "tool_router",
            Role::Inline => "inline",
        }
    }

    let all = [
        Role::Planner,
        Role::Reviewer,
        Role::Worker,
        Role::BulkRefactor,
        Role::Summarize,
        Role::ToolRouter,
        Role::Inline,
    ];

    for role in all {
        assert!(!describe(role).is_empty());
    }
}
