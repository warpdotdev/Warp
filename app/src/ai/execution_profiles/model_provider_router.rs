//! Provider Router for unified model selection
//!
//! Routes requests to either cloud-based LLMs or local models
//! based on user configuration and availability.

use warp_ai::local_models::{LocalModelClient, LocalModelProvider, ProviderFactory};
use std::sync::Arc;

/// Unified model provider source
#[derive(Clone, Debug, PartialEq)]
pub enum ModelProviderSource {
    /// Cloud-based providers (OpenAI, Anthropic, Google, etc.)
    Cloud,
    /// Local providers (Ollama, LMStudio)
    Local(LocalModelProvider),
}

impl ModelProviderSource {
    pub fn is_local(&self) -> bool {
        !matches!(self, Self::Cloud)
    }

    pub fn is_cloud(&self) -> bool {
        matches!(self, Self::Cloud)
    }
}

/// Model provider factory with auto-detection
pub struct ModelProviderRouter {
    local_client: Option<Arc<dyn LocalModelClient>>,
    provider_source: ModelProviderSource,
}

impl ModelProviderRouter {
    /// Create a new router with cloud as default
    pub fn new() -> Self {
        Self {
            local_client: None,
            provider_source: ModelProviderSource::Cloud,
        }
    }

    /// Initialize with a specific provider
    pub async fn with_provider(provider: LocalModelProvider) -> Result<Self, String> {
        let client = match provider {
            LocalModelProvider::None => return Err("Provider not configured".to_string()),
            LocalModelProvider::Ollama => {
                let client = ProviderFactory::create_ollama_client(
                    "http://localhost:11434",
                    None,
                )?;
                Arc::new(client)
            }
            LocalModelProvider::LMStudio => {
                let client = ProviderFactory::create_lmstudio_client(
                    "http://localhost:1234",
                    None,
                )?;
                Arc::new(client)
            }
        };

        Ok(Self {
            local_client: Some(client),
            provider_source: ModelProviderSource::Local(provider),
        })
    }

    /// Get the current provider source
    pub fn provider_source(&self) -> &ModelProviderSource {
        &self.provider_source
    }

    /// Check if a local provider is available
    pub async fn is_local_provider_available(&self) -> bool {
        if let Some(client) = &self.local_client {
            client.check_connection().await.is_ok()
        } else {
            false
        }
    }

    /// Get available models from the configured provider
    pub async fn get_available_models(&self) -> Result<Vec<String>, String> {
        match &self.local_client {
            Some(client) => {
                let models = client.list_models().await
                    .map_err(|e| format!("Failed to list models: {}", e))?;
                Ok(models.iter().map(|m| m.name.clone()).collect())
            }
            None => Err("No local provider configured".to_string()),
        }
    }

    /// Generate a completion using the configured provider
    pub async fn generate_completion(
        &self,
        prompt: &str,
        model: &str,
    ) -> Result<String, String> {
        match &self.local_client {
            Some(client) => {
                client
                    .generate_completion(prompt, model)
                    .await
                    .map_err(|e| format!("Completion failed: {}", e))
            }
            None => Err("No local provider configured".to_string()),
        }
    }

    /// Switch provider
    pub async fn switch_provider(&mut self, provider: LocalModelProvider) -> Result<(), String> {
        *self = Self::with_provider(provider).await?;
        Ok(())
    }
}

impl Default for ModelProviderRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_source_is_local() {
        let local = ModelProviderSource::Local(LocalModelProvider::Ollama);
        let cloud = ModelProviderSource::Cloud;

        assert!(local.is_local());
        assert!(!cloud.is_local());
    }

    #[test]
    fn test_provider_source_is_cloud() {
        let cloud = ModelProviderSource::Cloud;
        let local = ModelProviderSource::Local(LocalModelProvider::LMStudio);

        assert!(cloud.is_cloud());
        assert!(!local.is_cloud());
    }

    #[test]
    fn test_model_provider_router_default() {
        let router = ModelProviderRouter::default();
        assert_eq!(router.provider_source(), &ModelProviderSource::Cloud);
        assert!(router.local_client.is_none());
    }

    #[test]
    fn test_provider_source_equality() {
        let provider1 = ModelProviderSource::Cloud;
        let provider2 = ModelProviderSource::Cloud;
        assert_eq!(provider1, provider2);
    }
}
