# Git Worktrees Context Chip — Spec

## Scope

This spec covers the full feature in three sequenced phases. Each phase ships independently behind the same feature flag.

- **Fase 1** — Read-only chip: lists existing worktrees and opens a selected worktree in a new tab.
- **Fase 2** — Creation modal: extends `NewWorktreeModal` with a base-ref picker (Current HEAD or Pick a base → local/remote branches) and a "Fetch from remote first" checkbox. Worktree name is required (no autogeneration).
- **Fase 3** — Removal flow: "Remove worktree" menu item with confirmation; after `git worktree remove`, automatically closes any tab whose CWD is under the removed worktree's path.

Phases 2 and 3 share infrastructure with Fase 1 (the chip is the only entry point for both creation and removal actions).

## Context

Warp's prompt input renders a row of "context chips" above the editor (working directory, git branch, git diff stats, etc.). The chip system is plug-in: each chip is a `ContextChipKind` enum variant that wires a value generator (typically a shell command) to a renderer (`DisplayChip`) and an optional click menu.

This feature adds a `GitWorktrees` chip that, across the three phases, becomes the central UI surface for inspecting, creating, and removing git worktrees in the active repo. Worktrees follow Warp's existing convention of living under `~/.warp/worktrees/{repo_name}/{worktree_name}` (established by APP-3679), so the chip operates inside a known path namespace rather than discovering arbitrary worktree locations.

### Relevant code

**Chip system (Fase 1):**
- `app/src/context_chips/mod.rs:159` — `ContextChipKind` enum (19 variants today). Add `GitWorktrees` here, then wire it in `to_chip()` at line 189.
- `app/src/context_chips/builtins.rs:149` — `shell_other_git_branches()` is the closest template for a shell-backed list generator. Add `shell_git_worktree_list()` next to it (only used as fallback; primary data source is the watcher — see below).
- `app/src/context_chips/display_chip.rs` — `git_branch_chip()` (~line 995), `git_diff_stats_chip()` (~line 1113), `working_directory_chip()` (~line 1213) are the structural templates for the new `git_worktrees_chip()`.
- `app/src/context_chips/display_menu.rs:89` — `ChipMenuType` enum (`Directories`, `Branches`, `CodeReview`, `Environments`). Add `Worktrees`. The `Branches` variant is the closest behavioral analogue.
- `app/src/context_chips/display.rs:403` — `PromptDisplay::render()` already iterates `display_chips` and respects `should_render()`; no changes needed here.
- `app/src/workspace/view.rs:7266` — `open_directory_in_new_tab(path: PathBuf)` already exists and creates a single-terminal pane layout with `initial_directory` set.

**Worktree convention and creation modal (Fase 2):**
- `app/src/tab_configs/tab_config.rs:42-59` — `generated_worktree_repo_dir(repo_path)` and `generated_worktree_path(repo_path, worktree_name)` define the hard-coded path convention `~/.warp/worktrees/{repo}/{name}`. No user override.
- `app/src/tab_configs/new_worktree_modal.rs:97-290` — Existing `NewWorktreeModal` from APP-3679. Inputs today: `RepoPicker`, `BranchPicker`, autogenerate-name checkbox + manual name field. Submits `NewWorktreeModalEvent::Submit { repo, branch, autogenerate_name }`.
- `app/src/tab_configs/branch_picker.rs:51-233` — `BranchPicker`. `refetch_branches(cwd)` already supports cross-repo. **Limitation**: only lists local branches (uses `git branch` / `git for-each-ref refs/heads`). Fase 2 must extend with a "Remote" tab that runs `git for-each-ref refs/remotes/`.
- `app/src/tab_configs/repo_picker.rs` — `RepoPicker` (used by the modal; no changes required).
- `app/src/workspace/view.rs:9176-9258` — `handle_new_worktree_submit()` writes the TOML and opens the resulting tab config. Fase 2 modifies this to optionally run `git fetch` first.

**Filesystem watcher (Fase 1):**
- `app/src/code_review/git_status_update.rs:147-212` — `GitRepoStatusModel::new`, `should_refresh_metadata()` at lines 259-277. Pull-based with `MetadataChanged` event subscription; refcounted singleton cache.
- `crates/repo_metadata/src/watcher.rs:143-155` — Tier 1 routing already sends `.git/worktrees/<name>/` events to the matching repo's `external_git_directory`.
- `crates/repo_metadata/src/watcher.rs:431-451` — `should_ignore_git_path` filter; currently allows only HEAD, `refs/heads/*`, and `index.lock`. Fase 1 extends this to also pass directory-add/delete events for `.git/worktrees/` itself.
- `crates/repo_metadata/src/repository.rs:62` — `Repository` struct already exposes `external_git_dir` and `shared_git_root`.

**Tab close + tab→cwd mapping (Fase 3):**
- `app/src/workspace/view.rs:10347-10446` — `close_tab(index, skip_confirmation, add_to_undo_stack)`.
- `app/src/pane_group/mod.rs:6532-6541` — `terminal_view_working_directories(ctx) -> Vec<(EntityId, Option<String>)>`. Iterate `self.tabs`, call this on each `PaneGroup` to find tabs whose CWD is under a given path.

**Cross-reference:** `specs/APP-3679/TECH.md` — Existing worktree-creation work. Fase 2 of this spec extends APP-3679's modal directly (does not create a parallel creation path).

## Proposed changes

### Fase 1 — Listing chip + open in new tab

**1. New `ContextChipKind::GitWorktrees` variant** in `app/src/context_chips/mod.rs`:
- Variant added to the enum and wired in `to_chip()` as a `shell_builtin` with `GIT_REFRESH_CONFIG`. Behind `FeatureFlag::GitWorktreesChip` for staged rollout (mirrors `GithubPullRequest` at `mod.rs:303`).

**2. Refresh strategy — shell-on-refresh for now, watcher integration deferred to Fase 1.1**:
- Phase 1 ships with the same `GIT_REFRESH_CONFIG` (30s periodic refresh) used by `git_branch_chip` and `git_diff_stats_chip`. This matches the existing pattern and avoids touching shared infrastructure in `crates/repo_metadata/`.
- The watcher integration (Option A: extend `RepositoryUpdate` with `worktrees_changed`, emit `GitRepoStatusEvent::WorktreesChanged` from `git_status_update.rs`) is now a Fase 1.1 follow-up, scoped as a perf/responsiveness improvement. Splitting it from this PR keeps the surface small and avoids regression risk on the existing chips that already depend on `GitRepoStatusModel`.

**3. Shell generator** `shell_git_worktree_list()` in `app/src/context_chips/builtins.rs`: runs `GIT_OPTIONAL_LOCKS=0 git worktree list --porcelain` (per-shell mapping mirroring `shell_other_git_branches()`).

**4. Porcelain parser** in new module `app/src/context_chips/worktree.rs`:
- Pure function `parse_porcelain_list(input: &str) -> Vec<Worktree>`.
- `Worktree { path: PathBuf, branch: Option<String>, head: Option<String>, is_detached: bool, is_bare: bool }` plus `name()` helper (basename, falls back to full path).
- Helper `current_worktree(&[Worktree], cwd: &Path) -> Option<&Worktree>` — best-effort match by canonicalized path equality or prefix.
- 7 unit tests covering: single, multiple, detached HEAD, bare repo, empty input, name fallback, current detection.

**5. Chip rendering** `git_worktrees_chip()` in `app/src/context_chips/display_chip.rs`:
- **Icon**: `Icon::GitBranch` placeholder (designer to provide a dedicated worktrees icon). The label `"wt"` was explicitly rejected.
- **Label**: basename of the *current* worktree's path (matched via `worktree::current_worktree(&worktrees, current_repo_path)`). Falls back to `"{count} worktrees"` if the current path can't be matched.
- **`should_render()`**: existing `_ => true` arm in `ContextChipKind::should_render` is reused. The chip naturally hides outside a git repo because the shell command produces no output. Hiding when there's only 1 worktree is a follow-up (currently shows a single-item menu) — small UX miss tracked in Open questions.
- **Click handler**: dispatches `DisplayChipAction::OpenWorktreesSelector`, which delegates to `ToggleMenu` and routes per-kind through `handle_action`.

**6. Menu** `ChipMenuType::Worktrees` in `app/src/context_chips/display_menu.rs`:
- New variant `Worktrees` added to `ChipMenuType`. All grouped match arms that contained `Branches` were extended to also include `Worktrees` (visual padding, search-input config, scroll behavior, drop shadow). Search placeholder: `"Search worktrees..."`.
- `WorktreeMenuItem { display_name, path, is_current }` in `display_chip.rs` implements `GenericMenuItem`. The current worktree is non-clickable: the menu subscriber checks `is_current` on the action item and short-circuits with a menu-close instead of opening a tab.
- (Naming note: the public type used as a menu item is `WorktreeMenuItem`, not `Worktree` — the `Worktree` name is owned by the parser module to keep porcelain semantics distinct from UI rendering.)
- Footer item: `CreateWorktreeFooterItem` ("Create new worktree…", `Icon::Plus`). Wired only when `FeatureFlag::GitWorktreesChipCreate` is enabled; until Fase 2 it's effectively hidden. Even if surfaced, the click is treated as a no-op (sentinel `__create_new_worktree__` in `action_data` is matched and ignored by the subscriber).

**7. Click action wiring (chain)**:
- `PromptDisplayChipEvent::OpenWorktreeInNewTab(PathBuf)` (`display_chip.rs`)
  → relayed by `PromptDisplay::reset_chips` subscriber as `PromptDisplayEvent::OpenWorktreeInNewTab(PathBuf)` (`display.rs`)
  → handled in `terminal/input.rs::handle_prompt_event`, emits `Event::OpenDirectoryInNewTab { path }`
  → `pane_group::pane::terminal_pane.rs` relays to `pane_group::Event::OpenDirectoryInNewTab { path }` (already plumbed for the file tree at `left_panel.rs:776`)
  → workspace handler at `view.rs:13257` already calls `self.open_directory_in_new_tab(path, ctx)`.
- No new code in `workspace/view.rs`; this reuses the same path the file tree uses.

### Fase 2 — Creation modal

Extend `NewWorktreeModal` in `app/src/tab_configs/new_worktree_modal.rs`:

**1. Base-ref selector (replaces existing single branch picker)**:
- Radio group with two options:
  - **Current HEAD** (default — quick path, uses the active terminal's HEAD).
  - **Pick a base** — activates a tabbed picker with **Local** and **Remote** tabs.
    - **Local** tab: existing `BranchPicker` behavior (lists `refs/heads/*`).
    - **Remote** tab: extend `BranchPicker` to support `git for-each-ref refs/remotes/`. Items render as `origin/main`, `origin/develop`, etc.
- The dropdown subsumes the original "main root" radio idea — `main` is just one entry in the local list, `origin/main` is one entry in the remote list.

**2. "Fetch from remote first" checkbox**:
- Default ON when the selected base is a remote ref.
- Default OFF when base is Current HEAD or a local branch.
- When checked: runs `git fetch origin <ref-name>` before the worktree creation. Surfaces fetch errors inline and does not proceed.
- Auto-pull is explicitly rejected (mutates local branches without consent, can produce merge conflicts). Auto-fetch is safe (only updates remote-tracking refs).

**3. Worktree name**:
- Field is **required** (autogenerate checkbox removed).
- Open button disabled until non-empty.

**4. Submit handler** (extend `handle_new_worktree_submit` in `workspace/view.rs:9176`):
- Resolve target path via existing `generated_worktree_path(repo, name)` from `tab_config.rs:42`.
- Run `git fetch` if checkbox checked.
- Run `git worktree add <target_path> <ref>` (no `--force`; let it fail loudly if the branch is already checked out elsewhere).
- Open the new worktree in a new tab via `open_directory_in_new_tab(target_path)` (the same path Fase 1 uses — keeps behavior consistent).

**5. Trigger from chip**: the menu's "Create new worktree…" footer item dispatches an action that opens the modal pre-populated with the current repo.

### Fase 3 — Remove worktree + close associated tabs

**1. Menu item per worktree**: each `Worktree` item in the menu (except the current one) gets a "Remove worktree" affordance (right-side action button or context-menu).

**2. Confirmation dialog** (reuse existing confirm modal pattern):
- Lists the path being removed.
- Lists tabs that will close (queried via `terminal_view_working_directories(ctx)` filtered by path prefix).
- Calls `git status --porcelain` on the worktree path; if dirty, surfaces a warning ("3 unstaged changes will be lost") and requires a second click.

**3. Removal action**:
- Run `git worktree remove <path>`. No `--force` by default (let git reject removal of dirty/locked worktrees).
- If the user opted into "force remove" via the dirty-warning second-click, run with `--force`.

**4. Tab cleanup**:
- After successful removal, iterate `self.tabs` in workspace; for each `PaneGroup`, call `terminal_view_working_directories(ctx)`.
- Any tab whose CWD starts with the removed path: call `close_tab(tab_index, true, true)` (skip confirmation since user already confirmed at the worktree level; add to undo stack so the user can recover if a tab matched unexpectedly).
- Tabs whose terminals have `cd`-ed outside the worktree path do **not** close — this is correct behavior.

## Open questions

- **Icon choice**: Designer to provide. Placeholder during dev: `Icon::GitBranch` (same as branch chip).
- **Hide chip when ≤1 worktree**: Spec called for hiding; implementation currently uses the existing `should_render` default arm (always render when value present). The chip therefore appears even with one worktree, showing a single-item menu. Tracked as a follow-up — fixing requires either parsing `value` inside `should_render` (awkward, no chip-level access to porcelain) or filtering at the chip-display level.
- **`git worktree remove` on dirty worktree** (Fase 3): Three plausible UX paths — (a) bare warning + force opt-in via second click [recommended], (b) block removal entirely, (c) auto-stash before removing. Decide before Fase 3 review.
- **Confirm dialog presentation** (Fase 3): How exactly to render the "N tabs will close" list — full paths, basenames, terminal numbers, or all three? Designer call.

## Testing and validation

**Fase 1**:
- *Unit (shipped)*: 7 tests in `app/src/context_chips/worktree.rs` covering parser (single, multiple, detached HEAD, bare repo, empty input), name fallback, current-worktree match.
- *Unit (follow-up)*: Chip label derivation (current matched / not matched / empty) — currently inlined in chip construction; extract for direct testing.
- *Unit (Fase 1.1)*: Watcher emits `WorktreesChanged` on `.git/worktrees/<name>/` dir add/delete; doesn't emit on file modifications within.
- *Manual*: Repo with 3 worktrees → click chip → list shows 3 entries with current one identifiable → select non-current → new tab opens at correct path.
- *Manual*: Non-git directory → chip absent (shell command returns nothing).
- *Manual*: Toggle `FeatureFlag::GitWorktreesChip` off → chip gone.
- *Manual*: Repo with 1 worktree → chip currently shows (see Open questions about hiding).

**Fase 2**:
- *Manual*: Create from Current HEAD → new worktree at `~/.warp/worktrees/{repo}/{name}` checked out at HEAD's commit.
- *Manual*: Create from local branch (e.g. `feature/x`) → new worktree at that branch's commit; original worktree's local branches untouched.
- *Manual*: Create from `origin/main` with fetch ON → fetch runs, worktree created at fresh `origin/main` commit.
- *Manual*: Create from `origin/main` with fetch OFF → worktree created at locally-cached `origin/main` ref (may be stale).
- *Manual*: Empty name → Open button disabled.
- *Manual*: Branch already checked out elsewhere → `git worktree add` fails, error surfaced in modal.

**Fase 3**:
- *Manual*: Create 2 worktrees, open tabs in both, remove one → only the removed worktree's tabs close.
- *Manual*: Tab where user `cd`-ed outside the worktree → does not close (verify by `pwd`).
- *Manual*: Remove dirty worktree → warning shown, second click with force succeeds.
- *Manual*: Remove dirty worktree without force → `git worktree remove` fails, no tabs closed.

**Build**:
- `cargo check -p warp` clean across all phases.
- `cargo nextest run -p warp context_chips` passes.
- `cargo nextest run -p warp tab_configs` passes (Fase 2 touches `new_worktree_modal`).

## Risks and mitigations

- **Watcher extension churn (Fase 1.1)**: changing `RepositoryUpdate` and `should_ignore_git_path` will affect every consumer of `GitRepoStatusModel` when implemented. Mitigation: a new flag defaulting to `false` so existing consumers ignore it. Regression test: `git_branch_chip` and `git_diff_stats_chip` continue to refresh on HEAD/index changes only.
- **Shell-on-refresh polling cost (Fase 1)**: `git worktree list --porcelain` runs every 30s while the chip is enabled. Cheap on small repos, measurable on monorepos with 50+ worktrees. Mitigation: behind feature flag during dogfooding; benchmark before stable rollout. Watcher integration (Fase 1.1) eliminates polling.
- **`git worktree remove` on dirty worktree**: user could lose unstaged changes. Mitigation: confirm dialog runs `git status --porcelain` first, lists unstaged files, requires explicit force opt-in.
- **Path prefix false positives in tab close**: a worktree at `~/.warp/worktrees/repo/foo` could match `~/.warp/worktrees/repo/foo-bar` if compared as raw string prefix. Mitigation: compare with trailing separator (`path.starts_with(&format!("{}/", removed_path))`) or canonicalize and use `PathBuf::starts_with`.
- **Fetch race on modal close**: user dismisses the modal mid-fetch. Mitigation: cancel the spawned fetch task on dismiss; if fetch already completed, the worktree is not created.
- **Branch picker remote tab cost**: `git for-each-ref refs/remotes/` on a large monorepo can return thousands of entries. Mitigation: limit to top N most-recently-updated and add a search input (already present in `BranchPicker`).
- **Conflict with APP-3679 spec**: Fase 2 directly modifies the modal that APP-3679 introduced. Mitigation: spec changes land in the same PR as the code; APP-3679's TECH.md gets a forward-pointer to this spec.

## Follow-ups

- **Fase 1.1 — Watcher integration**: extend `GitRepoStatusModel` with `WorktreesChanged` event so the chip refreshes on `.git/worktrees/` dir add/delete instead of polling every 30s.
- **Hide chip when ≤1 worktree**: needs porcelain access in `should_render` (or chip-display-level filtering); currently the chip renders even for single-worktree repos.
- Tag tabs explicitly as "worktree-owned" (add metadata to `TabData`) so Fase 3 doesn't rely on CWD prefix matching.
- Support worktrees created outside the `~/.warp/worktrees/` convention (created via raw `git worktree add` from terminal). They will appear in the chip list (since `git worktree list` returns all of them) but Fase 2 always creates inside the convention.
- Per-worktree dirty indicator in the menu (e.g. `●` next to dirty worktrees). Cost: `git status --porcelain` per worktree per refresh — design carefully (parallelize, cache, throttle).
- "Reveal in Finder" / "Copy path" affordances on menu items.
- Worktree pruning UI (`git worktree prune` for stale `.git/worktrees/<name>` dirs whose path no longer exists).
