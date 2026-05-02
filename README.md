# danieljohnmorris/warp · personal fork

My personal AGPL fork of [warpdotdev/warp](https://github.com/warpdotdev/warp). The OSS build boots without server auth, the notifications inbox is gone, and agent runs route to local CLIs (`claude`, `codex`) instead of Warp's cloud.

> [!IMPORTANT]
> Not affiliated with or endorsed by Warp Inc. The upstream README (Warp's product description, contribution flow, support links) lives at [UPSTREAM_README.md](UPSTREAM_README.md).

> [!NOTE]
> This is the `main` branch: purpose 1 only (local-AI bypass). Purpose 2 (ilo-lang testbench) lives on the [`ilo`](https://github.com/danieljohnmorris/warp/tree/ilo) branch.

## Why this fork exists

### 1. A local-AI version of Warp with cloud features stripped

I want to run Warp's terminal and agent UI without signing into Warp's cloud and without paying for Warp's hosted models. The fork swaps Warp's server-mediated AI path for direct calls to local CLIs I already have authenticated (`claude`, `codex`). It removes the auth gate. It hides cloud-only UI: the notifications inbox, the upgrade-required model badges, the "free AI disabled" modal.

This is useful if you already pay for Claude or Codex and don't want a second AI subscription, if you want to keep agent traffic off a third-party server, or if you want to remix Warp's UI without the live cloud dependency.

### 2. A testbench for ilo-lang

Implemented on the [`ilo`](https://github.com/danieljohnmorris/warp/tree/ilo) branch. This branch (`main`) does not include the ilo-lang context injection.

I'm using this fork to test how my agent-optimised programming language, [**ilo-lang**](https://ilo-lang.ai), behaves when wired into a working developer tool. Ilo is token-minimal: programs are written and read primarily by LLMs, so every saved token compounds across millions of agent turns.

This is iteration 3 of integrating ilo into a real agent loop. v1 was Claude Code with the ilo spec pasted manually into the system message. v2 was a Claude skill that loaded the ilo spec on demand. v3 is this fork: ilo context injected at the harness layer of a terminal that runs an agent loop on every keystroke, against whatever codebase I'm in. If ilo expresses agent prompts, tool definitions, and workflows in fewer tokens than Markdown, YAML, or JSON, the difference shows up in day-to-day work, not in a benchmark.

Purpose 1 helps purpose 2: the local-AI bypass lets me run the same ilo system prompt against different LLMs (Claude, Codex, eventually Ollama) and compare token usage and output quality across them. See the [`ilo` branch](https://github.com/danieljohnmorris/warp/tree/ilo) for the implementation.

**Forked from**: [`warpdotdev/warp@00df35b`](https://github.com/warpdotdev/warp/commit/00df35b5dc951b9ed9ac57f873ea0b0484f42ad6), May 2026.
**Sync upstream**: `git fetch upstream master && git merge upstream/master`.

## Configuration (`.env`)

The OSS build reads `~/.warp/.env`, then `./.env` from the cwd, at startup. See [`.env.example`](.env.example) for the full list.

```env
# Boot directly into a logged-in fake-user state. Skips the browser-check / paywall.
WARP_BYPASS_AUTH=1

# Route agent runs to a local CLI. Currently supported: claude, codex.
# Ollama is wired but not yet implemented.
WARP_LOCAL_AI=claude
```

## What's changed vs upstream

- **Auth bypass**. `WARP_BYPASS_AUTH=1` short-circuits `AuthState::initialize` to a Test user, drops the `cfg(skip_login)` gates so `Credentials::Test` is available in OSS builds, and forces `is_any_ai_enabled` so the AI menu, Cmd+I binding, and agent-mode predicate light up.
- **Local-AI harness override**. When `WARP_LOCAL_AI` is set, `harness_kind()` overrides the user's selection to the matching `ThirdParty` harness. A new `CodexHarness` shells `codex --full-auto`. The agent driver synthesises the `ResolvedHarnessPrompt` locally, so it never calls `ServerApi::resolve_prompt()`.
- **Notifications inbox hidden**. `HeaderToolbarItemKind::NotificationsMailbox::is_supported()` returns `false`, removing the inbox icon and dropdown from the header toolbar.
- **`.env` loading**. dotenvy loads `.env` at the top of `run()`, from the cwd and from `~/.warp/.env`, so launching via `open WarpOss.app` (cwd = `/`) still picks up env vars.

## Known limitations

- The interactive agent panel under `WARP_LOCAL_AI` routes through a local CLI subprocess (`claude -p` / `codex exec`). Tool calls the CLI emits (file edits, shell commands) are not forwarded back to the Warp panel UI - only the assistant text reply is shown. Full tool-loop support requires a richer protocol bridge and is left as follow-up.
- `WARP_LOCAL_AI=ollama` logs a warning and is not yet implemented.

## Build

```bash
./script/bootstrap   # one-time platform setup (Xcode tools, brew, rustup)
./script/run         # build, bundle, launch WarpOss.app
```

For everything else (engineering guide, coding style, testing, platform notes), see [WARP.md](WARP.md).

## Related work

[`regismesquita/warp-with-local-server`](https://github.com/regismesquita/warp-with-local-server) takes a different route to the same goal. It runs a local OpenAI-compatible shim server that intercepts Warp's GraphQL/WebSocket calls and translates them to any OpenAI-compatible upstream (Anthropic, OpenAI, Ollama, LiteLLM). Network-layer interception, vs this fork's harness-layer override that shells out to local CLIs.

## Licensing

Same as upstream. `warpui_core` and `warpui` crates are [MIT](LICENSE-MIT). The rest of the repo is [AGPL v3](LICENSE-AGPL). My changes inherit the AGPL of the files they edit.

---

Looking for the original project? Read [the upstream Warp README](UPSTREAM_README.md).
