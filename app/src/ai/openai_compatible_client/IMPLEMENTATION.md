# OpenAI-Compatible Custom Endpoint

This module lets users point Warp at any server that implements the OpenAI Chat Completions API (Ollama, vLLM, LM Studio, etc.) and use it as an AI backend with tool support and conversation persistence.

## How It Works

```
User types a query
       |
       v
RequestParams::new() (api.rs:164)
  - if model_id starts with "custom:", resolves to OpenAiCompatibleEndpoint (api.rs:312)
       |
       v
generate_multi_agent_output() (impl.rs:12)
  - if openai_compatible_endpoint is set, routes to custom endpoint
  - otherwise, sends request to Oz server as normal
       |
       v
generate_openai_compatible_output() (mod.rs:32)
  - sends JSON request to user's endpoint
  - receives SSE stream back
  - emits: Init -> CreateTask -> UserQuery -> [text/tool chunks] -> Finished
       |
       v
External API (e.g. localhost:11434/v1/chat/completions)
```

## Why Conversion Exists

Warp's agent system uses protobuf for everything. The proto definitions come from an external repo (`warp-proto-apis`, pulled in via Cargo.toml:307). The generated Rust types live in `warp_multi_agent_api` and include things like `ResponseEvent`, `ClientAction`, `Message`, `UserQuery`, `AgentOutput`, `ToolCall`, etc. (defined in `response.proto`, `task.proto`, etc. in that repo).

When Warp talks to its own Oz server, the entire conversation is protobuf:
- The request body is protobuf bytes with `Content-Type: application/x-protobuf` (http_client/src/lib.rs:396, server_api.rs:1183)
- The SSE response is base64-encoded protobuf (server_api.rs:1207-1231, decoded via `BASE64_URL_SAFE.decode()` then `ResponseEvent::decode()`)

External OpenAI-compatible servers don't speak protobuf. They use JSON over SSE. So this module converts between the two formats.

### Outbound: Warp protobuf -> OpenAI JSON (`convert.rs::from_request_params`)

The user's conversation history is stored as protobuf `Message` objects in tasks. Before sending to an external API, `from_request_params()` converts them:

- `api::Message::UserQuery` -> `ChatMessage { role: "user" }`
- `api::Message::AgentOutput` -> `ChatMessage { role: "assistant" }`
- `api::Message::ToolCall` -> `ChatMessage { role: "assistant", tool_calls: [...] }` (with the tool name and arguments converted from Warp's proto tool types to OpenAI function-calling JSON)
- `api::Message::ToolCallResult` -> `ChatMessage { role: "tool" }` (with the result formatted as text)

Context that Warp normally sends to the Oz server (working directory, git branch, shell type) doesn't exist in the OpenAI API. It gets injected as a system message instead, so the model at least knows the environment.

### Inbound: OpenAI JSON -> Warp protobuf (`convert.rs::delta_to_response_events`)

The external API sends back SSE chunks in OpenAI format. Each chunk has a `delta` with either `content` (text) or `tool_calls` (function calls). These get converted back to protobuf:

- `delta.content` -> `api::Message::AgentOutput` wrapped in `AddMessagesToTask` client action (first chunk uses `make_text_client_action`, subsequent chunks use `make_append_text_client_action`)
- `delta.tool_calls` -> accumulated in `StreamingState`, then finalized into `api::Message::ToolCall` messages when the stream ends (via `finalize_tool_call_events`)

### Why UserQuery Has to Be Explicit

This is the part that caused a bug. In the normal Oz flow, the server adds `UserQuery` messages to the task as part of its response stream. When a conversation is restored from the database later, `into_exchanges()` (convert_conversation.rs:378) looks for `UserQuery` messages in the task to reconstruct what the user said.

Custom endpoints skip the Oz server entirely. They talk directly to the external API. So nobody was adding `UserQuery` messages to the task. The result: after restarting Warp, the AI could see its own responses but had no record of what the user asked. The fix is that `generate_openai_compatible_output` explicitly emits a `UserQuery` `AddMessagesToTask` event right after `CreateTask`, before the SSE stream starts (mod.rs:78-84, convert.rs:689-710).

## Key Design Decisions

**Feature-gated** - All runtime code is behind `#[cfg(not(target_family = "wasm"))]` and `FeatureFlag::OpenAiCompatibleEndpoints`. The data types in `crates/ai/src/openai_compatible.rs` are NOT gated because the settings system needs to serialize/deserialize them on all platforms. The feature flag is in DOGFOOD_FLAGS only (warp_features/src/lib.rs:949), so non-dev users must opt in at runtime.

**Streaming-only** - `stream: true` is hardcoded (convert.rs:446). The client uses `reqwest_eventsource` (mod.rs:87). There is no non-streaming code path. This matches how all major OpenAI-compatible servers work and is needed for real-time token display in the Warp UI.

**Tool call accumulation** - OpenAI streams tool calls in pieces: the function name comes in one chunk, then arguments arrive as partial JSON fragments across multiple chunks. `StreamingState` collects these fragments and only creates `ToolCall` messages when the stream ends (after `[DONE]`), so we never emit half-formed tool calls.

**Shared message helper** - `make_add_messages_client_action()` (convert.rs:665) is used by both `make_text_client_action` and `make_user_query_client_action` to wrap a protobuf `Message` in the `AddMessagesToTask` -> `ClientAction` -> `ClientActions` structure. This avoids duplicating the wrapping boilerplate.

**Secure API key storage** - API keys go to the OS keychain via `secure_storage`, keyed as `CustomEndpoint:{id}:api_key` (openai_compatible.rs:130). The settings TOML only stores a `has_api_key: bool` flag. The `api_key` field on the struct is `#[serde(skip)]` so it never gets written to TOML. There is a backward-compat path that reads a plain-text `api_key` from old TOML files and migrates it to secure storage on first use (openai_compatible.rs:46-81).

## Files

| File | What it does |
|------|-------------|
| `app/src/ai/openai_compatible_client/mod.rs` | SSE stream orchestration - builds the HTTP request, receives the SSE stream, parses chunks, emits proto events |
| `app/src/ai/openai_compatible_client/convert.rs` | All conversion logic between proto and OpenAI JSON, tool definitions, message construction |
| `crates/ai/src/openai_compatible.rs` | Data types for endpoint configuration (shared crate, used by both app and settings) |

## Integration Points

These files in the main codebase were modified to wire in the custom endpoint:

- `app/src/ai/agent/api.rs` - Resolves `custom:` model ID prefix to `OpenAiCompatibleEndpoint` in `RequestParams::new()`
- `app/src/ai/agent/api/impl.rs` - Routes to `generate_openai_compatible_output` when the endpoint is set
- `app/src/ai/llms.rs` - Registers custom endpoint models in the LLM picker, subscribes to settings changes to re-inject them
- `app/src/ai/blocklist/controller/response_stream.rs` - Handles the custom endpoint response stream path, disables resume-on-error for custom endpoints
- `app/src/ai/agent/conversation.rs` - Passes `openai_compatible_endpoint` through the conversation flow
- `app/src/settings/ai.rs` - Stores endpoint configuration (URL, models, enabled flag)
- `app/src/settings_view/ai_page.rs` - UI for adding, editing, and removing custom endpoints
- `crates/warp_features/src/lib.rs` - `OpenAiCompatibleEndpoints` feature flag definition
