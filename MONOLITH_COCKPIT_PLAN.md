# Monolith Cockpit MVP Execution Plan

## Repository

- Local fork path: `/Users/master/projects/warp-monolith`
- Branch: `monolith-cockpit-mvp`
- Upstream source: `https://github.com/warpdotdev/warp.git`

## Product Shape

The MVP turns Warp into a Monolith operator cockpit without changing the Monolith API or MCP server yet.

The core hierarchy is:

```text
tenant -> VM/runtime host -> agent runtime -> terminal/git/agent actions
```

Phase 1 leans on Warp's existing terminal panes, Git-aware shell state, agent conversations, Warp Drive workflows, and MCP settings. VM access continues through `gcloud` and SSH.

## Milestones

### M0: Fork and orientation

- Clone Warp OSS into the local fork path.
- Preserve upstream as the update source.
- Map existing MCP settings, left-panel navigation, panes, terminal launch paths, and secure-storage APIs.

### M1: Cockpit shell

- Add a Monolith cockpit tab to Warp's left tool panel.
- Render the tenant > VM > runtime hierarchy.
- Keep the first view local/static so it compiles before wiring live data.

Status: implemented.

### M2: Platform auth

- Add Monolith local settings for API URL and cockpit profile metadata.
- Add a platform-admin API key entry area inside the existing MCP Servers settings page.
- Store the key in Warp secure storage, not in `settings.toml`.

Status: implemented.

### M3: Local inventory

- Load tenant/VM/runtime data from a local cockpit profile file.
- Keep the schema explicit and reviewable.
- Do not expose fleet-wide VM inventory through user-scoped MCP tools.

Status: implemented for local JSON profiles through `monolith.cockpit.profile_path`.

### M4: Operator actions

- Open VM shell panes through generated `gcloud compute ssh ...` commands.
- Open runtime workdir panes on the selected VM.
- Provide launch helpers for logs, git status, deploy, pause, stop, and start.

Status: implemented as cockpit buttons that open new Warp terminal tabs preloaded with `gcloud compute ssh ...` commands.

### M5: Agentic workflows

- Start Warp agent/Codex conversations with selected tenant/VM/runtime context.
- Add guarded workflows for updating agent runtime code and committing changes.
- Keep destructive or fleet-wide mutations behind platform-admin auth and explicit operator action.

### Backlog: tenant fleet chat

- Add a tenant-select action that opens a Warp chat/agent workspace scoped to that tenant.
- Bind chat context to the selected tenant, environment, VMs, runtimes, API URL, and MCP/server authority.
- Ensure chat tools can manage only the selected tenant by default; platform-admin fleet-wide actions require explicit elevation.
- Let the operator ask for fleet summaries, runtime health, SSH sessions, logs, deploys, starts, pauses, restarts, and code changes from that tenant chat.
- Keep staging/prod visible in the chat context and require typed confirmation before production writes.

## Guardrails

- Phase 1 must not require Monolith backend changes.
- API keys are secrets and must use secure storage.
- `settings.toml` may hold API URL and local profile paths, but not credentials.
- Platform-admin authority is required for fleet-wide VM or runtime discovery.
- Tenant/user-scoped MCP users must never see all platform VMs.
