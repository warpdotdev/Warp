---
name: route
description: Recommend a tier and dispatch target for a development task in the Warp repo, based on scope and complexity. Use before starting non-trivial work to pick the right tool, or when the current agent feels mismatched. Returns a recommendation with reasoning — the calling harness decides whether to act on it.
---

# route

Recommend a tier and dispatch target for a development task in this repo.

This skill is harness-agnostic. The classification logic is the same for Claude Code, Codex, Gemini CLI, or any other supported agent. Only the final dispatch step is harness-specific (see [Dispatch](#dispatch)).

The skill recommends *tiers* (capability profiles), not specific model versions. Specific model IDs change too often to bake into rules; the dispatch target — a subagent file under `.claude/agents/` for Claude Code, or an out-of-band CLI invocation for other harnesses — is where the concrete model lives, and where it gets updated when versions change.

## When to use

- Starting a non-trivial task and unsure which tier fits.
- The current agent feels mismatched (slow on rote work; missing nuance on hard work).
- A task has clearly bimodal characteristics (e.g. small but subtle, large but mechanical) and a routine default would pick wrong.

Skip routing for routine work. Most edits don't need a routing decision; the user's current default model is fine.

## Tiers

| Tier | When | Default Claude Code dispatch |
|---|---|---|
| `fast` | Mechanical, scope-bounded edits with no design judgment. | `haiku-mechanical` |
| `balanced` | Mid-complexity, scope-bounded subtasks with some design judgment. | `sonnet-balanced` |
| `deep` | Cross-module reasoning, lock-stack work, invariant-heavy changes. | `opus-architect` |
| `long-context` | Read-heavy: full-spec audits, large diffs, dependency analysis. | `gemini-long-context` (wraps Gemini CLI) |
| `local` | Offline or cost-sensitive work where the prompt itself doesn't need to stay off hosted models. | `local-coder` (wraps Ollama; orchestrator runs on hosted Sonnet) |
| `current` | Default — keep the current model. | (no dispatch) |

`second-opinion` is *not* a primary tier — see [Second-opinion augmentation](#second-opinion-augmentation) below for how it appears in `alternatives`.

The mapping from tier to specific model lives inside each subagent's frontmatter (Claude tiers use short aliases like `model: opus`, which auto-track latest; wrapper tiers read env-var-driven defaults like `${GEMINI_MODEL:-gemini-2.5-pro}`). When a new version ships, update the subagent file or override via env var; this skill doesn't need to know.

## Classification contract

The classifier consumes a fixed input shape and emits a fixed output shape. This contract is the single source of truth — harness adapters consume it; they do not re-classify.

### Inputs

| Field | Type | Description |
|---|---|---|
| `task_description` | string | Free-text description of what the task asks for. |
| `expected_scope` | `single-file` \| `few-files` \| `many-files` \| `cross-crate` | How widely the change is expected to spread. |
| `complexity` | `shallow` \| `moderate` \| `deep` | Shallow = rote / explicit; moderate = local logic; deep = cross-module reasoning, invariants, concurrency. |
| `context_size_estimate` | `small` \| `medium` \| `large` | Roughly how much existing code must be read to do the work well. `large` = whole spec, full diff, `Cargo.lock` audit, etc. |
| `correctness_criticality` | `low` \| `medium` \| `high` | `high` = touches concurrency, locks, persistence schemas, public APIs, security boundaries. |
| `privacy_constraint` | `none` \| `local-only` | `local-only` = the user has asked to keep the prompt content off hosted models. |
| `current_tier` | `fast` \| `balanced` \| `deep` \| `unknown` | The capability tier of the calling session's primary model. Used to decide whether dispatching to a different tier is worth the cost. |

If a field is unknown, the caller fills in its best guess and lowers `confidence` in the output accordingly.

### Outputs

| Field | Type | Description |
|---|---|---|
| `tier` | string | One of the tier names above. |
| `dispatch` | string \| `null` | Subagent name to dispatch to. `null` when `tier = current`, when the constraint is informational (e.g. prompt-confidentiality requires out-of-band action), or when no suitable subagent is available. |
| `reasoning` | string | One or two sentences explaining the choice in terms of the inputs. |
| `alternatives` | array | 0–2 other reasonable tiers, each with `tier`, `dispatch`, and a one-line `why`. Includes second-opinion augmentation when applicable (see below). |
| `confidence` | `low` \| `medium` \| `high` | How sure the classifier is. Low when inputs were guesses or the task profile is unusual. |

## Decision rules

Apply in order. Stop at the first matching rule.

1. **Prompt-confidentiality constraint.** `privacy_constraint = local-only` → `tier: local`, **`dispatch: null`**. Reasoning: the calling harness's subagent wrappers (`local-coder` and friends) all run under a hosted-model orchestrator that processes the user's prompt before delegating to a local model. The wrapper does not satisfy a prompt-confidentiality requirement; the user must invoke Ollama (or another local model) directly, outside this harness. Include `local-coder` in `alternatives` for callers whose actual concern is offline-or-cost rather than prompt confidentiality (orchestrator runs hosted; only the task work runs locally).

2. **Long-context audit.** `context_size_estimate = large` AND task is read-heavy (audit, review, summarize) rather than write-heavy → `tier: long-context`, dispatch `gemini-long-context`. This rule comes before rule 3 so deep / high-criticality audits still get the long-context tier; for read-heavy work, the volume of context matters more than the difficulty of the reasoning over it. If the user doesn't have the Gemini CLI installed, the wrapper subagent reports that and the recommendation falls back to `tier: deep` (dispatch `opus-architect`) for a tightened-scope read.

3. **Deep cross-cutting reasoning OR high-stakes correctness.** `complexity = deep` OR `correctness_criticality = high` → `tier: deep`, dispatch `opus-architect`. Examples: WarpUI Entity-Handle redesigns, `TerminalModel` lock-stack work, designing a feature flag rollout, root-cause debugging across module boundaries, single-line changes in security-sensitive code. The criticality clause means even a shallow narrow edit gets routed to `deep` when it touches locks, schemas, public APIs, or security boundaries. See [Second-opinion augmentation](#second-opinion-augmentation) for how Codex appears in `alternatives` when this rule fires.

4. **Mechanical, narrow, low/medium criticality.** `complexity = shallow` AND `expected_scope ∈ {single-file, few-files}` AND `context_size_estimate ∈ {small, medium}` AND `correctness_criticality ∈ {low, medium}` → `tier: fast`, dispatch `haiku-mechanical`. Examples: variable rename, format-only fix, single-test repair, removing unused imports, applying an explicit patch from a spec. High-criticality narrow edits are caught by rule 3 instead.

5. **Mid-tier, current model is fast or deep.** `complexity = moderate` AND `current_tier ∈ {fast, deep}` → `tier: balanced`, dispatch `sonnet-balanced`. When the calling session is already on the balanced tier (`current_tier = balanced`), this rule falls through to the default — there's no benefit to dispatching balanced from balanced unless context isolation is the goal. When `current_tier = unknown`, this rule also falls through; conservatively assume balanced.

6. **Default.** Anything else → `tier: current`, `dispatch: null`. `reasoning` should explicitly say "no routing benefit expected."

## Second-opinion augmentation

Independent of which primary rule fires, if `correctness_criticality = high` AND the calling harness has the Codex plugin available, *additionally* include in `alternatives`:

```
{ tier: "second-opinion",
  dispatch: "codex:codex-rescue",
  why: "independent read for high-criticality work; complements the primary recommendation" }
```

This is a complementary recommendation, not a primary route — it doesn't replace the rule that fired. The dispatch target is the existing `codex:codex-rescue` plugin agent (see [Dispatch](#dispatch)). If the codex plugin isn't loaded, omit the alternative or surface as informational text and let the user run Codex CLI manually.

## Specific signals worth weighting

These are warp-repo-specific cues that should shift the recommendation even when the broad inputs look ordinary:

- **`TerminalModel::lock` appears in the task.** Push toward rule 3 (`deep`). The lock-stack rules in `WARP.md` are easy to violate; the deeper-reasoning tier catches stack interactions cheaper tiers miss.
- **WarpUI Entity / Handle / ViewContext changes.** Push toward rule 3.
- **Pure feature-flag plumbing using the `add-feature-flag` skill.** Push toward rule 4 (`fast`); the skill is explicit and the work is mechanical.
- **Whole-`Cargo.lock` or whole-spec analysis.** Push toward rule 2 (`long-context`).
- **Persistence schema migrations under `crates/persistence/`.** Push toward rule 3 (architect) AND second-opinion augmentation (independent pass before merge).
- **WGSL shader edits.** Push toward `current` if `current_tier = balanced`; otherwise rule 5 (`balanced`). Specialized syntax that the balanced tier handles reliably.

## Dispatch

The classifier output is the same regardless of harness. Dispatch is harness-specific:

- **Claude Code.** If `dispatch` matches a subagent under `.claude/agents/` OR a plugin-prefixed name (e.g., `codex:codex-rescue` from the `codex` plugin) that is loaded in the current session, dispatch with the Agent tool using that name. If the named target isn't available (plugin not loaded, file missing), surface the recommendation as informational and let the user dispatch manually. If `null`, do nothing — but surface the `reasoning` and `alternatives` to the user; that's the whole output for an informational recommendation.
- **Codex.** No native subagent format; the recommendation is informational. The user (or an outer orchestrator) acts on it.
- **Gemini CLI.** Same — informational; the user acts on it.
- **Other harnesses.** Same as Codex/Gemini.

## Worked examples

**Example 1.**
> "Rename `delimeter` → `delimiter` everywhere in `crates/foo/`."

inputs: `shallow` / `few-files` / `small` / `low` / `none` / `current_tier=balanced` → rule 4 → `tier: fast`, dispatch `haiku-mechanical`, confidence `high`. Reasoning: rote rename, narrow scope, low correctness risk.

**Example 2.**
> "Refactor `TerminalModel::lock` callers to avoid the deadlock pattern in WARP.md."

inputs: `deep` / `cross-crate` / `medium` / `high` / `none` / `current_tier=balanced` → rule 3 → `tier: deep`, dispatch `opus-architect`, confidence `high`. Alternatives include `tier: second-opinion` via `codex:codex-rescue` (per the augmentation rule, since criticality is high). Reasoning: lock-stack reasoning is the WARP.md cliff edge.

**Example 3.**
> "Audit the workspace's `Cargo.lock` for transitive dependencies pulling in OpenSSL."

inputs: `shallow` / `cross-crate` / `large` / `medium` / `none` / `current_tier=balanced` → rule 2 → `tier: long-context`, dispatch `gemini-long-context`, confidence `high`. Reasoning: long-context audit; write-out is small.

**Example 4.**
> "Add an integration test for the new completion flow."

inputs: `moderate` / `few-files` / `medium` / `medium` / `none` / `current_tier=balanced` → rule 6 (default) → `tier: current`, no dispatch, confidence `medium`. Reasoning: routine mid-tier work and the calling session is already on balanced — no routing benefit expected.

**Example 5.**
> "Prototype a parser change locally — don't send the code to a hosted model."

inputs: `moderate` / `single-file` / `small` / `medium` / `local-only` / `current_tier=balanced` → rule 1 → `tier: local`, **`dispatch: null`**, confidence `high`. Reasoning: prompt confidentiality requires out-of-band invocation (run `ollama` directly outside this harness); the wrapper subagent's hosted orchestrator would see the prompt before delegating. Alternatives include `local-coder` for callers whose actual concern is offline-or-cost rather than prompt confidentiality.

## Extending

To add a new dispatch target — for a CLI not covered above, a custom local model server, a different hosted provider, or anything else:

1. Create `.claude/agents/<your-target>.md` with frontmatter:
   - `name: <your-target>`
   - `description:` — a sharp "use when" so the dispatcher picks correctly
   - `model: sonnet` (or whichever Claude tier orchestrates the wrapper logic; short aliases auto-track latest)

2. In the body, follow the wrapper pattern from `gemini-long-context.md` and `local-coder.md`:
   - Verify the underlying CLI / endpoint is available; stop with a clear error if not. Do not silently fall back to a different model.
   - Read the model name from an env var with a default (e.g. `${MY_MODEL:-default-id}`); don't hardcode versions.
   - Invoke the CLI with the user's task and surface the output verbatim. Pass user-supplied input via stdin or a temp file rather than constructing a shell string; validate any model name before passing it through.
   - Refuse and route elsewhere if the task is outside the wrapper's strengths — including refusing prompt-confidentiality routing requests, since the wrapper's orchestrator is hosted.

3. (Optional) Add a tier to this skill if the new target represents a new capability profile. Most additions fit an existing tier; only add a new tier when the routing decision genuinely needs to distinguish.

Examples of additions that fit existing tiers without skill changes: a wrapper around Together AI / Groq / OpenRouter (`fast` or `balanced` depending on the underlying model), a wrapper around a custom local vLLM server (`local`), a wrapper around a different long-context provider (`long-context`).

## Out of scope

- Routing for tasks outside this repo.
- Cost optimization beyond tier selection (no spending caps, no token budgets).
- Automated dispatch without user confirmation. The harness layer decides whether to act on a recommendation; the classifier never side-effects.
