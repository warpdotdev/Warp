use crate::{
    error::UserFacingError, object::ObjectMetadata, request_context::RequestContext,
    response_context::ResponseContext, schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct UntrashObjectVariables {
    pub input: UntrashObjectInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct UntrashObjectOutput {
    pub success: bool,
    pub response_context: ResponseContext,
    pub metadata: ObjectMetadata,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RootMutation", variables = "UntrashObjectVariables")]
pub struct UntrashObject {
    #[arguments(requestContext: $request_context, input: $input)]
    pub untrash_object: UntrashObjectResult,
}
crate::client::define_operation! {
    untrash_object(UntrashObjectVariables) -> UntrashObject;
}

#[derive(cynic::InlineFragments, Debug)]
pub enum UntrashObjectResult {
    UntrashObjectOutput(UntrashObjectOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InputObject, Debug)]
pub struct UntrashObjectInput {
    pub uid: cynic::Id,
}
