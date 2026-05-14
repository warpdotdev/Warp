//! Drives the orchestration pill bar in shared session viewers.
//!
//! After the viewer joins a parent ambient-agent session, [`OrchestrationViewerModel`]:
//!
//! 1. Calls `GET /agent/runs?ancestor_run_id={task_id}` to discover child agents.
//! 2. Creates a local conversation for each child via [`BlocklistAIHistoryModel`]
//!    marked as `is_viewing_shared_session = true` so [`crate::ai::blocklist::task_status_sync_model::TaskStatusSyncModel`]
//!    does not report viewer-side status transitions back to the server.
//! 3. Polls the children list periodically (~5s) until all reach a terminal
//!    state, updating each child conversation's [`ConversationStatus`] when the
//!    server-side state changes.
//! 4. When a child first reports a `session_id`, emits a
//!    [`crate::terminal::Event::EnsureSharedSessionViewerChildPane`] on the
//!    parent's `TerminalView` so the pane group can materialize a hidden
//!    shared-session viewer pane for that child. Each hidden child pane owns
//!    its own `TerminalView`, `BlocklistAIController`, and viewer-side
//!    `Network`, so child-session traffic never crosses the parent
//!    controller's single-stream state.
//! 5. Pill clicks navigate via `SwapPaneToConversation` (the existing
//!    local-orchestration mechanism), swapping the parent pane for the
//!    hidden child pane.
//!
//! The model itself owns no `Network`s and does not subscribe to history
//! events for navigation; it is purely a children poller + materialization
//! trigger.
use std::collections::HashMap;
use std::time::Duration;

use session_sharing_protocol::common::SessionId;
use warpui::r#async::{SpawnedFutureHandle, Timer};
use warpui::{Entity, EntityId, ModelContext, SingletonEntity, WeakViewHandle};

use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::ambient_agents::{AmbientAgentTask, AmbientAgentTaskId, AmbientAgentTaskState};
use crate::ai::blocklist::history_model::BlocklistAIHistoryEvent;
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::server::server_api::ai::TaskListFilter;
use crate::server::server_api::ServerApiProvider;
use crate::terminal::{Event as TerminalViewEvent, TerminalView};

/// Maximum number of child runs to request in a single
/// `GET /agent/runs?ancestor_run_id=` page. Orchestrations rarely exceed a
/// handful of children today; the limit is generous enough to absorb future
/// growth without paginating.
const CHILD_DISCOVERY_FETCH_LIMIT: i32 = 100;
/// How often we re-poll the children list while at least one child is still
/// in a non-terminal state.
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Slower polling cadence used once every known child has reached a
/// terminal [`AmbientAgentTaskState`]. We don't stop polling entirely
/// because the orchestrator can spawn new children or re-activate existing
/// ones via follow-up messages (including viewer-sent input), and we want
/// those to surface in the pill bar without forcing the user to reload.
/// Polling tightens back to [`STATUS_POLL_INTERVAL`] automatically the next
/// time a fetch surfaces a non-terminal child, and is kicked immediately on
/// any [`BlocklistAIHistoryEvent::AppendedExchange`] for a tracked
/// conversation.
const STATUS_POLL_INTERVAL_IDLE: Duration = Duration::from_secs(30);

/// Per-child orchestration metadata tracked by the viewer model. Keyed by
/// `AmbientAgentTaskId` in [`OrchestrationViewerModel::children`].
struct ChildAgentEntry {
    /// Local id minted via [`BlocklistAIHistoryModel::start_new_child_conversation`].
    conversation_id: AIConversationId,
    /// Session id for the child's shared session. `None` until the server
    /// reports a session for the child (typically present once execution
    /// has been claimed).
    session_id: Option<SessionId>,
    /// Most recent server-side state observed for this child. We compare
    /// successive polls against this snapshot to decide whether the local
    /// conversation needs a [`BlocklistAIHistoryModel::update_conversation_status`]
    /// call so the pill badge re-renders.
    last_state: AmbientAgentTaskState,
    /// True once we've emitted
    /// [`TerminalViewEvent::EnsureSharedSessionViewerChildPane`] for this
    /// child. The pane group's `ensure_shared_session_viewer_child_pane`
    /// is itself idempotent, but we track this locally so re-polls don't
    /// spam the event bus.
    pane_materialization_requested: bool,
}

/// Owns child discovery + status polling for a shared session viewer of an
/// orchestrated session.
pub struct OrchestrationViewerModel {
    /// The orchestrator's run id. Used as the `ancestor_run_id` filter on
    /// every children fetch.
    parent_task_id: AmbientAgentTaskId,
    /// The terminal view id we're attached to. Used as the owner of every
    /// child conversation in [`BlocklistAIHistoryModel`] and as the pivot
    /// for finding the orchestrator's local conversation.
    terminal_view_id: EntityId,
    /// Weak handle to the parent's terminal view. Used to emit
    /// [`TerminalViewEvent::EnsureSharedSessionViewerChildPane`] when a
    /// child first becomes joinable, so the pane group can materialize a
    /// dedicated shared-session pane for it.
    terminal_view: WeakViewHandle<TerminalView>,
    /// Known child agents indexed by server-side task id.
    children: HashMap<AmbientAgentTaskId, ChildAgentEntry>,
    /// Handle for the next scheduled poll. Aborted and replaced by every
    /// [`Self::schedule_next_poll`] call so we never have more than one
    /// timer chain in flight, and cleared while a kick-induced fetch is
    /// in flight — the fetch's response callback schedules a fresh timer
    /// using the freshest child state. The timer chain itself runs for
    /// the lifetime of this model; the model is dropped when the parent
    /// pane's `TerminalManager` is torn down (closing the orchestrator's
    /// shared session).
    polling_handle: Option<SpawnedFutureHandle>,
    /// Monotonic counter incremented before every `fetch_children`
    /// dispatch. The response callback compares the captured generation
    /// against this field and drops stale responses, preventing two
    /// concurrent fetches (typically a timer-fired fetch racing a kick-
    /// induced fetch) from clobbering each other. Without this guard a
    /// late-arriving stale snapshot could revert a child's status from
    /// the freshly-applied state to an older one.
    fetch_generation: u64,
}

impl Entity for OrchestrationViewerModel {
    type Event = ();
}

impl OrchestrationViewerModel {
    /// Builds a viewer model attached to the given parent shared session.
    ///
    /// Kicks off the initial children fetch and schedules the first poll.
    pub fn new(
        parent_task_id: AmbientAgentTaskId,
        terminal_view_id: EntityId,
        terminal_view: WeakViewHandle<TerminalView>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        // Kick polling back to fast cadence (and trigger an immediate
        // fetch) any time a new exchange lands on the orchestrator or one
        // of its tracked children. This catches the case where polling has
        // dropped to idle (all children terminal) and then the viewer (or
        // the agent) sends a follow-up that spawns new children or
        // re-activates existing ones; without the kick the pill bar would
        // not update until the next 30s idle poll.
        ctx.subscribe_to_model(&BlocklistAIHistoryModel::handle(ctx), |me, event, ctx| {
            me.maybe_kick_polling(event, ctx);
        });

        let mut model = Self {
            parent_task_id,
            terminal_view_id,
            terminal_view,
            children: HashMap::new(),
            polling_handle: None,
            fetch_generation: 0,
        };

        // Kick off the recurring poll. `fetch_children`'s response
        // callback schedules the next poll, which in turn fires another
        // fetch, creating a self-perpetuating cycle that adapts its
        // interval to the latest known child state.
        model.fetch_children(ctx);
        model
    }

    /// Schedules the next poll. The interval is [`STATUS_POLL_INTERVAL`]
    /// while at least one known child is still in a non-terminal state (or
    /// before any children have been seen), and drops to
    /// [`STATUS_POLL_INTERVAL_IDLE`] once every known child has reached a
    /// terminal state. Polling never stops on its own — see the field doc
    /// on [`Self::polling_handle`] for the lifecycle.
    fn schedule_next_poll(&mut self, ctx: &mut ModelContext<Self>) {
        // Abort any prior timer before replacing it. `SpawnedFutureHandle`
        // does NOT abort on drop, so without this call the previous
        // timer's continuation could still fire and dispatch an extra
        // `fetch_children`, eventually multiplying into parallel timer
        // chains.
        if let Some(prior) = self.polling_handle.take() {
            prior.abort();
        }

        let all_terminal = !self.children.is_empty()
            && self
                .children
                .values()
                .all(|child| child.last_state.is_terminal());
        let interval = if all_terminal {
            STATUS_POLL_INTERVAL_IDLE
        } else {
            STATUS_POLL_INTERVAL
        };

        let handle = ctx.spawn(
            async move {
                Timer::after(interval).await;
            },
            |me, _, ctx| {
                // The next reschedule is driven by `fetch_children`'s
                // response callback so the interval is picked using the
                // freshest state.
                me.fetch_children(ctx);
            },
        );
        self.polling_handle = Some(handle);
    }

    /// Kicks polling back to fast cadence on `AppendedExchange` for a
    /// conversation we track — but only during the idle→active
    /// transition. While we're already polling at
    /// [`STATUS_POLL_INTERVAL`] (5s) every received exchange would
    /// otherwise drive an extra REST request even though the next
    /// scheduled poll is imminent. Narrowing the kick to idle
    /// transitions keeps the request load bounded for active sessions
    /// while still surfacing newly spawned / re-activated children
    /// within seconds of follow-up input. See the subscription wired up
    /// in [`Self::new`] for context.
    fn maybe_kick_polling(
        &mut self,
        event: &BlocklistAIHistoryEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let BlocklistAIHistoryEvent::AppendedExchange {
            conversation_id, ..
        } = event
        else {
            return;
        };
        // While we're polling at the fast cadence the next poll is
        // already imminent; kicking would just double the request load on
        // every streamed exchange. Skip unless every known child is
        // terminal (we're polling at the 30s idle cadence).
        let all_terminal = !self.children.is_empty()
            && self
                .children
                .values()
                .all(|child| child.last_state.is_terminal());
        if !all_terminal {
            return;
        }
        // `polling_handle = None` is the sentinel that a kick fetch is
        // already in flight (cleared below before dispatching the fetch,
        // re-armed by the fetch's response callback). Skipping here
        // prevents pile-up when several exchanges land in quick
        // succession during the idle→active transition.
        if self.polling_handle.is_none() {
            return;
        }
        let conversation_id = *conversation_id;
        let is_orchestrator = self.find_parent_conversation_id(ctx) == Some(conversation_id);
        let is_tracked_child = self
            .children
            .values()
            .any(|child| child.conversation_id == conversation_id);
        if !is_orchestrator && !is_tracked_child {
            return;
        }
        // Abort the pending timer; `fetch_children`'s response callback
        // will queue a fresh one using the post-fetch state, so a kick
        // landing during the idle (30s) interval correctly tightens back
        // to the fast (5s) interval as soon as the new child surfaces.
        // Aborting (rather than just dropping the handle) is required
        // because `SpawnedFutureHandle` does not cancel on drop.
        if let Some(prior) = self.polling_handle.take() {
            prior.abort();
        }
        self.fetch_children(ctx);
    }

    /// Issues a `GET /agent/runs?ancestor_run_id={parent_task_id}` request
    /// and routes the response into [`Self::apply_children_fetch`].
    ///
    /// Errors are logged and ignored — the next poll will retry. This is
    /// preferable to surfacing a failure modal because orchestrations that
    /// haven't spawned any children yet legitimately return an empty list,
    /// and transient REST errors should not break the viewer.
    fn fetch_children(&mut self, ctx: &mut ModelContext<Self>) {
        // Stamp the generation BEFORE dispatching so any in-flight stale
        // fetch (a timer-fired fetch whose REST request hasn't returned
        // yet, racing this kick) is invalidated when its response
        // callback compares against the freshly-bumped
        // `self.fetch_generation`.
        self.fetch_generation = self.fetch_generation.wrapping_add(1);
        let fetch_generation = self.fetch_generation;

        let ai_client = ServerApiProvider::as_ref(ctx).get_ai_client();
        let filter = TaskListFilter {
            ancestor_run_id: Some(self.parent_task_id.to_string()),
            ..TaskListFilter::default()
        };
        let parent_task_id = self.parent_task_id;

        ctx.spawn(
            async move {
                ai_client
                    .list_ambient_agent_tasks(CHILD_DISCOVERY_FETCH_LIMIT, filter)
                    .await
            },
            move |me, result, ctx| {
                // Drop stale responses: a newer fetch was dispatched while
                // this one was in flight, so applying our snapshot could
                // revert state the newer fetch's response already wrote.
                // Skip rescheduling too; the newer fetch's response
                // callback owns the next reschedule.
                if me.fetch_generation != fetch_generation {
                    return;
                }
                match result {
                    Ok(tasks) => me.apply_children_fetch(tasks, ctx),
                    Err(err) => {
                        log::warn!(
                            "OrchestrationViewerModel: failed to fetch children for {parent_task_id}: {err:#}"
                        );
                    }
                }
                // Always reschedule — even on error so transient network
                // failures don't break the polling loop — using the
                // freshest child state to pick the interval.
                me.schedule_next_poll(ctx);
            },
        );
    }

    /// Consumes a children list response, creating new local conversations
    /// for previously-unseen children and updating statuses + session ids
    /// on existing ones. Requests pane materialization for any child whose
    /// `session_id` is freshly known.
    fn apply_children_fetch(&mut self, tasks: Vec<AmbientAgentTask>, ctx: &mut ModelContext<Self>) {
        let history_handle = BlocklistAIHistoryModel::handle(ctx);

        // Collect materialization requests to dispatch outside the
        // `&mut self.children` borrow.
        let mut to_materialize: Vec<(AIConversationId, SessionId)> = Vec::new();

        for task in tasks {
            // The public API filter is `ancestor_run_id`, which returns every
            // descendant of the parent run. We previously enforced
            // single-level by skipping tasks whose direct `parent_run_id`
            // didn't match us, but locally-spawned children may have an
            // empty `parent_run_id` on the server (e.g. legacy Oz local
            // children) or carry a sibling/sub-orchestrator id. Trust the
            // ancestor filter for membership and let nested grandchildren
            // through; the pill bar already lays them out under their
            // correct parent in spawn order. Only the parent task itself
            // must be skipped to avoid recursing.
            if task.task_id == self.parent_task_id {
                continue;
            }

            let task_id = task.task_id;
            let session_id = task
                .session_id
                .as_deref()
                .and_then(|s| s.parse::<SessionId>().ok());
            let new_state = task.state.clone();
            let conversation_status = conversation_status_from_state(&new_state);

            if let Some(entry) = self.children.get_mut(&task_id) {
                // Existing child: update status if it changed and fill in
                // session id once it becomes available.
                if entry.last_state != new_state {
                    let conversation_id = entry.conversation_id;
                    let terminal_view_id = self.terminal_view_id;
                    let status_for_update = conversation_status.clone();
                    history_handle.update(ctx, |history, ctx| {
                        history.update_conversation_status(
                            terminal_view_id,
                            conversation_id,
                            status_for_update,
                            ctx,
                        );
                    });
                    entry.last_state = new_state;
                }
                let was_missing_session_id = entry.session_id.is_none();
                if entry.session_id.is_none() {
                    entry.session_id = session_id;
                }
                // Queue a materialization request for this child if its
                // `session_id` just became known and we haven't requested
                // it before.
                if was_missing_session_id && entry.session_id.is_some() {
                    let conversation_id = entry.conversation_id;
                    let sid = entry.session_id.expect("session_id checked just above");
                    if !entry.pane_materialization_requested {
                        entry.pane_materialization_requested = true;
                        to_materialize.push((conversation_id, sid));
                    }
                }
                continue;
            }

            // New child: register a local conversation under the
            // orchestrator if we can find it; otherwise wait for the next
            // poll. Without the orchestrator's local id `start_new_child_conversation`
            // would record an empty parent agent id and the pill bar would
            // never link the two.
            let Some(parent_conversation_id) = self.find_parent_conversation_id(ctx) else {
                // Repeats on every poll until the orchestrator's local
                // conversation lands; intentionally silent to avoid spam
                // during the initial join.
                continue;
            };

            let name = if task.title.is_empty() {
                "Agent".to_string()
            } else {
                task.title.clone()
            };
            let harness = task
                .agent_config_snapshot
                .as_ref()
                .and_then(|c| c.harness.as_ref())
                .map(|h| h.harness_type);
            let terminal_view_id = self.terminal_view_id;
            let status_for_initial = conversation_status.clone();

            let conversation_id = history_handle.update(ctx, |history, ctx| {
                let conversation_id = history.start_new_child_conversation(
                    terminal_view_id,
                    name,
                    parent_conversation_id,
                    harness,
                    ctx,
                );
                // Suppress server-side status reporting: see
                // `TaskStatusSyncModel::on_conversation_status_updated`'s
                // `is_viewing_shared_session` guard. This flag is also what
                // the eventual hidden child pane will use to disambiguate
                // viewer-spawned children from local-orchestration ones.
                history.set_viewing_shared_session_for_conversation(conversation_id, true);
                if let Some(conversation) = history.conversation_mut(&conversation_id) {
                    conversation.set_task_id(task_id);
                }
                history.update_conversation_status(
                    terminal_view_id,
                    conversation_id,
                    status_for_initial,
                    ctx,
                );
                conversation_id
            });

            let pane_materialization_requested = session_id.is_some();
            if let Some(sid) = session_id {
                to_materialize.push((conversation_id, sid));
            }
            self.children.insert(
                task_id,
                ChildAgentEntry {
                    conversation_id,
                    session_id,
                    last_state: new_state,
                    pane_materialization_requested,
                },
            );
        }

        // Dispatch materialization events outside the children-borrow.
        for (conversation_id, session_id) in to_materialize {
            self.request_child_pane_materialization(conversation_id, session_id, ctx);
        }
    }

    /// Resolves the orchestrator's local conversation id, defaulting to the
    /// viewer's active conversation. In practice the shared-session viewer
    /// always sets the orchestrator as the active conversation immediately
    /// after the first `Init` event (see
    /// `controller/shared_session.rs::on_shared_init`), so this is the
    /// stable anchor for hanging children off of.
    fn find_parent_conversation_id(&self, ctx: &ModelContext<Self>) -> Option<AIConversationId> {
        BlocklistAIHistoryModel::as_ref(ctx).active_conversation_id(self.terminal_view_id)
    }

    /// Tells the parent's `TerminalView` (and therefore the surrounding
    /// `TerminalPane` / `PaneGroup`) to materialize a hidden shared-session
    /// viewer pane for this child. Idempotent on the pane group side.
    fn request_child_pane_materialization(
        &self,
        conversation_id: AIConversationId,
        session_id: SessionId,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(view) = self.terminal_view.upgrade(ctx) else {
            log::warn!(
                "[orch-viewer] cannot request child pane materialization for conv={conversation_id:?}: \
                 parent terminal view is gone"
            );
            return;
        };
        view.update(ctx, |_view, ctx| {
            ctx.emit(TerminalViewEvent::EnsureSharedSessionViewerChildPane {
                conversation_id,
                session_id,
            });
        });
    }
}

/// Maps a server-side run state to the [`ConversationStatus`] used by the
/// pill bar and the conversation list. Working states (queued/pending/claimed/
/// in-progress) all collapse to [`ConversationStatus::InProgress`] so the
/// pill badge stays in the loading spinner until the run terminates.
fn conversation_status_from_state(state: &AmbientAgentTaskState) -> ConversationStatus {
    match state {
        AmbientAgentTaskState::Queued
        | AmbientAgentTaskState::Pending
        | AmbientAgentTaskState::Claimed
        | AmbientAgentTaskState::InProgress => ConversationStatus::InProgress,
        AmbientAgentTaskState::Succeeded => ConversationStatus::Success,
        AmbientAgentTaskState::Failed | AmbientAgentTaskState::Error => ConversationStatus::Error,
        AmbientAgentTaskState::Blocked => ConversationStatus::Blocked {
            blocked_action: String::new(),
        },
        AmbientAgentTaskState::Cancelled => ConversationStatus::Cancelled,
        // The `Unknown` variant only exists for forward-compatibility with
        // newer server states. Treat it as still working so the pill bar
        // doesn't prematurely commit to a final badge.
        AmbientAgentTaskState::Unknown => ConversationStatus::InProgress,
    }
}

#[cfg(test)]
#[path = "orchestration_viewer_model_tests.rs"]
mod tests;
