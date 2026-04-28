use std::{
    borrow::Cow,
    env,
    ffi::OsStr,
    path::{self, Path},
};

use is_executable::IsExecutable as _;
use itertools::Itertools as _;

pub fn file_exists_and_is_executable(path: &Path) -> bool {
    // We need to check that the file exists, as the `is_executable` crate doesn't validate this on
    // Windows.
    path.is_file() && path.is_executable()
}

/// Resolves `command` into an executable path, matching the shell's search behavior.
/// If the command contains a path separator, it should resolve to an executable
/// file. Otherwise, it should exist in the process's `PATH`.
///
/// Callers that need to resolve against a different PATH (e.g. one
/// captured from the user's interactive login shell) should use
/// [`resolve_executable_in_path`] directly.
pub fn resolve_executable(command: &str) -> Option<Cow<'_, Path>> {
    let path_var = env::var_os("PATH").unwrap_or_default();
    resolve_executable_in_path(command, &path_var)
}

/// Like [`resolve_executable`], but resolves PATH-based lookups against
/// the given `path_env` instead of the process's own `PATH`.
///
/// Intended for callers that have a specific PATH to search (e.g. one
/// captured from the user's interactive login shell, matching how
/// MCP/LSP find binaries). Callers that want the process's PATH should
/// use [`resolve_executable`] instead.
pub fn resolve_executable_in_path<'a>(command: &'a str, path_env: &OsStr) -> Option<Cow<'a, Path>> {
    if command.contains(path::MAIN_SEPARATOR) {
        let path = Path::new(command);
        return file_exists_and_is_executable(path).then_some(Cow::Borrowed(path));
    }
    for path_dir in env::split_paths(path_env).unique() {
        let resolved = path_dir.join(command);
        if file_exists_and_is_executable(&resolved) {
            return Some(Cow::Owned(resolved));
        }
    }
    None
}

#[cfg(test)]
#[path = "path_test.rs"]
mod tests;
