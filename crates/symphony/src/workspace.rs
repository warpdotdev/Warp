//! Per-issue workspace management (spec §9).
//!
//! A workspace is a regular directory under `WorkspaceConfig::root`. The
//! manager guarantees:
//!
//!   * the on-disk path stays inside the configured root (no traversal),
//!   * the workspace key derived from `Issue::identifier` only contains
//!     `[A-Za-z0-9._-]` (other chars are replaced with `_`),
//!   * the `after_create` hook runs exactly once on first creation, and
//!     that failure aborts dispatch,
//!   * the `before_run` hook is fatal if it fails,
//!   * the `after_run` hook is best-effort and never panics.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::process::Command;

use crate::tracker::Issue;
use crate::workflow::HooksConfig;

/// Materialized workspace returned by [`WorkspaceManager::ensure_for`].
#[derive(Debug, Clone)]
pub struct Workspace {
    /// On-disk path to the workspace.
    pub path: PathBuf,
    /// Sanitized key derived from the issue identifier.
    pub workspace_key: String,
    /// `true` if this call created the workspace; `false` if it already existed.
    pub created_now: bool,
}

/// Errors raised by the workspace manager.
#[derive(Debug, Error)]
pub enum WorkspaceError {
    /// I/O failure (mkdir, canonicalize, etc).
    #[error("io: {0}")]
    Io(String),
    /// Sanitized path escaped the configured root.
    #[error("path traversal blocked: workspace path is outside the root")]
    PathTraversal,
    /// A configured hook returned a non-zero status.
    #[error("hook failed: {hook} (exit={code:?}): {stderr}")]
    HookFailed {
        /// Name of the hook (`after_create`, `before_run`, …).
        hook: String,
        /// Exit code, if any.
        code: Option<i32>,
        /// Captured stderr.
        stderr: String,
    },
    /// Hook timed out.
    #[error("hook timed out: {0}")]
    HookTimeout(String),
}

/// Trait for executing shell hooks. Pulled out so tests can mock execution.
#[async_trait]
pub trait HookRunner: Send + Sync {
    /// Run `script` with `sh -lc` inside `cwd` with the given timeout.
    async fn run(
        &self,
        script: &str,
        cwd: &Path,
        timeout: Duration,
    ) -> Result<(), WorkspaceError>;
}

/// Default `tokio::process`-backed hook runner.
pub struct ShellHookRunner;

#[async_trait]
impl HookRunner for ShellHookRunner {
    async fn run(
        &self,
        script: &str,
        cwd: &Path,
        timeout: Duration,
    ) -> Result<(), WorkspaceError> {
        let mut cmd = Command::new("sh");
        cmd.arg("-lc")
            .arg(script)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = cmd
            .spawn()
            .map_err(|e| WorkspaceError::Io(e.to_string()))?;
        let fut = child.wait_with_output();
        let output = match tokio::time::timeout(timeout, fut).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(WorkspaceError::Io(e.to_string())),
            Err(_) => return Err(WorkspaceError::HookTimeout(format!("{:?}", timeout))),
        };
        if !output.status.success() {
            return Err(WorkspaceError::HookFailed {
                hook: "(shell)".to_string(),
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }
}

/// Workspace lifecycle manager.
pub struct WorkspaceManager {
    root: PathBuf,
    hooks: HooksConfig,
    runner: Arc<dyn HookRunner>,
}

impl WorkspaceManager {
    /// Construct a manager rooted at `root`, using the default shell runner.
    pub fn new(root: PathBuf, hooks: HooksConfig) -> Self {
        Self::with_runner(root, hooks, Arc::new(ShellHookRunner))
    }

    /// Construct with a custom hook runner.
    pub fn with_runner(root: PathBuf, hooks: HooksConfig, runner: Arc<dyn HookRunner>) -> Self {
        Self {
            root,
            hooks,
            runner,
        }
    }

    /// Ensure a workspace directory exists for `issue`. On first creation,
    /// runs the `after_create` hook (fatal on failure).
    pub async fn ensure_for(&self, issue: &Issue) -> Result<Workspace, WorkspaceError> {
        let key = sanitize_identifier(&issue.identifier);
        let path = self.root.join(&key);

        // Defence in depth: ensure the joined path is still under root, even
        // after `..`-style sanitization quirks.
        if !path_within(&self.root, &path) {
            return Err(WorkspaceError::PathTraversal);
        }

        let created_now = !path.exists();
        if created_now {
            tokio::fs::create_dir_all(&path)
                .await
                .map_err(|e| WorkspaceError::Io(e.to_string()))?;
            if let Some(script) = &self.hooks.after_create {
                self.runner
                    .run(
                        script,
                        &path,
                        Duration::from_millis(self.hooks.timeout_ms),
                    )
                    .await
                    .map_err(|e| name_hook(e, "after_create"))?;
            }
        }

        Ok(Workspace {
            path,
            workspace_key: key,
            created_now,
        })
    }

    /// Run the `before_run` hook. Fatal on failure.
    pub async fn run_before_run_hook(&self, ws: &Workspace) -> Result<(), WorkspaceError> {
        if let Some(script) = &self.hooks.before_run {
            self.runner
                .run(
                    script,
                    &ws.path,
                    Duration::from_millis(self.hooks.timeout_ms),
                )
                .await
                .map_err(|e| name_hook(e, "before_run"))?;
        }
        Ok(())
    }

    /// Run the `after_run` hook. Best-effort: failure is logged, never returned.
    pub async fn run_after_run_hook(&self, ws: &Workspace) {
        if let Some(script) = &self.hooks.after_run {
            if let Err(e) = self
                .runner
                .run(
                    script,
                    &ws.path,
                    Duration::from_millis(self.hooks.timeout_ms),
                )
                .await
            {
                tracing::warn!(error = %e, workspace = %ws.workspace_key, "after_run hook failed");
            }
        }
    }
}

fn name_hook(e: WorkspaceError, name: &str) -> WorkspaceError {
    match e {
        WorkspaceError::HookFailed { code, stderr, .. } => WorkspaceError::HookFailed {
            hook: name.to_string(),
            code,
            stderr,
        },
        other => other,
    }
}

/// Sanitize an issue identifier into a filesystem-safe workspace key.
///
/// Replaces any character outside `[A-Za-z0-9._-]` with `_`.
pub fn sanitize_identifier(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// Check whether `candidate` is `root` or a descendant of `root`.
///
/// Reject any candidate that contains a literal `..` component (defence in
/// depth — sanitization should have stripped these). Otherwise canonicalize
/// the parent directory of the candidate (which is expected to exist —
/// it's the root we're about to create children inside) and compare prefixes
/// against the canonicalized root.
fn path_within(root: &Path, candidate: &Path) -> bool {
    if candidate
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    let canon_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    // The candidate file may not exist yet, but its parent (the workspace
    // root) does. Canonicalize the parent and re-attach the file_name so
    // symlink-resolved comparisons hold on platforms where /tmp is itself
    // a symlink (e.g. macOS).
    let candidate_canon = match candidate.parent() {
        Some(parent) => {
            let canon_parent = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
            match candidate.file_name() {
                Some(name) => canon_parent.join(name),
                None => canon_parent,
            }
        }
        None => candidate.to_path_buf(),
    };
    candidate_canon.starts_with(&canon_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize_identifier("PDX-12"), "PDX-12");
        assert_eq!(sanitize_identifier("PDX/12"), "PDX_12");
        assert_eq!(sanitize_identifier("../etc/passwd"), ".._etc_passwd");
        assert_eq!(sanitize_identifier(""), "_");
    }
}
