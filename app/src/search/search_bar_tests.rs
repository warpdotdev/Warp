use crate::search::QueryFilter;

use super::{FilterState, SearchBarState, SearchResultOrdering};

#[derive(Clone, Debug)]
struct TestAction;

#[test]
fn skips_empty_query_when_showing_zero_state() {
    let state = SearchBarState::<TestAction>::new(SearchResultOrdering::TopDown);

    assert!(state.should_show_zero_state_for_buffer(""));
    assert!(!state.should_run_query_for_buffer(""));
}

#[test]
fn runs_empty_query_when_search_bar_opts_in() {
    let state = SearchBarState::<TestAction>::new(SearchResultOrdering::TopDown)
        .run_query_on_buffer_empty();

    assert!(state.should_show_zero_state_for_buffer(""));
    assert!(state.should_run_query_for_buffer(""));
}

#[test]
fn runs_query_when_filter_is_visible() {
    let mut state = SearchBarState::<TestAction>::new(SearchResultOrdering::TopDown);
    state.query_filter = FilterState::Visible(QueryFilter::Files);

    assert!(!state.should_show_zero_state_for_buffer(""));
    assert!(state.should_run_query_for_buffer(""));
}

#[test]
fn runs_query_when_buffer_has_text() {
    let state = SearchBarState::<TestAction>::new(SearchResultOrdering::TopDown);

    assert!(!state.should_show_zero_state_for_buffer("git"));
    assert!(state.should_run_query_for_buffer("git"));
}
