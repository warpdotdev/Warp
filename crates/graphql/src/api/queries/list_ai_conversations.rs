use crate::{
    ai::AIConversation, ai::AIConversationArtifact, ai::AgentHarness, ai::ConversationUsage,
    error::UserFacingError, object::ObjectMetadata, object_permissions::ObjectPermissions,
    request_context::RequestContext, response_context::ResponseContext, schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct ListAIConversationsVariables {
    pub input: ListAIConversationsInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RootQuery", variables = "ListAIConversationsVariables")]
pub struct ListAIConversations {
    #[arguments(input: $input, requestContext: $request_context)]
    #[cynic(rename = "listAIConversations")]
    pub list_ai_conversations: ListAIConversationsResult,
}
crate::client::define_operation! {
    list_ai_conversations(ListAIConversationsVariables) -> ListAIConversations;
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ListAIConversationsOutput {
    pub conversations: Vec<AIConversation>,
    pub response_context: ResponseContext,
}

#[derive(cynic::InlineFragments, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ListAIConversationsResult {
    ListAIConversationsOutput(ListAIConversationsOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InputObject, Debug)]
pub struct ListAIConversationsInput {
    pub conversation_ids: Option<Vec<cynic::Id>>,
}

// Metadata-only fragment that omits final_task_list for efficiency
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "AIConversation")]
pub struct AIConversationMetadata {
    pub conversation_id: cynic::Id,
    pub harness: AgentHarness,
    pub title: String,
    pub working_directory: Option<String>,
    pub usage: ConversationUsage,
    pub metadata: ObjectMetadata,
    pub permissions: ObjectPermissions,
    pub ambient_agent_task_id: Option<cynic::Id>,
    pub artifacts: Option<Vec<AIConversationArtifact>>,
}

// Query and types for listing metadata only (without final_task_list)
#[derive(cynic::QueryVariables, Debug)]
pub struct ListAIConversationMetadataVariables {
    pub input: ListAIConversationsInput,
    pub request_context: RequestContext,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "RootQuery",
    variables = "ListAIConversationMetadataVariables"
)]
pub struct ListAIConversationMetadata {
    #[arguments(input: $input, requestContext: $request_context)]
    #[cynic(rename = "listAIConversations")]
    pub list_ai_conversations: ListAIConversationMetadataResult,
}

crate::client::define_operation! {
    list_ai_conversation_metadata(ListAIConversationMetadataVariables) -> ListAIConversationMetadata;
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ListAIConversationsOutput")]
pub struct ListAIConversationMetadataOutput {
    pub conversations: Vec<AIConversationMetadata>,
    pub response_context: ResponseContext,
}

#[derive(cynic::InlineFragments, Debug)]
#[cynic(graphql_type = "ListAIConversationsResult")]
#[allow(clippy::large_enum_variant)]
pub enum ListAIConversationMetadataResult {
    ListAIConversationsOutput(ListAIConversationMetadataOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}
