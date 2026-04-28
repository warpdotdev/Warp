# APP-4218: Git operations dialogs compare against the branch's actual parent — Tech Spec
Product spec: `specs/APP-4218/PRODUCT.md`
## Context
The Push / Publish dialog, the Create PR dialog, and their AI helpers compare the current branch against a base ref to compute "what's on this branch." The original v1 of this feature picked a base **branch name** via topology heuristics (`detect_parent_branch`), then used `<branch>..HEAD` everywhere.
That shape has a structural bug: when the branch's actual parent (typically `master`/`main`) advances past the fork point, it stops being a strict ancestor of HEAD and gets dropped from the candidate pool. The algorithm then picks a random older ancestor branch, and `<wrong-branch>..HEAD` includes commits the user never wrote.
The v2 design swaps the abstraction. Instead of asking "which branch is my parent?", we directly compute the **fork-point commit** — the most recent commit shared between HEAD and any other branch. All diff/log/AI flows use that SHA. PR creation, which still needs a branch name for `gh pr create --base`, gets a separate, smaller helper.
Relevant code:
- `app/src/util/git.rs` — `detect_parent_branch` / `detect_parent_branch_with_context` (returns a branch name; replaced).
- `app/src/util/git.rs` — `get_unpushed_commits`, `get_branch_diff_entries`, `get_diff_for_pr`, `get_branch_commit_messages`, `create_pr` (callers; switched to fork-point SHA).
- `app/src/code_review/git_dialog/{push,pr}.rs` — per-dialog state, unchanged in shape.
## Proposed changes
### 1. `detect_fork_point` — primary
```rust path=null start=null
// app/src/util/git.rs
/// Returns the SHA of the most recent commit shared between `HEAD` and any
/// other branch. This is the commit at which the current branch began
/// diverging from the rest of the repo.
///
/// Returns `None` when the current branch is fully shared with another
/// branch (e.g. you're on `main` itself) or when no other refs exist.
pub async fn detect_fork_point(repo_path: &Path) -> Result<Option<String>>;
```
Algorithm (3 subprocesses total, regardless of repo size):
1. `git for-each-ref --format=%(refname) refs/heads refs/remotes` — list all refs.
2. Filter out the current branch's ref and any `*/HEAD` symbolic refs.
3. `git rev-list HEAD --not <other-refs>` — commits reachable from HEAD but not from any other branch. Output is reverse-chronological, so the **last** line is the oldest commit unique to HEAD.
4. `git rev-parse <oldest-unique>^` — the parent of the oldest unique commit is the fork point.
Why this fixes the v1 bug: the result depends only on what's reachable from each ref, never on which ref is a strict ancestor. A `master` that has advanced past your fork still contributes its reachability — your unique commits are still correctly identified, and their predecessor is the fork point.
Edge cases:
- HEAD has no commits unique to any other ref → fork point is HEAD itself; return `Some(HEAD)`.
- No other refs exist → return `None`.
- Detached HEAD → still works; HEAD resolves to a commit.
### 2. `detect_pr_base_branch` — secondary
```rust path=null start=null
// app/src/util/git.rs
/// Returns a branch *name* suitable for `gh pr create --base`. Picks the
/// branch whose tip is closest to (or equals) the detected fork point,
/// preferring the detected main branch on ties.
pub async fn detect_pr_base_branch(repo_path: &Path) -> Result<Option<String>>;
```
Used only by `create_pr` and any UI label that wants to show "Comparing against `<name>`." Implementation can layer on top of `detect_fork_point`: walk the candidate refs, find the one whose tip equals (or has the smallest distance to) the fork point, with the same tiebreakers as v1 (prefer detected main, then local over `origin/*`, then alphabetical).
Isolating the name lookup keeps the diff/log path independent of branch-naming details.
### 3. Switch the four diff/log helpers to use the fork-point SHA
Each helper drops its `parent_branch: Option<&str>` argument (or keeps it for caller-supplied overrides) and replaces internal `detect_parent_branch(...).await?` calls with `detect_fork_point(...).await?`. The range expression changes from `"{base}..{end_ref}"` to `"{fork_sha}..{end_ref}"` — git accepts a SHA on either side of `..`.
- `get_unpushed_commits`: when `@{u}` is unresolvable, fall back to `<fork_sha>..HEAD` instead of `<parent-branch>..HEAD`. Primary `@{u}..HEAD` path is unchanged.
- `get_branch_diff_entries`: `git diff --numstat <fork_sha>..<end_ref>`.
- `get_diff_for_pr`: `git diff <fork_sha>..<end_ref>`, feeds AI.
- `get_branch_commit_messages`: `git log <fork_sha>..HEAD --format=%s`, feeds AI.
No dialog-level plumbing changes — the helpers are the seams.
### 4. `create_pr` calls `detect_pr_base_branch`
```rust path=null start=null
pub async fn create_pr(
    repo_path: &Path,
    title: Option<&str>,
    body: Option<&str>,
) -> Result<PrInfo> {
    let base = detect_pr_base_branch(repo_path).await?;
    let base = base.map(|b| b.strip_prefix("origin/").unwrap_or(&b).to_string());
    // ...existing gh pr create invocation; if Some(base), pass --base <base>...
}
```
If detection returns `None`, omit `--base` and let `gh` infer from repo defaults. Errors propagate to the existing `user_facing_git_error` toast.
### 5. Feature flag gating
All changes live under `FeatureFlag::GitOperationsInCodeReview` (already gating the dialogs); no new flag.
## Risks and mitigations
### A nearby branch shifts the fork-point earlier than the user expects
If an unrelated branch (e.g. `dev-experiments`) happens to point at a commit close to HEAD's history, that commit becomes the fork point. The result is still mathematically correct — those commits really are shared with another ref — but it can be surprising. In practice this is rare; mitigation is to prune stale branches periodically.
### Stacked parent gets amended/rebased
With parent-feature originally at `B` and amended to `B'`, a child forked at `B` will report fork point `A` (the older common ancestor) and include `B` in its "unique" set. This is mathematically correct — `B` is no longer reachable from any other ref, so it genuinely belongs to the child now. Surprising but accurate.
### Stale remote-tracking refs pollute the candidate set
Long-untouched `refs/remotes/*` entries still feed into `rev-list --not`. Effect is conservative (fork point can only get pushed earlier, not later, by adding more refs to the exclusion set). Mitigation: `git fetch --prune` periodically, or filter the candidate set by `committerdate` if profiles show this matters.
### Cost of detection
`detect_fork_point` runs once per dialog open per helper. 3 subprocesses total (`for-each-ref`, `rev-list HEAD --not …`, `rev-parse`), with `rev-list` accelerated by reachability bitmaps where available. Sub-100ms even in large repos. Cheaper than v1's per-candidate `merge-base` calls.
### Backwards compatibility
Fresh-feature-off-main shapes still resolve correctly: HEAD's commits unique to any other ref end at the fork commit; its parent is `main`'s tip at fork time. Today's `main..HEAD` behavior is preserved on the common path.
## Testing and validation
References below are to `specs/APP-4218/PRODUCT.md` success criteria.
### Manual validation
- `feature-a` (pushed), `git checkout -b feature-b feature-a`, 1 new commit, no push: Publish dialog shows 1 commit (SC 1). Create PR shows only those files; confirming runs `gh pr create --base feature-a` and the PR targets `feature-a` (SC 2).
- Fresh branch off main, no upstream: dialog shows commits since the fork from main (SC 3).
- Rebase `feature-b` onto `main`, reopen dialog: commits list reflects the rebased range (SC 4).
- **New regression case:** branch off `master`, then advance `master` past the fork point (`git pull` after a merge), reopen Publish dialog: shows only your commit, not master's intervening commits.
- Commit-and-create-PR on `feature-b`: PR targets `feature-a` (SC 6).
- Change the pane's diff-mode dropdown; reopen dialogs: previews are unchanged (SC 7).
### Unit coverage
- `detect_fork_point` regression tests in `app/src/util/git_tests.rs`:
  - Branch off main, main advances past fork point — fork point still equals the original main tip.
  - Stacked branch where parent is amended — fork point still well-defined and includes orphaned commits in the unique set.
  - On main itself — returns `None`.
  - No other refs — returns `None`.
### Integration / screenshot coverage
None added. The `git_dialog` module ships without an integration harness today (see `specs/APP-4125/TECH.md`).
## Follow-ups
- **Surface the detected fork commit and base branch in the dialog chrome** (a "Based on" row) once we have a way to populate it without visible latency.
- **Per-branch override** for misdetected base branches when a user has a stronger opinion than the heuristic (`branch.<name>.warpBase` in git config or a Warp DB row).
- **Optionally cache the fork-point SHA at branch-creation time** (Warp DB, keyed by `(repo, branch)`), with the topology-based detection above as the fallback. Skips the 3 subprocess calls entirely on the happy path; falls back cleanly for branches Warp didn't create.
- **Use `git merge-base --fork-point`** in `detect_pr_base_branch` once we know the candidate base, to handle the rebased-parent case via reflog. Complementary, not a replacement.
