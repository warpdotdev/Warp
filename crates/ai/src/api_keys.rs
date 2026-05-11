pub use crate::aws_credentials::{AwsCredentials, AwsCredentialsState};
use serde::{Deserialize, Serialize};
use warp_multi_agent_api as api;
use warpui::{Entity, ModelContext, SingletonEntity};
use warpui_extras::secure_storage::{self, AppContextExt};

const SECURE_STORAGE_KEY: &str = "AiApiKeys";

/// Emitted when user-provided API keys are updated in-memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyManagerEvent {
    KeysUpdated,
}

/// User-provided API keys for AI providers.
///
/// These are used for "Bring Your Own API Key" functionality, allowing
/// users to use their own API keys instead of Warp's.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiKeys {
    pub google: Option<String>,
    pub anthropic: Option<String>,
    pub openai: Option<String>,
    pub open_router: Option<String>,
    pub custom_inference: Option<CustomInference>,
    pub custom_endpoints: Vec<CustomEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomInference {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomEndpoint {
    pub name: String,
    pub url: String,
    pub api_key: String,
    pub models: Vec<CustomEndpointModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomEndpointModel {
    pub name: String,
    pub alias: Option<String>,
}

impl CustomInference {
    fn is_effectively_empty(&self) -> bool {
        self.endpoint.is_empty() && self.model.is_empty() && self.api_key.is_empty()
    }
}

impl ApiKeys {
    pub fn has_any_key(&self) -> bool {
        self.openai.is_some()
            || self.anthropic.is_some()
            || self.google.is_some()
            || self.open_router.is_some()
            || self
                .custom_inference
                .as_ref()
                .is_some_and(|c| !c.api_key.is_empty())
            || self
                .custom_endpoints
                .iter()
                .any(|endpoint| !endpoint.api_key.trim().is_empty())
    }

    /// Returns `true` when the user has at least one custom endpoint configured.
    pub fn has_custom_endpoints(&self) -> bool {
        !self.custom_endpoints.is_empty()
    }
}

/// Controls how AWS credentials are refreshed by [`ApiKeyManager`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AwsCredentialsRefreshStrategy {
    /// Load credentials from the local AWS credential chain (~/.aws). This is the default.
    #[default]
    LocalChain,
    /// Credentials are managed externally via OIDC/STS.
    /// The task ID is used to scope the STS AssumeRoleWithWebIdentity session.
    /// The role ARN is the IAM role to assume via STS.
    OidcManaged {
        task_id: Option<String>,
        role_arn: String,
    },
}

/// A structure that manages API keys for AI providers.
pub struct ApiKeyManager {
    keys: ApiKeys,
    pub(crate) aws_credentials_state: AwsCredentialsState,
    aws_credentials_refresh_strategy: AwsCredentialsRefreshStrategy,
}

impl ApiKeyManager {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let keys = Self::load_keys_from_secure_storage(ctx);
        Self {
            keys,
            aws_credentials_state: AwsCredentialsState::Missing,
            aws_credentials_refresh_strategy: AwsCredentialsRefreshStrategy::default(),
        }
    }

    pub fn keys(&self) -> &ApiKeys {
        &self.keys
    }

    pub fn set_google_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.google = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_anthropic_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.anthropic = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_openai_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.openai = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_open_router_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.open_router = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_custom_inference_key(
        &mut self,
        endpoint: String,
        model: String,
        api_key: String,
        ctx: &mut ModelContext<Self>,
    ) {
        self.keys.custom_inference = Some(CustomInference {
            endpoint,
            model,
            api_key,
        });
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn clear_custom_inference_key(&mut self, ctx: &mut ModelContext<Self>) {
        self.keys.custom_inference = None;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn add_custom_endpoint(
        &mut self,
        name: String,
        url: String,
        api_key: String,
        models: Vec<(String, Option<String>)>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.keys.custom_endpoints.push(CustomEndpoint {
            name,
            url,
            api_key,
            models: models
                .into_iter()
                .map(|(name, alias)| CustomEndpointModel { name, alias })
                .collect(),
        });
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn save_custom_endpoint(
        &mut self,
        index: usize,
        name: String,
        url: String,
        api_key: String,
        models: Vec<(String, Option<String>)>,
        ctx: &mut ModelContext<Self>,
    ) {
        if index >= self.keys.custom_endpoints.len() {
            return;
        }
        self.keys.custom_endpoints[index] = CustomEndpoint {
            name,
            url,
            api_key,
            models: models
                .into_iter()
                .map(|(name, alias)| CustomEndpointModel { name, alias })
                .collect(),
        };
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn remove_custom_endpoint(&mut self, index: usize, ctx: &mut ModelContext<Self>) {
        if index >= self.keys.custom_endpoints.len() {
            return;
        }
        self.keys.custom_endpoints.remove(index);
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn clear_custom_endpoints(&mut self, ctx: &mut ModelContext<Self>) {
        if self.keys.custom_endpoints.is_empty() {
            return;
        }
        self.keys.custom_endpoints.clear();
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_aws_credentials_state(
        &mut self,
        state: AwsCredentialsState,
        ctx: &mut ModelContext<Self>,
    ) {
        self.aws_credentials_state = state;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
    }

    pub fn aws_credentials_state(&self) -> &AwsCredentialsState {
        &self.aws_credentials_state
    }

    pub fn aws_credentials_refresh_strategy(&self) -> AwsCredentialsRefreshStrategy {
        self.aws_credentials_refresh_strategy.clone()
    }

    pub fn set_aws_credentials_refresh_strategy(
        &mut self,
        strategy: AwsCredentialsRefreshStrategy,
    ) {
        self.aws_credentials_refresh_strategy = strategy;
    }

    pub fn user_provided_llm_endpoint_for_request(
        &self,
        include_byo_keys: bool,
    ) -> Option<api::request::settings::UserProvidedLlmEndpoint> {
        include_byo_keys
            .then(|| {
                self.keys
                    .custom_inference
                    .clone()
                    .filter(|c| !c.is_effectively_empty())
                    .or_else(|| {
                        let endpoint = self.keys.custom_endpoints.first()?;
                        let model = endpoint.models.first()?;
                        Some(CustomInference {
                            endpoint: endpoint.url.clone(),
                            model: model.name.clone(),
                            api_key: endpoint.api_key.clone(),
                        })
                    })
            })
            .flatten()
            .map(|c| api::request::settings::UserProvidedLlmEndpoint {
                base_url: c.endpoint,
                model_id: c.model,
                api_key: c.api_key,
            })
    }

    pub fn api_keys_for_request(
        &self,
        include_byo_keys: bool,
        include_aws_bedrock_credentials: bool,
    ) -> Option<api::request::settings::ApiKeys> {
        let anthropic = include_byo_keys
            .then(|| self.keys.anthropic.clone())
            .flatten()
            .unwrap_or_default();
        let openai = include_byo_keys
            .then(|| self.keys.openai.clone())
            .flatten()
            .unwrap_or_default();
        let google = include_byo_keys
            .then(|| self.keys.google.clone())
            .flatten()
            .unwrap_or_default();
        let open_router = include_byo_keys
            .then(|| self.keys.open_router.clone())
            .flatten()
            .unwrap_or_default();

        // Also include credentials when running with OIDC-managed Bedrock inference, regardless
        // of the per-user setting flag (which only applies to the local credential chain path).
        let include_aws = include_aws_bedrock_credentials
            || matches!(
                self.aws_credentials_refresh_strategy,
                AwsCredentialsRefreshStrategy::OidcManaged { .. }
            );
        let aws_credentials = include_aws
            .then(|| match self.aws_credentials_state {
                AwsCredentialsState::Loaded {
                    ref credentials, ..
                } => Some(credentials.clone().into()),
                _ => None,
            })
            .flatten();

        if anthropic.is_empty()
            && openai.is_empty()
            && google.is_empty()
            && open_router.is_empty()
            && aws_credentials.is_none()
        {
            None
        } else {
            Some(api::request::settings::ApiKeys {
                anthropic,
                openai,
                google,
                open_router,
                allow_use_of_warp_credits: false,
                aws_credentials,
            })
        }
    }

    fn load_keys_from_secure_storage(ctx: &mut ModelContext<Self>) -> ApiKeys {
        let key_json = match ctx.secure_storage().read_value(SECURE_STORAGE_KEY) {
            Ok(json) => json,
            Err(e) => {
                if !matches!(e, secure_storage::Error::NotFound) {
                    log::error!("Failed to read API keys from secure storage: {e:#}");
                }
                return ApiKeys::default();
            }
        };

        let keys = match serde_json::from_str(&key_json) {
            Ok(keys) => keys,
            Err(e) => {
                log::error!("Failed to deserialize API keys: {e:#}");
                ApiKeys::default()
            }
        };

        keys
    }

    fn write_keys_to_secure_storage(&mut self, ctx: &mut ModelContext<Self>) {
        let json = match serde_json::to_string(&self.keys) {
            Ok(json) => json,
            Err(e) => {
                log::error!("Failed to serialize API keys: {e:#}");
                return;
            }
        };

        // Defer the keychain write so it doesn't block the current event
        // processing. The in-memory state is already updated and events
        // already emitted, so the UI updates immediately while the
        // potentially slow platform secure-storage call runs in a
        // subsequent main-thread callback.
        ctx.spawn(async move { json }, |_, json, ctx| {
            if let Err(e) = ctx.secure_storage().write_value(SECURE_STORAGE_KEY, &json) {
                log::error!("Failed to write API keys to secure storage: {e:#}");
            }
        });
    }
}

impl Entity for ApiKeyManager {
    type Event = ApiKeyManagerEvent;
}

impl SingletonEntity for ApiKeyManager {}

#[cfg(test)]
#[path = "api_keys_tests.rs"]
mod tests;
