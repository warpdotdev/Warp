# Tech Spec: Nushell support

**Issue:** [warpdotdev/warp#2038](https://github.com/warpdotdev/warp/issues/2038)
**Product spec:** [`product.md`](product.md)

## Context

Warp's shell integration is keyed by `ShellType` and fans out through shell detection, local shell spawning, bootstrap scripts, command execution, environment-variable serialization, update commands, and UI shell-selection metadata.

Relevant code:

- `crates/warp_terminal/src/shell/mod.rs` — central shell metadata: `ShellType`, executable-name parsing, markdown language spec parsing, history/rcfile locations, command combiners, and alias parsing.
- `app/src/terminal/available_shells.rs` — shell discovery and display names for local shells and Windows executables.
- `app/src/terminal/local_tty/shell.rs` — direct, WSL, and MSYS2 shell starter paths plus shell-specific spawn arguments.
- `app/src/terminal/bootstrap.rs` and `app/assets/bundled/bootstrap/*` — bootstrap asset selection and shell-specific init/body scripts.
- `app/src/terminal/model/session/command_executor/*` and `app/src/terminal/model/terminal_model.rs` — command execution and unsupported-shell handling.
- `app/src/env_vars/mod.rs` — environment-variable initialization/export syntax by shell.
- `app/src/drive/export.rs` and `app/src/drive/index.rs` — Warp Drive environment-variable collection export/copy.
- `app/src/autoupdate/linux.rs` — Linux package-manager update commands that require success-gated control flow before `warp_finish_update`.

Before this change, Nushell could only be represented as the broad POSIX `ShellFamily`, which caused two problems: Warp did not know how to detect/bootstrap `nu`, and paths that converted `ShellFamily::Posix` back to a concrete shell defaulted to Bash syntax.

## Proposed changes

1. Add `ShellType::Nu` as a first-class shell type.
   - Detect `nu`, `-nu`, Unix paths ending in `/nu`, and Windows basenames exactly equal to `nu.exe`.
   - Parse markdown language specs `nu` and `nushell` as Nushell.
   - Keep `ShellFamily::from(ShellType::Nu)` as `Posix` only for legacy family-level behavior, but avoid converting that family back to Bash in Nushell-specific export paths.
   - Add Nushell history and rcfile locations.
   - Parse Nushell aliases from tab-separated bootstrap output.

2. Add local shell discovery and spawn support.
   - Include `nu`/`nu.exe` in shell discovery and display it as "Nushell".
   - Add `/bin/nu` fallback on Unix-like systems.
   - Start direct Nushell sessions with login/execute arguments that run the Warp init script and then enter the interactive shell.
   - Support WSL and MSYS2 spawning when the detected shell basename maps to `ShellType::Nu`.
   - Use basename parsing for WSL detection to avoid substring false positives.

3. Add Nushell bootstrap assets.
   - `nu_init_shell.nu` sends the initial `InitShell` hook and establishes `WARP_SESSION_ID`.
   - `nu_body.nu` installs Nushell functions/hooks for `Preexec`, `Precmd`, `CommandFinished`, `Bootstrapped`, `Clear`, `InputBuffer`, `FinishUpdate`, prompt-mode toggles, PATH append, and initial working directory handling.
   - `nu.nu` includes the body script through the existing bundled asset mechanism.
   - Use rc-file bootstrap for local Nushell where Warp already uses that method for shells that should not receive large bootstrap payloads through the PTY.

4. Add Nushell command-execution behavior.
   - Use Nushell's no-config flag where command executors need isolated command execution.
   - Preserve command escaping rules separately from Bash/Zsh/Fish where needed.
   - Route unsupported local child/subshell flows through the existing unsupported-shell handling instead of pretending Nushell is Bash.

5. Add Nushell environment-variable serialization.
   - Constants serialize as Nushell literals assigned to `$env`.
   - Command-backed values serialize as Nushell command substitutions.
   - Non-identifier environment names use quoted `$env."NAME"` syntax.
   - Warp Drive export and copy paths carry `ShellType` from the active terminal session so Nushell sessions emit Nushell syntax.

6. Add Nushell-safe update command construction.
   - Keep generic `and_combiner()` behavior unchanged for existing shells.
   - Special-case Linux package-manager update commands for `ShellType::Nu` so package-manager commands and `warp_finish_update` run inside one Nushell `try` block.
   - This preserves the success dependency without using unsupported POSIX `&&` syntax or a plain separator that would report completion after a failed update command.

7. Scope remote/subshell follow-up work explicitly.
   - Remote SSH/session warpification and Nushell subshell bootstrap remain unsupported in this first iteration.
   - Future work can add those paths once the local shell contract is stable.

## Testing and validation

Product behavior coverage:

- Behavior 1, 2, and 5: unit tests in `crates/warp_terminal/src/shell/mod_tests.rs` and `app/src/terminal/local_tty/shell_tests.rs` verify `nu`, `-nu`, `/usr/bin/nu`, Windows `nu.exe`, and false positives such as `menu.exe`/`/usr/bin/menu.exe`.
- Behavior 3 and 14: existing shell-discovery tests are extended so Nushell appears with the known shell types without regressing other shells.
- Behavior 4, 6, 7, 8, 9, and 10: bootstrap unit coverage verifies Nushell asset selection and local smoke testing runs the rendered Nushell init/body scripts with `nu`.
- Behavior 11 and 12: `app/src/env_vars/mod.rs` tests verify Nushell initialization/export syntax, command substitution, and quoted environment-variable names; Drive export tests cover the `ShellType` API change.
- Behavior 13: `app/src/autoupdate/linux_test.rs` verifies the Nushell update command gates `warp_finish_update` behind the package-manager command sequence and does not emit POSIX `&&`.
- Behavior 15: unsupported child/subshell paths intentionally keep using existing unsupported-shell behavior for Nushell.

Local verification commands used for this implementation:

```bash
nix develop /home/vitalyr/projects/dev/ai/warp#default -c bash -lc 'cd /home/vitalyr/projects/dev/ai/warp-dev && cargo fmt --all -- --check'
nix develop /home/vitalyr/projects/dev/ai/warp#default -c bash -lc 'cd /home/vitalyr/projects/dev/ai/warp-dev && cargo test -p warp env_vars::tests --lib'
nix develop /home/vitalyr/projects/dev/ai/warp#default -c bash -lc 'cd /home/vitalyr/projects/dev/ai/warp-dev && cargo test -p warp drive::export::tests --lib'
nix develop /home/vitalyr/projects/dev/ai/warp#default -c bash -lc 'cd /home/vitalyr/projects/dev/ai/warp-dev && cargo test -p warp_terminal test_from_name --lib'
nix develop /home/vitalyr/projects/dev/ai/warp#default -c bash -lc 'cd /home/vitalyr/projects/dev/ai/warp-dev && cargo test -p warp_terminal test_nu_parse_aliases --lib'
nix develop /home/vitalyr/projects/dev/ai/warp#default -c bash -lc 'cd /home/vitalyr/projects/dev/ai/warp-dev && cargo test -p warp test_nu_update_command_gates_finish_update_on_success --lib && cargo test -p warp test_nu_update_command_uses_nu_dist_upgrade_handler --lib'
nix develop /home/vitalyr/projects/dev/ai/warp#default -c bash -lc 'cd /home/vitalyr/projects/dev/ai/warp-dev && cargo clippy -p warp -p warp_terminal --all-targets --tests -- -D warnings'
```

Manual smoke validation:

```bash
nu -n --no-std-lib rendered-nu_init_shell.nu
WARP_BOOTSTRAPPED= WARP_SESSION_ID=12345 WARP_INITIAL_WORKING_DIR="$PWD" nu -n --no-std-lib rendered-nu_body.nu
```

Both smoke commands should exit successfully after replacing build placeholders such as `@@USING_CON_PTY_BOOLEAN@@` with a concrete boolean.

## Risks and mitigations

- **Nushell syntax changes over time.** Nushell is still evolving, so the bootstrap sticks to stable, simple constructs where possible and is covered by smoke tests against the local `nu` binary.
- **POSIX-family assumptions.** Nushell remains `ShellFamily::Posix` for legacy escaping/family APIs, but code that needs concrete syntax now carries `ShellType` through the flow.
- **False-positive detection.** Basename checks and regression tests reduce the risk from the short executable name `nu`.
- **Partial first iteration.** Remote and subshell flows are explicit non-goals so unsupported paths fail predictably instead of producing incorrect Bash/POSIX behavior.

## Follow-ups

- Add remote SSH/session warpification support for Nushell.
- Add Nushell subshell bootstrap support.
- Investigate deeper integration with Nushell-native completions, plugins, LSP, or MCP once local shell support is stable.
