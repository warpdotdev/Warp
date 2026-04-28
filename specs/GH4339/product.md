# Local Ollama Model Support - Product Spec
GitHub issue: https://github.com/warpdotdev/warp/issues/4339
Figma: none provided

## Summary
Add first-class local model support for Warp's built-in agent experience, starting with Ollama-compatible local endpoints. Users can configure an Ollama server, choose locally served models in the existing model picker, and run normal local Warp agent conversations without sending prompt text, terminal context, command output, file contents, or tool results to Warp's model servers.

This spec prioritizes the maintainer feedback requested in issue #4339: fully local execution, offline access, and preserving Warp's native agent UX. Ollama is the initial provider because it is widely used for local inference and exposes an OpenAI-compatible API surface, but the configuration should not hard-code assumptions that prevent future support for LM Studio, MLX server, llama.cpp server, or any other OpenAI-compatible local endpoint.

## Problem
Warp's current native agent mode depends on Warp-hosted model and harness infrastructure. That makes the feature unavailable or unacceptable for users who cannot send terminal history, code, credentials-adjacent output, or internal project context to external services. The current BYO API key flow still routes through Warp's hosted agent path and does not satisfy the strongest user requirement in #4339: data should remain on the local machine.

Users already run local models through Ollama and similar tools, but Warp cannot target them from the native agent UX. The workaround is to run a separate CLI agent or editor, losing Warp's terminal-integrated conversation UI, permissions, code review surfaces, and model picker.

## Goals
- Let users add one local Ollama-compatible endpoint from Settings.
- Let users discover or manually enter local model IDs exposed by that endpoint.
- Show configured local models in Warp's existing agent model picker.
- Route local agent-mode requests to the configured endpoint from the client process, without sending prompt contents or agent tool payloads to Warp-hosted model servers.
- Preserve the native local agent UX: streaming messages, tool calls, file reads, command execution, diffs, permissions, cancellation, and conversation history.
- Work offline after configuration when the Ollama server and selected model are available locally.
- Keep cloud models and BYO API key behavior unchanged for users who do not configure local models.
- Make data-boundary behavior explicit in the UI so users can tell whether a selected model is local or cloud-backed.

## Non-goals
- Shipping a bundled model runtime or downloading models from Warp.
- Supporting cloud ambient agent runs on local endpoints. Local models run only on the user's machine.
- Supporting remote/shared-session participants by relaying local model prompts through Warp servers.
- Guaranteeing that every Ollama model has enough context length or tool-calling quality for every Warp agent workflow.
- Adding arbitrary provider-specific APIs beyond the OpenAI-compatible chat-completions surface in the first iteration.
- Replacing Warp's cloud model list, subscription, usage, or credit accounting for cloud-backed models.
- Changing third-party CLI agent harnesses such as Claude Code, Gemini CLI, Codex, or OpenCode.

## Behavior
1. Settings exposes a **Local models** section under AI settings.

2. The section has an enable switch. When disabled, local models do not appear in model pickers and no local endpoint checks run.

3. When enabled, the user can configure:
   - endpoint URL, defaulting to `http://localhost:11434/v1`
   - optional API key, default empty
   - a list of model IDs

4. The endpoint URL accepts only `http://` or `https://` URLs. Empty, malformed, or non-HTTP URLs are rejected before save. Warp classifies the endpoint before saving:
   - loopback endpoints (`localhost`, `127.0.0.0/8`, `::1`) are treated as **local machine** endpoints and may use `http://`
   - private-network endpoints (RFC1918 IPv4, link-local addresses, and unique-local IPv6) are treated as **network-local** endpoints and must show a warning that prompts and optional API keys will leave this device for another host on the user's network
   - public endpoints are treated as **remote** endpoints, must use `https://`, and require an explicit warning/confirmation before Warp sends prompts or API keys to them

5. Cleartext `http://` is allowed by default only for loopback endpoints. `http://` private-network endpoints require explicit user confirmation that the endpoint is not encrypted. `http://` public endpoints are rejected.

6. The optional API key is stored using the same local secret-storage expectations as existing BYO API keys. If no API key is set, local requests omit authorization headers. Warp must show the endpoint locality label before saving an API key so users can see where that key will be sent.

7. The user can click **Refresh models** to fetch the endpoint's model list. For Ollama's OpenAI-compatible API this uses `GET /models` relative to the configured base URL.

8. If refresh succeeds, the returned model IDs replace the local model list after user confirmation when the replacement would remove currently selected local models.

9. If refresh fails, Settings shows a concise error that includes the failed endpoint and the high-level reason: unreachable, unauthorized, invalid response, timeout, or TLS error.

10. Users can also add model IDs manually. Manually added IDs are preserved across refreshes unless the user removes them.

11. Configured local models appear in every model picker that controls local agent-mode requests. Each local model row uses a local/provider indicator and does not show Warp credit pricing.

12. Local models are excluded from cloud-only surfaces, including cloud ambient agent launch and schedule flows. If a cloud-only surface currently reads the same active model preference, it must either filter local models or show a blocking message explaining that local models only run on this machine.

13. Selecting a local model updates the same per-terminal/default profile preference that cloud model selection uses today, so the selected model remains stable across tabs and app restarts.

14. Starting a normal local agent conversation with a local model keeps the same conversation UI as cloud-backed local agent mode: streaming assistant output, tool cards, permission prompts, command execution, diffs, cancellation, and retry surfaces.

15. For local-model conversations, Warp sends no prompt text, terminal context, command output, file contents, tool inputs, tool results, screenshots, or local conversation transcript to Warp's model-generation service.

16. Non-content service calls that are unrelated to generation, such as auth state, feature flags, update checks, or cloud Drive metadata sync, are unchanged. The product copy must not imply that the entire app becomes air-gapped unless offline mode is separately implemented for those systems.

17. If the app is offline and the selected local endpoint is reachable, starting and continuing local-model agent conversations still works.

18. If the local endpoint is unreachable when the user submits a prompt, the conversation shows a recoverable error with actions to retry and open Local model settings.

19. If the endpoint returns a model-not-found error, Warp shows a recoverable error naming the selected model and offering to refresh the model list.

20. If the endpoint does not support a capability Warp requested, such as tool calling or streaming, Warp fails before running tools and explains which local-model capability is missing.

21. The first release requires streaming chat completions and tool-call compatible responses. Models/endpoints that do not support those capabilities are allowed to be listed but disabled for native agent mode with a tooltip explaining why.

22. The local agent path respects the user's existing execution profile permissions. A local model cannot bypass file-read, command-execution, MCP, or code-diff approval gates.

23. The local agent path applies the same secret redaction setting currently used before agent requests. Redaction remains defense in depth; local routing does not disable it.

24. Conversation history for local-model conversations is stored locally in the same local conversation/history surfaces as comparable Warp agent conversations. In the first release, local-model conversation transcripts are not uploaded to Warp Drive and are not eligible for cross-device transcript sync. If a future version adds opt-in sync, it must require explicit user consent before uploading local-model transcript content.

25. If the user shares a session while a local-model conversation is running, remote participants can see the terminal/session state that session sharing already exposes, but the local model request payload itself is not sent to Warp's generation service.

26. Usage and billing UI distinguishes local model usage from Warp-credit model usage. Local model turns show no Warp credit charge.

27. Telemetry for local-model usage may record non-content events such as provider type, endpoint locality class, success/failure category, latency buckets, and capability checks. It must not include prompt text, terminal context, file contents, command output, tool payloads, API keys, endpoint paths/query strings, or model responses.

28. Existing users with no local models configured see no behavior change.

29. Existing users with cloud models selected continue to use the current server-backed path.

30. Removing the selected local model from Settings does not automatically route future prompts to cloud generation. The next prompt is blocked until the user explicitly selects another local model or confirms switching that terminal/profile back to a cloud model.

31. Importing/exporting settings includes non-secret local model configuration, but not the local endpoint API key.

## Open Questions
- Should the first version support only OpenAI-compatible endpoints, or expose an Ollama-native adapter for better model discovery and error reporting?
- Should local model configuration be global per user, per workspace, or both?
- Which minimum tool-calling contract should Warp require from local endpoints before enabling native agent mode?
- Should local endpoints be allowed for remote SSH sessions, and if so, should requests run on the local machine, the remote host, or a user-selected host?
