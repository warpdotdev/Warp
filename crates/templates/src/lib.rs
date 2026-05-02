//! Template loader and variable substitution engine for Helm.
//!
//! A *template* is a directory on disk that contains:
//!
//! * A `template.toml` manifest describing metadata, declared variables, and
//!   post-init hooks (hooks are declared here; execution is handled by
//!   PDX-58).
//! * Any number of files — text or binary — whose paths and contents may
//!   reference variables using `{{variable_name}}` placeholders.
//!
//! At instantiation time the engine copies the template tree to a target
//! directory, substituting every placeholder with the caller-supplied value.
//! Unknown placeholders are left intact, so partial substitution and default
//! value injection are both possible on the caller's side.
//!
//! # Example
//!
//! ```no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use std::collections::HashMap;
//! use templates::Template;
//!
//! let template = Template::load("/path/to/cloudflare-fullstack").await?;
//! let context = HashMap::from([
//!     ("project_name".to_string(), "my-api".to_string()),
//!     ("author".to_string(), "Alice".to_string()),
//! ]);
//! template.instantiate("/projects/my-api", &context).await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, warn};
use walkdir::WalkDir;

// ────────────────────────────────────────────── manifest types ──

/// Manifest parsed from a template's `template.toml` file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateManifest {
    /// Short identifier shown in the UI (e.g. `"cloudflare-fullstack"`).
    pub name: String,
    /// One-line human description of the template.
    #[serde(default)]
    pub description: String,
    /// SemVer version string (e.g. `"0.1.0"`).
    #[serde(default)]
    pub version: String,
    /// Author name or contact email.
    #[serde(default)]
    pub author: String,
    /// Variable definitions — the set of `{{placeholders}}` the user must or
    /// may supply before instantiation.
    #[serde(default)]
    pub variables: Vec<VariableDef>,
    /// Post-init shell hooks. Declared here; executed by the hook runner
    /// (PDX-58).
    #[serde(default)]
    pub hooks: Hooks,
}

/// A single substitution variable declared in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDef {
    /// Name as it appears inside `{{name}}` placeholders.
    pub name: String,
    /// Human-readable prompt shown to the user when collecting the value.
    #[serde(default)]
    pub description: String,
    /// Pre-filled default; an empty string means "no default".
    #[serde(default)]
    pub default: String,
    /// When `true`, the instantiation call returns
    /// [`TemplateError::MissingVariable`] if no non-empty value is supplied.
    #[serde(default)]
    pub required: bool,
}

/// Post-init hook commands to run after scaffolding completes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Hooks {
    /// Commands executed in order from the target directory after all files
    /// are written.
    #[serde(default)]
    pub post_init: Vec<String>,
}

// ────────────────────────────────────────────── errors ──

/// Errors produced by template loading and instantiation.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// An I/O error occurred while reading or writing template files.
    #[error("io error at {path}: {error}")]
    Io { path: String, error: String },

    /// `template.toml` could not be parsed.
    #[error("manifest parse error in {path}: {error}")]
    ManifestParse { path: String, error: String },

    /// A variable marked `required = true` was absent or empty in the context.
    #[error("missing required variable: {name}")]
    MissingVariable { name: String },

    /// The target directory already exists and is non-empty.
    #[error("target directory is not empty: {path}")]
    TargetNotEmpty { path: String },
}

// ────────────────────────────────────────────── Template ──

/// A template loaded from disk, ready to be instantiated.
#[derive(Debug, Clone)]
pub struct Template {
    /// Parsed manifest.
    pub manifest: TemplateManifest,
    /// Root directory the template was loaded from.
    pub source_dir: PathBuf,
    /// Paths of every non-manifest file, relative to `source_dir`, sorted.
    pub files: Vec<PathBuf>,
}

impl Template {
    /// Load a template from `dir`.
    ///
    /// `dir` must contain a `template.toml` manifest. Every other regular
    /// file in the tree is catalogued as a template file and will be
    /// substituted on [`instantiate`](Self::instantiate).
    pub async fn load(dir: impl AsRef<Path>) -> Result<Self, TemplateError> {
        let dir = dir.as_ref();
        let manifest_path = dir.join("template.toml");

        let manifest_text = tokio::fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| TemplateError::Io {
                path: manifest_path.display().to_string(),
                error: e.to_string(),
            })?;

        let manifest =
            toml_edit::de::from_str::<TemplateManifest>(&manifest_text).map_err(|e| {
                TemplateError::ManifestParse {
                    path: manifest_path.display().to_string(),
                    error: e.to_string(),
                }
            })?;

        debug!(name = %manifest.name, dir = %dir.display(), "loaded template manifest");

        let files = collect_template_files(dir)?;

        Ok(Self {
            manifest,
            source_dir: dir.to_path_buf(),
            files,
        })
    }

    /// Instantiate the template into `target_dir`.
    ///
    /// `target_dir` must not exist or must be an empty directory; if it is
    /// non-empty the call returns [`TemplateError::TargetNotEmpty`] before
    /// touching the filesystem.
    ///
    /// `{{variable_name}}` placeholders are substituted in both file
    /// **contents** and **paths** (directory segments and filenames).
    /// Placeholders absent from `context` are preserved verbatim.
    ///
    /// Required variables (manifest entries with `required = true`) that are
    /// missing or empty in `context` cause an early
    /// [`TemplateError::MissingVariable`] error — no files are written.
    ///
    /// Binary files (those that are not valid UTF-8) are copied unchanged;
    /// path substitution still applies to their names.
    ///
    /// Returns absolute paths of all written files.
    pub async fn instantiate(
        &self,
        target_dir: impl AsRef<Path>,
        context: &HashMap<String, String>,
    ) -> Result<Vec<PathBuf>, TemplateError> {
        let target_dir = target_dir.as_ref();

        // Validate required variables before touching the filesystem.
        for var in &self.manifest.variables {
            if var.required {
                match context.get(&var.name) {
                    Some(v) if !v.is_empty() => {}
                    _ => {
                        return Err(TemplateError::MissingVariable {
                            name: var.name.clone(),
                        })
                    }
                }
            }
        }

        // Refuse to overwrite a non-empty target.
        if target_dir.exists() && is_non_empty_dir(target_dir)? {
            return Err(TemplateError::TargetNotEmpty {
                path: target_dir.display().to_string(),
            });
        }

        tokio::fs::create_dir_all(target_dir)
            .await
            .map_err(|e| TemplateError::Io {
                path: target_dir.display().to_string(),
                error: e.to_string(),
            })?;

        let mut written = Vec::with_capacity(self.files.len());

        for rel in &self.files {
            // Substitute variables in the relative path (dir segments + filename).
            let rel_str = rel.to_string_lossy();
            let rendered_rel = substitute(&rel_str, context);
            let dest = target_dir.join(Path::new(&*rendered_rel));

            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| TemplateError::Io {
                        path: parent.display().to_string(),
                        error: e.to_string(),
                    })?;
            }

            let src = self.source_dir.join(rel);
            let bytes = tokio::fs::read(&src).await.map_err(|e| TemplateError::Io {
                path: src.display().to_string(),
                error: e.to_string(),
            })?;

            // Only apply text substitution to valid UTF-8 content.
            let out_bytes = match std::str::from_utf8(&bytes) {
                Ok(text) => substitute(text, context).into_bytes(),
                Err(_) => {
                    warn!(path = %rel.display(), "skipping substitution on non-UTF-8 file");
                    bytes
                }
            };

            tokio::fs::write(&dest, &out_bytes)
                .await
                .map_err(|e| TemplateError::Io {
                    path: dest.display().to_string(),
                    error: e.to_string(),
                })?;

            debug!(dest = %dest.display(), "wrote template file");
            written.push(dest);
        }

        Ok(written)
    }
}

// ────────────────────────────────────────────── substitution engine ──

/// Replace every `{{name}}` placeholder in `input` with its value from
/// `context`.
///
/// Placeholders whose name is not present in `context` are copied to the
/// output unchanged, allowing callers to apply defaults in a second pass.
///
/// Triple-brace sequences `{{{name}}}` are treated as escape markers and
/// emitted verbatim (the inner `{{name}}` is not substituted).
///
/// A placeholder name must match `[a-zA-Z_-][a-zA-Z0-9_-]*`; anything else
/// (e.g. embedded whitespace or nested braces) is passed through unchanged.
pub fn substitute(input: &str, context: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut remaining = input;

    while !remaining.is_empty() {
        match remaining.find("{{") {
            None => {
                out.push_str(remaining);
                break;
            }
            Some(open_pos) => {
                out.push_str(&remaining[..open_pos]);
                remaining = &remaining[open_pos..];

                if remaining.starts_with("{{{") {
                    // Escaped triple-brace block: emit verbatim.
                    match remaining[3..].find("}}}") {
                        Some(close_rel) => {
                            let end = close_rel + 6; // 3 open + content + 3 close
                            out.push_str(&remaining[..end]);
                            remaining = &remaining[end..];
                        }
                        None => {
                            // Unclosed triple — emit one `{` and retry.
                            out.push('{');
                            remaining = &remaining[1..];
                        }
                    }
                } else {
                    let after_open = &remaining[2..];
                    match after_open.find("}}") {
                        Some(close_rel) => {
                            let name = &after_open[..close_rel];
                            if is_valid_placeholder(name) {
                                match context.get(name) {
                                    Some(value) => out.push_str(value),
                                    None => out.push_str(&remaining[..close_rel + 4]),
                                }
                                remaining = &after_open[close_rel + 2..];
                            } else {
                                // Invalid placeholder — emit `{{` and retry.
                                out.push_str("{{");
                                remaining = after_open;
                            }
                        }
                        None => {
                            // No closing `}}` — emit `{{` and move on.
                            out.push_str("{{");
                            remaining = after_open;
                        }
                    }
                }
            }
        }
    }

    out
}

// ────────────────────────────────────────────── helpers ──

/// Walk `dir` and return relative paths of every non-manifest file, sorted.
fn collect_template_files(dir: &Path) -> Result<Vec<PathBuf>, TemplateError> {
    let mut files = Vec::new();

    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| e.file_name().to_string_lossy() != ".git")
    {
        let entry = entry.map_err(|e| TemplateError::Io {
            path: dir.display().to_string(),
            error: e.to_string(),
        })?;

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.into_path();

        // Exclude the manifest itself.
        if path.file_name().and_then(|s| s.to_str()) == Some("template.toml") {
            continue;
        }

        let rel = path
            .strip_prefix(dir)
            .expect("walkdir yields paths under root")
            .to_path_buf();
        files.push(rel);
    }

    files.sort();
    Ok(files)
}

fn is_non_empty_dir(dir: &Path) -> Result<bool, TemplateError> {
    let mut entries = std::fs::read_dir(dir).map_err(|e| TemplateError::Io {
        path: dir.display().to_string(),
        error: e.to_string(),
    })?;
    Ok(entries.next().is_some())
}

/// Return `true` iff `s` is a valid placeholder identifier:
/// `[a-zA-Z_-][a-zA-Z0-9_-]*`
fn is_valid_placeholder(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        None => false,
        Some(first) => {
            (first.is_alphabetic() || first == '_' || first == '-')
                && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        }
    }
}

// ────────────────────────────────────────────── unit tests ──

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
