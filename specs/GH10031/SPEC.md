# Spec: Refresh repo metadata after `claude --worktree` (GH-10031)

## Problem

When the user runs `claude --worktree`, it creates a new git
worktree and the user's CWD inside the running pane changes.
But Warp's per-pane repo metadata (branch label, PR pill, diff
chip) does not refresh — it keeps showing the old worktree's
metadata until the user manually triggers a refresh (e.g., by
focusing another tab and back).

## Goal

Detect CWD changes within a single pane and re-derive repo
metadata when the pane's CWD lands inside a different git repo
or a different git worktree.

## Behavior contract

- B1. The terminal's existing CWD-tracking hook (already used to
  feed prompt PWD into Warp) gains a "repo-root changed"
  derived signal: run one repo-context derivation process for
  the new CWD. The derivation should ask a single `git rev-parse`
  invocation for both `--show-toplevel` and `--git-common-dir`
  so the two values are produced from the same snapshot. The
  latter resolves to the shared `.git` directory across worktrees,
  identifying the worktree group.
- B2. When (`toplevel`, `git-common-dir`) tuple changes, fire a
  `RepoContextChanged` event that the existing repo-metadata
  subsystem already responds to. (It currently fires only on
  pane creation and tab switch.)
- B3. Throttle repo-context derivation: at most one derivation
  process per 500ms per pane. A derivation means one spawned
  `git rev-parse --show-toplevel --git-common-dir` process, not
  one process per output value. Skip the spawn entirely if CWD
  hasn't changed since the last check.
- B4. Worktree detection: two paths with the same
  `git-common-dir` are different worktrees of the same repo;
  metadata refreshes (branch / PR / diff) but the repo "identity"
  (org/name) stays the same. Two paths with different
  `git-common-dir` are different repos; everything refreshes.
- B5. Failure mode: if `git rev-parse` fails (CWD is no longer
  inside a git repo), clear the metadata for the pane (no stale
  branch label hanging around) and emit no event beyond the
  clear.

## Acceptance criteria

- A1. Run `claude --worktree feature-x` in a pane; within ≤1s
  the branch label updates from the old worktree's branch to
  `feature-x`.
- A2. PR pill and diff chip refresh on the same trigger.
- A3. `cd` into a non-git directory clears the metadata.
- A4. `cd` between two unrelated repos refreshes everything
  including repo identity.
- A5. Throttle: rapid back-and-forth `cd` does not spawn more
  than 2 repo-context derivation processes per second per pane.

## Implementation pointers

- CWD tracking hook is fed by the shell's PWD update path
  (existing); the receiver is in
  `app/src/terminal/...` (grep for `pwd_changed` /
  `current_directory_changed`).
- Repo-metadata subsystem entry: `crates/repo_metadata/src/...`
  already exposes a `refresh(pane_id)` API.

## Test plan

- T1. Synthetic CWD-change event triggers a single
  `RepoContextChanged`.
- T2. Two synthetic events within 100ms produce one repo-context
  derivation process and one event (throttle).
- T3. CWD into a non-git path emits a "clear" event.
- T4. Worktree-vs-different-repo distinction returns the
  expected identity field.

## Out of scope

- File-watching the repo for branch changes (e.g., `git checkout`
  in another terminal). That's a separate enhancement.
- Refreshing on file system events inside the worktree (HEAD
  file change). V1 is CWD-driven only.
