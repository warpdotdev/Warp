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
//! 4. Subscribes to [`BlocklistAIHistoryEvent::SetActiveConversation`]. When the
//!    user clicks a child pill that hasn't been joined yet, opens a new
//!    [`Network`] connection to the child's session id so the child's
//!    transcript flows through `handle_shared_session_response_event`.
//! 5. Persists each child connection in `child_sessions`; they're closed when
//!    the model is dropped (parent viewer torn down).
//!
//! Follows the [`crate::terminal::view::ambient_agent::AmbientAgentViewModel`]
//! pattern of a dedicated model initialized after session join, with state
//! polling driven by `ctx.spawn` + `Timer::after`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::FairMutex;
use session_sharing_protocol::common::SessionId;
use warpui::r#async::{SpawnedFutureHandle, Timer};
use warpui::{Entity, EntityId, ModelContext, ModelHandle, SingletonEntity, WeakViewHandle};

use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::ambient_agents::{AmbientAgentTask, AmbientAgentTaskId, AmbientAgentTaskState};
use crate::ai::blocklist::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};
use crate::server::server_api::ai::TaskListFilter;
use crate::server::server_api::ServerApiProvider;
use crate::terminal::event_listener::ChannelEventListener;
use crate::terminal::shared_session::viewer::event_loop::SharedSessionInitialLoadMode;
use crate::terminal::shared_session::viewer::network::Network;
use crate::terminal::{TerminalModel, TerminalView};

/// Maximum number of child runs to request in a single
/// `GET /agent/runs?ancestor_run_id=` page. Orchestrations rarely exceed a
/// handful of children today; the limit is generous enough to absorb future
/// growth without paginating.
const CHILD_DISCOVERY_FETCH_LIMIT: i32 = 100;
/// How often we re-poll the children list while at least one child is still
/// in a non-terminal state. Polling halts once every known child reaches a
/// terminal [`AmbientAgentTaskState`] so we don't burn requests indefinitely.
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Per-child orchestration metadata tracked by the viewer model. Keyed by
/// `AmbientAgentTaskId` in [`OrchestrationViewerModel::children`], so the
/// task id itself is not duplicated on the entry.
struct ChildAgentEntry {
    /// Local id minted via [`BlocklistAIHistoryModel::start_new_child_conversation`].
    conversation_id: AIConversationId,
    /// Session id for the child's shared session, used by
    /// `maybe_join_child_session` to open a [`Network`] when the user
    /// switches to the child for the first time. `None` until the server
    /// reports a session for the child (typically present once execution
    /// has been claimed).
    session_id: Option<SessionId>,
    /// Most recent server-side state observed for this child. We compare
    /// successive polls against this snapshot to decide whether the local
    /// conversation needs a [`BlocklistAIHistoryModel::update_conversation_status`]
    /// call so the pill badge re-renders.
    last_state: AmbientAgentTaskState,
    /// True once we've bound the server-side conversation id (from
    /// [`AmbientAgentTask::conversation_id`]) to the local conversation's
    /// `server_conversation_token`. Pre-binding ensures that when the
    /// child's [`Network`] eventually delivers its `Init` event,
    /// [`crate::ai::blocklist::BlocklistAIController::on_shared_init`] finds
    /// the existing local conversation via
    /// `find_existing_conversation_by_server_token` and reuses it instead
    /// of creating a brand-new conversation (which would surface to the
    /// user as a stranded "new conversation" pane).
    server_token_bound: bool,
    /// Sticky intent: set to `true` the first time the user clicks this
    /// child's pill (via [`Self::maybe_join_child_session`]). Survives the
    /// user navigating away before the child's `session_id` is known, so
    /// the next poll that surfaces a session id can complete the join
    /// without requiring a second click.
    wants_join: bool,
}

/// Owns child discovery, status polling, and child session connections for a
/// shared session viewer of an orchestrated session.
pub struct OrchestrationViewerModel {
    /// The orchestrator's run id. Used as the `ancestor_run_id` filter on
    /// every children fetch.
    parent_task_id: AmbientAgentTaskId,
    /// The terminal view id we're attached to. Used both as the owner of
    /// every child conversation in [`BlocklistAIHistoryModel`] and as the
    /// pivot for finding the orchestrator's local conversation.
    terminal_view_id: EntityId,
    /// Weak handle to the parent's terminal view. Reused for child
    /// [`Network`] connections so their agent response events route through
    /// the same `ai_controller` as the parent (see
    /// `event_loop.rs::AgentResponseEvent`).
    terminal_view: WeakViewHandle<TerminalView>,
    /// Parent terminal model. Reused for child [`Network`] connections; in a
    /// shared session viewer the terminal grid is never displayed under a
    /// child's agent view, so the resize / PTY side effects are acceptable
    /// for V1.
    terminal_model: Arc<FairMutex<TerminalModel>>,
    /// Channel event proxy reused for child sessions. Mirrors how the
    /// parent's [`Network`] is wired up in [`crate::terminal::shared_session::viewer::TerminalManager`].
    channel_event_proxy: ChannelEventListener,
    /// Known child agents indexed by server-side task id.
    children: HashMap<AmbientAgentTaskId, ChildAgentEntry>,
    /// Reverse index from local conversation id to task id. Used by
    /// [`maybe_join_child_session`] to look up the child entry from a
    /// [`BlocklistAIHistoryEvent::SetActiveConversation`] payload.
    child_task_id_by_conversation: HashMap<AIConversationId, AmbientAgentTaskId>,
    /// Per-child [`Network`] connections, keyed by task id. Created lazily on
    /// first switch to the corresponding child pill; persisted for the
    /// lifetime of the viewer so switching away and back doesn't replay
    /// scrollback.
    child_sessions: HashMap<AmbientAgentTaskId, ModelHandle<Network>>,
    /// Handle for the next scheduled poll. Cleared once every child reaches
    /// a terminal state so the timer chain doesn't keep firing.
    polling_handle: Option<SpawnedFutureHandle>,
}

impl Entity for OrchestrationViewerModel {
    type Event = ();
}

impl OrchestrationViewerModel {
    /// Builds a viewer model attached to the given parent shared session.
    ///
    /// Kicks off the initial children fetch and subscribes to history events
    /// so pill clicks can lazily join child sessions.
    pub fn new(
        parent_task_id: AmbientAgentTaskId,
        terminal_view_id: EntityId,
        terminal_view: WeakViewHandle<TerminalView>,
        terminal_model: Arc<FairMutex<TerminalModel>>,
        channel_event_proxy: ChannelEventListener,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        log::info!(
            "[orch-viewer] new: parent_task_id={parent_task_id} terminal_view_id={terminal_view_id:?}"
        );
        // Pill clicks bubble up as `SwitchAgentViewToConversation`, which the
        // pane-header action surface forwards into `swap_active_pane_to_conversation`.
        // That call ends in `BlocklistAIHistoryModel::set_active_conversation_id`
        // emitting `SetActiveConversation`. Subscribing here lets us lazily
        // open the child's WebSocket on first switch.
        let history_handle = BlocklistAIHistoryModel::handle(ctx);
        ctx.subscribe_to_model(&history_handle, |me, event, ctx| {
            if let BlocklistAIHistoryEvent::SetActiveConversation {
                conversation_id,
                terminal_view_id: emitting_view_id,
            } = event
            {
                log::info!(
                    "[orch-viewer] SetActiveConversation: conv={conversation_id:?} \
                     emitting_view_id={emitting_view_id:?} our_view_id={our_view_id:?}",
                    our_view_id = me.terminal_view_id,
                );
                me.maybe_join_child_session(*conversation_id, ctx);
            }
        });

        let mut model = Self {
            parent_task_id,
            terminal_view_id,
            terminal_view,
            terminal_model,
            channel_event_proxy,
            children: HashMap::new(),
            child_task_id_by_conversation: HashMap::new(),
            child_sessions: HashMap::new(),
            polling_handle: None,
        };

        model.fetch_children(ctx);
        model.schedule_next_poll(ctx);
        model
    }

    /// Schedules the next poll iff at least one known child is still in a
    /// non-terminal state. When the model has not yet seen any children we
    /// keep polling so live orchestrations whose `run_agents` call hasn't
    /// settled yet can still surface their first child.
    fn schedule_next_poll(&mut self, ctx: &mut ModelContext<Self>) {
        let should_stop = !self.children.is_empty()
            && self
                .children
                .values()
                .all(|child| child.last_state.is_terminal());
        if should_stop {
            self.polling_handle = None;
            return;
        }

        let handle = ctx.spawn(
            async {
                Timer::after(STATUS_POLL_INTERVAL).await;
            },
            |me, _, ctx| {
                me.fetch_children(ctx);
                me.schedule_next_poll(ctx);
            },
        );
        self.polling_handle = Some(handle);
    }

    /// Issues a `GET /agent/runs?ancestor_run_id={parent_task_id}` request
    /// and routes the response into [`Self::apply_children_fetch`].
    ///
    /// Errors are logged and ignored — the next poll will retry. This is
    /// preferable to surfacing a failure modal because orchestrations that
    /// haven't spawned any children yet legitimately return an empty list,
    /// and transient REST errors should not break the viewer.
    fn fetch_children(&self, ctx: &mut ModelContext<Self>) {
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
            move |me, result, ctx| match result {
                Ok(tasks) => me.apply_children_fetch(tasks, ctx),
                Err(err) => {
                    log::warn!(
                        "OrchestrationViewerModel: failed to fetch children for {parent_task_id}: {err:#}"
                    );
                }
            },
        );
    }

    /// Consumes a children list response, creating new local conversations
    /// for previously-unseen children and updating statuses + session ids
    /// on existing ones.
    fn apply_children_fetch(&mut self, tasks: Vec<AmbientAgentTask>, ctx: &mut ModelContext<Self>) {
        let history_handle = BlocklistAIHistoryModel::handle(ctx);

        log::info!(
            "[orch-viewer] apply_children_fetch: parent_task_id={} tasks_returned={} \
             known_children={}",
            self.parent_task_id,
            tasks.len(),
            self.children.len(),
        );

        for task in tasks {
            log::debug!(
                "[orch-viewer] task: task_id={} state={:?} parent_run_id={:?} \
                 has_session_id={} has_conversation_id={} title={:?}",
                task.task_id,
                task.state,
                task.parent_run_id,
                task.session_id.is_some(),
                task.conversation_id.is_some(),
                task.title,
            );
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
                log::debug!(
                    "[orch-viewer] skipping self task_id={} (matches parent)",
                    task.task_id
                );
                continue;
            }

            let task_id = task.task_id;
            let session_id = task
                .session_id
                .as_deref()
                .and_then(|s| s.parse::<SessionId>().ok());
            // Server-side conversation id for this run. When present we pre-
            // bind it to the local child conversation's
            // `server_conversation_token` so the eventual `Init` event for
            // this child routes back to the same local conversation in
            // `BlocklistAIController::on_shared_init` instead of creating a
            // new one.
            let task_server_conversation_id = task.conversation_id.clone();
            let new_state = task.state.clone();
            let conversation_status = conversation_status_from_state(&new_state);

            if let Some(entry) = self.children.get_mut(&task_id) {
                // Existing child: update status if it changed and fill in
                // session id once it becomes available.
                if entry.last_state != new_state {
                    log::info!(
                        "[orch-viewer] child state update: conv={:?} task={} {:?} -> {:?}",
                        entry.conversation_id,
                        task_id,
                        entry.last_state,
                        new_state,
                    );
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
                // Bind the server conversation token once it's known. The
                // local child conversation was created with an empty token,
                // so without this step `find_existing_conversation_by_server_token`
                // in `on_shared_init` would miss us and create a fresh
                // conversation when the child's Network sends its first
                // event.
                let was_token_bound = entry.server_token_bound;
                if !entry.server_token_bound {
                    if let Some(token) = task_server_conversation_id.as_deref() {
                        let conversation_id = entry.conversation_id;
                        let token_string = token.to_string();
                        history_handle.update(ctx, |history, ctx| {
                            history.set_server_conversation_token_for_conversation(
                                conversation_id,
                                token_string,
                            );
                            ctx.notify();
                        });
                        entry.server_token_bound = true;
                        log::info!(
                            "[orch-viewer] bound server token to existing child: \
                             conv={conversation_id:?} task={task_id} token={token}"
                        );
                    }
                }
                let was_missing_session_id = entry.session_id.is_none();
                if entry.session_id.is_none() {
                    entry.session_id = session_id;
                }
                // Once both the `session_id` and the server-side
                // `conversation_id` are known we can safely open the child's
                // Network: any `Init` that arrives will match our local
                // conversation by `server_conversation_token`, so
                // `on_shared_init` reuses our child conv instead of minting
                // a new "new conversation" alongside it. Either transition
                // (session id arriving, or token getting bound) can be the
                // one that completes the pair; in either case we try the
                // join now if the user has shown intent (clicked the pill)
                // or is currently focused on the child.
                let token_newly_bound = !was_token_bound && entry.server_token_bound;
                let session_id_newly_present = was_missing_session_id && entry.session_id.is_some();
                let is_join_ready = entry.server_token_bound && entry.session_id.is_some();
                if is_join_ready && (token_newly_bound || session_id_newly_present) {
                    let conversation_id = entry.conversation_id;
                    let wants_join = entry.wants_join;
                    let active_id = BlocklistAIHistoryModel::as_ref(ctx)
                        .active_conversation_id(self.terminal_view_id);
                    let is_active = active_id == Some(conversation_id);
                    log::info!(
                        "[orch-viewer] child join ready: conv={conversation_id:?} \
                         task={task_id} session_id={session_id:?} \
                         token_newly_bound={token_newly_bound} \
                         session_id_newly_present={session_id_newly_present} \
                         active_conv={active_id:?} is_active={is_active} \
                         wants_join={wants_join}",
                        session_id = entry.session_id,
                    );
                    if is_active || wants_join {
                        self.maybe_join_child_session(conversation_id, ctx);
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
                log::info!(
                    "[orch-viewer] cannot register new child task={task_id} yet: no parent \
                     conversation found for terminal_view_id={view_id:?}",
                    view_id = self.terminal_view_id,
                );
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

            let token_for_new_child = task_server_conversation_id.clone();
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
                // `is_viewing_shared_session` guard at
                // `task_status_sync_model.rs:146-148`. The pill-click handler
                // also reads this flag (and the parent's, as a defensive
                // fallback) to decide whether to switch agent view in place
                // instead of materializing a hidden local child pane.
                history.set_viewing_shared_session_for_conversation(conversation_id, true);
                if let Some(conversation) = history.conversation_mut(&conversation_id) {
                    conversation.set_task_id(task_id);
                }
                // Pre-bind the server conversation token (when the task
                // already exposes one) so the first `Init` from this child's
                // Network reuses this conversation via
                // `find_existing_conversation_by_server_token` rather than
                // creating a parallel "new conversation".
                if let Some(token) = token_for_new_child.as_deref() {
                    history.set_server_conversation_token_for_conversation(
                        conversation_id,
                        token.to_string(),
                    );
                }
                history.update_conversation_status(
                    terminal_view_id,
                    conversation_id,
                    status_for_initial,
                    ctx,
                );
                conversation_id
            });

            log::info!(
                "[orch-viewer] discovered new child: conv={conversation_id:?} task={task_id} \
                 parent_conv={parent_conversation_id:?} state={new_state:?} \
                 has_session_id={has_session_id} has_server_token={has_server_token} \
                 harness={harness:?}",
                has_session_id = session_id.is_some(),
                has_server_token = task_server_conversation_id.is_some(),
            );

            self.child_task_id_by_conversation
                .insert(conversation_id, task_id);
            let server_token_bound = task_server_conversation_id.is_some();
            self.children.insert(
                task_id,
                ChildAgentEntry {
                    conversation_id,
                    session_id,
                    last_state: new_state,
                    server_token_bound,
                    wants_join: false,
                },
            );
        }

        let dump = self
            .children
            .iter()
            .map(|(tid, e)| {
                format!(
                    "(task={} conv={:?} session_id={:?} state={:?})",
                    tid, e.conversation_id, e.session_id, e.last_state,
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        log::info!(
            "[orch-viewer] children snapshot after fetch (count={count}, joined_sessions={joined}): [{dump}]",
            count = self.children.len(),
            joined = self.child_sessions.len(),
        );
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

    /// Lazily opens a [`Network`] for the child whose conversation just
    /// became active. No-ops when we don't recognise the conversation as a
    /// child, when we've already connected, or when the server hasn't yet
    /// surfaced a session id for the child.
    fn maybe_join_child_session(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(task_id) = self
            .child_task_id_by_conversation
            .get(&conversation_id)
            .copied()
        else {
            log::info!(
                "[orch-viewer] maybe_join_child_session: ignoring conv={conversation_id:?} \
                 (not a known child of parent_task_id={parent})",
                parent = self.parent_task_id,
            );
            return;
        };
        // Record the user's intent to view this child. We do this before the
        // session-id check so that a pill click that races ahead of the
        // server-side session assignment still gets honored: when the next
        // poll surfaces the session id, `apply_children_fetch` consults this
        // flag and finishes the join even if the user has since navigated
        // away from the child.
        if let Some(entry) = self.children.get_mut(&task_id) {
            entry.wants_join = true;
        }
        if self.child_sessions.contains_key(&task_id) {
            log::info!(
                "[orch-viewer] maybe_join_child_session: already joined conv={conversation_id:?} \
                 task={task_id}"
            );
            return;
        }
        let Some(session_id) = self.children.get(&task_id).and_then(|c| c.session_id) else {
            log::info!(
                "[orch-viewer] maybe_join_child_session: no session id yet for conv={conversation_id:?} \
                 task={task_id}; will join once the next poll surfaces it"
            );
            return;
        };
        // Refuse to open the Network until we've bound the server
        // conversation token onto the local child conversation. Otherwise
        // the child's first `Init` event arrives at
        // `BlocklistAIController::on_shared_init` before our local conv has
        // a matching `server_conversation_token`, and a brand-new
        // ("new conversation") conversation gets minted in parallel.
        // `wants_join` is sticky, so the next poll that surfaces
        // `task.conversation_id` will retry through the eager-join in
        // `apply_children_fetch`.
        if !self
            .children
            .get(&task_id)
            .map(|c| c.server_token_bound)
            .unwrap_or(false)
        {
            log::info!(
                "[orch-viewer] maybe_join_child_session: have session id but no server \
                 conversation token yet for conv={conversation_id:?} task={task_id}; \
                 deferring until the next poll surfaces task.conversation_id"
            );
            return;
        }

        log::info!(
            "[orch-viewer] maybe_join_child_session: joining conv={conversation_id:?} \
             task={task_id} session_id={session_id}"
        );

        let terminal_view = self.terminal_view.clone();
        let terminal_model = self.terminal_model.clone();
        let channel_event_proxy = self.channel_event_proxy.clone();
        // Viewers never write to a child session's PTY (read-only path), so
        // the sender side is dropped immediately and the receiver is just a
        // dead channel that satisfies the constructor.
        let (_write_to_pty_events_tx, write_to_pty_events_rx) =
            async_channel::unbounded::<Vec<u8>>();

        let network = ctx.add_model(|ctx| {
            Network::new(
                session_id,
                channel_event_proxy,
                terminal_view,
                terminal_model,
                write_to_pty_events_rx,
                SharedSessionInitialLoadMode::ReplaceFromSessionScrollback,
                ctx,
            )
        });
        self.child_sessions.insert(task_id, network);
        log::info!(
            "[orch-viewer] maybe_join_child_session: child network created task={task_id} \
             total_joined={joined}",
            joined = self.child_sessions.len(),
        );
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
