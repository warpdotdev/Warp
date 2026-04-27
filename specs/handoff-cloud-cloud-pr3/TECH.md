# Cloud-to-cloud handoff PR 3 tech spec
## Context
PR 3 adds the first user-visible cloud-to-cloud follow-up entrypoint. PR 1 added the disabled `HandoffCloudCloud` flag, typed follow-up API client, and execution-aware task accessors. PR 2 added model-level follow-up orchestration: `AmbientAgentViewModel::submit_cloud_followup` submits a prompt, waits for a fresh session, and emits `FollowupSessionReady`, while `create_cloud_mode_view` already routes that event to `viewer::TerminalManager::attach_followup_session` in `app/src/terminal/view/ambient_agent/mod.rs (69-82)`.
The current tombstone UI is generic. `ConversationEndedTombstoneView` receives only `terminal_view_id` and optional `task_id`, derives display data from `BlocklistAIHistoryModel`, and creates a “Continue locally” action on desktop in `app/src/terminal/view/shared_session/conversation_ended_tombstone_view.rs (139-194)`. Its action enum only supports `ContinueLocally`/`OpenInWarp`, and `render_action_buttons` only renders those existing actions in `app/src/terminal/view/shared_session/conversation_ended_tombstone_view.rs (436-495)`.
The current ambient session-ended path does not insert that tombstone. Generic `TerminalView::on_session_share_ended` can insert a tombstone, but it also performs generic shared-session end cleanup and may make viewer input selectable/read-only in `app/src/terminal/view/shared_session/view_impl.rs (684-745)`. PR 2 intentionally kept `viewer::TerminalManager::ambient_session_ended` narrower: it clears outgoing shared-session writes, records the ended session ID on the ambient model behind `HandoffCloudCloud`, and clears `current_network` without marking the pane finished in `app/src/terminal/shared_session/viewer/terminal_manager.rs (1527-1564)`.
Terminal input submission still assumes an active shared-session network. `Input::submit_viewer_ai_query` freezes the input, collects context attachments, and emits `InputEvent::SendAgentPrompt` in `app/src/terminal/input.rs (12400-12545)`. `TerminalView::handle_input_event` forwards that as `TerminalViewEvent::SendAgentPrompt` in `app/src/terminal/view.rs (19655-19666)`, and the viewer manager sends it to the current `Network` in `app/src/terminal/shared_session/viewer/terminal_manager.rs (1389-1402)`. Between cloud executions there is intentionally no current network, so PR 3 needs a separate follow-up submission route.
Cloud Mode setup-v2 already has UI to show startup progress and errors from the ambient model. `TerminalView::handle_ambient_agent_event` handles `FollowupDispatched` by marking the active conversation in progress and notifying the view in `app/src/terminal/view/ambient_agent/view_impl.rs (133-145)`, and `render_ambient_agent_status_screen` renders loading/error/auth/cancelled states from `AgentProgress` in `app/src/terminal/view/ambient_agent/view_impl.rs (546-603)`. `CloudModeInitialUserQuery` currently renders only the initial spawn request prompt and listens to initial-run events in `app/src/terminal/view/ambient_agent/block/query.rs (18-54)`.
## Goals
Show a cloud “Continue” entrypoint on eligible ambient Cloud Mode tombstones behind `HandoffCloudCloud`.
Keep “Continue locally” available and unchanged when the feature flag is off.
Use the existing terminal input/editor for the follow-up prompt instead of embedding a separate tombstone editor.
Route follow-up submission to `AmbientAgentViewModel::submit_cloud_followup`, not to the ended shared-session `Network`.
Reuse the setup-v2 loading/error UI while waiting for the new execution session.
Render the submitted follow-up prompt optimistically while setup is in progress.
Preserve the same terminal pane, local conversation, stable run/task ID, and hotswap attach path.
## Non-goals
No first-class attachments support for cloud follow-up prompts unless it falls out naturally from the existing input path. It is acceptable for PR 3 to submit text-only follow-ups and leave file attachment support for a follow-up.
No changes to the server follow-up API.
No rollout enablement for `HandoffCloudCloud`.
No details-panel redesign or per-execution history UI.
No change to normal non-ambient shared-session ended behavior.
## Proposed changes
### Ambient execution-ended tombstone insertion
Add a dedicated terminal-view method such as `TerminalView::on_ambient_agent_execution_ended(ctx)`. It should insert or update the conversation-ended tombstone for ambient Cloud Mode without calling the full generic `on_session_share_ended` cleanup. The method should keep the terminal view/model, input, ambient view model, pane configuration, and shareable object alive for follow-up hotswap.
Call this method from `viewer::TerminalManager::ambient_session_ended` after recording `ended_session_id` on the ambient model. Keep the existing `SharedSessionStatus` behavior from PR 2: do not set `FinishedViewer`, do not call `insert_shared_session_ended_banner`, and do not cancel the local conversation solely because the execution ended.
For PR 3, prefer the simplest tombstone policy: append a tombstone for each ended execution. Each tombstone marks a real transcript boundary and avoids needing mutable rich-content replacement APIs in the first UX PR. If product review finds repeated cards too noisy, a later PR can replace the latest tombstone in place.
### Tombstone Continue action
Extend `ConversationEndedTombstoneView` on desktop with an optional cloud-continue button. The view already receives `terminal_view_id` and `task_id`; use those plus `FeatureFlag::HandoffCloudCloud` and AI enablement to decide eligibility. The button should only appear when:
- the feature flag is enabled;
- the tombstone has an ambient `task_id`;
- the terminal view still has an ambient agent model for that run;
- AI is enabled; and
- the app is not wasm.
Add a new action variant such as `ContinueInCloud { terminal_view_id, task_id }` or `ContinueInCloud(AmbientAgentTaskId)`. The action should not submit a prompt directly. It should dispatch a higher-level event/action that lets `TerminalView` focus the existing input and mark the input as being in “cloud follow-up compose” mode.
Keep “Continue locally” visible. If both actions are visible, make “Continue” the primary cloud action and demote “Continue locally” visually or position it second, depending on what `ActionButton` variants support. With `HandoffCloudCloud` disabled, the tombstone should render exactly as it does today.
### Follow-up input mode and submission route
Add a small input/view state that records “the next agent prompt should be submitted as a cloud follow-up.” The state belongs close to `TerminalView` or `Input`, not in the tombstone, because the tombstone should only reveal/focus the existing input. A likely shape is:
- `TerminalView` receives a tombstone action and calls `focus_input_box(ctx)`.
- `TerminalView` tells `Input` to enter a follow-up compose mode, or stores a terminal-view-level pending follow-up mode checked when `InputEvent::SendAgentPrompt` is received.
- The input remains editable even though there is no active network.
When the user submits the prompt in this mode, route to `AmbientAgentViewModel::submit_cloud_followup(prompt, ctx)`. Do not emit `TerminalViewEvent::SendAgentPrompt` to the viewer manager for this prompt, because `current_network` belongs to the ended execution. After submission, clear the input buffer and leave the input in the same loading/frozen state used for shared-session agent prompt submission until the ambient model emits success or failure.
Slash commands like `/fork` and `/fork-and-compact` should keep the existing local behavior from `Input::submit_viewer_ai_query`; the follow-up route should only handle normal non-empty agent prompts.
### Loading UI and optimistic prompt rendering
PR 2 already sets `Status::WaitingForSession { kind: Followup }` and emits `FollowupDispatched`. PR 3 should reuse that to render setup-v2 loading UI while polling. If the current terminal layout only shows the loading screen before a session is joined, make sure the same screen appears between executions after a follow-up is submitted.
Generalize `CloudModeInitialUserQuery` into a reusable submitted-user-query view, or add a sibling `CloudModeFollowupUserQuery`. It should render the submitted follow-up prompt using the same `render_user_query` styling in `app/src/terminal/view/ambient_agent/block/query.rs (40-105)`. Insert it on `FollowupDispatched`, not on `DispatchedAgent`, so initial-run behavior remains unchanged.
The ambient view model may need to expose the pending follow-up prompt for rendering. If PR 2 removed or did not keep that field, add a minimal `pending_followup_prompt()` accessor and clear it when `FollowupSessionReady`, `Failed`, `NeedsGithubAuth`, or `Cancelled` reaches a terminal UI state. If the server rejects the follow-up before accepting it, do not permanently append the optimistic follow-up prompt.
### Error, retry, and state cleanup
API or polling errors should reuse the existing `AmbientAgentViewModelEvent::Failed`, auth, quota, and capacity events so the setup screen can show the same error UI as initial Cloud Mode. The input should become editable again after a failure so the user can retry from the tombstone or input.
If a follow-up is accepted but fails before a session becomes ready, keep the tombstone and local conversation available for retry. The conversation status should be updated to `Error` by the existing ambient event handling, but retry should not allocate a new local conversation.
When `FollowupSessionReady` fires, clear follow-up compose/input state, clear the optimistic pending prompt, and rely on the existing `FollowupSessionReady -> attach_followup_session` hotswap path to attach the new shared session.
### Telemetry
Add telemetry events for cloud follow-up UX usage near the existing tombstone telemetry in `AgentManagementTelemetryEvent`: clicked cloud Continue, submitted cloud follow-up prompt, follow-up session ready, and follow-up setup failed. Keep the event payloads small: stable run/task ID and coarse failure type are sufficient if available.
## End-to-end flow
1. A Cloud Mode execution ends and the viewer manager receives `SessionEnded`.
2. `ambient_session_ended` records the ended session ID and calls the new ambient execution-ended terminal-view method.
3. The terminal view appends a tombstone while keeping the pane/input resumable.
4. The user clicks “Continue” on the tombstone.
5. The terminal focuses the existing input and marks the next normal agent prompt as a cloud follow-up.
6. The user submits a prompt.
7. The input/view routes the prompt to `AmbientAgentViewModel::submit_cloud_followup`.
8. The view inserts an optimistic follow-up user query and shows setup-v2 loading UI.
9. The model submits the follow-up API request and polls for a fresh session.
10. `FollowupSessionReady` reaches `create_cloud_mode_view`, which calls `attach_followup_session`.
11. The viewer manager joins the new shared session in append mode, and new output streams into the same terminal pane.
## Testing and validation
Unit tests for `ConversationEndedTombstoneView` or adjacent rendering logic should cover button visibility with `HandoffCloudCloud` on/off, with and without `task_id`, and AI enabled/disabled. Existing “Continue locally” behavior should remain covered.
Terminal-view or input tests should cover the follow-up compose mode: clicking Continue focuses/reveals input, a normal prompt calls `AmbientAgentViewModel::submit_cloud_followup`, slash-command prompts still use local fork behavior, and the viewer `Network` send path is not invoked while between executions.
Ambient view tests should cover that `FollowupDispatched` inserts/renders an optimistic follow-up prompt separately from `DispatchedAgent`, and that failure clears/re-enables follow-up input state for retry.
Viewer manager tests should cover that ambient `SessionEnded` inserts the tombstone without setting `SharedSessionStatus::FinishedViewer` and without calling the generic shared-session ended path.
Manual validation:
- with `HandoffCloudCloud` disabled, complete a Cloud Mode run and verify the tombstone is unchanged;
- with the flag enabled, complete a Cloud Mode run, click Continue, submit a prompt, verify setup UI appears, and verify a fresh shared session attaches in the same pane;
- repeat once to catch stale session IDs, duplicate tombstones, and subscription leaks;
- verify “Continue locally” still forks locally from the tombstone;
- verify a normal shared-session viewer still becomes read-only/finished when its session ends.
Targeted commands after implementation should include `cargo check -p warp --features handoff_cloud_cloud`, focused ambient model/spawn tests from PR 2, and any new tombstone/input/viewer-manager tests. Before opening or updating the PR, follow repo rules for formatting and clippy; do not use `cargo fmt --all` or file-specific `cargo fmt`.
## Risks and mitigations
### Prompt routed to stale network
The largest correctness risk is accidentally sending the follow-up prompt through `TerminalViewEvent::SendAgentPrompt` to a missing or ended `Network`. Mitigate by making follow-up compose mode intercept submission before the viewer-manager network path.
### Tombstone insertion regresses generic viewer teardown
Generic `on_session_share_ended` has important cleanup for ordinary shared sessions, but it is too broad for resumable ambient executions. Mitigate by adding a dedicated ambient execution-ended method and leaving `shared_session_ended` untouched.
### Optimistic prompt duplication
The follow-up prompt could appear once as optimistic UI and again from replayed shared-session scrollback. Mitigate by only rendering the optimistic prompt while waiting for setup, and clear or allow the shared-session append path to take over once the new session is ready.
### Retry state drift
Failures between API acceptance and session readiness can leave input frozen or the tombstone hidden. Mitigate by centralizing cleanup on ambient model failure/cancel/auth events and ensuring the tombstone remains available.
### Repeated tombstone noise
Appending a card per execution is simple but may be visually noisy. Mitigate by keeping the implementation isolated so a later PR can update the latest tombstone in place without changing follow-up submission mechanics.
## Definition of done
With `HandoffCloudCloud` off, tombstone and shared-session behavior are unchanged.
With the flag on, eligible ambient Cloud Mode tombstones show a cloud Continue action while preserving Continue locally.
Clicking Continue focuses/reveals the existing terminal input and submitting a normal prompt calls `AmbientAgentViewModel::submit_cloud_followup`.
The follow-up prompt does not go through the ended shared-session network.
Setup-v2 loading/error UI appears while the follow-up session is starting.
An optimistic follow-up user query renders during setup without reusing initial-run dispatch UI.
When the new session is ready, the existing `FollowupSessionReady` hotswap path attaches it to the same pane.
Targeted tests and `cargo check -p warp --features handoff_cloud_cloud` pass.
