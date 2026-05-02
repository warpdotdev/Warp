//! Integration tests for the handoff summarizer.
//!
//! These tests use a tiny stub agent (`SummarizerStub`) registered with a
//! real [`Router`] so we exercise the cheapest-model selection path
//! end-to-end without spinning up a subprocess.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use async_stream::stream;
use async_trait::async_trait;
use chrono::Utc;
use orchestrator::{
    format_handoff_prompt, Agent, AgentEvent, AgentEventStream, AgentId, AgentRegistration,
    Budget, Cap, Capabilities, HandoffState, HandoffSummarizer, Health, Provider, Role, Router,
    RouterHandoffSummarizer, SummarizerError, Task, TaskContext, TaskId,
};

/// Stub summarizer agent. Records how many times it has been invoked, then
/// emits a canned event sequence ending with `Completed { summary: <text> }`.
struct SummarizerStub {
    id: AgentId,
    capabilities: Capabilities,
    health: Health,
    summary_text: String,
    invocations: Arc<AtomicU32>,
}

impl SummarizerStub {
    fn new(id: &str, healthy: bool, summary_text: &str) -> (Self, Arc<AtomicU32>) {
        let invocations = Arc::new(AtomicU32::new(0));
        let stub = Self {
            id: AgentId(id.to_string()),
            capabilities: Capabilities {
                roles: [Role::Summarize].into_iter().collect::<HashSet<_>>(),
                max_context_tokens: 8_192,
                supports_tools: false,
                supports_vision: false,
            },
            health: Health {
                healthy,
                last_check: Utc::now(),
                error_rate: 0.0,
            },
            summary_text: summary_text.to_string(),
            invocations: invocations.clone(),
        };
        (stub, invocations)
    }
}

#[async_trait]
impl Agent for SummarizerStub {
    fn id(&self) -> AgentId {
        self.id.clone()
    }
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    fn health(&self) -> Health {
        self.health.clone()
    }
    async fn execute(
        &self,
        task: Task,
    ) -> Result<AgentEventStream, orchestrator::AgentError> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        let summary = self.summary_text.clone();
        let task_id = task.id;
        let s = stream! {
            yield AgentEvent::Started { task_id };
            yield AgentEvent::Completed { task_id, summary: Some(summary) };
        };
        Ok(Box::pin(s))
    }
}

/// Stub that emits `OutputChunk`s but no Completed-summary; the summarizer
/// must fall back to concatenating the chunks.
struct ChunkOnlyStub {
    id: AgentId,
    capabilities: Capabilities,
    health: Health,
    chunks: Vec<String>,
}

impl ChunkOnlyStub {
    fn new(id: &str, chunks: Vec<&str>) -> Self {
        Self {
            id: AgentId(id.to_string()),
            capabilities: Capabilities {
                roles: [Role::Summarize].into_iter().collect::<HashSet<_>>(),
                max_context_tokens: 8_192,
                supports_tools: false,
                supports_vision: false,
            },
            health: Health {
                healthy: true,
                last_check: Utc::now(),
                error_rate: 0.0,
            },
            chunks: chunks.into_iter().map(str::to_string).collect(),
        }
    }
}

#[async_trait]
impl Agent for ChunkOnlyStub {
    fn id(&self) -> AgentId {
        self.id.clone()
    }
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    fn health(&self) -> Health {
        self.health.clone()
    }
    async fn execute(
        &self,
        task: Task,
    ) -> Result<AgentEventStream, orchestrator::AgentError> {
        let chunks = self.chunks.clone();
        let task_id = task.id;
        let s = stream! {
            yield AgentEvent::Started { task_id };
            for chunk in chunks {
                yield AgentEvent::OutputChunk { text: chunk };
            }
            yield AgentEvent::Completed { task_id, summary: None };
        };
        Ok(Box::pin(s))
    }
}

/// Stub that fails its task with `AgentEvent::Failed`.
struct FailingStub {
    id: AgentId,
    capabilities: Capabilities,
    health: Health,
}

impl FailingStub {
    fn new(id: &str) -> Self {
        Self {
            id: AgentId(id.to_string()),
            capabilities: Capabilities {
                roles: [Role::Summarize].into_iter().collect::<HashSet<_>>(),
                max_context_tokens: 8_192,
                supports_tools: false,
                supports_vision: false,
            },
            health: Health {
                healthy: true,
                last_check: Utc::now(),
                error_rate: 0.0,
            },
        }
    }
}

#[async_trait]
impl Agent for FailingStub {
    fn id(&self) -> AgentId {
        self.id.clone()
    }
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    fn health(&self) -> Health {
        self.health.clone()
    }
    async fn execute(
        &self,
        task: Task,
    ) -> Result<AgentEventStream, orchestrator::AgentError> {
        let task_id = task.id;
        let s = stream! {
            yield AgentEvent::Failed {
                task_id,
                error: "stub failure".to_string(),
            };
        };
        Ok(Box::pin(s))
    }
}

fn ample_budget(providers: &[Provider]) -> Arc<Budget> {
    let mut caps = HashMap::new();
    for p in providers {
        caps.insert(
            *p,
            Cap {
                monthly_micro_dollars: 1_000_000_000,
                session_micro_dollars: 1_000_000_000,
            },
        );
    }
    Arc::new(Budget::new(caps))
}

fn handoff_state_for(prompt: &str) -> HandoffState {
    HandoffState {
        task: Task {
            id: TaskId::new(),
            role: Role::Worker,
            prompt: prompt.to_string(),
            context: TaskContext {
                cwd: PathBuf::from("/tmp"),
                env: HashMap::new(),
                metadata: HashMap::new(),
            },
            budget_hint: None,
        },
        events: vec![],
        from_agent: Some(AgentId("worker-1".to_string())),
        to_role: Some(Role::Reviewer),
    }
}

#[tokio::test]
async fn router_summarizer_uses_completed_summary() {
    let budget = ample_budget(&[Provider::FoundationModels]);
    let mut router = Router::new(budget);
    let (stub, invocations) =
        SummarizerStub::new("foundation-models", true, "Goal: …\nProgress: …");
    router.register(AgentRegistration {
        agent: Arc::new(stub),
        provider: Provider::FoundationModels,
        estimated_micros_per_task: 0,
    });

    let summarizer = RouterHandoffSummarizer::new(Arc::new(router));
    let summary = summarizer
        .summarize(handoff_state_for("ship the feature"))
        .await
        .expect("summarize succeeds");

    assert_eq!(invocations.load(Ordering::SeqCst), 1);
    assert_eq!(summary.model.0, "foundation-models");
    assert_eq!(summary.text, "Goal: …\nProgress: …");
}

#[tokio::test]
async fn router_summarizer_picks_cheapest_when_multiple_summarizers_registered() {
    // Three healthy summarize-capable agents at three cost points. The
    // summarizer must pick the cheapest one (Foundation Models @ $0).
    let budget = ample_budget(&[
        Provider::FoundationModels,
        Provider::ClaudeCode,
        Provider::Codex,
    ]);
    let mut router = Router::new(budget);

    let (cheap, cheap_calls) = SummarizerStub::new("foundation-models", true, "cheap-result");
    let (mid, mid_calls) = SummarizerStub::new("haiku-4-5", true, "mid-result");
    let (expensive, expensive_calls) =
        SummarizerStub::new("sonnet-expensive", true, "expensive-result");

    router.register(AgentRegistration {
        agent: Arc::new(cheap),
        provider: Provider::FoundationModels,
        estimated_micros_per_task: 0,
    });
    router.register(AgentRegistration {
        agent: Arc::new(mid),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 800,
    });
    router.register(AgentRegistration {
        agent: Arc::new(expensive),
        provider: Provider::Codex,
        estimated_micros_per_task: 50_000,
    });

    let summarizer = RouterHandoffSummarizer::new(Arc::new(router));
    let summary = summarizer
        .summarize(handoff_state_for("anything"))
        .await
        .expect("summarize succeeds");

    assert_eq!(summary.model.0, "foundation-models");
    assert_eq!(cheap_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mid_calls.load(Ordering::SeqCst), 0);
    assert_eq!(expensive_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn router_summarizer_falls_back_to_haiku_when_foundation_models_unhealthy() {
    // Foundation Models is registered but unhealthy (stub default in
    // production). The summarizer must transparently pick Haiku.
    let budget = ample_budget(&[Provider::FoundationModels, Provider::ClaudeCode]);
    let mut router = Router::new(budget);

    let (fm, fm_calls) = SummarizerStub::new("foundation-models", false, "should-not-run");
    let (haiku, haiku_calls) = SummarizerStub::new("haiku-4-5", true, "haiku-result");

    router.register(AgentRegistration {
        agent: Arc::new(fm),
        provider: Provider::FoundationModels,
        estimated_micros_per_task: 0,
    });
    router.register(AgentRegistration {
        agent: Arc::new(haiku),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 800,
    });

    let summarizer = RouterHandoffSummarizer::new(Arc::new(router));
    let summary = summarizer
        .summarize(handoff_state_for("anything"))
        .await
        .expect("summarize succeeds");

    assert_eq!(summary.model.0, "haiku-4-5");
    assert_eq!(summary.text, "haiku-result");
    assert_eq!(fm_calls.load(Ordering::SeqCst), 0);
    assert_eq!(haiku_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn router_summarizer_concatenates_chunks_when_no_completed_summary() {
    let budget = ample_budget(&[Provider::FoundationModels]);
    let mut router = Router::new(budget);
    router.register(AgentRegistration {
        agent: Arc::new(ChunkOnlyStub::new(
            "chunk-only",
            vec!["Goal: ", "ship\n", "Progress: ", "drafted PR"],
        )),
        provider: Provider::FoundationModels,
        estimated_micros_per_task: 0,
    });

    let summarizer = RouterHandoffSummarizer::new(Arc::new(router));
    let summary = summarizer
        .summarize(handoff_state_for("ship"))
        .await
        .expect("summarize succeeds");

    assert_eq!(summary.text, "Goal: ship\nProgress: drafted PR");
}

#[tokio::test]
async fn router_summarizer_propagates_failed_event_as_execute_error() {
    let budget = ample_budget(&[Provider::FoundationModels]);
    let mut router = Router::new(budget);
    router.register(AgentRegistration {
        agent: Arc::new(FailingStub::new("flaky")),
        provider: Provider::FoundationModels,
        estimated_micros_per_task: 0,
    });

    let summarizer = RouterHandoffSummarizer::new(Arc::new(router));
    let err = summarizer
        .summarize(handoff_state_for("ship"))
        .await
        .expect_err("expected Execute error");
    match err {
        SummarizerError::Execute { agent, .. } => assert_eq!(agent.0, "flaky"),
        other => panic!("expected Execute, got {other:?}"),
    }
}

#[tokio::test]
async fn router_summarizer_returns_routing_when_no_summarize_capable_agent() {
    let budget = ample_budget(&[Provider::ClaudeCode]);
    let router = Router::new(budget);

    let summarizer = RouterHandoffSummarizer::new(Arc::new(router));
    let err = summarizer
        .summarize(handoff_state_for("anything"))
        .await
        .expect_err("expected Routing error");
    assert!(matches!(err, SummarizerError::Routing(_)));
}

#[test]
fn format_handoff_prompt_is_deterministic_and_contains_key_fields() {
    let mut state = handoff_state_for("Refactor authentication subsystem");
    state.events = vec![
        AgentEvent::Started {
            task_id: state.task.id,
        },
        AgentEvent::OutputChunk {
            text: "Inspecting auth.rs\nmore detail".to_string(),
        },
        AgentEvent::ToolCall {
            name: "read_file".to_string(),
            args: serde_json::json!({"path": "auth.rs"}),
        },
        AgentEvent::Completed {
            task_id: state.task.id,
            summary: Some("Drafted refactor outline".to_string()),
        },
    ];

    let a = format_handoff_prompt(&state);
    let b = format_handoff_prompt(&state);
    assert_eq!(a, b, "prompt must be deterministic");

    assert!(a.contains("Refactor authentication subsystem"));
    assert!(a.contains("From agent: worker-1"));
    assert!(a.contains("Reviewer"));
    assert!(a.contains("Tool call: read_file"));
    assert!(a.contains("Drafted refactor outline"));
    // Output chunk truncated to first non-empty line.
    assert!(a.contains("Output: Inspecting auth.rs"));
    assert!(!a.contains("more detail"));
    assert!(a.contains("Goal, Progress, Pending, Hand-off notes"));
}

#[test]
fn format_handoff_prompt_handles_empty_events() {
    let state = handoff_state_for("ship");
    let p = format_handoff_prompt(&state);
    assert!(p.contains("- (none)"));
}
