# Product Spec: User-configurable language servers

**Issue:** [warpdotdev/warp#8803](https://github.com/warpdotdev/warp/issues/8803)
**Figma:** none provided

## Summary

Let users add language servers Warp does not ship out of the box by declaring them in a settings file (binary path, arguments, file types they apply to, optional root-file globs, optional initialization options). When the user opens a file matching a configured server's file types, Warp offers to enable that server for the workspace and — once enabled — uses it for code intelligence (diagnostics, hover, goto, completions) on that file type, alongside any built-in servers.

This is the contributor-facing alternative to baking each language server into the Warp binary. It directly addresses why PRs adding specific built-in LSPs (e.g. PHP Intelephense in #9562, JSON in #9568) were closed in favor of this product direction.

## Problem

Warp currently ships a closed set of language servers as variants of `crates/lsp/src/supported_servers::LSPServerType`. Every new language requires:

1. A new `LSPServerType` enum variant.
2. A new `LanguageServerCandidate` impl with detection, install, and `fetch_latest_server_metadata` logic.
3. A new `LanguageId` enum variant in `crates/lsp/src/config.rs` plus extension mapping.
4. A new entry in `LanguageId::lsp_language_identifier`.

This scales poorly and pulls language-specific install logic into the core. Users with niche languages (Lua, Zig, Swift, Elixir, Solidity, Bash, Tailwind, etc.) cannot add support without modifying the binary. Maintainers cannot accept PRs adding specific servers without committing to ongoing maintenance of those servers' install/version-fetch code paths.

## Goals

- A user can add a language server to Warp by editing a settings file — no binary modifications, no agent involvement.
- Configured servers run side-by-side with built-in servers and follow the same code-intelligence surface (diagnostics, hover, goto, completions, semantic tokens).
- Configuration is portable across workspaces and discoverable in Warp's settings UI.
- Enablement is per-workspace by default with a clear opt-in moment, not silent global activation.
- Mis-configuration produces a visible, actionable error — not a silent disabled state.

## Non-goals

- **Auto-installation of user-configured servers.** The user installs the binary themselves (npm, cargo, system package manager, brew, etc.). Warp does not run package managers on the user's behalf for these.
- **Version pinning / auto-update of user-configured servers.** Out of scope; the user owns the lifecycle.
- **Per-file dynamic switching.** A single file is associated with at most one user-configured server, plus any built-in servers that already match. No "try server A, fall back to server B" runtime logic.
- **Replacing built-in servers.** A user-configured server with the same `file_types` as a built-in does **not** disable the built-in. Both run; the LSP client merges their results.
- **Cross-workspace global enablement on first open.** A configured server is *defined* globally but *enabled* per-workspace.
- **Marketplace / discovery of community configs.** Out of scope; users find configs themselves.

## User experience

### Adding a server

1. User edits their Warp settings file (TOML, located at the standard Warp settings path) and adds an `[[lsp.servers]]` entry. (See the "Configuration shape" section below.)
2. Warp detects the new entry on settings reload. No restart required.
3. If the entry is malformed (missing `name`, missing `command`, empty `file_types`), Warp shows a non-blocking notification: *"Custom LSP `<name>` is misconfigured: <reason>. See settings."* with a button to open the settings file at the offending line.

### First time opening a matching file

1. User opens a file whose extension matches a configured server's `file_types` (e.g. opens `foo.lua` with a Lua server configured).
2. Warp detects the configured server is *defined* but not yet *enabled* for this workspace.
3. The editor footer shows a chip: *"Enable `<name>` for this workspace?"* with `Enable` / `Dismiss` buttons.
4. If `Enable` is pressed, Warp:
   - Records the per-workspace enablement.
   - Spawns the server process with the configured command and args.
   - Starts driving LSP traffic for files matching `file_types` in this workspace.
5. If `Dismiss` is pressed, the chip is suppressed for this workspace until the user re-opens it from settings.

### Subsequent opens in an enabled workspace

1. User opens any file matching `file_types` in a workspace where the server is already enabled.
2. The server is already running; no UI surfaces. Diagnostics, hover, goto, completions all behave as they do for built-in servers.

### Disabling a server in a workspace

1. User opens settings → "Code intelligence" → "Custom language servers".
2. Each configured server lists which workspaces it is enabled for.
3. User clicks the workspace row's `Disable` button. Warp shuts down that server's process for that workspace and removes the per-workspace enablement record.

### Misconfiguration scenarios

1. **Binary not on PATH:** When a user enables a server whose `command[0]` is not on PATH, Warp shows: *"Could not start `<name>`: binary `<cmd>` not found on PATH."* The chip's `Enable` button is replaced with `Open settings`.
2. **Binary on PATH but spawn fails:** Warp shows: *"`<name>` exited with status `<n>`. Last 200 bytes of stderr: `<...>`."* with `Open settings` and `Retry` buttons.
3. **Spawn hangs:** A configurable `start_timeout` (default 5s) bounds the LSP `initialize` request. On timeout, Warp shows: *"`<name>` did not respond to `initialize` within 5s."* The server's process is killed.

## Configuration shape

The user's Warp settings TOML grows a new `[lsp]` table. Multiple servers via array-of-tables `[[lsp.servers]]`:

```toml
[[lsp.servers]]
name = "intelephense"
command = ["intelephense", "--stdio"]
file_types = ["php", "phtml"]
root_files = ["composer.json", "composer.lock"]      # optional; default: file's parent dir
initialization_options = { storagePath = "/tmp/intelephense" }   # optional, opaque to Warp
start_timeout_ms = 5000                                          # optional, default 5000
```

| Field | Required | Type | Notes |
|---|---|---|---|
| `name` | yes | string | Display name. Must be unique across all configured servers. |
| `command` | yes | array of strings | First element is the binary; rest are args. Resolved against PATH. |
| `file_types` | yes | array of strings | File extensions (no dot) the server handles. Must be non-empty. |
| `root_files` | no | array of strings | Glob patterns whose presence in an ancestor directory marks the workspace root. Default: the file's parent directory. |
| `initialization_options` | no | TOML table | Passed verbatim to the LSP `initialize` request as `initializationOptions`. Opaque to Warp. |
| `start_timeout_ms` | no | integer | Bound on the time we wait for `initialize` to return. Default 5000. |

Settings reload re-reads the entire `[lsp]` table; servers whose configuration changed are restarted. Removed entries shut down. Added entries become available (but are not auto-enabled).

## Testable behavior invariants

Numbered list — each maps to a verification path in the tech spec:

1. A `[[lsp.servers]]` entry with `name`, `command` (non-empty array), and `file_types` (non-empty array) is accepted at settings parse time.
2. A `[[lsp.servers]]` entry missing any of `name` / `command` / `file_types`, OR with empty `command` / `file_types`, OR with a duplicate `name`, is rejected at parse time and surfaces a settings-error notification with the offending line range.
3. Opening a file whose extension is in `file_types` of a configured-but-not-enabled server shows the "Enable" chip in the editor footer for that file's workspace exactly once per (server, workspace) pair until the user dismisses or enables.
4. Pressing "Enable" on the chip starts a server process via `command[0]` with `command[1..]` as args, sends an LSP `initialize` request (with `initializationOptions` from the config), and on receiving a successful response begins routing LSP traffic for files matching `file_types` in that workspace.
5. If `command[0]` is not on PATH at enablement time, no process is spawned and the user sees an error notification with `Open settings` action.
6. If the spawned process exits non-zero before sending an `initialize` response, the user sees an error notification with the exit status and last 200 bytes of stderr.
7. If `initialize` does not return within `start_timeout_ms` (default 5000), the spawned process is killed via `Drop` of the `Child` handle, the LSP client is torn down cleanly, and the user sees a timeout notification.
8. After a server is enabled in workspace W, opening any file matching `file_types` in W routes LSP requests to that server **without** showing the chip again.
9. After settings change (server entry edited or removed), an enabled-and-running server for the changed entry is restarted with the new config (or shut down if removed) within 1s of settings reload, with no Warp restart required.
10. The user-configured server runs alongside any built-in server whose `LSPServerType` matches the same file extension; both servers receive requests, and the LSP client merges responses (existing built-in client behavior; not changed by this feature).
11. Disabling a server via settings UI shuts down its process for the targeted workspace within 1s, removes the per-workspace enablement record, and the chip reappears on next file open.
12. Restarting Warp preserves per-workspace enablement state — workspaces where a server was enabled before restart auto-spawn the server on the next file open without the chip reappearing.
13. `initialization_options` in the TOML is forwarded to the `initialize` request's `initializationOptions` field byte-equivalent (TOML → JSON conversion preserves nested tables and arrays).
14. The chip is **not** shown for a configured server in a workspace that has explicitly dismissed it; the user must re-enable from settings UI.
15. Shutting down the LSP system (e.g. on Warp quit) sends `shutdown` then `exit` to all running custom servers and waits up to 1s for graceful exit before SIGKILL.

## Open questions

- **Should `command` support `~` and `$VAR` expansion?** Cmd-O / `/open-file` use `shellexpand::tilde`; consistent behavior here would be friendly. Recommend yes for `~`, defer `$VAR` to a follow-up.
- **Should we ship example configs?** A `docs/custom-lsp-examples.md` with intelephense / lua-language-server / zls / bash-language-server entries would shorten time-to-first-success. Recommend yes; not part of the core feature gate.
- **Schema-fetching restriction:** The JSON LSP work in #9568 found that VS Code's JSON server fetches remote schemas by default. Should the spec mandate that custom LSPs run with `network_access = false` by default? This is hard to enforce generically (each server has its own config keys for schema fetching). Recommend punting to per-server `initialization_options` and documenting the pattern.
