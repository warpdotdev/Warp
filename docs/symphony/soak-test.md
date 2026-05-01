## Symphony soak test (PDX-29)

Verifies the Symphony daemon's "always-on" promise: a real Linear project, multiple bounded issues, the daemon left running for 60+ hours, and an audit log that captures every state transition without gaps.

### What's pre-staged

| Artifact | Where |
|---|---|
| Sandbox project | https://linear.app/pdx-software/project/symphony-sandbox-4c92d5ea8f4c |
| Test issues | PDX-93 through PDX-97 (all `agent:claude` labeled, in Backlog) |
| Workflow config | `crates/symphony/examples/WORKFLOW.soak.md` |
| Daemon binary | `target/debug/symphony` (or `cargo build -p symphony --release` for the soak) |
| API key | `LINEAR_API_KEY` in Doppler `helm/dev` |
| Crash reporting | Sentry via `crash_reporting` feature; DSN in Doppler `SENTRY_DSN` if you want it |

### Run command

For a real soak (Friday → Monday), run from a host that won't sleep:

```bash
cd /Users/aloe/Development/warp
cargo build -p symphony --release

# Move test issues from Backlog → Todo to make them eligible
# (do this manually in the Linear UI for PDX-93, 94, 95, 96, 97)

# Run with a long-lived process supervisor
nohup doppler run -- \
  ./target/release/symphony \
    --workflow crates/symphony/examples/WORKFLOW.soak.md \
  > /tmp/symphony-soak.stdout.log 2> /tmp/symphony-soak.stderr.log &

echo $! > /tmp/symphony-soak.pid
```

To stop:

```bash
kill -INT $(cat /tmp/symphony-soak.pid)
# Symphony catches Ctrl-C / SIGINT and drains in-flight agents before exiting.
```

### What to watch

While running:
- `tail -f /tmp/symphony-soak.stderr.log` — orchestrator tracing
- `tail -f ~/.warp/symphony/audit.log | jq .` — JSONL state transitions

After:
- Linear: how many of PDX-93/94/95/96/97 reached Done? Any stuck in Backlog (failures with `handoff_state_on_failure: Backlog`)?
- Sentry: any unrecovered panics? Stalls? (Stalls audit-log but shouldn't crash.)
- Workspaces: each `~/.warp/symphony_workspaces/PDX-XX/` should have exactly one expected output file (HAIKU.md, TWEET.md, etc.).

### Acceptance criteria (Symphony spec PDX-29)

- [ ] Daemon stays up 60+ hours.
- [ ] All 5 tasks attempted; ≥3 reach Done.
- [ ] No production-deploy command ever executed (irrelevant for these prompts but verify via shell history).
- [ ] No diff exceeds `max_diff_lines = 500`.
- [ ] Sentry shows < 5 crash events; none unrecovered.
- [ ] Total spend < $5 (each haiku-class task is ~$0.05; 5 tasks × 1 retry max ≈ $0.50).
- [ ] Audit log has zero gaps — every Claimed has a matching Completed/Failed/Stalled.

### What "failure" looks like, and how to triage

- **Issue stuck in Todo, no Claimed event:** Symphony isn't picking it up. Check `agent:claude` label is present and `tracker.team_key: PDX` is set.
- **Claimed but never Dispatched:** Workspace creation or hook failed. Check tracing for `WorkspaceError`. Inspect `~/.warp/symphony_workspaces/PDX-XX/`.
- **Dispatched then nothing:** Agent CLI crashed silently OR is hung. After 5 minutes the stall reconciler aborts + retries.
- **Repeated retries:** `max_retry_attempts` is hit; audit log shows `RetryGivenUp`. Linear issue stays in Todo (or whatever state it was in) for human triage.
- **Comment posted, no transition:** `tracker.team_key` is wrong or the state name in `handoff_state_on_success` doesn't match a real WorkflowState in that team. Check Linear UI for state names.

### Pre-flight checks (run before starting the soak)

```bash
# 1. Doppler can fetch the Linear key
doppler secrets get LINEAR_API_KEY --plain | head -c 20

# 2. Symphony can reach Linear (one-shot tick run)
doppler run -- ./target/debug/symphony --once \
  --workflow crates/symphony/examples/WORKFLOW.soak.md

# 3. Claude CLI works
claude --version

# 4. Test issues are in Todo (not Backlog)
# — go to https://linear.app/pdx-software/project/symphony-sandbox-4c92d5ea8f4c
#   and bulk-move PDX-93..97 to Todo.
```

### Why these 5 tasks

The soak deliberately avoids:
- **Git operations** — would taint the warp repo.
- **Shell commands** — risk of arbitrary execution.
- **Network calls** — would invalidate the "no network" guarantee.
- **Large diffs** — diff guard would trip.
- **Multi-turn reasoning** — keeps the bill predictable.

Each task is "write one short markdown file." Failure modes are mostly about Symphony's robustness (can it stay up? does the audit log stay consistent?), not about whether Claude can do the task. That's by design.

### After the soak

Whichever follow-up is most painful gets filed as a new issue under PDX-29. Examples that came out of OpenAI's own soak (per the Symphony blog post): inflexible state machines, missing tools, ambiguous `WORKFLOW.md` directives, slow-to-detect stalls.
