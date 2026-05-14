use anyhow::Result;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use warpui::{Entity, ModelContext, SingletonEntity};

/// 默认规则文件列表。顺序 = 优先级(靠前者优先);同目录多个文件
/// 同时存在时 `RuleAtPath::respected_rule()` 只取优先级最高的一个。
///
/// - WARP.md  项目原生约定。
/// - AGENTS.md 社区通用(opencode / Cursor / Cline 等都识别)。
/// - CLAUDE.md Claude Code 原生约定，让从 Claude Code 迁过来的项目一键可用。
///
/// 扩展新名称只需调整本数组(插入位置 = 优先级),`RuleAtPath`
/// 以 priority-indexed slot 数组实现，不需动 if-else 逻辑。
///
/// 定义在 `cfg_if` 外部，以便不编译 `local_fs` 的路径(WASM / 测试)也能引用。
pub(crate) const RULES_FILE_PATTERN: &[&str] = &["WARP.md", "AGENTS.md", "CLAUDE.md"];

cfg_if::cfg_if! {
    if #[cfg(feature = "local_fs")] {
        use repo_metadata::entry::{Entry, FileMetadata};
        use repo_metadata::repository::RepositorySubscriber;
        use repo_metadata::{Repository, DirectoryWatcher, RepositoryUpdate};
        use ignore::gitignore::Gitignore;
        use async_channel::Sender;
        // `instant::Instant` 是本仓库全局约定的跨平台(含 WASM)起点,代替
        // `std::time::Instant`。使用 `clippy.toml` 中的 disallowed_types 强制。
        use instant::Instant;
        use std::time::{Duration, SystemTime};

        const MAX_SCAN_DEPTH: usize = 3;
        const MAX_FILES_TO_SCAN: usize = 5000;

        // —— Fast-path(对齐 opencode `findUp` 模式)——
        //
        // 主用途:cd 进入新 git 仓库后,异步 `index_and_store_rules` 完成前的
        // 时间窗口内,`pending_context` 同步调用此 fast-path 直接 stat + 读 cwd
        // 及其祖先目录的规则文件,保证 AGENTS.md / WARP.md / CLAUDE.md
        // **不会因为异步竞争漏注入**。
        // 正常路径(`find_applicable_rules`)一旦可用,fast-path 让位并清缓存。
        //
        // UI 不卡顿保障:
        //   - 单次最坏 `MAX_WALK_DEPTH * RULES_FILE_PATTERN.len()` 次 metadata
        //     + 命中文件 `read_to_string`(规则文件一般几 KB,Windows NTFS < 1ms/文件)。
        //   - `FAST_PATH_BUDGET` 时间预算硬截断,超时立即返回已收集部分,绝不阻塞。
        //   - 稳态命中(目录无变化)只做 stat,不重读文件;mtime / size / parent-dir-mtime
        //     任一变化即重扫。
        const MAX_WALK_DEPTH: usize = 6;
        const FAST_PATH_BUDGET: Duration = Duration::from_millis(20);
    }
}

/// Fast-path 缓存条目。`stamps` 记录已命中文件的 (path, mtime, size),
/// `walked_dir_stamps` 记录遍历过的目录的 (path, mtime),用于检测
/// "目录里新增 / 删除 / 修改了规则文件"两类失效。`negative` 缓存表示
/// 上次扫描没找到任何规则,后续相同 stamps 直接复用,不再 IO。
#[cfg(feature = "local_fs")]
#[derive(Clone, Debug)]
struct FastPathEntry {
    rules: Vec<ProjectRule>,
    /// fast-path 用的 "root" — 取**首层命中**的目录;全 miss 时取 cwd 本身。
    /// 用于构造 `ProjectRulesResult.root_path`,语义对齐 `find_applicable_rules`。
    root_path: PathBuf,
    stamps: Vec<(PathBuf, SystemTime, u64)>,
    walked_dir_stamps: Vec<(PathBuf, SystemTime)>,
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
    /// Fast-path 同步规则缓存(对齐 opencode `findUp` 模式)。
    ///
    /// 仅在 `find_applicable_rules` 返回 None(异步索引未就绪 / 不在已索引根下)时
    /// 兜底使用,避免 cd 后立即发 AI 请求时漏注入 AGENTS.md / WARP.md。
    /// 单线程访问(WarpUI Singleton 在 main thread),用 `RefCell` 而非锁,
    /// 满足 `pending_context(&self, app: &AppContext)` 这种 `&self` 调用形态。
    #[cfg(feature = "local_fs")]
    fast_path_cache: RefCell<HashMap<PathBuf, FastPathEntry>>,
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
                // OpenWarp:原这里会对每个持久化的 root 调
                // `try_initialize_and_register_watcher`,后者内部走
                // `DetectedRepositories::detect_possible_git_repo(ProjectRulesIndexing)`
                // 触发事件,让 RepoMetadataModel 全量索引 6 个持久化仓库
                // (对 OpenWarp BYOP 是冷启动最大后台 CPU 耗费头)。
                //
                // 现只填充 in-memory 的 path_to_rules cache,不主动发
                // detect 事件。用户后续通过 terminal cd 进入仓库时,
                // RepoDetectionSource::TerminalNavigation 会自然触发独立的 detect,
                // 到时再走 register_watcher_for_path。
                //
                // 实际影响:持久化 rules 未被实时 watch,直到用户进入该仓库。
                // cache 本身仍可用,AI 查 rule 不受影响。
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

    // OpenWarp:原 `try_initialize_and_register_watcher` 是从持久化 rule 路径启动时
    // 强制 detect repo 的入口,启动后续走 RepoMetadataModel 全量索引。已随
    // `new_from_persisted` 中的 detect 调用一并删除;现在仅靠 terminal cd 触发的
    // `RepoDetectionSource::TerminalNavigation` 路径被动走 register_watcher_for_path。

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

    /// 规则查询的统一入口:正常路径优先,异步索引未就绪时同步 fast-path 兜底。
    ///
    /// 对齐 opencode `Instruction.systemPaths()` 的 `findUp` 行为(
    /// `opencode/packages/opencode/src/session/instruction.ts`):从 cwd 起逐级
    /// 向上 stat 规则文件,首层命中即停。fast-path 与正常路径**绝不并存**:
    /// 正常路径一返回 Some,立即清掉 fast-path cache 中对应条目,确保索引完成后
    /// 后续请求百分百走正常路径(能拿到子目录规则 + watcher 实时更新)。
    #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
    pub fn find_rules_with_fast_path(&self, cwd: &Path) -> Option<ProjectRulesResult> {
        if let Some(found) = self.find_applicable_rules(cwd) {
            #[cfg(feature = "local_fs")]
            {
                // 正常路径已可用,丢弃 fast-path cache(避免后续拿到 stale 数据)。
                self.fast_path_cache.borrow_mut().remove(cwd);
            }
            return Some(found);
        }

        #[cfg(feature = "local_fs")]
        {
            return self.fast_path_lookup(cwd);
        }

        #[allow(unreachable_code)]
        None
    }

    /// Fast-path 同步查找 + 读取 cwd 及祖先目录的规则文件。只在正常路径 None 时调。
    ///
    /// 返回语义与 `find_applicable_rules` 一致:
    ///   - Some(ProjectRulesResult) 带至少 1 个 active rule
    ///   - None 表示未找到任何规则(已写 negative cache,后续相同 stamps 不再 IO)
    #[cfg(feature = "local_fs")]
    fn fast_path_lookup(&self, cwd: &Path) -> Option<ProjectRulesResult> {
        // 1) 缓存命中路径:stat 一遍 stamps,全部对齐则复用缓存(不重读文件)。
        if let Some(entry) = self.fast_path_cache.borrow().get(cwd).cloned() {
            if Self::fast_path_entry_still_valid(&entry) {
                return Self::result_from_fast_path_entry(&entry);
            }
        }

        // 2) 缓存 miss / 失效:同步扫描。预算 `FAST_PATH_BUDGET` 硬截断,UI 绝不卡。
        let entry = Self::scan_fast_path(cwd);
        let result = Self::result_from_fast_path_entry(&entry);
        self.fast_path_cache
            .borrow_mut()
            .insert(cwd.to_path_buf(), entry);
        result
    }

    /// 从 `start` 起逐级向上同步 stat + 读取规则文件。对齐 opencode `findUp`,
    /// 但添加 `MAX_WALK_DEPTH` + `FAST_PATH_BUDGET` 双保障让 UI 绝不阻塞。
    ///
    /// 每层依 `RULES_FILE_PATTERN`(WARP.md > AGENTS.md)取首个命中的,对齐
    /// `RuleAtPath::respected_rule()` 语义。
    #[cfg(feature = "local_fs")]
    fn scan_fast_path(start: &Path) -> FastPathEntry {
        let deadline = Instant::now() + FAST_PATH_BUDGET;
        let mut rules = Vec::new();
        let mut stamps = Vec::new();
        let mut walked_dir_stamps = Vec::new();
        let mut first_hit_dir: Option<PathBuf> = None;
        let mut current: PathBuf = start.to_path_buf();

        for _ in 0..MAX_WALK_DEPTH {
            if Instant::now() >= deadline {
                break;
            }

            // 记录目录 mtime,后续可以识别"目录里新增/删除了规则文件"两类变动。
            if let Ok(meta) = std::fs::metadata(&current) {
                if let Ok(mtime) = meta.modified() {
                    walked_dir_stamps.push((current.clone(), mtime));
                }
            }

            // 本层按优先级查找首个规则文件。对齐 RuleAtPath::respected_rule() 语义。
            for filename in RULES_FILE_PATTERN {
                if Instant::now() >= deadline {
                    break;
                }
                let candidate = current.join(filename);
                let Ok(meta) = std::fs::metadata(&candidate) else {
                    continue;
                };
                if !meta.is_file() {
                    continue;
                }
                let Ok(mtime) = meta.modified() else { continue };
                let size = meta.len();
                let Ok(content) = std::fs::read_to_string(&candidate) else {
                    continue;
                };
                if first_hit_dir.is_none() {
                    first_hit_dir = Some(current.clone());
                }
                rules.push(ProjectRule {
                    path: candidate.clone(),
                    content,
                });
                stamps.push((candidate, mtime, size));
                break; // 本层只取 1 个
            }

            if !current.pop() {
                break;
            }
        }

        FastPathEntry {
            root_path: first_hit_dir.unwrap_or_else(|| start.to_path_buf()),
            rules,
            stamps,
            walked_dir_stamps,
        }
    }

    /// 缓存失效检查。只 stat,不读文件内容。
    /// - 命中文件 mtime/size 不变 → 内容可复用
    /// - 遍历过的目录 mtime 不变 → 不会有新增/删除的规则文件
    ///
    /// 带 `FAST_PATH_BUDGET` 预算,stat 期间超时即视为失效重扫。
    #[cfg(feature = "local_fs")]
    fn fast_path_entry_still_valid(entry: &FastPathEntry) -> bool {
        let deadline = Instant::now() + FAST_PATH_BUDGET;
        for (path, mtime, size) in &entry.stamps {
            if Instant::now() >= deadline {
                return false;
            }
            let Ok(meta) = std::fs::metadata(path) else {
                return false;
            };
            if meta.len() != *size {
                return false;
            }
            if meta.modified().ok().as_ref() != Some(mtime) {
                return false;
            }
        }
        for (dir, mtime) in &entry.walked_dir_stamps {
            if Instant::now() >= deadline {
                return false;
            }
            let Ok(meta) = std::fs::metadata(dir) else {
                return false;
            };
            if meta.modified().ok().as_ref() != Some(mtime) {
                return false;
            }
        }
        true
    }

    /// 把 FastPathEntry 转换为对外统一的 `ProjectRulesResult`。
    /// 空 rules 返 None,语义对齐 `find_applicable_rules`。
    #[cfg(feature = "local_fs")]
    fn result_from_fast_path_entry(entry: &FastPathEntry) -> Option<ProjectRulesResult> {
        if entry.rules.is_empty() {
            return None;
        }
        Some(ProjectRulesResult {
            root_path: entry.root_path.clone(),
            active_rules: entry.rules.clone(),
            additional_rule_paths: Vec::new(),
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
