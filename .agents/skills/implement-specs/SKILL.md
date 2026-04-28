---
name: implement-specs
description: Implement an approved feature from PRODUCT.md and TECH.md, keeping specs and code aligned in the same PR as implementation evolves. Use after the product and tech specs are approved and the next step is building the feature.
---

# implement-specs

Implement an approved feature from `PRODUCT.md` and `TECH.md`.

## Overview

Use this skill after the product and tech specs are approved. The goal is to build the feature described by the specs while keeping the checked-in specs and the implementation aligned as the work evolves.

Approved specs should live directly under a ticket-named directory in `specs/`, for example `specs/APP-1234/PRODUCT.md` and `specs/APP-1234/TECH.md`.

In many cases, the implementation should be pushed in the same PR as the product and tech specs. As the engineer iterates, changes to `PRODUCT.md`, `TECH.md`, and the code should all be pushed in that same PR so review stays anchored to the feature that will actually ship.

## Prerequisites

Before using this skill:

- confirm that `PRODUCT.md` exists
- confirm that `TECH.md` exists when the feature warranted one
- confirm that the relevant specs have been reviewed and approved enough to start implementation

## Workflow

### 1. Read the approved specs first

Treat:

- `PRODUCT.md` as the source of truth for user-facing behavior
- `TECH.md` as the source of truth for architecture, sequencing, and implementation shape

Make sure you understand the expected behavior, constraints, risks, and validation plan before writing code.

### 2. Offer optional implementation aids for large features

For large or long-running features, optionally offer one of these aids to the user before implementation begins:

- `PROJECT_LOG.md` to track checkpoints, explored paths, partial findings, and current implementation state
- `DECISIONS.md` to capture concrete product and technical decisions made during the PRD and tech design process

These are optional aids, not required deliverables. Offer them when they would reduce confusion or help future agents avoid re-exploring the same paths.

### 3. Plan and implement against the specs

Break the work into concrete implementation steps, then implement the feature against the approved specs.

During implementation:

- keep behavior aligned with `PRODUCT.md`
- keep architecture and sequencing aligned with `TECH.md`
- add or update tests and verification artifacts as the work lands

Use the same PR for the specs and implementation when practical so the full feature evolution is reviewable in one place.

### 4. Update specs as the implementation evolves

If implementation reveals that the intended behavior or design should change, update the checked-in specs rather than letting them go stale.

In particular:

- update `PRODUCT.md` when user-facing behavior, UX, edge cases, or success criteria change
- update `TECH.md` when architecture, sequencing, module boundaries, or validation strategy change
- keep those updates in the same PR as the corresponding code changes

The PR should describe the feature that actually ships, not just the initial draft of the specs.

### 5. Verify against the specs

Before considering the work complete, verify that the code matches the current specs.

Prefer:

- `rust-unit-tests` for unit tests and regression coverage
- integration or end-to-end tests for important user flows

## Best Practices

- Keep specs and code synchronized throughout implementation.
- Prefer updating the spec immediately when decisions change rather than batching spec cleanup until the end.
- Use optional tracking documents only when they add real value for a complex feature.
- Keep the same PR coherent: spec updates, code changes, tests, and optional tracking docs should all support the same feature narrative.

## Related Skills

- `spec-driven-implementation`
- `write-product-spec`
- `write-tech-spec`
- `rust-unit-tests`
