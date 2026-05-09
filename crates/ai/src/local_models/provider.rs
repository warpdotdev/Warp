//! ModelRouter — routes completion requests to the correct local model client.
//!
//! Replaces the old `ProviderFactory` with a stateful router that:
//! - supports multiple `ConfiguredModel` entries
//! - enforces the `LocalOnly` GDPR hard-block
//! - implements `LocalFirst` fallback logic
//! - caches connection status for the UI picker (30 s TTL)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::api_client::{ConnectionStatus, LocalModelClient, ModelPickerEntry};
use super::config::{
    is_local_url, ConfiguredModel, LocalModelConfig, LocalModelProvider, ModelParams,
    ModelSelectionMode,
};
use super::lmstudio::LMStudioClient;
use super::ollama::OllamaClient;
use super::{LocalModelError, LocalModelResult};

// ---------------------------------------------------------------------------
// Connection-status cache entry
// ---------------------------------------------------------------------------

struct CacheEntry {
    status: ConnectionStatus,
    checked_at: Instant,
}

const CACHE_TTL: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// ModelRouter
// ---------------------------------------------------------------------------

pub struct ModelRouter {
    config: LocalModelConfig,
    /// Connection status cache keyed by model id.
    /// Wrapped in Arc<Mutex> so it can be updated from async background tasks.
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
}

impl ModelRouter {
    pub fn new(config: LocalModelConfig) -> Self {
        Self {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Update the router config (e.g. after user saves settings).
    pub fn update_config(&mut self, config: LocalModelConfig) {
        self.config = config;
    }

    // -----------------------------------------------------------------------
    // Public API used by the completion pipeline
    // -----------------------------------------------------------------------

    /// Generate a completion using the currently active model.
    ///
    /// Routing logic:
    /// - `LocalOnly`  → use active local model, hard-block any external call.
    /// - `LocalFirst` → try local; fall back to cloud if context too long or
    ///                  local returns an error (unless model has `LocalOnly` tag).
    /// - `CloudOnly`  → caller should use cloud directly; returns error here.
    pub async fn generate_local(
        &self,
        prompt: &str,
        context_token_estimate: Option<usize>,
    ) -> LocalModelResult<String> {
        let model = self.active_model()?;

        // GDPR hard-block: verify the URL is local before any network call
        if !is_local_url(&model.base_url) {
            match self.config.selection_mode {
                ModelSelectionMode::LocalOnly => {
                    return Err(LocalModelError::ExternalCallBlocked(format!(
                        "Model '{}' base_url '{}' is not a local/private address. \
                         Blocked by LocalOnly mode.",
                        model.id, model.base_url
                    )));
                }
                // LocalFirst or CloudOnly with a non-local URL: still warn but allow
                // (the user explicitly configured a remote URL for this model).
                _ => {}
            }
        }

        match self.config.selection_mode {
            ModelSelectionMode::CloudOnly => Err(LocalModelError::ProviderNotConfigured),

            ModelSelectionMode::LocalOnly => {
                let client = self.build_client(model)?;
                client.generate_completion(prompt, &model.id).await
            }

            ModelSelectionMode::LocalFirst => {
                // Check context length against model's declared maximum
                let context_too_long = match (context_token_estimate, model.max_context_tokens) {
                    (Some(estimated), Some(max)) => estimated > max as usize,
                    _ => false,
                };

                if context_too_long && !model.is_local_only() {
                    // Signal to caller that cloud fallback should be used
                    return Err(LocalModelError::Unknown(
                        "__FALLBACK_TO_CLOUD__".to_string(),
                    ));
                }

                let client = self.build_client(model)?;
                let result = client.generate_completion(prompt, &model.id).await;

                match result {
                    Ok(response) => Ok(response),
                    Err(e) if model.is_local_only() => Err(e),
                    Err(_) => {
                        // Signal to caller that cloud fallback should be used
                        Err(LocalModelError::Unknown(
                            "__FALLBACK_TO_CLOUD__".to_string(),
                        ))
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Connection status (for UI picker)
    // -----------------------------------------------------------------------

    /// Check connection for all configured models and update the cache.
    /// Call this from a background task every 30 s.
    pub async fn refresh_connection_status(&self) {
        for model in &self.config.configured_models {
            let status = match self.build_client(model) {
                Err(_) => ConnectionStatus::Offline,
                Ok(client) => match client.check_connection().await {
                    Ok(()) => ConnectionStatus::Online,
                    Err(_) => ConnectionStatus::Offline,
                },
            };
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(
                    model.id.clone(),
                    CacheEntry {
                        status,
                        checked_at: Instant::now(),
                    },
                );
            }
        }
    }

    /// Returns all configured models with their cached connection status.
    /// Used by the UI model picker.
    pub fn get_available_models(&self) -> Vec<ModelPickerEntry> {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        self.config
            .configured_models
            .iter()
            .map(|m| {
                let status = cache.get(&m.id).map_or(
                    ConnectionStatus::Offline,
                    |entry| {
                        if entry.checked_at.elapsed() < CACHE_TTL {
                            entry.status.clone()
                        } else {
                            ConnectionStatus::Offline
                        }
                    },
                );
                ModelPickerEntry {
                    id: m.id.clone(),
                    display_name: m.display_name.clone(),
                    provider: m.provider,
                    status,
                    is_local: is_local_url(&m.base_url),
                }
            })
            .collect()
    }

    /// Returns the current `ModelSelectionMode`.
    pub fn selection_mode(&self) -> ModelSelectionMode {
        self.config.selection_mode
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn active_model(&self) -> LocalModelResult<&ConfiguredModel> {
        let id = self
            .config
            .active_model_id
            .as_deref()
            .ok_or(LocalModelError::ProviderNotConfigured)?;

        self.config
            .configured_models
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| LocalModelError::ModelNotFound(id.to_string()))
    }

    fn build_client(&self, model: &ConfiguredModel) -> LocalModelResult<Box<dyn LocalModelClient>> {
        // Merge global fallback params with per-model params
        let params = merge_params(&model.params, &self.config.model_params);
        let timeout = None; // use per-provider default (30 s)

        match model.provider {
            LocalModelProvider::None => Err(LocalModelError::ProviderNotConfigured),

            LocalModelProvider::Ollama => Ok(Box::new(OllamaClient::new(
                &model.base_url,
                Some(params),
                timeout,
            )?)),

            // LM Studio and any OpenAI-compatible endpoint share the same client
            LocalModelProvider::LMStudio | LocalModelProvider::CustomOpenAICompatible => {
                Ok(Box::new(LMStudioClient::new(
                    &model.base_url,
                    Some(params),
                    timeout,
                )?))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Param merging: per-model params take priority over global defaults
// ---------------------------------------------------------------------------

fn merge_params(model: &ModelParams, global: &ModelParams) -> ModelParams {
    ModelParams {
        temperature: model.temperature.or(global.temperature),
        top_p: model.top_p.or(global.top_p),
        max_tokens: model.max_tokens.or(global.max_tokens),
    }
}

// ---------------------------------------------------------------------------
// Backwards-compat: keep ProviderFactory as a thin wrapper for old call sites
// ---------------------------------------------------------------------------

/// Deprecated: use `ModelRouter` instead.
/// Kept to avoid breaking existing call sites outside this module.
#[deprecated(since = "2.0.0", note = "Use ModelRouter instead")]
pub struct ProviderFactory;

#[allow(deprecated)]
impl ProviderFactory {
    pub fn create_client(config: &LocalModelConfig) -> LocalModelResult<Box<dyn LocalModelClient>> {
        // Build a temporary single-model config from the legacy provider field
        // by delegating to the first configured model if available.
        if let Some(id) = &config.active_model_id {
            if let Some(model) = config.configured_models.iter().find(|m| &m.id == id) {
                let router = ModelRouter::new(config.clone());
                return router.build_client(model);
            }
        }
        Err(LocalModelError::ProviderNotConfigured)
    }
}
