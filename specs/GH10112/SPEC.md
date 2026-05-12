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
- `{ "type": "plan_step", "title": "...", "status": "pending" | "in_progress" | "complete" | "blocked" | "cancelled", "items": [ /* see B2.1.2 */ ] }`
  - The top-level `plan_step.status` field uses the SAME fixed
    five-value enum as `items[].status` (see B2.1.2). Earlier drafts
    listed only three values (`pending | in_progress | complete`); that
    was an editorial oversight and is **superseded** here. Top-level
    `status` is a roll-up summary; its relationship to nested
    `items[].status` is defined in B2.1.2 below.
- `{ "type": "code_diff", "files": [ { "path": "...", "before": "...", "after": "...", "hunks": [ /* see B2.1.3 */ ] } ] }`
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

**"Round-trip" terminology used in this spec.** Several tests and
acceptance criteria below refer to values "round-tripping losslessly".
Since V1 ships no conversation re-import / restore-from-export feature
(see Non-Goals), "round-trip" in this document means **structural /
data** round-trip only:

```
in-memory value
    → JSON-serialize via the exporter
    → re-parse to a generic JSON value or to the typed
      `export_schema.rs` decoder
    → assert deep equality with the original
```

It does **NOT** mean "decoded back into a live agent conversation". No
V1 test asserts that an exported file can be opened as a conversation in
Warp — only that the bytes round-trip through `serde_json` /
`export_schema.rs` cleanly and that the schema describes them exactly.
Any earlier wording that implied a behavioral re-import path is
**superseded** by this clarification.

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

##### B2.1.2 `plan_step.items` shape (concrete)

Earlier drafts left `items: [ ... ]` as a placeholder. The concrete schema:

```json
{
  "type": "plan_step",
  "title": "Implement OAuth flow",
  "status": "in_progress",
  "items": [
    {
      "id": "string",
      "title": "string",
      "status": "pending" | "in_progress" | "complete" | "blocked" | "cancelled",
      "description": "string|null",
      "started_at": "ISO8601|null",
      "completed_at": "ISO8601|null",
      "blocked_reason": "string|null",
      "subitems": [ /* recursive: same shape as items[], or [] when there are none */ ]
    }
  ]
}
```

Field rules:

- `id`, `title`, `status` are REQUIRED.
- `status` enum is fixed at `"pending" | "in_progress" | "complete" |
  "blocked" | "cancelled"` for BOTH top-level `plan_step.status` and
  nested `items[].status` (and recursively `subitems[].status`).
  Decoders MUST reject unknown values within the same major schema
  version (per B3 unknown-major / known-major rules). Adding a new
  status is a minor-version-bump change.
- `description`, `started_at`, `completed_at`, `blocked_reason` are
  OPTIONAL but MUST be present and `null` (not omitted) so consumers
  can shape-match without a second null-safety branch.
- `subitems` is REQUIRED but MAY be the empty array `[]` when a step
  has no children. Recursion depth is bounded only by source data.
- `blocked_reason` is REQUIRED to be non-`null` iff `status == "blocked"`.

**Top-level `plan_step.status` vs nested `items[].status` (coverage
rule):**

- Each `plan_step.items[]` entry (and each `subitems[]` entry recursively)
  carries its OWN `status`. Nested statuses are independent and
  authoritative for that node.
- The TOP-LEVEL `plan_step.status` is a **roll-up summary** of its
  direct `items[]`, computed by the producer at export time using these
  precedence rules (highest precedence wins):
  1. If `items[]` is empty, `plan_step.status` is taken directly from
     the agent's plan-step model (its own status, independent of any
     items). No roll-up applies.
  2. Otherwise the roll-up considers ONLY the direct `items[]` (not
     recursive `subitems[]`) and emits:
     - `"blocked"` if ANY direct item has `status == "blocked"`.
     - else `"in_progress"` if ANY direct item has
       `status == "in_progress"`, OR if items contain a MIX of
       `"complete"` / `"cancelled"` and `"pending"`.
     - else `"complete"` if EVERY direct item has
       `status ∈ { "complete", "cancelled" }` (i.e., all terminal).
     - else `"cancelled"` if EVERY direct item has
       `status == "cancelled"`.
     - else `"pending"` (every direct item is `"pending"`).
- The producer MUST emit `plan_step.status` according to the above
  rules; consumers MAY rely on it as a summary but should consult
  `items[].status` for per-item state. The schema enforces only that
  both fields use the same enum; the roll-up itself is a producer
  invariant verified by tests (T_plan_step_status_rollup).

##### B2.1.3 `code_diff.hunks` shape (concrete)

Earlier drafts left `hunks: [ ... ]` as a placeholder. The concrete
schema follows standard unified-diff hunk semantics, expressed as
structured JSON so consumers do not need a unified-diff parser:

```json
{
  "type": "code_diff",
  "files": [
    {
      "path": "src/auth.rs",
      "before": "string|null",
      "after":  "string|null",
      "language": "string|null",
      "is_binary": false,
      "is_renamed": false,
      "old_path": "string|null",
      "hunks": [
        {
          "header": "@@ -10,5 +10,7 @@ fn authenticate",
          "old_start": 10,
          "old_lines": 5,
          "new_start": 10,
          "new_lines": 7,
          "lines": [
            { "kind": "context", "text": "    let user = ..." },
            { "kind": "removed", "text": "    let token = old_token();" },
            { "kind": "added",   "text": "    let token = new_token();" },
            { "kind": "added",   "text": "    refresh(token);" },
            { "kind": "context", "text": "    return user;" }
          ]
        }
      ]
    }
  ]
}
```

Field rules:

- `path`, `hunks` are REQUIRED on each file entry.
- `before` / `after` are the full file contents (or `null` when the
  agent did not capture them). They are optional but MUST be `null`
  when absent (not omitted) for the same reason as B2.1.2.
- `language` is an OPTIONAL syntax-highlighting hint; `null` when
  unknown.
- `is_binary` is REQUIRED (default `false`); when `true`, `hunks` is
  the empty array `[]` and `before` / `after` are `null`.
- `is_renamed` + `old_path` together signal a rename; if
  `is_renamed == true`, `old_path` MUST be the prior path.
- Each hunk REQUIRES `header`, `old_start`, `old_lines`, `new_start`,
  `new_lines`, and `lines`. `old_start` and `new_start` are 1-based
  line numbers consistent with unified diff.
- `lines[].kind` is the fixed enum `"context" | "added" | "removed" |
  "no_newline"`. The `"no_newline"` kind encodes the `\ No newline at
  end of file` marker; its `text` field MUST be the empty string.

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

V1 **EXCLUDES** reasoning blocks (`{ "type": "reasoning", ... }`) by
default. Reasoning text can contain sensitive draft thoughts, API keys or
secrets the agent reasoned over but chose not to call, and intermediate
decisions the user does not intend to share. The safe default therefore
omits reasoning from exports; users who want to include it must opt in
explicitly — either by flipping the setting or by checking the
per-export box.

This is a security-driven default and supersedes any earlier draft that
made reasoning include-by-default.

- Setting: `agent.conversation_export.include_reasoning` — bool, **default
  `false`** (was `true` in earlier drafts).
- UI surface: Settings → Agents → Conversation Export → "Include
  reasoning blocks in exports" toggle.
- CLI: `--include-reasoning <true|false>` flag overrides the setting for
  a single run; otherwise the setting value applies. When the flag is
  omitted and the setting is `false`, the CLI silently omits reasoning
  (no stderr warning — the safe default is already in effect; see
  B4.2.1).

When `include_reasoning = false`:

- Reasoning content blocks are OMITTED from each message's `content` array entirely. There is NO empty placeholder, NO `[REDACTED]` stub, and NO `"omitted": true` marker — the reasoning block simply is not present in the output.
- The surrounding block order is preserved. Text blocks before and after a reasoning block remain in their original order. If a message originally had `[text, reasoning, text]`, the export contains `[text, text]` with both text blocks intact and in order.
- Other content (tool calls, plan steps, code diffs, images) is unaffected.

This setting is independent of redaction. Redaction is about secrets/PII; this toggle is about whether reasoning text is part of the export at all.

#### B4.2.1 Per-export reasoning opt-in (security)

Because B4.2 makes `include_reasoning = false` the default, the
per-export surface is structured as an **opt-in to inclusion**, not a
warning before inclusion. A user who has not changed the user-level
setting AND does not check the per-export box will never ship reasoning.

- **Export dialog (UI):** the Export Conversation dialog (the same
  dialog that hosts the Markdown / JSON format selector) renders an
  inline per-export **"Include reasoning blocks"** checkbox **whenever
  the conversation contains at least one reasoning block**, regardless
  of the user-level setting. The checkbox is **pre-seeded from the
  user's `agent.conversation_export.include_reasoning` setting** —
  unchecked by default (the safe default), pre-checked only if the
  user has explicitly enabled inclusion globally. Toggling the box
  here applies to **this export only**; the user-level setting is not
  written.
  - When the effective value at export time is `true` (either the user
    enabled it globally OR they checked the per-export box), the
    dialog renders an additional inline notice **above the action
    buttons**, immediately before the user clicks Export:
    > **Reasoning will be included.** This export contains internal
    > model reasoning (`<N>` blocks), which may contain draft
    > thoughts, secrets the agent reasoned over, or intermediate
    > decisions you may not intend to share.
  - When the conversation contains zero reasoning blocks, neither the
    checkbox nor the notice is shown (no warning fatigue when there is
    nothing to disclose).
- **CLI (headless):** the CLI mirrors this opt-in shape:
  - If the user passes `--include-reasoning false` (explicit), or
    omits the flag while the user-level setting is `false` (default),
    reasoning is omitted with NO stderr warning — the safe default is
    in effect.
  - If the user passes `--include-reasoning true` (explicit), or
    omits the flag while the user-level setting has been flipped to
    `true`, AND the conversation contains ≥1 reasoning block, the CLI
    emits exactly one stderr line BEFORE writing any output to stdout:
    `warp: this conversation contains <N> reasoning block(s); they
    will be included.` Stderr is used so it does not contaminate
    `--output -` stdout pipelines. Passing `--include-reasoning
    false` always suppresses this warning (reasoning is not being
    included).

This disclosure does NOT replace the user-level setting; it complements
it. Users on the safe default never see warnings, and users who have
opted in globally still get a per-export reminder.

### B5. Tool-call output truncation (opt-in)

Default: large tool outputs are included in full.

If the user enables `agent.conversation_export.truncate_large_outputs`,
each tool-call `output` whose **measured size** exceeds
`agent.conversation_export.large_output_limit_kb` (default 64 KiB) is
truncated. Because `output` is a discriminated `TypedValue` (B2.1.1) and
may be a string, array, object, number, boolean, or null, the truncation
contract is defined per-`type` so it is unambiguous for every shape.

#### B5.1 Sizing rule (applies to every TypedValue type)

A single, deterministic byte-size function `S(value)` is used everywhere
in this contract — for both the threshold check and the reported
`original_size_bytes`. It is the byte length of the **canonical UTF-8
JSON serialization of the inner `value` field**, with no surrounding
TypedValue wrapper, no whitespace, and JSON object keys serialized in
lexicographic order.

Concretely, by `TypedValue.type`:

- `string`: `S = bytecount(canonical_json_quote(value))`. For a string
  whose raw UTF-8 form is `s`, `canonical_json_quote(s)` is `'"' + s +
  '"'` after JSON-escaping any required characters (`\"`, `\\`, control
  chars). For pure-ASCII strings with no escapes, `S = len(s) + 2`.
- `array` / `object`: `S` is the byte length of their canonical JSON
  serialization (object keys lexicographic, no whitespace).
- `number`: `S` is the byte length of the canonical JSON number form
  (the shortest representation that round-trips, per `serde_json` /
  ECMAScript-style rules).
- `boolean`: `S ∈ {4, 5}` (`true` = 4, `false` = 5).
- `null`: `S = 4` (`null`).

The **threshold check** in B5.2 is `S(value) > limit_bytes`. The
**reported `original_size_bytes`** in B5.2 is exactly that same
`S(value)` for the pre-truncation `value`. The earlier-draft wording that
described the string rule in terms of "first `limit_bytes` bytes of the
original UTF-8 string" mixed two different metrics (raw UTF-8 bytes vs
canonical JSON bytes); it is **superseded** by the rules in B5.2 below,
which are stated entirely in terms of `S`.

The `truncated` flag is set on the `tool_call` block — not on the inner
TypedValue — and the `output` field always remains a valid TypedValue
after truncation.

`limit_bytes` is derived from the `large_output_limit_kb` setting as
`limit_bytes = large_output_limit_kb * 1024` (KiB, base-2). For the
default of `64`, `limit_bytes = 65536`.

#### B5.2 Per-type truncation rules

All truncation operates on the inner `value` and is checked against
`S(value)` from B5.1. A `tool_call` whose `S(output.value) > limit_bytes`
is truncated as follows; otherwise it is left untouched.

| `output.type` | Truncation behavior when `S(value) > limit_bytes` |
|---|---|
| `string` | Find the largest prefix `p` of the original UTF-8 string `value` such that `S("string", p) ≤ limit_bytes`, where `S("string", p) = bytecount(canonical_json_quote(p))` (the JSON-quoted/escaped form). Snap `p` backward to the nearest valid Unicode codepoint boundary if needed, so the result is well-formed UTF-8. Replace `value` with `p`. |
| `array`  | Drop trailing elements one at a time until `S("array", value') ≤ limit_bytes`, where `value'` is the surviving array. The remaining `value` is a strict prefix of the original. |
| `object` | Drop trailing key-value pairs (in lexicographic key order) one at a time until `S("object", value') ≤ limit_bytes`. Earlier-sorted keys are preserved; the result is a strict subset of the original keys. |
| `number` / `boolean` / `null` | NEVER truncated — these encode in well under any practical `limit_bytes` (worst case `S = 5`). The `truncated` flag is NOT set for these types regardless of the configured limit. |

For every type that can be truncated (`string` / `array` / `object`),
when truncation actually occurs the `tool_call` block carries:

- `"truncated": true`
- `"original_size_bytes": <int>` — the canonical-serialization size
  of the **pre-truncation** inner `value`
- `"truncation_strategy": "string_bytes" | "array_tail" | "object_tail_keys"`
  — names the rule applied so consumers can reason about what was
  dropped.

When `output.type ∈ { number, boolean, null }`, the `tool_call` block
MUST NOT contain `truncated`, `original_size_bytes`, or
`truncation_strategy` fields. (Their presence on a non-truncatable type
is a schema violation.)

When `output.type ∈ { string, array, object }` but
`original_size_bytes <= limit_bytes`, the block likewise MUST NOT
contain those fields — they only appear when an actual truncation
occurred.

#### B5.3 Worked example (object)

A `read_file` tool returning a 200 KB file's bytes as a string:

```json
{
  "type": "tool_call",
  "tool": "read_file",
  "input":  { "type": "string", "value": "/var/log/big.log" },
  "output": { "type": "string", "value": "<first 65536 bytes, codepoint-aligned>" },
  "truncated": true,
  "original_size_bytes": 204800,
  "truncation_strategy": "string_bytes",
  "duration_ms": 17,
  "status": "ok"
}
```

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

### B9. JSON Schema artifact (location, generation, validation)

Earlier drafts referenced "the documented JSON Schema" without saying
where it lives. V1 ships the schema as an in-tree artifact so
implementers, tests, and external consumers all validate against the
same byte-identical document.

- **In-tree path**: `app/src/ai/agent/export_schema/v1.0.0/schema.json`.
  This is JSON Schema **draft 2020-12**.
- **`$id`**: `https://warp.dev/schemas/agent-conversation-export/v1.0.0/schema.json`.
  This is the canonical retrieval URL for external consumers; it
  resolves to the in-tree artifact at release time.
- **Single source of truth**: `app/src/ai/agent/export_schema.rs`
  (defined under Implementation Pointers as the typed schema module)
  is the single source of truth for the schema's shape. The
  `schema.json` artifact is derived from those Rust types via a
  build-time generator (`schemars` or equivalent). A CI check fails
  if `schema.json` drifts from the generator output, preventing the
  Rust types and the published schema from diverging silently.
- **Coverage**: the schema MUST cover every typed block defined in
  this spec — `text`, `reasoning`, `tool_call` (including all six
  `TypedValue` variants in B2.1.1), `plan_step` (B2.1.2),
  `code_diff` (B2.1.3), `image`. Required vs optional fields and
  enum values match this spec exactly.
- **Truncation flags (what the schema CAN enforce):** the schema
  enforces only the **shape-level** rules — the parts that depend on
  static structure, not on runtime byte sizes:
  - The three fields `truncated`, `original_size_bytes`, and
    `truncation_strategy` form a co-required group: a `tool_call`
    either has all three or none of them (encoded via JSON Schema
    `dependentRequired` / `dependentSchemas`).
  - When `truncated` is present it MUST be `true`.
  - When `truncation_strategy` is present, its value MUST be one of
    `"string_bytes"`, `"array_tail"`, `"object_tail_keys"` (fixed
    enum).
  - When `truncation_strategy == "string_bytes"`, the schema requires
    `output.type == "string"`; for `"array_tail"`, `output.type ==
    "array"`; for `"object_tail_keys"`, `output.type == "object"`.
    These are `if`/`then` conditional sub-schemas (JSON Schema
    draft 2020-12) that look only at the discriminator field, not at
    byte sizes.
  - JSON Schema therefore enforces that `tool_call` blocks whose
    `output.type ∈ { number, boolean, null }` carry NONE of the three
    truncation fields — because no valid `truncation_strategy` enum
    value pairs with those types.
- **Runtime-only rules (what tests, not the schema, enforce):** the
  schema cannot evaluate byte-size predicates such as
  `S(output.value) > limit_bytes` or "`original_size_bytes` equals the
  pre-truncation canonical-JSON size of `output.value`". These
  invariants are verified by exporter unit tests
  (T_truncation_string_codepoint_boundary, T_truncation_array_tail,
  T_truncation_object_lex_keys, etc.) and by the producer's
  contract — not by JSON Schema. Earlier-draft wording that said the
  schema "constrains ... iff the rules in B5.2 hold" is **superseded**
  by this split: the schema enforces static co-required/conditional
  shape; tests enforce the size predicates.
- **Validation in tests**: T_schema_validates (and any other
  schema-validation test in this spec) loads the in-tree
  `schema.json` and validates produced exports against it; tests do
  NOT hand-roll a JSON Schema fixture.
- **External documentation**: the schema is also published as part
  of the Warp docs site at the canonical `$id` URL.

## Settings / API surface

- `agent.conversation_export.default_format` — enum, default `Markdown`. Seeds the UI format selector. (Single canonical key — see B1.)
- `agent.conversation_export.truncate_large_outputs` — bool, default `false`.
- `agent.conversation_export.large_output_limit_kb` — int, default `64`. Only consulted when truncation is enabled.
- `agent.conversation_export.include_reasoning` — bool, **default `false`** (safe default; was `true` in earlier drafts — see B4.2). When `false`, reasoning content blocks are omitted from exports.
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
- A11. With `agent.conversation_export.include_reasoning = false` (the new safe default), reasoning blocks are OMITTED from the exported `content` arrays — no placeholder, no `[REDACTED]` stub — and surrounding block order is preserved.
- A12. With `agent.conversation_export.include_reasoning = true` (explicit opt-in), reasoning blocks appear in the exported `content` arrays.
- A_plan_step_status_rollup. For a `plan_step` with non-empty `items[]`, the top-level `plan_step.status` matches the roll-up rules in B2.1.2: `blocked` if any item is blocked; else `in_progress` if any item is in-progress or items mix terminal+pending; else `complete` if all items are terminal (`complete`/`cancelled`); else `cancelled` if all items are `cancelled`; else `pending`. For empty `items[]`, the top-level status is taken verbatim from the agent's plan-step model.
- A_truncation_uses_S. The exporter's threshold check uses the SAME canonical-JSON byte-size function `S(value)` for both the comparison and the reported `original_size_bytes` (see B5.1). The exporter MUST NOT use raw UTF-8 byte length for strings while reporting canonical-JSON size — these are different sizes and must not be mixed.
- A_schema_enforces_shape_only. The in-tree JSON Schema enforces the static shape rules for truncation flags (co-required group, fixed `truncation_strategy` enum, conditional `output.type`) but does NOT attempt to express runtime byte-size predicates. Byte-size invariants are verified by exporter tests, not by JSON Schema.
- A_schema_in_tree. The exporter's schema lives at
  `app/src/ai/agent/export_schema/v1.0.0/schema.json` and is the artifact
  used by every schema-validation test. CI fails when the in-tree
  artifact drifts from the generator output of `export_schema.rs`.
- A_plan_step_items_concrete. `plan_step.items` entries match the
  B2.1.2 shape — required `id`, `title`, `status`; nullable optional
  fields explicitly present as `null` when not set; `subitems` always
  present (possibly `[]`); `blocked_reason` non-null iff
  `status == "blocked"`.
- A_code_diff_hunks_concrete. `code_diff.files[].hunks` entries match
  the B2.1.3 shape — required `header`, `old_start`, `old_lines`,
  `new_start`, `new_lines`, `lines`; each line carries a fixed
  `kind ∈ { context, added, removed, no_newline }`; `is_binary == true`
  yields `hunks: []`.
- A_truncation_string. With `truncate_large_outputs = true` and a
  string output exceeding the limit, `output.value` is a UTF-8 prefix
  snapped to a codepoint boundary; the block carries `truncated: true`,
  `original_size_bytes` matching the canonical-serialization size, and
  `truncation_strategy: "string_bytes"`.
- A_truncation_array. An array output exceeding the limit is truncated
  by dropping trailing elements; resulting `value` is a strict prefix;
  block carries `truncation_strategy: "array_tail"`.
- A_truncation_object. An object output exceeding the limit is
  truncated by dropping trailing keys in lexicographic order;
  resulting `value` is a strict subset of the original keys; block
  carries `truncation_strategy: "object_tail_keys"`.
- A_truncation_scalars_never. Tool calls whose `output.type ∈
  { number, boolean, null }` NEVER carry `truncated`,
  `original_size_bytes`, or `truncation_strategy` — regardless of the
  configured limit; presence of any of those fields on such a block
  is a schema violation.
- A_reasoning_disclosure_ui. When the Export dialog is opened on a
  conversation containing ≥1 reasoning block AND the effective
  `include_reasoning` is `true`, the dialog renders the disclosure
  notice and a per-export checkbox pre-seeded from the user setting.
  Toggling the checkbox affects only this export and does NOT write
  the user-level setting.
- A_reasoning_disclosure_cli. When the CLI export verb is invoked
  without `--include-reasoning` on a conversation containing ≥1
  reasoning block, a single warning line is emitted to **stderr**
  naming the reasoning-block count; stdout (`--output -`) is
  uncontaminated. Passing `--include-reasoning true|false`
  suppresses the warning.
- A_reasoning_no_disclosure_when_absent. When the conversation
  contains zero reasoning blocks, neither the UI notice nor the CLI
  stderr warning is emitted.

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
- T_reasoning_omitted_default. Unit: with `include_reasoning` left at its NEW default (`false`), reasoning blocks are OMITTED from `content` arrays — no empty marker, no `[REDACTED]` stub — and surrounding text blocks remain in their original order. Cover the `[text, reasoning, text]` -> `[text, text]` case explicitly.
- T_reasoning_included_when_opted_in. Unit: with `include_reasoning = true` (explicit opt-in, either via the user-level setting or the per-export checkbox), reasoning blocks appear in the exported `content` arrays unchanged.
- T9. Unit: filename slug matches `<conversation_title-or-id>-<timestamp>.warp-export.json` for several title shapes including titles with spaces, slashes, and unicode.
- T10. Unit: a conversation with an inline image serializes as a `data_uri` content block.
- T11. Unit: unknown content-block `type` encountered during decode round-trips unchanged (forward-compat probe).
- T_schema_artifact_in_tree. Unit: `app/src/ai/agent/export_schema/v1.0.0/schema.json` exists, parses as JSON Schema draft 2020-12, and matches the generator output of `export_schema.rs` byte-for-byte.
- T_plan_step_items_concrete. Unit: a plan step exercising every `status` value (including `blocked` with a non-null `blocked_reason` and `cancelled`), with mixed `subitems` depths, round-trips losslessly and validates against the schema.
- T_code_diff_hunks_concrete. Unit: a code-diff block with multi-hunk content, all four `lines.kind` values, a binary file (`is_binary=true`, `hunks=[]`), and a renamed file (`is_renamed=true`, `old_path=...`) round-trips losslessly and validates against the schema.
- T_truncation_string_codepoint_boundary. Unit: a string output containing multi-byte UTF-8 just past the limit boundary truncates at the nearest valid codepoint boundary (no split codepoints); `original_size_bytes` matches canonical-serialization byte length; `truncation_strategy == "string_bytes"`.
- T_truncation_array_tail. Unit: an array of mixed scalar+object elements truncates by dropping trailing elements; resulting array is a prefix of the original; `truncation_strategy == "array_tail"`.
- T_truncation_object_lex_keys. Unit: an object with keys `{z, m, a}` (insertion order) and oversized total size has trailing-by-lex-key entries dropped first (so `z` is dropped before `m` before `a`); resulting object is a subset of original keys; `truncation_strategy == "object_tail_keys"`.
- T_truncation_no_flags_for_scalars. Unit: tool calls with `output.type` of `number`, `boolean`, and `null` (three sub-cases) never carry `truncated`, `original_size_bytes`, or `truncation_strategy`, regardless of the configured limit. Adding any of those fields to such a block fails schema validation.
- T_reasoning_disclosure_ui_checkbox_default_unchecked. Integration: opening the Export dialog on a conversation with reasoning blocks while the user-level setting is at the new default (`false`) shows the per-export "Include reasoning blocks" checkbox UNCHECKED. The disclosure notice ("Reasoning will be included") is NOT shown because the effective value is `false`.
- T_reasoning_disclosure_ui_checkbox_default_checked_when_optin. Integration: with the user-level setting set to `true`, the per-export checkbox is shown PRE-CHECKED, and the disclosure notice IS shown.
- T_reasoning_disclosure_ui_per_export_toggle_no_setting_write. Integration: toggling the per-export checkbox (in either direction) affects only the produced export and does NOT write the user-level setting.
- T_reasoning_disclosure_ui_hidden_when_absent. Integration: on a conversation with zero reasoning blocks, neither the per-export checkbox nor the disclosure notice is shown.
- T_reasoning_disclosure_cli_default_silent. Integration: invoking the CLI export verb without `--include-reasoning` on a conversation with reasoning blocks while the user-level setting is at the new default (`false`) emits NO stderr warning and produces an export with reasoning omitted.
- T_reasoning_disclosure_cli_stderr_when_optin. Integration: invoking the CLI export verb without `--include-reasoning` on a conversation with N reasoning blocks while the user-level setting has been flipped to `true` (OR invoking with explicit `--include-reasoning true`) emits exactly one stderr line of the form `warp: this conversation contains <N> reasoning block(s); they will be included.` and emits NO such line on stdout, including when `--output -` is used.
- T_reasoning_disclosure_cli_explicit_false_silent. Integration: invoking the CLI with `--include-reasoning false` emits no warning line on stderr (reasoning not being included).

Plan-step status roll-up (B2.1.2):

- T_plan_step_status_rollup_blocked. Unit: a `plan_step` with `items[]` containing one `blocked` item and several `pending`/`complete` items rolls up to `plan_step.status == "blocked"`.
- T_plan_step_status_rollup_in_progress_mixed. Unit: `items[]` with a `complete` item and a `pending` item (no `in_progress`, no `blocked`) rolls up to `"in_progress"` (mixed terminal+pending).
- T_plan_step_status_rollup_complete. Unit: `items[]` all `complete` (or a mix of `complete` and `cancelled`) rolls up to `"complete"`.
- T_plan_step_status_rollup_cancelled. Unit: `items[]` all `cancelled` rolls up to `"cancelled"`.
- T_plan_step_status_rollup_pending. Unit: `items[]` all `pending` rolls up to `"pending"`.
- T_plan_step_status_empty_items_passthrough. Unit: a `plan_step` with `items: []` exports `plan_step.status` directly from the agent's model (no roll-up); each of the five enum values is preserved on the top-level field.

Truncation sizing function `S` (B5.1):

- T_truncation_sizing_function_uniform. Unit: for several `output.value` instances (a 1000-byte JSON string with no escapes, a string requiring `\"` and `\\` escapes, a small array, a small object with non-lexicographic insertion order), `S(value)` matches both (a) the byte length used in the threshold check and (b) the reported `original_size_bytes` when truncation triggers. The test asserts they are equal — preventing future drift between "what we measured" and "what we reported".
- T_truncation_threshold_uses_canonical_json. Unit: a string whose raw UTF-8 bytes are exactly `limit_bytes` but whose JSON-quoted form (`"..."`) is `limit_bytes + 2` triggers truncation (`S > limit_bytes`), even though the raw bytes alone would not. Verifies the canonical-JSON metric is used uniformly.

JSON Schema shape-only enforcement (B9):

- T_schema_enforces_truncation_co_required. Unit: a `tool_call` block containing only `truncated: true` (without `original_size_bytes` and `truncation_strategy`) fails schema validation; a block containing all three passes; a block containing none passes.
- T_schema_enforces_truncation_strategy_enum. Unit: a `tool_call` with `truncation_strategy: "bogus_strategy"` fails schema validation. Each of the three valid values passes.
- T_schema_enforces_truncation_strategy_type_pairing. Unit: `truncation_strategy == "string_bytes"` paired with `output.type == "array"` fails schema validation (and analogues for the other two strategies). Correct pairings pass.
- T_schema_does_not_enforce_byte_size. Unit: a hand-crafted block with `original_size_bytes: 5` and a multi-megabyte `output.value.value` (clearly inconsistent) PASSES JSON Schema validation — the schema cannot inspect actual byte sizes. The same block fails the exporter's integration test (T_truncation_sizing_function_uniform), confirming the split between schema-enforced shape and test-enforced size invariants.

## Open Questions

- Should we ALSO ship YAML and JSONL? Suggest V1 = JSON only, V1.5 = JSONL (one message per line) for streaming consumption. Defer YAML unless concrete demand surfaces.
- Should image blocks support an external-file mode (`{"type":"image","path":"..."}`) for very large images, with the JSON exported alongside an `assets/` sibling directory? Suggest defer to V1.5.
- ~~Should there be an explicit "include reasoning blocks" toggle?~~ Resolved in B4.2: V1 ships `agent.conversation_export.include_reasoning` (bool, **default `false`** — safe default after round-3 security review). UI toggle in Settings → Agents → Conversation Export. Per-export checkbox in the Export dialog. CLI flag `--include-reasoning <true|false>`.
- Should `warp_version` be the app version, the agent-runtime version, or both? Suggest app version in V1; expand to a version map if it becomes ambiguous.

## Telemetry

No new event. Extend the existing `conversation.exported` event payload with a `format: "markdown" | "json"` field. This lets us measure JSON adoption without adding a parallel event.
