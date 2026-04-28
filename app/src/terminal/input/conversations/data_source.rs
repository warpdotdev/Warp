//! Data source for the inline conversation menu.

use itertools::Itertools;
use ordered_float::OrderedFloat;
use warpui::{AppContext, Entity, ModelHandle};

use crate::ai::blocklist::agent_view::AgentViewController;
use crate::ai::conversation_navigation::ConversationNavigationData;
use crate::search::data_source::{Query, QueryFilter, QueryResult};
use crate::search::mixer::DataSourceRunErrorWrapper;
use crate::search::SyncDataSource;
use crate::terminal::input::conversations::search_item::ConversationSearchItem;
use crate::terminal::input::conversations::AcceptConversation;
use crate::terminal::model::session::active_session::ActiveSession;

pub struct ConversationMenuDataSource {
    agent_view_controller: ModelHandle<AgentViewController>,
    active_session: ModelHandle<ActiveSession>,
}

impl ConversationMenuDataSource {
    pub fn new(
        agent_view_controller: ModelHandle<AgentViewController>,
        active_session: ModelHandle<ActiveSession>,
    ) -> Self {
        Self {
            agent_view_controller,
            active_session,
        }
    }
}

impl SyncDataSource for ConversationMenuDataSource {
    type Action = AcceptConversation;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        let conversation_navigation_data = ConversationNavigationData::all_conversations(app);
        let query_text = query.text.trim().to_lowercase();

        let active_conversation_id = self
            .agent_view_controller
            .as_ref(app)
            .agent_view_state()
            .active_conversation_id();

        let filter_by_cwd = query
            .filters
            .contains(&QueryFilter::CurrentDirectoryConversations);
        let session_pwd = if filter_by_cwd {
            self.active_session
                .as_ref(app)
                .current_working_directory()
                .cloned()
        } else {
            None
        };

        // When the "Current Directory" filter is active, include only conversations
        // whose most recent directory (falling back to initial directory) matches
        // the session's current working directory. If we can't determine the
        // session CWD, leave the results unfiltered.
        let matches_directory = |data: &ConversationNavigationData| -> bool {
            if !filter_by_cwd {
                return true;
            }
            let Some(session_pwd) = session_pwd.as_deref() else {
                return true;
            };
            data.latest_working_directory
                .as_deref()
                .or(data.initial_working_directory.as_deref())
                .is_some_and(|dir| {
                    dir.trim_end_matches(std::path::MAIN_SEPARATOR)
                        == session_pwd.trim_end_matches(std::path::MAIN_SEPARATOR)
                })
        };

        if query_text.is_empty() {
            // By default, show 50 most recent conversations in the list.
            const DEFAULT_RESULT_COUNT: usize = 50;

            // In the zero state, sort conversations in the active pane above all other conversations.
            // Within each segment, sort to reverse chronological order.
            Ok(conversation_navigation_data
                .into_iter()
                // Don't show the currently open conversation, that's redundant.
                .filter(|data| Some(data.id()) != active_conversation_id)
                .filter(|data| matches_directory(data))
                .sorted_by(|a, b| b.last_updated.cmp(&a.last_updated))
                .take(DEFAULT_RESULT_COUNT)
                .map(|navigation_data| {
                    QueryResult::from(ConversationSearchItem::new(navigation_data, app))
                })
                .rev()
                .collect())
        } else {
            let mut search_results = conversation_navigation_data
                .into_iter()
                .filter_map(|navigation_data| {
                    if Some(navigation_data.id()) == active_conversation_id {
                        // Don't show the currently open conversation, that's redundant.
                        return None;
                    }
                    if !matches_directory(&navigation_data) {
                        return None;
                    }
                    let match_result = fuzzy_match::match_indices_case_insensitive(
                        &navigation_data.title,
                        &query_text,
                    )?;

                    // 25 is arbitrary.
                    if match_result.score < 25 {
                        return None;
                    }

                    Some(QueryResult::from(
                        ConversationSearchItem::new(navigation_data, app)
                            .with_name_match_result(Some(match_result.clone()))
                            .with_score(OrderedFloat(match_result.score as f64)),
                    ))
                })
                .sorted_by(|a, b| b.score().cmp(&a.score()))
                .collect_vec();

            // This is basically here so the app doesn't choke.
            const MAX_SEARCH_RESULTS: usize = 500;

            search_results.truncate(MAX_SEARCH_RESULTS);
            Ok(search_results)
        }
    }
}

impl Entity for ConversationMenuDataSource {
    type Event = ();
}
