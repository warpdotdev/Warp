# Strip-Out Plan

Personal fork of [warpdotdev/warp](https://github.com/warpdotdev/warp). Goal: end up with **just the terminal** — blocks, themes, splits, command palette, font rendering — with **no agent, no auth, no cloud, no telemetry**.

Working name: `warp` (rename later before going public).
License: stays AGPL v3. Personal use only triggers no obligations.

---

## Context for a fresh session

If you're a Claude session reading this without prior context: the user has forked Warp (the agentic terminal) and wants to strip everything that isn't the bare terminal. Warp's agent runs server-side and isn't in this repo anyway — so there's no "swap in my own Claude key" path; the agent has to come *out*, not be replaced. Cloud/auth/agent code is mostly cleanly siloed in specific crates (verified by grepping workspace deps), so this is a tractable strip-out, not a rewrite.

Don't add features. Don't refactor. Just delete and stub until it compiles and runs as a plain terminal.

---

## Phases

Do these in order. After each phase, get to a clean `cargo check` before moving on. Commit per phase.

### Phase 0 — Safety baseline

1. Confirm clean working tree: `git status`
2. Create a working branch: `git checkout -b strip-cloud`
3. Verify it builds **before** changes:
   ```bash
   ./script/bootstrap
   cargo check --workspace
   ```
   If bootstrap fails, fix that first — don't strip anything until you have a known-good baseline. Use the `fix-errors` skill if needed.

### Phase 1 — Identify all reverse deps before deleting anything

For each crate slated for deletion, find what depends on it. Don't delete first and chase compile errors blindly — map the graph first.

```bash
# For each target crate, find Cargo.toml files that depend on it:
for c in ai firebase warp_server_client onboarding computer_use \
         voice_input managed_secrets managed_secrets_wasm \
         graphql warp_graphql_schema; do
  echo "=== $c ==="
  grep -l "^$c " crates/*/Cargo.toml app/Cargo.toml 2>/dev/null
done
```

Write the dep graph to `STRIP_NOTES.md` so you have a checklist of every place that needs touching.

### Phase 2 — Delete the agent

**Crates to remove:**
- `crates/ai/`
- `crates/computer_use/`
- `crates/voice_input/`
- `crates/natural_language_detection/` (only used by AI flows — verify with grep first)

**Workspace cleanup:**
- Remove from `Cargo.toml` `[workspace.dependencies]` block
- Remove from `default-members` if listed

**App-level cleanup:**
- `app/Cargo.toml`: remove `ai.workspace = true`, `computer_use.workspace = true`, `voice_input` (the optional dep + feature), and any `ai_*` feature flags (`ai_resume_button`, `ai_rules`, `ai_context_menu*`)
- `app/src/ai/` and `app/src/ai_assistant/`: delete entire directories
- Grep `app/src` for `use ai::`, `use computer_use::`, `use voice_input::` — strip every call site. Most will be in menu/keybinding/UI registration code; just delete the lines or empty the function bodies.

**Commit:** `phase 2: remove agent crates`

### Phase 3 — Delete auth and cloud client

**Crates to remove:**
- `crates/firebase/`
- `crates/warp_server_client/` (the boundary to Warp's backend)
- `crates/managed_secrets/`
- `crates/managed_secrets_wasm/`
- `crates/graphql/`
- `crates/warp_graphql_schema/`
- `crates/warp_web_event_bus/` (if it's cloud-event-bus only — verify)
- `crates/warp_files/` (if Drive-backed only — verify; otherwise keep)

**Replace with stubs, don't grep-delete blindly.** Many call sites assume "user is logged in" or "fetch config from server." For each, replace with a sensible local default:
- `is_logged_in()` → always `false` (or `true` if that's the path with fewer code branches stripped)
- `current_user()` → a fixed local user struct
- `fetch_remote_settings()` → return `Default::default()`
- Anything that POSTs to Warp's backend → no-op returning `Ok(())`

**Onboarding:**
- `crates/onboarding/` depends on `ai` and probably `firebase`. Delete the whole crate; in `app/src` find where onboarding is launched on first-run and skip straight to the main window.

**Commit:** `phase 3: remove auth, cloud client, onboarding`

### Phase 4 — Kill telemetry

Warp phones home in many places. Two strategies:

1. **Surgical:** find every call to telemetry-emitting functions and replace with no-ops.
2. **Brute force (recommended):** find the central telemetry sender in `crates/warp_core/src/telemetry.rs` (and any others) and stub the network-sending function to `Ok(())`. Local logging can stay.

Also: gut `crates/http_client` callers that point at `warp.dev` / `app.warp.dev` URLs. Search:
```bash
grep -rn "warp.dev\|app.warp.dev\|api.warp.dev" crates app --include="*.rs"
```

Don't break the HTTP client itself — it's used for legit things (fetching package indexes, etc.). Just neuter the Warp-backend URLs.

**Commit:** `phase 4: disable telemetry`

### Phase 5 — UI cleanup

Now the codebase compiles but the UI still has menu items, settings panels, and command palette entries that are dead.

- **Menus:** `app/src/app_menus.rs` — strip Account, Team, AI, Drive entries
- **Settings UI:** strip Account, Team, AI Provider, Drive sections
- **Command palette:** strip commands like "Sign in", "Open AI Panel", "Sync to Drive"
- **Block actions:** strip "Share to Drive," "Send to Agent" buttons
- **Keybindings:** strip bindings that pointed at AI/agent commands
- **Welcome / first-run UI:** ensure it goes straight to terminal, no "create account" step

Use grep to find dead references after each removal. The compiler won't catch UI strings.

**Commit:** `phase 5: ui cleanup`

### Phase 6 — Make it boot to a terminal

Smoke test: `cargo run` (or `./script/run`).

Expected outcome: window opens, drops you into a shell prompt with blocks, no login wall, no agent panel.

Things that may break:
- App startup may try to reach Warp's backend before showing the window — find and stub those calls.
- Settings load may panic if it expects a server-fetched config — provide local defaults.
- Some block features (e.g. "share block") may panic if the cloud client is gone — disable or wrap in `if false`.

Iterate until clean boot.

**Commit:** `phase 6: clean local boot`

### Phase 7 — Polish (optional, skip until daily-driving works)

- Remove now-empty feature flags from `app/Cargo.toml`
- Update README to describe the fork (preserve the original AGPL/MIT LICENSE files — don't touch those)
- Add `NOTICE.md` crediting Warp / Denver Technologies, Inc.
- Add a `CHANGELOG.md` noting the strip-out per AGPL §5 ("modified, removed cloud/auth/agent")
- Disable auto-updater (it points at Warp's release feed)
- Disable crash reporter (likely points at Warp's Sentry)

---

## Crates to KEEP (do not delete)

These are the terminal itself + utilities. Leave them alone:

- `warp_terminal` — PTY, shell, blocks (the actual terminal)
- `warp_core` — core app state
- `warpui`, `warpui_core`, `warpui_extras` — UI framework (MIT)
- `app` — main binary
- `command`, `command-signatures-v2` — command parsing
- `editor`, `vim` — input editing
- `syntax_tree`, `languages`, `lsp` — syntax highlighting
- `settings`, `settings_value`, `settings_value_derive`, `persistence` — settings + storage
- `markdown_parser`, `fuzzy_match`, `sum_tree`, `string-offset` — utilities
- `warp_completer`, `warp_ripgrep` — completions, search
- `ui_components` — shared UI widgets
- `asset_cache`, `asset_macro` — assets
- `http_client` — keep, but neuter Warp URLs (Phase 4)
- `http_server` — keep if the terminal uses it locally; remove if only cloud
- `ipc`, `jsonrpc`, `websocket` — local IPC
- `channel_versions`, `warp_features` — feature flags / channels (may need stubbing if they fetch from server)
- `simple_logger`, `warp_logging`, `warp_util` — logging/utilities
- `prevent_sleep`, `watcher`, `virtual_fs`, `node_runtime` — runtime support
- `app-installation-detection`, `repo_metadata`, `input_classifier` — local helpers
- `field_mask`, `handlebars`, `warp_js`, `warp_cli` — keep unless grep shows they're only cloud-used
- `isolation_platform`, `remote_server` — may be cloud-only; verify
- `integration` — tests; keep but expect many to break

---

## Verification checklist

After each phase, run:

```bash
cargo check --workspace 2>&1 | tee check.log
```

Before declaring done, run the full presubmit:

```bash
./script/presubmit
```

Expect many tests to fail (they'll reference deleted code). Either delete the dead tests or fix them. Don't merge a phase commit until `cargo check` is clean — failing tests are OK to defer, failing compilation is not.

---

## Pitfalls to expect

1. **`warp_server_client` is sticky.** It's pulled into more places than the workspace dep graph suggests because lots of features are gated on "is cloud available." Plan to spend most of Phase 3 here.
2. **`onboarding` may gate first-run.** If you delete it, you need to wire `app` to skip directly to the main window.
3. **Feature flags from a server.** `warp_features` may pull a remote config. Stub `is_enabled()` to either always-false or read from a local `settings.toml`.
4. **Sentry / crash reporter.** Look for `sentry`, `crash`, `panic_handler` in `app/src/main.rs` and disable.
5. **Auto-updater.** Look for `updater`, `Sparkle` (macOS) — disable so it doesn't pull Warp builds over your fork.
6. **License headers.** Don't strip `Copyright (C) Denver Technologies, Inc.` from source files. AGPL §5 requires preserving them.
7. **MIT crates depend on AGPL crates.** `warpui` workspace deps pull AGPL crates (`markdown_parser`, `sum_tree`, `command`, etc.). This doesn't matter for *your* fork (everything's AGPL anyway), but don't try to "extract just the MIT crates" — that's a different, harder project.

---

## Done criteria

- [ ] `cargo check --workspace` clean
- [ ] `cargo run` opens a window with a working shell prompt
- [ ] No login screen, no agent panel, no "sign in" prompts
- [ ] No HTTP requests to `*.warp.dev` on startup (verify with `tcpdump` or Charles)
- [ ] Splits, tabs, command palette, themes still work
- [ ] Settings UI loads without server roundtrip
- [ ] App quits cleanly

---

## After this

If it daily-drives well: rename (Cargo.toml package name, macOS bundle ID, window title strings, repo name), update README, push public.

If you want to add features back (say, your own Claude integration via the Anthropic SDK): start a new branch off the stripped version. The stripped baseline is a much better foundation for that than the original repo would be.
