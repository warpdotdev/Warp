# Git Worktrees Context Chip — Tech Spec

## Context

A new `ContextChipKind::GitWorktrees` chip in `app/src/context_chips/`. Lists the current repo's worktrees in the prompt, opens them in tabs, and provides creation / removal flows via a new modal and a confirmation dialog respectively. The chip's data comes from a single shell command (`git worktree list --porcelain` + a per-worktree reflog suffix), so the value layer follows the existing pattern used by `ShellGitBranch` and `GitDiffStats` — no new model or filesystem watcher in this PR.

See `PRODUCT.md` in this directory for behavior, edge cases, and validation steps.

### Relevant code

**Chip itself (new):**
- `app/src/context_chips/mod.rs` — `ContextChipKind::GitWorktrees` variant + wiring in `to_chip()`, `placeholder_value`, `default_styles`, `udi_icon`, and `available_chips()` (gated by `FeatureFlag::GitWorktreesChip`).
- `app/src/context_chips/builtins.rs` — `shell_git_worktree_list()` shell generator; emits porcelain output followed by `---ORIGIN---` and per-worktree `<path>|<branch>|<origin>` lines parsed from each branch's reflog.
- `app/src/context_chips/worktree.rs` (new) — `Worktree` struct + `parse_porcelain_list()` parser (with origin-section support) + `current_worktree()` helper + 9 unit tests.
- `app/src/context_chips/display_chip.rs` — `DisplayChipKind::GitWorktrees` variant, the `git_worktrees_chip()` render method, the `WorktreeMenuItem` and `CreateWorktreeFooterItem` `GenericMenuItem` impls (with `custom_body` for the 3-line layout), the static `WORKTREE_PENDING_UNTIL_MS` + `mark_worktree_chip_pending()` API, and the menu-event subscriber that bubbles open / remove / create requests.
- `app/src/context_chips/display_menu.rs` — `ChipMenuType::Worktrees` variant, `DisplayChipMenuAction::TrailingActionInvoked { action_data }`, `PromptDisplayMenuEvent::TrailingActionInvoked`, and the `GenericMenuItem::custom_body()` trait extension that lets an item replace the default `[icon | name | right_side_element]` layout.
- `app/src/context_chips/display.rs` — `PromptDisplayEvent::OpenWorktreeInNewTab(PathBuf)` + `RequestRemoveWorktree(PathBuf)` variants and their relays.
- `crates/warp_features/src/lib.rs` — `FeatureFlag::GitWorktreesChip` + `FeatureFlag::GitWorktreesChipCreate`. Both added to `DOGFOOD_FLAGS` for internal builds.
- `app/assets/bundled/svg/worktree-icon.svg` (new) + `crates/warp_core/src/ui/icons.rs` — `Icon::GitWorktree` registered.

**Open-in-new-tab plumbing (mostly reused):**
- `terminal::input::Event::OpenDirectoryInNewTab { path }` + `RequestRemoveWorktree { path }` (and their relays through `terminal::view::Event` and `pane_group::Event`).
- Workspace handler: `pane_group::Event::OpenDirectoryInNewTab` → existing `Workspace::open_directory_in_new_tab` (no new code in workspace for the open flow).

**Remove flow:**
- `app/src/workspace/remove_worktree_confirmation_dialog.rs` (new) — view + `Source` + event/action enums, mirroring the structure of `delete_conversation_confirmation_dialog.rs`.
- `app/src/workspace/view.rs::handle_remove_worktree_request` — async git status + unpushed check, then opens the dialog with the populated source.
- `app/src/workspace/view.rs::execute_remove_worktree` — closes affected tabs synchronously (snappy), then spawns `git worktree remove [--force] <path>`. `--force` only when dirty (the user already saw and confirmed the warning).

**Create flow:**
- `app/src/tab_configs/create_worktree_modal.rs` (new) — `CreateWorktreeModal` view, `CreateWorktreeModalSeed`, `CreateWorktreeModalEvent`, `CreateWorktreeModalAction`. Built per the open/closed principle: the existing `NewWorktreeModal` (APP-3679) stays untouched.
- `WorkspaceAction::OpenCreateWorktreeModalFromChip { porcelain_output, current_worktree_path }` (in `workspace/action.rs`) — the chip dispatches this, the workspace builds a seed and opens the modal. Action carries the porcelain text so the workspace doesn't have to re-shell.
- `app/src/workspace/view.rs::execute_create_worktree` — runs `git worktree add -b <branch_name> <destination> <source_branch>` and opens the result in a new tab. `-b` is critical so the source branch (often the user's current branch) isn't double-checked-out.

## Proposed changes

### 1. Chip foundation

- New `ContextChipKind::GitWorktrees` enum variant + 6 match-arm updates in `mod.rs` (`to_chip`, `placeholder_value`, `default_styles`, `udi_icon`, etc.).
- Behind `FeatureFlag::GitWorktreesChip` for staged rollout. Footer item gated by sibling `GitWorktreesChipCreate` flag.
- New `WORKTREES_REFRESH_CONFIG` (5s polling, vs. the 30s default `GIT_REFRESH_CONFIG` used by branch / diff-stats). Tighter so the count reflects worktree changes within ~5s.
- New `worktree.rs` module with the porcelain parser. The parser also handles the optional `---ORIGIN---` section emitted by our extended shell command, attaching `origin_branch` to each `Worktree`.

### 2. Shell command (extended porcelain)

`shell_git_worktree_list()` runs:

```bash
git worktree list --porcelain && \
  echo '---ORIGIN---' && \
  git worktree list --porcelain | awk '/^worktree/{print $2}' | \
  while read wt; do
    br=$(git -C "$wt" rev-parse --abbrev-ref HEAD)
    if [ -n "$br" ] && [ "$br" != "HEAD" ]; then
      origin=$(git -C "$wt" reflog show "$br" --format=%gs | \
        awk -F'Created from ' '/Created from /{print $2; exit}')
      printf '%s|%s|%s\n' "$wt" "$br" "$origin"
    fi
  done
```

Cost is N+1 git invocations per refresh (5s). Negligible for the typical N (2-5 worktrees). PowerShell falls back to plain porcelain (origin display is best-effort, missing on Windows for this PR).

### 3. Chip rendering (multi-line menu items)

`DisplayChipKind::GitWorktrees { menu_open, menu, worktrees, current_index }` is the new variant. The render path goes through `git_worktrees_chip()` for the chip itself and `WorktreeMenuItem::custom_body()` for the menu items.

`custom_body` was added as a new method on the `GenericMenuItem` trait (default `None`). When present, the menu's `UniformList` renderer uses it instead of the default `[icon | name | right_side_element]` layout. This was the cleanest way to support the chip's 3-line item layout (name + path + branch→origin) without disturbing the other menu types (Branches, Directories, Environments, CodeReview).

`WorktreeMenuItem::custom_body` always renders 3 lines (with an empty spacer line when origin is unknown) so every row in the `UniformList` has the same height — `UniformList` measures the first item's layout once and reuses that height for all rows; mismatched rows clip into each other otherwise.

The pending indicator (`⟳` in place of the count for ~6s after a remove or create) is driven by a static `AtomicU64` (`WORKTREE_PENDING_UNTIL_MS`) that any handler can flip via `mark_worktree_chip_pending()`. The chip's `TrailingActionInvoked` subscriber spawns a `Timer` (`schedule_pending_clear_render`) that triggers `ctx.notify()` ~100ms after the deadline so the chip re-renders without waiting for a hover / next refresh.

### 4. Remove flow

- `RemoveWorktreeConfirmationDialog` view file mirrors `delete_conversation_confirmation_dialog.rs`. Source is `RemoveWorktreeDialogSource { path, worktree_name, tabs_to_close, dirty_status }`.
- `Workspace::handle_remove_worktree_request` runs `git status --porcelain` + `git rev-list --count @{u}..HEAD` async, builds the source, and opens the dialog.
- `WorktreeDirtyStatus { has_uncommitted_changes, has_untracked_files, has_unpushed_commits }` summarizes the warning section.
- `Workspace::execute_remove_worktree` snapshots the affected tab indices via `tabs_under_worktree_path()` (which iterates `self.tabs` and calls `pane_group.terminal_view_working_directories(ctx)`), closes them synchronously, then spawns `git worktree remove [--force] <path>` in the background. Tab closure is intentionally before the git command so the UI feels snappy — if git fails, a toast surfaces; tabs stay closed because the user already confirmed the destructive intent.
- Friendly error messages translate common git failures (`invalid reference`, `is already used by`, `already exists`, branch-name conflicts) into actionable copy.

### 5. Create flow (new modal, open/closed)

- `CreateWorktreeModal` is a brand-new view — does NOT modify `NewWorktreeModal` from APP-3679.
- Inputs: `BranchPicker` (source branch), destination editor, name editor, branch-name editor, footer Cancel / Create.
- The branch-name editor auto-mirrors the worktree-name editor char-by-char; the mirror stops the moment the user manually edits the branch name (tracked via `branch_name_overridden` + comparing the editor buffer against `last_programmatic_branch_value`).
- The Create button is rendered subdued with no `on_click` when the form is invalid (empty name or invalid characters).
- A live preview of the final destination path renders below the inputs.
- Submit emits `CreateWorktreeModalEvent::Submit { source_worktree, branch, destination, worktree_name }`. The workspace runs `git worktree add -b <worktree_name> <destination> <branch>` from `source_worktree` (any worktree of the repo; git resolves up to the repo root). On success, `open_directory_in_new_tab(destination)`.

### 6. Action / event chain

The chip can't directly call workspace methods, so events bubble through the existing tree:

- **Open in new tab**: `PromptDisplayChipEvent::OpenWorktreeInNewTab(path)` → `PromptDisplayEvent::OpenWorktreeInNewTab` → `terminal::input::Event::OpenDirectoryInNewTab` → `terminal::view::Event::OpenDirectoryInNewTab` → `pane_group::Event::OpenDirectoryInNewTab` → workspace handler.
- **Remove**: identical chain with the `RequestRemoveWorktree` variants on each layer.
- **Create**: chip dispatches `WorkspaceAction::OpenCreateWorktreeModalFromChip { porcelain_output, current_worktree_path }` directly (typed actions bubble up the view tree until the workspace handles them — no per-layer relay needed).

## Testing and validation

**Unit tests** (in `worktree.rs`):
- `parses_single_worktree`, `parses_multiple_worktrees`, `parses_detached_head_entry`, `parses_bare_entry`, `empty_input_yields_empty_vec`, `name_falls_back_to_full_path_when_no_basename`, `current_worktree_matches_exact_path`.
- `parses_origin_section_into_matching_worktrees` — origin attaches when `<path>` matches; empty origin leaves `origin_branch = None`.
- `missing_origin_section_is_backward_compatible` — porcelain-only input still parses.
- `origin_section_ignores_unknown_paths` — origin entry pointing at a path not in the porcelain list is silently dropped.
- `origin_section_ignores_malformed_lines` — lines without enough `|`-separated fields are skipped.

**Manual validation** (covered in `PRODUCT.md` § Validation):
- Chip render + count, menu layout (green bar / root badge / trash), open-in-new-tab.
- Remove: confirm dialog content, dirty warning, tab cleanup, friendly error toasts.
- Create: branch-name auto-mirror + override behavior, validation message + disabled button on empty name, friendly errors, new tab opens at the created worktree.
- Pending `⟳` indicator clears automatically (no hover required) within ~6s.

**Build**:
- `cargo check -p warp` — clean (no warnings related to this work).
- `cargo nextest run -p warp context_chips::worktree` — 11 tests pass.

## Risks and mitigations

- **Polling cost**: 5s polling is N+1 git invocations per cycle. Acceptable for typical N (2-5). Watcher integration is deferred — see follow-ups.
- **`UniformList` row sizing**: the menu's `UniformList` measures the first item's height and reuses it. Mismatched item heights clip — mitigated by always rendering 3 lines (with an empty spacer when origin is unknown).
- **Tab cleanup false positives**: a tab whose terminal CWD is under the removed worktree path closes. Mitigated by canonicalizing both sides; tabs the user `cd`-ed out of stay open. Path-prefix match uses `starts_with` on canonical paths.
- **Snappy tab close vs. git failure**: tabs close before the git command runs. If git fails the worktree stays but tabs are gone. Mitigated by the upfront confirmation dialog (the user already accepted the destructive intent) and a persistent error toast.
- **Branch-name auto-mirror edge cases**: typing identical text into the branch field as our auto-fill is indistinguishable from "user accepted the auto-fill" — fine, mirror keeps working. Pasting an identical value via clipboard would also leave it in mirror mode; acceptable.
- **Reflog dependency for origin**: branches whose reflog has been pruned (default 90d for unreachable, configurable) lose their origin display. We render an empty spacer; not surfaced as an error.

## Follow-ups

- **Watcher integration (Fase 1.1)**: extend `GitRepoStatusModel` (`app/src/code_review/git_status_update.rs`) to detect `.git/worktrees/` directory add/delete and emit a new `GitRepoStatusEvent::WorktreesChanged`. Replace polling with subscription. Eliminates the up-to-5s lag on count updates.
- **Designer-provided icon**: current `worktree-icon.svg` is a placeholder folder + branch glyph. Designer review pending.
- **Working-directory chip compaction in worktrees**: when the user is inside a known worktree, the working-dir chip duplicates the worktree name (last segment). Setting / heuristic to compact this — deferred until a user complains.
- **Hide chip when ≤1 worktree**: `should_render` doesn't have access to the chip's parsed value. Solved by either inverting the check into the chip-display layer or threading porcelain through `should_render`. Deferred.
- **Tag tabs as "worktree-owned"**: would replace the path-prefix match in the remove flow with a deterministic tab→worktree association. Cleaner but adds metadata to `TabData`.
- **`git worktree prune` UI**: stale registrations whose paths no longer exist could surface as a maintenance action.
- **Per-worktree dirty indicator in the menu**: `git status` per worktree per refresh; design carefully (parallelize, cache, throttle) before shipping.
- **Branch prefix radios in create modal**: `feature/`, `fix/`, `chore/`, etc. as quick-pick prefixes that auto-fill into the branch name. Defer until the modal sees real usage.
- **Resolve origin = "HEAD"**: when worktrees are created without an explicit base branch, the reflog literally records "Created from HEAD". Possible heuristic resolution (find the closest branch containing the commit) could replace the literal display.
