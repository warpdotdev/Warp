# gh-10207 — File tree handling for repos exceeding `MAX_FILES_PER_REPO`

## Context

[Issue #10207](https://github.com/warpdotdev/warp/issues/10207): Project Explorer
silently shows populated folders (e.g. `.agents`) as empty when a repo exceeds
`MAX_FILES_PER_REPO` (100,000) during initial indexing. The reporter has
~153k tracked files in `~/code` and sees `Failed to build file tree for
repository: ExceededMaxFileLimit` in the log.

Root cause:

- `crates/repo_metadata/src/local_model.rs:892` runs `Entry::build_tree` with
  `MAX_TREE_DEPTH=200` and a 100k-file global quota
  (`crates/repo_metadata/src/local_model.rs:60`).
- On `ExceededMaxFileLimit` (`crates/repo_metadata/src/entry.rs:14-28`,
  raised in `entry.rs:157-220`), the spawn callback marked the repo
  `IndexedRepoState::Failed`. The view (`app/src/code/file_tree/view.rs:1584-1586`)
  swaps a `Failed` state for an empty `FileTreeEntry`.
- Lazy per-directory expansion already exists (`Entry::load`,
  `entry.rs:257-284`, with `LAZY_LOAD_FILE_LIMIT=5000`) but is unreachable
  when the root is `Failed`.

Current PR ([#10234](https://github.com/warpdotdev/warp/pull/10234)) — reactive
fallback:

- On `ExceededMaxFileLimit`, retry `build_tree` with `max_depth=1` and the
  same 100k file quota so the root is `Indexed` with unloaded subdirectories.
  Lazy-load handles expansion.
- **Edge case the current PR doesn't cover:** if the repo has >100k files
  *directly* at the root (not in subdirectories), the depth-1 retry hits
  `ExceededMaxFileLimit` again because the file quota is consumed at depth=1
  for direct-child files (`entry.rs:214-220`). The repo lands back in
  `IndexedRepoState::Failed` and the tree is empty — the original bug. This
  is rare in practice (most repos spread files across subdirectories) but
  the fix should remove it.
- New `RepositoryIndexedWithLimit` event surfaces a transient toast in the
  file tree.
- Skill watcher gains a `find_top_level_skill_directories_in_filesystem`
  fallback (gitignore-aware) so repo-local skills aren't lost; same probe is
  reused by the synchronous cloud-environment path
  (`SkillWatcher::read_skills_for_repos`).

@moirahuang's review asked three open questions before merging. This spec
evaluates the alternatives and recommends a path.

Out of scope but worth flagging: `ai::project_context::model`
(`crates/ai/src/project_context/model.rs:298`),
`warp::ai::outline::native`, and
`ai::index::full_source_code_embedding` each call `Entry::build_tree`
independently and hit the same hard limit. They are tracked separately and
this spec does not change their behavior.

## Proposed changes

### Q1 — Pre-detect "too many files" or always lazy-load?

**Three options considered:**

1. **Pre-detect, then choose mode.** Cheaply count files first (e.g.
   `git ls-files --cached | wc -l` for git repos) and skip the full walk
   when over budget.
2. **Always lazy-load.** Drop full-depth indexing; treat every repo as
   degraded.
3. **Reactive fallback (current PR).** Try full, fall back on the limit error.

**Recommendation: keep reactive fallback (option 3), do not add pre-detection
in this spec.**

- **Option 1** only helps git repos (the user's repro is git-tracked, but
  `IgnoredPathStrategy::IncludeLazy` in `local_model.rs:899` means we walk
  more than just tracked files anyway). It adds a separate code path, an
  external `git` invocation, and a latency floor to every open. The savings
  are bounded to the wasted partial walk before the quota trips, which on
  the reporter's 153k repo is ~15s of background work — perceptible in
  logs, not perceptible in the UI thanks to `ctx.spawn` (`local_model.rs:892`).
- **Option 2** regresses small/medium repos. Full-depth indexing is what
  feeds AI context, file search, and project rules; making everything lazy
  would either silently break those features for normal-size repos or push
  the same problem into other consumers. Not worth it.
- **Option 3** already handles git, non-git, partial-clone, submodule, and
  deeply-nested cases uniformly. The only cost is the partial walk on huge
  repos, paid once per session.

**One refinement to land with this spec:** the depth-1 retry should pass
`None` for `remaining_file_quota` (`entry.rs:94`) instead of reusing
`MAX_FILES_PER_REPO`. Cost is bounded by the count of root-level entries
(directories return as unloaded immediately at depth=1; only direct-child
files consume memory). For a hypothetical 1M-file root that's ~1M
`FileMetadata` allocations (~100 bytes each → ~100 MB) — order of magnitude
more than the typical case but still cheaper than the `Failed → empty tree`
status quo, and it ensures every repo can at least show its top level.
With this change, the only path that still fails is `read_dir` itself
returning an error, which already maps to `BuildTreeError::IOError`.

**Followup (not this PR):** add a telemetry counter on the persistent
`indexed_with_limit` transition (and reuse the existing
`RepoMetadataTelemetryEvent::BuildTreeFailed { error: "ExceededMaxFileLimit" }`
already emitted at `local_model.rs:1001`) plus the time spent in the failed
full-depth `build_tree`. If the partial-walk latency turns out to matter,
revisit option 1 (pre-detection) as an optimization layered on top.

### Q2 — Is the toast necessary?

The toast was added to satisfy the issue's expected behavior (b) — "show a
clear, user-visible message that indexing was limited." But:

- Degraded mode is persistent for the session, while the toast is transient
  (`add_ephemeral_toast`, `view.rs:1614-1623`). During testing the reporter
  missed it and we needed to relaunch.
- The same user opens the same repo every day; a daily toast becomes noise.
- Other features (AI codebase indexing) already render a persistent
  "Codebase too large" status (`settings_view/code_page.rs:1520-1539`), so a
  parallel persistent indicator in the file tree fits an existing pattern.

**Recommendation: replace the transient toast with a persistent, dismissible
inline indicator in the file tree header for the affected root.**

- New per-root state on `RootDirectory` (`view.rs:251`): `indexed_with_limit:
  bool` plus a per-view `dismissed_indexed_with_limit: HashSet<StandardizedPath>`.
- The indicator renders inline next to the root header (small warning glyph
  with hover tooltip "Repository is too large to fully index. Subfolders
  load when expanded."). Clicking dismisses it for the session.
- Drop `RepositoryMetadataEvent::RepositoryIndexedWithLimit` and the
  forwarding plumbing; carry the flag on `FileTreeState` instead
  (`crates/repo_metadata/src/file_tree_store.rs:353`). State-on-the-model
  beats one-shot event because it survives view-mount-after-indexing
  (the cause of the toast-missed bug we hit during manual testing) and
  workspace reopens within the same session.
- View checks the flag in two places: when handling `RepositoryUpdated` and
  when registering a root via `register_and_refresh_lazy_loaded_directory`
  (`view.rs:1532-1589`).

If the team prefers no UI signal at all, drop the indicator entirely and
just keep the existing `safe_warn!` log line. The folder visibly expanding
arguably is the signal — in degraded mode the user mostly cannot tell
unless they look for AI rules / search results that are missing.

### Q3 — Performance implications of lazy load

`Entry::load` (`entry.rs:257-284`) runs synchronously on the main thread via
`load_directory_from_model` (`view.rs:1410-1431`). Each expand:

- Reads one directory (one `read_dir` syscall + one `is_symlink/is_dir` per
  child).
- Up to `LAZY_LOAD_FILE_LIMIT=5000` entries are loaded; over that, the
  existing per-folder error toast fires (`view.rs:1421`).
- Walks gitignore matchers (`matches_gitignores` per child).

For the reporter's repo (153k files, 1500+ top-level subdirs of 100 files
each), each expand is bounded and fast. For real monorepos a single
subdirectory may exceed 5,000 (`node_modules`, `vendor`, generated
`.pb.go`s) and the user gets a toast with no contents — the existing
behavior. Subjectively still better than the silent-empty-tree this PR
fixes.

**Recommendations:**

- **Instrument**, don't tune. Add a one-time telemetry event on lazy-load
  with `entry_count` and `duration_ms`. Decide later whether to raise
  `LAZY_LOAD_FILE_LIMIT` based on real distributions; raising it without
  data risks main-thread stalls on giant subdirectories.
- **Consider moving lazy-load off the main thread** as a follow-up
  (`ctx.spawn` mirroring the initial-build pattern). Out of scope for this
  PR — the existing 5k cap keeps each call short enough that the UI doesn't
  visibly stall in our manual testing.
- The recursive watcher registered by `add_repository_internal`
  (`local_model.rs:382`) already covers the whole repo regardless of which
  subtrees are loaded, so file-change notifications are not affected by
  lazy mode.

## Testing and validation

- Unit: extend `crates/repo_metadata/src/local_model_test.rs` with a test
  that drives `Entry::build_tree` past the quota (parameterize
  `MAX_FILES_PER_REPO` via a test-only override or a constructor argument
  on the model) and asserts the resulting `FileTreeState.indexed_with_limit
  == true` with depth-1 children only. Also assert
  `RepositoryUpdated` fires (and `UpdatingRepositoryFailed` does not).
- Unit (regression for the depth-1 quota gap noted in Context): a fixture
  with `MAX_FILES_PER_REPO + 1` files placed *directly under the repo
  root* (i.e. no intermediate directories). Without the unquoted retry the
  fallback reproduces the original bug; with `remaining_file_quota = None`
  on the depth-1 retry the test asserts `IndexedRepoState::Indexed` with
  `indexed_with_limit == true` and the root entry containing all top-level
  files.
- Unit: `find_top_level_skill_directories_in_filesystem` honors gitignore.
  Add a fixture with a `.gitignore` excluding `.claude/skills` and a
  populated `.claude/skills/foo/SKILL.md`; assert the probe returns empty.
- Unit: `read_skills_for_repos` deduplicates between the tree-based and
  filesystem-probed paths.
- View: a layout test for the inline degraded indicator (one for visible,
  one for dismissed).
- Manual: use the fixture at `~/code-fixtures/warp-10207-large-repo`
  (150,001 files + `.agents/skills/example-skill/SKILL.md`). Build with
  `./script/run --dont-open` and `open -a target/debug/bundle/osx/WarpOss.app
  ~/code-fixtures/warp-10207-large-repo`. Verify:
  1. `~/Library/Logs/warp-oss.log` contains `Repository exceeded max file
     limit; indexed in degraded mode`.
  2. `.agents`, `src`, etc. expand and show their contents.
  3. The persistent inline indicator is visible on the root header.
  4. The agent panel lists `example-skill` (proves the filesystem probe
     ran).
  5. Subfolders within `LAZY_LOAD_FILE_LIMIT` expand without the per-folder
     toast.

## Risks and mitigations

- **Carrying `indexed_with_limit` on `FileTreeState`** is observable from
  many crates that pattern-match the struct. We avoid breakage by adding a
  `bool` field with a `Default` value of `false` and updating the two
  constructors (`new`, `new_lazy_loaded`); existing call sites compile
  unchanged.
- **Watcher pressure on 153k-file repos.** The recursive `register_path`
  call already happens today for any indexed repo; this PR doesn't change
  the watcher footprint. If users report watcher CPU/memory issues, a
  follow-up could prune the watch to actually-loaded subtrees, but that's
  a larger change.
- **Discovery for nested provider paths in degraded repos.** The
  filesystem probe only checks `<repo>/<provider>/skills`; a nested
  `<repo>/sub/.agents/skills` is missed until the watcher reports an
  add or until the user expands `sub`. Acceptable: the watcher catches
  later additions, and root-level provider paths are the predominant
  layout.

## Follow-ups

- Apply the same fallback / probe pattern to `ai::project_context::model`,
  `ai::outline::native`, and `ai::index::full_source_code_embedding`, or
  surface the "codebase too large" status alongside the file-tree
  indicator so users see one consistent message.
- Telemetry: counter on the `FileTreeState.indexed_with_limit` transition
  (the existing `BuildTreeFailed { error: "ExceededMaxFileLimit" }`
  telemetry at `local_model.rs:1001` already covers the trigger; add a
  paired success counter for the depth-1 retry); histogram for lazy-load
  entry-count and duration. Inform whether to raise `LAZY_LOAD_FILE_LIMIT`
  or move lazy-load off the main thread.
- Make `MAX_FILES_PER_REPO` configurable per-user (settings) so power
  users on large monorepos can opt into a higher limit at the cost of
  memory.
