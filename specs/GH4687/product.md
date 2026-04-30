# Product Spec: OpenAI-compatible BYOK endpoints

Issue: https://github.com/warpdotdev/warp/issues/4687
Related PR: https://github.com/warpdotdev/warp/pull/9253
Figma: none provided

## Summary

Users should be able to configure a custom OpenAI-compatible model endpoint for Warp Agent by entering a provider label, base URL, API key, and model ID. This gives users a narrow, predictable way to use OpenRouter, LiteLLM, internal OpenAI-compatible gateways, and local OpenAI-compatible servers without requiring Warp to add a bespoke provider integration for each one.

This spec is intentionally scoped to one generic "OpenAI-compatible" provider surface. It does not replace Warp-provided models, existing BYOK provider keys, enterprise model configuration, or future first-class provider integrations.

## Problem

Warp currently exposes BYOK for fixed providers and chooses models from server-provided model choices. That works when the desired provider and model are already known to Warp, but it leaves no self-service path for users who already have access to an OpenAI-compatible endpoint.

The common desired workflow is:

1. Use Warp's existing agent UX, permissions, execution profiles, and terminal context.
2. Point model requests at a compatible endpoint such as `https://openrouter.ai/api/v1`.
3. Provide an API key for that endpoint.
4. Enter the model ID used by that endpoint, such as `anthropic/claude-sonnet-4.5`.

PR #9253 is useful partial progress for OpenRouter-specific BYOK, but it still depends on Warp-approved model choices. The remaining user need is the generic endpoint contract from issue #4687.

## Goals

1. A user can add one custom OpenAI-compatible endpoint configuration from Settings > AI.
2. The configuration includes:
   - provider label
   - base URL
   - API key
   - model ID
3. The configured model appears in the existing model picker as a selectable BYOK model.
4. Selecting the model causes Warp Agent requests to use the configured endpoint and model ID.
5. The feature works for OpenRouter and other OpenAI-compatible endpoints that support Warp's required chat/agent request shape.
6. Existing OpenAI, Anthropic, Google, OpenRouter, AWS Bedrock, and Warp-credit model behavior remains unchanged.

## Non-goals

1. Fetching arbitrary provider model catalogs client-side.
2. Adding separate first-class UI for OpenRouter, LiteLLM, Ollama, Azure, or any other provider in this initial flow.
3. Supporting non-OpenAI-compatible protocols in this feature.
4. Guaranteeing every model behind a compatible endpoint supports all Warp Agent tools.
5. Changing the paid plan or workspace policy that gates BYOK access.
6. Changing where Warp Agent requests are executed or proxied beyond the existing BYOK request architecture.
7. Supporting multiple custom endpoint profiles in the first version. The UI and data model should not preclude this as a follow-up.

## Behavior

1. When BYOK is available for the current user or workspace, Settings > AI shows a section for "OpenAI-compatible endpoint" in addition to the existing provider API key inputs.
2. The section contains four inputs:
   - Label, defaulting to `OpenAI-compatible`
   - Base URL, for example `https://openrouter.ai/api/v1`
   - API key
   - Model ID, for example `anthropic/claude-sonnet-4.5`
3. Empty label uses the default label. Empty base URL, API key, or model ID means the custom endpoint is incomplete and should not appear as a selectable model.
4. Base URL validation is lightweight and user-facing: the value must parse as an absolute `http` or `https` URL. Warp does not perform a network validation request when the user saves the setting.
5. API key input uses the same password-style treatment as existing BYOK provider key fields.
6. The saved custom model appears in the existing model picker using the configured label and model ID. A key icon or equivalent BYOK affordance should make clear that it is billed to the user's endpoint credentials.
7. Selecting the custom model persists through the same execution profile mechanism as other model choices.
8. Agent requests for the custom model include the custom endpoint metadata needed by Warp's backend to route the request:
   - base URL
   - API key
   - model ID
   - provider label for display/diagnostics
9. If the endpoint returns an authentication error, the user sees an invalid API key/error state that names the configured provider label when available.
10. If the endpoint or model is unsupported by Warp's agent backend, the error should explain that the custom endpoint could not satisfy the request rather than asking the user to upgrade Warp credits.
11. Existing fixed-provider BYOK fields continue to work. Adding a custom endpoint does not clear or override OpenAI, Anthropic, Google, OpenRouter, or AWS Bedrock credentials.
12. Disabling BYOK at the workspace/plan level disables custom endpoint editing and selection using the same gating behavior as existing BYOK fields.

## Success criteria

1. A user can configure OpenRouter with:
   - Base URL: `https://openrouter.ai/api/v1`
   - Model ID: an OpenRouter model slug
   - API key: an OpenRouter key
2. The configured model can be selected in the model picker.
3. Agent requests with that model carry the custom endpoint config to the request layer.
4. Incomplete custom endpoint config does not create a broken model picker entry.
5. Existing BYOK provider-key behavior is unchanged.
6. Existing Warp-credit model behavior is unchanged.

## Open questions

1. Should the first implementation support exactly one custom endpoint, or should the persistence shape support a list immediately while the UI initially exposes one?
2. Should custom endpoint metadata live in the existing `api_keys` request payload or a distinct `custom_model_endpoint` request field?
3. Should the backend allow custom endpoint routing for all agent features immediately, or gate specific tool-heavy flows until compatibility is proven?
4. Should Warp add a preset button for OpenRouter after the generic flow lands, or keep the first version purely generic?
