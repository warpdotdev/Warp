## Symphony — adapted for Helm

> Symphony is OpenAI's open-source spec for orchestrating coding agents from a project-management board (Linear). Helm adopts the spec as the design for `crates/symphony/`, swapping the Codex backend for our existing `ClaudeCodeAgent` (and `CodexAgent`) wrappers.

**Source of truth (always fetch the latest):**
- Article: https://openai.com/index/open-source-codex-orchestration-symphony/
- Repo: https://github.com/openai/symphony (canonical SPEC.md + Elixir reference impl)
- Codex App Server protocol: https://developers.openai.com/codex/app-server/

The OpenAI repo's `SPEC.md` is the normative document. We do **not** vendor it — fetch it on demand. Symphony is intentionally a reference spec, not a maintained library.

## Why Symphony fits Helm

Symphony's central claim — *"every open task gets an agent, agents run continuously, and humans review the results"* — is exactly what Helm's Phase D milestone (M4 — Swarm and durability) calls for. Rather than reinvent, we adopt the spec verbatim and reuse Helm primitives we've already built:

| Symphony component (SPEC.md §) | Helm equivalent | Status |
|---|---|---|
| Workflow Loader (§5) | new `crates/symphony/workflow.rs` | TODO (PDX-24) |
| Config Layer (§6) | new `crates/symphony/config.rs` | TODO (PDX-24) |
| Issue Tracker Client (§11) | direct Linear GraphQL OR existing Linear MCP | TODO (PDX-24) |
| Orchestrator (§7, §8) | new `crates/symphony/orchestrator.rs` (consumes `crates/orchestrator::Agent`) | TODO (PDX-24) |
| Workspace Manager (§9) | new `crates/symphony/workspace.rs` | TODO (PDX-24) |
| Agent Runner (§10) | existing `crates/agents::{ClaudeCodeAgent, CodexAgent}` | DONE (PDX-44, PDX-45) |
| Status Surface (§13.4) | optional TUI; later issue | deferred |
| Logging (§13.1) | reuse `tracing` + Sentry (PDX-78) | DONE |
| Budget tracker (Helm addition) | `crates/orchestrator::Budget` | DONE (PDX-38) |
| Concurrency caps (§8.3) | wires Budget+caps in dispatcher | TODO (PDX-27) |
| Retry/backoff/stall (§8.4, §8.5) | TODO + Helm-specific diff-size + test-deletion + audit log | TODO (PDX-28) |
| Triggers (cron, GitHub webhook, Slack) | optional `server.port` extension (§13.7) | TODO (PDX-26) |
| `linear_graphql` dynamic tool (§10.5) | exposed to agent without leaking API token | TODO (sub of PDX-24) |
| 24/7 soak test (§3 "always-on" claim) | sandbox Linear project, weekend run | TODO (PDX-29) |

## Helm-specific divergences from the spec

1. **Backend = Claude CLI streaming, not `codex app-server`.** Spec section 10.1 says `codex.command` defaults to `codex app-server` and speaks JSON-RPC over stdio. Helm's `ClaudeCodeAgent` wraps `claude --print --output-format stream-json --verbose` instead — line-delimited NDJSON, equivalent logical contract. The spec's section 10.4 emitted-events list maps cleanly. CodexAgent uses real `codex exec --json` (no app-server mode in the codex CLI we have).
2. **No git worktree per issue.** Symphony's workspace = a regular directory under `workspace.root`, not a git worktree. Git worktrees are an internal Helm pattern for sub-agent fan-out, not Symphony workspaces.
3. **Backend selection by Linear label.** Issue tagged `agent:claude` → ClaudeCodeAgent; `agent:codex` → CodexAgent; `agent:auto` → router decides via `crates/orchestrator::Router` (PDX-39).
4. **Default `warp_hosted = OFF` for the Helm minimal build.** Symphony itself runs in either mode; this is a Helm fork concern.
5. **Sentry for crashes via `crash_reporting` feature** (PDX-78). Spec says nothing about crash reporting; Helm bolts it on.

## Out of scope for Helm v1

- Cloudflare port (SwarmDO + Workflows + Cron Triggers as Workers). Local Symphony first; cloud later (M3 work, separate issues).
- Multi-tenant / multi-user — single-user fork.
- Web UI dashboard. Optional TUI is deferred.
- Distributed Symphony (`worker.ssh_hosts` extension). Single-host.

## Implementation entry points

When ready to start `crates/symphony/`, point a Claude agent at:
- This README (for Helm-specific context)
- The live spec at https://github.com/openai/symphony/blob/main/SPEC.md (for the canonical contract)
- `crates/orchestrator/src/lib.rs` (for the Agent trait)
- `crates/orchestrator/src/budget.rs` (for budget integration)
- `crates/agents/src/claude_code.rs` (for the existing CLI streaming pattern that becomes the Symphony Agent Runner)
- `crates/doppler/src/lib.rs` (for fetching `LINEAR_API_KEY` from Doppler at startup)

Linear epic: PDX-24. Children: PDX-A9.1 through PDX-A9.5 (TBD).
