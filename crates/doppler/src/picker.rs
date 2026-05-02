// SPDX-License-Identifier: AGPL-3.0-only
//
// PDX-51 [A4.3]: Project / config picker.
//
// Listing helpers (`list_projects`, `list_configs`) wrap `doppler projects
// --json` and `doppler configs --project X --json` so a UI surface can
// render its own picker rather than embedding the CLI's interactive TUI.
// The selection is persisted by shelling out to `doppler setup` via
// [`bind_project`] — the dedicated CLI subcommand that updates
// `.doppler.yaml` for the chosen scope.
//
// All three helpers stream their command through the existing
// [`CommandRunner`] abstraction so tests can substitute a mock runner.

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use crate::{CommandRunner, DopplerError};

/// A Doppler project, as returned by `doppler projects --json`.
///
/// We keep a deliberately narrow projection of the JSON: enough to render a
/// picker (display name + slug for the underlying setup call), nothing more.
/// Extra fields are tolerated via `#[serde(default)]` so a future CLI
/// upgrade does not break this code.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DopplerProject {
    /// Stable, URL-safe identifier; this is what `doppler setup --project`
    /// expects.
    pub slug: String,
    /// Human-friendly display name. Often equal to `slug`.
    pub name: String,
    /// Optional description shown in the dashboard. May be empty.
    #[serde(default)]
    pub description: String,
}

/// A Doppler config (environment), as returned by `doppler configs --json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DopplerConfig {
    /// The config name; this is what `doppler setup --config` expects.
    pub name: String,
    /// `true` for the root config of an environment (e.g. `dev`, `stg`,
    /// `prd`); `false` for branch configs descended from one of those.
    #[serde(default)]
    pub root: bool,
    /// Environment slug (`dev`, `stg`, `prd`, …) the config lives under.
    #[serde(default)]
    pub environment: String,
}

/// List all Doppler projects visible to the authenticated user.
///
/// Runs `doppler projects --json` via `runner` and parses the result.
/// Returns [`DopplerError::NotAuthenticated`] if the CLI reports a missing
/// auth token, [`DopplerError::Unreachable`] for network errors, or
/// [`DopplerError::NonZeroExit`] otherwise.
pub async fn list_projects(
    runner: Arc<dyn CommandRunner>,
) -> Result<Vec<DopplerProject>, DopplerError> {
    let output = runner.run(&["projects", "--json"], None).await?;
    if !output.status.success() {
        return Err(map_listing_error(&output));
    }
    serde_json::from_slice::<Vec<DopplerProject>>(&output.stdout).map_err(|e| {
        DopplerError::NonZeroExit {
            code: 0,
            stderr: format!("failed to parse `doppler projects --json` output: {e}"),
        }
    })
}

/// List all configs (environments + branch configs) for `project`.
///
/// Runs `doppler configs --project <project> --json`.
pub async fn list_configs(
    runner: Arc<dyn CommandRunner>,
    project: &str,
) -> Result<Vec<DopplerConfig>, DopplerError> {
    let output = runner
        .run(&["configs", "--project", project, "--json"], None)
        .await?;
    if !output.status.success() {
        return Err(map_listing_error(&output));
    }
    serde_json::from_slice::<Vec<DopplerConfig>>(&output.stdout).map_err(|e| {
        DopplerError::NonZeroExit {
            code: 0,
            stderr: format!("failed to parse `doppler configs --json` output: {e}"),
        }
    })
}

/// Bind `project`/`config` to `scope` by shelling out to `doppler setup`.
///
/// Equivalent to running:
///
/// ```bash
/// doppler setup --no-prompt --project <project> --config <config> --scope <scope>
/// ```
///
/// `--no-prompt` keeps the CLI silent (no TUI), so this is safe to call from
/// a GUI button. Doppler writes the binding to the directory's
/// `.doppler.yaml`. After this returns, [`crate::read_status`] will report
/// the new binding.
pub async fn bind_project(
    runner: Arc<dyn CommandRunner>,
    project: &str,
    config: &str,
    scope: &Path,
) -> Result<(), DopplerError> {
    let scope_str = scope.to_str().ok_or_else(|| DopplerError::NonZeroExit {
        code: 0,
        stderr: format!("scope path is not valid utf-8: {}", scope.display()),
    })?;
    let output = runner
        .run(
            &[
                "setup",
                "--no-prompt",
                "--project",
                project,
                "--config",
                config,
                "--scope",
                scope_str,
            ],
            Some(scope),
        )
        .await?;
    if output.status.success() {
        return Ok(());
    }
    Err(map_listing_error(&output))
}

/// Map a non-zero `doppler` invocation to one of the rich [`DopplerError`]
/// variants by sniffing stderr. Mirrors the parser used in
/// [`crate::parse_output`] but kept private to this module so the listing
/// helpers can share it without taking a dependency on the secret-fetching
/// path.
fn map_listing_error(output: &std::process::Output) -> DopplerError {
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let lower = stderr.to_lowercase();
    if lower.contains("not authenticated") || lower.contains("you must login") {
        return DopplerError::NotAuthenticated;
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
