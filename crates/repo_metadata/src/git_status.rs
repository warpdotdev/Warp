use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitStatus {
    Modified,
    Added,
    Untracked,
    Deleted,
    Conflict,
    Renamed,
    Ignored,
}

#[cfg(not(target_family = "wasm"))]
mod native {
    use std::path::{Path, PathBuf};

    use git2::{Repository, Status, StatusEntry, StatusOptions};
    use thiserror::Error;
    use warp_util::standardized_path::{InvalidPathError, StandardizedPath};

    use super::{GitStatus, HashMap};

    #[derive(Debug, Error)]
    pub enum GitStatusError {
        #[error("failed to read git status: {0}")]
        Git(#[from] git2::Error),
        #[error("failed to standardize status path: {0}")]
        InvalidPath(#[from] InvalidPathError),
        #[error("git status entry had no path")]
        MissingPath,
    }

    pub fn statuses_for_repo(
        repo_root: &Path,
        pathspec: Option<&[&Path]>,
    ) -> Result<HashMap<StandardizedPath, GitStatus>, GitStatusError> {
        let repository = Repository::open(repo_root)?;
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .include_ignored(true)
            .recurse_untracked_dirs(true)
            .recurse_ignored_dirs(true)
            .disable_pathspec_match(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);

        let pathspecs = pathspec
            .unwrap_or_default()
            .iter()
            .map(|path| repo_relative_path(repo_root, path))
            .collect::<Vec<_>>();

        for path in &pathspecs {
            options.pathspec(path.as_path());
        }

        let statuses = repository.statuses(Some(&mut options))?;
        let mut result = HashMap::new();

        for entry in statuses.iter() {
            let status = entry.status();
            let Some(git_status) = map_status(status) else {
                continue;
            };
            let path = status_path(&entry, status).ok_or(GitStatusError::MissingPath)?;
            let standardized = StandardizedPath::try_from_local(&repo_root.join(path))?;
            result.insert(standardized, git_status);
        }

        Ok(result)
    }

    fn repo_relative_path(repo_root: &Path, path: &Path) -> PathBuf {
        path.strip_prefix(repo_root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.to_path_buf())
    }

    fn status_path<'entry>(
        entry: &'entry StatusEntry<'entry>,
        status: Status,
    ) -> Option<&'entry Path> {
        if status.is_index_renamed() {
            entry
                .head_to_index()
                .and_then(|delta| delta.new_file().path())
        } else if status.is_wt_renamed() {
            entry
                .index_to_workdir()
                .and_then(|delta| delta.new_file().path())
        } else {
            entry.path().map(Path::new)
        }
    }

    fn map_status(status: Status) -> Option<GitStatus> {
        if status.is_conflicted() {
            Some(GitStatus::Conflict)
        } else if status.is_ignored() {
            Some(GitStatus::Ignored)
        } else if status.is_index_renamed() || status.is_wt_renamed() {
            Some(GitStatus::Renamed)
        } else if status.is_index_new() {
            Some(GitStatus::Added)
        } else if status.is_wt_new() {
            Some(GitStatus::Untracked)
        } else if status.is_index_deleted() || status.is_wt_deleted() {
            Some(GitStatus::Deleted)
        } else if status.is_index_modified()
            || status.is_wt_modified()
            || status.is_index_typechange()
            || status.is_wt_typechange()
        {
            Some(GitStatus::Modified)
        } else {
            None
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{fs, path::Path};

        use git2::Signature;
        use tempfile::TempDir;

        use super::*;

        #[test]
        fn statuses_for_repo_maps_modified_untracked_added_and_ignored() -> anyhow::Result<()> {
            let temp_dir = TempDir::new()?;
            let repo = Repository::init(temp_dir.path())?;

            fs::write(temp_dir.path().join(".gitignore"), "*.log\n")?;
            fs::write(temp_dir.path().join("tracked.txt"), "baseline\n")?;
            commit_all(&repo)?;

            fs::write(temp_dir.path().join("tracked.txt"), "modified\n")?;
            fs::write(temp_dir.path().join("untracked.txt"), "new\n")?;
            fs::write(temp_dir.path().join("ignored.log"), "ignored\n")?;
            fs::write(temp_dir.path().join("staged.txt"), "staged\n")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("staged.txt"))?;
            index.write()?;

            let statuses = statuses_for_repo(temp_dir.path(), None)?;

            assert_eq!(
                statuses.get(&StandardizedPath::try_from_local(
                    &temp_dir.path().join("tracked.txt")
                )?),
                Some(&GitStatus::Modified)
            );
            assert_eq!(
                statuses.get(&StandardizedPath::try_from_local(
                    &temp_dir.path().join("untracked.txt")
                )?),
                Some(&GitStatus::Untracked)
            );
            assert_eq!(
                statuses.get(&StandardizedPath::try_from_local(
                    &temp_dir.path().join("ignored.log")
                )?),
                Some(&GitStatus::Ignored)
            );
            assert_eq!(
                statuses.get(&StandardizedPath::try_from_local(
                    &temp_dir.path().join("staged.txt")
                )?),
                Some(&GitStatus::Added)
            );

            let tracked_path = temp_dir.path().join("tracked.txt");
            let pathspecs = [tracked_path.as_path()];
            let scoped_statuses = statuses_for_repo(temp_dir.path(), Some(&pathspecs))?;
            assert_eq!(scoped_statuses.len(), 1);
            assert_eq!(
                scoped_statuses.get(&StandardizedPath::try_from_local(&tracked_path)?),
                Some(&GitStatus::Modified)
            );

            Ok(())
        }

        fn commit_all(repo: &Repository) -> anyhow::Result<()> {
            let mut index = repo.index()?;
            index.add_path(Path::new(".gitignore"))?;
            index.add_path(Path::new("tracked.txt"))?;
            index.write()?;

            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let signature = Signature::now("Warp Test", "test@example.com")?;
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                "initial commit",
                &tree,
                &[],
            )?;

            Ok(())
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub use native::{statuses_for_repo, GitStatusError};
