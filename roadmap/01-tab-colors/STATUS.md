# 01 — Tab color shortcuts

**Phase:** spec-in-review
**Spec PR:** [#1](https://github.com/timomak/twarp/pull/1)
**Impl PR:** —

## Scope

Keyboard shortcuts (⌘⌥1–8 for color, ⌘⌥0 to reset) that set the active tab's color. See README §2 for the full table and intent.

## Approach note

Build on top of upstream `oz-agent/APP-4321-active-tab-color-indication` if that branch has landed by the time the spec is written; otherwise implement the per-tab color indicator surface from scratch. This decision belongs in TECH.md, not here — flag it for the spec phase.

## Sub-phases

Single phase — one PRODUCT.md + TECH.md, one impl PR.

## Why this is feature 01

Smallest scope of the four; validates the whole spec → review → impl → review loop with the least blast radius. If something about the workflow is wrong, we find out here.
