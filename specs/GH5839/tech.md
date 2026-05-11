# Cygwin and MSYS2 startup shells on Windows — Tech Spec
Product spec: `specs/GH5839/product.md`
## Context
Warp already has several pieces of Windows shell infrastructure, but they are split between legacy startup-shell settings, the newer shell selector model, and an MSYS2-specific launch path.
- `app/src/settings_view/features/startup_shell.rs (176-197)` validates Custom input by calling `is_valid_path_or_command_for_supported_shell`, and saves only when `AvailableShell::try_from` succeeds. This is the red-border behavior users hit.
- `app/src/settings_view/features/startup_shell.rs (250-265)` persists the selected `AvailableShell` through `AvailableShells::set_user_preferred_shell`.
- `app/src/terminal/session_settings/startup_shell.rs (1-69)` is the legacy `StartupShell` setting, which serializes to a string command or custom shell path.
- `app/src/terminal/session_settings/new_session_shell.rs (24-33)` defines the newer persisted `NewSessionShell` variants, including `Executable`, `MSYS2`, `WSL`, and `Custom`.
- `app/src/terminal/available_shells.rs (52-71)` has internal `Config` variants for `KnownLocal`, `Wsl`, `MSYS2`, and `Custom`.
- `app/src/terminal/available_shells.rs (533-618)` discovers known shell executable names and special-cases Windows `bash.exe`, `fish.exe`, and `zsh.exe` by looking for MSYS2/Git Bash executables instead of searching the full PATH.
- `app/src/terminal/available_shells.rs (722-781)` currently discovers Git Bash and, behind `FeatureFlag::MSYS2Shells`, `C:\msys64\usr\bin\{bash,fish,zsh}.exe`. It does not discover Cygwin locations.
- `app/src/terminal/local_tty/shell.rs (73-139)` converts selected `ShellLaunchData` into a `ShellStarter`. It maps MSYS2 launch data to `ShellStarter::MSYS2`.
- `app/src/terminal/local_tty/shell.rs (541-551)` validates a path by resolving the executable and inferring `ShellType` from the executable filename. It has no concept of command-line arguments, MSYS2 launcher scripts, Cygwin roots, or shell dialect.
- `app/src/terminal/local_tty/shell.rs (683-704)` supplies MSYS2-specific startup arguments for zsh, bash, and fish.
- `app/src/terminal/local_tty/terminal_manager.rs (988-1008)` enqueues the init script for zsh and MSYS2 sessions before PTY creation.
- `app/src/terminal/local_tty/windows/mod.rs (252-297)` builds the Windows `CreateProcessW` command line from a shell path plus arguments, with quoting handled centrally.
- `app/src/terminal/model/session/command_executor.rs (270-307)` routes `ShellLaunchData::MSYS2` sessions to `MSYS2CommandExecutor`.
- `app/src/terminal/model/session/command_executor/msys2_command_executor.rs (1-182)` executes generators for MSYS2 sessions and converts MSYS2 PATH entries to Windows paths when delegating to PowerShell.
- `crates/warp_terminal/src/shell/mod.rs (740-843)` defines `ShellLaunchData`, including the `MSYS2` variant and MSYS2 path conversion hooks.
- `crates/warp_util/src/path.rs (338-424)` converts MSYS2 Unix paths to Windows paths using the MSYS2 root.
- `crates/warp_util/src/path.rs (536-542)` recognizes only Git Bash and `msys64\usr\bin` paths as MSYS2-like paths; Cygwin paths are not recognized.
The main gap is that validation and persistence still treat user-entered Custom values as a single executable path, while the launch stack needs to know the POSIX-on-Windows family, root directory, shell type, optional launcher arguments, and path dialect.
## Proposed changes
### 1. Add a generalized POSIX-on-Windows launch model
Introduce a small Windows-only model for shells that run as POSIX environments on the Windows host:
- `WindowsPosixShellKind`: `GitBash`, `MSYS2`, `Cygwin`.
- `WindowsPosixPathDialect`: `MsysDrivePrefix` for `/c/...` and `CygwinDrivePrefix` for `/cygdrive/c/...`.
- `WindowsPosixShellConfig`: executable path, shell type, kind, root path, optional launcher path, optional launcher args, optional MSYS2 environment name.
Keep the existing `NewSessionShell::MSYS2(String)` deserialization path for backward compatibility, but migrate internal launch handling to a generalized `NewSessionShell::WindowsPosixShell(...)` or equivalent persisted shape. If adding a new serialized shape is too disruptive, keep the public enum variants stable and add a helper that resolves `NewSessionShell::MSYS2` and `NewSessionShell::Custom` into the generalized config at runtime.
Extend `ShellLaunchData` with a generalized Windows POSIX variant or add a sibling `Cygwin` variant while preserving `MSYS2` serialization compatibility. The preferred long-term shape is one generalized variant so path conversion, command execution, telemetry classification, and future POSIX-on-Windows families do not duplicate logic.
### 2. Split validation into path parsing and launch config resolution
Replace Custom input validation's direct use of `supported_shell_path_and_type` with a resolver that returns a typed launch candidate:
- Resolve executable-only paths with forward slashes, backslashes, doubled backslashes, and quotes.
- Infer shell type from `bash.exe`, `zsh.exe`, or `fish.exe`.
- Detect Cygwin by root layouts such as `C:\cygwin64\bin\<shell>.exe`, `C:\cygwin\bin\<shell>.exe`, and custom roots where the executable lives under a `bin` directory with Cygwin markers when available.
- Detect MSYS2 by root layouts such as `C:\msys64\usr\bin\<shell>.exe` and existing Git Bash layouts.
- Parse only recognized launcher commands. For `msys2_shell.cmd`, require a supported `-shell bash|zsh|fish` argument and preserve recognized environment flags such as `-mingw64`, `-ucrt64`, `-clang64`, and `-msys`.
- Validate by checking the executable or launcher file exists and by inspecting supported flags. Do not execute the command during validation.
The resolver should expose a validation error enum that the Settings UI can map to concise text. The existing red border can remain, but an inline explanation should be added using existing text-input/theme primitives rather than a one-off UI style.
### 3. Discover Cygwin and richer MSYS2 shells
Extend `AvailableShells::locate_msys2_executables` into a broader Windows POSIX discovery function:
- Keep Git Bash discovery unchanged.
- Keep current `C:\msys64\usr\bin\{bash,fish,zsh}.exe` discovery.
- Add Cygwin discovery for `C:\cygwin64\bin\{bash,fish,zsh}.exe` and `C:\cygwin\bin\{bash,fish,zsh}.exe`.
- Optionally inspect environment variables such as `CYGWIN_HOME` or registry/install metadata only if there is existing utility support or a low-risk Windows API wrapper.
- Represent discovered entries with stable IDs that include kind and executable path, for example `windows-posix:cygwin:<path>` and `windows-posix:msys2:<path>`.
If rollout risk is high, gate Cygwin discovery separately from expanded MSYS2 support. The existing `FeatureFlag::MSYS2Shells` can continue to protect MSYS2 discovery while a new flag protects Cygwin discovery.
### 4. Launch using the generalized config
Update `ShellStarter::init` so generalized Windows POSIX launch data maps to a POSIX-on-Windows starter rather than a generic direct executable. The starter should carry:
- logical shell path or launcher path
- shell type
- shell kind/path dialect
- arguments to start an interactive shell in the mode Warp expects
- optional environment metadata for MSYS2 environments
For direct shell executables, reuse the existing MSYS2 argument strategy where appropriate:
- zsh: `-g --no-rcs`
- bash: `--noprofile --norc`
- fish: `--login --no-config`
For `msys2_shell.cmd`, prefer resolving the launcher command into a direct shell executable plus explicit environment setup if that can be done safely. If preserving the launcher is necessary for environment correctness, add a launcher-aware starter that invokes the `.cmd` through `cmd.exe /d /c` and still records the target shell type for bootstrap. This must be tested against ConPTY because batch-file launchers can introduce an intermediate process.
Keep argument quoting centralized in `app/src/terminal/local_tty/windows/mod.rs` rather than hand-building command strings in the resolver.
### 5. Generalize command execution and path conversion
Rename or wrap `MSYS2CommandExecutor` as `WindowsPosixCommandExecutor` and parameterize it by path dialect and root path.
For MSYS2/Git Bash:
- Keep `/c/...` drive mapping and existing root-relative mapping.
- Preserve the existing PowerShell delegation for Windows-native command discovery.
For Cygwin:
- Add conversion from `/cygdrive/c/...` to `C:\...`.
- Add conversion from Cygwin root paths such as `/usr/bin` to `<cygwin-root>\usr\bin`.
- Convert Windows host paths back to Cygwin paths where Warp needs to send a startup directory or command context into the shell.
Update `ShellLaunchData::maybe_convert_absolute_path`, `maybe_convert_relative_path`, `join_to_native_path`, and `to_shell_encoding` to branch on the generalized path dialect.
### 6. Persist and reload settings safely
Ensure `AvailableShell::get_custom_path` or its replacement returns the original user-entered custom command for Custom entries so Settings can display it after reload. Store structured launch metadata where possible, but do not lose the user's text for supported launcher commands.
Backward compatibility requirements:
- Existing `new_session_shell_override = { MSYS2: <path> }` values continue to launch.
- Existing Git Bash selections continue to match discovered entries.
- Legacy `startup_shell_override` fallback still maps unsupported or missing values to System Default without panicking.
### 7. Telemetry and privacy
Update telemetry classification to distinguish `MSYS2`, `Cygwin`, `GitBash`, `WSL`, `WindowsNative`, and `CustomUnsupported` without sending full executable paths or launcher commands. Reuse the existing pattern where `AvailableShell::telemetry_value` avoids path PII.
## Testing and validation
- Product Behavior 4-9: unit-test the custom input resolver with forward-slash paths, backslash paths, quoted paths, doubled-backslash strings, spaces in paths, missing executables, unsupported filenames, Cygwin direct paths, MSYS2 direct paths, Git Bash paths, and `msys2_shell.cmd` commands with and without `-shell`.
- Product Behavior 2-3: unit-test discovery for Git Bash, `C:\msys64`, `C:\cygwin64`, and duplicate shell names. Use virtual filesystem helpers where possible; add Windows-only tests when host APIs are required.
- Product Behavior 10-11: settings round-trip tests for discovered entries, direct custom paths, launcher commands, legacy `MSYS2` settings, and invalid/missing saved values.
- Product Behavior 12-15: Windows integration or manual tests for Cygwin fish, Cygwin zsh, MSYS2 fish, MSYS2 zsh, MSYS2 bash, and Git Bash. Confirm Warp bootstraps, creates command blocks, and runs commands after startup.
- Product Behavior 16: remove or rename a saved executable and confirm new sessions fall back or fail consistently without silently rewriting the setting.
- Product Behavior 17-18: unit-test Cygwin/MSYS2 path conversion and manually verify session restore preserves PWD in Cygwin and MSYS2 sessions.
- Regression tests: PowerShell, WSL zsh/fish, native Unix shell tests, and existing `available_shells_tests.rs` command-name matching tests.
- Run targeted Rust tests for `available_shells`, `local_tty::shell`, `warp_util::path`, `warp_terminal::shell`, and command executor modules. Run the repository's Windows-capable presubmit subset before shipping because ConPTY launch behavior cannot be fully proven on Linux.
## Parallelization
Parallel sub-agents are useful after the specs are approved because the implementation can be split along natural boundaries, but the final integration must happen in one branch due to shared launch-data types.
- Agent `settings-discovery`: local execution in a worktree such as `/workspace/warp-worktrees/GH5839-settings` on branch `oz-agent/issue-5839-settings`. Owns Settings validation, `AvailableShells` discovery, persistence, telemetry classification, and unit tests for resolver/discovery. It must not edit PTY spawn or command executor code.
- Agent `launch-paths`: local execution in `/workspace/warp-worktrees/GH5839-launch` on branch `oz-agent/issue-5839-launch`. Owns `ShellStarter`, `ShellLaunchData`, Windows PTY launch, path conversion, and command executor changes. It must not edit Settings UI except for compile fixes.
- Agent `windows-validation`: remote execution in a Windows-capable environment if available; otherwise local/manual instructions only. Owns manual/integration validation for Cygwin/MSYS2/Git Bash launch behavior and records exact shell versions and install paths tested.
Merge strategy: land the implementation as a single combined PR after merging `settings-discovery` and `launch-paths` into an integration branch such as `oz-agent/issue-5839`. `windows-validation` does not push code unless it finds a fix that can be isolated; it reports findings back before final review.
Sequential dependencies: shared data model and serialized compatibility decisions must be agreed before both coding agents start. Discovery and launch code can proceed in parallel once that model is stable. Windows validation starts after the integration branch can build.
## Risks and mitigations
- Batch launcher behavior under ConPTY may differ from direct executable launch. Prefer direct executable launch plus explicit environment setup where possible, and test any `.cmd` path on Windows before shipping.
- Over-broad command parsing could create unsafe or confusing behavior. Only accept recognized launchers and flags; do not execute user input during validation.
- Cygwin path semantics differ from MSYS2. Keep path conversion dialect-specific and add tests for `/cygdrive/<drive>` and root-relative paths.
- Persisted settings migration could break existing Git Bash/MSYS2 users. Keep legacy deserialization and add round-trip tests before changing serialized shapes.
- Telemetry can accidentally include local paths. Centralize classification and avoid logging raw custom command strings.
- Linux CI cannot prove Windows launch behavior. Require Windows manual or integration validation before enabling beyond a feature flag.
## Follow-ups
- Consider first-class UI for MSYS2 environments if Custom launcher support is not discoverable enough.
- Consider supporting additional POSIX-on-Windows shells only after bash, zsh, and fish are stable.
- Consider adding a diagnostics action that explains why a saved custom shell cannot launch.
