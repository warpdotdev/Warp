---
name: twarp-next
description: Drive twarp's roadmap one step at a time. Reads roadmap/ROADMAP.md, identifies the active feature and phase, and dispatches to spec writing, implementation, or status report. Use when the user runs /twarp-next or asks "what's next on twarp".
---

# twarp-next

Single entry point for twarp side-project work. The user runs this when they come back to the project; the skill figures out the right next action without further prompting.

The user's expected loop is **review and approve, nothing else**. Anything that needs human judgment becomes a PR; everything else is automated.

## Workflow

1. Read `roadmap/ROADMAP.md` — find the active feature (`Currently active:` line) and its declared phase from the table.
2. Read `roadmap/<feature>/STATUS.md` — get sub-phase detail and PR references.
3. **Reconcile against git.** For any PR referenced, run `gh pr view <num> --json state,mergedAt,url`. If git says merged but STATUS says `*-in-review`, advance the phase before doing anything else.
4. Dispatch based on the (now-reconciled) phase using the table below.
5. Update `STATUS.md` and the `ROADMAP.md` table on completion of each step.

## Phase → action

| Phase | Action |
|-------|--------|
| `not-started` | Briefly confirm scope with the user (one-line summary from STATUS.md). On confirmation, set phase to `spec-pending` and proceed. |
| `spec-pending` | Use `write-product-spec` to fill `roadmap/<feature>/PRODUCT.md` (must include a smoke-test checklist — see "Smoke-test checklist" below). Then `write-tech-spec` to fill `roadmap/<feature>/TECH.md`. Open a spec PR via `create-pr`, title `[twarp NN] specs: <feature>`. Set phase to `spec-in-review`. |
| `spec-in-review` | Status report only. **Do not modify code.** Tell the user what they're waiting on. |
| `impl-pending` | Implement the next unchecked sub-phase using `implement-specs`, scoped only to that sub-phase. Run `./script/presubmit`; if it fails, use `diagnose-ci-failures` / `fix-errors` and iterate until green. Open an impl PR via `create-pr`, title `[twarp NN<sub>] <feature>: <sub-phase summary>`. Tick the sub-phase checkbox in STATUS.md. Set phase to `impl-in-review`. |
| `impl-in-review` | Status report only. **Do not modify code.** |
| `merged` | If unchecked sub-phases remain in STATUS.md, set phase back to `impl-pending` and recurse once. Otherwise update the `Currently active:` line in ROADMAP.md to the next feature, set its row to `not-started`, and recurse once. |

## Hard rules

- **Only one feature active at a time.** Never start feature N+1 while N has unchecked sub-phases.
- **Never auto-merge.** Open the PR; the user merges.
- **Never skip the spec PR.** Implementation cannot start until PRODUCT.md and TECH.md are merged to master.
- **Specs must include a smoke-test checklist.** The user uses it to validate the impl PR.
- **CI must pass before reporting "ready for review".** Iterate on red presubmit until green; never hand the user a red PR.
- **Sub-phased features (02, 04)** complete every sub-PR before reaching `merged`.
- **Git is authoritative.** If STATUS.md disagrees with `gh pr view`, trust git and fix STATUS.md.
- **Don't touch upstream cherry-picks.** That's a separate workflow run on its own cadence.
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

## Argument handling

- `/twarp-next` (no args) — full workflow above.
- `/twarp-next status` — status report only, even if work could be done. Useful when the user just wants a sitrep.
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
- Doesn't manage upstream cherry-picks — separate cadence.
- Doesn't modify the four feature definitions in README.md — those are the contract. If scope needs to change, that's a human decision and a separate PR.
