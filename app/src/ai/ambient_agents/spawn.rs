//! Stream-based API for spawning and monitoring ambient agents.
#![cfg_attr(target_family = "wasm", expect(dead_code))]

use std::time::Duration;

use futures::Stream;

use super::AmbientAgentTaskId;
use super::{AmbientAgentTask, AmbientAgentTaskState};
use crate::ai::ambient_agents::{SpawnAgentRequest, TaskStatusMessage};

/// How long to poll for the agent to be ready.
/// This should be long enough that the shared session will be joinable.
pub const TASK_STATUS_POLLING_DURATION: Duration = Duration::from_secs(80);

/// Information about a session join link for an ambient agent task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionJoinInfo {
    pub session_link: String,
}

impl SessionJoinInfo {
    pub fn from_task(task: &AmbientAgentTask) -> Option<Self> {
        // Prefer the server-provided session_link when available; it is a better signal
        // that a session-sharing link is ready to be shown to the user.
        if let Some(link) = task.session_link.as_ref().filter(|l| !l.is_empty()) {
            return Some(Self {
                session_link: link.to_string(),
            });
        }

        None
    }
}

/// Lifecycle events during ambient agent startup.
#[derive(Debug)]
pub enum AmbientAgentEvent {
    /// The task was successfully spawned with the given task ID and run ID.
    TaskSpawned {
        task_id: AmbientAgentTaskId,
        run_id: String,
    },
    /// The task state changed.
    StateChanged {
        state: AmbientAgentTaskState,
        status_message: Option<TaskStatusMessage>,
    },
    /// Session started and join information became available.
    SessionStarted { session_join_info: SessionJoinInfo },
    /// Timed out waiting for the agent session to be ready.
    TimedOut,
    /// Agent capacity limit has been reached. This does not block
    /// the task from eventually starting.
    AtCapacity,
}

/// Spawns an ambient agent task and monitors its state.
///
/// The stream completes when:
/// - The task completes (either successfully or with a failure)
/// - The task's shared session is ready to join
/// - The timeout expires (if provided)
/// - An error occurs
///
/// If `timeout` is `None`, there is no timeout.
pub fn spawn_task(
    _request: SpawnAgentRequest,
    _timeout: Option<Duration>,
) -> impl Stream<Item = Result<AmbientAgentEvent, anyhow::Error>> {
    async_stream::stream! {
        yield Err(anyhow::anyhow!("Agent spawning is disabled in OpenWarp"));
    }
}
