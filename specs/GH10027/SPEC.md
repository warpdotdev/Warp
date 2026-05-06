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
   subsequent lines unredacted.

## Goal

Extend the secret-redaction layer with two new mechanisms:
**output-rule redaction** keyed off the command that produced the
output, and **multiline-pattern redaction** for delimiter-bracketed
blocks (e.g., `BEGIN ... END`).

## Behavior contract

### B1 — Command-keyed output redaction

A new section in the redaction config:

```toml
[[redaction.command_output_rules]]
command_pattern = "^kubectl\\s+get\\s+secret\\b"
redact_full_output = true
replacement = "[redacted: kubectl secret output]"
```

When the active block's command (as detected by Warp's existing
command parser) matches `command_pattern`, the entire block output
sent to the agent is replaced with `replacement`. The user still
sees the real output in their terminal — only the agent context
is redacted.

### B2 — Multiline pattern redaction

```toml
[[redaction.multiline_rules]]
start_pattern = "-----BEGIN [A-Z ]+PRIVATE KEY-----"
end_pattern = "-----END [A-Z ]+PRIVATE KEY-----"
replacement = "[redacted: PEM key block]"
inclusive = true   # include the BEGIN/END lines in the redaction
```

The redaction layer walks output line-by-line. When `start_pattern`
matches, all subsequent lines are buffered until `end_pattern`
matches; the entire buffered range is replaced with `replacement`.
If `end_pattern` never matches (truncated output), the open block
is replaced at flush time so no partial PEM body leaks.

### B3 — Default rule set

V1 ships defaults for the most common cases:
- `kubectl get secret*` → full-output redact.
- PEM private-key blocks (RSA, EC, OPENSSH) → multiline.
- `aws configure --profile *` interactive output → full-output.
- Generic env-style `\bAKIA[0-9A-Z]{16}\b` (already handled by
  single-line redaction; called out so users don't add a duplicate).

Defaults are user-overridable via the existing redaction settings
TOML.

### B4 — Telemetry-safe

Redaction failures (a multiline block opened but never closed
before the buffer cap) are logged with the rule name — never
the buffered content.

### B5 — Performance bound

Multiline buffering is capped at `WARP_REDACTION_MAX_BUFFER_BYTES`
(default 1 MiB). If the buffer fills before `end_pattern` matches,
the buffered content is replaced (fail-safe) and a warn fires.

## Acceptance criteria

- A1. With default rules: running `kubectl get secret foo -o yaml`
  shows real output in the terminal but the agent context shows
  the replacement string.
- A2. With default rules: pasting a private-key block redacts the
  entire BEGIN..END range, not just the first line.
- A3. User-defined rule in TOML overrides defaults at the same
  pattern.
- A4. A truncated PEM block (no END line) is still fully redacted
  via the buffer-cap fail-safe.

## Test plan

- T1–T4 = unit tests for each rule type (single, command-keyed,
  multiline-bracketed, fail-safe).
- T5 = integration test feeding a fixture command output through
  the redaction pipeline and asserting both terminal display and
  agent context.

## Out of scope (V1)

- ML-based redaction (entropy heuristics, custom-trained models).
- Per-pane redaction overrides.
- Redaction in the *input* path (user's typed prompts) — V1 is
  output-side only.
