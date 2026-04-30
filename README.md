# Warp (personal fork)

A personal fork of [warpdotdev/warp](https://github.com/warpdotdev/warp) — Warp's terminal, with the agent, cloud sync, auth, and telemetry stripped out at runtime so the binary makes **zero outbound network calls to Warp's servers** and boots straight to a shell with no login.

You still get everything that makes Warp's terminal interesting: blocks, themes, splits, command palette, completions, GPU-accelerated rendering, vim mode, syntax highlighting. You don't get the AI agent, cloud-synced workspaces, "share to Drive," or any of the cloud features — they're hidden in the UI and short-circuited at the network layer.

This is for personal use. It's not affiliated with Warp / Denver Technologies, Inc.

---

## What's different from upstream

This fork is a **neuter, not a strip**. The agent and cloud code is still in the binary — it's just unreachable. The trade-off: the fork is small (~80 lines of changes across ~6 files), easy to merge with upstream when they update, and every commit `cargo check`s clean. Bigger binary, but no half-deleted code paths.

The full set of changes:

- **`http_client::Client::execute`** short-circuits any request to `warp.dev`, `*.warp.dev`, `warpdotdev.com`, or `*.warpdotdev.com` with a fake 503 response. All other hosts (GitHub, package indexes, MCP servers you configure, etc.) pass through normally.
- **`skip_login` is a default feature.** Auth treats you as signed-in with a local Test user, so the login modal never appears.
- **Onboarding tutorial is bypassed** because the test user has `is_onboarded: true`.
- **Telemetry sender** (`send_batch_messages_to_rudder`) returns `Ok(())` immediately. No events constructed, no payload sent.
- **`autoupdate` and `crash_reporting` features are off by default,** so Sentry and the auto-updater don't initialize. Even if you re-enabled them, the http_client block stops the actual traffic.
- **AI / Drive menus** removed from the menu bar.
- **Settings sidebar** trimmed to: Account, Code, Appearance, Features, Keybindings, Warpify, Privacy, About. Agent/Teams/Cloud/Drive/Billing entries removed.
- **`is_any_ai_enabled()`** returns `false` unconditionally. This single change gates ~265 call sites across the app: inline block AI buttons, agent footers, conversation features, palette entries, etc.
- **Right panel** (the agent / code-review side panel) hidden via `has_right_region() = false`.
- **Command palette filters out** the `WarpAi` binding group, plus the Warp Drive and Conversation data sources.

See [`CHANGELOG.md`](CHANGELOG.md) for the full diff history.

---

## Build & run

```bash
./script/bootstrap   # one-time setup; installs Xcode tools, brew deps, etc.
cargo run --bin warp-oss
```

First build takes 5-10 minutes (full debug compile + Metal shader compilation). Subsequent runs are ~30 seconds.

The binary is `warp-oss`, with bundle ID `dev.warp.WarpOss`. It runs **side-by-side** with the official Warp app — different bundle ID, different data directory, different keychain namespace. Your installed Warp's settings, history, and credentials are completely untouched.

Data lives at: `~/Library/Application Support/dev.warp.WarpOss/`

For an optimized binary:

```bash
cargo build --release --bin warp-oss
./target/release/warp-oss
```

Engineering details (testing, presubmit, platform-specific notes) are in [`WARP.md`](WARP.md).

---

## Warp Contributions Overview Dashboard

Explore [build.warp.dev](https://build.warp.dev) to:
- Watch thousands of Oz agents triage issues, write specs, implement changes, and review PRs
- View top contributors and in-flight features
- Track your own issues with GitHub sign-in
- Click into active agent sessions in a web-compiled Warp terminal

## Licensing

This fork inherits the upstream license setup unchanged:

- **`warpui` and `warpui_core` crates** — [MIT](LICENSE-MIT)
- **Everything else** — [AGPL-3.0](LICENSE-AGPL)

What this means in practice:

- **Personal use:** AGPL imposes no obligations. You can run this on your own machine forever without sharing source.
- **Distribution:** if you fork this further and share modified binaries, AGPL §5 requires you to make the source available and preserve the copyright headers.
- **Network service:** AGPL §13 only kicks in if you offer the software as a network service. A terminal you run on your own laptop doesn't trigger it.

Source-file copyright headers (`Copyright (C) Denver Technologies, Inc.`) are preserved everywhere, as required.

---

## Credits

- **Upstream:** [github.com/warpdotdev/warp](https://github.com/warpdotdev/warp) — Warp / Denver Technologies, Inc.
- All the heavy lifting (blocks, GPU rendering, terminal protocol, command palette, themes) is theirs. This fork is a small set of runtime stubs and UI hides on top of their work.

See [`NOTICE.md`](NOTICE.md) for the modification notice required by AGPL §5.
