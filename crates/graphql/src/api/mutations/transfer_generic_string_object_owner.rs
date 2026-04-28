use crate::{
    error::UserFacingError, object::ObjectMetadata, object_permissions::Owner,
    request_context::RequestContext, response_context::ResponseContext, schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct TransferGenericStringObjectOwnerVariables {
    pub input: TransferGenericStringObjectOwnerInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct TransferGenericStringObjectOwnerOutput {
    pub metadata: ObjectMetadata,
    pub response_context: ResponseContext,
    pub success: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "RootMutation",
    variables = "TransferGenericStringObjectOwnerVariables"
)]
pub struct TransferGenericStringObjectOwner {
    #[arguments(requestContext: $request_context, input: $input)]
    pub transfer_generic_string_object_owner: TransferGenericStringObjectOwnerResult,
}
crate::client::define_operation! {
    transfer_generic_string_object_owner(TransferGenericStringObjectOwnerVariables) -> TransferGenericStringObjectOwner;
}

#[derive(cynic::InlineFragments, Debug)]
pub enum TransferGenericStringObjectOwnerResult {
    TransferGenericStringObjectOwnerOutput(TransferGenericStringObjectOwnerOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InputObject, Debug)]
pub struct TransferGenericStringObjectOwnerInput {
    pub owner: Owner,
    pub uid: cynic::Id,
}
