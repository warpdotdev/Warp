# danieljohnmorris/warp · personal fork

My personal AGPL fork of [warpdotdev/warp](https://github.com/warpdotdev/warp). The OSS build boots without server auth, the notifications inbox is gone, and agent runs route to local CLIs (`claude`, `codex`) instead of Warp's cloud.

> [!IMPORTANT]
> Not affiliated with or endorsed by Warp Inc. The upstream README (Warp's product description, contribution flow, support links) lives at [UPSTREAM_README.md](UPSTREAM_README.md).

## Why this fork exists

### A local-AI version of Warp with cloud features stripped

I want to run Warp's terminal and agent UI without signing into Warp's cloud and without paying for Warp's hosted models. The fork swaps Warp's server-mediated AI path for direct calls to local CLIs I already have authenticated (`claude`, `codex`). It removes the auth gate. It hides cloud-only UI: the notifications inbox, the upgrade-required model badges, the "free AI disabled" modal.

This is useful if you already pay for Claude or Codex and don't want a second AI subscription, if you want to keep agent traffic off a third-party server, or if you want to remix Warp's UI without the live cloud dependency.

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

- The interactive agent panel still dispatches via `generate_multi_agent_output()` to Warp's GraphQL server. Under bypass this returns 401. The `WARP_LOCAL_AI` harness override only intercepts the `agent_sdk` CLI path. Routing the interactive UI to a local CLI requires synthesising a `warp_multi_agent_api::ResponseEvent` stream. Tracked as follow-up.
- `WARP_LOCAL_AI=ollama` logs a warning and is not yet implemented.

## Build

```bash
./script/bootstrap   # one-time platform setup (Xcode tools, brew, rustup)
./script/run         # build, bundle, launch WarpOss.app
```

For everything else (engineering guide, coding style, testing, platform notes), see [WARP.md](WARP.md).

## Licensing

Same as upstream. `warpui_core` and `warpui` crates are [MIT](LICENSE-MIT). The rest of the repo is [AGPL v3](LICENSE-AGPL). My changes inherit the AGPL of the files they edit.

---

Looking for the original project? Read [the upstream Warp README](UPSTREAM_README.md).
