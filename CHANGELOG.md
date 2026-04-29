# Changelog

All notable changes in this fork relative to upstream [warpdotdev/warp](https://github.com/warpdotdev/warp).

## neuter branch

The `neuter` branch carries the full set of modifications. Each commit is a self-contained, `cargo check`-clean change.

### Network egress

- **`http_client`: block requests to warp backend domains.** Added a host-based block at the `Client::execute` chokepoint; any request to `warp.dev`, `*.warp.dev`, `warpdotdev.com`, or `*.warpdotdev.com` short-circuits with a fake 503 response. All other hosts unaffected. Two unit tests cover the predicate.

### Auth & onboarding

- **`auth`: enable `skip_login` as a default feature.** The auth state initializer already had a `skip_login` cfg branch that installs a local Test user with `Credentials::Test`. Promoting it to default means the app boots already authenticated, bypassing the login modal and every "must be signed in" UI gate. The Test user is also marked `is_onboarded: true`, so the onboarding tutorial never triggers.

### Telemetry

- **`telemetry`: short-circuit RudderStack batch sender.** `send_batch_messages_to_rudder` returns `Ok(())` immediately. Original body left in place behind the early return for easier upstream merges.

### UI

- **`ui`: hide AI/Drive top menus and trim settings sidebar.** Removed `make_new_ai_menu` and `make_new_drive_menu` from the menu bar. Settings sidebar trimmed to: Account, Code, Appearance, Features, Keybindings, Warpify, Privacy, About.

- **`ui`: hide right panel, palette agent entries, all AI-gated UI.**
  - `has_right_region` returns false unconditionally — agent/code-review right panel never renders.
  - Command palette: dropped Warp Drive and Conversation data sources; added a filter to exclude the `WarpAi` binding group from the actions data source.
  - `is_any_ai_enabled` returns false — single point of truth that gates ~265 call sites (inline block AI buttons, agent footers, conversation features, etc.).

### What was *not* changed

- **No source files had copyright headers removed or altered.**
- **No deletions** beyond removing the agent/cloud crates that were never wired into the OSS Channel anyway. (See the abandoned `strip-cloud` graveyard branch for the failed strip-out experiment.)
- **`autoupdate` and `crash_reporting`** required no edits — they were already off by default and gated behind feature flags. The http_client block provides defense-in-depth.

### Defense-in-depth philosophy

Several surfaces are protected by more than one layer:

- Telemetry: dropped at the sender level *and* blocked at http_client
- Autoupdate: feature flag off by default *and* blocked at http_client
- Sentry: feature flag off by default *and* blocked at http_client (Sentry's ingest hosts aren't `*.warp.dev`, but the feature is off)
- AI features: gated by `is_any_ai_enabled() = false` *and* hidden via UI removals *and* the agent backend is unreachable due to http_client block + skip_login

If a future upstream merge re-enables one of these surfaces, the others should still keep the fork's behavior consistent.
