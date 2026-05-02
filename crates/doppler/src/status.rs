// SPDX-License-Identifier: AGPL-3.0-only
//
// PDX-52: Status reader — parses `doppler configure --all` output to
// determine login state, project bindings, and config selection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{CommandRunner, DopplerError};

/// Parsed result of `doppler configure --all`.
///
/// Covers the three things PDX-52 asks for:
/// - Login state    → `authenticated`
/// - Project bindings → `scoped_bindings`
/// - Config selection → `scoped_bindings[*].config` / `default_config`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DopplerStatus {
    /// `true` when a `token` key exists in the `(default)` scope, meaning the
    /// CLI has a personal auth token or service token configured.
    pub authenticated: bool,

    /// Per-directory project/config bindings, ordered as they appear in the
    /// `configure --all` output.
    pub scoped_bindings: Vec<ScopedBinding>,

    /// Project configured in the `(default)` scope (used when no directory
    /// binding overrides it).
    pub default_project: Option<String>,

    /// Config configured in the `(default)` scope.
    pub default_config: Option<String>,
}

/// A project/config pair bound to a specific directory scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedBinding {
    /// Absolute path the binding applies to.
    pub scope: PathBuf,
    pub project: Option<String>,
    pub config: Option<String>,
}

/// Parse the stdout of `doppler configure --all` into a [`DopplerStatus`].
///
/// The Doppler CLI renders a box-drawing table. This function identifies data
/// rows by the presence of the `│` cell separator and skips border/divider
/// lines. Both layout variants are handled:
/// - scope only on the first row of a group (blank on continuation rows)
/// - scope repeated on every row
///
/// Returns a zero-state [`DopplerStatus`] for empty or unrecognisable output.
pub fn parse_configure_all(stdout: &str) -> DopplerStatus {
    let mut entries: Vec<(String, String, String)> = Vec::new();
    let mut current_scope = String::new();

    for line in stdout.lines() {
        // Data rows contain the vertical-bar separator │.
        // Border and divider rows use ─, ┼, ├, ┤, ┬, ┴ but not │.
        if !line.contains('│') {
            continue;
        }

        // Split on │; expected layout: "" | scope | name | value | "" (≥5 parts).
        let cells: Vec<&str> = line.split('│').map(|s| s.trim()).collect();
        if cells.len() < 4 {
            continue;
        }

        let scope_cell = cells[1];
        let name_cell = cells[2];
        let value_cell = cells[3];

        // Skip the header row and any row with no key.
        if name_cell == "Name" || name_cell.is_empty() {
            continue;
        }

        // Keep current_scope updated; blank scope cell means "same as last".
        if !scope_cell.is_empty() {
            current_scope = scope_cell.to_string();
        }

        // Treat legacy `enclave.project` / `enclave.config` as their modern
        // equivalents; older CLI versions used the `enclave.` prefix.
        let name_normalised = name_cell
            .strip_prefix("enclave.")
            .unwrap_or(name_cell)
            .to_string();

        entries.push((current_scope.clone(), name_normalised, value_cell.to_string()));
    }

    // Build status from the collected (scope, name, value) triples.
    let mut authenticated = false;
    let mut default_project: Option<String> = None;
    let mut default_config: Option<String> = None;

    // Maintain insertion order for scoped bindings while deduplicating scopes.
    let mut scope_order: Vec<String> = Vec::new();
    let mut scope_project: HashMap<String, Option<String>> = HashMap::new();
    let mut scope_config: HashMap<String, Option<String>> = HashMap::new();

    for (scope, name, value) in entries {
        if scope == "(default)" {
            match name.as_str() {
                "token" => authenticated = !value.is_empty(),
                "project" => default_project = Some(value),
                "config" => default_config = Some(value),
                _ => {}
            }
        } else {
            if !scope_project.contains_key(&scope) {
                scope_order.push(scope.clone());
                scope_project.insert(scope.clone(), None);
                scope_config.insert(scope.clone(), None);
            }
            match name.as_str() {
                "project" => {
                    scope_project.insert(scope, Some(value));
                }
                "config" => {
                    scope_config.insert(scope, Some(value));
                }
                _ => {}
            }
        }
    }

    let scoped_bindings = scope_order
        .into_iter()
        .map(|scope| ScopedBinding {
            project: scope_project.remove(&scope).flatten(),
            config: scope_config.remove(&scope).flatten(),
            scope: PathBuf::from(scope),
        })
        .collect();

    DopplerStatus {
        authenticated,
        scoped_bindings,
        default_project,
        default_config,
    }
}

/// Run `doppler configure --all` and parse the result into a [`DopplerStatus`].
///
/// A non-zero exit that looks like "not authenticated" is mapped to
/// [`DopplerError::NotAuthenticated`]; all other non-zero exits become
/// [`DopplerError::NonZeroExit`].
pub async fn read_status(runner: Arc<dyn CommandRunner>) -> Result<DopplerStatus, DopplerError> {
    let output = runner.run(&["configure", "--all"]).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let lower = stderr.to_lowercase();
        if lower.contains("not authenticated") || lower.contains("you must login") {
            return Err(DopplerError::NotAuthenticated);
        }
        return Err(DopplerError::NonZeroExit {
            code: output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(parse_configure_all(&stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_status() -> DopplerStatus {
        DopplerStatus {
            authenticated: false,
            scoped_bindings: vec![],
            default_project: None,
            default_config: None,
        }
    }

    #[test]
    fn empty_string_returns_zero_state() {
        assert_eq!(parse_configure_all(""), empty_status());
    }

    #[test]
    fn only_borders_and_header_returns_zero_state() {
        let output = "\
┌─────────┬─────────┬─────────┐
│ Scope   │ Name    │ Value   │
├─────────┼─────────┼─────────┤
└─────────┴─────────┴─────────┘
";
        assert_eq!(parse_configure_all(output), empty_status());
    }

    #[test]
    fn authenticated_token_in_default_scope() {
        let output = "\
┌───────────┬─────────┬─────────────────┐
│ Scope     │ Name    │ Value           │
├───────────┼─────────┼─────────────────┤
│ (default) │ token   │ dp.pt.abc123    │
└───────────┴─────────┴─────────────────┘
";
        let status = parse_configure_all(output);
        assert!(status.authenticated);
        assert!(status.scoped_bindings.is_empty());
        assert_eq!(status.default_project, None);
        assert_eq!(status.default_config, None);
    }

    #[test]
    fn single_scoped_binding_scope_blank_on_continuation() {
        // Common layout: scope only on first row of a group.
        let output = "\
┌─────────────────┬─────────┬─────────────┐
│ Scope           │ Name    │ Value       │
├─────────────────┼─────────┼─────────────┤
│ /home/u/app     │ project │ my-app      │
│                 │ config  │ dev         │
└─────────────────┴─────────┴─────────────┘
";
        let status = parse_configure_all(output);
        assert!(!status.authenticated);
        assert_eq!(
            status.scoped_bindings,
            vec![ScopedBinding {
                scope: PathBuf::from("/home/u/app"),
                project: Some("my-app".into()),
                config: Some("dev".into()),
            }]
        );
    }

    #[test]
    fn single_scoped_binding_scope_repeated_every_row() {
        // Alternate layout: scope repeated on every row.
        let output = "\
┌─────────────────┬─────────┬─────────────┐
│ Scope           │ Name    │ Value       │
├─────────────────┼─────────┼─────────────┤
│ /home/u/app     │ project │ my-app      │
│ /home/u/app     │ config  │ dev         │
└─────────────────┴─────────┴─────────────┘
";
        let status = parse_configure_all(output);
        assert_eq!(status.scoped_bindings.len(), 1);
        assert_eq!(status.scoped_bindings[0].project, Some("my-app".into()));
        assert_eq!(status.scoped_bindings[0].config, Some("dev".into()));
    }

    #[test]
    fn multiple_scoped_bindings_preserve_order() {
        let output = "\
┌─────────────────┬─────────┬─────────────┐
│ Scope           │ Name    │ Value       │
├─────────────────┼─────────┼─────────────┤
│ /home/u/alpha   │ project │ alpha-proj  │
│                 │ config  │ dev         │
├─────────────────┼─────────┼─────────────┤
│ /home/u/beta    │ project │ beta-proj   │
│                 │ config  │ staging     │
└─────────────────┴─────────┴─────────────┘
";
        let status = parse_configure_all(output);
        assert_eq!(status.scoped_bindings.len(), 2);
        assert_eq!(status.scoped_bindings[0].scope, PathBuf::from("/home/u/alpha"));
        assert_eq!(status.scoped_bindings[0].project, Some("alpha-proj".into()));
        assert_eq!(status.scoped_bindings[0].config, Some("dev".into()));
        assert_eq!(status.scoped_bindings[1].scope, PathBuf::from("/home/u/beta"));
        assert_eq!(status.scoped_bindings[1].config, Some("staging".into()));
    }

    #[test]
    fn full_table_authenticated_with_binding_and_defaults() {
        let output = "\
┌─────────────────┬─────────┬─────────────┐
│ Scope           │ Name    │ Value       │
├─────────────────┼─────────┼─────────────┤
│ /home/u/app     │ project │ my-app      │
│                 │ config  │ dev         │
├─────────────────┼─────────┼─────────────┤
│ (default)       │ token   │ dp.st.tok   │
│                 │ project │ default-p   │
│                 │ config  │ prd         │
└─────────────────┴─────────┴─────────────┘
";
        let status = parse_configure_all(output);
        assert!(status.authenticated);
        assert_eq!(status.default_project, Some("default-p".into()));
        assert_eq!(status.default_config, Some("prd".into()));
        assert_eq!(status.scoped_bindings.len(), 1);
        assert_eq!(status.scoped_bindings[0].project, Some("my-app".into()));
    }

    #[test]
    fn partial_binding_project_only() {
        let output = "\
┌─────────────────┬─────────┬─────────────┐
│ Scope           │ Name    │ Value       │
├─────────────────┼─────────┼─────────────┤
│ /home/u/app     │ project │ my-app      │
└─────────────────┴─────────┴─────────────┘
";
        let status = parse_configure_all(output);
        assert_eq!(status.scoped_bindings.len(), 1);
        assert_eq!(status.scoped_bindings[0].project, Some("my-app".into()));
        assert_eq!(status.scoped_bindings[0].config, None);
    }

    #[test]
    fn legacy_enclave_prefix_normalised() {
        let output = "\
┌─────────────────┬──────────────────┬─────────────┐
│ Scope           │ Name             │ Value       │
├─────────────────┼──────────────────┼─────────────┤
│ /home/u/app     │ enclave.project  │ old-app     │
│                 │ enclave.config   │ dev         │
└─────────────────┴──────────────────┴─────────────┘
";
        let status = parse_configure_all(output);
        assert_eq!(status.scoped_bindings.len(), 1);
        assert_eq!(status.scoped_bindings[0].project, Some("old-app".into()));
        assert_eq!(status.scoped_bindings[0].config, Some("dev".into()));
    }

    #[test]
    fn unauthenticated_when_no_token_in_default_scope() {
        let output = "\
┌─────────────────┬─────────┬─────────────┐
│ Scope           │ Name    │ Value       │
├─────────────────┼─────────┼─────────────┤
│ /home/u/app     │ project │ my-app      │
│                 │ config  │ dev         │
└─────────────────┴─────────┴─────────────┘
";
        let status = parse_configure_all(output);
        assert!(!status.authenticated);
    }
}
