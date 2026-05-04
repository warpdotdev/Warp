# 02 — AI removal

**Phase:** impl-in-review
**Spec PR:** https://github.com/timomak/twarp/pull/4
**Impl PRs:** 2a — https://github.com/timomak/twarp/pull/6

## Scope

Rip out Warp's AI features: agentic mode UI, cloud-agent surfaces, inline AI suggestions, AI command palette, LLM-backed completion, and AI-only telemetry. See README §1.

## Strategy

Warp already supports an "AI disabled" path — onboarding lets the user decline AI, and the app gracefully degrades. Twarp piggybacks on that plumbing instead of cataloguing AI code by hand:

1. **Default to AI-disabled.** Set whatever flag / setting / onboarding-answer upstream uses to gate AI to default off. The onboarding question that lets the user pick "no AI" becomes the default (and may be reduced or removed once the alternative branch is gone).
2. **Remove the enable path.** Strip the code that turns AI on. With no caller, every AI module gated behind it is unreachable.
3. **Delete the dead code.** Iterate with `simplify` until nothing further collapses.

Smaller diff than auditing AI files one-by-one, and upstream cherry-picks touching AI features merge cleanly into the gated-off code path before being re-pruned.

## Sub-phases

- [x] **2a — Locate the gate.** Find the existing "AI disabled" mechanism upstream provides (likely a feature flag, settings key, or onboarding answer). Document where it's checked and what code it bypasses. Output: `roadmap/02-ai-removal/GATE.md`. Single PR, no behavior change.
- [ ] **2b — Default AI off + remove the enable path.** Default the gate to disabled; remove the UI/code that flips it on (or strip the onboarding question entirely if it's a binary choice). Behavior change: every install boots in no-AI mode. Diff stays small.
- [ ] **2c — Delete dead AI code.** With the enable path gone, every AI module reachable only from "gate on" is unreachable. Delete iteratively, running `simplify` between rounds until nothing further collapses.
- [ ] **2d — Final sweep.** AI-only telemetry events, feature flags whose only consumer was AI, config keys nothing reads.

## Notes

- Run the `simplify` skill on each sub-PR to catch dead code the rip-out leaves behind.
- Cherry-pick conflict cost is **lower** under this strategy than under file-by-file removal: until 2c starts, AI files still exist (just gated off), so upstream AI patches still apply mechanically. Cost spikes once 2c starts physically deleting modules.
- After 2a merges, schedule a recurring upstream-watcher agent (weekly) to surface conflicting upstream commits early.
- The fork inherits Warp's MIT/AGPL split. Removing AI code shouldn't change licensing, but call out anything ambiguous in 2a's gate doc.

## Why this is feature 02 (not last)

The fork's identity is "no AI." Establishing that early matters more than minimizing cherry-pick conflict cost — and the conflict cost is unavoidable regardless of timing.
