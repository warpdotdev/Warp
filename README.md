# danieljohnmorris/warp · personal fork

A personal AGPL fork of [warpdotdev/warp](https://github.com/warpdotdev/warp). Boots the OSS build without server auth, hides the notifications inbox, and adds plumbing to route AI agent runs to local CLIs (`claude`, `codex`) instead of Warp's cloud.

> [!IMPORTANT]
> Not affiliated with or endorsed by Warp Inc. The upstream README (Warp's product description, contribution flow, support links) lives at [UPSTREAM_README.md](UPSTREAM_README.md).

**Forked from**: [`warpdotdev/warp@00df35b`](https://github.com/warpdotdev/warp/commit/00df35b5dc951b9ed9ac57f873ea0b0484f42ad6) (2026-05-01).
**Sync upstream**: `git fetch upstream master && git merge upstream/master`.

## Configuration (`.env`)

The OSS build reads `~/.warp/.env` (or `./.env` from the cwd) at startup. See [`.env.example`](.env.example) for the full list. Common usage:

```env
# Boot directly into a logged-in fake-user state. Skips the browser-check / paywall.
WARP_BYPASS_AUTH=1

# Route agent runs to a local CLI. Currently supported: claude, codex.
# Ollama is wired but not yet implemented.
WARP_LOCAL_AI=claude

# Optional: prepended to the system prompt of every local-AI run.
WARP_ILO_SYSTEM_PROMPT="You are an ilo-lang expert."
```

## What's changed vs upstream

- **Auth bypass**: `WARP_BYPASS_AUTH=1` short-circuits `AuthState::initialize` to a Test user, drops the `cfg(skip_login)` gates so `Credentials::Test` is available in OSS builds, and forces `is_any_ai_enabled` so the AI menu, Cmd+I binding, and agent-mode predicate all light up.
- **Local-AI harness override**: when `WARP_LOCAL_AI` is set, `harness_kind()` overrides the user's selection to the matching `ThirdParty` harness. New `CodexHarness` shells `codex --full-auto`. The agent driver synthesizes the `ResolvedHarnessPrompt` locally so it never calls `ServerApi::resolve_prompt()`.
- **ilo-lang context injection**: `WARP_ILO_SYSTEM_PROMPT` is prepended to every system prompt before the harness writes it to the temp file passed to the CLI subprocess.
- **Notifications inbox hidden**: `HeaderToolbarItemKind::NotificationsMailbox::is_supported()` returns `false`, removing the inbox icon and dropdown from the header toolbar.
- **`.env` loading**: dotenvy loads `.env` at the top of `run()` from cwd and `~/.warp/.env`, so launching via `open WarpOss.app` (cwd = `/`) still picks up env vars.

## Known limitations

- The interactive agent panel still dispatches via `generate_multi_agent_output()` → Warp's GraphQL server. Under bypass this returns 401. The `WARP_LOCAL_AI` harness override only intercepts the `agent_sdk` CLI path. Full interactive-UI routing to a local CLI requires synthesizing a `warp_multi_agent_api::ResponseEvent` stream — tracked follow-up.
- `WARP_LOCAL_AI=ollama` logs a warning and is not yet implemented.
- `WARP_ILO_SYSTEM_PROMPT` accepts plain text only; loading from a file path is a follow-up.

## Build

```bash
./script/bootstrap   # one-time platform setup (Xcode tools, brew, rustup)
./script/run         # build + bundle + launch WarpOss.app
```

For everything else (engineering guide, coding style, testing, platform notes), see [WARP.md](WARP.md).

## Licensing

Same as upstream: `warpui_core` and `warpui` crates are [MIT](LICENSE-MIT); everything else is [AGPL v3](LICENSE-AGPL). My changes inherit the AGPL of the files they edit.

---

→ Looking for the original project? Read [the upstream Warp README](UPSTREAM_README.md).
