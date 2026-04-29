# Strip Notes — Phase 1 reverse-dep map

Generated 2026-04-29 from a clean `cargo check --workspace` baseline on the unmodified fork.

Each "DELETE" target lists every Cargo.toml that currently depends on it. The dir name and TOML package name often differ (`crates/managed_secrets` → `warp_managed_secrets`); the *package name* is what shows up in dependent Cargo.tomls, so that's what we'll be ripping out.

## Crate dir → package name (delete-targets only)

| Dir | Package name |
|---|---|
| `crates/ai` | `ai` |
| `crates/firebase` | `firebase` |
| `crates/warp_server_client` | `warp_server_client` |
| `crates/onboarding` | `onboarding` |
| `crates/computer_use` | `computer_use` |
| `crates/voice_input` | `voice_input` |
| `crates/managed_secrets` | **`warp_managed_secrets`** |
| `crates/managed_secrets_wasm` | `managed_secrets_wasm` |
| `crates/graphql` | **`warp_graphql`** |
| `crates/warp_graphql_schema` | `warp_graphql_schema` |
| `crates/natural_language_detection` | `natural_language_detection` |
| `crates/warp_web_event_bus` | `warp_web_event_bus` |
| `crates/warp_files` | `warp_files` |
| `crates/isolation_platform` | **`warp_isolation_platform`** |

## Dep graph (who depends on what)

```
ai              ← onboarding, app, [workspace]
firebase        ← app, [workspace]
warp_server_client ← app, [workspace]
onboarding      ← app, [workspace]
computer_use    ← ai, app, [workspace]
voice_input     ← app, [workspace]
warp_managed_secrets ← managed_secrets_wasm, app, [workspace]
managed_secrets_wasm ← (none — only the workspace declaration)
warp_graphql    ← ai, warp_server_client, warp_managed_secrets, app, [workspace]
warp_graphql_schema ← warp_graphql, [workspace]
natural_language_detection ← input_classifier (KEEP), app, [workspace]   ⚠️
warp_web_event_bus ← warp_logging (KEEP), app, [workspace]               ⚠️
warp_files      ← app, [workspace]
warp_isolation_platform ← warp_managed_secrets, app, [workspace]
```

Internal sub-graph among delete-targets — order matters; delete leaves first to keep `cargo check` informative:

```
warp_graphql_schema → warp_graphql → {ai, warp_server_client, warp_managed_secrets} → {onboarding, managed_secrets_wasm} → app
warp_isolation_platform → warp_managed_secrets
computer_use → ai
voice_input, firebase, warp_files, warp_web_event_bus → app directly
natural_language_detection → input_classifier (KEEP) + app
```

## Surprises & corrections to STRIP_PLAN.md

### 1. `natural_language_detection` is used by `input_classifier` (a KEEP crate)

`crates/input_classifier/src/util.rs` and `heuristic_classifier/mod.rs` call `check_if_token_has_shell_syntax`, `natural_language_words_score`, and define a `natural_language_detection_heuristic`. NLD itself is **local heuristics**, not cloud-backed.

**Recommendation: KEEP `natural_language_detection`.** Drop it from the Phase 2 deletion list. The plan flagged it as "verify"; verification says it's a local-only utility input_classifier needs.

### 2. `warp_web_event_bus` is used by `warp_logging` (a KEEP crate)

Single call site: `crates/warp_logging/src/wasm.rs:173` — emits a `WarpEvent::ErrorLogged` to the event bus.

**Strategy:** stub or delete the single call site in `warp_logging/src/wasm.rs`, then delete `warp_web_event_bus`. Trivial.

### 3. `warp_files` is NOT Drive-backed

`crates/warp_files/src/lib.rs` uses `remote_server::client::RemoteServerClient` — it's the local file-editing model used for *SSH-style remote terminals*, not Google Drive.

**Recommendation: KEEP `warp_files`** (and KEEP `remote_server`). Drop both from Phase 3 deletions.

### 4. `remote_server` references `app.warp.dev`

Only in `setup.rs` and `install_remote_server.sh` — used as the base URL for downloading the remote-server CLI binary that gets installed on the remote host. Not core terminal logic.

**Strategy:** KEEP the crate. Phase 4 (URL neutering) handles the warp.dev string. Worst case, the remote-server install flow breaks; we can disable it entirely if we don't use that feature.

### 5. `warp_isolation_platform` is for docker/kubernetes sandboxing

Only depended on by `warp_managed_secrets` (DELETE) and `app`. It's the agent-sandbox infrastructure (computer_use runs in a docker/k8s sandbox).

**Recommendation: ADD to deletion list.** Goes in Phase 2 alongside the agent crates, or Phase 3 with managed_secrets — whichever ordering keeps the build informative. Suggest Phase 2.

### 6. `managed_secrets_wasm` has no reverse deps outside the workspace declaration

Means it's safe to delete first with zero ripple, then deal with the rest of the secrets/graphql chain.

## Revised phase ordering (within Phase 2/3)

To keep `cargo check` errors meaningful, delete leaves first:

**Phase 2 — agent removal, in this order:**
1. `voice_input` (only app depends on it)
2. `computer_use` (only ai + app depend on it)
3. `warp_isolation_platform` (only warp_managed_secrets + app — but managed_secrets is going in Phase 3, so we'll have a transient broken state; alternative: defer warp_isolation_platform to Phase 3)
4. `ai` (depended on by onboarding + app — onboarding goes in Phase 3, so app will be broken transiently; that's OK because we'll be editing app/src anyway)

**Drop from Phase 2 deletion list: `natural_language_detection`** (keep it — input_classifier needs it).

**Phase 3 — auth/cloud/onboarding, in this order:**
1. `onboarding` (only app depends on it)
2. `managed_secrets_wasm` (no deps)
3. `warp_managed_secrets` (depended on only by managed_secrets_wasm + app)
4. `warp_isolation_platform` (if deferred from Phase 2)
5. `firebase` (only app)
6. `warp_server_client` (only app, but **sticky** per plan)
7. `warp_graphql` (depended on by ai/warp_server_client/managed_secrets — all gone by now — and app)
8. `warp_graphql_schema` (only warp_graphql, gone)
9. `warp_web_event_bus` (after stubbing the warp_logging call site)

**Drop from Phase 3 deletion list: `warp_files`** (keep it — it's the local files model).

## Workspace edits required (each phase)

For every crate deleted, three places need updating:

1. `Cargo.toml` `[workspace.dependencies]` — remove the `<pkg> = { path = "crates/<dir>" }` line
2. `Cargo.toml` `[workspace] members` / `default-members` — remove `"crates/<dir>"`
3. `rm -rf crates/<dir>`

Plus every dependent Cargo.toml needs the dependency line removed.

## App-level surface to expect

`app/Cargo.toml` is the largest cluster of removals — it depends on every delete-target. Also has feature flags like `ai_resume_button`, `ai_rules`, `ai_context_menu*` — strip those alongside.

## Done criteria for Phase 1

- [x] Every delete-target has its true package name identified
- [x] Every reverse dep mapped
- [x] Surprises documented with revised strategy
- [x] Within-phase ordering refined for leaf-first deletion

Ready for Phase 2.
