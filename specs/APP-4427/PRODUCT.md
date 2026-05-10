# Handoff Environment Selection: PWD-Based Overlap at Activation
Linear: [APP-4427](https://linear.app/warpdotdev/issue/APP-4427)

## Summary
When a user enters `&` handoff compose mode or uses `/handoff`, auto-select the cloud environment that matches the repo they're currently working in — at activation time, not after pressing Enter. This replaces the current two-layer system where the environment visibly shifts after the handoff is dispatched.

Figma: none provided

## Behavior

1. When the user types `&` or runs `/handoff` (no query), the system checks the terminal's current working directory for a git repo (walk up to `.git`, read `origin` remote URL, parse `<owner>/<repo>`).

2. If a git repo is found and at least one cloud environment's `github_repos` contains that repo, select the environment with the most overlap (breaking ties by most-recently-used). This is a non-explicit selection — the user can still override it from the environment dropdown.

3. If no git repo is found, or no environment matches the repo, fall back to the existing default: saved `last_selected_environment_id` setting, then most-recently-used environment.

4. The environment chip in the footer reflects the selected environment immediately (or near-immediately after the async git check completes). There is no environment shift after pressing Enter.

5. For `/handoff query` (auto-submit path), the same pwd-based overlap check runs before dispatching. The result is passed as a non-explicit environment selection alongside the launch payload.

6. If the user explicitly selects an environment from the dropdown during `&` compose, that explicit selection takes priority over the pwd-based result and is preserved through the handoff.

7. The post-dispatch async overlap check (layer 2) that previously ran after pressing Enter is removed. The `pick_handoff_overlap_env` call in `complete_local_to_cloud_handoff_open` no longer overwrites the selected environment after the handoff pane opens.

8. The pwd-based check uses the same `find_git_root` / `git_origin_url` / `parse_github_repo` / `pick_handoff_overlap_env` utilities as the existing touched-workspace pipeline — just scoped to a single directory instead of the full conversation history.

9. The async git check should complete quickly (single local git command with the existing 5-second timeout). If the check hasn't completed when the user presses Enter, the current selection (from the fallback defaults) is used.
