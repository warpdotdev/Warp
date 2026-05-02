//! Helpers for understanding the structure of shell commands whose output contains
//! bare filenames rooted at a directory argument (e.g. `ls SUBDIR/`, `tree SUBDIR`).
//!
//! When the terminal detects a clickable filename in the output of such a command,
//! resolving the candidate against the block's CWD alone is incorrect: bare
//! filenames listed by `ls SUBDIR/` are rooted at `SUBDIR`, not at the CWD. If the
//! user happens to have a same-named file in the CWD (very common for `README.md`),
//! the detector would silently open the wrong file. See sibling module docs for the
//! full bug class.
//!
//! This module provides a narrow helper that parses a command line to extract the
//! first positional path argument, joins it to the block's CWD, and returns it only
//! if the resolved path is a directory on disk.

use std::path::{Path, PathBuf};

/// Default set of directory-listing commands whose bare-filename output should be
/// resolved against their directory argument.
///
/// Exposed as a constant so callers can start from this set and extend it from user
/// settings. Deliberately conservative: `ls` is the canonical case. Modern ls
/// replacements (`exa`, `eza`, `lsd`) behave identically for plain-arg output and
/// are included.
///
/// Not included by default:
/// - `tree` produces output with box-drawing characters that the grid tokenizer
///   currently does not recognize as link separators (tracked separately). Adding
///   `tree` here has no effect until that tokenizer fix lands.
/// - Recursive listings (`ls -R`, `tree`) have multiple root directories within a
///   single block and need per-section resolution, which is out of scope here.
pub const DEFAULT_LISTING_COMMANDS: &[&str] = &["ls", "exa", "eza", "lsd"];

/// Given a shell command line and the block's CWD, return the directory argument
/// (joined to CWD) if the command is a directory-listing command from `listing_commands`
/// and the first positional argument resolves to an existing directory on disk.
///
/// Returns `None` if:
/// - The command cannot be tokenized.
/// - The command name is not in `listing_commands`.
/// - There is no positional argument (e.g. plain `ls`).
/// - The positional argument does not resolve to an existing directory.
///
/// `resolved_command_name` is an optional override for the command-name match. When
/// the caller has already resolved shell aliases (e.g. via `Block::top_level_command`),
/// pass the alias-resolved name here and we match against it instead of the raw first
/// token. Positional-argument extraction still uses the raw command tokens, which is
/// correct for the common case where aliases only add flags (`alias ll='ls -l'` →
/// `ll DIR/` still has `DIR/` as the first positional). Aliases that introduce their
/// own positional arguments (`alias lsd='ls /tmp'`) are a known limitation: we'd pick
/// the user's typed arg, missing the aliased one. Rare and documented.
///
/// ## Examples
///
/// ```text
/// "ls -la subdir/"            + cwd=/a/b  -> Some(/a/b/subdir)  (if /a/b/subdir is a dir)
/// "ls --color=always subdir"  + cwd=/a/b  -> Some(/a/b/subdir)  (if dir)
/// "ls /etc/"                  + cwd=/a/b  -> Some(/etc)         (if dir)
/// "ls"                        + cwd=/a/b  -> None
/// "ls subdir1/ subdir2/"      + cwd=/a/b  -> Some(/a/b/subdir1) (first arg wins)
/// "cat subdir/foo"            + cwd=/a/b  -> None               (cat not in listing_commands)
/// ```
///
/// With `resolved_command_name`:
///
/// ```text
/// "ll subdir/", resolved="ls"  + cwd=/a/b  -> Some(/a/b/subdir)
/// "ll subdir/", resolved=None  + cwd=/a/b  -> None              (ll not in listing_commands)
/// ```
///
/// ## Shell parsing
///
/// Uses `shlex` to split the command into tokens. This handles POSIX-style quoting
/// (`'...'`, `"..."`, and backslash escapes). It does NOT handle:
/// - Shell variable expansion (`$HOME`, `$VAR`). If a user runs `ls $HOME/foo/`, the
///   command as stored in the block may be either pre- or post-expansion depending on
///   how Warp captures it; this function sees whatever is in the string and will fail
///   gracefully if `$HOME/foo` is not a valid directory name.
/// - Aliases with baked-in positional arguments. `alias lsd='ls /tmp'; lsd DIR/` will
///   resolve to `DIR/` (the user-typed arg), not `/tmp` (the aliased arg). This is a
///   rare edge case; if it matters, callers can expand the full alias upstream and
///   pass the expanded command string.
/// - Environment variable prefixes (`FOO=bar ls DIR/`). Handled by skipping leading
///   `KEY=VALUE`-shaped tokens.
/// - Compound shells (`cd /x && ls DIR/`). We only inspect the first command.
///
/// ## Flag heuristic
///
/// After identifying a listing command, we skip all tokens that start with `-` as
/// flag tokens. This is a simplification: `ls` flags with values like
/// `--color=always` are a single token that starts with `-` (handled correctly);
/// `--color always` (two tokens) would incorrectly consume `always` as a flag, but no
/// `ls` flag actually takes a path-like separate value, so this is safe for `ls`.
pub fn listing_command_argument_dir(
    command: &str,
    resolved_command_name: Option<&str>,
    pwd: &Path,
    listing_commands: &[&str],
) -> Option<PathBuf> {
    let tokens = shlex::split(command)?;
    let mut iter = tokens.iter();

    // First non-empty token is the command name. Skip leading env-var assignments
    // (e.g. `FOO=bar ls DIR/`) by stepping past any `KEY=VALUE`-shaped tokens.
    let raw_command_name = loop {
        let token = iter.next()?;
        if token.contains('=') && !token.starts_with('-') {
            continue;
        }
        break token.as_str();
    };

    // Use the alias-resolved name for the listing-command match if provided; otherwise
    // fall back to the raw first token. Positional extraction below always uses the
    // raw tokens regardless.
    let name_for_match = resolved_command_name.unwrap_or(raw_command_name);
    if !listing_commands.contains(&name_for_match) {
        return None;
    }

    // Find the first positional argument (skipping flags).
    let first_positional = iter.find(|t| !t.starts_with('-'))?;

    // Expand tilde if present. Without shell-level `$HOME`, `~` by itself or `~/foo`
    // won't resolve otherwise. We reuse the minimal expansion here rather than
    // depending on `shellexpand` to keep warp_util lean.
    let expanded = expand_leading_tilde(first_positional);

    // Join with pwd if relative; leave absolute paths alone.
    let candidate = if expanded.is_absolute() {
        expanded
    } else {
        pwd.join(&expanded)
    };

    // Only return the path if it actually resolves to a directory on disk. This
    // guards against the user typing a typo'd path, or a path that exists as a file
    // (which wouldn't be a listing target anyway).
    candidate.is_dir().then_some(candidate)
}

/// Expands a leading `~` or `~/` in a path to `$HOME` / `$HOME/`. Returns the input
/// unchanged if it does not start with `~`, or if `$HOME` cannot be determined.
fn expand_leading_tilde(token: &str) -> PathBuf {
    if let Some(rest) = token.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if token == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(token)
}

#[cfg(test)]
#[path = "listing_command_test.rs"]
mod tests;
