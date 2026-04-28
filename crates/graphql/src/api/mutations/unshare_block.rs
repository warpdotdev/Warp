use crate::{
    error::UserFacingError, request_context::RequestContext, response_context::ResponseContext,
    schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct UnshareBlockVariables {
    pub input: UnshareBlockInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct UnshareBlockOutput {
    pub success: bool,
    pub response_context: ResponseContext,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RootMutation", variables = "UnshareBlockVariables")]
pub struct UnshareBlock {
    #[arguments(input: $input, requestContext: $request_context)]
    pub unshare_block: UnshareBlockResult,
}
crate::client::define_operation! {
    unshare_block(UnshareBlockVariables) -> UnshareBlock;
}

#[derive(cynic::InlineFragments, Debug)]
pub enum UnshareBlockResult {
    UnshareBlockOutput(UnshareBlockOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InputObject, Debug)]
pub struct UnshareBlockInput {
    pub block_uid: String,
}
