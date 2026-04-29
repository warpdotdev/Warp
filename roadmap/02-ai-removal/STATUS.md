# 02 — AI removal

**Phase:** not-started
**Spec PR:** —
**Impl PRs:** —

## Scope

Rip out Warp's AI features: agentic mode UI, cloud-agent surfaces, inline AI suggestions, AI command palette, LLM-backed completion, and AI-only telemetry. See README §1.

## Sub-phases

- [ ] **2a — Audit doc (no code change).** Enumerate every file, module, crate, feature flag, telemetry event, and config key tied to AI. Output: `roadmap/02-ai-removal/AUDIT.md`, which drives 2b–2d. Reviewable as a single PR with no behavior change.
- [ ] **2b — Remove agent UI + cloud-mode codepaths.**
- [ ] **2c — Remove AI command palette + inline suggestions + LLM-backed completion.**
- [ ] **2d — Remove AI-only telemetry, feature flags, and dead config.**

## Notes

- Run the `simplify` skill on each sub-PR to catch dead code the rip-out leaves behind.
- Cherry-pick conflict cost begins here. After 2a merges, schedule a recurring upstream-watcher agent (weekly) to surface conflicting upstream commits early.
- The fork inherits Warp's MIT/AGPL split. Removing AI code shouldn't change licensing, but call out anything ambiguous in 2a's audit.

## Why this is feature 02 (not last)

The fork's identity is "no AI." Establishing that early matters more than minimizing cherry-pick conflict cost — and the conflict cost is unavoidable regardless of timing.
