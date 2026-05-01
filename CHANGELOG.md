# Changelog

All notable changes to the **Helm fork** of Warp are documented in this file.

This file follows the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
convention and tracks **only Helm fork divergences from upstream Warp**. It is
**not** a mirror of upstream Warp's changelog.

## About this changelog

- Helm is a hard fork of Warp ([warpdotdev/Warp](https://github.com/warpdotdev/Warp))
  that strips Warp-hosted backend dependencies (Firebase auth, RudderStack
  telemetry, hosted Object/Drive sync, hosted Agent Mode / Oz) so the binary
  runs fully offline against local-only model providers.
- Upstream Warp commits flow into this fork via a **rebase model**. Their
  changes are visible in `git log` and `git diff origin/master..HEAD`; we do
  **not** re-enumerate them here.
- This changelog records only what Helm **removes**, **gates**, **replaces**,
  or **adds** on top of upstream — i.e. what a reviewer needs to understand the
  fork's surface area.
- Linear project (issue tracker for the fork):
  <https://linear.app/pdx-software/project/helm-4bdefa429aaa>

## [Unreleased] — Helm phase A

Phase A is the "strip Warp-hosted, ship local-only minimal binary" milestone.
Everything Warp-hosted is gated behind the `warp_hosted` Cargo feature in
`app/Cargo.toml` (`warp_hosted = ["warp_core/warp_hosted"]`). The default build
(`cargo build`) keeps the upstream feature set so the fork stays
rebase-friendly; the Helm binary ships with `--no-default-features`.

### Removed / gated under `warp_hosted`

#### Authentication strip — PDX-31

The hosted Firebase OAuth2 device-code flow and Warp identity API are gated.
With `warp_hosted` off, the app starts unauthenticated and never reaches
`app.warp.dev` / `oz.warp.dev` / `identitytoolkit.googleapis.com`.

- `app/src/auth/auth_manager.rs` — Firebase device-code flow gated; offline
  builds short-circuit `sign_in` / `refresh_token` to a no-op.
- `app/src/server/server_api/auth.rs` — GraphQL `create_anonymous_user` and
  `mint_custom_token` mutations compiled out.
- Hardcoded URL constants (`app.warp.dev`, `oz.warp.dev`,
  `identitytoolkit.googleapis.com`) gated; root URLs resolve to empty
  strings under `warp_hosted` off, fail-closing any caller that still
  reaches the network.
- Restore upstream behavior: `cargo build --features warp_hosted`.

#### Telemetry strip — PDX-33

RudderStack analytics and the four telemetry-mutation GraphQL calls become
no-ops in offline builds. No analytics data leaves the device.

- `send_batch_messages_to_rudder` — removed under `warp_hosted` off.
- Privacy-settings sync gated behind `warp_hosted`.
- The four GraphQL telemetry mutations are compiled to no-op stubs:
  - `set_is_telemetry_enabled`
  - `set_is_crash_reporting_enabled`
  - `set_is_cloud_conversation_storage_enabled`
  - `update_user_settings`
- Restore upstream behavior: `cargo build --features warp_hosted`.

#### Hosted-Oz strip — PDX-34

Hosted Agent Mode endpoints and the OpenWarp launch flow are gated. Note that
**full `ambient_agents` module gating was scoped out** because of a 25+
consumer cascade; the actual mechanism is **runtime fail-closed** via PDX-31's
empty `oz_root_url`, so the module compiles but never reaches the network.

- `app/src/workspace/view/openwarp_launch_modal/view.rs` — `oz.warp.dev` URL
  constants gated under `warp_hosted`.
- `app/src/ai/agent_management/cloud_setup_guide_view.rs` — hosted-setup URL
  constants gated under `warp_hosted`.
- `ambient_agents` module: **not** feature-gated at compile time; it relies on
  PDX-31's empty Oz root URL to fail-close at runtime.
- Restore upstream behavior: `cargo build --features warp_hosted`.

#### Drive cloud sync strip — PDX-32 (umbrella) → PDX-79, PDX-80, PDX-81, PDX-82

Drive (Warp's hosted notebook/workflow sync) is gated end-to-end. Local
SQLite-backed notebooks remain fully functional; only the cloud sync layer is
removed.

- **PDX-79** — `SyncQueue` + `ObjectClient` feature-gated to no-op offline.
  Outbound mutations are dropped silently when `warp_hosted` is off.
- **PDX-80** — Drive UI surfaces gated under `warp_hosted`:
  - Drive panel
  - Drive import modal
  - Sharing dialog
- **PDX-81** — Local SQLite notebook persistence verified to work fully
  offline; no behavior change, only validation.
- **PDX-82** *(in progress as of this changelog write)* — Local notebook
  hydration without the cloud `object_metadata` sidecar; ensures notebooks
  load from local SQLite alone.
- Restore upstream behavior: `cargo build --features warp_hosted`.

#### Sentry crash-reporting replacement — PDX-78

This is a **replacement**, not a strip. Upstream wires Sentry through a
hardcoded DSN derived from `ChannelState::sentry_url()`; the Helm fork
introduces a Doppler-backed DSN resolution shim while leaving the
`crash_reporting` feature flag mechanic intact.

- `app/src/sentry_init.rs` *(new)* — startup shim that reads `SENTRY_DSN`
  from Doppler, falling back to `ChannelState::sentry_url()` if Doppler is
  unavailable.
- The `crash_reporting` Cargo feature still gates Sentry entirely.
- Behavior matrix:
  - `crash_reporting` off → no Sentry, no DSN lookup.
  - `crash_reporting` on + Doppler available → Helm DSN.
  - `crash_reporting` on + Doppler unavailable → upstream fallback DSN.

### Added — Helm-specific surfaces

These are net-new modules that exist in the Helm fork only. They are **not**
gated by `warp_hosted`; they are first-class members of the local-only build.

#### `crates/orchestrator/`

Local agent routing and budget enforcement. Replaces the role formerly played
by hosted Agent Mode.

- **PDX-37** — `Agent` trait (uniform interface across local providers).
- **PDX-38** — `Budget` tracker (per-session token / cost ceilings).
- **PDX-39** — `Router` (capability-based dispatch across registered agents).

#### `crates/agents/`

Concrete agent implementations behind the orchestrator's `Agent` trait.

- **PDX-44** — Claude Code adapter.
- **PDX-45** — Codex adapter.
- **PDX-46** — Ollama adapter.
- **PDX-47** — Foundation Models stub (Apple on-device, stub only).
- **PDX-48** — Remote stub (placeholder for gateway-routed remote agents).

#### `crates/doppler/`

Doppler integration for secret resolution at startup.

- **PDX-49** — CLI detection (locates a working `doppler` binary).
- **PDX-53** — Secret fetcher with TTL cache.

#### `crates/symphony/`

Linear-driven daemon that watches the Helm Linear project and drives planning
/ orchestration cycles. **MVP shipped.**

- **PDX-24** — Symphony MVP.

#### App-level additions

- `app/src/sentry_init.rs` — Doppler DSN shim (PDX-78; see above).
- `app/src/settings_view/doppler_page.rs` — Doppler sign-in button in
  Settings (PDX-50).

#### Documentation

- `docs/symphony/README.md` — Symphony adaptation context.
- `docs/symphony/soak-test.md` — Symphony soak-test protocol.

## Build instructions

### Helm minimal (local-only) binary

```sh
cargo build --no-default-features
```

This excludes the `warp_hosted` feature transitively and produces the
offline-only binary used by the Helm fork.

### Upstream-equivalent build

```sh
cargo build
```

The default feature set still includes `warp_hosted`, preserving upstream
Warp behavior. Useful for keeping the fork rebase-compatible — if a build
breaks under default features, an upstream contract has shifted under the
gate.

### Explicit upstream restore

```sh
cargo build --features warp_hosted
```

Equivalent to the default build; useful inside CI matrices that pin
`--no-default-features` and re-add features explicitly.

## Linear cross-reference

All `PDX-*` issue identifiers above resolve under the Helm project:

<https://linear.app/pdx-software/project/helm-4bdefa429aaa>

Phase A epics span **PDX-5 through PDX-12**; concrete tickets referenced in
this changelog fall in **PDX-24, PDX-31..82**. Identifiers above PDX-82 (if
any are added in future revisions of this file) refer to in-progress or
future work and will be marked explicitly.
