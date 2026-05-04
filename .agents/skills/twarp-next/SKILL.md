---
name: twarp-next
description: Drive twarp's roadmap one step at a time. Reads roadmap/ROADMAP.md, identifies the active feature and phase, and dispatches to spec writing, implementation, status report, or post-merge upstream-contribution assessment. Use when the user runs /twarp-next or asks "what's next on twarp".
---

# twarp-next

Single entry point for twarp side-project work. The user runs this when they come back to the project; the skill figures out the right next action without further prompting.

The user's expected loop is **review and approve, nothing else**. Anything that needs human judgment becomes a PR; everything else is automated. **Invoking `/twarp-next` is itself the user's confirmation to proceed** — never pause to ask "shall I start?" or "ready to begin?". The only stopping points are `*-in-review` phases, where the user is reviewing a real PR.

## Workflow

1. Read `roadmap/ROADMAP.md` — find the active feature (`Currently active:` line) and its declared phase from the table.
2. Read `roadmap/<feature>/STATUS.md` — get sub-phase detail and PR references.
3. **Reconcile against git.** For any PR referenced, run `gh pr view <num> --json state,mergedAt,url`. If git says merged but STATUS says `*-in-review`, advance the phase before doing anything else.
4. Dispatch based on the (now-reconciled) phase using the table below.
5. Update `STATUS.md` and the `ROADMAP.md` table on completion of each step.

## Phase → action

| Phase | Action |
|-------|--------|
| `not-started` | Set phase to `spec-pending` and recurse once. Invoking `/twarp-next` is the confirmation — do not ask. |
| `spec-pending` | Use `write-product-spec` to fill `roadmap/<feature>/PRODUCT.md` (must include a smoke-test checklist — see "Smoke-test checklist" below). Then `write-tech-spec` to fill `roadmap/<feature>/TECH.md`. Open a spec PR via `create-pr`, title `[twarp NN] specs: <feature>`. Set phase to `spec-in-review`. |
| `spec-in-review` | Status report only. **Do not modify code.** Tell the user what they're waiting on. |
| `impl-pending` | Implement the next unchecked sub-phase using `implement-specs`, scoped only to that sub-phase. Run `./script/presubmit`; if it fails, use `diagnose-ci-failures` / `fix-errors` and iterate until green. Open an impl PR via `create-pr`, title `[twarp NN<sub>] <feature>: <sub-phase summary>`. Tick the sub-phase checkbox in STATUS.md. Set phase to `impl-in-review`. |
| `impl-in-review` | Status report only. **Do not modify code.** |
| `merged` | If unchecked sub-phases remain in STATUS.md, set phase back to `impl-pending` and recurse once. Otherwise update the `Currently active:` line in ROADMAP.md to the next feature, set its row to `not-started`, and recurse once. The just-merged feature is now eligible for `/twarp-next upstream NN` (see "Upstream assessment" below) — mention it once in the next status output, then move on. |

## Hard rules

- **Only one feature active at a time.** Never start feature N+1 while N has unchecked sub-phases.
- **Never auto-merge.** Open the PR; the user merges.
- **Never skip the spec PR.** Implementation cannot start until PRODUCT.md and TECH.md are merged to master.
- **Specs must include a smoke-test checklist.** The user uses it to validate the impl PR.
- **CI must pass before reporting "ready for review".** Iterate on red presubmit until green; never hand the user a red PR.
- **Sub-phased features (02, 05, 06)** complete every sub-PR before reaching `merged`.
- **Git is authoritative.** If STATUS.md disagrees with `gh pr view`, trust git and fix STATUS.md.
- **Don't touch upstream cherry-picks.** That's a separate workflow run on its own cadence.
- **Upstream assessment is read-only.** Never run `gh issue create`, `gh pr create`, or any commenting / pushing command against `warpdotdev/warp`. Drafts are written to `roadmap/<feature>/UPSTREAM.md` for the user to submit by hand.
- **Upstream assessment never blocks the roadmap.** A feature reaching `merged` immediately advances to the next; the upstream check is only run when the user explicitly asks via `/twarp-next upstream [NN]`.
- **Never edit ROADMAP.md to reorder features.** That's a human decision; if the user wants to reorder, they'll do it.

## Smoke-test checklist

Every PRODUCT.md must end with a `## Smoke test` section. This is a numbered list of concrete steps the user runs against a built twarp binary to validate the feature works. Example for tab colors:

```
## Smoke test
1. Open twarp. Open three tabs.
2. Focus tab 2. Press ⌘⌥1. Tab 2 indicator turns red.
3. Focus tab 2. Press ⌘⌥0. Tab 2 indicator returns to default.
4. Press ⌘⌥4 on tab 3. Tab 3 indicator turns green; tab 2 unaffected.
5. Restart twarp. Tab colors persist.
```

If a sub-phase has its own smoke test (e.g. 4a vs 4b), include sub-headings.

## Status report format

When the active phase is `*-in-review`, or there is otherwise nothing to do without user action, output **exactly** this and stop:

```
twarp status
  Active:  <NN — feature name>
  Phase:   <phase>
  PR:      <gh url, or — if none>
  Waiting: <user review | CI | other>
  Next:    <one line: what happens after this clears>
```

No code edits, no preamble, no follow-up offers.

## Upstream assessment (opt-in, post-merge)

Once a feature reaches `merged`, twarp can optionally pitch the underlying mechanism upstream to `warpdotdev/warp`. This workflow is **read-only and report-only** — it never files issues or PRs against upstream. The user takes the publishing action by hand after reviewing the draft.

**Trigger:** explicit `/twarp-next upstream [NN]` only. Never automatic. The standard status report after a merge mentions this command once as an optional follow-up; that is the only nudge.

**Skip list (refuse and exit):**

- **02 (AI removal).** Permanent skip. Refuse with: "AI removal is twarp's identity, not an upstream contribution candidate."
- Any feature whose phase isn't `merged`. Refuse with: "Feature NN hasn't shipped yet — finish the impl loop first."

**Workflow:**

1. Validate against the skip list. Bail early if hit.
2. Read `roadmap/<feature>/PRODUCT.md` and the corresponding README section to recover the feature's scope and intent.
3. Search upstream for the **nearest open work**. Run all three; combine results:
   ```bash
   gh pr list    --repo warpdotdev/warp --state open --search "<keywords>"
   gh issue list --repo warpdotdev/warp --state open --search "<keywords>"
   gh api 'repos/warpdotdev/warp/branches?per_page=100' --jq '.[].name' \
     | grep -iE "<keywords>"
   ```
   Derive keywords from the feature name and PRODUCT.md scope (e.g. for `01-tab-colors`: `tab color`, `tab indicator`, `APP-4321`).
4. Classify the closest match and pick a recommendation:
   - **Open PR closely related** → `contribute`. Output the URL, the author, and a one-paragraph suggested comment offering to help. Do **not** post.
   - **Branch with active commits but no PR yet** (e.g. `oz-agent/APP-4321-active-tab-color-indication`) → `coordinate`. Output the branch URL and the author handle; suggest reaching out before duplicating work.
   - **Open issue with `ready-to-implement`** → `target-issue`. Output the URL; this is the cleanest path to a PR.
   - **Open issue without that label** → `request-label`. Output the URL and a short draft comment asking `@oss-maintainers` to consider the readiness label.
   - **Nothing close** → `new-issue`. Draft the issue body to `roadmap/<feature>/UPSTREAM.md` for the user to review and submit by hand. Title pattern: `[Feature] <generic primitive>`. Body: problem, proposed behavior, sketch of API/config, link to twarp's merged impl as a reference implementation.
5. **Strip AI-adjacent framing** from any draft or suggested comment. Feature 04 (custom command shortcuts) in particular: the upstream pitch is the **generic shortcut-to-action primitive**, not the "auto-type `claude`" examples. Replace AI-tooling examples with neutral ones (e.g. "auto-type `git status`", "open a tab and run a frequently-used command sequence"). If the AI-adjacency is intrinsic to the feature (it isn't, for any of 01/03/04/05), bail and recommend `skip`.
6. Output the report block below and stop. Do not loop, do not retry, do not act on the recommendation.

**Failure modes:** if any `gh` call fails (auth, rate limit, network), output the report with `Nearest: unknown` and `Recommend: retry`. Do not retry in a loop; the user reruns when ready.

**Report format:**

```
twarp upstream — <NN feature name>
  Nearest:    <pr | issue | branch | none | unknown> — <url or — >
  Recommend:  <contribute | coordinate | target-issue | request-label | new-issue | retry>
  Draft:      <path to roadmap/<feature>/UPSTREAM.md, or — >
  Notes:      <one line; e.g. "AI-adjacent examples stripped from draft">
```

## Argument handling

- `/twarp-next` (no args) — full workflow above.
- `/twarp-next status` — status report only, even if work could be done. Useful when the user just wants a sitrep.
- `/twarp-next upstream [NN]` — run the upstream-assessment workflow for feature `NN` (or, if omitted, the most recently merged feature). Read-only: scans `warpdotdev/warp` for related PRs/issues/branches, drafts `roadmap/<feature>/UPSTREAM.md` if the recommendation is `new-issue`, prints the report, and stops. Refuses for `02-ai-removal` and for any feature not yet `merged`.
- `/twarp-next reset` — refuse. State changes that destroy work need explicit human action; tell the user to edit `ROADMAP.md` and the relevant `STATUS.md` themselves.

## Child skills used

- `write-product-spec` — fill PRODUCT.md (override its default `specs/<linear-ticket>/` path; write to `roadmap/<feature>/PRODUCT.md`)
- `write-tech-spec` — fill TECH.md (same path override)
- `implement-specs` — write code from approved specs (point it at the merged spec files)
- `create-pr` — open PR with structured description
- `diagnose-ci-failures` — investigate red CI
- `fix-errors` — recover from build/test failures
- `simplify` — clean up dead code (especially during 02 sub-phases)
- `add-feature-flag` / `remove-feature-flag` / `promote-feature` — for any flag-gated rollouts inside a feature
- `warp-integration-test` / `rust-unit-tests` — test coverage

## What this skill does NOT do

- Doesn't pick the order — that's in ROADMAP.md, set by humans.
- Doesn't merge PRs — only the user merges.
- Doesn't manage upstream cherry-picks (upstream → twarp) — separate cadence.
- Doesn't publish to upstream (twarp → `warpdotdev/warp`) — the upstream-assessment workflow drafts and reports only; the user files the issue or PR by hand.
- Doesn't modify the four feature definitions in README.md — those are the contract. If scope needs to change, that's a human decision and a separate PR.
