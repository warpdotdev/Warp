//! Three-layer prompt composer for Helm agents.
//!
//! Helm prompts are assembled from three layers:
//!
//! 1. **Base** (always present) — locked stack decisions: AI Gateway routing,
//!    model→role mapping, AGPL stance, etc.
//! 2. **Role overlay** (one per [`Role`]) — role-specific behaviour.
//! 3. **Project overlay** (optional, per-repo `WARP.md`) — project-specific
//!    rules a particular tree wants to layer on top.
//!
//! Layers are concatenated with explicit `## Layer: <kind>` headers so that
//! downstream consumers can introspect provenance if they need to.

#[cfg(feature = "dev")]
pub mod hot_reload;

use std::path::{Path, PathBuf};

use orchestrator::Role;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A fully composed prompt, ready to be sent to a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposedPrompt {
    /// The concatenated system prompt.
    pub system: String,
    /// Per-layer provenance, in the order they were concatenated.
    pub layers: Vec<LayerSnapshot>,
}

/// Provenance metadata for a single composed layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSnapshot {
    /// Which layer kind this snapshot describes.
    pub kind: LayerKind,
    /// Source identifier — typically the path the layer was read from.
    pub source: String,
    /// Number of bytes contributed (after trimming) by this layer.
    pub bytes: usize,
}

/// Identifies the kind of a composed layer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayerKind {
    /// The base prompt — always present.
    Base,
    /// A role-specific overlay — one per [`Role`].
    Role,
    /// An optional project-level overlay (e.g. `WARP.md`).
    Project,
}

/// Errors that can occur while composing a prompt.
#[derive(Debug, Error)]
pub enum PromptError {
    /// The base prompt template is missing on disk.
    #[error("base prompt missing at {0}")]
    BaseMissing(String),
    /// An I/O error occurred while reading a layer.
    #[error("io error reading {path}: {error}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// Underlying error message.
        error: String,
    },
    /// The role overlay template is missing on disk.
    #[error("role overlay missing for {0:?}")]
    RoleMissing(Role),
}

/// Configuration for the [`Composer`] — paths to where each layer lives.
#[derive(Debug, Clone)]
pub struct ComposerConfig {
    /// Path to the base prompt (e.g. `crates/prompts/templates/base.md`).
    pub base_path: PathBuf,
    /// Directory containing role overlays (e.g. `crates/prompts/templates/roles/`).
    pub role_overlay_dir: PathBuf,
    /// Optional path to the project overlay (e.g. `<project_root>/WARP.md`).
    pub project_overlay_path: Option<PathBuf>,
}

/// Three-layer prompt composer.
pub struct Composer {
    config: ComposerConfig,
}

impl Composer {
    /// Construct a new composer with the given configuration.
    pub fn new(config: ComposerConfig) -> Self {
        Self { config }
    }

    /// Compose a system prompt for `role`.
    ///
    /// Reads the base layer, the role overlay, and (if configured and present)
    /// the project overlay, then concatenates them with explicit section
    /// headers. Strict — base and role MUST exist; project is optional.
    pub async fn compose(&self, role: Role) -> Result<ComposedPrompt, PromptError> {
        let mut sections: Vec<String> = Vec::with_capacity(3);
        let mut layers: Vec<LayerSnapshot> = Vec::with_capacity(3);

        // --- Base layer (required) ---
        let base = read_required(&self.config.base_path).await.map_err(|e| match e {
            ReadError::NotFound => {
                PromptError::BaseMissing(self.config.base_path.display().to_string())
            }
            ReadError::Io(msg) => PromptError::Io {
                path: self.config.base_path.display().to_string(),
                error: msg,
            },
        })?;
        let base = base.trim().to_string();
        layers.push(LayerSnapshot {
            kind: LayerKind::Base,
            source: self.config.base_path.display().to_string(),
            bytes: base.len(),
        });
        sections.push(format!("## Layer: Base\n\n{}", base));

        // --- Role layer (required) ---
        let role_path = self.config.role_overlay_dir.join(role_filename(role));
        let role_text = read_required(&role_path).await.map_err(|e| match e {
            ReadError::NotFound => PromptError::RoleMissing(role),
            ReadError::Io(msg) => PromptError::Io {
                path: role_path.display().to_string(),
                error: msg,
            },
        })?;
        let role_text = role_text.trim().to_string();
        layers.push(LayerSnapshot {
            kind: LayerKind::Role,
            source: role_path.display().to_string(),
            bytes: role_text.len(),
        });
        sections.push(format!("## Layer: Role\n\n{}", role_text));

        // --- Project layer (optional) ---
        if let Some(project_path) = &self.config.project_overlay_path {
            match read_optional(project_path).await {
                Ok(Some(text)) => {
                    let text = text.trim().to_string();
                    layers.push(LayerSnapshot {
                        kind: LayerKind::Project,
                        source: project_path.display().to_string(),
                        bytes: text.len(),
                    });
                    sections.push(format!("## Layer: Project\n\n{}", text));
                }
                Ok(None) => {
                    tracing::debug!(
                        path = %project_path.display(),
                        "project overlay not found, skipping",
                    );
                }
                Err(msg) => {
                    return Err(PromptError::Io {
                        path: project_path.display().to_string(),
                        error: msg,
                    });
                }
            }
        } else {
            tracing::debug!("no project overlay configured, skipping");
        }

        let system = sections.join("\n\n");
        Ok(ComposedPrompt { system, layers })
    }
}

/// Map a [`Role`] to its expected overlay filename (snake_case + `.md`).
fn role_filename(role: Role) -> String {
    let stem = match role {
        Role::Planner => "planner",
        Role::Reviewer => "reviewer",
        Role::Worker => "worker",
        Role::BulkRefactor => "bulk_refactor",
        Role::Summarize => "summarize",
        Role::ToolRouter => "tool_router",
        Role::Inline => "inline",
    };
    format!("{stem}.md")
}

enum ReadError {
    NotFound,
    Io(String),
}

async fn read_required(path: &Path) -> Result<String, ReadError> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(ReadError::NotFound),
        Err(err) => Err(ReadError::Io(err.to_string())),
    }
}

async fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_filename_uses_snake_case() {
        assert_eq!(role_filename(Role::Planner), "planner.md");
        assert_eq!(role_filename(Role::BulkRefactor), "bulk_refactor.md");
        assert_eq!(role_filename(Role::ToolRouter), "tool_router.md");
        assert_eq!(role_filename(Role::Inline), "inline.md");
    }
}
