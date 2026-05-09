use super::api_client::LocalModelClient;
use super::config::{LocalModelConfig, LocalModelProvider, ModelParams};
use super::lmstudio::LMStudioClient;
use super::ollama::OllamaClient;
use super::{LocalModelError, LocalModelResult};

pub struct ProviderFactory;

impl ProviderFactory {
    pub fn create_client(config: &LocalModelConfig) -> LocalModelResult<Box<dyn LocalModelClient>> {
        match config.provider {
            LocalModelProvider::None => Err(LocalModelError::ProviderNotConfigured),
            LocalModelProvider::Ollama => Ok(Box::new(Self::create_ollama_client(
                &config.ollama.base_url,
                Some(config.model_params.clone()),
            )?)),
            LocalModelProvider::LMStudio => Ok(Box::new(Self::create_lmstudio_client(
                &config.lmstudio.base_url,
                Some(config.model_params.clone()),
            )?)),
        }
    }

    pub fn create_ollama_client(
        base_url: &str,
        model_params: Option<ModelParams>,
    ) -> LocalModelResult<OllamaClient> {
        OllamaClient::new(base_url, model_params, None)
    }

    pub fn create_lmstudio_client(
        base_url: &str,
        model_params: Option<ModelParams>,
    ) -> LocalModelResult<LMStudioClient> {
        LMStudioClient::new(base_url, model_params, None)
    }
}
