# REMOTE-1453: Resolve service-account global skills at agent driver startup
## Summary
Service accounts (a.k.a. agents in the product UI) can have a configured set of "global skill specs" that should be available in every conversation on every device. The server already exposes those specs on `User.globalSkills` (warp-server PR #10799). This spec defines what the client guarantees about those skills at agent run time: namely that, before the agent's first query runs, the SKILL.md files referenced by `globalSkills` are reachable on the agent's filesystem and discoverable by the existing skill machinery.
## Problem
Today, an `oz agent run` (local CLI run, sandboxed cloud run, scheduled / triggered run, etc.) only sees skills that:
- Live in directories already present on disk (`.agents/skills/`, `.warp/skills/`, `.claude/skills/`, `.codex/skills/`).
- Are explicitly requested via `--skill <SPEC>`. For sandboxed runs, an `org/repo:skill` spec triggers a one-shot clone of `org/repo` into the working directory before resolution.
There is no mechanism for "this agent always has these skills available" — every run that wants them has to repeat the spec on the command line, and any skill whose source repo is not in the run's environment is silently unavailable.
## Goals
- For service-account-authenticated agent runs, ensure that every skill referenced by `User.globalSkills` is discoverable before the agent processes its first query.
- Reuse the existing skill discovery, parsing, and `--skill` resolution paths — no parallel skill registry.
- Reuse the existing repo-cloning flow used for cloud-environment repos — no Warp-managed cache directory.
- Keep behavior unchanged for user (non-service-account) principals: `globalSkills` is empty for them today, so the run looks identical to before this spec.
## Non-goals
- A general plugin / skill registry mechanism beyond this one-time-per-run resolution.
- Cloning skill repos into a Warp-managed cache or data directory, or periodically syncing them outside an agent run.
- Supporting `globalSkills` for individual users (not service accounts).
- Changing how `--skill` is resolved at the CLI layer, except to share the same underlying repo-cloning helpers.
- Periodic or background refresh of the global skill list during a single run; for now, the list is read once at startup.
## Behavior
1. When an agent driver starts a run (any of `oz agent run`, scheduled / triggered runs, cloud runs that re-enter the agent driver inside the sandbox) and the principal is a service account, the client reads `User.globalSkills` from the server. The list is a flat `Vec<String>` of skill specs in the same format accepted by `--skill`: `skill_name`, `repo:skill_name`, `org/repo:skill_name`, or full-path variants.
2. For non-service-account principals, the server returns an empty list and the client treats `globalSkills` as empty. No additional work runs and behavior is identical to today.
3. If `OzPlatformSkills` is disabled, the client treats `globalSkills` as empty regardless of what the server returns. The feature is gated by the existing platform-skills flag rather than introducing a new one.
4. Each spec in `globalSkills` is parsed using the existing `SkillSpec` parser. Parse failures are logged at warn level and the offending entry is skipped; the rest of the list still resolves.
5. The client classifies each parsed spec into one of two buckets:
   - **Cloneable**: `org/repo:skill` (org-qualified). The repo `org/repo` can be cloned from `https://github.com/org/repo.git` if it is not already present.
   - **Non-cloneable**: bare `skill_name`, bare `repo:skill_name`, or full-path variants without an org. The client cannot deterministically locate the source repo for these entries, so they are not cloned. They are still passed through to the same resolution path as `--skill` and may resolve if the matching skill is present on disk (e.g. an environment repo already includes it, the user's home skills directory contains it). This matches how `--skill` handles unqualified / repo-only specs today: no implicit clone, fall back to on-disk discovery.
6. For each cloneable spec, the client computes `(org, repo)` and dedupes the set of repos to clone (multiple skills in the same repo result in one clone).
7. For each `(org, repo)` pair from step 6:
   - If a repo with the same `(owner, repo)` is already in the run's cloud environment (`AmbientAgentEnvironment.github_repos`), no extra action is taken; the existing environment-prep flow will clone it. The skill becomes discoverable when the existing post-environment skill load runs.
   - If a directory at `working_dir/<repo>` already exists and contains a `.git` subdirectory, it is treated as already cloned and reused. No re-clone, no error.
   - If a directory at `working_dir/<repo>` exists and is not a git repo, the run logs a warning and skips this repo. Other global-skill repos still clone. The agent run does not fail.
   - Otherwise, the client clones `https://github.com/<org>/<repo>.git` into `working_dir/<repo>` using the same shallow / partial clone semantics already used for environment repos.
8. Cloning of global-skill repos happens as part of the same repo-cloning step that clones environment repos. From a user's perspective, the setup-commands phase shows one combined cloning step rather than two distinct phases.
9. After cloning completes, the existing skill scanner picks up SKILL.md files from the newly cloned repos. Specifically, the cloud-environment skill load (`SkillManager::set_cloud_environment(true)` + `read_skills_for_repos`) runs over the union of the environment's `github_repos` and the global-skill repos.
10. Once skills are loaded, any subsequent skill resolution in the run — `--skill` resolution, slash-command lookups, agent skill listings — sees global skills exactly the same way it sees environment-cloned skills. There is no special "global" badge or precedence; ordering follows the existing skill-directory precedence rules.
11. Failure modes:
    - Network failure cloning a global-skill repo: log a warn-level message naming the repo and continue with the rest of the run. The skill is unavailable for this run; the agent's prompt and other skills still execute. (This is a deliberate departure from the existing `--skill` clone path, which surfaces clone failures as run-fatal errors. Global skills are best-effort: a single broken entry should not block the run.)
    - Failure fetching `User.globalSkills` from the server: log a warn-level message and continue with `globalSkills` treated as empty. The run does not fail.
    - The same repo appears in `globalSkills` and as an explicit `--skill org/repo:...` argument: the existing `--skill` resolver still wins for the explicit selection; the global-skills clone is a no-op since the repo is already present.
12. Replays and historical runs: this spec only changes behavior for newly starting agent driver runs. Historical conversations are unaffected; their stored transcripts already reflect whatever skills were available at the time they ran.
13. Local (non-sandboxed) developer runs as a service account: the same flow applies. Cloned global-skill repos land in `working_dir`, the same place existing `--skill` clones land in sandboxed runs.
## Edge cases
1. `globalSkills` contains a duplicate spec (same `org/repo:skill` listed twice). Dedup is by `(org, repo)` for cloning; resolution downstream handles duplicate skill names via existing precedence rules.
2. `globalSkills` contains two specs that point to different repos but the same `repo` name (e.g. `org-a/foo:x` and `org-b/foo:y`). The first wins for the `working_dir/foo` slot; the second logs a warn-level conflict message and skips its clone. This matches today's `clone_repo_for_skill` behavior, which only ever clones one repo per `repo` directory name. (The existing `--skill` resolver already validates org via the cloned repo's git remote when needed.)
3. The agent run uses an environment whose `github_repos` already covers every cloneable global skill. The client performs no extra clones and the run looks identical to a normal environment-only run.
4. The agent run has no environment configured (`AgentDriverOptions.environment == None`). Global-skill repos still clone into `working_dir`. The cloud-environment skill load path is invoked with the global-skill repos as its repo set so the skills are picked up.
5. The agent run is configured for a non-Oz harness (Claude / Gemini / etc.). Global-skill cloning still occurs (the harness shouldn't influence whether the repo can be made available), but the existing post-environment skill load is Oz-only and remains so. Non-Oz harnesses pick up SKILL.md files via their own discovery (e.g. claude reading `.claude/skills/`) once the repo is cloned, which is the same behavior as for environment repos today.
6. `working_dir` does not exist (misconfigured run). The existing working-directory check in the agent driver fails first; this spec adds nothing to that path.
7. The user is logged out / has no credentials at run start. Fetch fails; per (11) we log and continue with empty `globalSkills`.
