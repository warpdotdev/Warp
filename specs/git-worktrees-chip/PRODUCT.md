# Git Worktrees Context Chip

## Summary

A new prompt context chip that surfaces the git worktrees of the current repo and lets the user switch between them, create a new one, or remove an existing one — all without leaving the terminal. Listing comes from `git worktree list --porcelain`, augmented with each branch's "Created from" entry from the reflog so the user sees `<branch> → <origin>` in the menu. Creating a worktree opens a dedicated modal (`CreateWorktreeModal`) that runs `git worktree add -b <new-branch> <destination> <source-branch>` and immediately opens the result in a new tab.

## Problem

Multi-worktree git workflows are increasingly common — especially with AI agents (Claude / cursor / codex) that auto-create worktrees per task. Today there's no first-class UI for this in Warp: users drop to the CLI for `git worktree list / add / remove`, and the terminal gives no indication that they're in a worktree (vs. the "main" repo). Switching between worktrees means typing the path or opening a new tab via the file tree.

## Goals

- Show the current repo's worktrees in the prompt with a single click to switch.
- Make creating a worktree a 4-field modal flow (source branch → destination → worktree name → branch name) instead of a multi-step CLI ritual.
- Make removing a worktree safe: confirm dialog that lists tabs that will close and warns about uncommitted / untracked / unpushed changes.
- Preserve the user's tabs: removing a worktree closes only the tabs whose CWD lives under the removed path.
- Visually distinguish the "root" (main) worktree so the user understands they can't remove it (it owns the `.git` directory).
- Stay out of the way for users who don't use worktrees — the chip only renders inside a git repo and is gated behind a feature flag.

## Non-goals

- Listing worktrees of repos other than the one the active terminal is in.
- Managing remote-tracking branches as worktrees (creation always uses `git worktree add -b` to create a fresh local branch).
- Per-worktree dirty indicators in the listing (would be `git status` per worktree per refresh — premature; deferred).
- Pruning stale `.git/worktrees/<name>` registrations whose paths no longer exist (`git worktree prune`).
- Replacing or duplicating the existing `NewWorktreeModal` (APP-3679), which generates a reusable `tab_config` TOML for menu-based worktree creation. The chip's modal runs `git worktree add` directly and opens the result in a new tab.

## User Experience

### The chip

- Visible only inside a git repo (the underlying shell command produces no output otherwise).
- Hidden behind `FeatureFlag::GitWorktreesChip` (and its sub-flag `GitWorktreesChipCreate` for the creation footer).
- Label format: `<current-worktree-name> · <count>`, where `<count>` is "alternative worktrees" — total minus one — so the number reflects "how many other worktrees can I jump to?". When the user is in the main worktree, count is the number of linked worktrees.
- Icon: dedicated worktree glyph (a folder with a small branch indicator), distinct from the existing branch chip's git-branch icon.
- During an in-flight remove, the count is replaced by `⟳` for ~6s as a visual hint that the displayed number is about to change. A scheduled re-render restores the real count automatically (no need for the user to hover the chip).

### The menu

Opens on click. Filterable list of all worktrees in the current repo, plus a footer item.

Each item is a 3-line block:
1. **Name** (bold). Linked worktrees are prefixed with the root repo name as `RootName_worktreename` so basenames like `89bn` or `wt1` stay disambiguated across repos.
2. **Path** (subdued). `$HOME` is replaced by `~` for compactness.
3. **Branch arrow origin** (subdued, optional). Format `<branch> → <origin>`, parsed from the branch's reflog entry "branch: Created from <ref>". When the reflog has no entry (or the worktree is the root), this line is rendered as a blank-but-same-height spacer so all rows in the menu's `UniformList` line up.

The current worktree is marked by a 3px green vertical bar to the left of the row. The root (main) worktree gets a "root" badge in the trailing slot — root is never removable. All other items render a hover-reactive trash icon in the trailing slot.

The menu's footer item is `+ Create new worktree…` (only present when `GitWorktreesChipCreate` is enabled).

### Click behavior

- Click on a non-current worktree row → opens that worktree in a new tab (existing `open_directory_in_new_tab` workspace handler).
- Click on the current worktree row → no-op (you're already there).
- Click on a trash icon → opens the remove confirmation dialog (see below).
- Click on the `+ Create new worktree…` footer → opens the creation modal.

### Create flow

The "+ Create new worktree…" footer dispatches `WorkspaceAction::OpenCreateWorktreeModalFromChip` carrying the parsed porcelain text and the current worktree path. The workspace handler builds a seed (current worktree path + default destination directory) and opens `CreateWorktreeModal`.

The modal has four inputs:
1. **Source branch** (`BranchPicker`): the branch the new worktree will start from. Defaults to whatever `BranchPicker` picks for the active terminal's CWD.
2. **Destination directory** (text input): pre-filled with `~/.warp/worktrees/<repo>/`. The final worktree path becomes `<destination-directory>/<worktree-name>`.
3. **Worktree name for feature** (text input): the folder name. Required. ASCII alphanumerics + `-` + `_` only. The Create button is disabled and a red "Worktree name is required." message appears when empty.
4. **Branch name** (text input): the new branch the worktree will check out. Auto-mirrors the worktree name char-by-char until the user manually edits it; once edited, the mirror stops so the user can keep the worktree name and branch name independent (e.g. worktree name `JIRA-1234`, branch name `feature/JIRA-1234`).

A live preview shows the final destination path under the inputs so the user can see exactly what will be created.

On Create, the workspace runs `git worktree add -b <branch_name> <destination> <source_branch>` from any worktree of the repo (the chip's current cwd). On success, the new worktree opens in a new tab. The chip is marked pending (`⟳`) so the count updates within ~5s.

Friendly error messages translate the most common git failures so the toast is actionable instead of dumping raw git output:
- "Repository has no commits on the selected branch yet. Create an initial commit before creating a worktree." (for `fatal: invalid reference`)
- "A worktree or directory already exists at that destination."
- "That branch is already checked out in another worktree." (for `is already used by` / `is already checked out`)
- "A branch with that name already exists. Pick a different worktree name."

### Remove flow

Click the trash icon next to a non-root worktree → workspace runs an async git status check (`git status --porcelain` + `git rev-list --count @{u}..HEAD`) on that worktree, then opens the `RemoveWorktreeConfirmationDialog` with:

- The worktree path being removed.
- A list of open tabs whose CWD is under that path (these will close).
- A warning section listing dirty signals — uncommitted changes, untracked files, unpushed commits — when present. "Worktree is clean." otherwise.

On Cancel / ESC: dialog closes; no side effects.

On Confirm:
1. Affected tabs close **immediately** (snappy UI).
2. `git worktree remove [--force] <path>` runs in the background. `--force` only when the worktree was dirty (the user already saw and confirmed the warning).
3. Failure surfaces as a persistent error toast — tabs stay closed because the user already confirmed the destructive intent.
4. The chip is marked pending (`⟳`) so the count updates.

The root worktree is never removable: its menu item shows a green "root" badge in place of the trash icon.

## Edge Cases

1. **No git repo**: shell command returns nothing → chip doesn't render.
2. **Single worktree (just root)**: chip still renders; count shows `0` alternatives. Could be hidden in a follow-up.
3. **Bare repo**: no main worktree; all entries are linked. The "root" detection (`<path>/.git is_dir`) returns no match, so no badge and no prefix on names. Count is total - 1.
4. **Worktree created outside `~/.warp/worktrees/` convention**: appears in the chip normally — the chip uses `git worktree list` directly and doesn't filter by location. Claude/cursor/codex worktrees in their own folders are visible.
5. **Repo with no commits**: BranchPicker may show a branch that resolves to nothing (HEAD = `0000…`). `git worktree add` fails with `invalid reference`; the friendly error tells the user to commit first.
6. **Origin = "HEAD"**: when a worktree was created via `git worktree add -b <name> <path>` (no base branch specified), git records "Created from HEAD" in the reflog. The chip shows `<branch> → HEAD` literally. Future improvement could resolve HEAD → branch heuristically.
7. **Reflog pruned**: origin info missing for old branches; the third line is rendered as an empty spacer to keep row heights aligned.
8. **Remove on dirty worktree without `--force` enabled**: handled by git, surfaced as toast.
9. **Tab closes during remove**: tab indices are snapshotted before the git command runs, so concurrent tab closures don't corrupt the cleanup.

## Success Criteria

1. The chip renders inside a git repo and shows `<current-name> · <count-of-alternatives>`.
2. Clicking the chip opens a menu listing all worktrees with the green-bar marker on the current one and the "root" badge on the main.
3. Clicking a non-current row opens that worktree in a new tab.
4. Clicking the trash icon opens the remove confirmation dialog with accurate dirty-state info and a list of affected tabs.
5. Confirming the remove closes affected tabs immediately and runs `git worktree remove` in the background.
6. The "Create new worktree…" footer opens a 4-field modal that runs `git worktree add -b` and opens the result in a new tab.
7. Creating a worktree with an empty name keeps the Create button disabled.
8. Branch name field auto-mirrors the worktree name until the user edits it.
9. The chip count flips to `⟳` for ~6s after a remove or create and updates on the next 5s refresh.

## Validation

- Build and run Warp locally with `FeatureFlag::GitWorktreesChip` + `GitWorktreesChipCreate` enabled.
- Open a git repo with multiple worktrees (or create some via `git worktree add` in CLI).
- Verify the chip appears with the correct count and current-worktree name.
- Open the menu; verify the green bar marks the current worktree, the "root" badge marks the main, the trash icon appears on non-root rows, and the third line shows `<branch> → <origin>` where reflog has the entry.
- Click another worktree row; verify a new tab opens at that worktree's path.
- Click a trash icon; verify the confirmation dialog appears with the path and the list of tabs that will close. If the worktree is dirty, verify the warning section lists the dirty signals.
- Confirm the dialog; verify affected tabs close instantly and the worktree is gone from the menu after the next 5s refresh.
- Click "+ Create new worktree…"; verify the modal opens with destination pre-filled. Type a worktree name and verify the branch name field mirrors char-by-char. Override the branch name and verify the mirror stops. Click Create; verify the worktree appears at the destination path and opens in a new tab.
- Try to create with an empty name; verify the Create button is disabled and the validation message appears.
- Try to create with a duplicate branch name (or in a repo with no commits); verify the friendly error toast.

## Open Questions

- Designer to confirm the worktree icon glyph (current is a placeholder folder + branch SVG).
- Whether to add a setting to compact the working-directory chip's last segment when in a worktree (avoids displaying the worktree name twice — once in folder chip, once in worktree chip). Tracked as a follow-up; default behavior is to show both.
- Whether to add radio buttons for common branch prefixes (`feature/`, `fix/`, `chore/`) in the create modal. Deferred until usage proves it's worth the extra UI.
- Whether to include a watcher (`.git/worktrees/` filesystem events) instead of the current 5s polling refresh. Tracked as Fase 1.1 follow-up — works fine in practice but adds latency on the count update.
