use crate::{
    error::UserFacingError,
    object::{ObjectMetadata, ObjectType},
    object_permissions::Owner,
    request_context::RequestContext,
    response_context::ResponseContext,
    schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct MoveObjectVariables {
    pub input: MoveObjectInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RootMutation", variables = "MoveObjectVariables")]
pub struct MoveObject {
    #[arguments(input: $input, requestContext: $request_context)]
    pub move_object: MoveObjectResult,
}
crate::client::define_operation! {
    move_object(MoveObjectVariables) -> MoveObject;
}

#[derive(cynic::QueryFragment, Debug)]
pub struct MoveObjectOutput {
    pub success: bool,
    pub response_context: ResponseContext,
    pub metadata: ObjectMetadata,
}

#[derive(cynic::InlineFragments, Debug)]
pub enum MoveObjectResult {
    MoveObjectOutput(MoveObjectOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InputObject, Debug)]
pub struct MoveObjectInput {
    pub new_folder_uid: Option<cynic::Id>,
    pub new_owner: Owner,
    pub object_type: ObjectType,
    pub uid: cynic::Id,
}
