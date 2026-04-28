---
name: write-tech-spec
description: Write a TECH.md spec for a significant Warp feature after researching the current codebase and implementation constraints. Use when the user asks for a technical spec, implementation plan, or architecture doc tied to a product spec.
---

# write-tech-spec

Write a `TECH.md` spec for a significant feature in Warp.

## Overview

The tech spec should translate product intent into an implementation plan that fits the existing codebase, documents architectural choices, and makes the work easier for agents to execute and reviewers to evaluate.

Write specs to `specs/<id>/TECH.md`, where `<id>` is one of:

- a Linear ticket number (e.g. `specs/APP-1234/TECH.md`)
- a GitHub issue id, prefixed with `gh-` (e.g. `specs/gh-4567/TECH.md`)
- a short kebab-case feature name (e.g. `specs/vertical-tabs-hover-sidecar/TECH.md`)

Match the id used by the sibling `PRODUCT.md` when one exists. `specs/` should contain only id-named directories as direct children.

Ticket / issue references are optional. If the user has a Linear ticket or GitHub issue, use its id. If they don't, ask them for a feature name to use as the directory. Only create a new Linear ticket or GitHub issue when the user explicitly asks for one; in that case use the Linear MCP tools or `gh` CLI respectively (and `ask_user_question` if team, labels, or repo are unclear).

## When to use

Use this skill when the implementation spans multiple modules, has meaningful architectural tradeoffs, or when reviewers will benefit from seeing the plan before or alongside the code. For pure UI changes or straightforward fixes, a tech spec is often unnecessary.

Prefer to have a `PRODUCT.md` first so the technical plan is anchored to agreed behavior. If the implementation is still too uncertain, build an e2e prototype first and then write the tech spec from what was learned.

## Research before writing

Before drafting, read the product spec (if any), inspect the relevant code, and identify the main files, types, data flow, and ownership boundaries. Do not guess about current architecture when the code can be inspected directly.

## Structure

Required sections:

1. **Context** — What's being built, how the current system works in the area being changed, and the most relevant files with line references. Combine the "problem," "current state," and "relevant code" into one grounded section. Example references:
   - `app/src/workspace/mod.rs:42` — entry point for the user flow
   - `app/src/workspace/workspace.rs (120-220)` — state and event handling that will likely change
   Reference `PRODUCT.md` for user-visible behavior rather than restating it.
2. **Proposed changes** — The implementation plan: which modules change, new types/APIs/state being introduced, data flow, ownership boundaries, and how the design follows existing patterns. Call out tradeoffs when there is more than one reasonable path.
3. **Testing and validation** — How the implementation will be verified against the product behavior. Owns everything about proving the feature works: unit tests, integration tests, manual steps, screenshots, videos, and any other verification. Reference the numbered Behavior invariants from `PRODUCT.md` directly rather than restating them; each important invariant should map to a concrete test or verification step. This section is where validation lives — `PRODUCT.md` intentionally does not have a Validation section.

Optional sections — include only when they add signal. Omit the heading entirely if empty; do not write "None" as a placeholder.

- **End-to-end flow** — Include only when tracing the path through the system tells you something the Proposed changes list doesn't.
- **Diagram** — Include a Mermaid diagram only when a visual will explain the design faster than prose (data flow, state transitions, sequence across layers). Prefer one or two focused diagrams over decorative ones.
- **Risks and mitigations** — Include when there are real failure modes, regressions, migration concerns, or rollout hazards worth calling out.
- **Parallelization** — Include when work can cleanly split across multiple agents and that split is non-obvious.
- **Follow-ups** — Include when there is deferred cleanup or future work worth naming.

## Length heuristic

Right-size the spec to the feature:

- Single-file change with clear approach: skip the tech spec or keep it under ~40 lines.
- Multi-module change with some ambiguity: target ~80–150 lines.
- Large cross-cutting or architecturally novel change: longer is fine when every section earns its place.

If Context and Proposed changes end up describing the same files and state from different angles, collapse them.

## Writing guidance

- Ground the plan in actual codebase structure and patterns.
- Prefer concrete implementation guidance over generic architecture language.
- Explain why the proposed design fits this repo.
- Reference `PRODUCT.md` for behavior instead of restating it.
- Each section should earn its place — if a section would repeat another or contain only boilerplate, omit it.

## Keep the spec current

Approved specs may ship in the same PR as the implementation. Update `TECH.md` in the same PR when module boundaries, implementation sequencing, risks, validation strategy, or rollout assumptions change. The checked-in spec should describe the implementation that actually ships.

For large features, the implementer may optionally keep a `DECISIONS.md` file summarizing concrete decisions. Offer it when it would help future agents; otherwise skip it.

## Related Skills

- `implement-specs`
- `write-product-spec`
- `spec-driven-implementation`
