# SSH Profiles Panel - Tech Spec
Product spec: `specs/GH2127/product.md`
GitHub issue: https://github.com/warpdotdev/warp/issues/2127

## Context
Warp already has the core pieces needed for a dedicated SSH profile manager, but they are spread across the workspace panel system, settings, terminal command execution, SSH detection, and secure storage.

Relevant current code:

- `app/src/workspace/header_toolbar_item.rs:29` defines `HeaderToolbarItemKind` variants for toolbar items and maps them to labels/icons/availability.
- `app/src/workspace/view.rs:18966` renders configurable panels for toolbar items. Tabs, Tools, and Code Review already have panel branches; other toolbar items return `None`.
- `app/src/workspace/view.rs:19097` already has access to both `SshSettings` and `WarpifySettings` when assembling the settings template context.
- `app/src/workspace/view.rs:14421` exposes `TerminalView::execute_command_or_set_pending`, which is the right command path for a new tab whose shell may not be bootstrapped yet.
- `app/src/terminal/warpify/settings.rs:46` defines `EnableSshWarpification`; `app/src/terminal/warpify/settings.rs:56` defines `UseSshTmuxWrapper`. The profile flow should respect these existing settings rather than introducing a parallel SSH mode.
- `app/src/settings/ssh.rs` currently owns SSH-related settings through `define_settings_group!`; it is the right place for saved profile metadata.
- `app/src/terminal/ssh/util.rs:204` parses interactive SSH commands for Warpify detection. The same module is the natural home for profile command rendering and strict prompt helpers.
- `app/src/terminal/view.rs:10422` detects interactive SSH commands and starts the existing SSH login/Warpify monitoring path.
- `app/src/terminal/view.rs:23991` evaluates whether a completed SSH login should prompt for Warpification.
- Existing modals use `Modal::new(...)` and typed action views, for example `app/src/settings_view/mcp_servers_page.rs:93` and workspace modal construction sites in `app/src/workspace/view.rs`.
- Existing credential-bearing settings use `warpui_extras::secure_storage` rather than TOML-backed settings. SSH profile passwords should follow that pattern.

The proposed feature should add a new profile surface without changing the behavior of Warpify internals, `SshTmuxWrapper`, `SshRemoteServer`, or remote server installation. The implementation should treat profiles as a structured way to launch SSH commands and optionally provide one-shot password input during login.

## Proposed changes

### 1. Profile data model and settings
Add `SshHostProfile` and `SshJumpHost` to `app/src/terminal/ssh/util.rs` or a sibling `profile.rs` under `terminal/ssh`. The model should include:

- `SshHostProfile { id: Uuid, name, host, user, port, identity_file, jump_hosts, tags }`
- `SshJumpHost { host, user, port, identity_file }`

The profile id is immutable after creation and is the key for local password storage. Use serde defaults to migrate profiles created before ids or ports existed:

- missing/nil/duplicate ids get a new UUID
- zero/missing ports normalize to 22
- jump-host ports normalize the same way

Add `saved_ssh_host_profiles: Vec<SshHostProfile>` to `SshSettings` in `app/src/settings/ssh.rs` with:

- `private: true`
- `sync_to_cloud: SyncToCloud::Never`
- a TOML path under `ssh.saved_profiles`

Do not add password fields to the settings model.

### 2. Command rendering
Implement profile command rendering as argv assembly plus shell quoting, not string concatenation. Required helpers:

- `SshHostProfile::to_ssh_command()` for the normal Warpify-aware path
- `SshHostProfile::to_ssh_command_bypassing_warpify()` for the Warpify-disabled path
- `ssh_profile_password_key(id: Uuid) -> String`
- `is_ssh_password_prompt_strict(output: &str) -> bool`

The renderer should:

- include `-i` only when identity file is non-empty
- include `-p` only when the port is not 22
- use `-J` for jump chains that do not need per-jump identity files
- use `ProxyCommand` chaining when any jump host needs an identity file, so per-jump `-i` and `-p` can be honored
- add `--` before the final target to prevent option injection
- quote every argument through the repo's existing shell quoting crate/pattern

### 3. Toolbar and panel integration
Add an SSH Profiles item to `HeaderToolbarItemKind` with a globe/server-style icon and a label of "SSH Profiles". Mark it as a panel item and include it in the default left toolbar only if product/review agrees it should be shown by default; otherwise make it configurable but not default.

In `Workspace`, add:

- `ssh_profiles_panel_view: ViewHandle<SshProfilesPanelView>`
- `ssh_profiles_panel_open: bool`
- `WorkspaceAction::ToggleSshProfilesPanel`
- `WorkspaceAction::ConnectSshProfile(Uuid)`

Update `render_config_panel` so the SSH Profiles branch returns the profile panel without mutating `PaneGroup::left_panel_open` or `right_panel_open`. This keeps the panel independent from Tabs, Tools, and Code Review.

### 4. SSH profiles panel UI
Create `app/src/workspace/view/ssh_profiles_panel.rs` for the panel and modal body. Reuse existing Warp UI primitives:

- `Hoverable`, `Container`, `Flex`, `Stack`, `ConstrainedBox`
- shared `icon_button` and `ActionButton` themes
- `EditorView::single_line` for form fields
- `Dropdown` for jump-host selection
- `Modal::new(...)` with typed events for Add/Edit

Panel behavior:

- header with title and add button
- empty state when no profiles exist
- profile rows styled like existing tab cards
- edit/remove hover controls anchored at the row's upper-right corner
- row click dispatches `ConnectSshProfile`
- edit/remove clicks defer to children and must not trigger row connect

Modal behavior:

- add/edit modes share the same body
- Save disabled while invalid
- Escape emits cancel
- Enter submits only when valid
- password field defaults to masked, supports reveal and explicit clear
- on close, clear password editor contents and reset masking state

Jump-host dropdown behavior:

- candidates come from other saved profiles
- exclude the current profile and already selected targets
- render selected hosts as removable chips
- when saving, snapshot the selected profile's host/user/port/identity metadata into `SshJumpHost`

### 5. Profile save/delete and secure storage
On submit, update `SshSettings::saved_ssh_host_profiles` and separately handle `PasswordIntent`:

- `Keep`: no secure-storage write
- `Set(Zeroizing<String>)`: write to `ssh_profile_password_key(profile.id)`
- `Clear`: remove that key

On profile removal:

- remove the profile metadata
- remove its secure-storage password key, ignoring `NotFound`
- remove matching jump-host references from remaining profiles

Use `Zeroizing<String>` for password values that pass through Rust-owned application memory. Do not log password contents.

### 6. Connect flow and Warpify behavior
`WorkspaceAction::ConnectSshProfile` should:

1. read the profile by id from `SshSettings`
2. inspect `WarpifySettings::enable_ssh_warpification`
3. render `profile.to_ssh_command()` when enabled, otherwise render `profile.to_ssh_command_bypassing_warpify()`
4. create a new tab
5. arm SSH profile state on the new terminal view
6. call `execute_command_or_set_pending(&command, ctx)`

Do not use a direct "run command now" path that can silently fail before bootstrap.

When `enable_ssh_warpification` is false, the bypass command should avoid matching Warp's SSH detection wrappers and should suppress remote bootstrap hooks only for the profile command until that command finishes. This prevents a plain SSH profile connection from displaying as Warpified when the setting is off.

When `enable_ssh_warpification` is true, the profile connection should use the same existing `evaluate_warpify_ssh_host` and SSH login completion flow as manual SSH.

`UseSshTmuxWrapper` should influence only the existing Warpify decision after SSH login; it must not decide whether profiles use the Warpify-enabled or Warpify-disabled command path.

### 7. Password auto-entry state
Add a small `SshAutoInjectState` under `app/src/terminal/ssh/auto_inject.rs` with:

- profile id
- original command string
- target block id
- attempted flag
- login completed flag
- 30-second expiry

Activation:

- arm only for direct profiles with no jump hosts
- match the active block command against the profile command, allowing expected wrapper normalization
- bind to that block id before any password lookup

Polling or output-triggered check:

- inspect only a bounded tail of the active block output
- require `is_ssh_password_prompt_strict`
- set `attempted = true` before secure-storage lookup
- read the password by profile id
- write password + carriage return directly to the PTY path, not through shared-session/user-input broadcasting paths
- immediately disarm after the write

Disarm when:

- the target block changes
- command no longer matches
- login completes
- the state expires
- the first attempt has already happened

This design intentionally does not auto-enter passwords for jump-host profiles in the first version.

### 8. Tests and validation
Map product invariants to tests:

- Command rendering unit tests in `app/src/terminal/ssh/util.rs`:
  - shell quoting with spaces/metacharacters
  - `--` before final target
  - default and non-default port rendering
  - `-J` rendering for simple jump chains
  - `ProxyCommand` rendering for jump chains with per-jump identity files
  - password key includes stable UUID
  - normalization migrates missing/duplicate ids and zero ports
  - strict prompt detection accepts OpenSSH password/passphrase prompts and rejects sudo/generic prompts

- Auto-inject unit tests in `app/src/terminal/ssh/auto_inject.rs`:
  - command matching across wrapper normalization
  - rejects different host/port
  - target block/attempt/login-complete one-shot state transitions

- Terminal model/view tests:
  - Warpify-disabled profile command suppresses remote bootstrap hooks only until command completion
  - Warpify-enabled profile leaves existing SSH Warpify detection available

- UI/view tests:
  - `SshProfilesPanelView` and modal can layout in light/dark theme without panic
  - invalid form disables Save
  - jump-host candidates exclude self and selected profiles
  - removing a profile prunes stale jump references

- Integration tests under `crates/integration`:
  - add a profile, save it, reopen panel, and see it listed
  - edit profile password visibility/clear behavior without leaking to settings
  - click a key-auth profile and verify a new tab queues/runs the expected SSH command
  - with SSH Warpify enabled, profile connection enters the same Warpify prompt/success path as a manual SSH command
  - with SSH Warpify disabled, profile connection remains plain SSH and does not show the Warpified success state
  - jump-host dropdown lists other profiles, excludes self, and renders a command that chains through selected profiles

Manual validation should cover a real SSH host for password auto-entry, a wrong-password retry, a sudo prompt after login, a host-trust `yes/no` prompt, and a jump-host profile with identity-file authentication.

### 9. PR sequencing
This is a feature request and should follow the repository's contribution model:

1. Use issue `#2127` as the tracking issue; do not open a duplicate issue.
2. Ask maintainers to mark `#2127` as `ready-to-spec`.
3. Open a spec PR containing only `specs/GH2127/product.md` and `specs/GH2127/tech.md`.
4. After spec approval, open or update the implementation PR from a separate branch with code, tests, and a changelog entry.

## Risks and mitigations

### Risk: password injection into the wrong prompt
The largest security risk is entering a saved password into a prompt that is not the intended SSH login prompt.

Mitigation: strict prompt matching, target block binding, one-shot attempted flag, login-complete disarm, short TTL, no auto-entry for jump-host chains in v1, and no password writes through user-input/shared-session broadcasting.

### Risk: profile connection ignores Warpify settings
A profile-launched SSH command can accidentally bypass or trigger Warpify differently from manual SSH.

Mitigation: branch only on `enable_ssh_warpification` for command rendering, keep `UseSshTmuxWrapper` inside the existing post-login Warpify decision, and add regression tests for both setting states.

### Risk: jump-host metadata drift
If jump hosts are stored only as raw strings, selected profile metadata such as port or identity file is lost.

Mitigation: snapshot structured jump-host metadata into `SshJumpHost`, and remove stale references when a profile is deleted.

### Risk: large UI surface without integration coverage
The feature spans settings, secure storage, panels, modals, terminal execution, and SSH login state.

Mitigation: keep the implementation split into small modules, add focused unit tests for logic, and add integration coverage for the user-facing add/edit/connect/Warpify flows before the implementation PR is marked ready.

## Follow-ups
- Import and periodically refresh profiles from `~/.ssh/config`.
- Folder/group organization and connect-to-all.
- Search/filter inside the SSH Profiles panel.
- Per-profile environment variables and startup directories.
- Safe multi-hop password handling if Warp can reliably associate prompts with each hop.
- Optional cloud sync for non-secret profile metadata if product/security agree.
