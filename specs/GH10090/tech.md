# Watch remote refs for updates — Tech Spec
Product spec: `specs/GH10090/product.md`
GitHub issue: https://github.com/warpdotdev/warp/issues/10090

## Problem
`repo_metadata` already watches repository roots and selected Git internals, but remote-tracking refs under `.git/refs/remotes/*` are filtered out. Code review metadata computes unpushed commits from the current branch's upstream ref, so a push or fetch that updates a loose remote-tracking ref can leave `DiffStateModel` and the Git operations UI stale until another invalidation happens.

The implementation needs to allow loose remote-ref watcher events through, route them only to watched repositories whose current branch tracks that remote ref, and reuse the existing metadata refresh path by marking the resulting `RepositoryUpdate` as commit-related.

## Relevant code
- `crates/repo_metadata/src/entry.rs (361-484)` — Git internal path helpers: `git_suffix_components`, `extract_worktree_git_dir`, `is_shared_git_ref`, `is_commit_related_git_file`, `is_index_lock_file`, and `should_ignore_git_path`.
- `crates/repo_metadata/src/entry_test.rs (222-395)` — current allowlist and shared-ref tests; remote refs are currently asserted as ignored and not shared.
- `crates/repo_metadata/src/watcher.rs (120-194)` — `DirectoryWatcher::find_repos_for_git_event`, which routes worktree-specific, shared local branch ref, and repo-specific Git events.
- `crates/repo_metadata/src/watcher.rs (293-335)` — `start_watching_directory`, which registers watched paths with the `should_ignore_git_path` filter.
- `crates/repo_metadata/src/watcher.rs (391-529)` — filesystem event handling that converts Git internal events into `RepositoryUpdate`.
- `crates/repo_metadata/src/watcher.rs (588-626)` — `RepositoryUpdate`, especially `commit_updated` and `index_lock_detected`.
- `crates/repo_metadata/src/repository.rs (54-151)` — `Repository` stores `root_dir`, optional per-worktree `external_git_directory`, and optional shared `common_git_directory`.
- `crates/repo_metadata/src/repository.rs (162-229)` — `Repository::start_watching`, which registers the worktree root, per-worktree gitdir, and shared `refs/heads` for linked worktrees.
- `app/src/code_review/diff_state.rs (1116-1247)` — code review repository subscriber maps `commit_updated` to full metadata invalidation and throttled metadata refresh.
- `app/src/code_review/diff_state.rs (1347-1393)` — `load_metadata_for_repo` reads `@{u}` and recomputes `unpushed_commits`.
- `app/src/code_review/git_status_update.rs (168-283)` — Git status metadata watcher also refreshes when `commit_updated` is true.
- `app/src/util/git.rs (391-468)` — `get_unpushed_commits` computes `<upstream>..HEAD`.

## Current state
`should_ignore_git_path` uses an allowlist for `.git` internals. Only commit-related files (`HEAD` and `refs/heads/*`) and `index.lock` are allowed through; `.git/refs/remotes/origin/main` is explicitly ignored in `entry_test.rs`.

For normal repositories, the repository root watcher is recursive, so `.git/refs/remotes/*` would be observable if the filter allowed it. For linked worktrees, remote refs are stored in the shared common `.git` directory, outside the worktree checkout and outside the per-worktree gitdir. `Repository::start_watching` currently adds `common_git_dir/refs/heads` for shared local branch refs, but it does not add `common_git_dir/refs/remotes`.

Once a Git internal path reaches `DirectoryWatcher::handle_watcher_event`, it is classified with `is_commit_related_git_file` or `is_index_lock_file`. A `commit_updated` update is enough to trigger the existing code review and git-status metadata refresh paths. The missing pieces are path classification, watcher registration for linked worktrees, and scope-aware routing to only repositories tracking the changed remote ref.

## Proposed changes

### 1. Add remote-tracking ref path helpers
Extend `crates/repo_metadata/src/entry.rs` with helpers for loose remote-tracking refs:

- `is_remote_tracking_ref(path: &Path) -> bool`
  - true for paths under `.git/refs/remotes/<remote>/<branch...>`.
  - false for paths under `.git/worktrees/<name>/...`, `.git/refs/heads/*`, `.git/refs/tags/*`, `.git/packed-refs`, and non-Git paths.
  - requires at least a remote component and one branch component after `refs/remotes`.
- `remote_tracking_ref_components(path: &Path) -> Option<(String, String)>`
  - returns `(remote, branch)` where `branch` preserves slash-separated branch names, for example `.git/refs/remotes/origin/users/alice/feature` maps to `("origin", "users/alice/feature")`.
- `remote_tracking_ref_path_under_common_git_dir(path: &Path) -> Option<PathBuf>`
  - canonicalization-friendly helper for routing. It returns the full loose ref path only for shared remote refs, not worktree-local files.

Update `should_ignore_git_path` so loose remote-tracking refs are allowlisted. Update `is_commit_related_git_file` to return true for remote-tracking refs, or add a separate `remote_ref_updated` classification and map it to `commit_updated` when constructing `RepositoryUpdate`. Reusing `commit_updated` is the smallest change because all current consumers already use it to reload metadata.

Keep `packed-refs` ignored. Do not add broad `refs/*` matching; tags and other Git internals remain filtered out.

### 2. Teach `Repository` how to identify its tracked remote ref
Add a small synchronous helper to `crates/repo_metadata/src/repository.rs`:

- `pub(crate) fn tracks_remote_ref_path(&self, remote_ref_path: &Path) -> bool`

The helper should determine whether the repository's current branch tracks the loose remote ref that changed. It can do this without introducing a Git command dependency:

1. Read the repository-specific `HEAD` from `self.git_dir().join("HEAD")`.
2. If `HEAD` is not `ref: refs/heads/<branch>`, return false.
3. Read config from `self.common_git_dir().join("config")`.
4. Parse the branch section for the current branch, including quoted subsection syntax such as `[branch "feature/foo"]`.
5. Read `remote = <remote>` and `merge = refs/heads/<merge-branch>`.
6. Build the expected loose remote ref path as `self.common_git_dir()/refs/remotes/<remote>/<merge-branch>`.
7. Compare it with the changed `remote_ref_path` after normalizing/canonicalizing existing parent paths where possible.

Implementation details:

- Keep the parser intentionally narrow. It only needs branch `remote` and `merge` values from `.git/config`.
- Return false on detached `HEAD`, missing config, malformed branch section, missing `remote`, missing `merge`, `remote = .`, or merge refs outside `refs/heads/`.
- Branch and merge names may contain slashes.
- Use the repository's `git_dir()` for `HEAD` so linked worktrees read their own active branch, and `common_git_dir()` for config and remote refs so worktrees share upstream config and refs.
- File reads are small and happen only for allowlisted remote-ref events. If reviewers are concerned about synchronous I/O in watcher routing, move the same check behind the existing task queue before delivering subscriber updates; do not broaden routing to all repositories as a shortcut.

This helper is the scope boundary that satisfies the product requirement that only repositories tracking the changed remote ref refresh metadata.

### 3. Register shared refs for linked worktrees
Update `Repository::start_watching` in `crates/repo_metadata/src/repository.rs` so linked worktrees watch shared refs from the common Git directory in a way that includes remote-tracking refs. Prefer registering `common_git_dir/refs` when it exists, then rely on `should_ignore_git_path` to keep the watcher allowlist narrow. This lets a linked worktree observe both existing remote refs and first-time creation under `refs/remotes` without having to create Git directories from Warp.

If reviewers prefer the narrower current shape, an acceptable implementation is to register both `common_git_dir/refs/heads` and `common_git_dir/refs/remotes` when they exist, plus fall back to `common_git_dir/refs` only when `refs/remotes` is absent. The important requirement is that linked worktrees observe loose remote-tracking ref file creation and updates in the common Git directory while tags and other refs remain filtered out.

Update `Repository::stop_watching` to unregister the same shared refs path or paths when the last subscriber is removed.

For normal repositories, no extra watch path is required because the root watcher already recursively covers `.git/refs/remotes` once the filter allows those paths. It is acceptable to share a small helper that appends optional common refs directories so start/stop stay symmetric.

### 4. Add remote-ref routing in `DirectoryWatcher`
Update `DirectoryWatcher::find_repos_for_git_event` in `crates/repo_metadata/src/watcher.rs` with a new routing tier for shared remote-tracking refs:

1. Worktree-specific paths under `.git/worktrees/<name>/...` keep the existing highest-priority route.
2. Remote-tracking refs under `.git/refs/remotes/*` route to watched repositories whose `common_git_dir()` contains the event path and whose `tracks_remote_ref_path(event_path)` returns true.
3. Shared local branch refs under `.git/refs/heads/*` keep the existing broadcast-to-shared-common-git-dir route.
4. Repo-specific paths keep the existing fallback route.

The remote-ref tier should deduplicate repository handles just like the existing tiers. It should log the routing tier and affected repo count with the existing `[GIT_EVENT_ROUTING]` pattern.

Remote-ref events should produce a `RepositoryUpdate` with `commit_updated = true`. No changed file should be added to `added`, `modified`, `deleted`, or `moved`, matching the current treatment for local Git metadata files.

### 5. Preserve code review and git status consumers
No consumer-specific code should be needed if remote-ref events set `commit_updated`.

Existing flow after the watcher update:

1. Watcher sees `.git/refs/remotes/origin/feature`.
2. `find_repos_for_git_event` routes only to repositories tracking `origin/feature`.
3. `handle_watcher_event` enqueues `RepositoryUpdate { commit_updated: true, ... }`.
4. `DiffStateModelRepositorySubscriber` forwards the update unless the index lock is present.
5. `DiffStateModel::handle_file_update` emits `DiffMetadataChanged(All(MetadataChange))` and schedules throttled metadata refresh.
6. `load_metadata_for_repo` re-reads `@{u}` and recomputes `get_unpushed_commits`.
7. `CodeReviewView` observes the updated metadata and recalculates the primary Git action.

`GitRepoStatusModel::should_refresh_metadata` also returns true for `commit_updated`, so branch/status metadata remains consistent.

### 6. Tests
Add and update unit tests in `crates/repo_metadata`:

- `entry_test.rs`
  - `should_ignore_git_path` does not ignore `.git/refs/remotes/origin/main`.
  - remote branch names with slashes are recognized.
  - `.git/refs/remotes/origin` without a branch is not recognized.
  - `.git/packed-refs`, `.git/refs/tags/*`, `.git/refs/heads/*`, and worktree-local paths are not remote-tracking refs.
  - existing local branch and index-lock assertions remain unchanged.
- `repository.rs` tests or a new `repository_tests.rs`
  - `tracks_remote_ref_path` returns true for a branch with `remote = origin` and `merge = refs/heads/feature`.
  - returns true when the merge branch contains slashes.
  - returns false for detached `HEAD`, missing branch config, wrong remote, wrong branch, `remote = .`, and malformed `merge`.
  - uses worktree `HEAD` plus common config for linked worktrees.
- `watcher_tests.rs`
  - a remote-ref event is delivered to a subscriber as `commit_updated = true` and contains no file-list changes.
  - an unrelated remote-ref event is not delivered to a repository that tracks a different upstream.
  - two repositories/worktrees tracking the same remote ref both receive the update.
  - linked worktrees register/unregister shared refs in a way that covers `refs/remotes`, including first-time creation under `refs/remotes` when the common `refs` directory exists.
  - local `refs/heads/*`, worktree-specific `HEAD`, and `index.lock` routing continue to pass existing regression tests.

Add or extend app-level tests only if the repo-metadata tests cannot prove the end-to-end invalidation contract. The key app-level assertion is that `DiffStateModel::handle_file_update` already treats `commit_updated` as full metadata invalidation; this behavior is already in `app/src/code_review/diff_state.rs`.

### 7. Manual validation
1. Open Warp code review on a branch tracking `origin/<branch>` with one or more unpushed commits.
2. Push the branch from Warp or an external terminal.
3. Confirm the loose ref `.git/refs/remotes/origin/<branch>` updates.
4. Confirm the unpushed commit list clears and the primary Git action updates without reopening code review.
5. Repeat with another branch's remote ref update and confirm the active branch does not refresh.
6. Repeat in a linked worktree whose common `.git` directory is outside the worktree checkout.

## Risks and mitigations

### Risk: over-invalidating every worktree sharing a common `.git`
Shared refs are visible to every linked worktree. Broadcasting remote-ref changes to all watched worktrees would satisfy freshness but violate the product requirement and create unnecessary metadata work.

Mitigation: add `Repository::tracks_remote_ref_path` and make it part of the remote-ref routing tier. Tests must include two worktrees sharing the same common `.git` directory but tracking different upstream refs.

### Risk: parsing Git config incorrectly
Git config syntax has edge cases. This feature only needs branch upstream fields and should not become a general config parser.

Mitigation: implement a narrow parser with conservative false negatives. If parsing is ambiguous or unsupported, return false and let existing manual/other refresh paths handle the repository. Cover quoted branch names and slash-containing branch names because those are common.

### Risk: over-broad shared refs watching
Watching `common_git_dir/refs` for linked worktrees is broader than the current `refs/heads` registration.

Mitigation: keep `should_ignore_git_path` as the allowlist boundary. Only local branch refs, remote-tracking refs, and index/HEAD files should pass through; tags and other refs remain ignored. Add tests proving tag changes under the broader watched directory do not produce repository updates.

### Risk: stale reads while Git is updating refs
A filesystem event can fire while Git is still writing a ref or updating config.

Mitigation: route based on path and current config, then reuse the existing debounced watcher and throttled metadata refresh. If metadata load races and observes stale state, later Git file events or manual refresh paths should correct it. Avoid adding bespoke retry loops unless tests prove they are necessary.

## Follow-ups
- Support `.git/packed-refs` if product later needs packed remote refs to trigger metadata refresh.
- Consider a shared utility for reading branch upstream config if other Git UI features need the same information.
- Consider adding telemetry around metadata refresh causes if remote-ref invalidations become performance-sensitive.
