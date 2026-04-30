# Tech Spec: OpenAI-compatible BYOK endpoints

Issue: https://github.com/warpdotdev/warp/issues/4687
Product spec: `specs/GH4687/product.md`

## Context

Warp already has the main pieces needed for provider-specific BYOK:

- secure local credential storage
- Settings > AI provider key inputs
- server-provided model choices
- model-picker BYOK affordances
- request-time API key payloads sent with Warp Agent requests

The missing piece for issue #4687 is a custom endpoint model entry that carries a user-provided base URL and model ID, instead of requiring the model to exist in Warp's server-approved model list.

Relevant code in the current client:

- `crates/ai/src/api_keys.rs:20` defines the locally persisted BYOK key shape. It currently contains fixed provider slots, including `google`, `anthropic`, `openai`, and `open_router`.
- `crates/ai/src/api_keys.rs:120` builds the `warp_multi_agent_api::request::settings::ApiKeys` payload for agent requests and returns `None` when no request credentials are present.
- `app/src/settings_view/ai_page.rs:6274` defines `ApiKeysWidget`, the Settings > AI widget that renders fixed provider key editors.
- `app/src/settings_view/ai_page.rs:6417` renders each API key input, and `app/src/settings_view/ai_page.rs:6454` adds the existing OpenAI, Anthropic, and Google inputs.
- `app/src/ai/llms.rs:28` marks a model as using a user API key when BYOK is enabled and the model provider has a matching stored key.
- `app/src/ai/llms.rs:87` defines fixed client-side `LLMProvider` variants.
- `app/src/terminal/input/models/data_source.rs:224` builds model picker rows, clears upgrade disablement for BYOK-capable provider models, and renders BYOK/manage affordances.
- `app/src/terminal/input/models/data_source.rs:494` limits the "bring your own key" upsell to fixed providers.
- `app/src/ai/agent/api.rs:156` creates `RequestParams` for Warp Agent requests.
- `app/src/ai/agent/api.rs:237` pulls BYOK request credentials from `ApiKeyManager`.
- `app/src/ai/agent/api/impl.rs:59` serializes the final `warp_multi_agent_api::Request`, including selected model IDs and request API keys.
- `crates/warp_graphql_schema/api/schema.graphql:1913` lists server `LlmProvider` enum values used for server-provided models.
- `crates/warp_graphql_schema/api/schema.graphql:1927` already has `LlmSettingsInput` fields such as `apiKey` and `baseUrl` for workspace-level LLM settings, but the client workspace conversion currently stores only host-level enablement in `app/src/workspaces/workspace.rs:619`.

## Proposed changes

### 1. Add a custom endpoint config type

Add a client-owned settings type to `crates/ai/src/api_keys.rs`:

```rust
pub struct OpenAICompatibleEndpoint {
    pub label: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_id: Option<String>,
}
```

Store it alongside existing provider keys in `ApiKeys`:

```rust
pub openai_compatible_endpoint: Option<OpenAICompatibleEndpoint>
```

Implementation notes:

- Keep this in secure storage with the other BYOK credentials because it includes an API key.
- Treat `label` as optional; the display layer can default to `OpenAI-compatible`.
- Add helper methods such as `is_complete()` and `display_label()` to centralize validation.
- Consider a vector shape later, but keep the initial UI and request payload to one endpoint unless maintainers prefer a list immediately.

### 2. Render the endpoint editor in Settings > AI

Extend `ApiKeysWidget` in `app/src/settings_view/ai_page.rs` with editors for:

- label
- base URL
- API key
- model ID

Use the existing single-line editor pattern from `create_api_key_editor!` for consistency. The API key field should stay password-style; label, base URL, and model ID should be plain text inputs.

Save behavior:

- On blur or Enter, persist the full config via `ApiKeyManager`.
- Empty base URL, API key, or model ID keeps the config incomplete.
- If BYOK is disabled for the workspace, clear/disable the endpoint editors using the same `UserWorkspacesEvent::TeamsChanged` handling as the fixed provider key fields.

Validation:

- Parse base URL with `url::Url`.
- Accept only `http` and `https` schemes.
- Do not send a validation request to the provider while saving settings.

### 3. Add a synthetic custom model choice

Warp's current model picker is built around `LLMInfo` choices. Add a synthetic `LLMInfo` when the custom endpoint config is complete.

Recommended shape:

- `id`: the configured model ID, preferably with a stable custom prefix if the backend needs to distinguish custom endpoint models from server-known models.
- `display_name`: configured label plus model ID, for example `OpenRouter: anthropic/claude-sonnet-4.5`.
- `base_model_name`: configured model ID.
- `provider`: either a new `LLMProvider::OpenAICompatible` client variant or `LLMProvider::Unknown` plus a separate custom-model marker.
- `disable_reason`: `None` when BYOK is enabled and the config is complete.
- `host_configs`: default/direct host configuration unless the backend requires a distinct custom host.

The least surprising model-picker behavior is to inject the synthetic choice at the `LLMPreferences` boundary where server-provided model choices are already cached and exposed to the UI. If maintainers prefer to keep `LLMPreferences` server-only, the alternative is to append the custom row in `app/src/terminal/input/models/data_source.rs`, but that risks duplicating model-selection behavior across surfaces.

### 4. Extend request payloads with endpoint metadata

The selected model ID alone is not enough; the request layer must also know base URL and API key. There are two viable server contracts:

Option A: extend `warp_multi_agent_api::request::settings::ApiKeys`:

```protobuf
message OpenAICompatibleEndpoint {
  string label = 1;
  string base_url = 2;
  string api_key = 3;
  string model_id = 4;
}
```

and include it as an optional field under `ApiKeys`.

Option B: add a distinct request settings field:

```protobuf
message CustomModelEndpoint {
  string provider_label = 1;
  string base_url = 2;
  string api_key = 3;
  string model_id = 4;
}
```

and send it independently from fixed provider keys.

Option B is clearer because this is not only an API key; it is routing metadata. It also avoids overloading fixed provider-key semantics.

In the client:

- Extend `RequestParams` in `app/src/ai/agent/api.rs` with optional custom endpoint metadata.
- Populate it from `ApiKeyManager` only when the active model is the custom endpoint model.
- Serialize it in `app/src/ai/agent/api/impl.rs` alongside existing `settings.model_config` and `settings.api_keys`.

### 5. Error handling and display

Existing invalid-key handling maps provider names for fixed providers in `app/src/ai/blocklist/controller.rs`. Add a custom endpoint path that can show the configured label when available.

Expected user-facing behavior:

- Authentication failure: "Invalid API key for <label>".
- Provider/model failure: endpoint-specific error that names the configured label/model ID when possible.
- Plan/BYOK disabled: existing BYOK upgrade/disabled behavior.

### 6. Keep #9253 compatible

If #9253 lands first, keep its OpenRouter fixed-provider support as a separate convenience path. The generic endpoint should not depend on `LLMProvider::OpenRouter` or server-approved OpenRouter model IDs.

If maintainers prefer to avoid both surfaces, #9253 can become a preset over the generic endpoint:

- label: `OpenRouter`
- base URL: `https://openrouter.ai/api/v1`
- model ID: user-entered
- API key: OpenRouter key

## End-to-end flow

```mermaid
sequenceDiagram
    participant User
    participant Settings as Settings > AI
    participant Keys as ApiKeyManager
    participant Picker as Model picker
    participant Agent as Agent request builder
    participant Server as Warp backend
    participant Provider as Compatible endpoint

    User->>Settings: Enter label, base URL, API key, model ID
    Settings->>Keys: Persist secure custom endpoint config
    Picker->>Keys: Read complete endpoint config
    Picker-->>User: Show custom BYOK model row
    User->>Picker: Select custom model
    Agent->>Keys: Read endpoint config for active model
    Agent->>Server: Send model ID + endpoint metadata
    Server->>Provider: Route OpenAI-compatible request
    Provider-->>Server: Model response
    Server-->>Agent: Stream Warp Agent response events
```

## Testing and validation

Product behavior mapping:

1. Settings editor renders the four custom endpoint fields when BYOK is available.
   - Add settings-view tests if an existing harness covers `ApiKeysWidget`; otherwise validate manually in Settings > AI.
2. Incomplete config does not produce a model picker entry.
   - Unit test `OpenAICompatibleEndpoint::is_complete()`.
   - Unit test synthetic model injection/filtering.
3. Complete config produces one model picker entry with the expected display label and model ID.
   - Unit test the model source or `LLMPreferences` injection point.
4. Selecting the custom model causes request params to include endpoint metadata.
   - Unit test `RequestParams::new` or a focused helper that decides whether a selected model matches the custom endpoint.
5. Existing provider-key behavior is unchanged.
   - Existing `ApiKeyManager::api_keys_for_request` behavior should keep returning fixed provider keys.
   - Add regression coverage that OpenAI/Anthropic/Google/OpenRouter keys are not cleared when custom endpoint config changes.
6. Base URL validation rejects invalid or non-HTTP(S) values.
   - Unit test accepted examples:
     - `https://openrouter.ai/api/v1`
     - `http://localhost:11434/v1`
   - Unit test rejected examples:
     - `not a url`
     - `file:///tmp/model`

Manual validation after implementation:

- Configure OpenRouter with `https://openrouter.ai/api/v1`, an OpenRouter API key, and a known model ID.
- Select the custom model in an execution profile.
- Send a simple Warp Agent prompt.
- Confirm the request uses the custom endpoint path and does not consume Warp credits unless the configured fallback explicitly does so.

## Risks and mitigations

- Risk: custom endpoints may not support the complete Warp Agent protocol or tool expectations.
  Mitigation: scope the first version to OpenAI-compatible chat/model routing and return clear provider/model errors when the endpoint cannot satisfy a request.
- Risk: users may expect local-only execution.
  Mitigation: copy should say this follows Warp's BYOK request flow and should not promise local-only routing unless the backend/client architecture changes.
- Risk: model IDs collide with server-provided IDs.
  Mitigation: use an internal custom model prefix or a separate marker to distinguish custom endpoint selections.
- Risk: storing routing metadata in `ApiKeys` overloads a fixed-provider key store.
  Mitigation: keep the initial secure-storage location for safety, but model the endpoint as a distinct typed config and prefer a distinct request field.
- Risk: #9253 and the generic endpoint surface duplicate OpenRouter UX.
  Mitigation: keep the generic flow as the base capability and treat OpenRouter-specific UI as a preset or convenience layer.

## Follow-ups

- Multiple saved custom endpoint profiles.
- Optional OpenRouter preset that pre-fills the base URL.
- Optional model catalog import for endpoints that expose a compatible `/models` endpoint.
- Per-profile custom endpoint selection if execution profiles need separate endpoint configs.
