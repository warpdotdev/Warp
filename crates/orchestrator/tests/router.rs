//! Integration tests for [`orchestrator::Router`].
//!
//! These tests use a `MockAgent` rather than spawning a real subprocess —
//! the router is a pure policy gate, so a stub that returns canned values
//! from the [`Agent`] trait is sufficient (and keeps tests deterministic
//! and fast).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use orchestrator::{
    Agent, AgentEventStream, AgentId, AgentRegistration, Budget, BudgetTier, Cap, Capabilities,
    Health, Provider, Role, Router, RouterError, Task, TaskContext, TaskId,
};

/// Stub [`Agent`] used to drive the router under test.
///
/// `execute()` is unreachable (the router only reads metadata) but we panic
/// loudly if anyone wires it up by mistake — better than silently returning
/// an empty stream.
struct MockAgent {
    id: AgentId,
    capabilities: Capabilities,
    health: Health,
}

impl MockAgent {
    fn new(id: &str, roles: &[Role], healthy: bool) -> Self {
        Self {
            id: AgentId(id.to_string()),
            capabilities: Capabilities {
                roles: roles.iter().copied().collect::<HashSet<_>>(),
                max_context_tokens: 100_000,
                supports_tools: true,
                supports_vision: false,
            },
            health: Health {
                healthy,
                last_check: Utc::now(),
                error_rate: 0.0,
            },
        }
    }
}

#[async_trait]
impl Agent for MockAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    async fn execute(
        &self,
        _task: Task,
    ) -> Result<AgentEventStream, orchestrator::AgentError> {
        unreachable!("Router tests must not invoke Agent::execute on the mock");
    }

    fn health(&self) -> Health {
        self.health.clone()
    }
}

fn task_with_role(role: Role) -> Task {
    Task {
        id: TaskId::new(),
        role,
        prompt: "test prompt".to_string(),
        context: TaskContext {
            cwd: PathBuf::from("/tmp"),
            env: HashMap::new(),
            metadata: HashMap::new(),
        },
        budget_hint: None,
    }
}

/// Convenience for the common "ample budget" case: a generous monthly cap
/// keeps every provider in [`BudgetTier::Healthy`] for the duration of the
/// test unless the test explicitly drains it.
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

/// Drain `provider`'s budget until [`Budget::current_tier`] reports `target`.
async fn drain_to_tier(budget: &Budget, provider: Provider, cap: Cap, target: BudgetTier) {
    let monthly = cap.monthly_micro_dollars;
    let charge = match target {
        BudgetTier::Healthy => return,
        // 50% of monthly cap -> Warning boundary.
        BudgetTier::Warning => monthly / 2,
        // 90% of monthly cap -> Critical boundary.
        BudgetTier::Critical => (monthly / 10) * 9,
        // Saturate -> Halted (try_charge will latch the tier).
        BudgetTier::Halted => monthly,
    };
    let _ = budget.try_charge(provider, charge).await;
}

#[tokio::test]
async fn select_returns_capable_agent_when_only_one_matches() {
    let budget = ample_budget(&[Provider::ClaudeCode, Provider::Codex]);
    let mut router = Router::new(budget);

    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("planner-only", &[Role::Planner], true)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 1_000,
    });
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("worker-only", &[Role::Worker], true)),
        provider: Provider::Codex,
        estimated_micros_per_task: 1_000,
    });

    let task = task_with_role(Role::Planner);
    let chosen = router.select(&task).await.expect("select should succeed");
    assert_eq!(chosen.id().0, "planner-only");
}

#[tokio::test]
async fn select_excludes_unhealthy_agents() {
    let budget = ample_budget(&[Provider::ClaudeCode, Provider::Codex]);
    let mut router = Router::new(budget);

    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("a-sick", &[Role::Worker], false)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 1_000,
    });
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("z-well", &[Role::Worker], true)),
        provider: Provider::Codex,
        estimated_micros_per_task: 1_000,
    });

    let task = task_with_role(Role::Worker);
    let chosen = router.select(&task).await.expect("healthy survivor wins");
    assert_eq!(chosen.id().0, "z-well");
}

#[tokio::test]
async fn select_returns_nocapableagent_when_no_role_match() {
    let budget = ample_budget(&[Provider::ClaudeCode]);
    let mut router = Router::new(budget);

    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("worker", &[Role::Worker], true)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 1_000,
    });

    let task = task_with_role(Role::Reviewer);
    let err = router.select(&task).await.err().expect("expected error");
    assert!(matches!(err, RouterError::NoCapableAgent(Role::Reviewer)));
}

#[tokio::test]
async fn select_returns_allunhealthy_when_all_unhealthy() {
    let budget = ample_budget(&[Provider::ClaudeCode, Provider::Codex]);
    let mut router = Router::new(budget);

    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("a", &[Role::Worker], false)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 1_000,
    });
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("b", &[Role::Worker], false)),
        provider: Provider::Codex,
        estimated_micros_per_task: 1_000,
    });

    let task = task_with_role(Role::Worker);
    let err = router.select(&task).await.err().expect("expected error");
    assert!(matches!(err, RouterError::AllUnhealthy));
}

#[tokio::test]
async fn select_excludes_halted_provider() {
    // Two providers; halt one and confirm the surviving provider's agent
    // is chosen.
    let cap = Cap {
        monthly_micro_dollars: 1_000,
        session_micro_dollars: 1_000_000_000,
    };
    let mut caps = HashMap::new();
    caps.insert(Provider::ClaudeCode, cap);
    caps.insert(
        Provider::Codex,
        Cap {
            monthly_micro_dollars: 1_000_000_000,
            session_micro_dollars: 1_000_000_000,
        },
    );
    let budget = Arc::new(Budget::new(caps));

    drain_to_tier(&budget, Provider::ClaudeCode, cap, BudgetTier::Halted).await;
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Halted
    );

    let mut router = Router::new(budget);
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("halted-agent", &[Role::Worker], true)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    });
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("ok-agent", &[Role::Worker], true)),
        provider: Provider::Codex,
        estimated_micros_per_task: 100,
    });

    let task = task_with_role(Role::Worker);
    let chosen = router.select(&task).await.unwrap();
    assert_eq!(chosen.id().0, "ok-agent");
}

#[tokio::test]
async fn select_with_critical_tier_excludes_non_planner_reviewer() {
    // Single provider in Critical tier; tasks for Worker should be excluded
    // even though the agent is otherwise capable and healthy.
    let cap = Cap {
        monthly_micro_dollars: 1_000,
        session_micro_dollars: 1_000_000_000,
    };
    let mut caps = HashMap::new();
    caps.insert(Provider::ClaudeCode, cap);
    let budget = Arc::new(Budget::new(caps));

    drain_to_tier(&budget, Provider::ClaudeCode, cap, BudgetTier::Critical).await;
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Critical
    );

    let mut router = Router::new(budget);
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new(
            "all-roles",
            &[Role::Worker, Role::Planner, Role::Reviewer],
            true,
        )),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    });

    let task = task_with_role(Role::Worker);
    let err = router.select(&task).await.err().expect("expected error");
    assert!(matches!(err, RouterError::NoFallbackForTier(Role::Worker)));
}

#[tokio::test]
async fn select_with_critical_tier_returns_planner_when_task_role_is_planner() {
    let cap = Cap {
        monthly_micro_dollars: 1_000,
        session_micro_dollars: 1_000_000_000,
    };
    let mut caps = HashMap::new();
    caps.insert(Provider::ClaudeCode, cap);
    let budget = Arc::new(Budget::new(caps));

    drain_to_tier(&budget, Provider::ClaudeCode, cap, BudgetTier::Critical).await;

    let mut router = Router::new(budget);
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new(
            "planner-agent",
            &[Role::Planner, Role::Worker],
            true,
        )),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    });

    let task = task_with_role(Role::Planner);
    let chosen = router.select(&task).await.unwrap();
    assert_eq!(chosen.id().0, "planner-agent");
}

#[tokio::test]
async fn select_warning_tier_prefers_cheaper_agent() {
    // Both providers in Warning tier; sort key should pick the cheaper agent.
    let cap = Cap {
        monthly_micro_dollars: 100,
        session_micro_dollars: 1_000_000_000,
    };
    let mut caps = HashMap::new();
    caps.insert(Provider::ClaudeCode, cap);
    caps.insert(Provider::Codex, cap);
    let budget = Arc::new(Budget::new(caps));

    drain_to_tier(&budget, Provider::ClaudeCode, cap, BudgetTier::Warning).await;
    drain_to_tier(&budget, Provider::Codex, cap, BudgetTier::Warning).await;
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Warning
    );
    assert_eq!(
        budget.current_tier(Provider::Codex).await.unwrap(),
        BudgetTier::Warning
    );

    let mut router = Router::new(budget);
    // Expensive agent — even though its id sorts earlier alphabetically.
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("a-expensive", &[Role::Worker], true)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 10_000,
    });
    // Cheap agent — should be picked despite later alphabetical id.
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("z-cheap", &[Role::Worker], true)),
        provider: Provider::Codex,
        estimated_micros_per_task: 100,
    });

    let task = task_with_role(Role::Worker);
    let chosen = router.select(&task).await.unwrap();
    assert_eq!(chosen.id().0, "z-cheap");
}

#[tokio::test]
async fn select_is_deterministic_across_repeated_calls() {
    let budget = ample_budget(&[Provider::ClaudeCode, Provider::Codex, Provider::Ollama]);
    let mut router = Router::new(budget);

    // Three healthy capable agents with the same tier and same cost — only
    // the AgentId tiebreaker should disambiguate. Lexicographic minimum wins.
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("gamma", &[Role::Worker], true)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    });
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("alpha", &[Role::Worker], true)),
        provider: Provider::Codex,
        estimated_micros_per_task: 100,
    });
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("beta", &[Role::Worker], true)),
        provider: Provider::Ollama,
        estimated_micros_per_task: 100,
    });

    let task = task_with_role(Role::Worker);
    let first = router.select(&task).await.unwrap().id();
    for _ in 0..100 {
        let again = router.select(&task).await.unwrap().id();
        assert_eq!(again, first, "selection must be deterministic");
    }
    assert_eq!(first.0, "alpha", "lexicographic minimum should win the tie");
}

#[tokio::test]
async fn select_returns_nofallbackfortier_when_critical_filter_kills_only_candidate() {
    let cap = Cap {
        monthly_micro_dollars: 100,
        session_micro_dollars: 1_000_000_000,
    };
    let mut caps = HashMap::new();
    caps.insert(Provider::ClaudeCode, cap);
    let budget = Arc::new(Budget::new(caps));

    drain_to_tier(&budget, Provider::ClaudeCode, cap, BudgetTier::Critical).await;

    let mut router = Router::new(budget);
    // Only candidate; advertises Worker only, so Critical-tier filter kills
    // it for a Worker task.
    router.register(AgentRegistration {
        agent: Arc::new(MockAgent::new("only-worker", &[Role::Worker], true)),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 100,
    });

    let task = task_with_role(Role::Worker);
    let err = router.select(&task).await.err().expect("expected error");
    assert!(
        matches!(err, RouterError::NoFallbackForTier(Role::Worker)),
        "expected NoFallbackForTier, got {:?}",
        err
    );
}

#[test]
fn router_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Router>();
}
