use crate::{
    error::UserFacingError, request_context::RequestContext, response_context::ResponseContext,
    schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct CreateFileArtifactUploadTargetVariables {
    pub input: CreateFileArtifactUploadTargetInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "RootMutation",
    variables = "CreateFileArtifactUploadTargetVariables"
)]
pub struct CreateFileArtifactUploadTarget {
    #[arguments(input: $input, requestContext: $request_context)]
    pub create_file_artifact_upload_target: CreateFileArtifactUploadTargetResult,
}

crate::client::define_operation! {
    create_file_artifact_upload_target(CreateFileArtifactUploadTargetVariables) -> CreateFileArtifactUploadTarget;
}

#[derive(cynic::InlineFragments, Debug)]
pub enum CreateFileArtifactUploadTargetResult {
    CreateFileArtifactUploadTargetOutput(CreateFileArtifactUploadTargetOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct CreateFileArtifactUploadTargetOutput {
    pub artifact: FileArtifact,
    pub response_context: ResponseContext,
    pub upload_target: FileArtifactUploadTarget,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct FileArtifact {
    pub artifact_uid: cynic::Id,
    pub filepath: String,
    pub description: Option<String>,
    pub mime_type: String,
    pub size_bytes: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "CreateUploadTarget")]
pub struct FileArtifactUploadTarget {
    pub url: String,
    pub method: String,
    pub headers: Vec<FileArtifactUploadHeader>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "CreateUploadTargetHeader")]
pub struct FileArtifactUploadHeader {
    pub name: String,
    pub value: String,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "CreateFileArtifactUploadTargetInput")]
pub struct CreateFileArtifactUploadTargetInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<cynic::Id>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<cynic::Id>,
    pub filepath: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i32>,
}
