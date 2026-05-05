# REMOTE-1345: MCP Servers for Third-Party Harnesses

## Context

MCP server setup in the agent driver is gated on `HarnessKind::Oz` (`driver.rs:1258-1303`). Third-party harnesses (Claude Code, Codex, Gemini) receive zero MCP servers even when the user specifies them via `--mcp` or environment config. File-based MCP discovery (`driver.rs:1325-1341`) is also Oz-only.

The user's MCP config arrives as `Task.mcp_specs: Vec<MCPSpec>` (`driver.rs:336`), where each spec is either a `MCPSpec::Uuid` (reference to an installed server) or `MCPSpec::Json` (inline JSON config). For the Oz harness, `resolve_mcp_specs` (`driver.rs:826`) splits these into UUID refs and ephemeral `TemplatableMCPServerInstallation` objects, which are then spawned as child processes (stdio) or HTTP clients by `TemplatableMCPServerManager`.

Third-party harnesses have their own MCP client implementations:
- **Claude Code** reads `.mcp.json` or accepts `--mcp-config <json-file>` with `{ "mcpServers": { name: { command, args, env } } }` format.
- **Codex** reads `~/.codex/config.toml` with `[mcp_servers.name]` sections: `command`/`args`/`env` (stdio) or `url`/`bearer_token_env_var` (HTTP).
- **Gemini** has no documented MCP config support — out of scope for now.

Each harness already has a `prepare_environment_config` hook (`driver/harness/mod.rs:84`) called before the CLI launches, which writes auth, trust, and onboarding config. This is the natural place to also write MCP config.

### Key files
- `app/src/ai/agent_sdk/driver.rs:1258-1303` — Oz-only MCP setup block
- `app/src/ai/agent_sdk/driver.rs:826-858` — `resolve_mcp_specs`
- `app/src/ai/agent_sdk/driver/harness/mod.rs:83-91` — `prepare_environment_config` trait method
- `app/src/ai/agent_sdk/driver/harness/claude_code.rs:66-78` — Claude impl
- `app/src/ai/agent_sdk/driver/harness/codex.rs:50-62` — Codex impl
- `app/src/ai/mcp/parsing.rs:382-397` — `resolve_json` (template → final JSON)
- `app/src/ai/mcp/templatable_installation.rs:113-156` — `apply_secrets`
- `app/src/ai/mcp/mod.rs:178-228` — `JSONTransportType`, `JSONMCPServer`
- `app/src/ai/agent_sdk/driver/environment.rs:289-300` — single-repo `cd` into repo subdir

## Proposed changes

### Approach: let each harness spawn its own MCP servers

Write resolved MCP configs into each harness's native config format before the CLI launches. The harness CLI spawns/connects to servers itself. This avoids multiplexing stdio pipes and keeps MCP lifecycle owned by the harness.

### 1. Resolve MCP specs to `HashMap<String, JSONMCPServer>` on the UI thread

Add a new helper `resolve_mcp_specs_to_json` that runs on the foreground spawner (where model context is available):
- For `MCPSpec::Uuid`: look up `TemplatableMCPServerManager::get_installed_server(uuid)` → `TemplatableMCPServerInstallation`, call `apply_secrets`, then `resolve_json(installation)` → parse via `MCPServer::from_user_json`.
- For `MCPSpec::Json`: parse → `TemplatableMCPServerInstallation` → `apply_secrets` → `resolve_json`.
- Collect all into `HashMap<String, JSONMCPServer>`.

Call this in `prepare_harness` (around `driver.rs:1564`) before `prepare_environment_config`. This keeps harness impls free of Warp model context.

No redundant work for Oz: `prepare_harness` is only called from the `HarnessKind::ThirdParty` branch of `run_internal` (line 1455). The Oz path (line 1435) goes straight to `execute_run` and continues to use its own `resolve_mcp_specs` → `start_mcp_servers` flow unchanged.

### 2. Extend `ThirdPartyHarness::prepare_environment_config` signature

Add a `resolved_mcp_servers: &HashMap<String, JSONMCPServer>` parameter to the trait method. Gemini ignores it; Claude and Codex use it.

### 3. Claude Code: pass `--mcp-config <temp-file>`

In `prepare_claude_environment_config`, if `resolved_mcp_servers` is non-empty:
- Serialize to `{ "mcpServers": { name: config, ... } }` JSON. This is the same `McpJsonConfig` schema used by `.mcp.json`, `~/.claude.json`, and `--mcp-config` — `parseMcpConfigFromFilePath` (`config.ts:1384`) parses all three identically. The `--mcp-config` flag accepts a file path or inline JSON string; both go through `parseMcpConfig` → `McpJsonConfigSchema` validation.
- Write to a `NamedTempFile` (same pattern as prompt files — held by `ClaudeHarnessRunner` so it lives until the CLI exits).
- Pass the path as `--mcp-config '<path>'` in `claude_command()`.

Using `--mcp-config` instead of writing `.mcp.json` avoids a cwd problem: in the single-repo case, `environment.rs:291-296` does `cd_in_terminal(repo_name)` so the terminal cwd is `{working_dir}/{repo}`, not `working_dir`. Claude discovers `.mcp.json` relative to its cwd, so a file at `working_dir/.mcp.json` wouldn't be found. The `--mcp-config` flag is path-independent and also avoids merge conflicts with repo-owned `.mcp.json` files.

Implementation detail: `prepare_environment_config` currently returns `Result<(), AgentDriverError>`, not a temp file handle. Options:
- (a) Return an optional `NamedTempFile` from `prepare_environment_config` and thread it to `build_runner` / the runner struct — messy signature change.
- (b) Write the MCP config file inside `build_runner` instead — `build_runner` already has `working_dir` and creates temp files for prompts. This is cleaner; `prepare_environment_config` stays focused on static config, and `build_runner` handles per-run temp files.

Option (b) is preferred. Pass `resolved_mcp_servers` to `build_runner` as well, and let `ClaudeHarnessRunner::new` write the temp file + append `--mcp-config` to the command.

### 4. Codex: write `[mcp_servers]` entries to `config.toml`

In `prepare_codex_environment_config`, if `resolved_mcp_servers` is non-empty:
- Convert `JSONMCPServer` → TOML entries: `CLIServer { command, args, env }` → `[mcp_servers.name]` with `command`, `args`, `env`; `SSEServer { url, headers }` → `url` field.
- Append to `~/.codex/config.toml` inside `prepare_codex_config_toml` (already writes to this file).

This is safe to do in `prepare_environment_config` since codex config is global, not cwd-relative.

### 5. Transport mapping

Warp's `JSONTransportType` maps to each harness as follows:

**`CLIServer { command, args, env, working_directory }`:**
- Claude Code: `{ "type": "stdio", "command": "...", "args": [...], "env": {...} }`
- Codex: `command = "..."`, `args = [...]`, `env = { ... }` under `[mcp_servers.name]`

**`SSEServer { url, headers }`:**
- Claude Code: `{ "type": "sse", "url": "...", "headers": {...} }`
- Codex: `url = "..."` under `[mcp_servers.name]`. Codex's `StreamableHttp` transport (`mcp_types.rs:378-391`) supports three header mechanisms:
  - `http_headers` — static `HashMap<String, String>`, maps directly from Warp's `SSEServer.headers`
  - `env_http_headers` — headers where the value is an env var name (no Warp equivalent; unused)
  - `bearer_token_env_var` — env var name for a bearer token (no direct Warp equivalent, but could be extracted from `headers` if there's an `Authorization: Bearer $ENV_VAR` pattern)

  For now, map `SSEServer.headers` → `http_headers` directly. This covers the common case where headers contain literal values (API keys, auth tokens). The `env_http_headers` and `bearer_token_env_var` paths can be added later if needed.

## Testing and validation

1. **Unit tests for `resolve_mcp_specs_to_json`** — verify UUID and JSON specs both resolve to correct `JSONMCPServer` entries with secrets applied.
2. **Unit tests for Claude MCP config serialization** — verify `HashMap<String, JSONMCPServer>` produces valid `.mcp.json`-format JSON.
3. **Unit tests for Codex MCP config serialization** — verify `HashMap<String, JSONMCPServer>` produces valid `config.toml` `[mcp_servers]` entries.
4. **Integration test (Claude)** — run a cloud agent with `--harness claude --mcp '{"test": {"command": "echo", "args": ["hello"]}}'` and verify the Claude CLI receives the MCP config (visible in logs or session output).
5. **Integration test (Codex)** — same as above with `--harness codex`.
6. **Regression** — verify Oz harness MCP setup is unchanged (existing tests should cover this).

## Parallelization

After step 1 (shared resolution helper) and step 2 (trait signature change) are merged:
- Claude Code config generation (step 3) and Codex config generation (step 4) can be implemented by parallel agents — they touch separate files (`claude_code.rs` vs `codex.rs`) with no overlap.

## Follow-ups

- **File-based MCP discovery for Codex**: Claude Code auto-discovers `.mcp.json` in cloned repos. Codex would need discovered configs translated to `config.toml` format.
- **Profile MCP servers**: `start_profile_mcp_servers` resolves UUID-based servers from the profile allowlist. Same resolution path as `task.mcp_specs` — could be included in a follow-up.
- **Gemini MCP support**: add when Gemini CLI supports MCP configuration.
