# Cygwin and MSYS2 startup shells on Windows
GitHub issue: #5839
Figma: none provided
## Summary
Windows users can select fish, zsh, or bash installed through Cygwin or MSYS2 as the default startup shell for new Warp sessions. Warp validates recognizable Cygwin/MSYS2 shell paths and supported launcher commands, persists the selection, and opens new terminal sessions in that shell instead of falling back to PowerShell.
## Problem
The Windows startup shell setting currently rejects or fails to persist paths such as `C:/tools/cygwin/bin/fish.exe`, even when the executable exists. This blocks users who rely on POSIX-like Windows environments such as Cygwin, MSYS2, Git for Windows, and MSYS2-based zsh/fish distributions from using Warp daily.
## Goals
- Let Windows users configure Cygwin and MSYS2 bash, zsh, and fish as startup shells from Settings > Features.
- Accept normal Windows path formats for custom shell executables, including forward slashes and backslashes.
- Support common MSYS2 launcher commands when they identify a supported shell and environment.
- Preserve Warp shell integration, prompt detection, command execution, path conversion, and session restore behavior for these shells.
- Provide clear validation feedback when the entered path or launcher cannot be used.
## Non-goals
- Supporting arbitrary unsupported shells such as Nushell through this issue; native Nushell support remains separate.
- Supporting every possible Cygwin/MSYS2 package layout or custom wrapper script.
- Adding shell-specific configuration UI beyond the existing startup shell selector and custom shell input.
- Changing WSL shell support.
## Behavior
1. On Windows, Settings > Features > Startup shell for new sessions continues to show the existing Default option, discovered Windows-native shells, WSL distributions, and Git Bash entries.
2. When Cygwin or MSYS2 shells are discovered in well-known install locations, Warp lists them as selectable startup shell options. Entries must make the shell family and source distinguishable when names collide, for example "Fish (MSYS2 UCRT64)" versus "Fish (Cygwin)" or by showing the executable path in the existing disambiguation style.
3. Warp treats the following Cygwin/MSYS2 shells as supported startup shells: bash, zsh, and fish. Selecting one persists it as the startup shell for new terminal sessions.
4. The Custom input accepts an absolute Windows path to a supported Cygwin/MSYS2 shell executable when the executable exists and is runnable. Valid path examples include:
   - `C:/tools/cygwin/bin/fish.exe`
   - `C:\tools\cygwin\bin\fish.exe`
   - `C:\\tools\\cygwin\\bin\\fish.exe`
   - `C:/msys64/usr/bin/zsh.exe`
5. The Custom input accepts quoted paths when quoting is needed for spaces, for example `"C:\Program Files\Git\usr\bin\bash.exe"`.
6. The Custom input accepts a supported MSYS2 launcher command when the command identifies a supported shell and MSYS2 environment, for example `C:/msys64/msys2_shell.cmd -defterm -here -no-start -ucrt64 -shell fish -use-full-path`.
7. Warp validates the Custom input without launching the user's shell command. Validation may inspect the executable path, filename, recognized launcher flags, and file metadata, but it must not execute arbitrary user-provided commands.
8. While the Custom input is invalid, Warp keeps the visible invalid state and does not save the value. If possible, the UI explains the reason in user terms: executable not found, unsupported shell, unsupported launcher, or missing shell argument.
9. When the Custom input becomes valid and the user presses Enter or blurs the field, Warp saves the value and the dropdown remains on Custom.
10. A saved Cygwin/MSYS2 startup shell survives app restart and Settings reload. Returning to Settings shows the saved custom path or launcher command rather than resetting to Default.
11. Opening a new terminal session after saving a Cygwin/MSYS2 startup shell starts that shell, not PowerShell, Cmd, or WSL.
12. Warp launches the selected shell in an interactive mode compatible with Warp shell integration. Prompt detection, command blocks, completions, history-backed behavior, and command execution should work at least as well as they do for Git Bash/MSYS2 bash today.
13. For fish, Warp avoids FinalTerm prompt marker conflicts in the same way as other fish startup paths.
14. For zsh, Warp avoids loading user rc files before its bootstrap takes over in the same way as other zsh startup paths.
15. For bash, Warp avoids loading profile/rc files before its bootstrap takes over in the same way as other bash startup paths.
16. If the saved executable or launcher later disappears, new sessions fall back to the existing safe default behavior and surface a user-visible spawn failure or fallback state consistent with other missing startup shells. Warp must not silently rewrite the user's saved setting unless the user chooses a different shell.
17. For Cygwin and MSYS2 sessions, paths shown by shell commands and paths used by Warp features are interpreted in that shell's path dialect. MSYS2-style paths such as `/c/Users/alice/project` and Cygwin-style paths such as `/cygdrive/c/Users/alice/project` map to the corresponding Windows host path when Warp needs a host path.
18. Session restore preserves the working directory for Cygwin/MSYS2 sessions when the directory still exists.
19. Tab/session restore, launching new panes, and opening additional sessions use the saved Cygwin/MSYS2 shell consistently.
20. Choosing Default or another shell from the dropdown replaces the Cygwin/MSYS2 selection using the existing behavior.
21. Warp does not log the full custom shell path or launcher command in telemetry. Telemetry may classify the selection as Cygwin, MSYS2, Git Bash, WSL, Windows-native, or Custom without including personally identifying path segments.
22. Existing Git Bash behavior does not regress. Users who already have Git Bash selected keep working after the change.
23. Existing WSL zsh/fish behavior does not regress.
## Success criteria
1. A Windows user can enter `C:/tools/cygwin/bin/fish.exe`, save it, restart Warp, and open new sessions in Cygwin fish.
2. A Windows user can enter `C:/msys64/usr/bin/zsh.exe`, save it, restart Warp, and open new sessions in MSYS2 zsh.
3. A Windows user can use a supported `msys2_shell.cmd` command with `-shell fish`, save it, and open new sessions in the requested MSYS2 environment.
4. Invalid paths and unsupported shells are rejected before save with a clear invalid state.
5. Git Bash, WSL shells, PowerShell, and Default startup behavior continue to work.
## Validation
Product validation should cover Settings save/reload behavior, new-session launch behavior, command-block behavior after bootstrap, Cygwin/MSYS2 path conversion, missing executable fallback, and regressions for Git Bash, WSL, PowerShell, bash, zsh, and fish.
## Open questions
1. Should Cygwin be enabled in the same rollout as expanded MSYS2 support, or should MSYS2 ship first behind the existing MSYS2 feature flag while Cygwin remains behind a separate flag?
2. Should the Settings UI expose MSYS2 environments as first-class dropdown entries, or is support for direct shell paths plus `msys2_shell.cmd` launcher commands sufficient for the first release?
3. Should Warp support user-provided Cygwin launcher commands in addition to direct `bin\bash.exe`, `bin\zsh.exe`, and `bin\fish.exe` paths?
