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

The existing "Export Conversation" UI gains a format selector with two options: `Markdown` (existing, default behavior preserved) and `JSON` (new). Settings → Agents → Conversation Export adds `default_export_format` (enum, default `Markdown`) which seeds that selector.

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
- `{ "type": "tool_call", "tool": "<name>", "input": { ... }, "output": { ... }, "duration_ms": <int>, "status": "ok" | "error" }`
- `{ "type": "plan_step", "title": "...", "status": "pending" | "in_progress" | "complete", "items": [ ... ] }`
- `{ "type": "code_diff", "files": [ { "path": "...", "before": "...", "after": "...", "hunks": [ ... ] } ] }`
- `{ "type": "image", "data_uri": "..." }` (base64-encoded inline; no external file refs in V1)

### B3. Schema versioning

`schema_version` follows semver:

- Patch (`1.0.0` -> `1.0.1`): documentation-only or non-semantic clarifications.
- Minor (`1.0.x` -> `1.1.0`): backward-compatible additions (new optional fields, new content-block types).
- Major (`1.x.y` -> `2.0.0`): breaking changes (removed fields, changed semantics).

Decoders MUST accept the current major. Decoders SHOULD reject unknown majors and SHOULD pass through unknown minor-version fields and content-block types unchanged.

### B4. Redaction is preserved

Any content already redacted in the displayed conversation MUST appear redacted in the JSON export — typically as a `"[REDACTED]"` placeholder string in the relevant `text` / `input` / `output` field. The export pipeline reuses the redaction state from the rendered conversation tree; it never re-reads the raw underlying source. This guarantees secrets that were stripped from the UI cannot reappear in an export.

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
warp export-conversation <conversation_id> --format json --output path/to/file.json
```

Same redaction, truncation, and schema rules apply. Output to stdout when `--output -` is passed, so the export can pipe into `jq` / other tools.

## Settings / API surface

- `agent.conversation_export.default_format` — enum, default `Markdown`. Seeds the UI format selector.
- `agent.conversation_export.truncate_large_outputs` — bool, default `false`.
- `agent.conversation_export.large_output_limit_kb` — int, default `64`. Only consulted when truncation is enabled.
- UI: Settings → Agents → Conversation Export panel hosting the three settings above.
- CLI: `--format json` flag on the existing export-conversation verb (or net-new verb if Markdown export is UI-only today).

## Acceptance Criteria

- A1. Selecting "JSON" in the Export Conversation UI writes a file matching the schema in B2.
- A2. The exported file contains a top-level `schema_version` field equal to `"1.0.0"`.
- A3. Content already redacted in the displayed conversation is redacted in the exported JSON.
- A4. With `truncate_large_outputs = false`, a tool call that produced 200 KB of output appears in full.
- A5. With `truncate_large_outputs = true` and limit `64 KiB`, the same tool call's `output` is truncated to 64 KiB and the block carries `"truncated": true` and `"original_size_bytes": 204800`.
- A6. The CLI `--format json` flag produces output identical to the UI export for the same conversation.
- A7. A roundtrip viewer can render every typed content block listed in B2.1 from the exported JSON.
- A8. Plan-step blocks preserve `status` correctly across all states.
- A9. Tool-call blocks preserve both `input` and `output`, not just one.
- A10. Filename matches `<conversation_title-or-id>-<timestamp>.warp-export.json` when no custom location is chosen.

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

- T1. Unit: minimal conversation (one user msg, one assistant text reply) round-trips through `export_json` and matches a fixture.
- T2. Unit: `schema_version` field is present and equals `"1.0.0"`.
- T3. Unit: a conversation containing a redacted secret produces JSON with `"[REDACTED]"` in the corresponding text field.
- T4. Unit: large tool output is included in full when `truncate_large_outputs` is `false`.
- T5. Unit: large tool output is truncated to limit and flagged with `truncated: true` + `original_size_bytes` when truncation is enabled.
- T6. Unit: plan steps with `pending`, `in_progress`, and `complete` statuses all serialize correctly.
- T7. Unit: tool calls preserve both `input` and `output`.
- T8. Integration: CLI `--format json --output -` and UI export produce byte-identical (or at minimum JSON-structurally-identical) output for the same conversation snapshot.
- T9. Unit: filename slug matches `<conversation_title-or-id>-<timestamp>.warp-export.json` for several title shapes including titles with spaces, slashes, and unicode.
- T10. Unit: a conversation with an inline image serializes as a `data_uri` content block.
- T11. Unit: unknown content-block type encountered during decode round-trips unchanged (forward-compat probe).

## Open Questions

- Should we ALSO ship YAML and JSONL? Suggest V1 = JSON only, V1.5 = JSONL (one message per line) for streaming consumption. Defer YAML unless concrete demand surfaces.
- Should image blocks support an external-file mode (`{"type":"image","path":"..."}`) for very large images, with the JSON exported alongside an `assets/` sibling directory? Suggest defer to V1.5.
- Should there be an explicit "include reasoning blocks" toggle? Reasoning may be sensitive in some workflows. Default include; offer toggle if early users ask.
- Should `warp_version` be the app version, the agent-runtime version, or both? Suggest app version in V1; expand to a version map if it becomes ambiguous.

## Telemetry

No new event. Extend the existing `conversation.exported` event payload with a `format: "markdown" | "json"` field. This lets us measure JSON adoption without adding a parallel event.
