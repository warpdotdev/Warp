use anyhow::{anyhow, Result};
use warp_cli::agent::Harness;
use warp_graphql::managed_secrets::ManagedSecretType;
use warp_managed_secrets::ManagedSecretValue;

pub struct AuthSecretTypeField {
    pub label: &'static str,
    pub optional: bool,
}

pub struct AuthSecretTypeInfo {
    pub display_name: &'static str,
    pub secret_type: ManagedSecretType,
    pub fields: &'static [AuthSecretTypeField],
}

pub fn auth_secret_types_for_harness(harness: Harness) -> &'static [AuthSecretTypeInfo] {
    match harness {
        Harness::Claude => &CLAUDE_AUTH_SECRET_TYPES,
        _ => &[],
    }
}

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
