# Machine-Readable Export for Agent Conversations (GH-10112)

## Summary

Warp's agent conversations can already be exported to Markdown, but Markdown is intended for human reading: tool calls, plan steps, reasoning blocks, and structured tool I/O all collapse into prose. Scripts and external tools that want to consume agent transcripts have to parse rendered Markdown, which is brittle. This spec adds a JSON export option to the existing Export Conversation flow so transcripts are losslessly machine-readable. The schema is versioned and stable, redaction-aware, and round-trips messages, tool calls, plans, reasoning, code diffs, and images.

## Problem

Users (engineers building eval harnesses, dataset pipelines, internal tooling, post-mortem analyses, and replay viewers) need a structured local export of agent conversations. Markdown export loses fidelity:

- Tool calls render as code blocks; input vs output structure is gone.
- Plan steps render as bullet lists; status state is lost.
- Reasoning blocks render as quoted text; duration is lost.
- Code diffs render as unified-diff fences; per-file structure is lost.

The fix is a versioned JSON export alongside the existing Markdown export — same UI entry point, parallel CLI flag, same redaction guarantees.

## Goals

- Add JSON to the existing "Export Conversation" UI as a peer of Markdown.
- Versioned JSON schema (`schema_version` semver) stable across Warp versions.
- Full round-trip fidelity for messages, tool calls (input + output), plan steps with status, reasoning blocks with duration, code diffs with per-file structure, and inline images.
- Honors existing redaction rules — secrets/PII already stripped from the rendered conversation stay stripped in the JSON.
- Optional truncation for large tool outputs, off by default, with `truncated: true` and `original_size_bytes` flags so consumers know what happened.
- CLI parity with the UI export.

## Non-Goals

- Not a sync, replication, or live-streaming API. One-way local file export.
- Not a server-hosted export endpoint.
- Not auto-export per turn — manual user action only.
- Not bulk-export-all-conversations in V1.
- Not a re-import / restore-from-export feature in V1 (the schema enables it; UI does not yet).

## Behavior Contract

### B1. UI entry point

The existing "Export Conversation" UI gains a format selector with two options: `Markdown` (existing, default behavior preserved) and `JSON` (new). Settings → Agents → Conversation Export adds `agent.conversation_export.default_format` (enum, default `Markdown`) which seeds that selector. This is the SINGLE canonical setting key used everywhere in this spec; any earlier draft references to `default_export_format` (without the `agent.conversation_export.` prefix) are deprecated and must read as `agent.conversation_export.default_format`.

### B2. JSON schema (V1.0.0)

Top-level shape:

```json
{
  "schema_version": "1.0.0",
  "exported_at": "ISO8601",
  "warp_version": "<warp app version>",
  "conversation": {
    "id": "string",
    "title": "string",
    "model": "string",
    "profile": "string",
    "started_at": "ISO8601",
    "messages": [
      {
        "id": "string",
        "role": "user" | "assistant" | "system" | "tool",
        "started_at": "ISO8601",
        "content": [ /* typed content blocks, see B2.1 */ ],
        "metadata": { /* per-message free-form */ }
      }
    ]
  }
}
```

#### B2.1 Typed content blocks

Each message's `content` is an ordered array of typed blocks. Decoders that don't recognize a `type` MUST preserve it for round-tripping but MAY skip rendering it.

- `{ "type": "text", "text": "..." }`
- `{ "type": "reasoning", "text": "...", "duration_ms": <int> }`
- `{ "type": "tool_call", "tool": "<name>", "input": <TypedValue>, "output": <TypedValue>, "duration_ms": <int>, "status": "ok" | "error" }`
- `{ "type": "plan_step", "title": "...", "status": "pending" | "in_progress" | "complete", "items": [ ... ] }`
- `{ "type": "code_diff", "files": [ { "path": "...", "before": "...", "after": "...", "hunks": [ ... ] } ] }`
- `{ "type": "image", "data_uri": "..." }` (base64-encoded inline; no external file refs in V1)

##### B2.1.1 `tool_call` `input` / `output` are discriminated `TypedValue`s

Tool I/O in the wild is NOT always an object — many tools accept and return scalars (a single string, number, boolean) or arrays. To stay lossless, both `input` and `output` are wrapped in a discriminated `TypedValue`:

```jsonc
// object
{ "type": "object", "value": { "path": "/etc/hosts" } }

// array
{ "type": "array", "value": [1, 2, 3] }

// string
{ "type": "string", "value": "hello" }

// number
{ "type": "number", "value": 42 }

// boolean
{ "type": "boolean", "value": true }

// null
{ "type": "null" }
```

Decoders MUST switch on `type` to recover the original JSON value. `null` has no `value` field by design (its presence is fully encoded by `type`).

Worked example — a `read_file` tool whose input is a string path and whose output is the file's text content:

```json
{
  "type": "tool_call",
  "tool": "read_file",
  "input":  { "type": "string", "value": "/etc/hosts" },
  "output": { "type": "string", "value": "127.0.0.1 localhost\n" },
  "duration_ms": 12,
  "status": "ok"
}
```

Worked example — a `count_files` tool whose input is an object and whose output is a number:

```json
{
  "type": "tool_call",
  "tool": "count_files",
  "input":  { "type": "object", "value": { "glob": "**/*.rs" } },
  "output": { "type": "number", "value": 1387 },
  "duration_ms": 4,
  "status": "ok"
}
```

### B3. Schema versioning

`schema_version` follows semver:

- Patch (`1.0.0` -> `1.0.1`): documentation-only or non-semantic clarifications.
- Minor (`1.0.x` -> `1.1.0`): backward-compatible additions (new optional fields, new content-block types).
- Major (`1.x.y` -> `2.0.0`): breaking changes (removed fields, changed semantics).

Decoders MUST accept the current major. Decoders SHOULD reject unknown majors and SHOULD pass through unknown minor-version fields and content-block types unchanged.

### B4. Redaction is preserved

Any content already redacted in the displayed conversation MUST appear redacted in the JSON export — typically as a `"[REDACTED]"` placeholder string in the relevant `text` / `input` / `output` field. The export pipeline reuses the redaction state from the rendered conversation tree; it never re-reads the raw underlying source. This guarantees secrets that were stripped from the UI cannot reappear in an export.

#### B4.1 `RedactedConversationView` — single redacted source for UI and CLI

Both the UI export and the CLI export MUST source data from the SAME redacted-content store via a single `RedactedConversationView` interface. They MUST NOT reach into the raw conversation database directly. Concretely:

- `RedactedConversationView` is an in-process Rust trait/struct that exposes a redacted, render-ready view of a conversation: messages, content blocks, tool I/O, reasoning, plan steps, code diffs, images. Redaction is applied at retrieval time inside this view, before any export-pipeline code observes the data.
- The Markdown exporter, the JSON exporter, the UI Export action, and the CLI export verb all consume `RedactedConversationView` and only `RedactedConversationView`.
- The CLI is a thin wrapper around the same export pipeline used by the UI Export action — it does NOT have its own data path, and it does NOT bypass redaction.

##### B4.1.1 Headless-export redaction invariant (security)

Headless exports (CLI, scripted, automation) MUST NOT bypass redaction. Any code path that reads raw conversation content MUST apply the same redaction pipeline used by `RedactedConversationView` before serialization. A CLI export of a conversation containing a redacted secret MUST produce the same `[REDACTED]` placeholders the UI export produces. Implementations that read raw rows and serialize directly are non-conforming.

### B4.2 Reasoning blocks: include/exclude policy

V1 INCLUDES reasoning blocks (`{ "type": "reasoning", ... }`) by default. Some users will not want reasoning surfaced in shared exports, so V1 ships an explicit toggle:

- Setting: `agent.conversation_export.include_reasoning` — bool, default `true`.
- UI surface: Settings → Agents → Conversation Export → "Include reasoning blocks in exports" toggle.
- CLI: `--include-reasoning <true|false>` flag overrides the setting for a single run; otherwise the setting value applies.

When `include_reasoning = false`:

- Reasoning content blocks are OMITTED from each message's `content` array entirely. There is NO empty placeholder, NO `[REDACTED]` stub, and NO `"omitted": true` marker — the reasoning block simply is not present in the output.
- The surrounding block order is preserved. Text blocks before and after a reasoning block remain in their original order. If a message originally had `[text, reasoning, text]`, the export contains `[text, text]` with both text blocks intact and in order.
- Other content (tool calls, plan steps, code diffs, images) is unaffected.

This setting is independent of redaction. Redaction is about secrets/PII; this toggle is about whether reasoning text is part of the export at all.

### B5. Tool-call output truncation (opt-in)

Default: large tool outputs are included in full.

If the user enables `agent.conversation_export.truncate_large_outputs`, each tool-call `output` exceeding `agent.conversation_export.large_output_limit_kb` (default 64 KiB) is truncated to that limit. The block additionally carries `"truncated": true` and `"original_size_bytes": <int>` so consumers can detect and re-fetch if needed.

### B6. Sanitization carried through

Paths, environment variables, and PII that the agent saw redacted in the displayed conversation are redacted in the export. The export honors all existing privacy/redaction settings.

### B7. File output

Default filename: `<conversation_title-or-id>-<timestamp>.warp-export.json` written to the OS Downloads folder. The user may choose a custom location via the standard save-file dialog. Filename slug rules match the existing Markdown export.

### B8. CLI integration

If the existing CLI exposes a Markdown export verb, JSON export is a parallel flag:

```
warp export-conversation <conversation_id> --format json --output path/to/file.json [--include-reasoning <true|false>]
```

The CLI is a thin wrapper over the same export pipeline used by the UI Export action; it consumes `RedactedConversationView` (B4.1) and runs the same serializer. Same redaction, truncation, reasoning-include policy, and schema rules apply. Output to stdout when `--output -` is passed, so the export can pipe into `jq` / other tools.

Per B4.1.1, the CLI MUST NOT bypass redaction, MUST NOT read raw conversation rows directly, and MUST NOT branch into a separate exporter. Any future scripted/automation entry point must obey the same contract.

## Settings / API surface

- `agent.conversation_export.default_format` — enum, default `Markdown`. Seeds the UI format selector. (Single canonical key — see B1.)
- `agent.conversation_export.truncate_large_outputs` — bool, default `false`.
- `agent.conversation_export.large_output_limit_kb` — int, default `64`. Only consulted when truncation is enabled.
- `agent.conversation_export.include_reasoning` — bool, default `true`. When false, reasoning content blocks are omitted from exports (see B4.2).
- UI: Settings → Agents → Conversation Export panel hosting the four settings above (format selector, truncation toggle + limit, include-reasoning toggle).
- CLI: `--format json` and `--include-reasoning` flags on the export-conversation verb (or net-new verb if Markdown export is UI-only today). The CLI is a thin wrapper over the same export pipeline (B4.1, B8).

## Acceptance Criteria

- A1. Selecting "JSON" in the Export Conversation UI writes a file matching the schema in B2.
- A2. The exported file contains a top-level `schema_version` field equal to `"1.0.0"`.
- A3. Content already redacted in the displayed conversation is redacted in the exported JSON.
- A4. With `truncate_large_outputs = false`, a tool call that produced 200 KB of output appears in full.
- A5. With `truncate_large_outputs = true` and limit `64 KiB`, the same tool call's `output` is truncated to 64 KiB and the block carries `"truncated": true` and `"original_size_bytes": 204800`.
- A6. The CLI `--format json` flag produces output identical to the UI export for the same conversation. The CLI consumes `RedactedConversationView` (B4.1) and runs the same exporter the UI runs.
- A6a. CLI export of a conversation containing redacted secrets produces `[REDACTED]` placeholders in exactly the same fields the UI export does (CLI redaction parity).
- A7. The exported JSON validates against the documented JSON Schema for `schema_version 1.0.0`. (Schema-decoder validation; this is NOT a re-import test — V1 has no re-import path. See Non-Goals.)
- A8. Plan-step blocks preserve `status` correctly across all states.
- A9. Tool-call blocks preserve both `input` and `output` as discriminated `TypedValue`s (B2.1.1) — `object`, `array`, `string`, `number`, `boolean`, and `null` all round-trip losslessly.
- A10. Filename matches `<conversation_title-or-id>-<timestamp>.warp-export.json` when no custom location is chosen.
- A11. With `agent.conversation_export.include_reasoning = true` (default), reasoning blocks appear in the exported `content` arrays.
- A12. With `agent.conversation_export.include_reasoning = false`, reasoning blocks are omitted from exports — no placeholder, no `[REDACTED]` stub — and surrounding block order is preserved.

## Implementation Pointers

Verified in this codebase:

- `app/src/ai/agent/conversation.rs` — agent conversation domain model.
- `app/src/ai/agent/conversation_yaml.rs` — existing YAML serializer; mirror its shape for JSON. Reuse the same in-memory conversation model rather than re-walking from raw sources.
- `app/src/ai/agent_conversations_model.rs` and `app/src/ai/agent_conversations_model/entry.rs` — conversation list/index. Source for `id`, `title`, `started_at`, `model`, `profile`.
- `app/src/ai/agent/api/convert_conversation.rs` — existing conversion layer. Likely the right seam to plug a new exporter into.

Likely net-new modules (mark explicitly):

- `app/src/ai/agent/export_json.rs` — JSON exporter walking the in-memory rendered conversation tree.
- `app/src/ai/agent/export_schema.rs` — typed schema definitions (`ExportRoot`, `ExportConversation`, `ExportMessage`, `ContentBlock` enum). Acts as the contract surface.
- Settings additions in the existing agent settings module.
- Optional CLI plumbing if Markdown export today goes through a CLI surface — wire `--format` into that.

Reuse, do not duplicate:

- Reuse the conversation rendering pipeline that produces the redacted tree consumed by Markdown export. Walking that tree guarantees B4/B6 redaction parity.
- Reuse the existing extension `conversation.exported` telemetry event (B7 below).

## Tests

- T1. Unit: minimal conversation (one user msg, one assistant text reply) serializes through `export_json` and matches a fixture.
- T2. Unit: `schema_version` field is present and equals `"1.0.0"`.
- T3. Unit: a conversation containing a redacted secret produces JSON with `"[REDACTED]"` in the corresponding text field.
- T4. Unit: large tool output is included in full when `truncate_large_outputs` is `false`.
- T5. Unit: large tool output is truncated to limit and flagged with `truncated: true` + `original_size_bytes` when truncation is enabled.
- T6. Unit: plan steps with `pending`, `in_progress`, and `complete` statuses all serialize correctly.
- T7. Unit: tool calls preserve both `input` and `output` as `TypedValue`s with `type: "object"`.
- T_tool_call_string_input. Unit: a tool call whose input is a plain string serializes as `{ "type": "string", "value": "..." }` and round-trips losslessly through the schema.
- T_tool_call_scalar_output. Unit: tool calls whose outputs are `number`, `boolean`, and `null` each serialize as the corresponding `TypedValue` and round-trip losslessly. (Three sub-cases.)
- T_tool_call_array_io. Unit: a tool call whose output is an array serializes as `{ "type": "array", "value": [...] }`.
- T8. Integration: CLI `--format json --output -` and UI export produce byte-identical (or at minimum JSON-structurally-identical) output for the same conversation snapshot.
- T_cli_redaction_parity. Integration: CLI export of a conversation containing redacted secrets produces the SAME `[REDACTED]` placeholders in the SAME fields as the UI export. The CLI must consume `RedactedConversationView` and not the raw conversation store; verified by injecting a sentinel raw value that, if not redacted, would appear in the CLI output.
- T_schema_validates. Unit/integration: produced exports validate cleanly against the documented JSON Schema for `schema_version 1.0.0`. This is the renamed and re-scoped successor to the earlier "round-trip" test — it is JSON Schema validation, not behavioral re-import (V1 has no re-import path).
- T_reasoning_included_default. Unit: with `include_reasoning` left at its default (`true`), reasoning blocks appear in exports.
- T_reasoning_omitted. Unit: with `include_reasoning = false`, reasoning blocks are omitted entirely from `content` arrays — no empty marker, no `[REDACTED]` stub — and surrounding text blocks remain in their original order. Cover the `[text, reasoning, text]` -> `[text, text]` case explicitly.
- T9. Unit: filename slug matches `<conversation_title-or-id>-<timestamp>.warp-export.json` for several title shapes including titles with spaces, slashes, and unicode.
- T10. Unit: a conversation with an inline image serializes as a `data_uri` content block.
- T11. Unit: unknown content-block `type` encountered during decode round-trips unchanged (forward-compat probe).

## Open Questions

- Should we ALSO ship YAML and JSONL? Suggest V1 = JSON only, V1.5 = JSONL (one message per line) for streaming consumption. Defer YAML unless concrete demand surfaces.
- Should image blocks support an external-file mode (`{"type":"image","path":"..."}`) for very large images, with the JSON exported alongside an `assets/` sibling directory? Suggest defer to V1.5.
- ~~Should there be an explicit "include reasoning blocks" toggle?~~ Resolved in B4.2: V1 ships `agent.conversation_export.include_reasoning` (bool, default `true`). UI toggle in Settings → Agents → Conversation Export. CLI flag `--include-reasoning <true|false>`.
- Should `warp_version` be the app version, the agent-runtime version, or both? Suggest app version in V1; expand to a version map if it becomes ambiguous.

## Telemetry

No new event. Extend the existing `conversation.exported` event payload with a `format: "markdown" | "json"` field. This lets us measure JSON adoption without adding a parallel event.
