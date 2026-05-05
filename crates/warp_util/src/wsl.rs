//! WSL detection and binary resolution for Warp-internal subprocesses.
//!
//! Warp ships as a Linux ELF binary that users routinely run inside
//! WSL. WSL's default `appendWindowsPath = true` (in `/etc/wsl.conf`)
//! puts directories like `/mnt/c/Program Files/Git/cmd/` on `PATH`,
//! so a bare `Command::new("git")` can resolve to Windows `git.exe`
//! through WSL interop. That path works for some commands but is
//! dramatically slower, can mishandle Linux paths, and breaks
//! Linux-side hooks.
//!
//! [`git_binary`] and [`gh_binary`] return an absolute Linux-side
//! path when running inside WSL, falling back to the literal program
//! name everywhere else. The same `/mnt/*` filtering precedent is
//! used for `compgen` in
//! `app/src/terminal/model/session/command_executor/wsl_command_executor.rs`.
//!
//! Resolution is cached for the life of the process; PATH is
//! effectively static for the Warp host process.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(test)]
#[path = "wsl_tests.rs"]
mod tests;

/// True when running inside a WSL guest. Cached for the life of the
/// process. Detection probes `/proc/sys/fs/binfmt_misc/WSLInterop`,
/// matching the existing checks in `crates/warpui/src/platform/linux/`
/// and `app/src/crash_reporting/linux.rs`.
pub fn is_wsl() -> bool {
    static IS_WSL: OnceLock<bool> = OnceLock::new();
    *IS_WSL.get_or_init(|| Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists())
}

/// Program name to pass to `Command::new` for invoking `git` from
/// Warp-internal code. See module docs for the WSL behavior.
pub fn git_binary() -> &'static OsStr {
    static GIT_BIN: OnceLock<OsString> = OnceLock::new();
    GIT_BIN.get_or_init(|| resolve_or_warn("git"))
}

/// Program name to pass to `Command::new` for invoking `gh`.
pub fn gh_binary() -> &'static OsStr {
    static GH_BIN: OnceLock<OsString> = OnceLock::new();
    GH_BIN.get_or_init(|| resolve_or_warn("gh"))
}

fn resolve_or_warn(name: &str) -> OsString {
    let path_env = std::env::var_os("PATH");
    match resolve_binary_in_wsl_safe_path(name, path_env.as_deref(), is_wsl()) {
        Some(p) => p.into_os_string(),
        None => {
            if is_wsl() {
                log::warn!(
                    "wsl: no Linux-side `{name}` found on PATH (excluding /mnt/*); \
                     falling back to bare `{name}` which may resolve to a Windows .exe"
                );
            }
            OsString::from(name)
        }
    }
}

/// Returns the first executable named `name` on `path_env`, skipping
/// any PATH entry under `/mnt/` when `is_wsl` is true. Returns `None`
/// if no acceptable match exists. Pure — exposed for unit testing
/// without depending on a real WSL host.
pub fn resolve_binary_in_wsl_safe_path(
    name: &str,
    path_env: Option<&OsStr>,
    is_wsl: bool,
) -> Option<PathBuf> {
    let path_env = path_env?;
    for dir in std::env::split_paths(path_env) {
        if is_wsl && dir.starts_with("/mnt") {
            continue;
        }
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    match std::fs::metadata(path) {
        Ok(md) => md.is_file() && (md.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_executable_file(_path: &Path) -> bool {
    // The WSL-safe resolver only runs on Linux. Other targets short-
    // circuit through `is_wsl() == false`, so this stub is unreachable
    // in practice — present only to keep the crate compiling.
    false
}
