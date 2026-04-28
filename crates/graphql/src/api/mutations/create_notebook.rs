use crate::scalars::Time;
use crate::{
    error::UserFacingError, notebook::Notebook, object::CloudObjectEventEntrypoint,
    object_permissions::Owner, request_context::RequestContext, response_context::ResponseContext,
    schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct CreateNotebookVariables {
    pub input: CreateNotebookInput,
    pub request_context: RequestContext,
}

#[derive(cynic::InputObject, Debug)]
pub struct CreateNotebookInput {
    pub ai_document_id: Option<String>,
    pub conversation_id: Option<String>,
    pub data: Option<String>,
    pub entrypoint: CloudObjectEventEntrypoint,
    pub initial_folder_id: Option<cynic::Id>,
    pub owner: Owner,
    pub title: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RootMutation", variables = "CreateNotebookVariables")]
pub struct CreateNotebook {
    #[arguments(input: $input, requestContext: $request_context)]
    pub create_notebook: CreateNotebookResult,
}
crate::client::define_operation! {
    create_notebook(CreateNotebookVariables) -> CreateNotebook;
}

#[derive(cynic::QueryFragment, Debug)]
pub struct CreateNotebookOutput {
    pub notebook: Notebook,
    pub response_context: ResponseContext,
    pub revision_ts: Time,
}

#[derive(cynic::InlineFragments, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CreateNotebookResult {
    CreateNotebookOutput(CreateNotebookOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}
