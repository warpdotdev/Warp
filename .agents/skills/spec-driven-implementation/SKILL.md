---
name: spec-driven-implementation
description: Drive a spec-first workflow for substantial features by writing PRODUCT.md before implementation, writing TECH.md when warranted, and keeping both specs updated as implementation evolves. Use when starting a significant feature, planning agent-driven implementation, or when the user wants product and tech specs checked into source control.
---

# spec-driven-implementation

Drive a spec-first workflow for substantial features in Warp.

## Overview

Use this skill for significant features where a written spec will improve implementation quality, reduce ambiguity, or make review easier. Be pragmatic: not every change needs specs.

Specs should usually live in:

- `specs/<linear-ticket-number>/PRODUCT.md`
- `specs/<linear-ticket-number>/TECH.md`

For example:

- `specs/APP-1234/PRODUCT.md`
- `specs/APP-1234/TECH.md`

`specs/` should contain only ticket-named directories as direct children. Do not create engineer-named subdirectories or feature-slug directories there.

If a relevant Linear issue does not already exist, create one before writing specs. Use the Linear MCP tools directly:

- `list_teams` to find the appropriate team
- `list_issue_labels` to inspect the expected labels/tags
- `save_issue` to create the issue with the appropriate team and labels

If the correct team or labels are not obvious from the request and surrounding context, use `ask_user_question` to clarify rather than guessing.

These specs should largely be written by agents, not by hand, and should be checked into source control so they can be reviewed and kept current with the code.

## When specs are required

Strongly prefer specs when the change is substantial, such as:

- product or architectural ambiguity
- expected implementation size around 1k+ LOC
- deep or cross-cutting stack changes
- risky behavior changes where regressions would be expensive
- work where agent quality will improve materially from clearer inputs

Specs are often unnecessary for:

- small, local bug fixes
- straightforward refactors
- narrow UI tweaks with little ambiguity

For pure UI changes, the product spec is often useful while the tech spec may be unnecessary.

## Workflow

### 1. Decide whether the feature needs specs

Evaluate the size, ambiguity, and risk of the feature. If specs will not meaningfully improve execution or review, skip them and focus on verification instead.

### 2. Write the product spec first

Before implementation, create `PRODUCT.md` describing the desired user-facing behavior.

Use the `write-product-spec` skill to produce it. The product spec should define:

- what problem is being solved
- the desired user experience
- invariants and edge cases
- success criteria
- how the behavior will be validated

If the feature has UI or interaction design, ask for a Figma mock if one exists. If there is no mock, continue but call that out explicitly in the product spec.

Reference the Linear issue in the spec when one exists. Because specs live under `specs/<linear-ticket-number>/...`, this should usually be straightforward.

### 3. Write the tech spec when warranted

Use the `write-tech-spec` skill for substantial or ambiguous implementation work.

Prefer a tech spec when:

- the implementation spans multiple subsystems
- architecture or extensibility matters
- there are meaningful tradeoffs to document
- reviewers will benefit more from reviewing the plan than the raw code

It is acceptable to write the tech spec after an e2e prototype if that leads to a more accurate implementation plan. Do not force a premature tech spec when the implementation details are still too uncertain.

### 4. Implement approved specs

After the specs are approved, use the `implement-specs` skill to build from the approved `PRODUCT.md` and `TECH.md`.

The implementation can often be pushed in the same PR as the product and tech specs. As the engineer iterates, keep `PRODUCT.md`, `TECH.md`, code changes, and tests in that same PR so the review reflects the feature that will actually ship.

For large features, the implementer may optionally offer:

- `PROJECT_LOG.md` to track explored paths, checkpoints, and current implementation state
- `DECISIONS.md` to capture concrete product and technical decisions made during design and implementation

These are optional aids, not required outputs.

### 5. Keep specs current during implementation

If implementation changes from the spec, update the spec rather than leaving it stale.

Update `PRODUCT.md` when:

- user-facing behavior changes
- success criteria change
- UX details or edge cases change

Update `TECH.md` when:

- the implementation approach changes
- architectural boundaries move
- risks, dependencies, or rollout details change
- the testing or validation plan changes

The checked-in specs should describe the feature that actually ships, not just the initial intent. Keep those spec updates in the same PR as the related code changes whenever practical.

### 6. Verify behavior against the spec

Before considering the work complete, make sure verification maps back to the specs. Prefer tests and artifacts that validate the product behavior directly:

- use the `rust-unit-tests` skill for crate-level unit tests and regression coverage
- integration tests for critical user flows
- loom walkthroughs or equivalent feature demonstrations when appropriate
- screenshots or videos when useful for UI-heavy work

## Best Practices

- Be pragmatic above all else.
- Write specs to improve input quality for agents, not as ceremony.
- Keep product specs behavior-oriented and implementation-light.
- Keep tech specs implementation-oriented and grounded in current codebase patterns.
- Use review time to validate specs and behavior, not to over-index on code style nits.

## Related Skills

- `implement-specs`
- `write-product-spec`
- `write-tech-spec`
