# Tech Spec: Agent Policy Hooks for governed autonomous actions

**Issue:** [warpdotdev/warp#9914](https://github.com/warpdotdev/warp/issues/9914)

## Context

Warp already has the primitives needed to enforce agent autonomy locally:

- `app/src/ai/execution_profiles/mod.rs:35` defines `ActionPermission` as `AgentDecides`, `AlwaysAllow`, and `AlwaysAsk`.
- `app/src/ai/execution_profiles/mod.rs:220` stores per-profile permissions for code diffs, file reads, command execution, PTY writes, MCP permissions, and command/file/MCP allowlists.
- `app/src/settings/ai.rs:596` defines default command allowlist patterns and `app/src/settings/ai.rs:605` defines default command denylist patterns such as shells, `curl`, `wget`, `ssh`, and `rm`.
- `app/src/ai/blocklist/permissions.rs:640` gates file reads, `:711` gates file writes, `:735` gates MCP tool calls, `:767` gates MCP resource reads, and `:850` gates command execution.
- `app/src/ai/blocklist/action_model/execute/shell_command.rs:106` asks `BlocklistAIPermissions` whether a requested shell command can autoexecute.
- `app/src/ai/blocklist/action_model/execute/read_files.rs:36` gates agent file reads before execution.
- `app/src/ai/blocklist/action_model/execute/request_file_edits.rs:76` gates auto-applied file edits before execution.
- `app/src/ai/blocklist/action_model/execute/call_mcp_tool.rs:37` gates MCP tool calls before dispatching to the MCP peer.
- `app/src/ai/blocklist/action_model/execute.rs:526` centralizes action execution. It computes `can_auto_execute` at `:545`, maps non-autoexecuted actions to confirmation at `:551`, and then dispatches the selected executor at `:586`.

The key implementation constraint is that `should_autoexecute` is currently synchronous and returns `bool` (`app/src/ai/blocklist/action_model/execute.rs:834`). A real policy hook is asynchronous: it may launch a process or call an HTTP endpoint, has a timeout, may be cancelled, and must emit audit evidence. The implementation should therefore extend the action execution state machine rather than calling a policy process from the synchronous permission helpers.

## Proposed Changes

### 1. Add an agent policy hook module

Create `app/src/ai/policy_hooks/` with:

- `config.rs`: serializable hook configuration and validation.
- `event.rs`: redacted policy event schema.
- `decision.rs`: policy decision types and effective-decision composition.
- `engine.rs`: hook execution, timeout handling, cancellation handling, and audit emission.
- `redaction.rs`: helpers for command/path/MCP argument redaction and size limits.
- `tests.rs`: schema, redaction, decision-composition, and timeout tests.

Suggested core types:

```rust
pub enum AgentPolicyHookTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, AgentPolicyHookSecretValue>,
        working_directory: Option<PathBuf>,
    },
    Http {
        url: String,
        headers: BTreeMap<String, AgentPolicyHookSecretValue>,
    },
}

pub struct AgentPolicyHookConfig {
    pub enabled: bool,
    pub before_action: Vec<AgentPolicyHook>,
    pub timeout_ms: u64,
    pub on_unavailable: AgentPolicyUnavailableDecision,
    pub allow_hook_autoapproval: bool,
}

pub enum AgentPolicyAction {
    ExecuteCommand {
        command: String,
        normalized_command: String,
        is_read_only: Option<bool>,
        is_risky: Option<bool>,
    },
    ReadFiles {
        paths: Vec<PathBuf>,
    },
    WriteFiles {
        paths: Vec<PathBuf>,
        diff_stats: Option<PolicyDiffStats>,
    },
    CallMcpTool {
        server_id: Option<uuid::Uuid>,
        tool_name: String,
        argument_keys: Vec<String>,
    },
    ReadMcpResource {
        server_id: Option<uuid::Uuid>,
        name: String,
        uri: Option<String>,
    },
}

pub enum AgentPolicyDecisionKind {
    Allow,
    Deny,
    Ask,
}
```

The first version should prefer a stable JSON schema over Rust-internal shape. Include `schema_version: "warp.agent_policy_hook.v1"` so external guard tools can validate compatibility.

### 2. Add policy-aware action preflight

Replace the current `bool`-only autoexecute preflight with a richer result:

```rust
pub enum AutoexecuteDecision {
    Allowed {
        warp_reason: Option<String>,
    },
    NeedsConfirmation {
        reason: NotExecutedReason,
        policy_reason: Option<String>,
    },
    Denied {
        policy_result: AgentPolicyHookResult,
    },
    PendingPolicyHook {
        event_id: uuid::Uuid,
    },
}
```

Implementation approach:

1. Keep existing executor-specific `should_autoexecute` logic as the source of the base Warp permission decision.
2. Add an action-to-policy-event builder in `BlocklistAIActionExecutor`.
3. In `try_to_execute_action`, before the final execution dispatch, call a new `PolicyHookEngine::preflight(action, base_decision, ctx)`.
4. If hooks are disabled, return the base decision immediately.
5. If hooks are enabled and no cached decision exists for `(conversation_id, action_id, redacted action payload)`, start an async hook request, store pending state, and return `TryExecuteResult::NotExecuted { reason: NotReady, action }`.
6. When the hook completes, store the decision and notify the action model to retry the pending action.
7. On retry, recompose stored hook results with the current base Warp permission decision and continue, ask, or deny, so permission/profile changes while a hook is pending cannot leave a stale allow cached. A user click while the hook is pending does not pre-confirm a later hook `ask`; the confirmation UI must include the completed hook reason.

This avoids blocking the UI thread or changing every executor to directly await a hook.

### 3. Compose Warp permissions with hook decisions conservatively

Effective decision rules:

1. Existing hard Warp denials are never upgraded by hooks.
2. `deny` from any hook wins.
3. `ask` from any hook wins over `allow`.
4. `allow` from hooks preserves an existing Warp allow.
5. `allow` from hooks may auto-approve a Warp `NeedsConfirmation` only when `allow_hook_autoapproval` is enabled for that hook and the hook is trusted by configuration.
6. Hook timeout, process failure, HTTP failure, or malformed JSON maps to the configured unavailable decision, defaulting to `ask`.

This keeps the first implementation safe by default and still allows teams to opt into stronger policy automation later.

### 4. Make run-to-completion policy-aware

Current permission helpers return early for run-to-completion in several places:

- file reads: `app/src/ai/blocklist/permissions.rs:647`
- file writes: `app/src/ai/blocklist/permissions.rs:724`
- MCP server use: `app/src/ai/blocklist/permissions.rs:808`
- command execution: `app/src/ai/blocklist/permissions.rs:882`

Do not put hook invocation behind these branches. The hook preflight should run after the base Warp permission has been computed and before action execution. The policy event should include `run_until_completion: true` so external policy engines can decide whether to deny or ask.

### 5. Add denial and ask result plumbing

When a hook denies an action:

- Shell commands should return a `RequestCommandOutputResult` variant that tells the model the command was blocked by host policy.
- MCP tool calls should return `CallMCPToolResult::Error` with a policy-blocked message before `reconnecting_peer.call_tool(...)` starts.
- File reads should return `ReadFilesResult::Error` before local or remote file content is read.
- File edits should return a `RequestFileEditsResult` failure/cancelled variant with a policy-blocked reason before diffs are saved.

If new result variants are preferred over reusing existing error strings, add variants with stable, machine-readable policy metadata so the agent can recover reliably.

### 6. Configuration and settings integration

MVP configuration options:

- local profile-scoped hook settings under the Agent Profile model
- managed/team policy can be layered later using the same serialized config
- hook name, transport, command/url, args/headers/env, timeout, unavailable behavior, and autoapproval behavior

Suggested storage strategy:

1. Add optional `agent_policy_hooks` to `AIExecutionProfile`.
2. Keep default disabled so old profiles deserialize unchanged.
3. Persist hook credentials only as environment-variable references such as `{ "env": "WARP_POLICY_TOKEN" }`; do not store raw header, environment, or URL credentials in synced profile JSON.
4. Validate persisted credential-bearing fields even when hooks are disabled, and sanitize unsafe config during `AgentPolicyHookConfig` serialization so inactive profile config cannot be locally or cloud-synced with raw or URL-embedded credentials.
5. Detect URL-embedded credentials without relying only on successful URL parsing, because disabled configs may otherwise be incomplete while still containing raw userinfo in the URL authority.
6. Surface minimal settings UI after the engine exists: enabled toggle, hook list, timeout, unavailable behavior, and latest error.

### 7. Audit events

Add a local JSONL audit writer owned by `policy_hooks::engine`:

- event id
- action id
- conversation id
- timestamp
- action kind
- hook name
- hook decision
- effective decision
- reason
- timeout/error class when applicable
- redaction metadata

Do not include file contents, full env, access tokens, or unbounded MCP argument values. If a hook returns an `external_audit_id`, include it in the local record. On Unix, create the audit directory with private `0700` permissions at creation time and write audit files with private `0600` permissions.

### 8. Stdio hook protocol

MVP stdio protocol:

1. Warp launches the configured command with args.
2. Warp clears the child process environment and passes only explicitly configured environment-variable references resolved from the local host.
3. Warp writes one JSON policy event to stdin and closes stdin.
4. Hook writes one JSON decision to stdout.
5. Warp kills the process on timeout/cancellation.
6. Stderr is captured only for debug logs and truncated/redacted before UI display.

Example request:

```json
{
  "schema_version": "warp.agent_policy_hook.v1",
  "event_id": "018f5b3c-2c6b-7cf0-9e2a-6d3b2f0dd111",
  "conversation_id": "conv_123",
  "action_id": "action_456",
  "action_kind": "execute_command",
  "working_directory": "/repo",
  "run_until_completion": true,
  "warp_permission": {
    "decision": "allow",
    "reason": "RunToCompletion"
  },
  "action": {
    "command": "rm -rf .",
    "normalized_command": "rm -rf .",
    "is_read_only": false,
    "is_risky": true
  }
}
```

Example response:

```json
{
  "schema_version": "warp.agent_policy_hook.v1",
  "decision": "deny",
  "reason": "recursive delete in repository root is blocked",
  "external_audit_id": "audit_789"
}
```

### 9. HTTP hook protocol

If HTTP is included in MVP, use the same JSON body and expect the same JSON response:

- POST to the configured URL.
- Include an idempotency key header derived from `event_id`.
- Require HTTPS except for localhost.
- Reject embedded URL credentials such as `https://user:pass@example.com`; credentials must be supplied through configured header environment-variable references.
- Apply the same timeout and unavailable behavior.
- Redact resolved header credentials in settings, logs, hook errors, and hook-returned reasons.
- Redact both complete configured header values and credential fragments for bearer/basic auth headers when hook responses echo only the token portion.

If this is too much for MVP, defer HTTP and keep the JSON schema transport-independent.

## Testing and Validation

Unit tests:

1. Event builders generate stable schema for shell command, file read, file write, MCP tool, and MCP resource actions.
2. Redaction removes env-like secrets, access-token-like values, URL userinfo/basic-auth command credentials, and MCP argument values while preserving useful keys and counts.
3. Decision composition implements deny-wins, ask-over-allow, and no hard-denial upgrade.
4. Run-to-completion base decisions still pass through policy hook composition.
5. Timeout, malformed JSON, process nonzero exit, and missing executable map to configured unavailable behavior.

Action executor tests:

1. A hook denial for `RequestCommandOutput` returns a policy-blocked command result and does not write to the PTY.
2. A hook denial for `CallMCPTool` returns before `call_tool` is invoked.
3. A hook denial for `ReadFiles` returns before local or remote file content is read.
4. A hook denial for `RequestFileEdits` prevents diff save/application.
5. A hook `ask` decision returns `NeedsConfirmation` with policy reason.
6. A hook `allow` decision preserves existing autoexecution when the base Warp decision is allow.
7. A hook `allow` decision does not autoapprove an existing Warp prompt unless `allow_hook_autoapproval` is enabled.

Integration tests:

1. Configure a test stdio hook that denies `rm -rf .`; ask the Agent to run it; verify no command block starts and the agent receives policy denial.
2. Configure a hook that denies a test MCP tool; verify the MCP peer receives no call.
3. Toggle run-to-completion and verify the same denial still applies.
4. Configure a hook that sleeps past timeout; verify the configured fallback decision applies and an audit event is written.

Manual validation:

1. Enable a local policy hook from an Agent Profile.
2. Run a safe command and verify allow path.
3. Run a denied command and verify UI message, agent result, and audit record.
4. Call a known MCP tool and verify allow/deny behavior.
5. Disable the hook and verify existing Agent Profile behavior is unchanged.

## Risks and Mitigations

**Risk:** Blocking the UI thread while a policy hook runs.
**Mitigation:** Treat hook execution as async preflight state and retry the pending action after completion.

**Risk:** Hook providers exfiltrate sensitive context.
**Mitigation:** Redact by default, send no file contents, cap payload size, require explicit config for any expanded context, and make hook configuration visible.

**Risk:** Users expect "Run until completion" to bypass all prompts.
**Mitigation:** Make the distinction clear: run-to-completion bypasses normal interactive prompts, not configured host policy.

**Risk:** Broad first scope delays shipping.
**Mitigation:** Implement in phases. Phase 1: stdio hooks for shell commands, file writes, and MCP tool calls. Phase 2: file reads, MCP resources, HTTP hooks, team-managed policy, and cloud agents.

**Risk:** Third-party CLI agents appear governed when they are not.
**Mitigation:** Document that only Warp-owned Agent/MCP action surfaces are governed. CLI agents need their own native hooks or must route actions through Warp-owned MCP/Agent surfaces.

## Follow-ups

1. Reuse the schema for Oz cloud agent governance.
2. Add managed team policy distribution through Warp Drive.
3. Add a policy event viewer in Agent history.
4. Add SIEM/webhook export directly from Warp if external hook export is insufficient.
5. Add compatibility docs for external tools that implement the hook contract.
