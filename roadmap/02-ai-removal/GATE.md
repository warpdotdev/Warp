---
name: 02 — AI removal — GATE
status: draft
captured-from: master @ 2026-05-04
---

# AI kill-switch — gate map

This is the deliverable for sub-phase 2a. It maps the existing "AI disabled" mechanism that twarp piggybacks on, every place the gate is read, the onboarding entry point that writes it, and the surfaces that are reachable only when the gate is on. 2b uses this map to flip the default and remove the enable path; 2c uses it as the deletion shopping list.

Captured from `master` at commit-time of this PR. If a cherry-pick lands that touches `is_any_ai_enabled`, the gate must be re-verified before 2b starts.

## 1. The kill-switch

`agents.warp_agent.is_any_ai_enabled: bool` — Warp's master AI toggle. Every AI surface upstream ships is gated either directly on this boolean or on a derived getter that ANDs it with feature-specific state.

**Declaration** — `app/src/settings/ai.rs:710-720`

```rust
define_settings_group!(AISettings, settings: [
    // If `false`, all AI features are disabled.
    is_any_ai_enabled: IsAnyAIEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::No),
        private: false,
        toml_path: "agents.warp_agent.is_any_ai_enabled",
        description: "Controls whether all AI features are enabled.",
    },
```

Settings group: `AISettings`, defined in the same file. The setting syncs globally (cloud-replicated regardless of user sync preference).

**Reader** — `app/src/settings/ai.rs:1499-1508`

```rust
pub fn is_any_ai_enabled(&self, app: &AppContext) -> bool {
    // Disable AI for anonymous and logged-out users.
    let is_anonymous_or_logged_out = AuthStateProvider::as_ref(app)
        .get()
        .is_anonymous_or_logged_out();

    *self.is_any_ai_enabled
        && !is_anonymous_or_logged_out
        && !self.is_ai_disabled_due_to_remote_session_org_policy(app)
}
```

The reader is what every call site uses; **direct field reads via `*self.is_any_ai_enabled` are rare** (one is at `app/src/settings/ai.rs:3321` inside the same file). Replacing the reader body with `false` (2b step 2) short-circuits all three conjuncts — the auth check and remote-session policy become moot.

The same file at `app/src/settings/ai.rs:1559,1595,1604,1613,1617,1621,1625,1641,1735-1802` defines derived getters (`is_active_ai_enabled`, `is_ai_autodetection_enabled`, etc.) that all AND with `is_any_ai_enabled(app)`. Forcing the master reader to `false` collapses every derived getter to `false` as well.

## 2. Every call site

`git grep -n is_any_ai_enabled` returns ~85 hits in source. Grouped:

### Settings declaration / reader / derived getters (internal to `app/src/settings/ai.rs`)

`app/src/settings/ai.rs:712,718,1499,1505,1517,1559,1595,1604,1613,1617,1621,1625,1641,1735,1743,1751,1759,1766,1773,1781,1789,1793,1798,1802` — declaration, reader body, default-session-mode fallback, and the `is_*_enabled` derived getters that all AND with the master. **None of these need touching in 2b** beyond the default flip and reader rewrite; the derived getters all collapse mechanically.

### App menus

`app/src/app_menus.rs:990,1013,1015,1020` — "New Terminal Tab" disables agent-mode trigger when AI is off; "New Agent Tab" item is disabled when AI is off. **2b deletes the `New Agent Tab` item entirely**; the `New Terminal Tab` branch keeps only the AI-disabled path.

### Root view (window/session orchestration)

`app/src/root_view.rs:2446` — passes `ai_enabled` into `LoginSlideView` constructor.
`app/src/root_view.rs:3321` — direct field read `*AISettings::as_ref(ctx).is_any_ai_enabled` (skips the auth/remote-policy ANDs); used in onboarding-flow gating.
`app/src/root_view.rs:2395` — comment-only reference (block comment around the surrounding logic).

### Settings view (the AI page)

`app/src/settings_view/ai_page.rs:453,462,467,474,481,586,631,650,682,820,1195,1226,1264,1311,1586,1621,1659,1741,1743,1837,1900,2159,3041,3048,3177,3733,3745,3753,3819,3836,3843,3875,3882,3890,3936,3941,3963,3968,4127,4293` — the AI settings page itself reads the master and individual derived flags throughout. **Whole file deleted in 2b** (along with sidebar registration in `app/src/settings_view/mod.rs`); call sites go with it.

### Search / command palette / welcome palette

- `app/src/search/command_palette/data_sources.rs:111,152` — AI commands hidden when off.
- `app/src/search/command_palette/warp_drive/data_source.rs:191` — AI-related Drive commands.
- `app/src/search/command_palette/zero_state.rs:90,121` — AI zero-state items.
- `app/src/search/command_search/view.rs:237,261,297` — AI command-search entries (one combined with `FeatureFlag::AgentMode`).
- `app/src/search/command_search/workflows/cloud_workflows_data_source.rs:38` — cloud workflows source.
- `app/src/search/command_search/zero_state.rs:295` — AI command-search zero state.
- `app/src/search/welcome_palette/view.rs:250` — welcome palette AI items.

All collapse to "off" once the reader returns `false`. Physical removal is 2c (whole subtrees of AI command palette UI live under `app/src/ai/blocklist/` and `app/src/ai/agent/`).

### Editor / code review / code editor

- `app/src/editor/view/mod.rs:8155,8161` — editor AI-suggestion gating.
- `app/src/code/footer.rs:312` — code footer AI badge.
- `app/src/code/local_code_editor.rs:2131` — local code editor AI hooks.
- `app/src/code_review/code_review_view.rs:7028` — code review AI features.
- `app/src/code_review/comment_list_view.rs:285,934` — AI-generated comment summary.

### Auth view (consumes AI status for login UI)

- `app/src/auth/auth_view_body.rs:1051,1054` — login screen tweaks copy when AI is off; comment at :1051 explicitly notes the helper accounts for login state.

### AI subsystems (gate themselves)

- `app/src/ai/agent_conversations_model.rs:1080` — top-of-conversation guard.
- `app/src/ai/conversation_details_panel.rs:566` — panel render guard.
- `app/src/ai/execution_profiles/editor/mod.rs:1035,1087,1219,1224,1230,1237` — execution-profile editor disables fields when AI is off.

These are all inside modules slated for 2c deletion; they'll vanish with their containers.

### Onboarding (writer side)

- `app/src/settings/onboarding.rs:57` — `settings.is_any_ai_enabled.set_value(is_ai_enabled, ctx)` writes the gate from the onboarding choice. **2b removes this writer.**

## 3. Onboarding entry point

The onboarding agent slide is the only UI surface that flips the gate from `false` to `true`. (The settings AI page also has a toggle at `app/src/settings_view/ai_page.rs:2159`, but that page itself is gated on the gate; cold-boot users with AI off can't reach it.)

**Slide source** — `crates/onboarding/src/slides/agent_slide.rs`
- `:111` — `pub disable_oz: bool` field on `AgentDevelopmentSettings`.
- `:123` — default `disable_oz: false` (i.e., default-on AI).
- `:167,263` — mouse handle for the slide's "disable AI" checkbox.
- `:382,397,399` — slide layout branches on `settings.disable_oz`.
- `:515` — disabled-state styling.
- `:906,927` — autonomy radio group hidden when `disable_oz == true`.
- `:942-953` — `render_disable_oz_section()` — the actual checkbox UI.
- `:1483,1508,1566,1575,1596,1605,1607` — interaction handlers for the checkbox and downstream model events.

**Slide registration** — `crates/onboarding/src/slides/mod.rs:1,17-18`
```rust
mod agent_slide;
pub use agent_slide::{
    AgentAutonomy, AgentDevelopmentSettings, AgentSlide, AgentSlideEvent, OnboardingModelInfo,
    ...
};
```
Removing the slide is `mod agent_slide;` deletion plus pruning the re-exports. Downstream importers in the workspace need to compile after the prune; check via `cargo check -p onboarding`.

**Slide → settings** — three hops:
1. User checks the "disable AI" box → `agent_slide.rs:1607` calls `state.set_disable_oz(!current, ctx)`.
2. `crates/onboarding/src/model.rs:436-447` (`set_disable_oz`) updates `agent_settings.disable_oz`.
3. On onboarding finalize, `app/src/settings/onboarding.rs:24` reads `!agent_settings.disable_oz` into `is_ai_enabled`, then `:57` writes the result to `AISettings.is_any_ai_enabled`. **This is the entire enable-path that 2b removes.**

**Selected-settings helper** — `crates/onboarding/src/model.rs:80-93` exposes `SelectedSettings::is_ai_enabled()` derived from `disable_oz` and `OpenWarpNewSettingsModes`. Several callers in the onboarding flow read this; 2b can either return `false` constant or remove the helper altogether after the agent slide is gone.

**Old vs. new onboarding modes.** Two onboarding code paths coexist behind `FeatureFlag::OpenWarpNewSettingsModes` (see `app/src/settings/onboarding.rs:36,55`, `crates/onboarding/src/model.rs:90,559`):
- Old onboarding (flag off): `Terminal` intent leaves AI on; `disable_oz` checkbox is the only off-switch.
- New onboarding (flag on): `Terminal` intent itself disables AI; `disable_oz` only matters under `AgentDrivenDevelopment`.

2b strips both paths. Whichever flag state the user is in, AI must end up off and unreachable.

## 4. Surface inventory

Maps each surface called out in PRODUCT §2b ("no AI UI in any state") to its gate site and physical location. Order matches the order surfaces are listed in PRODUCT.

| Surface | Gate site (read) | Physical module(s) | Removal phase |
|---------|-------|--------------------|---------------|
| Agent panel | `app/src/ai/conversation_details_panel.rs:566` (panel-level guard); panel init at `app/src/lib.rs:1460` (`ai_assistant::panel::init(ctx)`) | `app/src/ai_assistant/` (whole module — `panel.rs`, `transcript.rs`, `requests.rs`, `execution_context.rs`, `utils.rs`) | 2c (file delete + `mod ai_assistant;` removal at `app/src/lib.rs:113`) |
| AI command palette | `app/src/search/command_palette/data_sources.rs:111,152` | `app/src/ai/blocklist/` (much of), `app/src/ai/agent/` | 2c |
| Inline AI suggestions (predict) | `app/src/settings/ai.rs:1559,1595,1604,1613` (derived getters); call sites in editor view | `app/src/ai/predict/` (subdirs: `generate_ai_input_suggestions/`, `generate_am_query_suggestions/`, `next_command_model.rs`, `predict_am_queries/`, `prompt_suggestions/`) | 2c |
| AI settings page | `app/src/settings_view/ai_page.rs:453+` (~40 reads inside the page itself); registration in `app/src/settings_view/mod.rs:28,79,113,503,813,991,1054-1056,1162,1252,1334-1397,1779-1790,1891,2565` | `app/src/settings_view/ai_page.rs` (file) + scattered registration lines | **2b** (file delete; one of the few non-trivial 2b deletions) |
| Onboarding agent slide | (writer side, see §3) | `crates/onboarding/src/slides/agent_slide.rs` + `slides/mod.rs` re-exports | **2b** |
| Agent input footer (CLI agent toolbar) | `app/src/settings/ai.rs:1641` (`should_render_cli_agent_footer`); call sites in `app/src/code/footer.rs:312` etc. | `app/src/ai/blocklist/agent_view/agent_input_footer/` | 2c |
| Agent management popup / view | telemetry-named gate (no direct reader; lives under blocklist agent_view) | `app/src/ai/agent_management/` | 2c |
| New Agent Tab menu item | `app/src/app_menus.rs:1013,1020` | inline in `app/src/app_menus.rs` | **2b** (delete the menu item) |
| Login slide AI variant | `app/src/root_view.rs:2446` | inline in `app/src/root_view.rs` (passes `ai_enabled` to `LoginSlideView`) | **2b** (drop the param; LoginSlideView's AI-on branch becomes dead) |
| Voice input for AI | `app/src/settings_view/ai_page.rs:349` (toggle); `crates/voice_input/` | `app/src/ai/voice/`, `crates/voice_input/` | 2c (audit whether `voice_input` is AI-only or has non-AI consumers — likely AI-only) |
| Code suggestions / explain | derived getters at `app/src/settings/ai.rs:1789-1802` | scattered under `app/src/ai/`, `app/src/code_review/` AI hooks | 2c |
| Ambient agents | `app/src/ai/ambient_agents/` self-gated | `app/src/ai/ambient_agents/` | 2c |
| MCP (model context protocol) | `app/src/settings/ai.rs:1625` (`FileBasedMcp` flag + `is_any_ai_enabled`) | `app/src/ai/mcp/` | 2c |
| Skills / facts / voice / artifacts (AI subsystems) | self-gated within their modules | `app/src/ai/skills/`, `facts/`, `voice/`, `artifacts/`, `document/`, etc. | 2c |
| AI-only telemetry | n/a (variants always emittable; emission sites gated upstream) | `app/src/server/telemetry/events.rs` (variants); `crates/ai/src/telemetry.rs` (helpers) | 2d (variant prune); 2c (helpers go with crate) |

**Top-level AI containers slated for full deletion in 2c:**
- `app/src/ai/` — 30+ subdirs/files (active_agent_views_model, agent/, agent_events/, agent_management/, agent_sdk/, ambient_agents/, artifacts/, blocklist/, cloud_agent_config/, cloud_environments/, conversation_*, document/, execution_profiles/, facts/, generate_block_title/, generate_code_review_content/, get_relevant_files/, llms.rs, loading/, mcp/, onboarding.rs, outline/, predict/, request_usage_model.rs, restored_conversations.rs, skills/, voice/, ...).
- `app/src/ai_assistant/` — the agent conversation panel and its support code.
- `crates/ai/` — workspace crate.
- `crates/natural_language_detection/` — workspace crate (depended on by `app/` and `crates/input_classifier/` per `app/Cargo.toml:233`, `crates/input_classifier/Cargo.toml:44`).

**Candidate AI-adjacent crates to audit during 2c (not yet confirmed AI-only):**
- `crates/computer_use/` — name suggests agent computer-use; verify no non-AI consumers before deleting.
- `crates/voice_input/` — may be the voice subsystem the AI page toggles; verify.
- `crates/input_classifier/` — depends on `natural_language_detection`; if its only purpose was routing prompts to AI vs shell, it goes too. Otherwise it stays and just loses the NLD dependency.

These three are not on the 2c hard-delete list yet — 2c step 1 is "audit each via `git grep` for non-AI callers" before deletion, per TECH.md §2c bullet on "files that look AI-named but cross the boundary."

## 5. Licensing note

twarp inherits Warp's MIT/AGPL split:

- **Workspace default** — `Cargo.toml:27` declares `license = "AGPL-3.0-only"`. Crates inherit this via `license.workspace = true`.
- **MIT exceptions** — `crates/warpui/Cargo.toml:7` and `crates/warpui_core/Cargo.toml:7` explicitly set `license = "MIT"`.
- **AI crates** — `crates/ai/` and `crates/natural_language_detection/` both inherit the AGPL workspace default. AI code is fully AGPL.

**Boundary risk for 2c:** if any AI module under `app/src/ai/` or `crates/ai/` re-exports types that are imported by `warpui` or `warpui_core` (the MIT crates), deleting the AI module breaks the MIT boundary's compile. Mitigation: `git grep "use ai::" -- crates/warpui/ crates/warpui_core/` and `git grep "use ai_assistant::" -- crates/warpui/ crates/warpui_core/` before each top-level deletion; if hits exist, the import is moved before the source is deleted. Spot-check at gate-doc time turned up no obvious cross-references, but the audit must repeat at 2c-execute time.

**Other cross-coupling already known:** `crates/onboarding/Cargo.toml:18` depends on `ai`; `crates/onboarding/src/model.rs:6` imports `ai::LLMId`. 2c must either remove the `LLMId` use from onboarding (replacing the type or deleting the consumer) or sequence the onboarding decoupling before the `crates/ai/` delete commit. Onboarding is AGPL (workspace default), so this isn't a license-boundary issue — it's a build-order issue.

**Documentation surface:** no separate licensing follow-up is anticipated. If 2c surfaces an MIT-boundary cross-reference, file a follow-up before deletion proceeds.

## 6. Cherry-pick implications

### Files most likely to attract upstream churn (audit before 2c)

- `app/src/ai/predict/generate_ai_input_suggestions/` — frequent upstream tuning of prompt construction.
- `crates/ai/src/` — LLM client code; tracks model-API changes.
- `app/src/server/telemetry/events.rs` — every product change touches this; AI variants are inline with non-AI variants. Conflict cost stays low until 2d strips the variants.
- `app/src/settings/ai.rs` — derived getter additions; touched by any new AI feature.
- `app/src/ai_assistant/panel.rs` — agent panel UI.

Until 2c starts physically deleting these, upstream patches keep applying mechanically against the gated-off code path. **2a-2b is the cheap window.**

### Cherry-pick strategy by phase

- **Pre-2a (today through 2a merge):** business as usual. AI patches apply normally.
- **2a → 2b window:** still safe. AI files are intact; only the gate's default and reader change in 2b.
- **2b merged, 2c not started:** still safe. AI files remain intact, just unreachable. Cherry-picks of AI feature work apply to dead code — that's fine, the dead code stays dead, and a future re-enable would just work (which we don't intend, but it's a free property). 
- **2c in flight:** danger zone. Each commit deletes a subtree. Upstream commits that touched the deleted subtree will conflict; resolve by dropping the upstream side of the hunk. The recurring upstream-watcher (separate workflow, set up after 2a merges) is what surfaces these early.
- **Post-2c:** any upstream commit touching a deleted module fails to apply. The picker decides per-commit whether to skip or to resurrect the file (skip is the default).

### Re-verification trigger

If between 2a merge and 2b start the upstream-watcher flags a commit touching `is_any_ai_enabled`, `disable_oz`, the slide list, or the AISettings reader/getter graph (`app/src/settings/ai.rs:1499-1810`), this gate doc must be re-read against current master before 2b proceeds. The map's call-site line numbers will drift on every upstream merge; 2b should re-grep, not trust the line numbers above.

## Notes for sub-phase 2b

- **Default flip + force-off reader together.** Flipping the default alone leaves users with `is_any_ai_enabled = true` in their settings file with AI on; the reader change (always-`false`) is what makes the default-flip safe. Both ship in the same PR.
- **Don't strip `disable_oz` from the model in 2b.** The field stays as dead code through 2b — Rust dead-code warnings are tolerable. 2c removes the model field along with the slide source. Trying to surgically remove `disable_oz` from `crates/onboarding/src/model.rs` while the slide file still exists invites churn.
- **Settings AI page is the biggest 2b deletion.** ~4000 lines in `ai_page.rs` plus the registration touchpoints in `settings_view/mod.rs`. Sidebar entry, action enum arms, event handlers, and `SettingsPage::new(ai_page_handle)` registration must all go.
- **Login slide's `ai_enabled` param.** The slide accepts AI state to tweak copy. After 2b, the slide always sees `ai_enabled = false`; either propagate that constant or drop the param. Simpler to drop.
