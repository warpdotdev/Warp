// SPDX-License-Identifier: AGPL-3.0-only
//
// Template repo sync for Helm (PDX-59).
//
// On app launch, pulls each configured template repo into
// `~/.warp/templates/<slug>/`. If a repo has not been cloned yet, clones it
// with `--depth=1`. If it has been cloned, fast-forward pulls the latest
// changes. Uses the system `git` binary via `tokio::process::Command`.
//
// Configuration is read from `~/.warp/templates.toml`. When the config file
// is absent or contains no `[[repos]]` entries, a built-in list of default
// repos is used instead. All failures are logged as warnings and never block
// app startup.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

/// A configured template repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRepo {
    /// Git clone URL.
    pub url: String,
    /// Local directory name under the templates cache dir. Defaults to the
    /// last path component of `url` with any `.git` suffix stripped.
    pub slug: Option<String>,
}

impl TemplateRepo {
    /// Returns the effective cache-directory slug for this repo.
    pub fn effective_slug(&self) -> String {
        if let Some(slug) = &self.slug {
            return slug.clone();
        }
        url_to_slug(&self.url)
    }
}

/// Root config deserialized from `~/.warp/templates.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplatesConfig {
    /// Repos to clone / pull on launch.
    #[serde(default)]
    pub repos: Vec<TemplateRepo>,
}

/// Errors produced by individual sync operations. These are logged as
/// warnings; they never propagate out of [`sync_all`].
#[derive(Debug, Error)]
pub enum SyncError {
    /// `git clone` exited non-zero.
    #[error("git clone failed for {url}: {detail}")]
    CloneFailed { url: String, detail: String },

    /// `git pull` exited non-zero.
    #[error("git pull failed for {slug}: {detail}")]
    PullFailed { slug: String, detail: String },

    /// Filesystem or process-spawn error.
    #[error("io error at {path}: {error}")]
    Io { path: String, error: String },
}

/// Built-in template repos that ship with v1 of the Helm templates system
/// (PDX-9). Used when `~/.warp/templates.toml` is absent or empty.
const DEFAULT_REPOS: &[(&str, &str)] = &[
    (
        "https://github.com/warpdotdev/template-cloudflare-fullstack",
        "cloudflare-fullstack",
    ),
    (
        "https://github.com/warpdotdev/template-apple-multiplatform",
        "apple-multiplatform",
    ),
];

/// Pull all configured template repos into `~/.warp/templates/`.
///
/// This is the top-level entry point called from app startup. It runs
/// entirely in the background and never blocks the UI. All errors are logged
/// as tracing warnings.
pub async fn sync_all() {
    let Some(cache_dir) = warp_core::paths::warp_home_templates_dir() else {
        warn!("could not resolve templates cache dir (no home directory?)");
        return;
    };

    let repos = load_config_or_defaults(&cache_dir).await;
    for repo in &repos {
        let slug = repo.effective_slug();
        let dest = cache_dir.join(&slug);
        match clone_or_pull(&repo.url, &dest).await {
            Ok(action) => info!("template {slug}: {action}"),
            Err(e) => warn!("template sync: {e}"),
        }
    }
}

/// Read `~/.warp/templates.toml`. Returns the configured repo list when the
/// file exists and parses correctly and contains at least one repo. Falls
/// back to [`DEFAULT_REPOS`] in all other cases.
async fn load_config_or_defaults(cache_dir: &Path) -> Vec<TemplateRepo> {
    // The config file sits one level above the cache dir: ~/.warp/templates.toml
    let config_path: Option<PathBuf> = cache_dir.parent().map(|p| p.join("templates.toml"));

    if let Some(path) = config_path {
        match tokio::fs::read_to_string(&path).await {
            Ok(text) => match toml::from_str::<TemplatesConfig>(&text) {
                Ok(cfg) if !cfg.repos.is_empty() => {
                    debug!(
                        config = %path.display(),
                        repos = cfg.repos.len(),
                        "loaded templates config"
                    );
                    return cfg.repos;
                }
                Ok(_) => {
                    debug!(
                        "templates.toml contains no repos; falling back to defaults"
                    );
                }
                Err(e) => {
                    warn!("failed to parse {}: {e}", path.display());
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("no templates.toml found, using built-in defaults");
            }
            Err(e) => {
                warn!("failed to read {}: {e}", path.display());
            }
        }
    }

    DEFAULT_REPOS
        .iter()
        .map(|(url, slug)| TemplateRepo {
            url: url.to_string(),
            slug: Some(slug.to_string()),
        })
        .collect()
}

/// Clone `url` into `dest` if it does not yet exist, or fast-forward pull
/// the latest commits if it does. Returns a short action label on success.
async fn clone_or_pull(url: &str, dest: &Path) -> Result<String, SyncError> {
    if dest.exists() {
        pull(dest).await?;
        Ok("pulled".to_string())
    } else {
        clone(url, dest).await?;
        Ok("cloned".to_string())
    }
}

async fn clone(url: &str, dest: &Path) -> Result<(), SyncError> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| SyncError::Io {
                path: parent.display().to_string(),
                error: e.to_string(),
            })?;
    }

    debug!(url, dest = %dest.display(), "cloning template repo");
    let output = tokio::process::Command::new("git")
        .args([
            "clone",
            "--depth=1",
            url,
            dest.to_str().unwrap_or("."),
        ])
        .output()
        .await
        .map_err(|e| SyncError::Io {
            path: "git".to_string(),
            error: e.to_string(),
        })?;

    if !output.status.success() {
        return Err(SyncError::CloneFailed {
            url: url.to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(())
}

async fn pull(dest: &Path) -> Result<(), SyncError> {
    debug!(dest = %dest.display(), "pulling template repo");
    let output = tokio::process::Command::new("git")
        .args([
            "-C",
            dest.to_str().unwrap_or("."),
            "pull",
            "--ff-only",
        ])
        .output()
        .await
        .map_err(|e| SyncError::Io {
            path: "git".to_string(),
            error: e.to_string(),
        })?;

    if !output.status.success() {
        let slug = dest
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        return Err(SyncError::PullFailed {
            slug,
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(())
}

/// Derive a cache-directory slug from a Git URL.
///
/// Takes the last path component and strips any `.git` suffix.
fn url_to_slug(url: &str) -> String {
    let last = url.trim_end_matches('/').rsplit('/').next().unwrap_or(url);
    last.strip_suffix(".git").unwrap_or(last).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_to_slug_strips_git_suffix() {
        assert_eq!(
            url_to_slug("https://github.com/org/my-template.git"),
            "my-template"
        );
        assert_eq!(
            url_to_slug("https://github.com/org/my-template"),
            "my-template"
        );
    }

    #[test]
    fn url_to_slug_trailing_slash() {
        assert_eq!(
            url_to_slug("https://github.com/org/my-template/"),
            "my-template"
        );
    }

    #[test]
    fn effective_slug_uses_explicit_value() {
        let repo = TemplateRepo {
            url: "https://github.com/org/foo.git".to_string(),
            slug: Some("bar".to_string()),
        };
        assert_eq!(repo.effective_slug(), "bar");
    }

    #[test]
    fn effective_slug_derives_from_url() {
        let repo = TemplateRepo {
            url: "https://github.com/org/my-template.git".to_string(),
            slug: None,
        };
        assert_eq!(repo.effective_slug(), "my-template");
    }

    #[test]
    fn templates_config_deserializes_empty() {
        let cfg: TemplatesConfig = toml::from_str("").unwrap();
        assert!(cfg.repos.is_empty());
    }

    #[test]
    fn templates_config_deserializes_repos() {
        let text = r#"
[[repos]]
url = "https://github.com/org/tmpl-a"
slug = "tmpl-a"

[[repos]]
url = "https://github.com/org/tmpl-b.git"
"#;
        let cfg: TemplatesConfig = toml::from_str(text).unwrap();
        assert_eq!(cfg.repos.len(), 2);
        assert_eq!(cfg.repos[0].effective_slug(), "tmpl-a");
        assert_eq!(cfg.repos[1].effective_slug(), "tmpl-b");
    }
}
