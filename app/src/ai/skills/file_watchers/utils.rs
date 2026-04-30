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

/// Finds all skill directories in a repository by querying the RepoMetadataModel tree.
///
/// Returns a list of paths to skill directories (e.g., `/repo/.agents/skills/`, `/repo/sub/.claude/skills/`).
///
/// Two passes are used to handle gitignored provider directories:
///
/// **Pass 1 — loaded skill dirs:** Standard tree traversal collecting directories that
/// end with a known provider skills path (e.g. `.agents/skills`). Gitignored directories
/// are skipped here because they are lazy-loaded with empty children in the tree.
///
/// **Pass 2 — lazy-loaded directories:** Traversal with `include_ignored: true` to find
/// directories that are lazy-loaded (`loaded: false`). Two sub-cases are handled:
///
/// - **Case (a) — provider root is lazy:** The lazy dir is named like a provider root
///   (`.agents`, `.claude`, …), e.g. `sub-project/.agents/` is gitignored. A single
///   `is_dir()` check is performed for `{provider_dir}/skills`.
///
/// - **Case (b) — parent of provider root is lazy:** The lazy dir is not a provider
///   root but could be a parent of one, e.g. `sub-project/` is gitignored so `.agents/`
///   is never in the tree at all. `{dir}/{provider_path}` is checked with `is_dir()`
///   for every known provider.
///
/// The scope of Pass 2 is bounded by the tree itself: a lazy dir only appears in the
/// tree if its parent was indexed, so only dirs that the indexer actually encountered
/// are candidates. Deeper subtrees below a lazy node (e.g. children of `node_modules/`)
/// are never in the tree and therefore never reached.
pub fn find_skill_directories_in_tree(
    repo_path: &Path,
    repo_metadata: &RepoMetadataModel,
    ctx: &AppContext,
) -> Vec<PathBuf> {
    let Some(id) = repo_metadata::RepositoryIdentifier::try_local(repo_path) else {
        return Vec::new();
    };

    // Collect provider skills paths (e.g. ".agents/skills", ".claude/skills") and the
    // corresponding provider root names (e.g. ".agents", ".claude") for the second pass.
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

    // ── Pass 1: find fully-loaded skill directories ───────────────────────────
    //
    // Filter during traversal: only collect directories that end with a skill provider path.
    // The filter rejects files and non-matching directories, avoiding intermediate allocations.
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
        // Only directories should reach this iterator due to the GetContentsArgs::filter.
        // Keep the File arm for exhaustive matching in case RepoContent grows new variants.
        .map(|content| match content {
            RepoContent::Directory(dir) => dir.path.to_local_path_lossy(),
            RepoContent::File(f) => f.path.to_local_path_lossy(),
        })
        .collect();

    // ── Pass 2: check lazy-loaded directories ────────────────────────────────
    //
    // Gitignored directories appear in the tree with `loaded: false` and no
    // children. Two sub-cases are handled with targeted `is_dir()` probes:
    //
    //   Case (a) — provider root is lazy (e.g. `sub-project/.agents/` is
    //   gitignored): probe `{dir}/skills`.
    //
    //   Case (b) — parent of provider root is lazy (e.g. `sub-project/` is
    //   gitignored, so `.agents/` is never in the tree at all): probe
    //   `{dir}/{provider_path}` for every known provider.
    //
    // The scope is bounded by the tree: a lazy dir only appears as a child of
    // an indexed (loaded) parent, so only directories the indexer encountered
    // are candidates. Subtrees below a lazy node are absent from the tree and
    // therefore never reached by this pass.
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
            // Case (a): the lazy dir is itself a provider root (e.g. `.agents/`).
            // Probe one level deeper for the `skills` subdirectory.
            let skills_path = lazy_dir.join("skills");
            if !result_set.contains(&skills_path) && skills_path.is_dir() {
                result_set.insert(skills_path.clone());
                result.push(skills_path);
            }
        } else {
            // Case (b): the lazy dir is a parent of a potential provider root
            // (e.g. `sub-project/` is gitignored, so `.agents/` was never
            // indexed). Probe `{dir}/{provider_path}` for every known provider.
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
    // Collect the skill provider paths from the definitions
    SKILL_PROVIDER_DEFINITIONS
        .iter()
        .map(|p| p.skills_path.to_string_lossy().to_string())
        .collect()
});

// Pattern: {prefix}/{provider_path}/{skill-name}/SKILL.md
// where provider_path is 2 parts (e.g., ".agents/skills") and skill-name is 1 part
#[cfg(not(target_os = "windows"))]
static SKILL_FILE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(.+)/([^/]+/[^/]+)/[^/]+/SKILL\.md$")
        .expect("Failed to compile skill file pattern")
});

// On windows, the path separator is \
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

/// Check if this path is a skill directory under a home directory provider path
/// E.g. ~/.agents/skills/skill-name
pub fn is_home_skill_directory(path: &Path) -> bool {
    let parent_directory = path.parent();
    if let Some(parent_directory) = parent_directory {
        is_home_provider_path(parent_directory)
    } else {
        false
    }
}

/// Check if this path is a home directory provider path
/// E.g. ~/.agents/skills
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
