// SPDX-License-Identifier: AGPL-3.0-only
//
// Doppler CLI wrappers for metadata-only queries.
//
// Hard rules:
//   * Secret values are NEVER fetched, stored, or returned by this module.
//   * `doppler secrets names` (not `doppler secrets`) is used so the CLI
//     itself never downloads values over the wire.

use std::process::Output;
use std::sync::Arc;

use doppler::{CommandRunner, DopplerError};
use serde::{Deserialize, Serialize};

/// A Doppler project (metadata only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub description: String,
}

/// A Doppler config (environment / branch) within a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub environment: String,
    #[serde(default)]
    pub locked: bool,
}

// Wire-format helpers — extra fields are silently ignored.

#[derive(Deserialize)]
struct RawProject {
    name: String,
    slug: String,
    #[serde(default)]
    description: String,
}

impl From<RawProject> for Project {
    fn from(r: RawProject) -> Self {
        Project { name: r.name, slug: r.slug, description: r.description }
    }
}

/// `doppler configs --project X --json` returns `{"page":…,"configs":[…]}`.
#[derive(Deserialize)]
struct RawConfigsResponse {
    configs: Vec<RawConfig>,
}

#[derive(Deserialize)]
struct RawConfig {
    name: String,
    environment: String,
    #[serde(default)]
    locked: bool,
}

impl From<RawConfig> for Config {
    fn from(r: RawConfig) -> Self {
        Config { name: r.name, environment: r.environment, locked: r.locked }
    }
}

// ── MetadataClient ────────────────────────────────────────────────────────

/// Async client for Doppler **metadata-only** queries via the local CLI.
///
/// No method on this type ever fetches or returns a secret value.
pub struct MetadataClient {
    runner: Arc<dyn CommandRunner>,
}

impl MetadataClient {
    pub fn new() -> Self {
        Self::with_runner(Arc::new(doppler::TokioCommandRunner))
    }

    pub fn with_runner(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }

    /// List all Doppler projects.  Runs `doppler projects list --json`.
    pub async fn list_projects(&self) -> Result<Vec<Project>, DopplerError> {
        let output = self.runner.run(&["projects", "list", "--json"]).await?;
        parse_projects(output)
    }

    /// List configs for `project`.  Runs `doppler configs --project P --json`.
    pub async fn list_configs(&self, project: &str) -> Result<Vec<Config>, DopplerError> {
        let output = self
            .runner
            .run(&["configs", "--project", project, "--json"])
            .await?;
        parse_configs(project, output)
    }

    /// List secret **names only** in `project`/`config`.
    ///
    /// Runs `doppler secrets names` — a dedicated sub-command that never
    /// downloads or emits secret values.
    pub async fn list_secret_names(
        &self,
        project: &str,
        config: &str,
    ) -> Result<Vec<String>, DopplerError> {
        let output = self
            .runner
            .run(&["secrets", "names", "--project", project, "--config", config])
            .await?;
        parse_secret_names(project, output)
    }

    /// Return `true` iff `name` exists in `project`/`config`.
    ///
    /// Implemented as a name-list membership check — no value is ever fetched.
    pub async fn has_secret(
        &self,
        project: &str,
        config: &str,
        name: &str,
    ) -> Result<bool, DopplerError> {
        let names = self.list_secret_names(project, config).await?;
        Ok(names.iter().any(|n| n == name))
    }
}

impl Default for MetadataClient {
    fn default() -> Self {
        Self::new()
    }
}

// ── Parsers ───────────────────────────────────────────────────────────────

fn parse_projects(output: Output) -> Result<Vec<Project>, DopplerError> {
    if !output.status.success() {
        return Err(classify_error(output));
    }
    serde_json::from_slice::<Vec<RawProject>>(&output.stdout)
        .map(|v| v.into_iter().map(Project::from).collect())
        .map_err(|e| DopplerError::NonZeroExit {
            code: -1,
            stderr: format!("failed to parse projects JSON: {e}"),
        })
}

fn parse_configs(project: &str, output: Output) -> Result<Vec<Config>, DopplerError> {
    if !output.status.success() {
        return Err(classify_error(output));
    }
    let stdout = &output.stdout;
    let raw: Vec<RawConfig> = if let Ok(w) = serde_json::from_slice::<RawConfigsResponse>(stdout) {
        w.configs
    } else {
        serde_json::from_slice(stdout).map_err(|e| DopplerError::NonZeroExit {
            code: -1,
            stderr: format!("failed to parse configs JSON for project '{project}': {e}"),
        })?
    };
    Ok(raw.into_iter().map(Config::from).collect())
}

/// Parse `doppler secrets names` output.
///
/// The command outputs one name per line. We try a JSON-array parse first so
/// both plain-text and any future JSON output formats work.
fn parse_secret_names(project: &str, output: Output) -> Result<Vec<String>, DopplerError> {
    if !output.status.success() {
        return Err(classify_error(output));
    }
    let text = String::from_utf8_lossy(&output.stdout);

    if let Ok(names) = serde_json::from_str::<Vec<String>>(text.trim()) {
        return Ok(names);
    }

    let names: Vec<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if names.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        if stderr.contains("not found")
            || stderr.contains("no config")
            || stderr.contains("no project")
        {
            return Err(DopplerError::NonZeroExit {
                code: 0,
                stderr: format!(
                    "unexpected empty output listing secret names for project '{project}': {}",
                    stderr.trim()
                ),
            });
        }
    }

    Ok(names)
}

fn classify_error(output: Output) -> DopplerError {
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let lower = stderr.to_lowercase();
    if lower.contains("not authenticated") || lower.contains("you must login") {
        return DopplerError::NotAuthenticated;
    }
    if lower.contains("no config selected")
        || lower.contains("no project")
        || lower.contains("setup configuration")
    {
        return DopplerError::NoProjectBound;
    }
    if lower.contains("could not reach") || lower.contains("network") || lower.contains("dial tcp")
    {
        return DopplerError::Unreachable;
    }
    DopplerError::NonZeroExit {
        code: output.status.code().unwrap_or(-1),
        stderr,
    }
}
