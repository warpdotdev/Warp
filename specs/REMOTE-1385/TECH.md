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

Add a new method on `AgentDriver` and call it from `run_internal`, between `prepare_harness` and `run_harness`:

```rust
/// Timeout for individual preflight commands.
const PREFLIGHT_CHECK_TIMEOUT: Duration = Duration::from_secs(30);

/// Run the authentication and billing preflight checks for a third-party harness.
///
/// Uses `execute_silent_command` so the check commands do not appear in the
/// user-visible block list.
async fn run_preflight_checks(
    harness: &dyn ThirdPartyHarness,
    foreground: &ModelSpawner<Self>,
) -> Result<(), AgentDriverError> {
    let harness_name = harness.cli_agent().command_prefix().to_owned();

    // Authentication check.
    if let Some(cmd) = harness.auth_check_command() {
        log::info!("Running auth check for {harness_name}: {cmd}");
        run_single_preflight(
            &cmd,
            &harness_name,
            HarnessAuthFailureKind::LoginFailed,
            foreground,
        )
        .await?;
    }

    // Billing check.
    if let Some(cmd) = harness.billing_check_command() {
        log::info!("Running billing check for {harness_name}: {cmd}");
        run_single_preflight(
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
    let output = foreground
        .spawn(move |me, ctx| {
            me.terminal_driver
                .as_ref(ctx)
                .execute_silent_command(cmd, ctx)
        })
        .await?
        .with_timeout(PREFLIGHT_CHECK_TIMEOUT)
        .await
        .map_err(|_| AgentDriverError::HarnessAuthCheckFailed {
            harness: harness_name.to_owned(),
            kind: failure_kind,
            detail: "command timed out".to_owned(),
        })?
        .map_err(|_| AgentDriverError::HarnessAuthCheckFailed {
            harness: harness_name.to_owned(),
            kind: failure_kind,
            detail: "failed to execute command".to_owned(),
        })?;

    // Check if the command succeeded. CommandOutput contains the exit code and
    // output text.
    let output_text = output.to_string().unwrap_or_default();
    if !output.was_successful() {
        log::error!(
            "Preflight {failure_kind:?} failed for {harness_name}. Output: {output_text}"
        );
        return Err(AgentDriverError::HarnessAuthCheckFailed {
            harness: harness_name.to_owned(),
            kind: failure_kind,
            detail: output_text,
        });
    }

    log::info!("Preflight {failure_kind:?} passed for {harness_name}");
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

### 7. Server-side changes

No new `PlatformErrorCode` variants are needed. Both failures map to the existing `AUTHENTICATION_REQUIRED` code, differentiated by the human-readable `statusMessage`. The server already handles `AUTHENTICATION_REQUIRED` in task state transitions. The client UI already renders `PlatformErrorCode::AuthenticationRequired` with appropriate styling.

## Testing and validation

### Unit tests
- `error_classification_tests.rs`: Add tests verifying that `HarnessAuthCheckFailed` with `LoginFailed` and `TestRequestFailed` map to `(AgentTaskState::Failed, AUTHENTICATION_REQUIRED)` with the expected messages.
- `harness/mod_tests.rs`: Add tests verifying that `auth_check_command()` and `billing_check_command()` return the expected commands for Claude, Codex, and `None` for Gemini.

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
