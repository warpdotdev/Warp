# gh-10207 — File tree falls back to lazy loading on `ExceededMaxFileLimit`

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
  raised in `entry.rs:157-220`), the spawn callback marks the repo
  `IndexedRepoState::Failed`. The view (`app/src/code/file_tree/view.rs:1584-1586`)
  swaps a `Failed` state for an empty `FileTreeEntry`.
- Lazy per-directory expansion already exists (`Entry::load`,
  `entry.rs:257-284`, with `LAZY_LOAD_FILE_LIMIT=5000`) but is unreachable
  when the root is `Failed`.

Scope agreed with @moirahuang and @alokedesai
([comment](https://github.com/warpdotdev/warp/pull/10490#issuecomment-4423492563)):
**make the file tree use lazy loading when indexing hits the maximum, and
otherwise leave the user experience unchanged.** No toast, no indicator —
the visible result should be "the folder expands" rather than "the folder
expands plus a banner." A broader "always-lazy file tree" exploration is
tracked internally as a follow-up.

Out of scope but worth flagging: `ai::project_context::model`
(`crates/ai/src/project_context/model.rs:298`), `warp::ai::outline::native`,
and `ai::index::full_source_code_embedding` each call `Entry::build_tree`
independently and hit the same hard limit. They are tracked separately and
this spec does not change their behavior.

## Proposed changes

### File tree fallback (load-bearing)

In the `index_repository` spawn callback (`local_model.rs:892-1030`), on
`Err(BuildTreeError::ExceededMaxFileLimit)`, retry `Entry::build_tree` once
with:

- `max_depth = 1` so the root is loaded with unloaded subdirectory entries
  (`entry.rs:142-150`).
- `remaining_file_quota = None`. Direct-child files at depth=1 consume the
  quota (`entry.rs:214-220`), so reusing `MAX_FILES_PER_REPO` would
  re-trigger `ExceededMaxFileLimit` on the rare repo with >100k files
  *directly* under the root and reproduce the empty-tree bug. Cost is
  bounded by root-entry count since subdirectories return as unloaded
  immediately at depth=1; only top-level files allocate `FileMetadata`.

On retry success, install the repo as `IndexedRepoState::Indexed` via the
existing `add_repository_internal`, which emits the usual
`RepositoryUpdated` event. The view's existing handlers refresh the tree;
the existing per-directory lazy-load path (`view.rs:1410-1431`,
`LAZY_LOAD_FILE_LIMIT=5000`) handles expansion from there.

On retry failure (e.g. `IOError` reading the root directory), keep the
existing `mark_repository_failed` path. This is the same outcome users
already get for unreadable repo roots and is outside the scope of #10207.

No new event, no new state on `FileTreeState`, no UI plumbing in the view.

### Skill discovery in degraded mode

The skill watcher (`app/src/ai/skills/file_watchers/skill_watcher.rs`)
discovers repo-local skills (`<repo>/.agents/skills/...`,
`<repo>/.claude/skills/...`, etc.) by querying the metadata tree for
directories ending in known skill provider paths
(`find_skill_directories_in_tree`, `app/src/ai/skills/file_watchers/utils.rs:20-56`).
In degraded mode the tree only knows about depth-1 entries, so
`<provider>/skills` (two levels under the repo root) is invisible and the
skills are silently dropped — visible in the AI panel, not the file tree,
but still a user-facing regression in our repro.

Fix: add a sibling probe `find_top_level_skill_directories_in_filesystem`
(`utils.rs`) that walks `SKILL_PROVIDER_DEFINITIONS` and returns
`<repo>/<provider.skills_path>` for the entries that exist on disk,
honoring `gitignores_for_directory(repo_path)` + `matches_gitignores` so it
matches the tree-based path's gitignore semantics (the tree uses
`get_repo_contents` with `include_ignored = false`).

Wire it via the existing two skill-discovery entry points:

- Async: `SkillWatcher::new_internal`'s `RepositoryUpdated` handler. Change
  `scan_repository_for_skills` (`skill_watcher.rs:259-268`) to combine the
  tree-based result with the filesystem probe and dedupe via a
  `HashSet<PathBuf>` before passing to `spawn_read_skills_from_directories`.
  Works for both normal and degraded repos.
- Sync: `SkillWatcher::read_skills_for_repos` (`skill_watcher.rs:77-94`,
  cloud-environment entry point) does the same combine-and-dedupe.

Nested provider paths (`<repo>/sub/.agents/skills`) under unloaded
subtrees are still picked up later by the repository file watcher when the
user expands `sub`, so the probe intentionally only covers top-level
provider paths.

## Testing and validation

- Unit (regression for the original empty-tree bug): drive
  `Entry::build_tree` past `MAX_FILES_PER_REPO` (parameterized via a
  test-only constructor on the model or by lowering the constant under
  `#[cfg(test)]`) and assert the resulting `IndexedRepoState::Indexed`
  with the root containing depth-1 unloaded subdirectory entries. Assert
  `RepositoryUpdated` fires and `UpdatingRepositoryFailed` does not.
- Unit (regression for the depth-1 quota gap, requires the `None` quota
  fix): fixture with `MAX_FILES_PER_REPO + 1` files placed *directly under
  the repo root*. Without the unquoted retry the fallback reproduces the
  original bug. With `remaining_file_quota = None` the test asserts
  `Indexed` with all top-level files present.
- Unit (skill probe gitignore): `.gitignore` excluding `.claude/skills`
  plus a populated `.claude/skills/foo/SKILL.md`; assert the probe
  returns empty.
- Unit (skill probe dedupe): `read_skills_for_repos` and
  `scan_repository_for_skills` produce the same skills regardless of
  whether the tree already contained the provider path.
- Manual: use the existing fixture at
  `~/code-fixtures/warp-10207-large-repo` (150,001 files +
  `.agents/skills/example-skill/SKILL.md`). Build with
  `./script/run --dont-open` and `open -a target/debug/bundle/osx/WarpOss.app
  ~/code-fixtures/warp-10207-large-repo`. Verify:
  1. `~/Library/Logs/warp-oss.log` contains the "indexed in degraded
     mode" warn line (or whatever the implementation logs — non-empty,
     non-error).
  2. `.agents`, `src`, etc. expand and show their contents.
  3. **No toast, no banner.** UI should look like any other repo.
  4. The agent panel lists `example-skill`.

## Risks and mitigations

- **Watcher pressure on 153k-file repos.** The recursive `register_path`
  call already happens today for any indexed repo; this PR does not
  change the watcher footprint. If users report watcher CPU/memory
  issues, a follow-up could prune the watch to actually-loaded subtrees.
- **Discovery for nested provider paths in degraded repos.** The
  filesystem probe only checks `<repo>/<provider>/skills`. Nested
  occurrences (`<repo>/sub/.agents/skills`) are picked up later by the
  repository file watcher when their subtrees are loaded. Acceptable: the
  watcher catches later additions, and root-level provider paths are the
  predominant layout.
- **Silent degradation.** The team's explicit position is that the file
  tree experience should not change, so there is no UI signal that the
  repo is in degraded mode. This means AI features that depend on the
  tree (project rules, outline, codebase index) may behave as if the
  repo is smaller than it is. Those consumers each handle the limit on
  their own and are out of scope here; their own status surfaces (e.g.
  the "Codebase too large" status in AI settings,
  `settings_view/code_page.rs:1520-1539`) remain the place where users
  see the codebase-level signal.

## Follow-ups

- Always-lazy file tree (tracked internally by @moirahuang /
  @alokedesai): if the file tree is moved to lazy loading by default,
  the special-case fallback in this spec collapses into the default
  path and `MAX_FILES_PER_REPO` can be dropped from the file-tree
  pipeline.
- Telemetry: a counter on the depth-1 fallback so we can see how often
  it fires in the wild. The existing
  `RepoMetadataTelemetryEvent::BuildTreeFailed { error: "ExceededMaxFileLimit" }`
  at `local_model.rs:1001` already covers the trigger side; pair it with
  a success counter on the fallback retry.
- Apply the same fallback / probe pattern to `ai::project_context::model`,
  `ai::outline::native`, and `ai::index::full_source_code_embedding`, or
  let the existing "Codebase too large" status remain the one place users
  see degraded-mode messaging.
