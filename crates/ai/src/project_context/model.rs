use anyhow::Result;
#[cfg(feature = "local_fs")]
use repo_metadata::repositories::RepoDetectionSource;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use warpui::{Entity, ModelContext, SingletonEntity};

cfg_if::cfg_if! {
    if #[cfg(feature = "local_fs")] {
        use repo_metadata::entry::{Entry, FileMetadata};
        use repo_metadata::repository::RepositorySubscriber;
        use repo_metadata::{Repository, DirectoryWatcher, RepositoryUpdate};
        use repo_metadata::repository::SubscriberId;
        use ignore::gitignore::Gitignore;
        use async_channel::Sender;
        use warp_core::safe_warn;
        use warp_util::standardized_path::StandardizedPath;
        use warpui::ModelHandle;
        use watcher::{HomeDirectoryWatcher, HomeDirectoryWatcherEvent};

        const RULES_FILE_PATTERN: [&str; 2] = ["WARP.md", "AGENTS.md"];
        const MAX_SCAN_DEPTH: usize = 3;
        const MAX_FILES_TO_SCAN: usize = 5000;
    }
}

/// A well-known location under `$HOME` that may contain a global rule file.
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlobalRuleSource {
    /// `~/.agents/AGENTS.md`.
    Agents,
}

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
impl GlobalRuleSource {
    /// Iterates every known global rule source.
    fn iter() -> impl Iterator<Item = Self> {
        [Self::Agents].into_iter()
    }

    /// Display name (used in safe logs that don't expose user paths).
    fn name(self) -> &'static str {
        match self {
            Self::Agents => "agents",
        }
    }

    /// Subdirectory under `$HOME`, e.g. `".agents"`.
    fn home_subdir(self) -> &'static str {
        match self {
            Self::Agents => ".agents",
        }
    }

    /// File name within the subdir, e.g. `"AGENTS.md"`.
    fn file_pattern(self) -> &'static str {
        match self {
            Self::Agents => "AGENTS.md",
        }
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

    /// Upsert a rule to the set of project rules. This will create a new RuleAtPath entry if none exists and update the existin one
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
    /// Global rule files keyed by absolute file path. Populated by
    /// [`Self::index_global_rules`] from [`GlobalRuleSource`]. Independent of
    /// `path_to_rules`: project-level `AGENTS.md` files never write here.
    /// Stored in a `BTreeMap` so iteration order is deterministic (sorted by
    /// path), which keeps `find_applicable_rules` output stable.
    global_rules: BTreeMap<PathBuf, ProjectRule>,
    /// Active home-subdir directory watchers, keyed by the absolute subdir
    /// path (e.g. `~/.agents`). Keying by path naturally deduplicates if two
    /// [`GlobalRuleSource`] variants ever share the same `home_subdir`.
    #[cfg(feature = "local_fs")]
    global_source_watchers: HashMap<PathBuf, GlobalSourceWatcherState>,
    /// Sender used by global-rule directory subscribers to push updates back
    /// into the model's main-thread stream handler. Initialized once on the
    /// first call to [`Self::index_global_rules`].
    #[cfg(feature = "local_fs")]
    global_updates_tx: Option<Sender<GlobalRulesUpdate>>,
}

#[cfg(feature = "local_fs")]
#[derive(Debug)]
struct GlobalSourceWatcherState {
    repository: ModelHandle<Repository>,
    subscriber_id: SubscriberId,
}

#[cfg(feature = "local_fs")]
#[derive(Debug)]
struct GlobalRulesUpdate {
    /// The [`GlobalRuleSource`] variant that produced this update. The
    /// receiver uses it to look up the matching `home_subdir`/`file_pattern`
    /// without needing a per-source channel.
    source: GlobalRuleSource,
    update: RepositoryUpdate,
}

#[derive(Default, Debug)]
pub struct RulesDelta {
    pub discovered_rules: Vec<ProjectRulePath>,
    pub deleted_rules: Vec<PathBuf>,
}

#[derive(Default, Debug)]
pub struct GlobalRulesDelta {
    pub discovered_rules: Vec<PathBuf>,
    pub deleted_rules: Vec<PathBuf>,
}

/// Events emitted by the ProjectContextModel
pub enum ProjectContextModelEvent {
    /// Emitted when a path has been indexed
    PathIndexed,
    /// Emitted when the known set of rule files changed
    KnownRulesChanged(RulesDelta),
    /// Emitted when the set of indexed global rule files changed
    GlobalRulesChanged(GlobalRulesDelta),
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

    /// Index all configured global rule sources (see [`GlobalRuleSource`]).
    ///
    /// All disk I/O is dispatched through `ctx.spawn` so this method does not
    /// block startup. Subscribes to [`HomeDirectoryWatcher`] to react to
    /// creation/deletion of the home subdirs at runtime, and registers a
    /// [`DirectoryWatcher`] per existing subdir for incremental updates.
    ///
    /// Idempotent: subsequent calls are a no-op once the channel is initialized.
    #[cfg(feature = "local_fs")]
    pub fn index_global_rules(&mut self, ctx: &mut ModelContext<Self>) {
        if self.global_updates_tx.is_some() {
            return;
        }

        let Some(home_dir) = dirs::home_dir() else {
            log::debug!("Home directory not found; skipping global rules indexing");
            return;
        };

        // Set up the channel that all per-source subscribers push into.
        let (tx, rx) = async_channel::unbounded::<GlobalRulesUpdate>();
        self.global_updates_tx = Some(tx);

        ctx.spawn_stream_local(
            rx,
            |me, update, ctx| {
                me.handle_global_rules_update(update.source, update.update, ctx);
            },
            |_, _| {},
        );

        // React to creation/deletion of home subdirs at runtime.
        ctx.subscribe_to_model(&HomeDirectoryWatcher::handle(ctx), |me, event, ctx| {
            me.handle_home_dir_event_for_global_rules(event, ctx);
        });

        for source in GlobalRuleSource::iter() {
            let subdir_path = home_dir.join(source.home_subdir());
            let target_file = subdir_path.join(source.file_pattern());

            // Initial async read; if the file doesn't exist yet, the watcher
            // will pick it up on creation.
            Self::spawn_global_rule_read(target_file, ctx);

            if subdir_path.exists() {
                self.register_global_source_watcher(source, &subdir_path, ctx);
            }
        }
    }

    /// Async read of a single global rule file. The async block runs on a
    /// background executor; the main-thread callback updates model state once
    /// the read completes.
    #[cfg(feature = "local_fs")]
    fn spawn_global_rule_read(file_path: PathBuf, ctx: &mut ModelContext<Self>) {
        ctx.spawn(
            async move {
                // `read_to_string` returning `Err` (e.g. NotFound, permission
                // denied, file replaced with a non-regular file) is converted
                // to `None`; the callback below decides whether that means
                // "insert/refresh" or "drop a previously-known entry."
                let content = async_fs::read_to_string(&file_path).await.ok();
                (file_path, content)
            },
            move |me, (file_path, content_opt), ctx| match content_opt {
                Some(content) => {
                    // Read succeeded: insert (or replace) the rule and notify
                    // subscribers.
                    me.global_rules.insert(
                        file_path.clone(),
                        ProjectRule {
                            path: file_path.clone(),
                            content,
                        },
                    );
                    ctx.emit(ProjectContextModelEvent::GlobalRulesChanged(
                        GlobalRulesDelta {
                            discovered_rules: vec![file_path],
                            deleted_rules: vec![],
                        },
                    ));
                }
                None => {
                    // Read failed. If we previously had content cached for
                    // this path we MUST drop it — silently keeping stale
                    // rule text active after the file becomes unreadable
                    // (deleted between the FS event and the read, perms
                    // revoked, replaced with a directory, …) would leave
                    // the user's prompts decorated with instructions they
                    // thought were gone. If we had nothing to begin with,
                    // this is the steady "file never existed" state and we
                    // do nothing.
                    if me.global_rules.remove(&file_path).is_some() {
                        ctx.emit(ProjectContextModelEvent::GlobalRulesChanged(
                            GlobalRulesDelta {
                                discovered_rules: vec![],
                                deleted_rules: vec![file_path],
                            },
                        ));
                    }
                }
            },
        );
    }

    /// Register a `DirectoryWatcher` on the given home subdir for incremental
    /// updates. Idempotent: subsequent calls for an already-watched subdir are
    /// a no-op (the `subdir_path` key dedups by directory rather than by
    /// source, so multiple sources sharing a `home_subdir` would only register
    /// the watcher once — a future change can fan out to multiple file
    /// patterns by extending the value).
    ///
    /// The subdir must exist on disk before this is called: `StandardizedPath::
    /// from_local_canonicalized` resolves symlinks and validates existence,
    /// and `DirectoryWatcher::add_directory` rejects non-existent paths.
    /// Creation at runtime is handled by `handle_home_dir_event_for_global_rules`,
    /// which calls back here once the subdir appears.
    #[cfg(feature = "local_fs")]
    fn register_global_source_watcher(
        &mut self,
        source: GlobalRuleSource,
        subdir_path: &Path,
        ctx: &mut ModelContext<Self>,
    ) {
        // If the subdir is already being watched, return early.
        if self.global_source_watchers.contains_key(subdir_path) {
            return;
        }

        let Some(update_tx) = self.global_updates_tx.clone() else {
            return;
        };

        // We use `StandardizedPath::from_local_canonicalized` (rather than
        // `CanonicalizedPath::try_from`) because the underlying `DirectoryWatcher::
        // add_directory` API takes `StandardizedPath` directly. The two are
        // equivalent in terms of I/O; this just avoids an extra type conversion.
        let Ok(std_path) = StandardizedPath::from_local_canonicalized(subdir_path) else {
            return;
        };

        let repo_handle = match DirectoryWatcher::handle(ctx)
            .update(ctx, |watcher, ctx| watcher.add_directory(std_path, ctx))
        {
            Ok(handle) => handle,
            Err(err) => {
                // `safe_warn!` because the path contains the user's home dir,
                // which is PII; we only want the full path on dogfood builds.
                // The error itself can also embed the canonicalized path
                // (e.g. `RepoMetadataError::RepoNotFound(...)`), so we keep
                // it out of the safe branch as well — only the source name
                // is safe to send to Sentry.
                safe_warn!(
                    safe: (
                        "Failed to register {} for global rules watching",
                        source.name()
                    ),
                    full: (
                        "Failed to register {} for global rules watching: {err}",
                        subdir_path.display()
                    )
                );
                return;
            }
        };

        let subscriber = Box::new(GlobalRulesRepositorySubscriber { source, update_tx });

        let start = repo_handle.update(ctx, |repo, ctx| repo.start_watching(subscriber, ctx));
        let subscriber_id = start.subscriber_id;
        let subdir_path_owned = subdir_path.to_path_buf();

        self.global_source_watchers.insert(
            subdir_path_owned.clone(),
            GlobalSourceWatcherState {
                repository: repo_handle.clone(),
                subscriber_id,
            },
        );

        let cleanup_key = subdir_path_owned.clone();
        let subdir_for_log = subdir_path_owned;
        ctx.spawn(start.registration_future, move |me, res, ctx| {
            if let Err(err) = res {
                // Same PII shape as the registration error above: the path
                // and the error can both contain the user's home dir, so
                // both stay in the `full` branch only.
                safe_warn!(
                    safe: (
                        "Failed to start watching {} for global rules",
                        source.name()
                    ),
                    full: (
                        "Failed to start watching {} for global rules: {err}",
                        subdir_for_log.display()
                    )
                );
                if let Some(state) = me.global_source_watchers.remove(&cleanup_key) {
                    state.repository.update(ctx, |repo, ctx| {
                        repo.stop_watching(state.subscriber_id, ctx);
                    });
                }
            }
        });
    }

    /// Handle an incremental update for the given global source.
    #[cfg(feature = "local_fs")]
    fn handle_global_rules_update(
        &mut self,
        source: GlobalRuleSource,
        update: RepositoryUpdate,
        ctx: &mut ModelContext<Self>,
    ) {
        if update.is_empty() {
            return;
        }
        let Some(home_dir) = dirs::home_dir() else {
            return;
        };
        let target_file = home_dir
            .join(source.home_subdir())
            .join(source.file_pattern());

        let was_deleted = update.deleted.iter().any(|f| f.path == target_file)
            || update.moved.values().any(|f| f.path == target_file);
        let was_added_or_modified = update.added_or_modified().any(|f| f.path == target_file)
            || update.moved.keys().any(|f| f.path == target_file);

        if was_deleted && self.global_rules.remove(&target_file).is_some() {
            ctx.emit(ProjectContextModelEvent::GlobalRulesChanged(
                GlobalRulesDelta {
                    discovered_rules: vec![],
                    deleted_rules: vec![target_file.clone()],
                },
            ));
        }

        if was_added_or_modified {
            Self::spawn_global_rule_read(target_file, ctx);
        }
    }

    /// React to creation/deletion of the registered home subdirs at runtime.
    #[cfg(feature = "local_fs")]
    fn handle_home_dir_event_for_global_rules(
        &mut self,
        event: &HomeDirectoryWatcherEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let HomeDirectoryWatcherEvent::HomeFilesChanged(fs_event) = event;
        let Some(home_dir) = dirs::home_dir() else {
            return;
        };

        for source in GlobalRuleSource::iter() {
            let subdir_path = home_dir.join(source.home_subdir());

            let subdir_added =
                fs_event.added.contains(&subdir_path) || fs_event.moved.contains_key(&subdir_path);
            if subdir_added {
                self.register_global_source_watcher(source, &subdir_path, ctx);
                let target_file = subdir_path.join(source.file_pattern());
                Self::spawn_global_rule_read(target_file, ctx);
            }

            let subdir_deleted = fs_event.deleted.contains(&subdir_path)
                || fs_event.moved.values().any(|v| v == &subdir_path);
            if subdir_deleted {
                if let Some(state) = self.global_source_watchers.remove(&subdir_path) {
                    state.repository.update(ctx, |repo, ctx| {
                        repo.stop_watching(state.subscriber_id, ctx);
                    });
                }
                let target_file = subdir_path.join(source.file_pattern());
                if self.global_rules.remove(&target_file).is_some() {
                    ctx.emit(ProjectContextModelEvent::GlobalRulesChanged(
                        GlobalRulesDelta {
                            discovered_rules: vec![],
                            deleted_rules: vec![target_file],
                        },
                    ));
                }
            }
        }
    }

    /// Project-only rule lookup. Returns `Some` only when an indexed project
    /// root above `path` actually contributes a rule — globals are
    /// deliberately ignored.
    ///
    /// Use this for callers that read "do we have rules for this repo?" as a
    /// project-initialization signal (for example the `/init` flow's
    /// `should_have_available_steps` check, or the code-review empty
    /// state's "Repo is initialized with a WARP.md file" hint). Mixing
    /// global fallbacks into that signal would make every repo look
    /// initialized as soon as the user drops a single `~/.agents/AGENTS.md`,
    /// which is the wrong product behavior.
    pub fn find_applicable_project_rules(&self, path: &Path) -> Option<ProjectRulesResult> {
        let mut current_path = path.to_owned();

        // Walk upwards from `path` toward the filesystem root, stopping at the
        // first directory we have indexed project rules for. `path_to_rules`
        // is keyed by indexed project root, so popping the path produces
        // every ancestor directory until we hit a known root or `pop()`
        // returns false (we've reached the top of the path).
        loop {
            if let Some(rules) = self.path_to_rules.get(&current_path) {
                let result = rules.find_active_or_applicable_rules(path);
                if result.active_rules.is_empty() && result.available_rule_paths.is_empty() {
                    return None;
                }
                return Some(ProjectRulesResult {
                    root_path: current_path,
                    active_rules: result.active_rules,
                    additional_rule_paths: result.available_rule_paths,
                });
            }

            if !current_path.pop() {
                return None;
            }
        }
    }

    /// Returns the rules applicable to `path`, layering global rules on top of
    /// any project rules discovered up the directory tree.
    ///
    /// Precedence is `global > project WARP.md > project AGENTS.md`. Globals
    /// are always included (when present) regardless of project state; the
    /// existing in-directory `WARP.md > AGENTS.md` shadow inside
    /// [`RuleAtPath::respected_rule`] still applies to project rules.
    ///
    /// This is the entry point used by `BlocklistAIContextModel` when packing
    /// `AIAgentContext::ProjectRules` for an agent query. Callers that need
    /// a project-only signal should use
    /// [`Self::find_applicable_project_rules`] instead.
    pub fn find_applicable_rules(&self, path: &Path) -> Option<ProjectRulesResult> {
        let project_result = self.find_applicable_project_rules(path);

        // Layered precedence: global rules are always included alongside
        // project rules. `global_rules` is a `BTreeMap`, so iteration is
        // sorted by path — deterministic without needing a separate
        // ordering pass.
        let mut active_rules: Vec<ProjectRule> = self.global_rules.values().cloned().collect();
        let (project_root, additional_rule_paths) = match project_result {
            Some(project) => {
                active_rules.extend(project.active_rules);
                (Some(project.root_path), project.additional_rule_paths)
            }
            None => (None, Vec::new()),
        };

        if active_rules.is_empty() && additional_rule_paths.is_empty() {
            return None;
        }

        // Use the indexed project root when available; otherwise fall back to
        // the parent of the first global rule (or empty).
        let root_path = project_root.unwrap_or_else(|| {
            self.global_rules
                .values()
                .next()
                .and_then(|rule| rule.path.parent().map(|p| p.to_path_buf()))
                .unwrap_or_default()
        });

        Some(ProjectRulesResult {
            root_path,
            active_rules,
            additional_rule_paths,
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

    /// Absolute paths of every indexed global rule file (e.g. `~/.agents/AGENTS.md`).
    /// Iteration order is sorted by path because `global_rules` is a `BTreeMap`.
    pub fn global_rule_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.global_rules.keys().cloned()
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

/// Subscriber for a single global rules home subdir (e.g. `~/.agents`).
/// Tags every update with the originating [`GlobalRuleSource`] variant so the
/// model can dispatch to the right entry without per-source channels.
#[cfg(feature = "local_fs")]
struct GlobalRulesRepositorySubscriber {
    source: GlobalRuleSource,
    update_tx: Sender<GlobalRulesUpdate>,
}

#[cfg(feature = "local_fs")]
impl RepositorySubscriber for GlobalRulesRepositorySubscriber {
    fn on_scan(
        &mut self,
        _repository: &Repository,
        _ctx: &mut ModelContext<Repository>,
    ) -> std::pin::Pin<Box<dyn std::prelude::rust_2024::Future<Output = ()> + Send + 'static>> {
        // Initial-state read is performed separately by `spawn_global_rule_read`,
        // so the on_scan event is intentionally a no-op.
        Box::pin(async {})
    }

    fn on_files_updated(
        &mut self,
        _repository: &Repository,
        update: &repo_metadata::RepositoryUpdate,
        _ctx: &mut ModelContext<Repository>,
    ) -> std::pin::Pin<Box<dyn std::prelude::rust_2024::Future<Output = ()> + Send + 'static>> {
        let tx = self.update_tx.clone();
        let source = self.source;
        let update = update.clone();
        Box::pin(async move {
            let _ = tx.send(GlobalRulesUpdate { source, update }).await;
        })
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
