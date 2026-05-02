//! Skill metadata: runtime statistics and user-supplied overrides.
//!
//! [`SkillMetadata`] captures the fields that cannot live inside a skill's
//! markdown file because they are either computed at runtime or set by the
//! user after the skill is installed:
//!
//! * `last_used` — wall-clock timestamp of the most recent invocation.
//! * `success_rate` — fraction of invocations that completed without error.
//! * `total_tokens` — cumulative token count across all invocations.
//! * `tool_call_count` — cumulative tool-call count across all invocations.
//! * `user_tags` — tags the user adds without editing the skill source file.
//! * `user_description` — a user-supplied description that overrides the one
//!   derived from the front matter or skill body.
//!
//! Persistence is handled by [`MetadataStore`], which serialises the full
//! collection to a single JSON file keyed by skill name. Missing entries are
//! silently treated as [`SkillMetadata::default()`].

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Runtime statistics and user-supplied overrides for a single skill.
///
/// All fields default to zero / empty / `None` so that a freshly-seen skill
/// behaves sensibly without any stored state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SkillMetadata {
    /// Wall-clock timestamp of the most recent invocation.
    ///
    /// `None` means the skill has never been invoked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<DateTime<Utc>>,

    /// Fraction of invocations that completed without error, in `[0.0, 1.0]`.
    ///
    /// `None` means no invocations have been recorded yet and no rate can be
    /// computed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_rate: Option<f32>,

    /// Cumulative number of tokens consumed across all invocations.
    #[serde(default)]
    pub total_tokens: u64,

    /// Cumulative number of tool calls made across all invocations.
    #[serde(default)]
    pub tool_call_count: u64,

    /// User-supplied tags that augment (not replace) the front-matter tags.
    ///
    /// These allow users to organise skills without modifying their source
    /// files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_tags: Vec<String>,

    /// User-supplied description.
    ///
    /// When present this takes precedence over the description inferred from
    /// the front matter or from the first non-blank body line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_description: Option<String>,
}

impl SkillMetadata {
    /// Return the effective description for this skill.
    ///
    /// Prefers `user_description` over the fallback supplied by the caller
    /// (which is typically the value derived from the skill's front matter).
    pub fn effective_description<'a>(&'a self, fallback: &'a str) -> &'a str {
        self.user_description.as_deref().unwrap_or(fallback)
    }

    /// Return the merged tag list for this skill.
    ///
    /// Combines `base_tags` (from front matter) with `user_tags`, deduplicating
    /// while preserving the order: front-matter tags first, then any user tags
    /// not already present.
    pub fn effective_tags(&self, base_tags: &[String]) -> Vec<String> {
        let mut seen: std::collections::HashSet<&str> =
            base_tags.iter().map(String::as_str).collect();
        let mut result: Vec<String> = base_tags.to_vec();
        for tag in &self.user_tags {
            if seen.insert(tag.as_str()) {
                result.push(tag.clone());
            }
        }
        result
    }
}

/// Errors produced by [`MetadataStore`] operations.
#[derive(Debug, Error)]
pub enum MetadataStoreError {
    /// IO error while reading or writing the store file.
    #[error("io error for {path}: {error}")]
    Io {
        /// Path that triggered the error.
        path: String,
        /// Stringified underlying error.
        error: String,
    },
    /// The store file contained invalid JSON.
    #[error("json error for {path}: {error}")]
    Json {
        /// Path of the offending file.
        path: String,
        /// Stringified underlying parse error.
        error: String,
    },
}

/// Persistent store for [`SkillMetadata`], keyed by skill name.
///
/// The store is backed by a single JSON file whose top-level value is an
/// object mapping `skill_name → SkillMetadata`. Skills with no stored entry
/// are treated as having [`SkillMetadata::default()`].
///
/// ```no_run
/// # use skills::metadata::{MetadataStore, SkillMetadata};
/// # use chrono::Utc;
/// # #[tokio::main] async fn main() {
/// let path = std::path::Path::new("/tmp/skill_metadata.json");
///
/// // Load (or create empty store if the file does not exist yet).
/// let mut store = MetadataStore::load(path).await.unwrap();
///
/// // Record a usage event.
/// let mut meta = store.get("my-skill").cloned().unwrap_or_default();
/// meta.last_used = Some(Utc::now());
/// meta.total_tokens += 512;
/// meta.tool_call_count += 3;
/// meta.success_rate = Some(1.0);
/// store.set("my-skill", meta);
///
/// // Persist.
/// store.save(path).await.unwrap();
/// # }
/// ```
#[derive(Debug, Default)]
pub struct MetadataStore {
    records: HashMap<String, SkillMetadata>,
}

impl MetadataStore {
    /// Load from `path`.
    ///
    /// Returns an empty store if the file does not exist yet. Returns an error
    /// if the file exists but cannot be read or parsed.
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, MetadataStoreError> {
        let path = path.as_ref();
        if !tokio::fs::try_exists(path).await.unwrap_or(false) {
            return Ok(Self::default());
        }
        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| MetadataStoreError::Io {
                path: path.display().to_string(),
                error: e.to_string(),
            })?;
        let records: HashMap<String, SkillMetadata> =
            serde_json::from_str(&raw).map_err(|e| MetadataStoreError::Json {
                path: path.display().to_string(),
                error: e.to_string(),
            })?;
        Ok(Self { records })
    }

    /// Persist the current state to `path`.
    ///
    /// Creates parent directories if they do not exist.
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<(), MetadataStoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| MetadataStoreError::Io {
                        path: parent.display().to_string(),
                        error: e.to_string(),
                    })?;
            }
        }
        let json =
            serde_json::to_string_pretty(&self.records).map_err(|e| MetadataStoreError::Json {
                path: path.display().to_string(),
                error: e.to_string(),
            })?;
        tokio::fs::write(path, json)
            .await
            .map_err(|e| MetadataStoreError::Io {
                path: path.display().to_string(),
                error: e.to_string(),
            })
    }

    /// Return the metadata for `skill_name`, or `None` if no entry exists.
    pub fn get(&self, skill_name: &str) -> Option<&SkillMetadata> {
        self.records.get(skill_name)
    }

    /// Overwrite the metadata record for `skill_name`.
    pub fn set(&mut self, skill_name: impl Into<String>, metadata: SkillMetadata) {
        self.records.insert(skill_name.into(), metadata);
    }

    /// Remove the metadata record for `skill_name`.
    ///
    /// Returns the removed record, or `None` if no record existed.
    pub fn remove(&mut self, skill_name: &str) -> Option<SkillMetadata> {
        self.records.remove(skill_name)
    }

    /// Iterate over all stored records.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &SkillMetadata)> {
        self.records.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of records in the store.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// True iff the store has no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
