use crate::{
    error::UserFacingError, request_context::RequestContext, response_context::ResponseContext,
    scalars::Time, schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct GetScheduledAgentHistoryVariables {
    pub request_context: RequestContext,
    pub input: ScheduledAgentHistoryInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "RootQuery",
    variables = "GetScheduledAgentHistoryVariables"
)]
pub struct GetScheduledAgentHistory {
    #[arguments(requestContext: $request_context, input: $input)]
    pub scheduled_agent_history: ScheduledAgentHistoryResult,
}

crate::client::define_operation! {
    get_scheduled_agent_history(GetScheduledAgentHistoryVariables) -> GetScheduledAgentHistory;
}

#[derive(cynic::InputObject, Debug)]
pub struct ScheduledAgentHistoryInput {
    pub schedule_id: cynic::Id,
}

#[derive(cynic::InlineFragments, Debug)]
pub enum ScheduledAgentHistoryResult {
    ScheduledAgentHistoryOutput(ScheduledAgentHistoryOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ScheduledAgentHistoryOutput {
    pub history: ScheduledAgentHistory,
    pub response_context: ResponseContext,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ScheduledAgentHistory {
    pub last_ran: Option<Time>,
    pub next_run: Option<Time>,
}
