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

## Behavior contract

### B1 — Command-keyed output redaction

A new section in the redaction config:

```toml
[[redaction.command_output_rules]]
id = "default-kubectl-secret"
command_pattern = "^kubectl\\s+get\\s+secret\\b"
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
4. **Pattern semantics:** `command_pattern` is matched against the
   normalized subcommand path. Optional `argv_contains` and
   `argv_excludes` fields (arrays of regexes) further constrain the
   match against the normalized argv set when present.

#### B1.2 — Default command-output rules

V1 ships defaults covering the most common secret-bearing CLIs.
Each is argument-order independent per B1.1:

| id                                  | subcommand path matched                                  |
| ----------------------------------- | -------------------------------------------------------- |
| `default-kubectl-get-secret`        | `kubectl get secret*` (incl. `secrets`, `secret/<name>`) |
| `default-kubectl-describe-secret`   | `kubectl describe secret*`                               |
| `default-kubectl-get-sealedsecret`  | `kubectl get sealedsecret*`                              |
| `default-aws-secretsmanager`        | `aws secretsmanager get-secret-value`                    |
| `default-aws-ssm-get-parameter`     | `aws ssm get-parameter*` (incl. `get-parameters`)        |
| `default-gcloud-secrets-access`     | `gcloud secrets versions access*`                        |
| `default-vault-kv-get`              | `vault kv get*`                                          |
| `default-vault-read`                | `vault read*`                                            |
| `default-op-item-get`               | `op item get*`                                           |
| `default-bw-get`                    | `bw get*`                                                |
| `default-gh-secret-list`            | `gh secret list`                                         |
| `default-doppler-secrets-get`       | `doppler secrets get*`                                   |

All defaults redact the full output block and use a stable `id`
so users can override or disable them (see B3).

### B2 — Multiline pattern redaction

```toml
[[redaction.multiline_rules]]
id = "default-pem-private-key"
start_pattern = "-----BEGIN [A-Z ]+PRIVATE KEY-----"
end_pattern = "-----END [A-Z ]+PRIVATE KEY-----"
replacement = "[redacted: PEM key block]"
inclusive = true   # include the BEGIN/END lines in the redaction
applies_to = ["output", "input"]
```

The redaction layer walks the stream line-by-line. When
`start_pattern` matches, all subsequent lines are buffered until
`end_pattern` matches; the entire buffered range is replaced with
`replacement`. If `end_pattern` never matches (truncated stream),
the open block is replaced at flush time so no partial PEM body
leaks.

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
  applied to both output and input.
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

### B4 — Telemetry-safe

Redaction failures (a multiline block opened but never closed
before the buffer cap, or an oversized block triggering the cap
behavior in B5) are logged with the rule `id` — never the buffered
content.

### B5 — Performance bound (buffer cap behavior)

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
  redacts the entire BEGIN..END range, not just the first line.
- A3. A user rule in TOML with the same `id` as a default rule
  replaces that default. A user rule with `id = "<default-id>"` and
  `disabled = true` disables the corresponding default. A user rule
  with a new id is added. (Precedence per B3.1.)
- A4. A truncated PEM block (no END line) is still fully redacted
  via the buffer-cap fail-safe (B5). No subsequent lines leak into
  the agent context until end-delimiter or stream flush.
- A5. An oversized PEM-style block (greater than 1 MiB) emits the
  oversized-block placeholder, drops the buffered bytes from
  memory, and continues to suppress subsequent lines until the END
  delimiter matches.

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

## Out of scope (V1)

- ML-based redaction (entropy heuristics, custom-trained models).
- Per-pane redaction overrides.
- User-typed-secret detection beyond explicit delimiter blocks
  (e.g., heuristic detection of password-like tokens in free-form
  input). Targeted for V1.5.
