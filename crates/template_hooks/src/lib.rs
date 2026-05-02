//! Post-init hook runner for the `warp new` scaffolding system (PDX-58).
//!
//! After template files are materialized by the loader (PDX-57), this crate
//! runs each `[[hooks.post_init]]` entry from `template.toml` in declared
//! order. Resolved variables are exported as `WARP_TPL_<NAME>` env vars;
//! per-hook `env` tables override them.
//!
//! Condition strings may use Tera output-expression syntax (`{{ expr }}`) or
//! bare expressions (`expr`). The runner strips the delimiters, wraps in
//! `{% if %}`, and renders to determine truthiness.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::Command;
use tracing::{info, warn};

/// A single `[[hooks.post_init]]` entry from `template.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostInitHook {
    /// Display label streamed to the user.
    pub name: String,
    /// Shell command executed via `sh -c` from the resolved working directory.
    pub command: String,
    /// Path relative to the project root where the command runs. Defaults to `.`.
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Extra env vars merged on top of the inherited env (and `WARP_TPL_*`).
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Tera expression. Hook is skipped when it evaluates falsy. `None` = always run.
    #[serde(default)]
    pub condition: Option<String>,
    /// What to do when the command exits non-zero or times out.
    #[serde(default)]
    pub fail_strategy: FailStrategy,
}

/// What to do when a post-init hook exits with a non-zero status code or times out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FailStrategy {
    /// Non-zero exit aborts the scaffold; project dir is left in place.
    #[default]
    Abort,
    /// Outcome is logged as a warning; scaffold continues.
    Warn,
    /// Outcome is silently dropped; scaffold continues.
    Ignore,
}

/// Resolved template variables (`name` → JSON scalar or null).
pub type Variables = HashMap<String, serde_json::Value>;

/// Everything the hook runner needs to execute hooks.
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Root of the newly-scaffolded project.
    pub project_root: PathBuf,
    /// Resolved template variables (for condition evaluation and `WARP_TPL_*` env block).
    pub variables: Variables,
    /// Per-hook wall-clock timeout.
    pub timeout: Duration,
}

impl HookContext {
    /// Create a context with a 5-minute per-hook timeout.
    pub fn new(project_root: PathBuf, variables: Variables) -> Self {
        Self {
            project_root,
            variables,
            timeout: Duration::from_secs(300),
        }
    }
}

/// Progress event emitted by [`run_hooks`] as each hook executes.
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// About to run this hook.
    Starting { index: usize, name: String },
    /// Hook was skipped (condition was falsy).
    Skipped { index: usize, name: String },
    /// Hook completed with exit code 0.
    Finished { index: usize, name: String },
    /// Hook exited non-zero; `strategy` determined what happened next.
    Failed {
        index: usize,
        name: String,
        exit_code: Option<i32>,
        stderr: String,
        strategy: FailStrategy,
    },
    /// Hook process was still running when the timeout elapsed.
    TimedOut { index: usize, name: String },
}

/// Trait for receiving progress events from [`run_hooks`].
pub trait HookProgress: Send + Sync {
    fn on_event(&self, event: HookEvent);
}

/// No-op progress handler.
pub struct SilentProgress;

impl HookProgress for SilentProgress {
    fn on_event(&self, _event: HookEvent) {}
}

/// Errors produced by the hook runner.
#[derive(Debug, Error)]
pub enum HookError {
    /// An `abort`-strategy hook exited with a non-zero status.
    #[error("hook '{name}' failed (exit={exit_code:?}): {stderr}")]
    HookFailed {
        name: String,
        exit_code: Option<i32>,
        stderr: String,
    },
    /// An `abort`-strategy hook exceeded its timeout.
    #[error("hook '{name}' timed out after {timeout:?}")]
    HookTimedOut { name: String, timeout: Duration },
    /// The hook's `working_dir` resolved outside the project root.
    #[error("hook '{name}' working_dir '{working_dir}' escapes project root")]
    WorkingDirEscape { name: String, working_dir: String },
    /// The Tera condition expression could not be compiled or rendered.
    #[error("hook '{name}' condition error: {reason}")]
    ConditionError { name: String, reason: String },
    /// I/O error while spawning the subprocess.
    #[error("hook '{name}' io error: {reason}")]
    Io { name: String, reason: String },
}

/// Execute a list of post-init hooks in declared order.
///
/// Returns `Ok(())` when all hooks complete (or are skipped/warned/ignored).
/// Returns `Err(HookError)` on the first hook that fails with `fail_strategy = "abort"`.
pub async fn run_hooks(
    hooks: &[PostInitHook],
    ctx: &HookContext,
    progress: &dyn HookProgress,
) -> Result<(), HookError> {
    let tpl_env = build_warp_tpl_env(&ctx.variables);
    let tera_ctx = build_tera_context(&ctx.variables);

    for (idx, hook) in hooks.iter().enumerate() {
        // 1. Evaluate condition.
        if let Some(cond) = &hook.condition {
            match eval_condition(cond, &tera_ctx) {
                Ok(true) => {}
                Ok(false) => {
                    info!(hook = %hook.name, "post-init hook skipped (condition is falsy)");
                    progress.on_event(HookEvent::Skipped { index: idx, name: hook.name.clone() });
                    continue;
                }
                Err(reason) => {
                    return Err(HookError::ConditionError { name: hook.name.clone(), reason });
                }
            }
        }

        // 2. Resolve working directory.
        let working_dir_display = hook.working_dir.clone().unwrap_or_else(|| ".".into());
        let cwd = resolve_working_dir(&ctx.project_root, hook.working_dir.as_deref())
            .map_err(|_| HookError::WorkingDirEscape {
                name: hook.name.clone(),
                working_dir: working_dir_display,
            })?;

        // 3. Emit Starting.
        info!(hook = %hook.name, command = %hook.command, cwd = %cwd.display(), "running post-init hook");
        progress.on_event(HookEvent::Starting { index: idx, name: hook.name.clone() });

        // 4. Spawn and await.
        let outcome = run_command(&hook.command, &cwd, &tpl_env, &hook.env, ctx.timeout).await;

        // 5. Apply fail_strategy.
        match outcome {
            Ok(()) => {
                progress.on_event(HookEvent::Finished { index: idx, name: hook.name.clone() });
            }
            Err(CommandOutcome::Failed { exit_code, stderr }) => {
                let event = HookEvent::Failed {
                    index: idx,
                    name: hook.name.clone(),
                    exit_code,
                    stderr: stderr.clone(),
                    strategy: hook.fail_strategy,
                };
                match hook.fail_strategy {
                    FailStrategy::Abort => {
                        progress.on_event(event);
                        return Err(HookError::HookFailed {
                            name: hook.name.clone(),
                            exit_code,
                            stderr,
                        });
                    }
                    FailStrategy::Warn => {
                        warn!(hook = %hook.name, ?exit_code, %stderr, "post-init hook failed (warn)");
                        progress.on_event(event);
                    }
                    FailStrategy::Ignore => {
                        progress.on_event(event);
                    }
                }
            }
            Err(CommandOutcome::TimedOut) => {
                let event = HookEvent::TimedOut { index: idx, name: hook.name.clone() };
                match hook.fail_strategy {
                    FailStrategy::Abort => {
                        progress.on_event(event);
                        return Err(HookError::HookTimedOut {
                            name: hook.name.clone(),
                            timeout: ctx.timeout,
                        });
                    }
                    FailStrategy::Warn => {
                        warn!(hook = %hook.name, "post-init hook timed out (warn)");
                        progress.on_event(event);
                    }
                    FailStrategy::Ignore => {
                        progress.on_event(event);
                    }
                }
            }
            Err(CommandOutcome::Io(reason)) => {
                return Err(HookError::Io { name: hook.name.clone(), reason });
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

enum CommandOutcome {
    Failed { exit_code: Option<i32>, stderr: String },
    TimedOut,
    Io(String),
}

async fn run_command(
    command: &str,
    cwd: &Path,
    tpl_env: &HashMap<String, String>,
    hook_env: &HashMap<String, String>,
    timeout: Duration,
) -> Result<(), CommandOutcome> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (k, v) in tpl_env {
        cmd.env(k, v);
    }
    // Per-hook env overrides the auto-generated WARP_TPL_* block.
    for (k, v) in hook_env {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| CommandOutcome::Io(e.to_string()))?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) if out.status.success() => Ok(()),
        Ok(Ok(out)) => Err(CommandOutcome::Failed {
            exit_code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }),
        Ok(Err(e)) => Err(CommandOutcome::Io(e.to_string())),
        Err(_) => Err(CommandOutcome::TimedOut),
    }
}

/// Build `WARP_TPL_<NAME>` env vars from resolved variables.
/// Booleans/numbers serialize to strings; null becomes empty string.
pub(crate) fn build_warp_tpl_env(variables: &Variables) -> HashMap<String, String> {
    variables
        .iter()
        .map(|(k, v)| {
            let env_key = format!("WARP_TPL_{}", k.to_ascii_uppercase());
            let env_val = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            (env_key, env_val)
        })
        .collect()
}

/// Build a Tera context from resolved variables.
pub(crate) fn build_tera_context(variables: &Variables) -> tera::Context {
    let mut ctx = tera::Context::new();
    for (k, v) in variables {
        match v {
            serde_json::Value::Bool(b) => ctx.insert(k, b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ctx.insert(k, &i);
                } else if let Some(f) = n.as_f64() {
                    ctx.insert(k, &f);
                }
            }
            serde_json::Value::String(s) => ctx.insert(k, s),
            serde_json::Value::Null => ctx.insert(k, &Option::<String>::None),
            serde_json::Value::Array(arr) => ctx.insert(k, arr),
            serde_json::Value::Object(obj) => ctx.insert(k, obj),
        }
    }
    ctx
}

/// Evaluate a Tera condition expression (`{{ expr }}` or bare `expr`).
/// Returns `true` when the expression is truthy in Tera's semantics.
pub(crate) fn eval_condition(condition: &str, ctx: &tera::Context) -> Result<bool, String> {
    let expr = strip_tera_delimiters(condition.trim());
    let src = format!("{{% if {expr} %}}true{{% else %}}false{{% endif %}}");
    let mut engine = tera::Tera::default();
    engine.add_raw_template("__cond__", &src).map_err(|e| e.to_string())?;
    let rendered = engine.render("__cond__", ctx).map_err(|e| e.to_string())?;
    Ok(rendered.trim() == "true")
}

/// Strip outer `{{ }}` delimiters, returning the inner expression.
/// `"{{ install_deps }}"` → `"install_deps"`. Unchanged if no delimiters.
pub(crate) fn strip_tera_delimiters(s: &str) -> &str {
    if let Some(inner) = s.strip_prefix("{{").and_then(|t| t.strip_suffix("}}")) {
        inner.trim()
    } else {
        s
    }
}

/// Resolve a working directory relative to `project_root`.
/// Rejects absolute paths, `..` components, and symlink-escaping paths.
pub(crate) fn resolve_working_dir(
    project_root: &Path,
    working_dir: Option<&str>,
) -> Result<PathBuf, ()> {
    let rel = working_dir.unwrap_or(".");
    let candidate = PathBuf::from(rel);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(());
    }
    let resolved = project_root.join(&candidate);
    let canon_root = std::fs::canonicalize(project_root)
        .unwrap_or_else(|_| project_root.to_path_buf());
    let canon_candidate = std::fs::canonicalize(&resolved)
        .unwrap_or_else(|_| resolved.clone());
    if !canon_candidate.starts_with(&canon_root) {
        return Err(());
    }
    Ok(resolved)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
