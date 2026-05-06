//! Helpers for resolving service-account "global" skill specs into repos to
//! ensure are available on disk before agent runs.

use std::{collections::BTreeSet, str::FromStr};

use warp_cli::skill::SkillSpec;

use crate::ai::cloud_environments::GithubRepo;

/// Parse raw skill spec strings into the unique set of GitHub repos they reference.
///
/// Specs that fail to parse are logged and skipped. Specs without an org/repo
/// qualifier are not cloneable, so they are skipped here and left to normal
/// on-disk skill discovery.
pub fn resolve_skill_repos(specs: &[String]) -> Vec<GithubRepo> {
    let mut seen = BTreeSet::new();
    let mut repos = Vec::new();

    for raw in specs {
        let spec = match SkillSpec::from_str(raw) {
            Ok(spec) => spec,
            Err(err) => {
                log::warn!("Failed to parse global skill spec '{raw}': {err}");
                continue;
            }
        };

        let (Some(org), Some(repo)) = (spec.org.as_ref(), spec.repo.as_ref()) else {
            continue;
        };

        if seen.insert((org.clone(), repo.clone())) {
            repos.push(GithubRepo::new(org.clone(), repo.clone()));
        }
    }

    repos
}

#[cfg(test)]
#[path = "global_skills_tests.rs"]
mod tests;
