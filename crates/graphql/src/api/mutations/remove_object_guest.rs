use crate::{
    error::UserFacingError, object_permissions::ObjectPermissions, request_context::RequestContext,
    response_context::ResponseContext, schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct RemoveObjectGuestVariables {
    pub input: RemoveObjectGuestInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "RootMutation",
    variables = "RemoveObjectGuestVariables"
)]
pub struct RemoveObjectGuest {
    #[arguments(input: $input, requestContext: $request_context)]
    pub remove_object_guest: RemoveObjectGuestResult,
}
crate::client::define_operation! {
    remove_object_guest(RemoveObjectGuestVariables) -> RemoveObjectGuest;
}

#[derive(cynic::QueryFragment, Debug)]
pub struct RemoveObjectGuestOutput {
    pub object_permissions: ObjectPermissions,
    pub response_context: ResponseContext,
}

#[derive(cynic::InlineFragments, Debug)]
pub enum RemoveObjectGuestResult {
    RemoveObjectGuestOutput(RemoveObjectGuestOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InputObject, Debug)]
pub struct RemoveObjectGuestInput {
    /// Email of the user or pending user to remove. One of email or team_uid must be provided.
    pub email: Option<String>,
    pub object_uid: cynic::Id,
    /// UID of the team to remove. One of email or team_uid must be provided.
    pub team_uid: Option<cynic::Id>,
}
