//! Data source for the inline repos menu.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use warpui::{AppContext, Entity, ModelContext, ModelHandle, SingletonEntity};

use crate::ai::persisted_workspace::PersistedWorkspace;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::{DataSourceRunErrorWrapper, SyncDataSource};
#[cfg(feature = "local_fs")]
use crate::terminal::input::repos::search_item::{repo_display_name, RepoSearchItem};
use crate::terminal::input::repos::AcceptRepo;
#[cfg(feature = "local_fs")]
use crate::util::git::{get_repo_git_summary, RepoGitSummary};

#[cfg(feature = "local_fs")]
const GIT_SUMMARY_TTL: Duration = Duration::from_secs(30);
#[cfg(feature = "local_fs")]
const MAX_CONCURRENT_GIT_SUMMARIES: usize = 4;

#[derive(Debug, Clone)]
pub enum RepoGitSummaryCacheEvent {
    Updated,
}

pub struct RepoGitSummaryCache {
    #[cfg(feature = "local_fs")]
    summaries: HashMap<PathBuf, RepoGitSummaryCacheEntry>,
    #[cfg(feature = "local_fs")]
    in_flight: HashSet<PathBuf>,
}

#[cfg(feature = "local_fs")]
struct RepoGitSummaryCacheEntry {
    summary: Option<RepoGitSummary>,
    updated_at: instant::Instant,
}

impl RepoGitSummaryCache {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "local_fs")]
            summaries: HashMap::new(),
            #[cfg(feature = "local_fs")]
            in_flight: HashSet::new(),
        }
    }

    #[cfg(feature = "local_fs")]
    pub fn summary(&self, path: &Path) -> Option<RepoGitSummary> {
        self.summaries
            .get(path)
            .and_then(|entry| entry.summary.clone())
    }

    pub fn refresh_missing(&mut self, paths: Vec<PathBuf>, ctx: &mut ModelContext<Self>) {
        #[cfg(feature = "local_fs")]
        {
            use futures_util::stream::{self, StreamExt};

            let now = instant::Instant::now();
            let paths_to_refresh = paths
                .into_iter()
                .filter(|path| {
                    if self.in_flight.contains(path) {
                        return false;
                    }

                    !self
                        .summaries
                        .get(path)
                        .is_some_and(|entry| now.duration_since(entry.updated_at) < GIT_SUMMARY_TTL)
                })
                .collect::<Vec<_>>();

            if paths_to_refresh.is_empty() {
                return;
            }

            self.in_flight.extend(paths_to_refresh.iter().cloned());

            let stream = stream::iter(paths_to_refresh)
                .map(|path| async move {
                    let summary = get_repo_git_summary(&path).await;
                    (path, summary)
                })
                .buffer_unordered(MAX_CONCURRENT_GIT_SUMMARIES);

            ctx.spawn_stream_local(
                stream,
                |me, (path, summary), ctx| {
                    me.in_flight.remove(&path);
                    me.summaries.insert(
                        path,
                        RepoGitSummaryCacheEntry {
                            summary,
                            updated_at: instant::Instant::now(),
                        },
                    );
                    ctx.emit(RepoGitSummaryCacheEvent::Updated);
                },
                |_, _| {},
            );
        }

        #[cfg(not(feature = "local_fs"))]
        {
            let _ = paths;
            let _ = ctx;
        }
    }
}

impl Entity for RepoGitSummaryCache {
    type Event = RepoGitSummaryCacheEvent;
}

pub struct RepoMenuDataSource {
    git_summary_cache: ModelHandle<RepoGitSummaryCache>,
}

impl RepoMenuDataSource {
    pub fn new(git_summary_cache: ModelHandle<RepoGitSummaryCache>) -> Self {
        Self { git_summary_cache }
    }

    pub fn matching_paths(query: &Query, app: &AppContext) -> Vec<PathBuf> {
        let query_text = normalized_query_text(query);
        PersistedWorkspace::as_ref(app)
            .workspaces()
            .map(|m| m.path)
            .filter(|path| repo_matches_query(path, &query_text))
            .collect()
    }
}

impl SyncDataSource for RepoMenuDataSource {
    type Action = AcceptRepo;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        #[cfg(feature = "local_fs")]
        {
            let query_text = normalized_query_text(query);
            let git_summary_cache = self.git_summary_cache.as_ref(app);
            let mut items = PersistedWorkspace::as_ref(app)
                .workspaces()
                .filter_map(|metadata| {
                    let display_name = repo_display_name(&metadata.path);
                    let match_result = match_result(&display_name, &query_text)?;
                    let git_summary = git_summary_cache.summary(&metadata.path);
                    Some(
                        RepoSearchItem::new(metadata.path, display_name, git_summary)
                            .with_name_match_result(match_result),
                    )
                })
                .collect::<Vec<_>>();

            items.sort_by(|a, b| a.display_name.cmp(&b.display_name));
            Ok(items.into_iter().map(QueryResult::from).collect())
        }

        #[cfg(not(feature = "local_fs"))]
        {
            let _ = query;
            let _ = app;
            Ok(vec![])
        }
    }
}

fn normalized_query_text(query: &Query) -> String {
    query.text.trim().to_lowercase()
}

fn repo_matches_query(path: &Path, query_text: &str) -> bool {
    #[cfg(feature = "local_fs")]
    {
        let display_name = repo_display_name(path);
        match_result(&display_name, query_text).is_some()
    }

    #[cfg(not(feature = "local_fs"))]
    {
        let _ = path;
        let _ = query_text;
        false
    }
}

#[cfg(feature = "local_fs")]
fn match_result(
    display_name: &str,
    query_text: &str,
) -> Option<Option<fuzzy_match::FuzzyMatchResult>> {
    if query_text.is_empty() {
        return Some(None);
    }

    let match_result = fuzzy_match::match_indices_case_insensitive(display_name, query_text)?;
    if match_result.score < 25 {
        return None;
    }

    Some(Some(match_result))
}
