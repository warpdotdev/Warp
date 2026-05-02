use anyhow::Result;
#[cfg(feature = "local_fs")]
use repo_metadata::repositories::RepoDetectionSource;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use warpui::{Entity, ModelContext, SingletonEntity};

cfg_if::cfg_if! {
    if #[cfg(feature = "local_fs")] {
        use repo_metadata::entry::{Entry, FileMetadata};
        use repo_metadata::repository::RepositorySubscriber;
        use repo_metadata::{Repository, DirectoryWatcher, RepositoryUpdate};
        use ignore::gitignore::Gitignore;
        use async_channel::Sender;

        const RULES_FILE_PATTERN: [&str; 2] = ["WARP.md", "AGENTS.md"];
        const MAX_SCAN_DEPTH: usize = 3;
        const MAX_FILES_TO_SCAN: usize = 5000;
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProjectRule {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Default)]
struct RuleAtPath {
    parent_path: PathBuf,
    warp_md: Option<ProjectRule>,
    agents_md: Option<ProjectRule>,
}

impl RuleAtPath {
    fn respected_rule(&self) -> Option<&ProjectRule> {
        self.warp_md.as_ref().or(self.agents_md.as_ref())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProjectRulesResult {
    pub root_path: PathBuf,
    pub active_rules: Vec<ProjectRule>,
    pub additional_rule_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRulePath {
    pub path: PathBuf,
    pub project_root: PathBuf,
}

struct FindRulesResult {
    /// Rules that are active and should be eagerly applied.
    active_rules: Vec<ProjectRule>,
    /// Rule paths that are currently not active but available to be applied if
    /// a file under its directory is edited.
    available_rule_paths: Vec<String>,
}

#[cfg(feature = "local_fs")]
fn matches_rules_pattern(file_name_str: &str) -> bool {
    for pattern in RULES_FILE_PATTERN {
        if file_name_str.to_lowercase() == pattern.to_lowercase() {
            return true;
        }
    }
    false
}

#[derive(Debug, Default)]
struct ProjectRules {
    rules: Vec<RuleAtPath>,
}

impl ProjectRules {
    /// Finds the set of rules that are active in the given path and the set that are available to be applied.
    fn find_active_or_applicable_rules(&self, path: &Path) -> FindRulesResult {
        let mut active_rules = Vec::new();
        let mut available_rule_paths = Vec::new();

        // Collect all applicable rules (rules in directories that are ancestors of the target path)
        for rule in &self.rules {
            if let Some(respected_rule) = rule.respected_rule() {
                // Check if the rule's directory is an ancestor of or equal to the target path
                if path.starts_with(&rule.parent_path) {
                    active_rules.push(respected_rule.clone());
                } else {
                    available_rule_paths.push(respected_rule.path.to_string_lossy().to_string());
                }
            }
        }

        FindRulesResult {
            active_rules,
            available_rule_paths,
        }
    }

    /// Remove a rule from the set of project rules. This returns the removed rule.
    #[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
    fn remove_rule(&mut self, path: &Path) -> Option<ProjectRule> {
        let parent = path.parent()?;
        let file_name = path.file_name().and_then(|name| name.to_str())?;

        let rule = self
            .rules
            .iter_mut()
            .find(|rule| rule.parent_path == parent)?;

        if file_name.to_lowercase() == "warp.md" {
            rule.warp_md.take()
        } else if file_name.to_lowercase() == "agents.md" {
            rule.agents_md.take()
        } else {
            None
        }
    }

    /// Upsert a rule to the set of project rules. This will create a new RuleAtPath entry if none exists and update the existing one
    /// otherwise.
    #[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
    fn upsert_rule(&mut self, path: &Path, content: String) {
        let Some(parent) = path.parent() else {
            return;
        };
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            return;
        };

        let existing_rule = self
            .rules
            .iter_mut()
            .find(|rule| rule.parent_path == parent);

        let rule_file = Some(ProjectRule {
            path: path.to_path_buf(),
            content,
        });

        match existing_rule {
            Some(rule) => {
                if file_name.to_lowercase() == "warp.md" {
                    rule.warp_md = rule_file;
                } else if file_name.to_lowercase() == "agents.md" {
                    rule.agents_md = rule_file;
                }
            }
            None => {
                let mut rule = RuleAtPath {
                    parent_path: parent.to_path_buf(),
                    ..Default::default()
                };
                if file_name.to_lowercase() == "warp.md" {
                    rule.warp_md = rule_file;
                } else if file_name.to_lowercase() == "agents.md" {
                    rule.agents_md = rule_file;
                }
                self.rules.push(rule);
            }
        };
    }
}

/// Singleton model that keeps track of mapping between paths and rule files
/// Currently supports WARP.md files, but designed to be extensible
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
#[derive(Debug, Default)]
pub struct ProjectContextModel {
    /// Mapping from directory path to list of rule files found in that directory
    path_to_rules: HashMap<PathBuf, ProjectRules>,
}

#[derive(Default, Debug)]
pub struct RulesDelta {
    pub discovered_rules: Vec<ProjectRulePath>,
    pub deleted_rules: Vec<PathBuf>,
}

/// Events emitted by the ProjectContextModel
pub enum ProjectContextModelEvent {
    /// Emitted when a path has been indexed
    PathIndexed,
    /// Emitted when the known set of rule files changed
    KnownRulesChanged(RulesDelta),
}

impl ProjectContextModel {
    #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
    pub fn new_from_persisted(
        persisted_rules: Vec<ProjectRulePath>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        #[cfg(feature = "local_fs")]
        ctx.spawn(
            async move { Self::read_persisted_rules(persisted_rules).await },
            |me, mut res, ctx| {
                for root in res.keys() {
                    me.try_initialize_and_register_watcher(root, ctx);
                }

                // If we have any rules detected before fully loading the persisted rules, we want to
                // keep the detected rules since it's more up to date.
                res.extend(me.path_to_rules.drain());
                me.path_to_rules = res;
                ctx.emit(ProjectContextModelEvent::PathIndexed);
            },
        );

        Self::default()
    }

    /// Index a path and find all rule files from that path up to the root directory
    /// Returns a list of all rule files found
    #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
    pub fn index_and_store_rules(
        &mut self,
        root_path: PathBuf,
        ctx: &mut ModelContext<Self>,
    ) -> Result<()> {
        if self.path_to_rules.contains_key(&root_path) {
            return Ok(());
        }
        #[cfg(feature = "local_fs")]
        {
            let root_clone = root_path.clone();

            ctx.spawn(
                async move { Self::scan_directory_for_rules(&root_path).await },
                move |me, res: Result<ProjectRules>, ctx| match res {
                    Ok(rule_files) => {
                        me.register_watcher_for_path(&root_clone, ctx);

                        // Persist the discovered rules.
                        let delta = RulesDelta {
                            discovered_rules: rule_files
                                .rules
                                .iter()
                                .filter_map(|rule| {
                                    rule.warp_md.as_ref().map(|rule| ProjectRulePath {
                                        project_root: root_clone.clone(),
                                        path: rule.path.clone(),
                                    })
                                })
                                .chain(rule_files.rules.iter().filter_map(|rule| {
                                    rule.agents_md.as_ref().map(|rule| ProjectRulePath {
                                        project_root: root_clone.clone(),
                                        path: rule.path.clone(),
                                    })
                                }))
                                .collect(),
                            deleted_rules: Default::default(),
                        };
                        ctx.emit(ProjectContextModelEvent::KnownRulesChanged(delta));

                        me.path_to_rules.insert(root_clone, rule_files);
                        ctx.emit(ProjectContextModelEvent::PathIndexed);
                    }
                    Err(e) => log::warn!(
                        "Couldn't index rules for path {}: {}",
                        root_clone.display(),
                        e
                    ),
                },
            );
        }

        Ok(())
    }

    /// This should be used when we are bootstrapping project rules from persisted rule paths. In this case,
    /// the actual repo watcher might not have been registered yet. We will attempt to register that repo watcher
    /// if it doesn't yet exists.
    #[cfg(feature = "local_fs")]
    fn try_initialize_and_register_watcher(&self, path: &Path, ctx: &mut ModelContext<Self>) {
        use repo_metadata::repositories::DetectedRepositories;

        let directory_watcher = DirectoryWatcher::handle(ctx);
        if directory_watcher
            .as_ref(ctx)
            .get_watched_directory_for_path(path)
            .is_some()
        {
            self.register_watcher_for_path(path, ctx);
            return;
        }

        let fut = DetectedRepositories::handle(ctx).update(ctx, |model, ctx| {
            model.detect_possible_git_repo(
                &path.to_string_lossy(),
                RepoDetectionSource::ProjectRulesIndexing,
                ctx,
            )
        });

        ctx.spawn(fut, move |me, repo_path_opt, ctx| {
            if let Some(path) = repo_path_opt {
                me.register_watcher_for_path(&path, ctx);
            }
        });
    }

    #[cfg(feature = "local_fs")]
    fn register_watcher_for_path(&self, path: &Path, ctx: &mut ModelContext<Self>) {
        let Some(repository_model) =
            DirectoryWatcher::as_ref(ctx).get_watched_directory_for_path(path)
        else {
            return;
        };

        let (repository_update_tx, repository_update_rx) = async_channel::unbounded();
        let start = repository_model.update(ctx, |repo, ctx| {
            repo.start_watching(
                Box::new(ProjectContextRepositorySubscriber {
                    repository_update_tx,
                }),
                ctx,
            )
        });

        let subscriber_id = start.subscriber_id;
        let repository_model_for_cleanup = repository_model.downgrade();
        let path_clone = path.to_path_buf();
        let path_for_log = path_clone.clone();
        ctx.spawn(start.registration_future, move |_, res, ctx| {
            if let Err(err) = res {
                log::warn!(
                    "Failed to start watching repository for rule updates at {}: {err}",
                    path_for_log.display()
                );

                if let Some(repository_model) = repository_model_for_cleanup.upgrade(ctx) {
                    repository_model.update(ctx, |repo, ctx| {
                        repo.stop_watching(subscriber_id, ctx);
                    });
                }
            }
        });

        ctx.spawn_stream_local(
            repository_update_rx.clone(),
            move |me, update, ctx| {
                if update.is_empty() {
                    return;
                }

                let existing_rules = me.path_to_rules.remove(&path_clone);
                let repo_path = path_clone.clone();
                if let Some(rules) = existing_rules {
                    let repo_path_for_closure = repo_path.clone();
                    ctx.spawn(
                        async move {
                            Self::process_repository_updates(update, rules, repo_path).await
                        },
                        move |me, (rules, rule_delta), ctx| {
                            ctx.emit(ProjectContextModelEvent::KnownRulesChanged(rule_delta));

                            me.path_to_rules.insert(repo_path_for_closure, rules);
                            ctx.emit(ProjectContextModelEvent::PathIndexed);
                        },
                    );
                }
            },
            |_, _| {},
        );
    }

    pub fn find_applicable_rules(&self, path: &Path) -> Option<ProjectRulesResult> {
        let mut current_path = path.to_owned();
        let mut active_rules = Vec::new();
        let mut available_rule_paths = Vec::new();

        // Find the root path with indexed rules and collect active rules
        let mut found_rules = false;
        loop {
            if let Some(rules) = self.path_to_rules.get(&current_path) {
                let result = rules.find_active_or_applicable_rules(path);

                active_rules = result.active_rules;
                available_rule_paths = result.available_rule_paths;

                found_rules = true;
                break;
            }

            if !current_path.pop() {
                break;
            }
        }

        if !found_rules {
            return None;
        }

        if active_rules.is_empty() && available_rule_paths.is_empty() {
            return None;
        }

        Some(ProjectRulesResult {
            root_path: current_path,
            active_rules,
            additional_rule_paths: available_rule_paths,
        })
    }

    #[cfg(feature = "local_fs")]
    async fn process_repository_updates(
        repository_update: RepositoryUpdate,
        mut existing_rules: ProjectRules,
        project_root: PathBuf,
    ) -> (ProjectRules, RulesDelta) {
        let mut rules_delta = RulesDelta::default();
        // Handle deleted files - remove rules for deleted rule files
        for target_file in &repository_update.deleted {
            // Skip gitignored files
            if target_file.is_ignored {
                continue;
            }
            if let Some(file_name_str) = target_file.path.file_name().and_then(|name| name.to_str())
            {
                if matches_rules_pattern(file_name_str) {
                    // Remove the rule from existing rules
                    existing_rules.remove_rule(&target_file.path);
                    rules_delta.deleted_rules.push(target_file.path.clone());

                    log::debug!("Removed rule file: {}", target_file.path.display());
                }
            }
        }

        // Handle moved files - update paths for moved rule files
        for (to_target, from_target) in &repository_update.moved {
            // Skip gitignored files
            if to_target.is_ignored || from_target.is_ignored {
                continue;
            }
            if let Some(file_name_str) = to_target.path.file_name().and_then(|name| name.to_str()) {
                if matches_rules_pattern(file_name_str) {
                    // Find and update the rule with the old path
                    if let Some(rule) = existing_rules.remove_rule(&from_target.path) {
                        // Emit deletion event for old path
                        rules_delta.deleted_rules.push(from_target.path.clone());

                        existing_rules.upsert_rule(&to_target.path, rule.content);

                        // Emit upsert event for new path
                        rules_delta.discovered_rules.push(ProjectRulePath {
                            path: to_target.path.clone(),
                            project_root: project_root.clone(),
                        });

                        log::debug!(
                            "Updated rule file path: {} -> {}",
                            from_target.path.display(),
                            to_target.path.display()
                        );
                    }
                }
            }
        }

        // Handle added/updated files - upsert rules for rule files
        for target_file in repository_update.added_or_modified() {
            // Skip gitignored files
            if target_file.is_ignored {
                continue;
            }
            if let Some(file_name_str) = target_file.path.file_name().and_then(|name| name.to_str())
            {
                if matches_rules_pattern(file_name_str) {
                    // Read the content of the rule file
                    match async_fs::read_to_string(&target_file.path).await {
                        Ok(content) => {
                            existing_rules.upsert_rule(&target_file.path, content);
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to read updated rule file {}: {}",
                                target_file.path.display(),
                                e
                            );
                        }
                    }
                }
            }
        }

        (existing_rules, rules_delta)
    }

    /// Scan a directory for rule files (currently WARP.md, extensible for future file types)
    /// Uses repo_metadata::entry::build_tree for efficient directory traversal
    #[cfg(feature = "local_fs")]
    async fn scan_directory_for_rules(dir_path: &Path) -> Result<ProjectRules> {
        use repo_metadata::entry::IgnoredPathStrategy;

        let mut rule_files = ProjectRules::default();

        if !async_fs::metadata(dir_path).await?.is_dir() {
            return Ok(rule_files);
        }

        // Use build_tree to collect all files, then filter for rule files
        let mut files = Vec::<FileMetadata>::new();
        let mut gitignores = Vec::<Gitignore>::new();

        // Collect patterns that should not be ignored
        let override_ignore_patterns: Vec<String> =
            RULES_FILE_PATTERN.iter().map(|s| s.to_string()).collect();
        let mut file_limit = MAX_FILES_TO_SCAN;

        // Build the file tree using repo_metadata's build_tree function
        let ignore_behavior = IgnoredPathStrategy::IncludeOnly(override_ignore_patterns.clone());

        let _ = Entry::build_tree(
            dir_path,
            &mut files,
            &mut gitignores,
            Some(&mut file_limit),
            MAX_SCAN_DEPTH,
            0,
            &ignore_behavior,
        )?;

        // Filter files to only include those matching RULES_FILE_PATTERN
        for file_metadata in files {
            let path = &file_metadata.path;
            let file_name = path.file_name();

            if let Some(file_name_str) = file_name {
                if matches_rules_pattern(file_name_str) {
                    // Read the content of the rule file
                    let local_path = file_metadata.path.to_local_path_lossy();
                    let content = match async_fs::read_to_string(&local_path).await {
                        Ok(content) => content,
                        Err(e) => {
                            log::warn!("Failed to read rule file {}: {e}", file_metadata.path,);
                            break;
                        }
                    };

                    rule_files.upsert_rule(&local_path, content);
                }
            }
        }

        Ok(rule_files)
    }

    #[cfg(feature = "local_fs")]
    async fn read_persisted_rules(
        rule_paths: Vec<ProjectRulePath>,
    ) -> HashMap<PathBuf, ProjectRules> {
        let mut rules: HashMap<PathBuf, ProjectRules> = HashMap::new();

        for rule in rule_paths {
            match async_fs::read_to_string(&rule.path).await {
                Ok(content) => {
                    let existing_rules = rules.entry(rule.project_root).or_default();
                    existing_rules.upsert_rule(&rule.path, content);
                }
                Err(e) => {
                    log::debug!(
                        "Failed to read rule file from persistence {}: {}",
                        rule.path.display(),
                        e
                    );
                    // Continue processing other files even if one fails
                }
            }
        }

        rules
    }

    pub fn indexed_rules(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.path_to_rules.values().flat_map(|rules| {
            rules.rules.iter().filter_map(|rules| {
                rules
                    .respected_rule()
                    .map(|project_rule| project_rule.path.clone())
            })
        })
    }

    /// Returns the rule file paths associated with a specific workspace root path.
    pub fn rules_for_workspace(&self, workspace_path: &Path) -> Vec<PathBuf> {
        self.path_to_rules
            .get(workspace_path)
            .into_iter()
            .flat_map(|rules| {
                rules.rules.iter().filter_map(|rule| {
                    rule.respected_rule()
                        .map(|project_rule| project_rule.path.clone())
                })
            })
            .collect()
    }
}

impl Entity for ProjectContextModel {
    type Event = ProjectContextModelEvent;
}

impl SingletonEntity for ProjectContextModel {}

#[cfg(feature = "local_fs")]
struct ProjectContextRepositorySubscriber {
    repository_update_tx: Sender<RepositoryUpdate>,
}

#[cfg(feature = "local_fs")]
impl RepositorySubscriber for ProjectContextRepositorySubscriber {
    fn on_scan(
        &mut self,
        _repository: &Repository,
        _ctx: &mut ModelContext<Repository>,
    ) -> std::pin::Pin<Box<dyn std::prelude::rust_2024::Future<Output = ()> + Send + 'static>> {
        // The model can safely ignore the initial scan because the model only subscribes
        // after the repository is already scanned.
        Box::pin(async {})
    }

    fn on_files_updated(
        &mut self,
        _repository: &Repository,
        update: &repo_metadata::RepositoryUpdate,
        _ctx: &mut ModelContext<Repository>,
    ) -> std::pin::Pin<Box<dyn std::prelude::rust_2024::Future<Output = ()> + Send + 'static>> {
        let tx = self.repository_update_tx.clone();
        let update = update.clone();
        Box::pin(async move {
            let _ = tx.send(update).await;
        })
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
