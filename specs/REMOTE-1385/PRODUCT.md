# Harness Auth Preflight Checks

## Summary
Cloud agent runs using third-party harnesses (Claude Code, Codex) should run lightweight authentication and billing preflight checks before starting the main agent task. This lets us fail fast with actionable error messages instead of letting the harness start, burn setup time, and then fail opaquely mid-run.

## Behavior

### Preflight sequence
1. After harness environment config is written (auth.json, config files, etc.) but before the main harness CLI command starts, the driver runs up to two preflight commands sequentially: an **authentication check** followed by a **billing check**.
2. If the authentication check fails (non-zero exit code), the task is marked as failed on the server with a `HarnessAuthFailed` error indicating that login credentials are invalid, and the process exits. The billing check is skipped.
3. If the authentication check succeeds but the billing check fails (non-zero exit code), the task is marked as failed on the server with a `HarnessAuthFailed` error indicating that a test API request did not succeed (e.g. expired credits, billing misconfiguration, rate limits), and the process exits.
4. If both checks pass (exit code 0), the driver proceeds to start the main harness CLI command as normal.
5. Each preflight check is optional per-harness. A harness that returns no command for a given check skips it silently.

### Per-harness commands
6. **Claude Code**:
   - Authentication check: `claude auth status --json`
   - Billing check: `claude -p hello`
7. **Codex**:
   - Authentication check: `codex login status`
   - Billing check: `codex exec hello`
8. **Gemini**: No preflight commands. Both checks return `None` and are skipped.
9. Future harnesses opt in by implementing the trait methods; the default is no check.

### Success / failure semantics
10. Success is determined solely by exit code. Exit code 0 = pass; any non-zero exit code = fail.
11. Stdout/stderr from preflight commands is captured but not displayed to the user. It is included in error-level logs for debugging.

### Error reporting
12. When a preflight check fails, the driver sends an `updateAgentTask` mutation to the server with:
    - `taskState: FAILED`
    - A `statusMessage` containing a human-readable description of which check failed and for which harness.
    - `errorCode: AUTHENTICATION_REQUIRED`
13. The two failure modes have distinct user-facing messages:
    - Authentication check failure: "Harness '{harness}' authentication check failed: login credentials are invalid or expired. Verify that the authentication secret configured for this harness is correct."
    - Billing check failure: "Harness '{harness}' billing check failed: a test API request did not succeed. This usually means the API key lacks billing access, credits are exhausted, or the account is misconfigured."
14. After reporting the failure to the server, the driver exits the harness and terminates the process. No retry is attempted.

### Timeouts
15. Each preflight command has a timeout of 30 seconds. If a command does not exit within the timeout, it is treated as a failure with a message indicating the check timed out.

### Interaction with existing checks
16. Preflight checks run after `ThirdPartyHarness::validate()` (which checks CLI installation) and after `build_runner()` (which writes auth config files). This ordering ensures the CLI binary exists and credentials are on disk before we attempt to use them.
17. Preflight checks run before `HarnessRunner::start()`, which launches the main CLI command. The main command is never started if a preflight check fails.

### Idempotency and side effects
18. The authentication check commands (`claude auth status`, `codex login status`) are read-only status queries with no side effects.
19. The billing check commands (`claude -p hello`, `codex exec hello`) send a minimal API request. This consumes a negligible amount of quota/credits. These commands should not produce persistent artifacts (files, git changes, etc.) in the working directory.

### Edge cases
20. If the harness CLI is installed but the auth config file was not written successfully (e.g. disk full), the authentication check will fail with a non-zero exit code. This is the correct behavior — the run cannot proceed without valid auth.
21. If network connectivity is unavailable, both checks will fail. The error message does not distinguish between "bad credentials" and "network unreachable" — the user sees the generic auth/billing failure message. The stderr logs will contain the CLI's own diagnostic output for further debugging.
22. If the harness CLI version is too old to support the preflight command (e.g. `claude auth status` not available), the command will fail with a non-zero exit code. This is acceptable — the subsequent main command would also likely fail.
