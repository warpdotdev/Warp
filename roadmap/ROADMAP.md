# twarp roadmap

Single source of truth for what's being built next. `/twarp-next` reads this file every invocation; the user reads it to see status at a glance.

**Currently active:** `02-ai-removal`

## Features

| # | Feature | Phase | Spec PR | Impl PR(s) |
|---|---------|-------|---------|-----------|
| 01 | [Tab color shortcuts](01-tab-colors/STATUS.md) | merged | [#2](https://github.com/timomak/twarp/pull/2) | [#3](https://github.com/timomak/twarp/pull/3) |
| 02 | [AI removal](02-ai-removal/STATUS.md) | spec-in-review | [#4](https://github.com/timomak/twarp/pull/4) | — |
| 03 | [Custom command shortcuts](03-command-shortcuts/STATUS.md) | not-started | — | — |
| 04 | [Open Changes panel](04-open-changes/STATUS.md) | not-started | — | — |
| 05 | [Rebrand to twarp](05-rebrand/STATUS.md) | not-started | — | — |

## Phases

- `not-started` — no work begun
- `spec-pending` — `/twarp-next` is writing PRODUCT.md / TECH.md
- `spec-in-review` — spec PR open, awaiting user review + merge
- `impl-pending` — specs merged, `/twarp-next` is implementing the next sub-phase
- `impl-in-review` — impl PR open, awaiting user review + merge
- `merged` — feature shipped

## Rules

- Only one feature is active at a time.
- A feature advances from `spec-in-review` → `impl-pending` only after the spec PR is **merged to master**.
- Features 02, 04, and 05 are sub-phased; their STATUS.md tracks individual sub-PRs and the feature only reaches `merged` after every sub-PR ships.
- The next feature only starts after the current one reaches `merged`.
- Git is the source of truth. If STATUS.md and `gh pr view` disagree, trust git and update STATUS.md.

## Order rationale

1. **Tab colors first** — smallest scope, validates the workflow at low risk; upstream has groundwork on `oz-agent/APP-4321-active-tab-color-indication`.
2. **AI removal second** — establishes the fork's identity. Cherry-pick conflicts from upstream become unavoidable from here, so eat the cost after the workflow is proven.
3. **Command shortcuts third** — independent subsystem, no dependency on 01 or 02.
4. **Open Changes panel fourth** — largest scope, sub-phased into panel scaffold → diffs → staging → commit/push → file timeline.
5. **Rebrand last** — file/crate renames are the worst case for git merges, so push them as late as possible to keep upstream cherry-picks clean. By feature 05, AI code is gone, so the brand surface to rename is smaller.

## Out of scope for `/twarp-next`

- **Upstream cherry-picks.** Run on a separate cadence — schedule a recurring agent (`/schedule`) to fetch, list new commits, and propose cherry-picks. Not driven by this skill.
- **CI / repo hygiene unrelated to the active feature.**

## Spec storage convention

For twarp roadmap features, specs live alongside `STATUS.md`:

```
roadmap/<NN-feature>/PRODUCT.md
roadmap/<NN-feature>/TECH.md
```

This intentionally overrides the repo's default `specs/<linear-ticket>/...` convention, because twarp roadmap features are not tracked in Linear.
