//! Integration tests for [`orchestrator::TaskBoundary`].
//!
//! The boundary tracker is a small synchronous primitive — these tests
//! exercise the public API end-to-end without spinning up a runtime.

use std::sync::{Arc, Barrier};
use std::thread;

use orchestrator::{AgentId, BoundaryError, TaskBoundary, TaskId};

#[test]
fn begin_returns_guard_when_no_existing_binding() {
    let bd = TaskBoundary::new();
    let task = TaskId::new();
    let agent = AgentId("a".to_string());

    let guard = bd.begin(task, agent.clone()).expect("first bind succeeds");
    assert_eq!(guard.task_id(), task);
    assert_eq!(guard.agent_id(), &agent);
    assert_eq!(bd.bound_agent(task), Some(agent));
    assert_eq!(bd.in_flight(), 1);
}

#[test]
fn second_begin_with_different_agent_fails_with_already_bound() {
    let bd = TaskBoundary::new();
    let task = TaskId::new();
    let a = AgentId("a".to_string());
    let b = AgentId("b".to_string());

    let _guard = bd.begin(task, a.clone()).unwrap();
    let err = bd
        .begin(task, b.clone())
        .expect_err("mid-task switch must fail");
    match err {
        BoundaryError::AlreadyBound {
            task_id,
            current,
            requested,
        } => {
            assert_eq!(task_id, task);
            assert_eq!(current, a);
            assert_eq!(requested, b);
        }
    }
    // The original binding is untouched by the failed switch.
    assert_eq!(bd.bound_agent(task), Some(a));
    assert_eq!(bd.in_flight(), 1);
}

#[test]
fn second_begin_with_same_agent_also_fails() {
    // Double-begin is treated as a caller bug even when the agent matches.
    let bd = TaskBoundary::new();
    let task = TaskId::new();
    let a = AgentId("a".to_string());

    let _guard = bd.begin(task, a.clone()).unwrap();
    let err = bd
        .begin(task, a.clone())
        .expect_err("double begin must fail");
    assert!(matches!(
        err,
        BoundaryError::AlreadyBound { ref current, ref requested, .. }
            if *current == a && *requested == a
    ));
}

#[test]
fn dropping_guard_releases_binding_and_allows_switch() {
    // After a task completes, switching to a different agent at the
    // boundary is the entire point.
    let bd = TaskBoundary::new();
    let task = TaskId::new();
    let a = AgentId("a".to_string());
    let b = AgentId("b".to_string());

    {
        let _guard = bd.begin(task, a.clone()).unwrap();
        assert_eq!(bd.bound_agent(task), Some(a));
    }
    assert_eq!(bd.bound_agent(task), None);
    assert_eq!(bd.in_flight(), 0);

    let guard = bd
        .begin(task, b.clone())
        .expect("post-boundary rebind succeeds");
    assert_eq!(guard.agent_id(), &b);
    assert_eq!(bd.bound_agent(task), Some(b));
}

#[test]
fn distinct_tasks_can_bind_different_agents_concurrently() {
    let bd = TaskBoundary::new();
    let t1 = TaskId::new();
    let t2 = TaskId::new();
    let a = AgentId("a".to_string());
    let b = AgentId("b".to_string());

    let _g1 = bd.begin(t1, a.clone()).unwrap();
    let _g2 = bd.begin(t2, b.clone()).unwrap();

    assert_eq!(bd.bound_agent(t1), Some(a));
    assert_eq!(bd.bound_agent(t2), Some(b));
    assert_eq!(bd.in_flight(), 2);
}

#[test]
fn cloning_boundary_shares_state() {
    let primary = TaskBoundary::new();
    let secondary = primary.clone();
    let task = TaskId::new();
    let agent = AgentId("ag".to_string());

    let _guard = primary.begin(task, agent.clone()).unwrap();
    assert_eq!(secondary.bound_agent(task), Some(agent));

    let err = secondary
        .begin(task, AgentId("other".to_string()))
        .expect_err("clone observes the primary's binding");
    assert!(matches!(err, BoundaryError::AlreadyBound { .. }));
}

#[test]
fn concurrent_begins_for_same_task_only_one_wins() {
    // Drive many threads at the same TaskId; exactly one should succeed and
    // every other should observe AlreadyBound. The successful agent is
    // recorded as `current` in every losing error.
    const THREADS: usize = 16;
    let bd = TaskBoundary::new();
    let task = TaskId::new();
    let barrier = Arc::new(Barrier::new(THREADS));

    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            let bd = bd.clone();
            let barrier = barrier.clone();
            let agent = AgentId(format!("agent-{i}"));
            thread::spawn(move || {
                barrier.wait();
                bd.begin(task, agent)
            })
        })
        .collect();

    let mut wins = 0;
    let mut losses = 0;
    for h in handles {
        match h.join().unwrap() {
            Ok(_guard) => {
                wins += 1;
            }
            Err(BoundaryError::AlreadyBound { task_id, .. }) => {
                assert_eq!(task_id, task);
                losses += 1;
            }
        }
    }
    assert_eq!(wins, 1, "exactly one bind should win the race");
    assert_eq!(losses, THREADS - 1);
}

#[test]
fn boundary_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TaskBoundary>();
    assert_send_sync::<orchestrator::BoundaryGuard>();
}
