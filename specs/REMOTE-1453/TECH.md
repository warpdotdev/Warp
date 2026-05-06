# REMOTE-1453: Tech Spec — Resolve service-account global skills at agent driver startup
## Context
The product behavior is defined in `specs/REMOTE-1453/PRODUCT.md`. This spec covers how to plug global-skill resolution into the existing agent-driver startup, the GraphQL plumbing for the new `User.globalSkills` field, and how to share the existing repo-cloning code path so we do not introduce a parallel skill registry.
The relevant existing pieces:
- Server: `User.globalSkills: [String!]!` on the GraphQL schema and `logic.GetGlobalSkillSpecsForPrincipal` (warp-server PR #10799).
- Client GraphQL fetch: `crates/graphql/src/api/queries/get_user.rs:107-141` defines `GetUser` and the `User` fragment. `app/src/server/server_api/auth.rs:282-379` runs `fetch_user_properties` / `fetch_user`.
- Client user model: `app/src/auth/user.rs:79-103` (`User` struct), `app/src/auth/auth_state.rs:453-461` (`AuthState::is_service_account`).
- Skill spec parsing and resolution:
  - `crates/warp_cli/src/skill.rs` — `SkillSpec`, `SkillSpec::from_str`, `is_full_path`, `with_org_and_repo`.
  - `app/src/ai/skills/resolve_skill_spec.rs:139-204` — `clone_repo_for_skill(org, repo, working_dir)` clones `https://github.com/<org>/<repo>.git` into `working_dir/<repo>` and is idempotent when the directory already contains a git repo.
  - `app/src/ai/skills/mod.rs:22-27` — re-exports `clone_repo_for_skill`, `resolve_skill_spec`, etc.
- Agent driver startup:
  - `app/src/ai/agent_sdk/mod.rs:540-667` (`AgentDriverRunner::setup_and_run_driver`) drives the run: `refresh_team_metadata` → `refresh_warp_drive` → `build_driver_options_and_task` → `resolve_environment` → `create_and_run_driver`.
  - `app/src/ai/agent_sdk/mod.rs:683-726` (`resolve_skill`) is the existing `--skill` resolution, including the sandboxed-mode `clone_repo_for_skill` call.
  - `app/src/ai/agent_sdk/driver.rs:200-228` (`AgentDriverOptions`) — what the `AgentDriver` is constructed with.
  - `app/src/ai/agent_sdk/driver.rs:1216-1333` — the section of `run_internal` that loads the environment, calls `prepare_environment`, then runs the Oz-only `SkillWatcher::read_skills_for_repos` over `environment.github_repos`.
- Environment preparation:
  - `app/src/ai/cloud_environments/mod.rs:21-39, 85-147` — `GithubRepo { owner, repo }` and `AmbientAgentEnvironment.github_repos`.
  - `app/src/ai/agent_sdk/driver/environment.rs:52-303` — `prepare_environment` loops over `github_repos`, clones each via `git clone --filter=tree:0`, registers them with `DetectedRepositories`, runs setup commands, optionally `cd`s into a single repo. Also includes the `is_sandbox` parameter that gates host-side detection.
- Feature flag: `FeatureFlag::OzPlatformSkills` (`crates/warp_features/src/lib.rs:659`).
## Current state
- `User.globalSkills` is exposed by the server but the client does not request it; the field is invisible to the agent driver today.
- `AgentDriver::run_internal` only clones `environment.github_repos` and only loads skills from those repos.
- `clone_repo_for_skill` is the only existing client-side helper that clones a GitHub repo for skill purposes. It is invoked exactly once today, in `resolve_skill` for sandboxed `--skill <org/repo:...>` runs. It does the right thing for our needs (HTTPS clone, skip-if-already-git, error if non-git directory) but it does not honor the partial-clone optimization used by `prepare_environment`.
## Proposed changes
### 1. Extend the `GetUser` GraphQL query
`crates/graphql/src/api/queries/get_user.rs`
- Add `global_skills: Vec<String>` to the `User` fragment (server already exposes the field as non-null `[String!]!`).
- Update the embedded query string in the `/* query GetUser ... */` doc comment to include `globalSkills` next to `experiments`, `is_onboarded`, etc.
This is a single-line schema-shape change; cynic regenerates types via the existing `./script/codegen` flow.
### 2. Plumb `global_skills` into the auth `User` model
`app/src/auth/user.rs`
- Add `pub global_skills: Vec<String>` to `User` (alongside `principal_type`).
- Default to empty in `User::test()` and any other constructors.
- Update the `From<warp_graphql::queries::get_user::User>`-style conversion in the `auth_state.rs` / `auth/user.rs` boundary so `global_skills` carries through. (The `UserProperties::user` shape is built in `app/src/server/server_api/auth.rs` around line 295-300 and friends; trace and forward the new field there.)
- The agent driver reads the field directly off `User` via `AuthStateProvider::as_ref(ctx).get().user.read().as_ref().map(|u| u.global_skills.clone()).unwrap_or_default()`. No new `AuthState` accessor is added.
### 3. New module: `app/src/ai/skills/global_skills.rs`
A small module that owns parsing skill specs into a deduped list of GitHub repos. Re-export through `app/src/ai/skills/mod.rs`.
```rust path=null start=null
//! Helpers for resolving service-account "global" skill specs into repos to
//! ensure are available on disk before agent runs.

use std::collections::BTreeSet;

use warp_cli::skill::SkillSpec;

use crate::ai::cloud_environments::GithubRepo;

/// Parse a list of raw skill spec strings into the unique set of GitHub repos
/// they reference.
///
/// - Specs that fail to parse are logged at warn level and skipped.
/// - Specs without an `org/repo` qualifier produce no repo (they fall back to
///   on-disk discovery in the existing skill resolver, the same way `--skill`
///   handles unqualified specs).
/// - Duplicates by `(org, repo)` are collapsed.
pub fn resolve_skill_repos(specs: &[String]) -> Vec<GithubRepo> {
    let mut repos: BTreeSet<(String, String)> = BTreeSet::new();
    for raw in specs {
        let spec = match SkillSpec::from_str(raw) {
            Ok(spec) => spec,
            Err(err) => {
                log::warn!("Failed to parse global skill spec '{raw}': {err}");
                continue;
            }
        };
        let (Some(org), Some(repo)) = (spec.org.as_ref(), spec.repo.as_ref()) else {
            continue;
        };
        repos.insert((org.clone(), repo.clone()));
    }
    repos
        .into_iter()
        .map(|(owner, repo)| GithubRepo::new(owner, repo))
        .collect()
}
```
Tests in `global_skills_tests.rs` cover: empty input, parse failures, unqualified / repo-only entries (silently skipped), org/repo entries collected, duplicates collapsed.
No dedup against the environment's `github_repos` lives here. The clone helper introduced in step 4 already short-circuits when the target directory exists, so a global-skill repo that is already covered by the environment is a no-op when we attempt to clone it — no separate filtering function is needed.
### 4. Factor a per-repo clone helper out of `prepare_environment`
`app/src/ai/agent_sdk/driver/environment.rs`
The body of the existing `for repo in github_repos` loop in `prepare_environment_impl` (the `terminal_directory_exists` probe, the `git clone --filter=tree:0` invocation, and the `DetectedRepositories::detect_possible_git_repo` registration when `!is_sandbox`) is extracted into a single helper that the driver can call for any repo set:
```rust path=null start=null
pub(super) async fn ensure_repo_cloned(
    repo: &GithubRepo,
    working_dir: &Path,
    is_sandbox: bool,
    spawner: &ModelSpawner<TerminalDriver>,
) -> Result<(), PrepareEnvironmentError>;
```
Behavior matches the current loop body exactly:
- Probe the session for `working_dir/<repo>`; if the directory already exists, log a skip and return `Ok(())`.
- Otherwise run `git clone --filter=tree:0 https://github.com/<owner>/<repo>.git` via the silent terminal executor; on non-zero exit, return `PrepareEnvironmentError::CloneRepo`.
- When `!is_sandbox`, register the freshly-cloned path with `DetectedRepositories` so the skill watcher and other repo-aware subsystems pick it up.
`prepare_environment_impl` becomes a thin wrapper that calls `ensure_repo_cloned` for each env `GithubRepo` and otherwise keeps its current responsibilities (setup-commands phase, single-repo `cd`, codebase-index waiting). This is a pure refactor — no behavior change for existing env runs. The helper's `pub(super)` visibility lets `AgentDriver::run_internal` call it directly for global-skill repos without going through `prepare_environment`.
### 5. Resolve and clone global-skill repos in the driver
`app/src/ai/agent_sdk/driver.rs` — `AgentDriver::run_internal`
The driver checks the user's `globalSkills` directly during initialization rather than threading anything through `AgentDriverOptions`. After the existing `prepare_environment` step (or in place of it for runs without an environment) the driver runs:
```rust path=null start=null
if FeatureFlag::OzPlatformSkills.is_enabled() {
    let global_specs = foreground
        .spawn(|_, ctx| {
            AuthStateProvider::as_ref(ctx)
                .get()
                .user
                .read()
                .as_ref()
                .map(|u| u.global_skills.clone())
                .unwrap_or_default()
        })
        .await?;
    let global_repos = resolve_skill_repos(&global_specs);
    if !global_repos.is_empty() {
        log::info!("Resolving {} global skill repo(s)", global_repos.len());
        for repo in &global_repos {
            if let Err(err) = foreground
                .spawn({
                    let repo = repo.clone();
                    let working_dir = self.working_dir.clone();
                    move |_, ctx| {
                        terminal_driver(ctx).update(ctx, |_, ctx| {
                            ensure_repo_cloned(
                                &repo,
                                &working_dir,
                                /* is_sandbox */ false,
                                ctx.spawner(),
                            )
                        })
                    }
                })
                .await?
                .await
            {
                log::warn!("Failed to clone global-skill repo {repo}: {err}");
            }
        }
    }
}
```
The exact spawning shape mirrors the existing `prepare_environment` invocation; the important properties are:
- The driver reads `User.global_skills` directly via `AuthStateProvider`. The CLI run-time invariant is that `User` is already populated by `fetch_user_properties` at login / refresh, so no extra async fetch is needed here. There's no client-side check for whether the principal is a service account; the server resolver returns an empty list for non-service-account principals, so non-SA runs naturally take the fast empty path.
- Each repo is cloned best-effort; per-repo failures are logged and the run continues.
- The clone helper's existing "directory exists → skip" check is the dedup mechanism. If a global-skill repo's `(owner, repo)` is also in the environment's `github_repos`, the env-repo clone runs first (during `prepare_environment`) and the global-skill loop's call becomes a no-op. No upfront filter against `env.github_repos` is needed.
After cloning, the existing post-environment skill load (`SkillWatcher::read_skills_for_repos` at driver.rs:1304-1322) extends its repo list with the global-skill repos so the SKILL.md files are picked up. For runs without an environment, this load runs over just the global-skill repos and `SkillManager::set_cloud_environment(true)` is still set when the list is non-empty so the new skills are in scope regardless of `cwd`.
### 6. Reuse `clone_repo_for_skill` for the explicit `--skill` flow only
No change. The existing sandboxed `--skill` clone in `agent_sdk/mod.rs:resolve_skill` continues to call `clone_repo_for_skill` directly. After this spec lands, that path is still useful for the case where `--skill org/repo:...` references a repo that's neither in the env nor in `globalSkills`.
### 7. Feature flag interaction
- `OzPlatformSkills` gates the entire flow (already used to gate `--skill` resolution). When disabled, the driver's global-skill block is skipped entirely and behavior is identical to today.
- No new feature flag is introduced.
### 8. Telemetry / logs
Log lines:
- Info: "Resolving N global skill repo(s)" when `global_repos` is non-empty.
- Warn: parse failures, clone failures (per-repo).
No new telemetry events for the first cut.
## Parallelization
Not applicable. This is a small, single-developer change that lands in one PR.
## Risks and mitigations
- **Stale `globalSkills` for long-running clients**: `User.globalSkills` is read from the cached `User` populated at login. If a service account's skills are updated after login but before a new run, the run won't see the change. Mitigated by service-account credentials always being short-lived (API key login refetches user) and by being explicit in the spec that this is read-once-per-run.
- **Repo-name conflicts**: If `org-a/foo` is in the env and `org-b/foo` is in `globalSkills`, the global-skill clone would otherwise try to land at `working_dir/foo` which is taken. The existing `dir_exists` guard inside `ensure_repo_cloned` already skips the clone. Detection of org mismatch is the existing `--skill` resolver's responsibility (it validates via git remote when an org filter is present); for global skills we tolerate the conflict and let resolution downstream pick the on-disk repo.
- **Sandbox host-detection skip**: the `is_sandbox` path inside `ensure_repo_cloned` already skips host-side `DetectedRepositories::detect_possible_git_repo` for sandbox-only paths. Global-skill repos cloned in a sandbox follow the same skip and are detected by the in-sandbox skill scanner.
## Testing and validation
Unit coverage:
- `resolve_skill_repos` with: empty input, parse failures, unqualified / repo-only entries, org/repo entries, duplicates collapsed.
- A test for `User` serialization round-trip covering the new `global_skills` field default-empty case.
Driver-level coverage (using the existing `agent_sdk` test surfaces under `app/src/ai/agent_sdk/test_support`):
- The driver clones the resolved global-skill repos when `User.global_skills` is non-empty, and the loop is a no-op when the list is empty (which is the server-side default for non-service-account principals).
- The clone helper is a no-op when the target directory already exists (covers the env-overlap case).
Manual / integration:
- Configure a test service account with `globalSkills = ["warpdotdev/warp-internal:read-google-doc"]`. Run `oz agent run -p "..."` (sandboxed cloud run) using that SA's API key. Verify the run logs the resolved global skill list, that `warp-internal` is cloned in the sandbox, and that the skill is listed by `oz agent list`.
- Configure the same SA with no environment. Verify the global-skill repo still clones into `working_dir` and the skill is discoverable. Repeat with the env including `warpdotdev/warp-internal` and confirm the global-skills code path becomes a no-op (no extra clone).
- Configure `globalSkills = ["bare-name", "warp-internal:foo", "warpdotdev/warp-internal:bar"]`. Verify the first two are silently skipped (not cloneable; they fall back to on-disk discovery), only `warpdotdev/warp-internal` is cloned, and `bar` resolves once cloned.
- Verify maps onto Behavior #11: simulate a clone failure (point at a non-existent repo), confirm a warn log is emitted and the run still completes.
- Re-run all `oz agent run --skill org/repo:foo` scenarios with `OzPlatformSkills` enabled to confirm no regression in the existing `--skill` flow.
- Disable `OzPlatformSkills` and confirm the new flow is fully bypassed.
## Follow-ups
- Refresh `User.globalSkills` periodically (e.g. on a long-running cloud run) so changes propagate without re-fetching the user.
- Allow user (non-service-account) principals to configure `globalSkills` once we have a UX for it.
- Consider moving the cloned repos into a Warp-managed cache directory so repeated runs reuse them across `working_dir`s; today repeated runs in fresh sandboxes re-clone every time.
- Surface global-skill provenance in `oz agent list --skill ...` output ("source: global skill").
