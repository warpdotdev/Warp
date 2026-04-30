use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use ai::skills::{
    home_skills_path, read_skills, ParsedSkill, SkillProvider, SKILL_PROVIDER_DEFINITIONS,
};
use anyhow::Error;
use regex::Regex;
use repo_metadata::{local_model::GetContentsArgs, RepoContent, RepoMetadataModel};
use warpui::AppContext;

use crate::warp_managed_paths_watcher::warp_managed_skill_dirs;

/// Returns all skill directories in the repo tree (e.g. `/repo/.agents/skills/`).
///
/// Uses two passes to handle gitignored provider directories:
///
/// - **Pass 1:** collect fully-loaded dirs ending with a known provider skills path.
/// - **Pass 2:** collect lazy-loaded dirs (`loaded: false`) and probe with `is_dir()`:
///   - *(a)* lazy dir is a provider root (`.agents`, `.claude`, …) → probe `{dir}/skills`.
///   - *(b)* lazy dir is a parent (e.g. gitignored `sub-project/`) → probe
///     `{dir}/{provider_path}` for every known provider.
///
/// Pass 2 scope is bounded by the tree: lazy dirs only appear as children of indexed
/// parents, so subtrees below a lazy node are never reached.
pub fn find_skill_directories_in_tree(
    repo_path: &Path,
    repo_metadata: &RepoMetadataModel,
    ctx: &AppContext,
) -> Vec<PathBuf> {
    let Some(id) = repo_metadata::RepositoryIdentifier::try_local(repo_path) else {
        return Vec::new();
    };

    // Provider skills paths (e.g. ".agents/skills") and root names (e.g. ".agents").
    let skill_path_suffixes: Vec<String> = SKILL_PROVIDER_DEFINITIONS
        .iter()
        .map(|p| p.skills_path.to_string_lossy().into_owned())
        .collect();

    let provider_root_names: HashSet<String> = SKILL_PROVIDER_DEFINITIONS
        .iter()
        .filter_map(|p| {
            p.skills_path
                .parent()
                .and_then(Path::file_name)
                .and_then(|n| n.to_str())
                .map(str::to_owned)
        })
        .collect();

    // Pass 1: loaded dirs whose path ends with a known provider skills suffix.
    let suffixes_1 = skill_path_suffixes.clone();
    let args = GetContentsArgs::default().with_filter(move |content| {
        let RepoContent::Directory(dir) = content else {
            return false;
        };
        suffixes_1
            .iter()
            .any(|suffix| dir.path.ends_with(suffix.as_str()))
    });

    let mut result: Vec<PathBuf> = repo_metadata
        .get_repo_contents(&id, args, ctx)
        .unwrap_or_default()
        .into_iter()
        .map(|content| match content {
            RepoContent::Directory(dir) => dir.path.to_local_path_lossy(),
            RepoContent::File(f) => f.path.to_local_path_lossy(),
        })
        .collect();

    // Pass 2: probe lazy-loaded (gitignored) dirs.
    let mut result_set: HashSet<PathBuf> = result.iter().cloned().collect();
    let args_lazy = GetContentsArgs::default()
        .include_ignored()
        .with_filter(move |content| {
            let RepoContent::Directory(dir) = content else {
                return false;
            };
            !dir.loaded
        });

    let lazy_dirs: Vec<PathBuf> = repo_metadata
        .get_repo_contents(&id, args_lazy, ctx)
        .unwrap_or_default()
        .into_iter()
        .map(|content| match content {
            RepoContent::Directory(dir) => dir.path.to_local_path_lossy(),
            RepoContent::File(f) => f.path.to_local_path_lossy(),
        })
        .collect();

    for lazy_dir in lazy_dirs {
        let dir_name = lazy_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if provider_root_names.contains(dir_name) {
            // Case (a): lazy dir is a provider root — probe {dir}/skills.
            let skills_path = lazy_dir.join("skills");
            if !result_set.contains(&skills_path) && skills_path.is_dir() {
                result_set.insert(skills_path.clone());
                result.push(skills_path);
            }
        } else {
            // Case (b): lazy dir may be a parent of a provider root — probe
            // {dir}/{provider_path} for each known provider.
            for provider in SKILL_PROVIDER_DEFINITIONS.iter() {
                let skills_path = lazy_dir.join(&provider.skills_path);
                if !result_set.contains(&skills_path) && skills_path.is_dir() {
                    result_set.insert(skills_path.clone());
                    result.push(skills_path);
                }
            }
        }
    }

    result
}

/// Reads all skills from the given skill directories.
pub fn read_skills_from_directories(
    skill_dirs: impl IntoIterator<Item = PathBuf>,
) -> Vec<ParsedSkill> {
    skill_dirs
        .into_iter()
        .flat_map(|dir| read_skills(&dir))
        .collect()
}

pub fn is_skill_file(path: &Path) -> bool {
    extract_skill_parent_directory(path).is_ok()
}

static SKILL_PROVIDER_PATHS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    SKILL_PROVIDER_DEFINITIONS
        .iter()
        .map(|p| p.skills_path.to_string_lossy().to_string())
        .collect()
});

// Matches {prefix}/{provider_path}/{skill-name}/SKILL.md (provider_path is 2 components).
#[cfg(not(target_os = "windows"))]
static SKILL_FILE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(.+)/([^/]+/[^/]+)/[^/]+/SKILL\.md$")
        .expect("Failed to compile skill file pattern")
});

#[cfg(target_os = "windows")]
static SKILL_FILE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(.+)\\([^\\]+\\[^\\]+)\\[^\\]+\\SKILL\.md$")
        .expect("Failed to compile skill file pattern")
});

pub fn extract_skill_parent_directory(path: &Path) -> Result<PathBuf, Error> {
    let is_warp_home_skill = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "SKILL.md")
        && path
            .parent()
            .and_then(Path::parent)
            .is_some_and(|parent| warp_managed_skill_dirs().iter().any(|dir| parent == dir));
    if is_warp_home_skill {
        return dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Home directory not available for {}", path.display()));
    }
    let path_str = path.to_string_lossy();

    if let Some(captures) = SKILL_FILE_PATTERN.captures(&path_str) {
        if let Some(provider_path) = captures.get(2) {
            if SKILL_PROVIDER_PATHS.contains(provider_path.as_str()) {
                if let Some(parent_directory) = captures.get(1) {
                    return Ok(PathBuf::from(parent_directory.as_str()));
                }
            }
        }
    }

    Err(anyhow::anyhow!("Not a skill path: {}", path.display()))
}

/// Returns true if `path` is a skill directory under a home provider path
/// (e.g. `~/.agents/skills/skill-name`).
pub fn is_home_skill_directory(path: &Path) -> bool {
    path.parent().is_some_and(is_home_provider_path)
}

/// Returns true if `path` is a home provider skills path (e.g. `~/.agents/skills`).
pub fn is_home_provider_path(path: &Path) -> bool {
    SKILL_PROVIDER_DEFINITIONS.iter().any(|provider| {
        if provider.provider == SkillProvider::Warp {
            return warp_managed_skill_dirs().iter().any(|dir| path == dir);
        }
        home_skills_path(provider.provider)
            .as_ref()
            .is_some_and(|home_skills_path| path == home_skills_path)
    })
}

#[cfg(test)]
#[path = "utils_tests.rs"]
mod tests;
