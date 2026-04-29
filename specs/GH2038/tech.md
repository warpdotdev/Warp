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
   - For direct local sessions, invoke Nushell as `nu --login --execute <warp init script>`. The `--login` flag preserves normal login-shell startup, and `--execute` runs Warp's init snippet before leaving the process in an interactive shell.
   - Startup ordering is part of the contract: Nushell loads the user's normal startup files first (`env.nu`, then `config.nu`, and `login.nu` for login shells), then Warp's `--execute` init script runs. Warp does not skip user config for the main interactive session.
   - If a user startup file throws an error before `--execute` runs, Nushell reports that error and Warp treats the shell like any other shell that failed to bootstrap; Warp does not mask or rewrite user config failures.
   - Support WSL and MSYS2 spawning when the detected shell basename maps to `ShellType::Nu`.
   - Use basename parsing for WSL detection to avoid substring false positives.

3. Add Nushell bootstrap assets.
   - `nu_init_shell.nu` sends the initial `InitShell` hook and establishes `WARP_SESSION_ID`.
   - `nu_body.nu` installs Nushell functions/hooks for `Preexec`, `Precmd`, `CommandFinished`, `Bootstrapped`, `Clear`, `InputBuffer`, `FinishUpdate`, prompt-mode toggles, PATH append, and initial working directory handling.
   - `nu.nu` includes the body script through the existing bundled asset mechanism.
   - Bootstrap must merge with user configuration rather than replacing it: prepend Warp's `pre_execution` and `pre_prompt` hooks ahead of existing hook lists, append Warp keybindings to existing keybindings, and store the user's original prompt closures/strings before changing prompt indicators.
   - Warp-managed prompt mode sets Nushell prompt indicators to empty strings and emits Warp prompt escape sequences. Honor-user-prompt mode restores the stored user prompt indicators and calls the stored `PROMPT_COMMAND`/`PROMPT_COMMAND_RIGHT` values when they are closures or strings.
   - Disable Nushell's built-in OSC 133/633 shell integration flags in `$env.config.shell_integration` while Warp's integration is active to avoid duplicate prompt markers.
   - Use rc-file bootstrap for local Nushell where Warp already uses that method for shells that should not receive large bootstrap payloads through the PTY.

4. Add Nushell command-execution behavior.
   - Use Nushell's `--no-config-file` flag where command executors need isolated command execution that should not re-run user config.
   - Preserve command escaping rules separately from Bash/Zsh/Fish where needed.
   - Route unsupported local child/subshell flows through the existing unsupported-shell handling instead of pretending Nushell is Bash.

5. Add Nushell environment-variable serialization.
   - Constants serialize as JSON string literals assigned to `$env`, using `serde_json::to_string`. This makes quotes, backslashes, newlines, `$()`, semicolons, and other shell-significant characters data rather than executable Nushell syntax.
   - Bare environment names are allowed only when they match `[A-Za-z_][A-Za-z0-9_]*`. Any other name serializes through the same JSON string-literal escaping and is emitted as `$env."NAME"`/`$env.<quoted-name>`.
   - Command-backed values are intentionally executable Nushell expressions emitted as `(<command>)`. These commands come from Warp's environment-variable command model and are not escaped as data; callers must treat them as trusted command snippets.
   - Secret-backed values are intentionally executable command substitutions around the external secret-manager retrieval command. The secret reference/manager configuration is the trust boundary, and the command output becomes the environment value.
   - Warp Drive export and copy paths carry `ShellType` from the active terminal session so Nushell sessions emit Nushell syntax.

6. Add Nushell-safe update command construction.
   - Keep generic `and_combiner()` behavior unchanged for existing shells.
   - Special-case Linux package-manager update commands for `ShellType::Nu` so package-manager commands and `warp_finish_update` run inside one outer Nushell block with the exact shape `try { <package-manager sequence>; warp_finish_update <update_id> } catch { print $in }`.
   - The contract depends on Nushell `0.109.x` behavior: a non-zero external command exit inside `try` raises an error, jumps to `catch`, and skips the remaining statements in that `try` block. This is the success gate that prevents `warp_finish_update` from running after a failed package-manager command.
   - Apt sequence: optionally run `try { warp_handle_dist_upgrade <repo> } catch { null }` first. That inner failure is intentionally ignored to preserve the existing dist-upgrade best-effort behavior. Then run `sudo apt update; sudo apt install <package>`. Failure in either sudo command aborts the outer `try` before finish.
   - Yum/Dnf/Zypper sequence: run the single package-manager upgrade/update command inside the outer `try`; any non-zero exit aborts before finish.
   - Pacman sequence: optional key setup commands, optional repository backup/config commands, and `sudo pacman -Sy <package>` all run as ordered statements inside the outer `try`; any non-zero exit in setup or install aborts before finish. Commands that must bypass Nushell built-ins use the external-command caret form such as `^mkdir` and `^cp`.

7. Scope remote/subshell follow-up work explicitly.
   - Remote SSH/session warpification and Nushell subshell bootstrap remain unsupported in this first iteration.
   - Future work can add those paths once the local shell contract is stable.

## Nushell metadata collection contract

Behavior 8 is satisfied by the `Bootstrapped` hook payload sent from `nu_body.nu`. The contract is concrete and testable:

| Payload field | Nushell source command/expression | Format sent to Warp | Test expectation |
| --- | --- | --- | --- |
| `shell` | literal `"nu"` | string | equals `nu` |
| `shell_version` | `version | get version` | string | parses as the running `nu` version |
| `histfile` | `$env.config.history.file_format?` and `$nu.history-path` | string path only when history format is `plaintext`; otherwise empty string | plaintext config reports the history path; non-plaintext config reports empty |
| `home_dir` | `$nu.home-path?` with `$env.HOME?` fallback | string path | non-empty when Nushell exposes a home path |
| `path` | `$env.PATH` normalized by `warp_path_string` | platform path string joined with `(char esep)` when `$env.PATH` is a list; raw string otherwise | list and string PATH inputs both produce a path string |
| `editor` | `$env.EDITOR? | default ""` | string | reflects EDITOR or empty |
| `aliases` | `scope aliases | each {|alias| $"($alias.name)\t($alias.expansion? | default "")" } | str join (char nl)` | newline-separated rows; each row is tab-separated `name<TAB>expansion` | parser accepts tab rows and preserves expansion text |
| `function_names` | `scope commands | where type == "custom" | get name | uniq | str join (char nl)` | newline-separated names | custom commands appear once |
| `builtins` | `scope commands | where type == "built-in" | get name | uniq | str join (char nl)` | newline-separated names | known built-ins appear |
| `keywords` | `scope commands | where type == "keyword" | get name | uniq | str join (char nl)` | newline-separated names | known keywords appear |
| `env_var_names` | `$env | columns | str join (char nl)` | newline-separated names | includes test env vars without values |
| `os_category` | `$nu.os-info.name?` mapped to `MacOS`, `Linux`, `Windows`, or empty | string enum | matches host category or empty for unknown |
| `linux_distribution` | parsed `NAME=` from `/etc/os-release` or `/usr/lib/os-release` on Linux | string | non-empty on standard Linux images when os-release exists |
| `wsl_name` | `$env.WSL_DISTRO_NAME? | default ""` | string | reflects WSL env var or empty |
| `shell_path` | `$nu.current-exe` | string path | equals the running Nushell executable path |
| `vi_mode_enabled` | `$env.config.edit_mode? == "vi"` | `"1"` or empty string | vi config maps to `"1"`; other modes map empty |

Automated metadata tests should run a rendered bootstrap script under a controlled Nushell environment that defines at least one alias, one custom command, one test environment variable, and deterministic PATH/EDITOR values. The test should capture the `Bootstrapped` JSON payload, decode it, and assert the field formats above.

## Compatibility and reproducible Nushell provisioning

The minimum supported Nushell version is `0.109.0`. The bootstrap and tests rely on the following Nushell behavior available in that line and newer versions:

- `nu --login --execute <commands>` starts a login shell, runs the command string after user startup files, and remains interactive.
- `$env.config.hooks.pre_execution`, `$env.config.hooks.pre_prompt`, prompt closures, and keybinding records can be updated with `upsert`.
- `scope commands`, `scope aliases`, `to json -r`, `encode hex`, `hide-env`, and `commandline` host commands are available with the syntax used by the bootstrap.
- External command failures inside `try { ... } catch { ... }` jump to `catch` and prevent later statements in that `try` block from running; the updater success-gating tests rely on this behavior.

Older Nushell versions are best-effort. If an older installed `nu` lacks required flags, hooks, or syntax, Warp should surface the normal shell bootstrap/startup failure rather than falling back to Bash syntax. Future compatibility expansion should add version-specific smoke tests before lowering the minimum.

For reproducible CI/reviewer validation, tests must not rely on an author-local `nu` from a user profile. Provision Nushell through a pinned Nix input, for example `nix shell github:NixOS/nixpkgs/<repo-flake-lock-rev>#nushell -c nu --version`, or add Nushell to the repo's CI/devshell package set and assert `version >= 0.109.0` before running smoke tests. If a non-Nix environment lacks `nu`, Nushell smoke/interactive tests should skip with an explicit "Nushell binary not available" reason rather than silently passing.

## Testing and validation

Product behavior coverage:

- Behavior 1, 2, and 5: unit tests in `crates/warp_terminal/src/shell/mod_tests.rs` and `app/src/terminal/local_tty/shell_tests.rs` verify `nu`, `-nu`, `/usr/bin/nu`, Windows `nu.exe`, and false positives such as `menu.exe`/`/usr/bin/menu.exe`.
- Behavior 3 and 14: existing shell-discovery tests are extended so Nushell appears with the known shell types without regressing other shells.
- Behavior 4, 6, 7, 8, 9, and 10: bootstrap unit coverage verifies Nushell asset selection; rendered-script smoke tests verify that the init/body scripts parse and run under the supported local `nu` binary after build placeholders are replaced; metadata payload tests should cover alias rows, newline-separated command-name lists for custom/built-in/keyword commands, newline-separated environment-variable names, normalized PATH, and `$nu.current-exe` shell path.
- Behavior 11 and 12: `app/src/env_vars/mod.rs` tests verify Nushell initialization/export syntax, command substitution, and quoted environment-variable names; Drive export tests cover the `ShellType` API change.
- Behavior 13: `app/src/autoupdate/linux_test.rs` verifies the Nushell update command gates `warp_finish_update` behind the package-manager command sequence, emits the expected outer `try { ... } catch { ... }` shape, preserves best-effort dist-upgrade handling for Apt, and does not emit POSIX `&&`.
- Behavior 15: unsupported child/subshell paths intentionally keep using existing unsupported-shell behavior for Nushell.

Planned repo-root validation commands for the implementation PR:

```bash
cargo fmt --all -- --check
cargo test -p warp env_vars::tests --lib
cargo test -p warp drive::export::tests --lib
cargo test -p warp_terminal test_from_name --lib
cargo test -p warp_terminal test_nu_parse_aliases --lib
cargo test -p warp test_nu_update_command_gates_finish_update_on_success --lib
cargo test -p warp test_nu_update_command_uses_nu_dist_upgrade_handler --lib
cargo clippy -p warp -p warp_terminal --all-targets --tests -- -D warnings
```

When using the companion Nix devshell locally, run those commands from the repository root through a relative devshell invocation such as `nix develop ../warp#default -c bash -lc '<command>'`; CI should use its normal pinned environment.

Rendered-script smoke validation:

```bash
nu -n --no-std-lib rendered-nu_init_shell.nu
WARP_BOOTSTRAPPED= WARP_SESSION_ID=12345 WARP_INITIAL_WORKING_DIR="$PWD" nu -n --no-std-lib rendered-nu_body.nu
```

Both smoke commands should exit successfully after replacing build placeholders such as `@@USING_CON_PTY_BOOLEAN@@` with a concrete boolean. These smoke checks prove script syntax and basic runtime evaluation, but they do not prove the interactive PTY lifecycle by themselves.

Interactive lifecycle acceptance validation:

1. Build/run Warp from the local checkout with the shared `../warp` flake development environment.
2. Select Nushell or set the default shell path to the local `nu` binary.
3. Open a new session and verify it remains interactive after the Warp init script runs.
4. Run `pwd`, `cd`, `echo`, `false`, and a command after `false`; verify block boundaries, working directory reporting, prompt redraw, and non-zero exit-code reporting.
5. Toggle between Warp prompt mode and honoring the user prompt; verify the original prompt is restored in honor-user-prompt mode and Warp block markers continue to work.
6. Verify custom user `pre_execution`/`pre_prompt` hooks and keybindings still run after Warp prepends/appends its integration entries.
7. Export/copy a Warp Drive environment-variable collection while the active session is Nushell and verify the generated text is valid Nushell `$env` assignment syntax.
8. On Linux package builds, inspect the generated update command and verify `warp_finish_update` is not emitted after a failing package-manager command.

## Risks and mitigations

- **Nushell syntax changes over time.** Nushell is still evolving, so the bootstrap sticks to stable, simple constructs where possible and is covered by smoke tests against the local `nu` binary.
- **POSIX-family assumptions.** Nushell remains `ShellFamily::Posix` for legacy escaping/family APIs, but code that needs concrete syntax now carries `ShellType` through the flow.
- **False-positive detection.** Basename checks and regression tests reduce the risk from the short executable name `nu`.
- **Partial first iteration.** Remote and subshell flows are explicit non-goals so unsupported paths fail predictably instead of producing incorrect Bash/POSIX behavior.

## Follow-ups

- Add remote SSH/session warpification support for Nushell.
- Add Nushell subshell bootstrap support.
- Investigate deeper integration with Nushell-native completions, plugins, LSP, or MCP once local shell support is stable.
