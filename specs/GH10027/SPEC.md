# Spec: Multiline redaction + matched-output redaction (GH-10027)

## Problem

Secret Redaction works for single-line tokens (API keys, etc.)
that match the existing single-line regex set. SysOps/DevOps users
hit two gaps:

1. **Matched command outputs:** running `kubectl get secret <name>
   -o yaml` outputs YAML containing base64-encoded secrets. The
   *output* of that command should be redacted before reaching the
   agent, even when the individual lines don't match secret regexes.
2. **Multiline secrets:** `BEGIN PRIVATE KEY` blocks span many
   lines; today only the first line might match a pattern, leaving
   subsequent lines unredacted. The same delimiter blocks may also
   appear in pasted/typed input (e.g., a user pastes a private key
   into the agent prompt).

## Goal

Extend the secret-redaction layer with two new mechanisms:
**output-rule redaction** keyed off the command that produced the
output, and **multiline-pattern redaction** for delimiter-bracketed
blocks (e.g., `BEGIN ... END`). Multiline-pattern redaction applies
to both command output streams and pasted/typed input blocks.

## Pattern syntax

All `command_pattern`, `start_pattern`, `end_pattern`, `argv_contains`,
and `argv_excludes` fields are **Rust regex** (the `regex` crate
syntax). They are NOT glob patterns. In particular:

- `*` is a regex quantifier, not "any string". Use `.*` if you mean
  "any string", or use anchored boundaries (`(\s|$)`) to allow
  trailing arguments.
- Patterns SHOULD be anchored at start (`^`) when matching a
  command-line; trailing-arg tolerance is achieved with
  `(\s+\S+)*\s*` or a lookahead `(\s|$)`.
- The `(?i)` inline flag may be used for case-insensitive matching.
- Whitespace inside patterns is literal (no `x` flag); use `\s+` for
  whitespace runs.

Every default pattern below has at least one positive unit test
(must match) and at least one negative unit test (must not match
near-miss commands like `kubectl get secrets-config` for the
`kubectl get secret` rule).

## Rule id naming convention

- **Built-in (default) rules** use the reserved prefix
  `default-<tool>-<purpose>` in kebab-case. Examples:
  `default-kubectl-get-secret`, `default-aws-secretsmanager`,
  `default-vault-kv-get`, `default-pem-private-key`.
- **User rules** use any non-empty string id that does NOT begin
  with `default-`. The `default-` prefix is reserved.
- A user rule whose id begins with `default-` and does NOT correspond
  to a real built-in id is rejected at config-load time with a
  warning surfaced in the Settings UI; the rule is dropped, and all
  other rules (including built-ins) remain active.

## Behavior contract

### B1 — Command-keyed output redaction

A new section in the redaction config:

```toml
[[redaction.command_output_rules]]
id = "default-kubectl-get-secret"
command_pattern = "^kubectl(\\s+\\S+)*\\s+get\\s+secret(\\s|$)"
redact_full_output = true
replacement = "[redacted: kubectl secret output]"
```

When the active block's command (as detected by Warp's existing
command parser) matches `command_pattern`, the entire block output
sent to the agent is replaced with `replacement`. The user still
sees the real output in their terminal — only the agent context
is redacted.

#### B1.1 — Command matching normalization

`command_pattern` does **not** apply to the raw command string
verbatim. Before matching, Warp normalizes the command:

1. **Argv parse:** Split using shell-aware tokenization (the same
   parser used elsewhere in Warp). Quoting and escaping are
   respected.
2. **Subcommand path extraction:** For known multi-word CLIs
   (`kubectl`, `aws`, `gcloud`, `vault`, `op`, `gh`, `doppler`,
   `bw`), extract the ordered sequence of non-flag positionals
   that form the subcommand path (e.g.,
   `kubectl get secret`, `aws secretsmanager get-secret-value`).
3. **Argument-order independence:** Flags (`-n ns`, `--namespace=ns`,
   `-o yaml`) and additional positionals (resource names) are
   collected into a *set*. The match operates on the tuple
   `(subcommand_path, normalized_argv_set)`. This means
   `kubectl -n ns get secret foo` and `kubectl get secret -n ns foo`
   both match a rule keyed on subcommand path `kubectl get secret`.
4. **Pattern semantics:** `command_pattern` is a Rust regex matched
   against the normalized subcommand path string (a single
   space-joined string like `kubectl get secret`). Optional
   `argv_contains` and `argv_excludes` fields (arrays of regexes)
   further constrain the match against the normalized argv set when
   present.

#### B1.2 — Default command-output rules (regex form)

V1 ships defaults covering the most common secret-bearing CLIs.
Each is argument-order independent per B1.1. Patterns are matched
against the normalized subcommand path. The `(\s|$)` trailing
boundary admits trailing positionals like resource names without
admitting near-miss tokens.

| id                                   | `command_pattern` (Rust regex)                                |
| ------------------------------------ | ------------------------------------------------------------- |
| `default-kubectl-get-secret`         | `^kubectl\s+get\s+secrets?(\s|$)`                             |
| `default-kubectl-describe-secret`    | `^kubectl\s+describe\s+secrets?(\s|$)`                        |
| `default-kubectl-get-sealedsecret`   | `^kubectl\s+get\s+sealedsecrets?(\s|$)`                       |
| `default-aws-secretsmanager`         | `^aws\s+secretsmanager\s+get-secret-value(\s|$)`              |
| `default-aws-ssm-get-parameter`      | `^aws\s+ssm\s+get-parameters?(\s|$)`                          |
| `default-gcloud-secrets-access`      | `^gcloud\s+secrets\s+versions\s+access(\s|$)`                 |
| `default-vault-kv-get`               | `^vault\s+kv\s+get(\s|$)`                                     |
| `default-vault-read`                 | `^vault\s+read(\s|$)`                                         |
| `default-op-item-get`                | `^op\s+item\s+get(\s|$)`                                      |
| `default-bw-get`                     | `^bw\s+get(\s|$)`                                             |
| `default-gh-secret-list`             | `^gh\s+secret\s+list(\s|$)`                                   |
| `default-doppler-secrets-get`        | `^doppler\s+secrets\s+get(\s|$)`                              |

All defaults redact the full output block and use a stable `id`
so users can override or disable them (see B3).

### B2 — Multiline pattern redaction

```toml
[[redaction.multiline_rules]]
id = "default-pem-private-key"
start_pattern = "-----BEGIN [A-Z ]+PRIVATE KEY-----"
end_pattern = "-----END [A-Z ]+PRIVATE KEY-----"
replacement = "[redacted: PEM key block]"
inclusive = true
applies_to = ["output", "input"]
```

The redaction layer walks the stream line-by-line. When
`start_pattern` matches, all subsequent lines are buffered until
`end_pattern` matches; the entire buffered range is replaced with
`replacement`. If `end_pattern` never matches (truncated stream),
the open block is replaced at flush time so no partial PEM body
leaks.

#### B2.1 — `inclusive` field

`inclusive` controls whether the lines that contain the matching
delimiters are themselves redacted:

- `inclusive = true` — the redaction range INCLUDES the line
  containing `start_pattern` and the line containing `end_pattern`.
  The delimiters themselves are replaced. Use for PEM blocks where
  the BEGIN/END markers should not appear in agent context.
- `inclusive = false` — only the content BETWEEN the delimiter
  lines is replaced. The lines containing `start_pattern` and
  `end_pattern` are forwarded verbatim. Use when the delimiter
  itself is a useful structural marker (e.g., a heredoc tag) and
  only the body is sensitive.

**Default**: `false`. Built-in PEM rule sets `inclusive = true`
explicitly.

#### B2.2 — `applies_to` field

`applies_to` is the explicit scope for the rule:

- `"output"` — redact this delimiter block when it appears in
  command output streams flowing to the agent.
- `"input"` — redact this delimiter block when it appears in
  pasted or typed user input flowing to the agent (e.g., the
  prompt box). Terminal display is not affected by input-side
  redaction either; only the agent context is sanitized.

V1 default for the PEM private-key rule is
`applies_to = ["output", "input"]` so a user pasting a private key
is protected by default. Users may scope rules narrower if needed.

### B3 — Default rule set and override semantics

V1 ships defaults for the most common cases:

- The 12 command-output rules listed in B1.2.
- PEM private-key blocks (RSA, EC, OPENSSH) → multiline (B2),
  applied to both output and input
  (`id = "default-pem-private-key"`, `inclusive = true`).
- Generic env-style `\bAKIA[0-9A-Z]{16}\b` (already handled by
  single-line redaction; called out so users don't add a
  duplicate).

#### B3.1 — Override semantics

User-supplied redaction rules merge with defaults by rule `id`
(string). Rule `id` is mandatory for both default and user-supplied
rules.

Rules:

1. **Identity by id.** Two rules with the same `id` are considered
   the same logical rule. User wins.
2. **Replace by id.** A user rule with the same `id` as a default
   rule fully replaces the default's pattern, replacement, and
   scope. The merged rule keeps the user's fields.
3. **Disable by id.** A user rule with `disabled = true` and the
   id of a default rule disables that default rule. No new rule
   is added.

   ```toml
   [[redaction.command_output_rules]]
   id = "default-kubectl-get-secret"
   disabled = true
   ```
4. **Add by new id.** A user rule with an id not matching any
   default is appended to the active rule set.
5. **Precedence (highest first):**
   1. User-explicit (user-supplied rule with non-disabled state)
   2. User-disabled (user-supplied rule with `disabled = true`)
   3. Default built-in
6. **Within the active set, evaluation order is:** command-output
   rules first (block-level, may short-circuit the rest);
   single-line rules next; multiline rules last (because they may
   span lines that single-line rules already redacted).

#### B3.2 — Invalid user-rule handling (per-rule fail-closed)

Each user-supplied rule is validated INDIVIDUALLY at config-load
time. The fail-closed invariant: **invalid rules drop in isolation
and never disable built-in protections**.

| Failure mode                                 | Result                                                                                                               |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Invalid regex in any pattern field           | Rule is DROPPED. Other user rules and ALL defaults remain active. Warning logged with rule id and parser error. Surfaced in Settings UI as `Rule '<id>' has invalid pattern: <error>`. |
| Missing required field (`id`, `pattern`, …)  | Rule is DROPPED. Other rules unaffected. Warning surfaced in Settings UI.                                            |
| Duplicate user-supplied id (same id twice in user config) | First occurrence is kept; subsequent duplicates are DROPPED with a warning. Defaults are never overridden by a duplicate-id user rule. |
| User id begins with reserved `default-` prefix but does not match a real built-in | Rule is REJECTED with warning; dropped. Other rules unaffected. |
| Whole config file malformed (TOML parse error)  | The user config is treated as empty. **All built-in defaults remain active.** Loud warning in Settings UI; redaction is never disabled by a config error. |

Net invariant: there is **no failure mode in user configuration
that disables built-in default redaction**. Defaults are always
loaded from a compiled-in source independent of user config
parsing.

### B4 — Centralized agent-context boundary

Redaction MUST run at a single choke-point on every path that
sends text to the agent. Decentralized redaction (each call site
remembering to call the redactor) is the failure mode this section
prevents.

#### B4.1 — `redact_for_agent` choke-point

A single function:

```rust
/// Redact `text` according to the active rule set before it leaves
/// Warp for the agent. Every agent-bound text path MUST call this.
pub fn redact_for_agent(text: &str, scope: AgentBoundScope) -> String;
```

`AgentBoundScope` carries enough context for the redactor to
distinguish output-side vs input-side rules and to associate
output with its originating command (for B1).

#### B4.2 — Enumerated agent-bound paths

Every path below MUST call `redact_for_agent`. This list is
exhaustive for V1:

| Path                                                                  | Scope    | Notes                                                                 |
| --------------------------------------------------------------------- | -------- | --------------------------------------------------------------------- |
| Block output capture (existing single-line redactor's call site)      | Output   | Passes originating command to enable B1 command-keyed rules.          |
| Prompt input field (text typed into the agent prompt)                 | Input    | Runs at submit, not on each keystroke.                                |
| Pasted text into the prompt                                           | Input    | Runs at paste-commit, before submission, so PEM blocks never reach the wire. |
| Tab config `commands` field input (user-configured command shortcut)  | Input    | Same as prompt input — text becomes part of agent context.            |
| Conversation history replay (re-sending prior context)                | Output   | Replays go through the same choke-point with the same rules.          |
| Tool-call inputs returned to the agent (bash tool stdout, edit tool diffs, file-read tool content) | Output   | Each tool's "result back to model" path calls the choke-point.        |
| File-read content forwarded to the agent                              | Output   | Includes editor selection forwarded as context.                       |

#### B4.3 — Lint enforcement

A custom clippy lint (mirroring the pattern used in PR #10222 for
safe-read enforcement) flags any new construction of
agent-bound text that bypasses `redact_for_agent`. The lint is
keyed on the type of the agent-bound message struct: any code
that constructs that struct from a `String` MUST source the string
from `redact_for_agent` or be annotated with a justification
comment that the linter accepts.

CI fails on lint violations. Adding a new agent-bound path is
therefore a deliberate act that requires either calling the
choke-point or explicitly justifying a bypass.

### B5 — Telemetry-safe

Redaction failures (a multiline block opened but never closed
before the buffer cap, an oversized block triggering the cap
behavior in B6, an invalid user rule dropped) are logged with the
rule `id` only — never the buffered or matched content.

### B6 — Performance bound (buffer cap behavior)

Multiline buffering is capped at `WARP_REDACTION_MAX_BUFFER_BYTES`
(default 1 MiB). The cap behavior is designed so that no
post-cap content ever leaks once redaction is active:

1. **State machine:** Each multiline rule has two states:
   `inactive` and `active`. Matching `start_pattern` transitions
   `inactive → active`. Matching `end_pattern` transitions
   `active → inactive` and emits the `replacement`.
2. **Cap reached while `active`:** When the accumulated buffer
   exceeds the cap, the redaction layer:
   1. **Drops** the buffered bytes from memory (no leak through
      logs or telemetry).
   2. **Keeps** the rule in the `active` state — does *not* revert
      to `inactive`.
   3. Emits the placeholder
      `[REDACTED: oversized block, content dropped]` once into the
      agent stream in place of the dropped buffer.
   4. Continues scanning subsequent input lines for `end_pattern`.
      Any line read while `active` is consumed by the redactor and
      not emitted to the agent.
3. **Exit:** State returns to `inactive` only on (a) `end_pattern`
   match or (b) stream flush / EOF. Stream flush in `active` state
   emits `[REDACTED: unterminated block, content dropped]` and
   resets state.
4. **Net guarantee:** Once a multiline rule transitions to
   `active`, no line is forwarded to the agent until the rule
   transitions back to `inactive`, regardless of buffer cap.

## Acceptance criteria

- A1. With default rules: running `kubectl get secret foo -o yaml`
  shows real output in the terminal but the agent context shows
  the replacement string. The same applies to
  `kubectl -n ns get secret foo` and
  `kubectl get -n ns secret foo` (argument-order independence per
  B1.1).
- A2. With default rules: pasting a private-key block (PEM
  delimiters) into the prompt or receiving one in command output
  redacts the entire BEGIN..END range INCLUDING the BEGIN and END
  lines (`inclusive = true`), not just the first line.
- A3. A user rule in TOML with the same `id` as a default rule
  replaces that default. A user rule with `id = "<default-id>"` and
  `disabled = true` disables the corresponding default. A user rule
  with a new id is added. (Precedence per B3.1.)
- A4. A truncated PEM block (no END line) is still fully redacted
  via the buffer-cap fail-safe (B6). No subsequent lines leak into
  the agent context until end-delimiter or stream flush.
- A5. An oversized PEM-style block (greater than 1 MiB) emits the
  oversized-block placeholder, drops the buffered bytes from
  memory, and continues to suppress subsequent lines until the END
  delimiter matches.
- A6. A user config containing one rule with an invalid regex and
  one valid rule loads the valid rule, drops the invalid rule with
  a warning, and keeps ALL built-in defaults active.
- A7. A completely malformed user config (unparseable TOML) keeps
  all built-in defaults active and surfaces a warning in the
  Settings UI.
- A8. Every agent-bound path enumerated in B4.2 routes through
  `redact_for_agent`. Adding a new agent-bound construction site
  without calling the choke-point fails CI via the clippy lint
  (B4.3).

## Test plan

- T1–T4 = unit tests for each rule type (single, command-keyed,
  multiline-bracketed, fail-safe).
- T5 = integration test feeding a fixture command output through
  the redaction pipeline and asserting both terminal display and
  agent context.
- T6 = command-matching normalization tests covering argument
  reordering across all 12 default command-output rules.
- T7 = override-semantics tests for the four merge cases in B3.1
  (replace, disable, add, precedence).
- T8 = buffer-cap state-machine test verifying that lines after
  the cap are not leaked until end-delimiter or flush.
- T9 = input-side multiline test: pasted PEM block in the agent
  prompt is redacted before the prompt is sent to the agent.
- T10 = per-rule positive/negative regex test for every default
  command-output rule (must match intended forms; must NOT match
  near-miss commands such as `kubectl get secrets-config`,
  `aws secretsmanager describe-secret`, `vault kv put`).
- T11 = `inclusive` field test: a multiline rule with
  `inclusive = false` retains delimiter lines in the agent stream
  while replacing only the body; `inclusive = true` replaces the
  entire range.
- T12 = invalid-user-rule isolation test: config with a mix of
  valid and invalid rules loads only the valid ones; built-in
  defaults remain active. Includes the duplicate-id case and the
  reserved-prefix case.
- T13 = malformed-config fail-closed test: TOML parse failure
  results in an empty user-rule set with all defaults active.
- T14 = agent-context boundary integration test: each path in
  B4.2 (block output, prompt submit, paste, tab-config command,
  history replay, tool-call result, file-read forwarding) is
  exercised end-to-end with a sentinel secret that MUST be
  redacted.
- T15 = lint test: a deliberately-broken commit constructs the
  agent-bound message struct from raw `String` without going
  through `redact_for_agent` and the clippy lint flags it.

## Out of scope (V1)

- ML-based redaction (entropy heuristics, custom-trained models).
- Per-pane redaction overrides.
- User-typed-secret detection beyond explicit delimiter blocks
  (e.g., heuristic detection of password-like tokens in free-form
  input). Targeted for V1.5.
