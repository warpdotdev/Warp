//! Bounded diff size enforcement (Helm-specific lightweight PDX-28).
//!
//! After each agent run, we shell out to `git diff --shortstat HEAD` in the
//! workspace and refuse runs whose `insertions + deletions` exceed
//! `max_lines`. This catches runaway agents that try to rewrite the world.

use std::path::Path;

use thiserror::Error;
use tokio::process::Command;

/// Per-run diff statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffStat {
    /// Number of inserted lines.
    pub insertions: usize,
    /// Number of deleted lines.
    pub deletions: usize,
}

impl DiffStat {
    /// Total of inserted + deleted lines.
    pub fn total(&self) -> usize {
        self.insertions + self.deletions
    }
}

/// Errors raised by [`DiffGuard::check`].
#[derive(Debug, Error)]
pub enum DiffGuardError {
    /// Diff size exceeded the configured cap.
    #[error("diff guard exceeded: {insertions}+{deletions} lines (max {max})")]
    Exceeded {
        /// Inserted lines.
        insertions: usize,
        /// Deleted lines.
        deletions: usize,
        /// Configured cap.
        max: usize,
    },
    /// The `git diff` invocation failed.
    #[error("git failed: {0}")]
    GitFailed(String),
}

/// Diff-size guard.
#[derive(Debug, Clone, Copy)]
pub struct DiffGuard {
    /// Maximum allowed `insertions + deletions`.
    pub max_lines: usize,
}

impl DiffGuard {
    /// New guard with the given cap.
    pub fn new(max_lines: usize) -> Self {
        Self { max_lines }
    }

    /// Run `git diff --shortstat HEAD` inside `workspace_path` and parse the
    /// result. Returns the parsed stat on success; rejects if the total
    /// exceeds `max_lines`.
    pub async fn check(&self, workspace_path: &Path) -> Result<DiffStat, DiffGuardError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(workspace_path)
            .args(["diff", "--shortstat", "HEAD"])
            .output()
            .await
            .map_err(|e| DiffGuardError::GitFailed(e.to_string()))?;
        if !output.status.success() {
            // No HEAD is also an error here; treat as zero-diff so we don't
            // block the very first run before anything has been committed.
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("unknown revision") || stderr.contains("ambiguous argument") {
                return Ok(DiffStat {
                    insertions: 0,
                    deletions: 0,
                });
            }
            return Err(DiffGuardError::GitFailed(stderr.into_owned()));
        }
        let stat = parse_shortstat(&String::from_utf8_lossy(&output.stdout));
        if stat.total() > self.max_lines {
            return Err(DiffGuardError::Exceeded {
                insertions: stat.insertions,
                deletions: stat.deletions,
                max: self.max_lines,
            });
        }
        Ok(stat)
    }
}

/// Parse a single `git diff --shortstat` line. Returns zeros if the input
/// is empty or unparseable.
///
/// Format examples:
///   ` 3 files changed, 12 insertions(+), 4 deletions(-)`
///   ` 1 file changed, 7 insertions(+)`
///   ` 1 file changed, 2 deletions(-)`
fn parse_shortstat(s: &str) -> DiffStat {
    let mut insertions = 0usize;
    let mut deletions = 0usize;
    for raw in s.lines() {
        let line = raw.trim();
        for chunk in line.split(',') {
            let chunk = chunk.trim();
            if let Some(rest) = chunk.strip_suffix(" insertions(+)") {
                insertions += rest.trim().parse::<usize>().unwrap_or(0);
            } else if let Some(rest) = chunk.strip_suffix(" insertion(+)") {
                insertions += rest.trim().parse::<usize>().unwrap_or(0);
            } else if let Some(rest) = chunk.strip_suffix(" deletions(-)") {
                deletions += rest.trim().parse::<usize>().unwrap_or(0);
            } else if let Some(rest) = chunk.strip_suffix(" deletion(-)") {
                deletions += rest.trim().parse::<usize>().unwrap_or(0);
            }
        }
    }
    DiffStat {
        insertions,
        deletions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_shortstat() {
        let s = " 3 files changed, 12 insertions(+), 4 deletions(-)\n";
        let stat = parse_shortstat(s);
        assert_eq!(stat.insertions, 12);
        assert_eq!(stat.deletions, 4);
        assert_eq!(stat.total(), 16);
    }

    #[test]
    fn parses_insertions_only() {
        let s = " 1 file changed, 7 insertions(+)\n";
        let stat = parse_shortstat(s);
        assert_eq!(stat.insertions, 7);
        assert_eq!(stat.deletions, 0);
    }

    #[test]
    fn parses_deletions_only() {
        let s = " 1 file changed, 2 deletions(-)\n";
        let stat = parse_shortstat(s);
        assert_eq!(stat.insertions, 0);
        assert_eq!(stat.deletions, 2);
    }

    #[test]
    fn parses_singular_forms() {
        let s = " 1 file changed, 1 insertion(+), 1 deletion(-)\n";
        let stat = parse_shortstat(s);
        assert_eq!(stat.insertions, 1);
        assert_eq!(stat.deletions, 1);
    }

    #[test]
    fn empty_input_zero() {
        let stat = parse_shortstat("");
        assert_eq!(stat.total(), 0);
    }
}
