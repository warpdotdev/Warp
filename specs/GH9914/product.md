# Product Spec: Agent Policy Hooks for governed autonomous actions

**Issue:** [warpdotdev/warp#9914](https://github.com/warpdotdev/warp/issues/9914)
**Figma:** none provided

## Summary

Warp should expose a vendor-neutral Agent Policy Hooks capability that lets a user or team connect an external policy engine before sensitive agent actions run. The policy engine can allow, deny, or require confirmation for proposed shell commands, file reads, file writes, MCP tool calls, and MCP resource reads. Warp remains the enforcement point: if the policy hook denies an action, the action does not execute.

## Problem

Warp already gives users strong local controls through Agent Profiles, command allowlists and denylists, MCP allowlists and denylists, and "Run until completion". These controls are useful for an individual user, but they do not provide a first-class integration point for teams that need deterministic policy enforcement, external approvals, audit exports, and compliance evidence independent of the agent model's reasoning.

Today, third-party guardrails can run beside Warp as MCP servers, project rules, or wrapper CLIs. Those integration points can influence an agent, but they cannot reliably enforce a host-side decision on every Warp-owned agent action before the terminal command, file mutation, or MCP tool call occurs.

## Goals

1. Give users and teams a first-class, host-enforced policy hook before high-impact Warp Agent actions execute.
2. Keep the contract vendor-neutral so tools such as HoldTheGoblin, SupraWall, internal policy engines, SIEM gateways, or approval services can integrate without Warp depending on any one provider.
3. Preserve existing Warp permission semantics when no policy hook is configured.
4. Make policy decisions visible to the user and understandable to the agent.
5. Emit auditable, redacted policy decision records for governed actions.
6. Ensure "Run until completion" does not bypass configured policy hooks.

## Non-goals

1. Building a full policy language inside Warp.
2. Replacing Agent Profiles, command allowlists, command denylists, or MCP allowlists.
3. Governing arbitrary third-party CLI processes that execute inside the terminal without going through Warp's agent action model.
4. Shipping a vendor-specific integration in the first implementation.
5. Designing a complete enterprise admin console in this spec.

## User Experience

### Configure a policy hook

In a personal or managed Agent Profile, a user or team admin can enable an Agent Policy Hook and provide a local command or HTTP endpoint that receives policy events. Example shape:

```json
{
  "agent_policy_hooks": {
    "enabled": true,
    "before_action": [
      {
        "name": "company-agent-guard",
        "transport": "stdio",
        "command": "company-agent-guard",
        "args": ["warp", "before-action"],
        "timeout_ms": 5000,
        "on_unavailable": "ask"
      }
    ]
  }
}
```

The exact storage location can be decided during implementation. The product behavior should be the same whether configuration comes from a local profile, project config, or managed team policy.

### Agent proposes a governed action

When the Agent proposes a governed action, Warp builds a redacted policy event and sends it to the configured hook before execution. The hook returns one of:

1. `allow`: continue with execution if Warp's own permissions also allow it.
2. `deny`: block execution and return a denial result to the agent.
3. `ask`: show the normal user confirmation UI with the hook's reason attached.

### User-visible denial

If a hook denies an action, Warp shows the action as blocked with the hook name and reason. The agent receives a structured result explaining that host policy denied the action, so it can revise its plan instead of retrying blindly.

Example:

```text
Blocked by company-agent-guard: production database commands require approval.
```

### Audit visibility

When hooks are enabled, Warp writes a redacted local audit record for every governed action decision and includes the external hook's returned audit id when provided. Teams can use the hook itself to export to SIEM, webhooks, or approval systems.

## Testable Behavior Invariants

1. If no policy hook is configured, Warp behavior is unchanged.
2. A configured hook runs before these Warp-owned action surfaces execute:
   - shell command execution
   - file reads requested by the agent
   - file write or code diff application
   - MCP tool calls
   - MCP resource reads
3. A hook decision of `deny` prevents the underlying command, file operation, or MCP call from starting.
4. A hook decision of `ask` routes the action through Warp's existing confirmation UI and includes the hook's reason in the UI.
5. A hook decision of `allow` cannot override a hard Warp denial such as protected write paths or a managed policy denial.
6. By default, a hook decision of `allow` only preserves an already-allowed Warp permission decision. Any option that lets a trusted hook auto-approve actions that Warp would otherwise ask for must be explicit and scoped to that hook.
7. "Run until completion" still invokes policy hooks and cannot bypass a hook denial.
8. Hook timeout, crash, malformed output, or unavailable endpoint maps to `ask` by default and can be configured to `deny` by managed policy.
9. Hook payloads and hook child processes do not include file contents, secret values, inherited full environment variables, access tokens, URL-embedded credentials, or unbounded command output by default.
10. Hook payloads include enough metadata for deterministic policy decisions: schema version, action id, conversation id, action type, normalized command or paths, MCP server/tool/resource identity, working directory, active profile id, Warp permission result, and whether auto-approve/run-to-completion is active.
11. Warp records a redacted audit event for every governed decision, including hook name, decision, reason, action id, conversation id, timestamp, and policy event id.
12. The agent receives a structured denial or ask result and can continue planning around it.
13. A user can disable a personal hook from settings unless it is provided by a managed team policy.
14. Hook failures are visible enough to debug without exposing secrets.
15. Third-party CLI agents launched as arbitrary terminal commands are out of scope unless they call back through Warp-owned MCP or Agent surfaces.

## Edge Cases

- **Multiple hooks:** Hooks are evaluated in configured order. The first `deny` wins. If any hook returns `ask` and none deny, the effective decision is `ask`.
- **Parallel actions:** Each action has its own policy event id. Decisions must not leak across actions.
- **Cancellation:** If the user cancels an agent run while a hook is pending, Warp cancels or ignores the pending hook result and does not execute the action.
- **Redacted data:** If a value is redacted, the payload should preserve shape where useful, for example path count or argument key names.
- **Offline operation:** If a remote hook cannot be reached, Warp applies the configured unavailable policy.
- **Remote sessions:** The policy event should identify that the action targets a remote session where Warp has that context, but it should still avoid sending remote file contents.

## Success Criteria

1. A local hook can deny `rm -rf .` before Warp starts the shell command.
2. A local hook can deny an MCP tool call before Warp calls the MCP peer.
3. A local hook can require user confirmation for a code diff touching a protected path.
4. Enabling "Run until completion" does not bypass the hook.
5. A malformed hook response fails into the configured fallback decision.
6. Audit records are emitted for allow, deny, ask, timeout, and malformed-response outcomes.
7. Existing Agent Profile behavior remains unchanged for users without hooks.

## Open Questions

1. Should the first implementation expose configuration only in local Agent Profiles, or also support project and team-managed configuration?
2. Should HTTP hooks be included in the first implementation, or should MVP start with local stdio commands only?
3. Should file reads be governed in MVP, or should MVP focus on shell commands, file writes, and MCP calls first?
4. Should Warp-owned cloud agent runs use the same event schema immediately, or should this spec start with desktop/local agents and extend to cloud agents later?
5. What user-visible wording should distinguish a Warp permission prompt from an external policy `ask` decision?
