//! Client-side metadata for auth secret types per harness.
//!
//! This mirrors the server's `authSecretTypesByHarness` map and defines
//! the input fields needed to create each secret type.

use warp_cli::agent::Harness;
use warp_graphql::managed_secrets::ManagedSecretType;

/// A single input field within an auth secret type.
pub struct AuthSecretTypeField {
    /// Label shown above the input field (e.g. "ANTHROPIC_API_KEY").
    pub label: &'static str,
    /// JSON key when constructing the encrypted value payload.
    pub json_key: &'static str,
    /// Whether this field is optional.
    pub optional: bool,
}

/// Metadata for one auth secret type that a harness supports.
pub struct AuthSecretTypeInfo {
    /// User-facing display name (e.g. "Anthropic API Key").
    pub display_name: &'static str,
    /// The GraphQL `ManagedSecretType` variant for this secret.
    pub secret_type: ManagedSecretType,
    /// Fields the user must fill in to create this secret.
    pub fields: &'static [AuthSecretTypeField],
}

/// Returns the auth secret types available for the given harness.
/// Returns an empty slice for harnesses that do not use auth secrets (e.g. Oz).
pub fn auth_secret_types_for_harness(harness: Harness) -> &'static [AuthSecretTypeInfo] {
    match harness {
        Harness::Claude => &CLAUDE_AUTH_SECRET_TYPES,
        _ => &[],
    }
}

static CLAUDE_AUTH_SECRET_TYPES: [AuthSecretTypeInfo; 3] = [
    AuthSecretTypeInfo {
        display_name: "Anthropic API Key",
        secret_type: ManagedSecretType::AnthropicApiKey,
        fields: &[AuthSecretTypeField {
            label: "ANTHROPIC_API_KEY",
            json_key: "api_key",
            optional: false,
        }],
    },
    AuthSecretTypeInfo {
        display_name: "Bedrock API Key",
        secret_type: ManagedSecretType::AnthropicBedrockApiKey,
        fields: &[
            AuthSecretTypeField {
                label: "AWS_BEARER_TOKEN_BEDROCK",
                json_key: "aws_bearer_token_bedrock",
                optional: false,
            },
            AuthSecretTypeField {
                label: "AWS_REGION",
                json_key: "aws_region",
                optional: false,
            },
        ],
    },
    AuthSecretTypeInfo {
        display_name: "Bedrock Access Key",
        secret_type: ManagedSecretType::AnthropicBedrockAccessKey,
        fields: &[
            AuthSecretTypeField {
                label: "AWS_ACCESS_KEY_ID",
                json_key: "aws_access_key_id",
                optional: false,
            },
            AuthSecretTypeField {
                label: "AWS_SECRET_ACCESS_KEY",
                json_key: "aws_secret_access_key",
                optional: false,
            },
            AuthSecretTypeField {
                label: "AWS_SESSION_TOKEN",
                json_key: "aws_session_token",
                optional: true,
            },
            AuthSecretTypeField {
                label: "AWS_REGION",
                json_key: "aws_region",
                optional: false,
            },
        ],
    },
];
