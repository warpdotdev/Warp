# danieljohnmorris/warp · personal fork

My personal AGPL fork of [warpdotdev/warp](https://github.com/warpdotdev/warp). The OSS build boots without server auth, the notifications inbox is gone, and agent runs route to local CLIs (`claude`, `codex`) or a local Ollama daemon instead of Warp's cloud.

> [!IMPORTANT]
> Not affiliated with or endorsed by Warp Inc. The upstream README (Warp's product description, contribution flow, support links) lives at [UPSTREAM_README.md](UPSTREAM_README.md).

> [!NOTE]
> This is the `main` branch: purpose 1 only (local-AI bypass). Purpose 2 (ilo-lang testbench) lives on the [`ilo`](https://github.com/danieljohnmorris/warp/tree/ilo) branch.

## Why this fork exists

### 1. A local-AI version of Warp with cloud features stripped

I want to run Warp's terminal and agent UI without signing into Warp's cloud and without paying for Warp's hosted models. The fork swaps Warp's server-mediated AI path for direct calls to local CLIs I already have authenticated (`claude`, `codex`), or to a local Ollama daemon. It removes the auth gate. It hides cloud-only UI: the notifications inbox, the upgrade-required model badges, the "free AI disabled" modal.

This is useful if you already pay for Claude or Codex and don't want a second AI subscription, if you want to keep agent traffic off a third-party server, or if you want to remix Warp's UI without the live cloud dependency.

### 2. A testbench for ilo-lang

Implemented on the [`ilo`](https://github.com/danieljohnmorris/warp/tree/ilo) branch. This branch (`main`) does not include the ilo-lang context injection.

I'm using this fork to test how my agent-optimised programming language, [**ilo-lang**](https://ilo-lang.ai), behaves when wired into a working developer tool. Ilo is token-minimal: programs are written and read primarily by LLMs, so every saved token compounds across millions of agent turns.

This is iteration 3 of integrating ilo into a real agent loop. v1 was Claude Code with the ilo spec pasted manually into the system message. v2 was a Claude skill that loaded the ilo spec on demand. v3 is this fork: ilo context injected at the harness layer of a terminal that runs an agent loop on every keystroke, against whatever codebase I'm in. If ilo expresses agent prompts, tool definitions, and workflows in fewer tokens than Markdown, YAML, or JSON, the difference shows up in day-to-day work, not in a benchmark.

Purpose 1 helps purpose 2: the local-AI bypass lets me run the same ilo system prompt against different LLMs (Claude, Codex, Ollama) and compare token usage and output quality across them. See the [`ilo` branch](https://github.com/danieljohnmorris/warp/tree/ilo) for the implementation.

**Forked from**: [`warpdotdev/warp@00df35b`](https://github.com/warpdotdev/warp/commit/00df35b5dc951b9ed9ac57f873ea0b0484f42ad6), May 2026.
**Sync upstream**: `git fetch upstream master && git merge upstream/master`.

## Configuration (`.env`)

The OSS build reads `~/.warp/.env`, then `./.env` from the cwd, at startup. See [`.env.example`](.env.example) for the full list.

```env
# Boot directly into a logged-in fake-user state. Skips the browser-check / paywall.
# The model-selector menu (footer button) will show the local model list.
WARP_BYPASS_AUTH=1

# Optional: override the default menu selection to a specific provider.
# Supported: claude, codex, ollama.
# WARP_LOCAL_AI=claude

# For Ollama: set the host and model.
# OLLAMA_HOST=http://localhost:11434
# OLLAMA_MODEL=qwen2.5-coder:7b
```

## What's changed vs upstream

- **Auth bypass**. `WARP_BYPASS_AUTH=1` short-circuits `AuthState::initialize` to a Test user, drops the `cfg(skip_login)` gates so `Credentials::Test` is available in OSS builds, and forces `is_any_ai_enabled` so the AI menu, Cmd+I binding, and agent-mode predicate light up.
- **Local-model selector**. When `WARP_BYPASS_AUTH=1` is set, the model-selector menu (footer button) replaces Warp's server-fetched model list with a local set: Claude Sonnet (latest), Opus (latest), Haiku (latest) using the `sonnet`/`opus`/`haiku` CLI aliases so they always resolve to the installed version without version-mismatch errors; GPT-5.5 at low/medium/high reasoning effort; and the Ollama section (see below). Legacy stored model IDs with hardcoded version strings (e.g. `local:claude:claude-sonnet-4-7`) are transparently remapped to the matching alias at parse time.
- **Dynamic Ollama model discovery**. The Ollama section of the model selector is populated from the live Ollama daemon (`GET $OLLAMA_HOST/api/tags`) rather than from hardcoded placeholders. At startup a background task fetches the installed model list and updates the selector; each subsequent menu open triggers a non-blocking refresh (throttled to once per 30 seconds). If Ollama is offline the selector falls back to two static placeholder entries (`qwen2.5-coder:7b`, `llama3.3:70b`). The `$OLLAMA_MODEL` custom entry is always shown last regardless of discovery results.
- **Local-AI harness override**. When `WARP_LOCAL_AI` is set, `harness_kind_with_model()` overrides the harness selection to the matching `ThirdParty` harness. A new `CodexHarness` shells `codex --full-auto`. The agent driver synthesises the `ResolvedHarnessPrompt` locally, so it never calls `ServerApi::resolve_prompt()`.
- **Ollama HTTP provider**. Selecting an Ollama model (or setting `WARP_LOCAL_AI=ollama`) routes the interactive agent panel to `POST $OLLAMA_HOST/api/chat` with streaming JSON. The response stream is mapped to the same `ResponseEvent` protocol as the CLI providers. Requires a running Ollama daemon (`ollama serve`).
- **Tool-call visibility (Slice B)**. Claude is invoked with `--output-format stream-json --verbose`; tool activity is surfaced as inline text annotations (`[tool: Bash] echo hello` / `[result] hello`).
- **Notifications inbox hidden**. `HeaderToolbarItemKind::NotificationsMailbox::is_supported()` returns `false`, removing the inbox icon and dropdown from the header toolbar.
- **`.env` loading**. dotenvy loads `.env` at the top of `run()`, from the cwd and from `~/.warp/.env`, so launching via `open WarpOss.app` (cwd = `/`) still picks up env vars.
- **`/init` cloud-only chips hidden**. The "create cloud-agent environment" prompt in the `/init` project-setup flow is suppressed when `WARP_BYPASS_AUTH` is active. The codebase indexing chip is also suppressed: the indexing pipeline sends code fragments to Warp's GraphQL backend for server-side embedding generation (OpenAI text-small-3 / Voyage models), so without a valid session token every `StoreClient` call would fail silently. Language-support installation still appears as normal.
- **Right-side log viewer sidebar**. A `TextBlock` icon button appears in the header toolbar (right side, alongside Code Review) that toggles a docked log viewer panel. The panel lives at the right edge of the workspace - full window height, resizable by dragging its left edge. It tails `~/Library/Logs/warp-oss.log` (or whichever channel log file is active), streams new lines in real time, supports case-insensitive text filter, and has level-filter chips (All / INFO / WARN / ERROR). Error lines are red, WARN lines are yellow. Help > "View Warp logs" opens the sidebar if it is closed. Escape or the X button closes it. The button is a first-class `HeaderToolbarItemKind::Logs` chip: users can drag it to the left side or remove it via the toolbar configurator (right-click any toolbar button). Default position is right side, after Code Review.
- **jCodeMunch local codebase index**. When `WARP_BYPASS_AUTH=1` is active and `jcodemunch-mcp` (or `uvx`) is on `$PATH`, Warp auto-registers [jCodeMunch](https://github.com/jgravelle/jcodemunch-mcp) in the Warp home MCP config (`~/.warp-oss/.mcp.json`) at startup. jCodeMunch is a local MCP server that walks the repo with tree-sitter, builds a symbol index in `~/.code-index/`, and exposes tools like `get_symbol`, `search_symbols`, and `get_file_outline`. The agent can pull individual functions by symbol ID instead of reading whole files, significantly reducing context usage. Requires `pip install jcodemunch-mcp` or `brew install uv` (for `uvx`). Set `JCODEMUNCH_DISABLED=1` to opt out if you manage the MCP config yourself.
- **Drive menu hidden under bypass**. When `WARP_BYPASS_AUTH=1` is set, the entire "Drive" top menu is omitted. The cloud-only entries (New Personal/Team Workflow/Notebook/Prompt/Env Vars, Search Warp Drive, Open Team Settings, Share Pane, Share Current Session) require Warp cloud auth and produce non-functional UX without it. The local entries that were also in the Drive menu - AI Rules and MCP Servers - remain accessible via the AI menu (unchanged for normal builds).
- **Warp Drive settings page hidden under bypass**. The Settings > "Warp Drive" sidebar entry and its enable/disable toggle are suppressed when `WARP_BYPASS_AUTH=1` is active. The page only controls cloud workflow/notebook/prompt sharing, which is non-functional without auth. Default behaviour (no bypass) is unchanged.
- **Referrals page and user-menu "Invite a friend" hidden under bypass**. The Settings > Referrals sidebar entry and the user-avatar dropdown "Invite a friend" item are both suppressed when `WARP_BYPASS_AUTH=1`. The underlying GraphQL referral-info call 401s against Warp's cloud without a valid session, so the page always showed "Failed to load referral code". The `ShowReferralSettingsPage` action is also a no-op under bypass, preventing navigation via any other code path.
- **User menu: What's new, Feedback, Slack, Upgrade hidden under bypass**. The four cloud-dependent user-menu entries are suppressed when `WARP_BYPASS_AUTH=1`: "What's new" (cloud changelog viewer), "Feedback" (cloud feedback form), "Slack" (community invite), and "Upgrade" (cloud subscription upsell). Settings, Keyboard shortcuts, Documentation, and View Warp logs remain.
- **User menu: email header and Log out hidden under bypass; avatar changed to gear icon**. The `test_user@warp.dev` header row and "Log out" entry are suppressed when `WARP_BYPASS_AUTH=1` - the Test user has no session to log out of. The avatar button in the header also switches from the user-initial circle ("T") to a gear icon, matching the anonymous/logged-out state. The remaining menu is Settings, Keyboard shortcuts, Documentation, View Warp logs.
- **"Store AI conversations in the cloud" toggle hidden under bypass**. In Settings > Privacy, `CloudConversationStorageWidget::should_render` returns `false` when `auth_bypass_enabled()` is true. Under bypass there is no Warp cloud session, so the toggle is misleading - conversations are never stored there regardless of the setting. Secret redaction, network logging, manage-your-data, and the privacy policy link are all kept.
- **Settings > Cloud platform hidden**. The "Cloud platform" sidebar group (Environments + Oz Cloud API Keys subsections) is suppressed when `WARP_BYPASS_AUTH=1`. Both pages gate on `local_ai::auth_bypass_enabled()` via their `should_render()` implementations; the umbrella header is automatically dropped when no subpages are visible. Default behaviour is unchanged.
- **Settings > Teams, Shared blocks hidden under bypass**. When `WARP_BYPASS_AUTH=1` is active, the Teams (cloud team management) and Shared blocks (cloud-shared terminal blocks) sidebar entries are suppressed. Both pages require a live Warp cloud session and produce non-functional UX without one. Warpify (SSH/subshell warpification) is retained as it is local functionality. Default behaviour is unchanged.
- **Settings > Account hidden under bypass**. The "Account" sidebar entry is removed from Settings when `WARP_BYPASS_AUTH=1` is active. The page only shows a fake `test_user@warp.dev` email, a cloud-dependent Settings sync toggle, and a Log out button - all pointless with a synthetic Test user. Opening Settings lands on Appearance instead.
- **Settings > Billing and usage hidden under bypass**. The "Billing and usage" entry is removed from the Settings sidebar, blocked from deep-link navigation (`warp://settings/billing_and_usage`, keyboard binding), and its cloud API calls (`GetRequestLimitInfo`, `GetConversationUsage`, `refresh_workspace_metadata`) are skipped when `WARP_BYPASS_AUTH=1`. The plan/usage/upgrade UI is not meaningful without a Warp cloud account.
- **Cloud codebase-indexing UI hidden under bypass**. In Settings > Code > "Indexing and projects", the "Codebase Indexing" toggle, its description text ("Code is never stored on the server"), the "Index new folders by default" toggle, the "Initialized / indexed folders" header, and the "Index new folder" button are all hidden when `WARP_BYPASS_AUTH=1`. The cloud indexer was already disabled in earlier PRs; this removes the misleading UI surface. The LSP Servers list and "Open project rules" button are preserved - they are local, not cloud-dependent.
- **WarpOss app menu: Invite People and Log out hidden under bypass**. In the leftmost macOS menu bar ("WarpOss > ..."), the "Invite People..." and "Log out" entries are suppressed when `WARP_BYPASS_AUTH=1`. Both require Warp cloud auth: Invite People triggers the GraphQL referral flow (401s without a session) and Log out has no meaningful effect against the synthetic Test user. All other entries (About Warp, Settings, Preferences, Services, Privacy Policy, Debug, Hide/Show, Set Default Terminal, Quit) are unchanged.

## Known limitations

- **Tool visibility (Slice B)**: The panel now shows inline text annotations for Claude tool calls: `[tool: Bash] echo hello` when the agent invokes a tool, and `[result] output` when the tool returns. Claude is invoked with `--output-format stream-json --verbose` so tool activity is visible in real-time. What's still missing: Warp's native tool-block UI (the rich block rendering you'd see in a cloud agent run). That requires mapping each `tool_use` event to an `api::message::ToolCall` protobuf message (Slice C), which is left as follow-up work.
- **Codex tool visibility**: Codex events are still parsed from `codex exec --json`, which only surfaces `item.completed / agent_message` lines. Tool calls from Codex are not annotated.
- **Ollama `/agent` TUI path**: The interactive agent panel routes to Ollama via HTTP and works. The `/agent` TUI command does not have an OllamaHarness yet; when `WARP_LOCAL_AI=ollama` is set and `/agent` is used, it falls back to the selected harness (typically Oz, which requires auth). Follow-up work.
- **jCodeMunch MCP: user must enable file-based MCP in settings**. Warp's file-based MCP feature (`FileBasedMcp` flag) must be toggled on in Settings > AI > MCP Servers for the auto-registered `~/.warp-oss/.mcp.json` entry to be picked up and spawned. It is compiled in by default but starts disabled per-user; a future PR could force-enable it under `WARP_BYPASS_AUTH`. For now: open Settings, go to AI, enable "File-based MCP servers". jCodeMunch will then be spawned automatically on next launch if `jcodemunch-mcp` is on `$PATH`.
- **jCodeMunch auto-spawn (Option A) deferred**. The current implementation writes a static entry into the home MCP config at startup (Option B). Option A - dynamically spawning jCodeMunch pointed at the current workspace path and wiring it into the in-session MCP server list - requires hooking `FileBasedMCPManager` or `TemplatableMCPServerManager` to inject a per-workspace entry at runtime. Deferred to a follow-up PR.

## Build

```bash
./script/bootstrap   # one-time platform setup (Xcode tools, brew, rustup)
./script/run         # build, bundle, launch WarpOss.app
```

For everything else (engineering guide, coding style, testing, platform notes), see [WARP.md](WARP.md).

## Related work

[`regismesquita/warp-with-local-server`](https://github.com/regismesquita/warp-with-local-server) takes a different route to the same goal. It runs a local OpenAI-compatible shim server that intercepts Warp's GraphQL/WebSocket calls and translates them to any OpenAI-compatible upstream (Anthropic, OpenAI, Ollama, LiteLLM). Network-layer interception, vs this fork's harness-layer override that shells out to local CLIs or posts to Ollama directly.

## Licensing

Same as upstream. `warpui_core` and `warpui` crates are [MIT](LICENSE-MIT). The rest of the repo is [AGPL v3](LICENSE-AGPL). My changes inherit the AGPL of the files they edit.

---

Looking for the original project? Read [the upstream Warp README](UPSTREAM_README.md).
