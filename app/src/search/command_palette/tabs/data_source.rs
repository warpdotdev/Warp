use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::tabs::SearchItem;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::{DataSourceRunErrorWrapper, SyncDataSource};
use crate::workspace::Workspace;
use warpui::{AppContext, Entity, WeakViewHandle, WindowId};

/// Data source that produces tabs sorted by MRU order for the Ctrl+Tab palette.
pub struct DataSource {
    workspace: WeakViewHandle<Workspace>,
    window_id: WindowId,
}

impl DataSource {
    pub fn new(workspace: WeakViewHandle<Workspace>, window_id: WindowId) -> Self {
        Self {
            workspace,
            window_id,
        }
    }
}

impl SyncDataSource for DataSource {
    type Action = CommandPaletteItemAction;

    fn run_query(
        &self,
        query: &Query,
        ctx: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        let Some(workspace) = self.workspace.upgrade(ctx) else {
            return Ok(vec![]);
        };
        let tabs = workspace
            .as_ref(ctx)
            .tab_navigation_data(self.window_id, ctx);

        let query_text = query.text.trim().to_lowercase();

        let results = tabs
            .into_iter()
            .enumerate()
            .filter(|(_, tab)| {
                query_text.is_empty() || tab.title.to_lowercase().contains(&query_text)
            })
            .map(|(i, tab)| QueryResult::from(SearchItem::new(tab, i)))
            .collect();

        Ok(results)
    }
}

impl Entity for DataSource {
    type Event = ();
}
