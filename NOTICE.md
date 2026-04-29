# NOTICE

This repository is a fork of [warpdotdev/warp](https://github.com/warpdotdev/warp), maintained at [github.com/GarethCott/warp](https://github.com/GarethCott/warp) for personal use.

The upstream project is © Denver Technologies, Inc., licensed under AGPL-3.0 (most of the codebase) and MIT (the `warpui` and `warpui_core` crates). This fork inherits both licenses unchanged. See [`LICENSE-AGPL`](LICENSE-AGPL) and [`LICENSE-MIT`](LICENSE-MIT) for the full license texts.

## Modifications (per AGPL §5)

AGPL §5(b) requires modified files to carry prominent notices. The modifications in this fork are confined to a small, well-documented set of files:

| File | Change |
|---|---|
| `crates/http_client/src/lib.rs` | Added `is_blocked_host` predicate; `Client::execute` short-circuits requests to `warp.dev` / `warpdotdev.com` / their subdomains |
| `app/Cargo.toml` | Added `skip_login` to `default = [...]` features list |
| `app/src/server/telemetry/mod.rs` | `send_batch_messages_to_rudder` returns early without sending |
| `app/src/app_menus.rs` | Removed AI and Drive menus from the menu bar |
| `app/src/settings_view/mod.rs` | Trimmed the settings sidebar nav |
| `app/src/workspace/view.rs` | `has_right_region` returns false |
| `app/src/search/command_palette/data_sources.rs` | Removed Warp Drive and Conversation data sources from the palette |
| `app/src/search/action/data_source.rs` | Filter out the `WarpAi` binding group |
| `app/src/settings/ai.rs` | `is_any_ai_enabled` returns false |

Each in-source modification carries a `// neuter:` comment marking the change.

The complete commit history of these modifications is preserved in git on the `neuter` branch. No upstream files have had their copyright headers altered or removed.

## Original copyright

Copyright (C) Denver Technologies, Inc. and Warp contributors. See the upstream repository and source file headers for full attribution.
