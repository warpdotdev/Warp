# Warp Shim Server

`warp-shim-server` is a local control-plane shim for the OSS Warp build. It accepts the Warp AI protobuf/SSE protocol, forwards Agent Mode turns to a user-controlled OpenAI-compatible `/chat/completions` upstream, and returns harmless local stubs for non-AI Warp control-plane surfaces.

## Smoke-test launch

Start the shim:

```bash
cargo run -p warp_shim_server --bin warp-shim-server -- --port 4444 --upstream-url http://127.0.0.1:11434/v1
```

Start OSS Warp pointed at the shim. The launch command must include `WARP_API_KEY=local-shim` (or any value):

```bash
WARP_API_KEY=local-shim \
WARP_SERVER_ROOT_URL=http://127.0.0.1:4444 \
WARP_WS_SERVER_URL=ws://127.0.0.1:4444/graphql/v2 \
cargo run -p warp --bin warp-oss
```

After this patch, OSS builds honor `WARP_API_KEY` and skip the login modal.

`WARP_WS_SERVER_URL` is required because Warp derives RTC HTTP endpoints from the WebSocket URL.

## Manual GUI smoke test

1. Build/run an OpenAI-compatible local model server, for example Ollama or vLLM.
2. Run the shim with the command above.
3. Run `warp-oss` with the environment command above.
4. Open Agent Mode and send `hello`; verify a streaming assistant reply appears.
5. Send `list files in /` or another safe read-only prompt; verify the model requests a client-executed tool and then continues after the tool result.

## Configuration

CLI values override environment variables, which override TOML, which override defaults.

Supported environment fallbacks:

- `WARP_SHIM_CONFIG`
- `WARP_SHIM_HOST`
- `WARP_SHIM_PORT`
- `WARP_SHIM_UPSTREAM_URL`
- `WARP_SHIM_API_KEY`
- `WARP_SHIM_API_KEY_ENV`
- `WARP_SHIM_MODEL_MAP` (comma-separated, for example `auto=llama3.1,cli-agent-auto=llama3.1`)

Config lookup order:

1. `--config <path>`
2. `WARP_SHIM_CONFIG`
3. `./warp-shim.toml`
4. `~/.warp-shim/config.toml`

Example TOML:

```toml
[server]
host = "127.0.0.1"
port = 4444
public_base_url = "http://127.0.0.1:4444"

[upstreams.default]
base_url = "http://127.0.0.1:11434/v1"
api_key_env = "OPENAI_API_KEY"
timeout_secs = 180
streaming = true

[models]
auto = { upstream = "default", model = "llama3.1" }
"cli-agent-auto" = { upstream = "default", model = "llama3.1" }
"computer-use-agent-auto" = { upstream = "default", model = "llama3.1" }
"coding-auto" = { upstream = "default", model = "llama3.1" }

[features]
tools_enabled = true
mcp_tools_enabled = true
passive_suggestions_enabled = true
```

You can also pass model mappings directly:

```bash
cargo run -p warp_shim_server --bin warp-shim-server -- \
  --upstream-url http://127.0.0.1:11434/v1 \
  --model auto=llama3.1 \
  --model cli-agent-auto=llama3.1
```

Secrets are never printed by the shim; startup logs only report whether an upstream API key is configured.

## Implemented local surfaces

- `POST /ai/multi-agent` OpenAI-compatible bridge with client-executed tool loops
- `POST /ai/passive-suggestions` successful no-op stream
- `POST /graphql/v2?op=GetFeatureModelChoices`
- `POST /graphql/v2?op=FreeAvailableModels`
- `POST /graphql/v2?op=GetUser`
- `POST /graphql/v2?op=GetUserSettings`
- `POST /graphql/v2?op=GetWorkspacesMetadataForUser`
- `POST /client/login`
- `POST /proxy/token` and `POST /proxy/customToken`
- `GET /api/v1/agent/events/stream`
- `POST /api/v1/agent/run`, `GET /api/v1/agent/runs`, and `GET /api/v1/agent/runs/:id`
- `POST /api/v1/harness-support/*` minimal no-op responses
- Browser auth pages such as `/login/*`, `/signup/*`, `/upgrade`, `/login_options/*`, and `/link_sso`

Unknown GraphQL operations return `{"data":null}` and are logged with the operation name, request field names, and whether auth was present. Unknown non-telemetry paths return `404 {"error":"warp-shim: unsupported endpoint"}` and are logged with method/path.

## Tool support

The shim declares only tools that it can convert safely and only when the client lists them in `request.settings.supported_tools` (an empty list follows the protobuf contract and is treated as no override). Supported Warp tools are `RunShellCommand`, `WriteToLongRunningShellCommand`, `ReadShellCommandOutput`, `ReadFiles`, `ApplyFileDiffs`, `SearchCodebase`, `Grep`, `FileGlob`, `FileGlobV2`, and `ReadMcpResource`.

MCP tools are generated from `request.mcp_context.servers[].tools` when `CallMcpTool` is client-supported. The upstream OpenAI tool name is `mcp__<sanitized_server_id>__<sanitized_tool_name>`, while the emitted Warp tool call preserves the original MCP `server_id` and tool `name`. MCP schemas are normalized to OpenAI-compatible object schemas before they are sent upstream.

Explicitly unsupported tools such as `StartAgent`, `Subagent`, and `UseComputer` are not declared upstream.

## Troubleshooting

- **Metal toolchain build errors:** some Warp crates require Apple’s Metal toolchain. Install it with Xcode’s Metal Toolchain component if unrelated Warp crates fail while building `warp-oss`.
- **Paste your auth token modal:** if you see a "Paste your auth token" modal at startup, you're missing `WARP_API_KEY=local-shim`.
- **Port conflicts:** if `4444` is busy, start the shim with another `--port` and update both `WARP_SERVER_ROOT_URL` and `WARP_WS_SERVER_URL` to match.
- **Model mapping mismatches:** if the upstream rejects `model: "auto"`, pass `--model auto=<upstream-model>` (and optionally mappings for `cli-agent-auto`, `coding-auto`, and `computer-use-agent-auto`).
- **BYOK confusion:** Warp BYOK settings do not route traffic for this shim. The shim’s `--upstream-url`, `--api-key`, `--api-key-env`, and model mappings control upstream routing.
- **No streaming reply:** confirm the upstream implements OpenAI-compatible streaming SSE at `/v1/chat/completions`. If it only supports non-streaming responses, set `streaming = false` in TOML.

## Limitations

- No Warp cloud agents; cloud-agent endpoints are local unsupported/no-op stubs.
- No Warp Drive, sharing, team collaboration, cloud conversation storage, billing, or telemetry.
- Conversation and pending tool-call state are in memory and are lost when the shim restarts.
- Passive suggestions intentionally return a successful no-op stream. This avoids client errors but does not surface suggestion UI content.
- Tool coverage is conservative; unsupported Warp tools are omitted rather than translated incorrectly.
- OpenAI-compatible servers vary in tool-call support and JSON schema strictness.

## Privacy warning

Terminal context, prompts, command output, file snippets, and tool results may be sent to the configured upstream model. Be deliberate about which model/server you point the shim at, especially when working in sensitive repositories or terminals.

## Security warning

The shim binds to `127.0.0.1` by default. Do **not** expose it to a network interface or the public internet. It accepts local Warp traffic and is not hardened as a public service.
