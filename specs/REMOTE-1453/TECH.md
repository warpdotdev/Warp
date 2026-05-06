# REMOTE-1453: Tech Spec - Resolve service-account global skills at agent driver startup
## Context
The product behavior is defined in `specs/REMOTE-1453/PRODUCT.md`. This spec covers the client-side runtime implementation for resolving service-account `User.globalSkills` during Oz agent startup.
Relevant existing pieces:
- `crates/warp_cli/src/skill.rs`: `SkillSpec`, `SkillSpec::from_str`, `SkillSpec::is_full_path`.
- `app/src/ai/skills/file_watchers/skill_watcher.rs`: scans repository trees for `SKILL.md` files.
- `app/src/ai/skills/skill_manager.rs`: stores the loaded skills for the active run.
- `app/src/ai/agent_sdk/driver/environment.rs`: environment repository clone helper used by environment prep.
- `app/src/ai/cloud_environments/mod.rs`: `GithubRepo { owner, repo }` and `AmbientAgentEnvironment.github_repos`.
- `app/src/ai/agent_sdk/driver.rs`: agent startup sequence, environment prep, and Oz-only repo skill loading.
## Implemented approach
### 1. Global skill parsing helpers
`app/src/ai/skills/global_skills.rs` owns the reusable parsing and filtering helpers and is re-exported through `app/src/ai/skills/mod.rs`.
- `resolve_skill_repos(raw_specs: &[String]) -> (Vec<SkillSpec>, Vec<GithubRepo>)` parses raw server strings with the existing CLI parser, logs warn-level parse failures, skips invalid entries, and extracts org-qualified `(org, repo)` pairs from the parsed specs while preserving first-seen order.
- `filter_explicit_global_skills(repo_path, skills, specs)` takes the full scan result for one global-only repo and returns only the parsed skills explicitly requested for that repo.
Filtering behavior:
- Full-path specs match `repo_path.join(spec.skill_identifier)` exactly.
- Simple-name specs match parsed `ParsedSkill::name` values and pick the first match by provider-directory precedence.
- Duplicate selected paths are collapsed.
### 2. Driver source classification
`app/src/ai/agent_sdk/driver.rs` separates global skill resolution from repository skill loading.
- `GlobalSkillResolution` carries parsed global specs and cloneable global skill repos.
- `SkillRepoLoadRequest` carries one repo plus a `SkillRepoLoadMode`, so a repo either loads all discovered skills or filters to explicit global specs without representing contradictory states.
- `skill_repo_load_requests(environment_repos, global_skill_repos, global_skill_specs)` builds the load requests:
  - Environment repos use `SkillRepoLoadMode::All`.
  - Global repos that are not also environment repos use `SkillRepoLoadMode::ExplicitGlobal` with only specs matching that repo.
  - Global repos already present in the environment are skipped as separate global requests, so the environment request wins.
Repository identity is `(owner, repo)` via derived `GithubRepo` equality.
### 3. Driver startup sequence
For Oz harnesses, `AgentDriver::run_internal` now does the following after the session is shared and before environment prep:
1. Read cached `AuthStateProvider::get().global_skills()`.
2. Parse specs and resolve cloneable repos with `resolve_skill_repos`.
3. Clone those repos best-effort with `environment::ensure_repo_cloned`.
4. Prepare the configured environment, if any, preserving the existing file-based MCP discovery and setup-command sequencing.
5. Build `SkillRepoLoadRequest`s from environment repos and global skill repos.
6. Load skills synchronously before the initial Oz prompt is sent.
Global-skill cloning currently remains Oz-only because the skill loading path is Oz-only and third-party harnesses have separate skill systems.
### 4. Repo skill loading
`load_skills_from_repos` now accepts `Vec<SkillRepoLoadRequest>` instead of raw `Vec<GithubRepo>`.
- It still waits for repository metadata indexing before scanning.
- It scans each repo with `SkillWatcher::read_skills_for_repos`.
- For `SkillRepoLoadMode::All` requests, it appends all scanned skills.
- For `SkillRepoLoadMode::ExplicitGlobal` requests, it applies `filter_explicit_global_skills` before appending. Filtering happens after the scan because the required repository metadata tree has already been built for the whole repo.
- It then sets `SkillManager::set_cloud_environment(true)` and adds the final selected skill list, preserving the existing in-scope behavior for repo-loaded skills.
## Behavior guarantees
- Environment repos continue to autodiscover all skills.
- Global-only repos do not expose unrelated skills from the same repo.
- Environment/global overlap is resolved in favor of environment autodiscovery.
- Clone failures for global-skill repos are warn-and-continue.
- Invalid global skill specs are warn-and-skip.
- Non-cloneable specs do not trigger clone attempts.
## Testing and validation
Unit coverage added or expected:
- `resolve_skill_repos` skips invalid specs and handles empty input, unqualified specs, org-qualified specs, and duplicate repos.
- `filter_explicit_global_skills` loads only requested simple-name skills, respects provider precedence, and matches full-path specs.
- `AgentDriver::skill_repo_load_requests` verifies that environment repos use the all-skills mode, global-only repos keep matching specs, and environment/global overlap is not emitted as a separate global-only request.
Validation commands:
- `cargo fmt`
- Targeted `cargo nextest` for the `global_skills` and `driver` tests touched by this change.
- A targeted `cargo check` or package build if test compilation exposes broader type issues.
## Follow-ups
- Add integration coverage around a real service-account run once the test harness can inject `User.globalSkills` and cloned repo contents cheaply.
- Consider refreshing `User.globalSkills` at run start if cached user state can become stale for long-lived clients.
- Consider global-skill provenance in skill listing UI or logs if users need to debug why a skill was available.
- Revisit third-party harness behavior if those harnesses adopt the same runtime skill loading path.
