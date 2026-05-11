# Environment Setup Failure UI for Cloud Mode (V2) — Tech Spec

## Context

Cloud mode V2 (`CloudModeSetupV2` feature flag) shows environment setup commands inline in the terminal via `CloudModeSetupCommandBlock` views. A boolean flag `is_executing_oz_environment_startup_commands` on `BlockList` (`terminal/model/blocks.rs:648`) controls visibility: while true, new command blocks are classified as setup commands and a loading footer ("Setting up environment") replaces the input.

The flag is cleared by two signals — neither fires when setup fails:
- First `AppendedExchange` from the Oz agent (`terminal/view.rs:5299-5310`)
- `HarnessCommandStarted` for non-Oz harnesses (`terminal/view/ambient_agent/view_impl.rs:301-322`)

Meanwhile, `AmbientAgentEvent::StateChanged { state: Failed }` from the server is ignored because the handler at `terminal/view/ambient_agent/model.rs:1170` only processes state changes when the model is in `Status::WaitingForSession` — by the time setup fails, the session has connected and the status is `AgentRunning`.

### Key files
- `app/src/terminal/view/ambient_agent/model.rs` — `AmbientAgentViewModel`, `Status` enum, `AmbientAgentViewModelEvent`
- `app/src/terminal/view/ambient_agent/block/setup_command.rs` — `CloudModeSetupCommandBlock`, detects per-command success/failure via `BlockCompleted`
- `app/src/terminal/view/ambient_agent/block/setup_command_text.rs` — `SetupCommandState`, `SetupCommandGroupId`, collapsible "Running setup commands…" text
- `app/src/terminal/view/ambient_agent/footer.rs` — `render_loading_footer`, `render_error_footer`
- `app/src/terminal/model/blocks.rs:1325-1363` — `is_executing_oz_environment_startup_commands`, `set_is_executing_oz_environment_startup_commands`, `finish_oz_environment_startup_commands_at_block`
- `app/src/terminal/view.rs:26180-26188` — footer rendering slot (loading footer vs input)
- `app/src/ai/agent_conversations_model.rs:1409-1490` — `get_or_async_fetch_task_data`, used by the side panel for task data including `status_message`

See `PRODUCT.md` for the full behavior spec.

## Proposed changes

### 1. Propagate setup command failure from `CloudModeSetupCommandBlock`

`CloudModeSetupCommandBlock` (`setup_command.rs:78-94`) already detects non-zero exit via its `BlockCompleted` subscription. Add a new event variant:

```rust
pub enum CloudModeSetupCommandBlockEvent {
    ToggleBlockVisibility(BlockId),
    SetupCommandFailed(BlockId),  // new
}
```

Emit `SetupCommandFailed` when `BlockCompleted` fires with a non-successful exit code. The `TerminalView` handler for this event (in `maybe_insert_setup_command_blocks`'s subscription at `view_impl.rs:471-477`) will drive the rest of the failure flow.

### 2. New state on `AmbientAgentViewModel`: setup failure with error message

Add a new field to `AmbientAgentViewModel`:

```rust
/// Error message from the server when environment setup failed.
/// Set after detecting a setup command failure and fetching task data.
setup_failure_error: Option<String>,
```

Add a new event variant:

```rust
pub enum AmbientAgentViewModelEvent {
    // ... existing variants ...
    EnvironmentSetupFailed,
}
```

Add a method `record_environment_setup_failure` that:
1. Clears `is_executing_oz_environment_startup_commands` on the blocklist (via the same `finish_oz_environment_startup_commands_at_block` path)
2. Marks the current setup command group as failed (new state on `SetupCommandState`, see §3)
3. Reads the task's `status_message` from `AgentConversationsModel::get_or_async_fetch_task_data`. If the task data is already cached (e.g. via RTC push), sets `setup_failure_error` immediately. If not, the fetch is triggered and a subscription to `AgentConversationsModelEvent::TasksUpdated` retries the read when data arrives.
4. Once the error is available (or if the fetch fails), sets `setup_failure_error` to either the server message or the generic fallback, then emits `EnvironmentSetupFailed`.

### 3. Setup command text: failed state

Add a `failed` field to `SetupCommandState` (`setup_command_text.rs`):

```rust
pub struct SetupCommandState {
    // ... existing fields ...
    failed_group_id: Option<SetupCommandGroupId>,
}
```

Add `mark_group_failed(group_id)` which sets `failed_group_id` and forces `should_expand` to true for that group.

In `CloudModeSetupTextBlock::render` (`setup_command_text.rs:157-222`):
- When the current group is failed, render "Setup failed" in the theme's error color instead of "Running setup commands…" / "Ran setup commands".
- Force the group to be expanded (override any user collapse).

### 4. Error footer rendering

Update `render_error_footer` (`footer.rs:97-117`) to accept a custom header parameter instead of hardcoding "Agent failed":

```rust
pub fn render_error_footer(
    header: &str,
    error_message: &str,
    appearance: &Appearance,
) -> Box<dyn Element> {
```

Update the existing call site to pass `"Agent failed"` to preserve current behavior.

In the footer rendering slot (`view.rs:26180-26188`), add a branch: when `AmbientAgentViewModel` has a `setup_failure_error`, render `render_error_footer("Environment setup failed", &error_message, appearance)` instead of `render_loading_footer`.

The precedence in the footer slot becomes:
1. If setup failure error is set → error footer
2. If `is_cloud_agent_pre_first_exchange` → loading footer
3. Otherwise → normal input

### 5. Wire the `TerminalView` handler

In `TerminalView`'s subscription to `CloudModeSetupCommandBlockEvent` (currently at `view_impl.rs:471-477`), handle the new `SetupCommandFailed` variant:

1. Call `ambient_agent_view_model.record_environment_setup_failure(block_id, ctx)` which drives §2.

In `handle_ambient_agent_event` (`view_impl.rs:95-384`), handle the new `EnvironmentSetupFailed` event:

1. Re-render to show the error footer via `ctx.notify()`.
2. Emit `TerminalViewStateChanged` so pane chrome updates.

### 6. Subscription to `AgentConversationsModel` for deferred error message

In `AmbientAgentViewModel`, when `get_or_async_fetch_task_data` returns `None` (fetch in flight), subscribe to `AgentConversationsModelEvent::TasksUpdated`. On that event, re-check the task data. If the `status_message` is now available, set `setup_failure_error` and emit `EnvironmentSetupFailed`. If the task data shows a terminal failure state but no `status_message`, use the generic fallback.

To avoid leaking the subscription, unsubscribe after the error is resolved or if the task data never arrives (the existing `TaskFetchState` cooldown/retry logic in `AgentConversationsModel` handles the backoff).

## Testing and validation

**Unit tests:**
- `SetupCommandState`: test `mark_group_failed` sets the failed group, forces expansion, and `is_failed` returns the correct state. (Behavior §6, §7, §8)
- `render_error_footer` with custom header: verify the header text parameter flows through. (Behavior §3)

**Integration / manual validation:**
- Trigger a cloud mode run with a setup command that will fail (e.g. an environment with `exit 1` as a setup command). Verify:
  - The "Running setup commands…" text changes to "Setup failed" in red. (Behavior §6)
  - The failed command group auto-expands. (Behavior §7)
  - The error footer appears with "Environment setup failed" and the server error message. (Behavior §3)
  - The loading footer does not persist indefinitely. (Behavior §11)
  - The side panel shows the same error message. (Behavior §12)
- Trigger a clone failure (environment with a nonexistent repo). Verify the same error flow. (Behavior §1)
- Trigger a follow-up run that fails during setup. Verify the same error flow applies to the follow-up group. (Behavior §15)

## Parallelization

Not beneficial — the changes are tightly coupled across a small number of files in the same module hierarchy. A single agent can implement this sequentially.
