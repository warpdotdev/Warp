# Spec: Refresh repo metadata after `claude --worktree` (GH-10031)

## Problem

When the user runs `claude --worktree`, it creates a new git
worktree and the user's CWD inside the running pane changes.
But Warp's per-pane repo metadata (branch label, PR pill, diff
chip, file-tree git-status overlays, status-bar repo segment)
does not refresh — it keeps showing the old worktree's
metadata until the user manually triggers a refresh (e.g., by
focusing another tab and back).

## Goal

Detect CWD changes within a single pane and re-derive repo
metadata when the pane's CWD lands inside a different git repo
or a different git worktree. Clear metadata when the CWD leaves
all git repos. Keep the derivation cheap under bursty CWD
updates without losing the final state.

## Behavior contract

### Derivation: single invocation

- B1. The terminal's existing CWD-tracking hook (already used to
  feed prompt PWD into Warp) gains a "repo-root changed"
  derived signal. Repo-context derivation is a SINGLE
  `git rev-parse` invocation that requests both fields at once:
  ```sh
  git rev-parse --show-toplevel --git-common-dir
  ```
  The two output lines are produced from the same git snapshot,
  so they are guaranteed consistent. There is no second
  `rev-parse` call. The throttle in B3 caps these single
  invocations.

### Path normalization (canonicalization)

- B-CANON. Both output paths are normalized before comparison
  via `canonical(p)` — the canonical path of `p` after symlink
  resolution and conversion to an absolute path (`std::fs::
  canonicalize` semantics). The repo-context tuple is:
  ```
  (canonical(top_level), canonical(git_common_dir))
  ```
- B-CANON-1. Implementations MUST NOT compare raw `git
  rev-parse` output strings — git can return relative or
  partially-resolved paths depending on invocation context.
- B-CANON-2. Consequences:
  - Same repo, different SUBDIRECTORY of the same worktree →
    same canonical tuple → no `RepoContextChanged` event.
  - Symlink that points to the same canonical worktree → same
    canonical tuple → no event.
  - Move into a different worktree of the same repo →
    `top_level` differs, `git_common_dir` matches → event with
    repo identity preserved (B4).
  - Move into an unrelated repo → both differ → full refresh.

### Event firing

- B2. When the canonical
  (`top_level`, `git_common_dir`) tuple changes, fire a
  `RepoContextChanged { pane_id, top_level, git_common_dir }`
  event that the existing repo-metadata subsystem responds to.
  (It currently fires only on pane creation and tab switch.)

### Throttle (leading + trailing edge)

- B3. Throttle repo-context derivation per pane:
  - **Leading-edge fire.** The first CWD change after an idle
    interval triggers a derivation immediately.
  - **Window.** During a 250ms window after a derivation,
    further CWD changes do NOT spawn additional derivations.
  - **Trailing-edge re-derivation (mandatory).** If the CWD
    changed at any point during the throttle window, schedule
    exactly ONE additional derivation to run once the window
    elapses, using the LATEST CWD at that moment. This
    guarantees the final state is correct after a burst.
  - Skip the spawn entirely if CWD hasn't changed since the
    last completed derivation.
- B3-budget. Net effect: at most one derivation immediately
  plus one trailing derivation per 250ms burst per pane. Steady
  state: ≤2 derivations per 250ms window per pane, ≤4 per
  second per pane.

### Worktree vs different repo

- B4. Worktree detection (canonical paths):
  - Different `top_level`, SAME `git_common_dir` → different
    worktrees of the same repo. Metadata refreshes (branch /
    PR / diff / file-tree overlays / status segment) but the
    repo "identity" (org/name) is preserved.
  - Different `top_level`, DIFFERENT `git_common_dir` →
    different repos. Everything refreshes including identity.

### Non-git clear path

- B5. **Non-git clear path (event-driven, full surface).** When
  `git rev-parse` exits non-zero (CWD is not inside any git
  repo), fire the explicit event:
  ```
  RepoMetadataCleared { pane_id }
  ```
  The metadata subsystem's clear handler MUST reset the FULL
  metadata surface for that pane, enumerated:
  - branch name label → cleared / hidden
  - ahead/behind counters → cleared
  - dirty-file count / status indicator → cleared
  - current PR (PR pill + linked PR data) → cleared
  - cached diff (diff chip + open diff panels) → cleared
  - file-tree git-status overlays for paths under the prior
    repo root → cleared
  - status-bar repo segment → collapsed to "no repo" empty
    state
  - any open PR or diff panels for the prior repo → collapse
    to "no repo" empty state
- B5a. Subsequent CWD changes that remain outside any repo do
  NOT re-fire `RepoMetadataCleared`; the cleared state is
  sticky until a new resolvable repo context appears.
- B5b. No event other than `RepoMetadataCleared` is emitted on
  this path; the prior `RepoContextChanged` is NOT emitted with
  empty fields.

## Acceptance criteria

- A1. Run `claude --worktree feature-x` in a pane; within ≤1s
  the branch label updates from the old worktree's branch to
  `feature-x`.
- A2. PR pill, diff chip, file-tree git-status overlays, and
  status-bar repo segment all refresh on the same trigger.
- A3. `cd` into a non-git directory clears the FULL metadata
  surface per B5 enumeration (branch, ahead/behind, dirty,
  PR, diff, file-tree overlays, status segment, open panels).
- A4. `cd` between two unrelated repos refreshes everything
  including repo identity.
- A5. Throttle + trailing edge: a burst of 10 rapid `cd`s
  within 250ms results in exactly TWO derivations (one leading,
  one trailing on the FINAL CWD), and the metadata reflects
  the FINAL CWD — not any intermediate one.
- A-CANON. `cd` between two subdirectories of the same
  canonical worktree (including via symlink) does NOT fire
  `RepoContextChanged`.
- A-CLEAR-EVENT. `cd` from a git repo to a non-git path emits
  exactly one `RepoMetadataCleared { pane_id }` event; no
  `RepoContextChanged` is emitted on this transition.

## Implementation pointers

- CWD tracking hook is fed by the shell's PWD update path
  (existing); the receiver is in
  `app/src/terminal/...` (grep for `pwd_changed` /
  `current_directory_changed`).
- Repo-metadata subsystem entry: `crates/repo_metadata/src/...`
  exposes `refresh(pane_id)` (used on `RepoContextChanged`) and
  must expose / use `clear(pane_id)` (used on
  `RepoMetadataCleared`) that resets the enumerated full
  surface in B5.
- Throttle implementation: leading-edge timer plus a
  "dirty-during-window" flag that schedules a trailing
  derivation; cancel the trailing schedule if the window
  elapses with no CWD change.
- Path canonicalization: use the platform `canonicalize` /
  realpath equivalent; cache the previous canonical tuple per
  pane.

## Test plan

- T1. Synthetic CWD-change event triggers a single
  `RepoContextChanged` with canonical paths.
- T2. Two synthetic events within 100ms produce ONE leading
  derivation and ONE trailing derivation with the latest CWD;
  both invocations are the single
  `git rev-parse --show-toplevel --git-common-dir` form.
- T3. CWD into a non-git path emits exactly one
  `RepoMetadataCleared` event and the clear handler resets the
  full surface enumerated in B5 (each field asserted).
- T4. Worktree-vs-different-repo distinction returns the
  expected identity field, using canonical-tuple comparison.
- T-CANON. Subdir `cd` within same worktree, and symlink-based
  CWD changes pointing to same canonical path, BOTH produce no
  `RepoContextChanged`.
- T-TRAILING. Burst of 10 CWD changes within 250ms results in
  exactly 2 derivation processes and the final metadata
  reflects the LAST CWD.
- T-CLEAR-STICKY. `cd` non-git → non-git emits no further
  `RepoMetadataCleared`.

## Out of scope

- File-watching the repo for branch changes (e.g., `git checkout`
  in another terminal). That's a separate enhancement.
- Refreshing on file system events inside the worktree (HEAD
  file change). V1 is CWD-driven only.
- Tuning the 250ms throttle window per platform — fixed in V1.
