---
name: 02 — AI removal
status: draft
---

# AI removal — PRODUCT

## Summary

twarp removes Warp's AI features. After this feature ships, every AI surface upstream Warp exposes — agentic mode, AI command palette, inline AI suggestions, AI assistant panel, AI settings page, AI onboarding slide, agent input footer, ambient agents — is gone. The terminal shell, blocks, history, themes, tabs, panes, command completion (non-LLM), Warp Drive, account login, and all other non-AI settings are unchanged.

The work ships as four sub-phases (2a-2d), each its own PR. Each phase ships an observable user state, but the **feature** is only "done" once 2d ships. The strategy uses Warp's existing master AI kill-switch (`agents.warp_agent.is_any_ai_enabled`), a fully-gated boolean already wired across every AI integration point. Phase 2b flips its default and removes the surface that turns it on; phases 2c/2d physically delete the now-unreachable code and trim its long tail.

## Goals / Non-goals

**Goals**
- After 2b ships, a freshly-installed twarp shows no AI UI in any state — no agent panel, no AI command palette, no inline AI suggestions, no AI settings page, no onboarding question about AI, no agent footer, no agent-management popup.
- After 2c ships, the AI source modules (`crates/ai/`, `crates/natural_language_detection/`, `app/src/ai/`, `app/src/ai_assistant/`) are physically deleted. `git grep` for top-level AI module names returns nothing in source.
- After 2d ships, AI-only telemetry events, settings keys, and feature flags are gone; in-tree docs no longer reference deleted AI subsystems.
- Upstream Warp's non-AI features (tabs, blocks, themes, Drive, completion, account login) are unaffected at every phase.
- Each sub-PR is independently reviewable: it compiles, presubmit is green, and its own smoke test passes before merge.

**Non-goals (deferred or excluded)**
- Replacing AI features with non-AI alternatives (a "natural language detector" replacement, a non-LLM "explain" tool, etc.). twarp simply doesn't offer them.
- A user-visible rebrand to advertise the AI absence ("AI-free" banners, splash text, etc.). The rebrand is feature 05.
- Touching non-AI onboarding slides (intent picker, theme picker, etc.) beyond what removing the agent slide forces.
- Detecting or refusing future upstream cherry-picks that re-introduce AI. Cherry-pick discipline is a separate cadence.
- Removing Warp Drive, account login, cloud sync, or any other non-AI cloud feature. Per the README, "no AI" ≠ "no cloud".
- Migrating per-user AI state (saved agents, blocklist entries, conversation history) from upstream Warp. That data is dropped silently on first launch.
- Resolving any MIT-vs-AGPL licensing implications surfaced by deleting AI code. Document anything ambiguous in 2a's GATE.md and follow up separately if needed.

## Behavior

The visible behavior changes per sub-phase. Each sub-section below describes the user-observable state once the corresponding sub-PR ships.

### 2a — Locate the gate (no behavior change)

Adds `roadmap/02-ai-removal/GATE.md` documenting the AI kill-switch (`is_any_ai_enabled`), every call site that reads it, the onboarding entry point (`disable_oz`), and the gated surface inventory. No code change. Behavior identical to upstream master: agent panel works, AI command palette works, AI settings page renders.

### 2b — Default AI off + remove the enable path

After this phase, twarp boots with AI disabled and offers no surface that flips it back on.

1. **Default-off boot.** A fresh twarp install (no prior settings) boots with `is_any_ai_enabled = false`. No AI surface is visible: no agent panel, no agent input footer, no AI command palette entries, no inline AI suggestions, no agent-management popup, no AI settings page. Application menus do not show AI items; menu structure compresses without empty separators where AI items used to live.

2. **No enable path.**
    - Onboarding no longer shows the agent slide ("Terminal" vs "Terminal + AI"). The flow proceeds straight to the next step (theme picker, etc.). The Terminal-only branch becomes the only branch.
    - The settings_view AI page is removed; its sidebar entry is gone.
    - There is no command-palette entry, keyboard shortcut, or settings toggle that flips `is_any_ai_enabled` to `true`. The settings key still **parses** (so legacy user TOML files don't crash), but the runtime treats it as always-`false`. Writing it has no effect.

3. **Migrating existing users.** A user upgrading from upstream Warp with AI enabled lands in twarp with AI off. Non-AI state (tabs, themes, Drive contents, blocks history, account session) is preserved. Per-user AI state (saved agents, blocklist entries, conversation history) is dropped silently — twarp does not migrate or surface a notice. The user is meant to know what twarp is when installing it.

4. **Telemetry.** AI-only telemetry events still exist in the event enum (cleanup is 2d), but they are unreachable because no AI code path runs.

### 2c — Delete dead AI code

After this phase, the AI modules are physically gone but no user-visible behavior changes from 2b. Same smoke surface as 2b; the validation here is structural.

The deletion is iterative — `simplify` between rounds — and proceeds in this order:

1. The four top-level AI modules: `app/src/ai_assistant/`, `app/src/ai/`, `crates/ai/`, `crates/natural_language_detection/`.
2. Their references in workspace `Cargo.toml` members and dependency lists.
3. Any function in non-AI modules whose only callers were the deleted AI modules. (`simplify` finds these.)
4. Any `mod ai;` / `pub use ai::*;` re-exports in `app/src/lib.rs`, `app/src/main.rs`, and parent `mod.rs` files.

Compilation must remain green and `./script/presubmit` must pass at the **final commit** of the PR. Intermediate commits may carry transient unused-warnings while the deletion teardown unfolds; the final state is what's reviewed for green CI. If physical deletion exposes a non-AI feature secretly leaning on an AI module (e.g., a shared util that happened to live under `app/src/ai/util/`), the util is moved out before the deletion, not left orphaned.

### 2d — Final sweep

After this phase, the long tail is clean:

1. **AI-only telemetry events** — variants in the telemetry event enums (`AIBlocklist`, `AICommandSearch`, `AgentManagementPopup`, `AgentManagementView`, `AIQueryTimeout`, `AICommandSearchOpened`, `InputAICommandSearch`, `InputAskWarpAI`, `WarpAIAction`, `OpenedWarpAI`, etc.) are removed from the enums and from any event-name lookup tables. Variants that are part of a mixed enum get their AI arms removed, not the whole enum.
2. **Orphan settings keys** — the `agents` settings block (including `agents.warp_agent.is_any_ai_enabled` and any sibling AI keys) is removed from the settings schema. Deserialization remains tolerant of unknown keys so legacy config files still parse.
3. **Orphan feature flags** — any `FeatureFlag::*` whose only consumer was AI is removed via the `remove-feature-flag` skill. Mixed flags are not removed.
4. **Documentation** — `WARP.md` references to AI subsystems and any internal docs under deleted module trees are removed or de-AI'd.

After 2d, `git grep -i "ai_assistant\|warp_agent\|is_any_ai_enabled\|natural_language_detection"` returns no matches in source files (test fixtures and `roadmap/` excepted).

## Smoke test

Each sub-phase has its own smoke test. All run against a freshly built twarp binary. "Fresh install" means deleting `~/Library/Application Support/twarp/` (or platform equivalent) before launch.

### 2a smoke test

1. Read `roadmap/02-ai-removal/GATE.md`. Verify it lists `is_any_ai_enabled` (declaration site, default value, reader function), the onboarding `disable_oz` mechanism, and at least the major call sites: settings AI page, root view session switching, app menus, agent panel, AI command palette, predict/suggest pipeline.
2. Open twarp (built from this branch). Confirm AI features still work end-to-end: open the agent panel, run a query, dismiss it; open the AI command palette, search for something; open Settings → AI and verify the page renders. 2a is documentation-only; behavior is unchanged from upstream master.

### 2b smoke test

1. Fresh install. Launch twarp. Walk through onboarding. There is no slide that mentions AI, agents, or "Terminal vs Terminal + AI"; the flow goes intent-picker → theme-picker (or whatever the post-AI-slide order is) → done.
2. After onboarding, the main window shows the terminal with no agent panel, no agent footer below the input, and no "ask AI" / "agent" entries in any menu (Application menu, Edit menu, View menu, etc.).
3. Open the command palette. Search for "ai", "agent", "ask", "explain". No AI-related commands appear.
4. Open Settings. Verify there is no "AI" or "Agents" page in the sidebar. Other pages (Appearance, Keybindings, Subscription/Account if present) render normally.
5. Type a command in the terminal (e.g. `git statu`). No inline AI suggestion bar / agent-suggestion balloon appears beneath the input. Tab completion (non-LLM) still works.
6. Hand-edit `~/Library/Application Support/twarp/settings.toml` to set `agents.warp_agent.is_any_ai_enabled = true`. Restart twarp. **No AI surface appears** — the runtime ignores the value because the gate reader is wired to always-false. No crash, no warning, no error log.
7. Quit twarp. Replace settings file with a fresh upstream-Warp-format settings file containing AI config (saved agents, blocklist entries, conversation history). Launch twarp. App boots cleanly, AI keys are silently ignored, non-AI settings (theme, font, keybindings) are applied normally. Tabs and blocks history present.

### 2c smoke test

1. `git grep -E "^(mod|pub mod) ai(_|;)" app/src/` — returns nothing.
2. `ls crates/ai crates/natural_language_detection 2>&1` — both report "No such file or directory".
3. `ls app/src/ai app/src/ai_assistant 2>&1` — both report "No such file or directory".
4. `./script/presubmit` is green.
5. Repeat 2b smoke test steps 1-7 — same observable behavior (this is the regression check that physical deletion didn't break the gated-off path).

### 2d smoke test

1. `git grep -wi "is_any_ai_enabled\|warp_agent\|ai_assistant\|natural_language_detection" -- ':!*.lock' ':!roadmap/'` — returns nothing in source/config; matches in `roadmap/` are spec-only and expected.
2. Open the telemetry event enums (search `app/src/server/telemetry/events.rs` for `AIBlocklist`, `AICommandSearch`, `WarpAIAction`, `OpenedWarpAI`, `AgentManagementPopup`). None remain. Mixed enums (if any) have their AI arms gone but other arms intact.
3. Open the feature-flag list. No flag whose name contains "ai", "agent", or "llm" remains as an AI-only flag (mixed flags retain non-AI arms).
4. `WARP.md` and any other in-tree docs (`docs/`, `app/src/**/README.md`) contain no live references to deleted AI modules.
5. Repeat 2b smoke test steps 1-7 — same observable behavior.
6. `./script/presubmit` is green.
