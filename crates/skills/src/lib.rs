//! Skills loader, registry, and usage ledger for Helm agents.
//!
//! The [`ledger`] module provides the append-only write path for recording
//! skill invocations to local SQLite (PDX-71).
//!
//! Skills are markdown files with optional YAML front matter that agents can
//! load contextually based on the current task's role and tags. They live in
//! two locations:
//!
//! * `~/.warp/skills/` — user-global skills.
//! * `<repo>/.agents/skills/` — per-repo skills (override globals on name
//!   conflict).
//!
//! Each skill file looks like:
//!
//! ```markdown
//! ---
//! name: cloudflare-deploy
//! description: Deploy a Worker via wrangler with safe defaults
//! roles: [Worker, BulkRefactor]
//! tags: [cloudflare, deploy]
//! ---
//!
//! # Skill body in markdown
//! ...
//! ```
//!
//! Front matter is optional. When absent the skill name defaults to the
//! filename stem, the description defaults to the first non-blank line of the
//! body, and `roles` / `tags` default to empty (matching every role).

pub mod ledger;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use orchestrator::Role;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, warn};
use walkdir::WalkDir;

/// One skill loaded from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Skill name. Either taken from front matter or derived from the
    /// filename stem.
    pub name: String,
    /// Path the skill was read from.
    pub source_path: PathBuf,
    /// Short human-readable description.
    pub description: String,
    /// Roles this skill applies to. An empty vector means the skill applies
    /// to every role.
    pub roles: Vec<Role>,
    /// Free-form tags used for filter matching.
    pub tags: Vec<String>,
    /// Markdown body (everything after the front matter, if any).
    pub body: String,
}

/// Skill metadata read from optional YAML front matter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillFrontMatter {
    /// Optional name override. Falls back to the filename stem.
    pub name: Option<String>,
    /// Optional one-line description.
    pub description: Option<String>,
    /// Roles this skill applies to (empty = all).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Free-form filter tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Loader configuration: paths to walk.
#[derive(Debug, Clone, Default)]
pub struct LoaderConfig {
    /// User-global skills root (e.g. `~/.warp/skills`). Skills found here are
    /// loaded first and may be overridden by repo-local skills.
    pub user_root: Option<PathBuf>,
    /// Per-repo skills root (e.g. `<repo>/.agents/skills`). Skills found here
    /// override user-global skills with the same name.
    pub repo_root: Option<PathBuf>,
}

/// Skills loaded from disk, indexed by name. Repo skills override user
/// skills on name conflict.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    by_name: HashMap<String, Skill>,
}

/// Errors produced by skill loading.
#[derive(Debug, Error)]
pub enum SkillsError {
    /// IO error while walking directories or reading skill files.
    #[error("io error reading {path}: {error}")]
    Io {
        /// Path that produced the error.
        path: String,
        /// Stringified underlying error.
        error: String,
    },
    /// YAML front matter failed to parse.
    #[error("yaml parse error in {path}: {error}")]
    Yaml {
        /// Path of the offending skill file.
        path: String,
        /// Stringified underlying parse error.
        error: String,
    },
}

impl SkillRegistry {
    /// Load all skills from the configured roots.
    ///
    /// Walks each root recursively, picks up every `*.md` file, parses any
    /// front matter, and indexes the resulting [`Skill`]s by name. The
    /// `user_root` is loaded first so repo-local skills can override it on
    /// name conflicts.
    pub async fn load(config: LoaderConfig) -> Result<Self, SkillsError> {
        let mut by_name: HashMap<String, Skill> = HashMap::new();

        if let Some(root) = config.user_root.as_deref() {
            load_root_into(root, &mut by_name).await?;
        }
        if let Some(root) = config.repo_root.as_deref() {
            // Repo skills override user skills on name conflict — load_root_into
            // unconditionally inserts, which is what we want.
            load_root_into(root, &mut by_name).await?;
        }

        Ok(Self { by_name })
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.by_name.get(name)
    }

    /// Iterator over every loaded skill.
    pub fn all(&self) -> impl Iterator<Item = &Skill> {
        self.by_name.values()
    }

    /// Number of loaded skills.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// True iff no skills are loaded.
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Contextual selection.
    ///
    /// Returns every skill that:
    ///
    /// * has an empty `roles` list (role-agnostic) **or** has `role` listed
    ///   in its `roles`, **and**
    /// * has at least one tag in common with `wanted_tags`, **or** has an
    ///   empty `tags` list, **or** the caller passed an empty
    ///   `wanted_tags` slice.
    pub fn select_for(&self, role: Role, wanted_tags: &[String]) -> Vec<&Skill> {
        self.by_name
            .values()
            .filter(|skill| skill.roles.is_empty() || skill.roles.contains(&role))
            .filter(|skill| {
                if wanted_tags.is_empty() || skill.tags.is_empty() {
                    return true;
                }
                skill.tags.iter().any(|t| wanted_tags.contains(t))
            })
            .collect()
    }
}

/// Walk `root` recursively, parse every `*.md` file as a skill, and insert
/// the result into `by_name`.
///
/// Existing entries are overwritten — callers rely on this for the
/// repo-overrides-user precedence.
async fn load_root_into(
    root: &Path,
    by_name: &mut HashMap<String, Skill>,
) -> Result<(), SkillsError> {
    if !root.exists() {
        debug!(path = %root.display(), "skills root does not exist, skipping");
        return Ok(());
    }

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(root).follow_links(true) {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                return Err(SkillsError::Io {
                    path: root.display().to_string(),
                    error: err.to_string(),
                });
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            paths.push(path);
        }
    }

    for path in paths {
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| SkillsError::Io {
                path: path.display().to_string(),
                error: e.to_string(),
            })?;
        let skill = parse_skill(&path, &raw)?;
        debug!(name = %skill.name, source = %skill.source_path.display(), "loaded skill");
        by_name.insert(skill.name.clone(), skill);
    }

    Ok(())
}

/// Split a markdown file into its optional YAML front matter and body, then
/// resolve the final [`Skill`] fields.
fn parse_skill(path: &Path, raw: &str) -> Result<Skill, SkillsError> {
    let (front_matter, body) = split_front_matter(raw);

    let fm = if let Some(fm_text) = front_matter {
        if fm_text.trim().is_empty() {
            SkillFrontMatter::default()
        } else {
            serde_yaml::from_str::<SkillFrontMatter>(fm_text).map_err(|e| SkillsError::Yaml {
                path: path.display().to_string(),
                error: e.to_string(),
            })?
        }
    } else {
        SkillFrontMatter::default()
    };

    let name = fm.name.unwrap_or_else(|| filename_stem(path));
    let description = fm.description.unwrap_or_else(|| first_nonblank_line(body));
    let roles = parse_roles(&fm.roles, path);
    let tags = fm.tags;

    Ok(Skill {
        name,
        source_path: path.to_path_buf(),
        description,
        roles,
        tags,
        body: body.to_string(),
    })
}

/// Split a markdown source into `(front_matter, body)`.
///
/// Front matter must start at the very first line with `---` and end at the
/// next line that consists solely of `---`. If no front matter is present
/// the whole source is returned as the body.
fn split_front_matter(raw: &str) -> (Option<&str>, &str) {
    // Front matter must start at the first line. Allow a leading BOM but
    // nothing else.
    let stripped = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let rest = match stripped.strip_prefix("---\n") {
        Some(r) => r,
        None => match stripped.strip_prefix("---\r\n") {
            Some(r) => r,
            None => return (None, raw),
        },
    };

    // Find the closing fence — a line that is exactly "---".
    let mut search_start = 0;
    while let Some(rel) = rest[search_start..].find("---") {
        let abs = search_start + rel;
        let at_line_start = abs == 0 || rest.as_bytes()[abs - 1] == b'\n';
        let after = abs + 3;
        let line_terminates =
            after == rest.len() || rest.as_bytes()[after] == b'\n' || rest.as_bytes()[after] == b'\r';
        if at_line_start && line_terminates {
            let fm = &rest[..abs];
            // Strip the trailing newline that belongs to the fence line.
            let fm = fm.strip_suffix('\n').unwrap_or(fm);
            let fm = fm.strip_suffix('\r').unwrap_or(fm);
            // Skip past the fence and its line ending in the body.
            let mut body_start = after;
            if body_start < rest.len() && rest.as_bytes()[body_start] == b'\r' {
                body_start += 1;
            }
            if body_start < rest.len() && rest.as_bytes()[body_start] == b'\n' {
                body_start += 1;
            }
            return (Some(fm), &rest[body_start..]);
        }
        search_start = abs + 3;
    }

    // Opening fence with no closing fence — treat as no front matter so the
    // markdown body is preserved verbatim.
    warn!("front matter opening fence without a closing fence; treating as body");
    (None, raw)
}

fn filename_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

fn first_nonblank_line(body: &str) -> String {
    body.lines()
        .map(|l| l.trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

fn parse_roles(raw: &[String], path: &Path) -> Vec<Role> {
    let mut roles = Vec::with_capacity(raw.len());
    for r in raw {
        match parse_role_str(r) {
            Some(role) => roles.push(role),
            None => warn!(
                role = %r,
                path = %path.display(),
                "ignoring unknown role in skill front matter"
            ),
        }
    }
    roles
}

fn parse_role_str(s: &str) -> Option<Role> {
    match s.trim() {
        "Planner" => Some(Role::Planner),
        "Reviewer" => Some(Role::Reviewer),
        "Worker" => Some(Role::Worker),
        "BulkRefactor" => Some(Role::BulkRefactor),
        "Summarize" => Some(Role::Summarize),
        "ToolRouter" => Some(Role::ToolRouter),
        "Inline" => Some(Role::Inline),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_front_matter_finds_fences() {
        let raw = "---\nname: foo\n---\nbody\n";
        let (fm, body) = split_front_matter(raw);
        assert_eq!(fm, Some("name: foo"));
        assert_eq!(body, "body\n");
    }

    #[test]
    fn split_front_matter_no_front_matter() {
        let raw = "# Just markdown\n\nbody\n";
        let (fm, body) = split_front_matter(raw);
        assert!(fm.is_none());
        assert_eq!(body, raw);
    }

    #[test]
    fn split_front_matter_empty_block() {
        let raw = "---\n---\nhello\n";
        let (fm, body) = split_front_matter(raw);
        assert_eq!(fm, Some(""));
        assert_eq!(body, "hello\n");
    }

    #[test]
    fn first_nonblank_line_strips_heading_marks() {
        assert_eq!(first_nonblank_line("\n\n# Hello there\n"), "Hello there");
        assert_eq!(first_nonblank_line(""), "");
    }
}
