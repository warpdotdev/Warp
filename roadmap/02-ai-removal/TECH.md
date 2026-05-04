---
name: 02 — AI removal
status: draft
---

# AI removal — TECH

Companion to [PRODUCT.md](PRODUCT.md). Section numbers below reference PRODUCT.md.

## Context

Warp already ships a complete "AI off" path. Onboarding asks the user whether they want AI; declining sets `agents.warp_agent.is_any_ai_enabled = false`, and that boolean is checked at every AI integration point — menus, settings rendering, root-view session switching, AI command palette, agent panel, predict/suggest pipeline, agent footer, etc. twarp's strategy is to make the "off" path the *only* path: default the boolean to `false`, force the reader to return `false`, remove the UI that flips it on, then physically delete the now-unreachable code.

This is intentionally less fragile than identifying AI files by hand. As long as no upstream PR fundamentally restructures the kill-switch — and any such PR would be a large, visible change — twarp's removal stays mechanical. During 2a-2b the cherry-pick path stays clean: incoming AI patches still apply against the gated-off code. Cherry-pick conflict cost spikes only once 2c lands.

Relevant files on master (research output; the full inventory ships as `roadmap/02-ai-removal/GATE.md` from 2a):

- `app/src/settings/ai.rs:712-720` — `is_any_ai_enabled` setting declaration in `agents.warp_agent`. Default `true`. **Phase 2b flips this default.**
- `app/src/settings/ai.rs:1499` — `AISettings::is_any_ai_enabled(app: &AppContext) -> bool` reader. **Phase 2b changes this to always return `false`.**
- `crates/onboarding/src/slides/agent_slide.rs:111` — `disable_oz` flag in onboarding agent slide (line 942-953 in the same file is the slide's checkbox UI). **Phase 2b deletes this slide.**
- `crates/onboarding/src/model.rs:86-91` — Terminal-intent → AI-disabled mapping. **Phase 2b removes the AI half of this mapping.**
- `app/src/settings/onboarding.rs:24,57` — translates onboarding choice into `AISettings::handle(app).update(...is_any_ai_enabled, ctx)`. **Phase 2b removes the AI half** (or the file, if it's AI-only).
- `app/src/settings_view/ai_page.rs` — settings AI page. **Phase 2b deletes the file** and removes its sidebar registration.
- `app/src/app_menus.rs:990,1013,1020` — AI menu entries gated on `is_any_ai_enabled`. **Phase 2b deletes the entries** (not just the gate).
- `app/src/root_view.rs:2446` — session-mode switching gated on `is_any_ai_enabled`. **Phase 2b removes the AI branch.**
- `app/src/ai_assistant/panel.rs` — agent conversation panel. **Whole module deleted in 2c.**
- `app/src/ai/blocklist/agent_view/agent_input_footer/` — agent footer in input bar. **Whole module deleted in 2c.**
- `app/src/ai/predict/generate_ai_input_suggestions.rs` — LLM-backed suggestions. **Whole module deleted in 2c.**
- `app/src/ai/ambient_agents/`, `app/src/ai/execution_profiles/`, `app/src/ai/cloud_agent_config/` — AI subsystems under `app/src/ai/`. **All deleted in 2c.**
- `crates/ai/` — self-contained AI infrastructure crate (LLM clients, codebase context, telemetry helpers). **Whole crate deleted in 2c.**
- `crates/natural_language_detection/` — natural-language vs shell detector for the AI command palette. **Whole crate deleted in 2c.**
- `app/src/server/telemetry/events.rs:725,780,794,802,872,1556,1606,1613,1630,1631` — AI-only telemetry variants (`AIBlocklist`, `AICommandSearch`, `AgentManagementPopup`, `AgentManagementView`, `AIQueryTimeout`, `AICommandSearchOpened`, `OpenedWarpAI`, `WarpAIAction`, `InputAICommandSearch`, `InputAskWarpAI`). **Phase 2d deletes them** plus any string lookup tables that name them.
- `crates/ai/src/telemetry.rs` — `AITelemetryEvent` enum. **Removed when the crate is deleted in 2c**; non-AI references to `AITelemetryEvent` (if any) are 2d.

## Proposed changes

The four sub-phases each ship as a separate PR. Each PR's title is `[twarp 02<sub>] ai-removal: <one-line>`.

### 2a — Locate the gate

**Output:** new file `roadmap/02-ai-removal/GATE.md`. No code change.

GATE.md sections:

1. **The kill-switch.** `agents.warp_agent.is_any_ai_enabled: bool`, default `true`, declared at `app/src/settings/ai.rs:712`. Reader at `app/src/settings/ai.rs:1499`.
2. **Every call site.** Output of `git grep -n is_any_ai_enabled` with one-line "what is gated" descriptions, grouped by category (menus, settings page, onboarding, panels, command palette, predict pipeline, telemetry).
3. **Onboarding entry point.** `crates/onboarding/src/slides/agent_slide.rs:111` (`disable_oz`); `crates/onboarding/src/model.rs:86` (intent → AI off); `app/src/settings/onboarding.rs:24,57` (writes the result).
4. **Surface inventory.** Map each surface from PRODUCT §2b to its gating site. Cross-reference against the seven surfaces listed in `app/src/ai/` subdirectories and `app/src/ai_assistant/panel.rs`.
5. **Licensing note.** twarp inherits Warp's MIT/AGPL split — `warpui_core` and `warpui` are MIT, the rest is AGPL. AI modules live under AGPL; document any cross-references that cross the boundary.
6. **Cherry-pick implications.** Note the files most likely to attract upstream churn (`app/src/ai/predict/`, `crates/ai/src/`, telemetry events). 2a-2b leaves them in place gated off; 2c is the cliff.

This PR is documentation-only; the reviewer is verifying the map is correct, not approving any code change. No `./script/presubmit` run needed (no source changes), but the file lints clean against any markdown-lint config the repo carries.

After 2a merges, set up the recurring upstream-watcher (`/schedule`-driven, separate workflow) if not already running. Mention it in the PR description; don't gate the PR on it.

### 2b — Default AI off + remove the enable path

**Diff shape:** small. Code-deletion-heavy in onboarding and settings_view, plus a default-flip and reader change.

1. **Flip the default.** `app/src/settings/ai.rs:712` — change `is_any_ai_enabled` default from `true` to `false`. Existing user settings files override the default; step 2 makes the runtime ignore the value.
2. **Force-off the reader.** `AISettings::is_any_ai_enabled(app)` at `app/src/settings/ai.rs:1499` — replace the body with `false`. Every existing call site continues to compile, every gated branch becomes dead.
3. **Bypass the onboarding agent slide.** Per GATE §"Notes for sub-phase 2b", do not physically delete `crates/onboarding/src/slides/agent_slide.rs` in 2b; the model still references `AgentDevelopmentSettings`, `AgentAutonomy`, `OnboardingModelInfo`, and `AgentSlideAction` from that source, and surgically removing those while leaving the slide file in place "invites churn". 2c removes the slide source and the model field together. For 2b, instead: short-circuit `OnboardingStateModel::next` and `OnboardingStateModel::back` so the user never reaches `OnboardingStep::Agent` regardless of intention or `OpenWarpNewSettingsModes` flag state, and force `SelectedSettings::is_ai_enabled` (`crates/onboarding/src/model.rs:80-92`) to always return `false` so post-onboarding code that branches on AI enablement collapses to the AI-off path.
4. **Hide the settings AI page from the sidebar.** Remove the "Agents" umbrella entry from `nav_items` in `app/src/settings_view/mod.rs` so the user cannot navigate to AI settings. The backing `ai_page.rs` module and the AI-related `SettingsSection` variants remain compiled but unreachable in this phase. Physical deletion of `app/src/settings_view/ai_page.rs` is deferred to 2c because `cli_agent_settings_widget_id()` is consumed by `app/src/ai/blocklist/agent_view/agent_input_footer/`, which lives under the 2c deletion scope; deleting both together avoids cross-phase dangling references. Empty `mod ai_page;`, the `AISettingsPageView` field, the `SettingsAction::AI` variant, and the registration touchpoints listed in GATE §4 also stay in place for 2c.
5. **Trim AI menu entries.** `app/src/app_menus.rs:990,1013,1020` — delete the entries entirely (not just the gate). Confirm visually that no empty separators remain.
6. **Trim root-view AI branch.** `app/src/root_view.rs:2446` — delete the AI branch and the conditional. Whatever the non-AI fallback was becomes the only path.
7. **Run `simplify`** on the resulting tree to catch obviously-dead code (unused imports, empty match arms, unreachable patterns). Don't aggressively delete in 2b — that's 2c. Cleanup that's mechanical ships here; cleanup that requires judgment ships in 2c.
8. **Smoke test.** PRODUCT §2b smoke test, all 7 steps. Steps 1-5 are user-facing validation; steps 6-7 are the regression check that the runtime really ignores `is_any_ai_enabled = true` in legacy settings files.

PR title: `[twarp 02b] ai-removal: default off and strip enable path`.

### 2c — Delete dead AI code

**Diff shape:** large by line count (entire modules deleted), small by review surface (mechanical deletions). Iterative: 3-5 commits, `simplify` between each.

Order of deletion (leaves first):

1. `app/src/ai_assistant/` (panel) — likely the cleanest call graph; delete first.
2. `app/src/ai/`, internal subdirs first:
   - `app/src/ai/predict/` (suggestions pipeline)
   - `app/src/ai/blocklist/` (agent permission UI, includes `agent_view/agent_input_footer/`)
   - `app/src/ai/ambient_agents/`
   - `app/src/ai/execution_profiles/`
   - `app/src/ai/cloud_agent_config/`
   - any remaining subdirs
   - finally `app/src/ai/mod.rs` and `mod ai;` from `app/src/lib.rs` / `app/src/main.rs`.
3. `crates/ai/` — remove from workspace `Cargo.toml`, from any `[dependencies]` blocks that reference `warp-ai` or whatever the crate name is, then delete the crate directory.
4. `crates/natural_language_detection/` — same pattern as `crates/ai/`.

**Per-iteration validation.** After each commit: `cargo check` (fast), `cargo clippy --workspace -- -D warnings` (catches dead code). Don't batch deletions in one massive commit — review needs to follow the dependency teardown.

**Run `simplify`** between iterations to surface dead code the deletion exposes (e.g., a util whose only callers were AI).

**Files that look AI-named but cross the boundary** (extract before delete):
- Generic utilities under `app/src/ai/util/` or similar — move to `app/src/util/` if any non-AI module imports them. Check via `git grep -l "use crate::ai::"` before each top-level deletion.
- Trait definitions used by both AI and non-AI code (none expected; verify).
- Telemetry helpers in `crates/ai/src/telemetry.rs` — if any non-AI telemetry references `AITelemetryEvent`, fix the reference before crate deletion. (Likely a stub — telemetry references usually go the other way.)

`./script/presubmit` runs at the **last commit** of the PR with all deletions in place. Intermediate commits may carry transient dead-code warnings; the final commit is what gets reviewed for green CI. Each intermediate commit must individually compile (so the reviewer can `git bisect` if a regression surfaces post-merge).

PR title: `[twarp 02c] ai-removal: delete dead AI modules`.

### 2d — Final sweep

**Diff shape:** small. Targeted cleanup of AI-only telemetry, settings keys, feature flags, and docs.

1. **Telemetry events.** Delete the AI-only variants from `app/src/server/telemetry/events.rs`: `AIBlocklist`, `AICommandSearch`, `AgentManagementPopup`, `AgentManagementView`, `AIQueryTimeout`, `AICommandSearchOpened`, `InputAICommandSearch`, `InputAskWarpAI`, `WarpAIAction`, `OpenedWarpAI`. Remove the corresponding strings from any name-lookup table or schema export. `git grep` each variant name to confirm no non-AI code references it.

2. **Settings schema.** Delete the `agents` settings block in `app/src/settings/`. Add a deserialize-tolerance test that confirms a legacy upstream-Warp `settings.toml` (with `agents.warp_agent.is_any_ai_enabled` and saved-agent entries) loads without crashing — values are ignored, sibling keys (`theme`, `font`, `keybindings`) are applied normally. If the relevant settings struct uses `#[serde(deny_unknown_fields)]`, switch it off for the top-level container so legacy keys are tolerated.

3. **Feature flags.** Use `remove-feature-flag` for any `FeatureFlag::*` whose only callers were AI. Likely candidates surface during impl — `git grep` each flag's call sites before removing. Mixed flags are not removed.

4. **Documentation.** Remove AI sections from `WARP.md`. De-AI any `docs/` content. READMEs under deleted module trees went with their modules in 2c; check for lingering top-level mentions in `app/src/lib.rs` doc-comments, root-level READMEs, etc.

5. **Run `simplify` once more** for a final pass.

PR title: `[twarp 02d] ai-removal: final sweep (telemetry, settings, flags, docs)`.

## Testing and validation

| PRODUCT § | Verification |
|-----------|--------------|
| Goals (no AI UI after 2b) | PRODUCT §2b smoke 1-5. Plus an integration test (or unit test, depending on what the onboarding crate already covers) asserting the agent slide is not in the slide list returned by the onboarding flow constructor. |
| Goals (AI modules deleted after 2c) | PRODUCT §2c smoke 1-3. Optional: a `presubmit` step that fails if `app/src/ai/` exists. |
| Goals (telemetry/settings cleanup after 2d) | PRODUCT §2d smoke 1-4. |
| Non-goal (no rebrand) | Spot-check: `git diff master..HEAD -- app/ crates/ \| grep -i "no.ai\|ai.free\|twarp"` returns nothing for sub-PRs 2b-2d. |
| Non-goal (cloud/Drive intact) | PRODUCT §2b smoke step 7 + a manual check that opening Warp Drive (if available) and account login still work after each phase. |
| Migration (existing user upgrading) | PRODUCT §2b smoke step 7. Plus a unit test in 2d that loads a legacy upstream `settings.toml` fixture (with `agents.warp_agent.is_any_ai_enabled = true`, saved agents, blocklist) and asserts startup succeeds with non-AI settings preserved. |
| Telemetry-event reachability after 2b | Optional: launch twarp under `RUST_LOG=trace` and run the smoke flow; assert no `WarpAIAction` / `AIBlocklist` events fire. The 2d cleanup makes this structural anyway. |

`./script/presubmit` must be green at the end of every sub-phase PR.

## Risks and mitigations

- **Risk: a non-AI feature secretly imports from `app/src/ai/` or `crates/ai/`.** Mitigation: 2c's iterative deletion catches this — `cargo check` fails, the import is moved out (the dependency is non-AI by definition, so it has somewhere to go), the deletion proceeds. This is the main reason 2c is iterative rather than one big delete commit.
- **Risk: upstream restructures the kill-switch between 2a and 2b.** Mitigation: 2a's GATE.md is dated; if a cherry-pick lands that touches `is_any_ai_enabled` between 2a and 2b, re-verify the gate before starting 2b. The recurring upstream-watcher (set up after 2a) surfaces this.
- **Risk: legacy user settings files containing AI keys crash twarp.** Mitigation: 2d's deserialize-tolerance test, plus PRODUCT §2b smoke step 7 (manual check before 2d ships). 2b's reader change tolerates the keys at runtime; 2d's schema change tolerates them at parse time.
- **Risk: 2c's massive deletion overwhelms review.** Mitigation: 3-5 incremental commits, `simplify` between, presubmit on the last commit. Each commit must individually compile. Single PR, multi-commit — not multiple PRs, because the deletion is one logical unit.
- **Risk: a deleted feature flag is also referenced by an upstream commit twarp wants to cherry-pick later.** Mitigation: future cherry-pick will conflict and the picker can decide; the alternative (keeping dead flags forever) is worse. Document removed flags in the PR description so the cherry-pick workflow has a search target.
- **Risk: hidden coupling between AI telemetry and non-AI telemetry (shared base type, shared serializer).** Mitigation: 2d audits each AI-only variant. If a variant is part of a shared enum used elsewhere, only its arm is removed; if the whole enum is AI-only, the enum goes too. Decide per-variant during impl.
- **Risk: `disable_oz` is wired into the onboarding model's intent enum in a way that makes the slide hard to remove cleanly.** Mitigation: 2a's GATE.md explicitly maps `disable_oz` and the intent picker's structure; if the coupling is too tight, 2b can leave `disable_oz` in the model unused (Rust dead-code-warnings are fine; cleanup follows in 2c) and the slide deletion proceeds.

## Follow-ups

- **Recurring upstream-watcher** — `/schedule`-driven agent that fetches `upstream/master` weekly and surfaces commits touching gated AI files. Set up after 2a; out of scope for this feature but should be running by the time 2c starts.
- **Brand surface trim** — feature 05 (rebrand) renames "Warp" → "twarp" across remaining surfaces; some touchpoints overlap with deleted AI modules, so 05 lands easier post-02. Not blocking.
- **License audit** — if 2a's GATE.md surfaces ambiguity at the MIT/AGPL boundary, file a separate follow-up before 2c. Not blocking 2b.
- **Empty-state polish** — once AI is gone, surfaces that previously co-housed AI and non-AI items (e.g. the command palette, the input area) may look slightly hollow. Minor visual polish is a follow-up, not part of 02.
