use crate::{
    error::UserFacingError, object_permissions::Owner, request_context::RequestContext,
    response_context::ResponseContext, schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct DeleteManagedSecretVariables {
    pub input: DeleteManagedSecretInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "RootMutation",
    variables = "DeleteManagedSecretVariables"
)]
pub struct DeleteManagedSecret {
    #[arguments(input: $input, requestContext: $request_context)]
    pub delete_managed_secret: DeleteManagedSecretResult,
}

crate::client::define_operation! {
    delete_managed_secret(DeleteManagedSecretVariables) -> DeleteManagedSecret;
}

#[derive(cynic::QueryFragment, Debug)]
pub struct DeleteManagedSecretOutput {
    pub response_context: ResponseContext,
}

#[derive(cynic::InlineFragments, Debug)]
pub enum DeleteManagedSecretResult {
    DeleteManagedSecretOutput(DeleteManagedSecretOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InputObject, Debug)]
pub struct DeleteManagedSecretInput {
    pub name: String,
    pub owner: Owner,
}
