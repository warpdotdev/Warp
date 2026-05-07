//! Client-side metadata for auth secret types per harness.
//!
//! This mirrors the server's `authSecretTypesByHarness` map and defines
//! the input fields needed to create each secret type.

use anyhow::{anyhow, Result};
use warp_cli::agent::Harness;
use warp_graphql::managed_secrets::ManagedSecretType;
use warp_managed_secrets::ManagedSecretValue;

/// A single input field within an auth secret type.
pub struct AuthSecretTypeField {
    /// Label shown above the input field (e.g. "ANTHROPIC_API_KEY").
    pub label: &'static str,
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

/// Builds a [`ManagedSecretValue`] from the given filled-in field values for
/// the secret type. The values must be in the same order as `info.fields`.
/// Returns an error if the secret type is not supported here.
pub fn build_managed_secret_value(
    info: &AuthSecretTypeInfo,
    field_values: &[String],
) -> Result<ManagedSecretValue> {
    if field_values.len() != info.fields.len() {
        return Err(anyhow!(
            "Expected {} field values, got {}",
            info.fields.len(),
            field_values.len()
        ));
    }
    // Validate non-optional fields are non-empty.
    for (field, value) in info.fields.iter().zip(field_values.iter()) {
        if !field.optional && value.trim().is_empty() {
            return Err(anyhow!("Field '{}' is required", field.label));
        }
    }
    match info.secret_type {
        ManagedSecretType::AnthropicApiKey => Ok(ManagedSecretValue::anthropic_api_key(
            field_values[0].clone(),
        )),
        ManagedSecretType::AnthropicBedrockApiKey => {
            Ok(ManagedSecretValue::anthropic_bedrock_api_key(
                field_values[0].clone(),
                field_values[1].clone(),
            ))
        }
        ManagedSecretType::AnthropicBedrockAccessKey => {
            // Session token is optional; treat empty as None so the server
            // accepts persistent IAM credentials.
            let session_token = if field_values[2].trim().is_empty() {
                None
            } else {
                Some(field_values[2].clone())
            };
            Ok(ManagedSecretValue::anthropic_bedrock_access_key(
                field_values[0].clone(),
                field_values[1].clone(),
                session_token,
                field_values[3].clone(),
            ))
        }
        ManagedSecretType::RawValue | ManagedSecretType::Dotenvx => Err(anyhow!(
            "Auth secret type {:?} is not supported via the harness FTUX flow",
            info.secret_type
        )),
    }
}

static CLAUDE_AUTH_SECRET_TYPES: [AuthSecretTypeInfo; 3] = [
    AuthSecretTypeInfo {
        display_name: "Anthropic API Key",
        secret_type: ManagedSecretType::AnthropicApiKey,
        fields: &[AuthSecretTypeField {
            label: "ANTHROPIC_API_KEY",
            optional: false,
        }],
    },
    AuthSecretTypeInfo {
        display_name: "Bedrock API Key",
        secret_type: ManagedSecretType::AnthropicBedrockApiKey,
        fields: &[
            AuthSecretTypeField {
                label: "AWS_BEARER_TOKEN_BEDROCK",
                optional: false,
            },
            AuthSecretTypeField {
                label: "AWS_REGION",
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
                optional: false,
            },
            AuthSecretTypeField {
                label: "AWS_SECRET_ACCESS_KEY",
                optional: false,
            },
            AuthSecretTypeField {
                label: "AWS_SESSION_TOKEN",
                optional: true,
            },
            AuthSecretTypeField {
                label: "AWS_REGION",
                optional: false,
            },
        ],
    },
];
