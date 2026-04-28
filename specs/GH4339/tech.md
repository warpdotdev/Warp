# Local Ollama Model Support - Tech Spec
Product spec: `specs/GH4339/product.md`
GitHub issue: https://github.com/warpdotdev/warp/issues/4339

## Context
Warp's native local agent path currently builds a `warp_multi_agent_api::Request` in the client and streams the response from `ServerApi::generate_multi_agent_output`. The important boundary is `app/src/ai/agent/api/impl.rs`: `generate_multi_agent_output` converts client inputs into the multi-agent protobuf request, then unconditionally calls the Warp server API.

Relevant code:
- `app/src/ai/agent/api.rs` defines `RequestParams`, including the selected base model, coding model, CLI-agent model, computer-use model, API keys, tool support, autonomy, and session context.
- `app/src/ai/agent/api/impl.rs` builds the `warp_multi_agent_api::Request`, computes supported tools, applies secret redaction, and calls `ServerApi::generate_multi_agent_output`.
- `app/src/server/server_api.rs` implements `generate_multi_agent_output` by sending the protobuf request to Warp's AI endpoint and decoding streamed `ResponseEvent` values.
- `app/src/ai/blocklist/controller/response_stream.rs` owns the in-flight response stream model and already consumes an abstract `api::ResponseStream`, so a local provider can reuse downstream rendering if it emits the same event stream shape.
- `app/src/ai/agent/api/convert_to.rs` and `convert_from.rs` are the conversion boundary between app-side conversation structures and `warp_multi_agent_api`.
- `app/src/ai/llms.rs` stores `LLMInfo`, `LLMProvider`, `ModelsByFeature`, `AvailableLLMs`, and `LLMPreferences`. The model list is currently server-derived through `get_feature_model_choices` and cached in private user preferences.
- `app/src/server/server_api/ai.rs` converts GraphQL `LlmProvider` and model-choice data into client `LLMInfo`.
- `app/src/terminal/view/ambient_agent/model_selector.rs` and `app/src/terminal/input/models/data_source.rs` render model choices from `LLMPreferences`.
- `app/src/ai/execution_profiles/profiles.rs` owns active/default execution profiles and per-session model preferences.
- `app/src/settings_view/ai_page.rs` and related settings modules are the natural home for local model configuration UI.
- `app/src/ai/agent_sdk/driver/harness/*` is for third-party CLI harnesses and should not be used for this feature; Ollama support is model routing for Warp's native agent, not a Claude/Gemini-style CLI harness.

The first implementation should add a client-side generation provider abstraction without rewriting downstream conversation rendering. The local provider should translate between Warp's existing request/response stream shape and OpenAI-compatible streaming chat completions.

## Proposed Changes
### 1. Add persisted local model settings
Add an AI settings group for local model configuration. The stored data should include:
- `enabled: bool`
- `endpoint_url: String`, default `http://localhost:11434/v1`
- `model_ids: Vec<String>`
- `manually_added_model_ids: Vec<String>` or equivalent metadata to preserve user-entered IDs across refreshes
- an API-key reference stored through the existing local secret mechanism rather than plain preferences

Use the existing settings pattern in `app/src/settings` and Settings UI in `app/src/settings_view/ai_page.rs`. Emit a specific `AISettingsChangedEvent` or a sibling local-model settings event so model lists refresh without restarting.

Validation should happen before committing settings:
- parse as URL
- require `http` or `https`
- trim trailing slash for stable joining
- reject empty model IDs

### 2. Extend model metadata for local providers
Update `app/src/ai/llms.rs`:
- Add `LLMProvider::Ollama` or `LLMProvider::Local`.
- Add icon/display metadata that clearly marks the provider as local. A generic local icon is acceptable if no Ollama asset is available.
- Add enough metadata to distinguish local models from server-backed models. This can be a new field on `LLMInfo`, a provider predicate, or a model ID namespace such as `local:ollama:<model>`.

Recommended model ID strategy:
- Store user-facing model IDs as Ollama returns them, such as `qwen2.5-coder:7b`.
- Convert them to a stable internal ID before inserting into `LLMPreferences`, for example `local/ollama/qwen2.5-coder:7b`.
- Keep the raw provider model ID in provider metadata so request routing does not have to parse display strings.

### 3. Merge local models into `LLMPreferences`
`LLMPreferences` currently updates from `ServerApi::get_feature_model_choices` and cached `ModelsByFeature`. Extend it so local models are overlaid on the server model list when local model settings are enabled.

Implementation shape:
- Build `AvailableLLMs` entries for local models with zero credit multiplier, no upgrade disable reason, provider `Local/Ollama`, and conservative/default `LLMSpec` values.
- Add local models to `agent_mode` and any other local-only model lists that should support native agent mode.
- Do not add local models to cloud-only model choices.
- When settings change, recompute the overlaid `ModelsByFeature` and emit `LLMPreferencesEvent::UpdatedAvailableLLMs`.
- Ensure `get_active_base_model` falls back to a valid model if the selected local model disappears.

This keeps `ModelSelector` and slash-command model picker changes small because they already read `LLMPreferences`.

### 4. Add endpoint model discovery
Add a local provider client, for example `app/src/ai/local_models/ollama.rs`.

Initial API:
- `list_models(endpoint: Url, api_key: Option<Secret>) -> Result<Vec<LocalModelInfo>, LocalModelError>`
- call `GET {endpoint}/models`
- parse OpenAI-compatible model-list responses
- return high-level error categories for Settings UI
- use a short timeout, for example 5 seconds

Keep this client independent from UI so it can be unit-tested with a local HTTP test server.

### 5. Introduce an agent generation router
Replace the unconditional server call in `app/src/ai/agent/api/impl.rs` with a provider router:

```rust
pub async fn generate_multi_agent_output(
    server_api: Arc<ServerApi>,
    mut params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let request = build_multi_agent_request(&mut params)?;
    match AgentGenerationProvider::for_model(&params.model, app_or_settings) {
        AgentGenerationProvider::WarpCloud => server_generate(server_api, request, cancellation_rx).await,
        AgentGenerationProvider::LocalOllama(config) => {
            local_ollama_generate(config, request, cancellation_rx).await
        }
    }
}
```

`generate_multi_agent_output` currently does not receive `AppContext`, so the real design needs one of:
- add provider-routing data to `RequestParams` at construction time
- pass an immutable provider registry/config into `generate_multi_agent_output`
- split request construction from dispatch so `ResponseStream::new` can choose the provider before spawning

The lowest-risk option is to add a routing enum to `RequestParams` when `RequestParams::new` already has `AppContext` and selected model information.

### 6. Split request construction from server dispatch
Move the request-building logic in `app/src/ai/agent/api/impl.rs` into a helper that returns `warp_multi_agent_api::Request`. Keep existing tests around tool support and settings, then add routing tests.

Suggested helpers:
- `build_multi_agent_request(params: &mut RequestParams) -> Result<api::Request, ConvertToAPITypeError>`
- `generate_via_warp_cloud(server_api, request, cancellation_rx) -> Result<ResponseStream, ConvertToAPITypeError>`
- `generate_via_local_provider(local_config, request, cancellation_rx) -> Result<ResponseStream, ConvertToAPITypeError>`

This isolates the existing cloud behavior and makes the local path explicit.

### 7. Implement a local harness adapter
The hard part is not the HTTP call to Ollama; it is preserving Warp's agent loop. Warp's server currently acts as the model/harness that emits `ResponseEvent` values containing assistant messages and tool calls. The client must own that loop for local models.

Add a local agent runner module, for example `app/src/ai/local_agent_runner/`.

Responsibilities:
- Convert `warp_multi_agent_api::Request` conversation input into OpenAI-compatible chat messages.
- Add the system/developer instructions required for Warp tools.
- Advertise supported tools as OpenAI-compatible tool definitions.
- Stream assistant deltas back as `warp_multi_agent_api::ResponseEvent` message chunks.
- Detect tool calls, emit the same client actions the cloud harness emits, and pause until existing tool execution returns results through the conversation controller.
- Continue the loop by sending tool results back to the local model.
- Emit a final `StreamFinished` with local usage metadata when available.

The first implementation can support a bounded single-agent loop and explicitly disable orchestration/subagent features for local models until a local multi-agent planner exists.

### 8. Capability gating
Before enabling a local model for native agent mode, check endpoint/model capabilities:
- streaming chat completions
- tool calls/function calls
- enough context length for the selected request, if the endpoint exposes it

If capabilities are unknown, allow the model to be configured but run a preflight on first use. Disable or fail gracefully if required features are missing.

For the first PR, local models should set `supported_tools_override` to a conservative subset:
- read files
- grep/glob
- run shell command
- read shell command output
- apply file diffs, if the model passes tool-call preflight

Keep computer use, orchestration v2, research agent, web search, and cloud child agents disabled for local provider routing until separately designed.

### 9. Preserve data-boundary guarantees
Audit local routing to ensure content-bearing data does not reach Warp generation APIs:
- no call to `ServerApi::generate_multi_agent_output`
- no prompt/tool payload in telemetry
- no transcript upload as a side effect of local generation
- no cloud ambient task creation for local-model runs

Non-content existing app calls can remain unchanged, but the local model path should be testable by injecting a mock `ServerApi` that fails if generation is called.

### 10. UI and UX updates
Settings:
- Add local model controls to AI settings.
- Add refresh, manual add/remove, status/error, and selected endpoint display.
- Store API key through the existing secret entry component if possible.

Model pickers:
- Show local models with a local/Ollama indicator.
- Hide credit pricing and BYOK key prompts for local models.
- Disable local models in cloud-only contexts with clear tooltip copy.

Error states:
- endpoint unreachable
- model not found
- invalid endpoint response
- missing streaming/tool support
- local generation cancelled

### 11. Conversation history and usage
For local conversations:
- Persist enough local history for restore/retry in the same surfaces used by local Warp agent conversations.
- Mark usage as provider-local and credit-free.
- Do not depend on server conversation tokens for local-only turns.

If existing history models require a server conversation token, introduce a local conversation token/ID variant rather than fabricating a server token.

## Testing and Validation
### Unit tests
- `app/src/ai/llms` tests: local provider metadata serializes/deserializes, overlaid models appear only when enabled, fallback selection works when a local model is removed.
- local settings tests: endpoint URL validation, model ID validation, secret omission from exported settings.
- local provider client tests: parses `GET /models`, handles unreachable/401/invalid JSON/timeouts.
- `app/src/ai/agent/api/impl_tests.rs`: cloud model routes to `ServerApi`; local model does not call `ServerApi::generate_multi_agent_output`; supported tool set is reduced for local provider.
- local runner tests: converts a simple user query into OpenAI-compatible messages; converts assistant text deltas into `ResponseEvent`; converts one tool call into the expected client action; converts tool results back into chat messages.

### Integration tests
- Configure a fake OpenAI-compatible local server, select a local model, submit a simple prompt, and assert the conversation renders streamed text.
- Configure a fake local server that requests a read-file tool, assert Warp shows the existing permission flow and returns the tool result to the local runner.
- Start with the network offline or with Warp server generation mocked unavailable, keep the fake local endpoint reachable, and assert a local-model conversation still completes.
- Select a local model, attempt to start a cloud ambient agent run, and assert the cloud-only surface blocks local selection.

### Manual validation
- Run Ollama locally with a tool-capable model. Configure `http://localhost:11434/v1`, refresh models, select one, and complete a basic local agent prompt.
- Disconnect from the internet while leaving Ollama running and verify a local agent prompt still works.
- Shut down Ollama and verify the recoverable endpoint error appears.
- Choose a model that does not support tool calls and verify Warp disables it or fails before running tools.
- Confirm cloud model behavior, BYOK behavior, and usage/credits UI are unchanged for cloud-backed models.

## Risks and Mitigations
### Risk: OpenAI-compatible endpoints differ in tool-call behavior
Mitigation: gate native agent mode on a preflight and keep the first release conservative. Document unsupported endpoints/models in the error state.

### Risk: local model quality creates unsafe tool calls
Mitigation: keep execution profiles and permission gates unchanged. Local models must pass through the same approval and denylist paths as cloud models.

### Risk: accidental data leakage through fallback
Mitigation: do not fall back from local to cloud automatically. If local generation fails, the user must explicitly choose a cloud model before Warp sends content to Warp's generation service.

### Risk: conversation history assumes server conversation tokens
Mitigation: introduce a local conversation identifier rather than overloading `ServerConversationToken`. Add tests that local conversations do not call server transcript APIs.

### Risk: implementation becomes a full duplicate of the server harness
Mitigation: split the first implementation into a minimal local single-agent runner and explicitly disable orchestration/research/computer-use features until they are separately ported.

## Follow-ups
- Support additional OpenAI-compatible local providers such as LM Studio, MLX server, llama.cpp server, and vLLM.
- Add workspace-scoped local model profiles.
- Add optional explicit sync/export for local-model conversation metadata.
- Support local routing for remote SSH sessions with a user-selected endpoint location.
- Consider ACP as an alternate local harness path if Warp chooses to delegate the agent loop to local agent runtimes instead of embedding it in the client.
