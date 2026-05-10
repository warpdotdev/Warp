# Handoff Environment Selection: PWD-Based Overlap at Activation — Tech Spec
Product spec: `specs/APP-4427/PRODUCT.md`
Linear: [APP-4427](https://linear.app/warpdotdev/issue/APP-4427)

## Context
Currently, environment auto-selection for `&` handoff has two layers:
- **Layer 1** (activation time): `EnvironmentSelector::ensure_default_selection` in `environment_selector.rs:372` picks from saved setting or MRU when `HandoffComposeState` activates.
- **Layer 2** (post-dispatch): After pressing Enter, an async task in `workspace/view.rs (13417-13469)` derives touched repos from the full conversation, then calls `pick_handoff_overlap_env` at `workspace/view.rs:13440` to overwrite the environment — unless the user made an explicit selection.

Layer 2 causes a visible environment shift after the handoff pane opens. We want to move the overlap logic into layer 1 (at `&` activation time, scoped to just the pwd's repo) and remove layer 2's environment overwrite entirely.

Key files:
- `app/src/terminal/input/handoff_compose.rs` — `HandoffComposeState` model
- `app/src/terminal/input.rs:3731` — `activate_cloud_handoff_compose`
- `app/src/terminal/input.rs:14258` — `active_session_path_if_local` (gives pwd)
- `app/src/ai/blocklist/agent_view/agent_input_footer/environment_selector.rs:372` — `ensure_default_selection`
- `app/src/ai/blocklist/handoff/touched_repos.rs:200` — `pick_handoff_overlap_env`
- `app/src/ai/blocklist/handoff/touched_repos.rs:121-164` — `find_git_root`, `git_origin_url`, `parse_github_repo`
- `app/src/workspace/view.rs:13437-13444` — post-dispatch overlap overwrite (to remove)
- `app/src/terminal/input/slash_commands/mod.rs:898-926` — `/handoff` slash command handler

## Proposed changes
### 1. Add pwd-based overlap selection to `HandoffComposeState` activation
In `activate_cloud_handoff_compose` (`input.rs:3731`), after activating the state, grab the pwd via `active_session_path_if_local` and spawn a lightweight async task that:
1. Calls `find_git_root(pwd)` → `git_origin_url(root)` → `parse_github_repo(url)` to get the pwd's `GithubRepo`
2. Gets all environments and calls `pick_handoff_overlap_env` with a single-repo `TouchedWorkspace`
3. On completion, calls `handoff_compose_state.set_environment_id(overlap_env, false, ctx)` — non-explicit, so it doesn't prevent user override

This uses the same utility functions already in `touched_repos.rs`. We'll add a small helper (e.g. `resolve_pwd_repo`) that wraps the `find_git_root` → `git_origin_url` → `parse_github_repo` chain for a single path, returning `Option<(PathBuf, GithubRepo)>`.

The `ensure_default_selection` in `EnvironmentSelector` still runs synchronously and picks the saved/MRU default. The async pwd overlap result then overwrites it (if found) once the git command returns. If the user has already explicitly selected an environment before the async result arrives, `set_environment_id` with `is_explicit: false` will not overwrite it because `HandoffComposeState::set_environment_id` respects `has_explicit_environment_selection`.

### 2. Apply the same logic for `/handoff query`
`/handoff query` dispatches `OpenLocalToCloudHandoffPane` synchronously, but `complete_local_to_cloud_handoff_open` already runs an async task for snapshot upload and touched-workspace derivation (`workspace/view.rs:13417-13469`). Replace the conversation-wide `pick_handoff_overlap_env` call in that async task's completion handler with a pwd-scoped overlap check using the same `resolve_pwd_repo` helper from §1. This piggybacks on the existing async work — no additional latency, no new async task, and the environment is set before auto-submit fires (since auto-submit already waits for touched-workspace derivation to complete).

### 3. Remove conversation-wide overlap overwrite
In `complete_local_to_cloud_handoff_open` (`workspace/view.rs:13432-13469`), replace the block at lines 13437-13444 that calls `pick_handoff_overlap_env` with the full conversation's `TouchedWorkspace`. Instead, call `resolve_pwd_repo` on the source terminal's pwd (captured before the async task is spawned) and use its result for the single-repo overlap check. The touched-workspace derivation and snapshot upload continue unchanged — only the environment selection source changes from conversation-wide repos to the pwd repo.

## Testing and validation
### Unit tests
- `touched_repos_tests.rs`: Add a test for the new `resolve_pwd_repo` helper — given a path inside a git repo with a GitHub origin, returns the correct `GithubRepo`; given a path outside any repo, returns `None`.
- `handoff_compose_tests.rs`: Verify that `set_environment_id(_, false, _)` does not overwrite when `has_explicit_environment_selection` is true.
- `input_test.rs`: Verify that `activate_cloud_handoff_compose` triggers the async pwd overlap check (mock the git command to return a known repo, assert the environment is updated).

### Manual
- In a terminal whose pwd is inside a repo that matches exactly one environment: type `&`, observe the matching environment selected in the chip.
- Same setup but explicitly pick a different environment from the dropdown, press Enter — the explicit choice is preserved.
- In a directory outside any git repo: type `&`, observe fallback to saved/MRU default.
- `/handoff query` from a repo-matching directory: observe the correct environment is used.
- Verify no environment shift after pressing Enter in any of the above cases.

## Parallelization
This is a small, tightly coupled change across input activation, environment selector, and workspace handoff paths. A single agent should implement it sequentially.
