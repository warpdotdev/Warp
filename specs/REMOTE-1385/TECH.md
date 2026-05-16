# Harness Auth Preflight Checks — Tech Spec

## Context
See `PRODUCT.md` for user-facing behavior. This document covers the implementation.

### Current state
Third-party harnesses implement the `ThirdPartyHarness` trait (`app/src/ai/agent_sdk/driver/harness/mod.rs:122`). The lifecycle for a third-party harness run is:

1. `ThirdPartyHarness::validate()` — checks CLI is on PATH.
2. `ThirdPartyHarness::build_runner()` — writes config files (auth, trust, system prompt, MCP) and returns a `HarnessRunner`.
3. `HarnessRunner::start()` — creates the external conversation on the server and launches the main CLI command.

These steps are orchestrated in `AgentDriver::run_internal` (`driver.rs:1528`), which calls `setup_harness` → `prepare_harness` → `run_harness`.

Driver errors are classified in `error_classification.rs` into `(AgentTaskState, TaskStatusUpdate)` pairs and reported to the server via `report_driver_error` (`driver.rs:791`). The `PlatformErrorCode` enum (`agent_task.graphqls:87`) already has `AUTHENTICATION_REQUIRED`.

### Relevant files
- `app/src/ai/agent_sdk/driver/harness/mod.rs` — `ThirdPartyHarness` trait, `HarnessRunner` trait
- `app/src/ai/agent_sdk/driver/harness/claude_code.rs` — Claude Code harness implementation
- `app/src/ai/agent_sdk/driver/harness/codex.rs` — Codex harness implementation
- `app/src/ai/agent_sdk/driver/harness/gemini.rs` — Gemini harness implementation
- `app/src/ai/agent_sdk/driver.rs` — `AgentDriver`, `AgentDriverError` enum, `run_internal`, `prepare_harness`, `run_harness`
- `app/src/ai/agent_sdk/driver/error_classification.rs` — maps `AgentDriverError` → `(AgentTaskState, TaskStatusUpdate)`
- `app/src/ai/agent_sdk/driver/terminal.rs` — `TerminalDriver::execute_silent_command` (line 406)

## Proposed changes

### 1. New error variant on `AgentDriverError`

Add a new variant to `AgentDriverError` in `driver.rs`:

```rust
/// Which preflight check failed.
#[derive(Debug, Clone, Copy)]
pub enum HarnessAuthFailureKind {
    /// The harness CLI's login/auth status check exited non-zero.
    LoginFailed,
    /// A lightweight test API request exited non-zero (billing, quota, etc.).
    TestRequestFailed,
}

// In AgentDriverError:
#[error("Harness '{harness}' auth preflight failed")]
HarnessAuthCheckFailed {
    harness: String,
    kind: HarnessAuthFailureKind,
    /// Stderr/stdout captured from the failing command, for logs.
    detail: String,
},
```

### 2. Error classification

Add a match arm in `classify_driver_error` (`error_classification.rs`):

```rust
AgentDriverError::HarnessAuthCheckFailed { harness, kind, detail } => {
    let message = match kind {
        HarnessAuthFailureKind::LoginFailed => format!(
            "Harness '{harness}' authentication check failed: login credentials \
             are invalid or expired. Verify that the authentication secret \
             configured for this harness is correct."
        ),
        HarnessAuthFailureKind::TestRequestFailed => format!(
            "Harness '{harness}' billing check failed: a test API request did not \
             succeed. This usually means the API key lacks billing access, credits \
             are exhausted, or the account is misconfigured."
        ),
    };
    log::error!("Preflight detail for {harness}: {detail}");
    (
        AgentTaskState::Failed,
        TaskStatusUpdate::with_error_code(
            message,
            PlatformErrorCode::AuthenticationRequired,
        ),
    )
},
```

### 3. New trait methods on `ThirdPartyHarness`

Add two default-`None` methods to the `ThirdPartyHarness` trait (`harness/mod.rs`):

```rust
/// Shell command to verify authentication credentials are valid.
/// Exit code 0 = pass; non-zero = fail.
fn auth_check_command(&self) -> Option<String> {
    None
}

/// Shell command to send a cheap test API request to verify billing/quota.
/// Exit code 0 = pass; non-zero = fail.
fn billing_check_command(&self) -> Option<String> {
    None
}
```

### 4. Per-harness implementations

**Claude Code** (`claude_code.rs`):
```rust
fn auth_check_command(&self) -> Option<String> {
    let cli = self.cli_agent().command_prefix();
    Some(format!("{cli} auth status --json"))
}

fn billing_check_command(&self) -> Option<String> {
    let cli = self.cli_agent().command_prefix();
    Some(format!("{cli} -p hello"))
}
```

**Codex** (`codex.rs`):
```rust
fn auth_check_command(&self) -> Option<String> {
    let cli = self.cli_agent().command_prefix();
    Some(format!("{cli} login status"))
}

fn billing_check_command(&self) -> Option<String> {
    let cli = self.cli_agent().command_prefix();
    Some(format!("{cli} exec hello"))
}
```

**Gemini** — inherits defaults (`None` for both). No changes needed.

### 5. Preflight runner in `AgentDriver`

Add a new method on `AgentDriver` and call it from `run_internal`, between `prepare_harness` and `run_harness`. The preflight commands are executed through `TerminalDriver::execute_command` (the same path used by environment setup commands), so each check appears as a collapsible block in the shared session UI. On failure, the driver fetches the resulting block's `stylized_output` via `TerminalDriver::block_snapshot` and stashes the rendered text in the error `detail`, which both server logs and the failure status message consume. This makes it easy for the user to inspect the failure output and for on-call to see the captured stderr.

```rust
/// Timeout for individual preflight commands.
const PREFLIGHT_CHECK_TIMEOUT: Duration = Duration::from_secs(30);

async fn run_preflight_checks(
    harness: &dyn ThirdPartyHarness,
    foreground: &ModelSpawner<Self>,
) -> Result<(), AgentDriverError> {
    let harness_name = harness.cli_agent().command_prefix().to_owned();

    if let Some(cmd) = harness.auth_check_command() {
        Self::run_single_preflight(
            &cmd,
            &harness_name,
            HarnessAuthFailureKind::LoginFailed,
            foreground,
        )
        .await?;
    }

    if let Some(cmd) = harness.billing_check_command() {
        Self::run_single_preflight(
            &cmd,
            &harness_name,
            HarnessAuthFailureKind::TestRequestFailed,
            foreground,
        )
        .await?;
    }

    Ok(())
}

async fn run_single_preflight(
    command: &str,
    harness_name: &str,
    failure_kind: HarnessAuthFailureKind,
    foreground: &ModelSpawner<Self>,
) -> Result<(), AgentDriverError> {
    let cmd = command.to_owned();
    let start_future = foreground
        .spawn(move |me, ctx| {
            me.terminal_driver
                .update(ctx, |driver, ctx| driver.execute_command(&cmd, ctx))
        })
        .await??;

    let command_handle = start_future.await?;

    let exit_code = match command_handle.with_timeout(PREFLIGHT_CHECK_TIMEOUT).await {
        Err(TimeoutError) => {
            return Err(AgentDriverError::HarnessAuthCheckFailed {
                harness: harness_name.to_owned(),
                kind: failure_kind,
                detail: "command timed out".to_owned(),
            });
        }
        Ok(result) => result?,
    };

    if !exit_code.was_successful() {
        return Err(AgentDriverError::HarnessAuthCheckFailed {
            harness: harness_name.to_owned(),
            kind: failure_kind,
            detail: format!("exit code {}", exit_code.value()),
        });
    }

    Ok(())
}
```

### 6. Integration point in `run_internal`

In `run_internal` (`driver.rs`), insert the preflight check call between `prepare_harness` and `run_harness` inside the `HarnessKind::ThirdParty` branch (around line 1773):

```rust
HarnessKind::ThirdParty(harness) => {
    let harness_exit_rx = Self::setup_harness(harness.as_ref(), &foreground).await?;
    let runner = Self::prepare_harness(
        &task.prompt,
        &task.mcp_specs,
        harness.as_ref(),
        &foreground,
    )
    .await?;

    // NEW: Run auth and billing preflight checks before starting
    // the main harness command.
    Self::run_preflight_checks(harness.as_ref(), &foreground).await?;

    // Existing harness execution continues unchanged.
    if let Some(task_id) = task_id_for_refresh {
        // ...
    }
}
```

This placement ensures:
- Config files (auth.json, etc.) are already written by `prepare_harness`.
- The harness CLI binary is validated by `setup_harness`.
- The preflight commands run in the terminal session with the correct env vars (secrets, cloud provider vars).
- On failure, the error propagates up through `run_internal` → `run` → `report_driver_error`, which will send the `updateAgentTask` mutation and terminate the process.

### 7. Treating preflight blocks as setup commands in the viewer

The cloud-mode shared-session viewer wraps environment-setup blocks in a collapsible "Set up environment commands" group. The viewer toggles out of this mode the moment it sees a block whose command is detected as the run's third-party harness CLI (via `TerminalView::block_matches_run_harness` in `app/src/terminal/view/ambient_agent/view_impl.rs`). Because the preflight commands share the harness CLI prefix (e.g. `claude auth status --json` starts with `claude`), the unmodified detector would mis-classify the first preflight command as the harness session start and tear down the setup-commands group prematurely.

To route preflight blocks into the existing setup-commands UI instead:

1. Expose a `pub(crate)` helper `preflight_commands_for(harness: Harness) -> Vec<String>` in `harness/mod.rs` next to `harness_kind`. It reuses `harness_kind` to dispatch and returns the union of `auth_check_command()` and `billing_check_command()` for the matching `ThirdPartyHarness` impl. Returns an empty `Vec` for `Oz`, unsupported harnesses, and harnesses with no preflight commands (e.g. Gemini). This is the single source of truth for the viewer.
2. In `block_matches_run_harness`, after `CLIAgent::detect` resolves a hit, compare the block's command (trimmed) against `preflight_commands_for(selected_harness)` by exact string equality. If any entry matches, return `false`. This keeps the block-list flag set, lets `maybe_insert_setup_command_blocks` append the preflight blocks into the existing setup-commands group, and defers `HarnessCommandStarted` until the actual harness invocation arrives.

String equality is exact and reliable: the driver constructs the preflight command via `format!("{cli} auth status --json", …)`, passes that exact string to `TerminalDriver::execute_command`, and the viewer reads the same string back via `block.command_with_secrets_obfuscated(false)`. The real harness invocations have very different shapes (`claude --session-id <uuid> --dangerously-skip-permissions …`, `codex --dangerously-bypass-approvals-and-sandbox "$(cat '…')"`) and never collide with the short preflight strings.

Adding a new preflight check in the future means adding a new `Some("…")` return in a `ThirdPartyHarness` impl. `preflight_commands_for` picks it up automatically; the viewer needs no edits.

### 8. Server-side changes

No new `PlatformErrorCode` variants are needed. Both failures map to the existing `AUTHENTICATION_REQUIRED` code, differentiated by the human-readable `statusMessage`. The server already handles `AUTHENTICATION_REQUIRED` in task state transitions. The client UI already renders `PlatformErrorCode::AuthenticationRequired` with appropriate styling.

## Testing and validation

### Unit tests
- `error_classification_tests.rs`: Add tests verifying that `HarnessAuthCheckFailed` with `LoginFailed` and `TestRequestFailed` map to `(AgentTaskState::Failed, AUTHENTICATION_REQUIRED)` with the expected messages.
- `harness/mod_tests.rs`: Add tests verifying that `auth_check_command()` and `billing_check_command()` return the expected commands for Claude, Codex, and `None` for Gemini. Also tests that `preflight_commands_for` returns the union of the two for Claude/Codex, and an empty `Vec` for Gemini, Oz, OpenCode (unsupported), and Unknown.

### Manual validation (per PRODUCT.md invariants 1–22)
Covers invariants 2, 3, 4, 6, 7, 10, 12, 13, 14:
- Claude Code with a valid API key → both checks pass, run proceeds normally.
- Claude Code with an invalid API key → auth check fails, task is marked FAILED with "login credentials are invalid" message.
- Claude Code with a valid key but exhausted credits → auth check passes, billing check fails, task marked FAILED with "test API request did not succeed" message.
- Same matrix for Codex.
- Gemini → no preflight checks run, harness proceeds directly to the main command.

Covers invariant 15 (timeout):
- Simulate a hanging command (e.g. network blocklist) → check fails after 30s with timeout detail.

## Parallelization
This feature is small and tightly coupled (trait changes, one new error variant, one new driver method, and error classification). Parallelization is not beneficial — the changes touch overlapping files and are best implemented sequentially.
