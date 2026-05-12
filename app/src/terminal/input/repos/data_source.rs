//! Async data source for the inline repos menu.

#[cfg(feature = "local_fs")]
use std::future::Future;
use std::path::PathBuf;
#[cfg(feature = "local_fs")]
use std::time::Duration;

use warpui::{AppContext, Entity, SingletonEntity};

use crate::ai::persisted_workspace::PersistedWorkspace;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::{AsyncDataSource, BoxFuture, DataSourceRunErrorWrapper};
use crate::terminal::input::repos::AcceptRepo;

#[cfg(feature = "local_fs")]
const REPO_GIT_SUMMARY_TIMEOUT: Duration = Duration::from_millis(200);

pub struct RepoMenuDataSource;

impl RepoMenuDataSource {
    pub fn new() -> Self {
        Self {}
    }
}

impl AsyncDataSource for RepoMenuDataSource {
    type Action = AcceptRepo;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> BoxFuture<'static, Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper>> {
        let workspace_paths: Vec<PathBuf> = PersistedWorkspace::as_ref(app)
            .workspaces()
            .map(|m| m.path)
            .collect();

        let query_text = query.text.trim().to_lowercase();

        Box::pin(async move {
            #[cfg(feature = "local_fs")]
            {
                use crate::terminal::input::repos::search_item::RepoSearchItem;

                let futures: Vec<_> = workspace_paths
                    .into_iter()
                    .map(repo_search_item_with_summary_timeout)
                    .collect();

                let mut items: Vec<RepoSearchItem> = futures::future::join_all(futures).await;
                items.sort_by(|a, b| a.display_name.cmp(&b.display_name));

                let results: Vec<QueryResult<AcceptRepo>> = if query_text.is_empty() {
                    items.into_iter().map(QueryResult::from).collect()
                } else {
                    items
                        .into_iter()
                        .filter_map(|item| {
                            let match_result = fuzzy_match::match_indices_case_insensitive(
                                &item.display_name,
                                &query_text,
                            )?;
                            if match_result.score < 25 {
                                return None;
                            }
                            Some(QueryResult::from(
                                item.with_name_match_result(Some(match_result)),
                            ))
                        })
                        .collect()
                };

                Ok(results)
            }

            #[cfg(not(feature = "local_fs"))]
            {
                let _ = workspace_paths;
                let _ = query_text;
                Ok(vec![])
            }
        })
    }
}

#[cfg(feature = "local_fs")]
async fn repo_search_item_with_summary_timeout(
    path: PathBuf,
) -> crate::terminal::input::repos::search_item::RepoSearchItem {
    use crate::util::git::get_repo_git_summary;

    let summary_path = path.clone();
    let summary_future = async move { get_repo_git_summary(&summary_path).await };
    repo_search_item_from_summary_future(path, summary_future, REPO_GIT_SUMMARY_TIMEOUT).await
}

#[cfg(feature = "local_fs")]
async fn repo_search_item_from_summary_future<F>(
    path: PathBuf,
    summary_future: F,
    timeout: Duration,
) -> crate::terminal::input::repos::search_item::RepoSearchItem
where
    F: Future<Output = Option<crate::util::git::RepoGitSummary>>,
{
    use crate::terminal::input::repos::search_item::RepoSearchItem;
    use futures::FutureExt;
    use warpui::r#async::Timer;

    let summary_future = summary_future.fuse();
    let timeout_future = Timer::after(timeout).fuse();
    futures::pin_mut!(summary_future, timeout_future);

    let summary = futures::select! {
        summary = summary_future => summary,
        _ = timeout_future => {
            log::debug!("Timed out loading git summary for repo menu item {path:?}");
            None
        }
    };

    RepoSearchItem::new(path, summary)
}

impl Entity for RepoMenuDataSource {
    type Event = ();
}

#[cfg(all(test, feature = "local_fs"))]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use warpui::r#async::Timer;
    use warpui::App;

    use super::repo_search_item_from_summary_future;
    use crate::util::git::RepoGitSummary;

    #[test]
    fn repo_search_item_uses_path_when_git_summary_times_out() {
        App::test((), |_app| async move {
            let path = PathBuf::from("/tmp/slow-repo");
            let item = repo_search_item_from_summary_future(
                path.clone(),
                async {
                    Timer::after(Duration::from_secs(30)).await;
                    Some(RepoGitSummary {
                        branch: "main".to_owned(),
                        lines_added: 1,
                        lines_removed: 0,
                    })
                },
                Duration::from_millis(1),
            )
            .await;

            assert_eq!(item.path, path);
            assert!(item.git_summary.is_none());
        });
    }
}
