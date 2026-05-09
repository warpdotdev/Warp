use super::api_client::{LocalModelClient, ModelInfo};
use super::config::{LocalModelProvider, ModelParams};
use super::{LocalModelError, LocalModelResult};

#[cfg(not(target_family = "wasm"))]
mod native {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    #[derive(Clone)]
    pub struct OllamaClient {
        base_url: String,
        model_params: ModelParams,
        http_client: reqwest::Client,
    }

    impl OllamaClient {
        pub fn new(
            base_url: &str,
            model_params: Option<ModelParams>,
            timeout_seconds: Option<u64>,
        ) -> LocalModelResult<Self> {
            let base_url = base_url.trim().trim_end_matches('/').to_string();
            if base_url.is_empty() {
                return Err(LocalModelError::ProviderNotConfigured);
            }

            let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30));
            let http_client = reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| LocalModelError::HttpError(e.to_string()))?;

            Ok(Self {
                base_url,
                model_params: model_params.unwrap_or_default(),
                http_client,
            })
        }

        fn tags_endpoint(&self) -> String {
            format!("{}/api/tags", self.base_url)
        }

        fn generate_endpoint(&self) -> String {
            format!("{}/api/generate", self.base_url)
        }
    }

    fn map_reqwest_error(context: &str, error: reqwest::Error) -> LocalModelError {
        if error.is_timeout() {
            LocalModelError::Timeout
        } else if error.is_connect() {
            LocalModelError::ConnectionFailed(format!("{context}: {error}"))
        } else {
            LocalModelError::RequestFailed(format!("{context}: {error}"))
        }
    }

    #[derive(Debug, Deserialize)]
    struct OllamaTagsResponse {
        #[serde(default)]
        models: Vec<OllamaModel>,
    }

    #[derive(Debug, Deserialize)]
    struct OllamaModel {
        name: String,
        #[serde(default)]
        model: Option<String>,
    }

    #[derive(Debug, Serialize)]
    struct OllamaGenerateRequest<'a> {
        model: &'a str,
        prompt: &'a str,
        stream: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        options: Option<OllamaGenerateOptions>,
    }

    #[derive(Debug, Default, Serialize)]
    struct OllamaGenerateOptions {
        #[serde(skip_serializing_if = "Option::is_none")]
        temperature: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        top_p: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        num_predict: Option<u32>,
    }

    #[derive(Debug, Deserialize)]
    struct OllamaGenerateResponse {
        #[serde(default)]
        response: String,
        #[serde(default)]
        error: Option<String>,
    }

    #[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
    #[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
    impl LocalModelClient for OllamaClient {
        fn provider(&self) -> LocalModelProvider {
            LocalModelProvider::Ollama
        }

        async fn check_connection(&self) -> LocalModelResult<()> {
            self.http_client
                .get(self.tags_endpoint())
                .send()
                .await
                .map_err(|e| map_reqwest_error("Failed to connect to Ollama", e))?
                .error_for_status()
                .map_err(|e| map_reqwest_error("Ollama health check failed", e))?;
            Ok(())
        }

        async fn list_models(&self) -> LocalModelResult<Vec<ModelInfo>> {
            let response = self
                .http_client
                .get(self.tags_endpoint())
                .send()
                .await
                .map_err(|e| map_reqwest_error("Failed to list Ollama models", e))?
                .error_for_status()
                .map_err(|e| map_reqwest_error("Failed to list Ollama models", e))?;

            let payload = response
                .json::<OllamaTagsResponse>()
                .await
                .map_err(|e| LocalModelError::SerializationError(e.to_string()))?;

            let models: Vec<ModelInfo> = payload
                .models
                .into_iter()
                .map(|m| {
                    let id = m.model.unwrap_or_else(|| m.name.clone());
                    ModelInfo::new(m.name, id)
                })
                .collect();

            if models.is_empty() {
                return Err(LocalModelError::NoModelsAvailable);
            }

            Ok(models)
        }

        async fn generate_completion(&self, prompt: &str, model: &str) -> LocalModelResult<String> {
            if model.trim().is_empty() {
                return Err(LocalModelError::ModelNotFound(
                    "No model selected".to_string(),
                ));
            }

            let options = OllamaGenerateOptions {
                temperature: self.model_params.temperature,
                top_p: self.model_params.top_p,
                num_predict: self.model_params.max_tokens,
            };
            let has_options = options.temperature.is_some()
                || options.top_p.is_some()
                || options.num_predict.is_some();

            let response = self
                .http_client
                .post(self.generate_endpoint())
                .json(&OllamaGenerateRequest {
                    model,
                    prompt,
                    stream: false,
                    options: has_options.then_some(options),
                })
                .send()
                .await
                .map_err(|e| map_reqwest_error("Failed to call Ollama completion endpoint", e))?
                .error_for_status()
                .map_err(|e| map_reqwest_error("Ollama completion request failed", e))?;

            let payload = response
                .json::<OllamaGenerateResponse>()
                .await
                .map_err(|e| LocalModelError::SerializationError(e.to_string()))?;

            if let Some(error) = payload.error {
                return Err(LocalModelError::RequestFailed(error));
            }

            if payload.response.trim().is_empty() {
                return Err(LocalModelError::InvalidResponse(
                    "Ollama returned an empty completion".to_string(),
                ));
            }

            Ok(payload.response)
        }
    }
}

#[cfg(target_family = "wasm")]
mod native {
    use super::*;

    #[derive(Clone)]
    pub struct OllamaClient;

    impl OllamaClient {
        pub fn new(
            _base_url: &str,
            _model_params: Option<ModelParams>,
            _timeout_seconds: Option<u64>,
        ) -> LocalModelResult<Self> {
            Err(LocalModelError::UnsupportedPlatform(
                "Ollama local models are not supported on wasm targets".to_string(),
            ))
        }
    }

    #[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
    #[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
    impl LocalModelClient for OllamaClient {
        fn provider(&self) -> LocalModelProvider {
            LocalModelProvider::Ollama
        }

        async fn check_connection(&self) -> LocalModelResult<()> {
            Err(LocalModelError::UnsupportedPlatform(
                "Ollama local models are not supported on wasm targets".to_string(),
            ))
        }

        async fn list_models(&self) -> LocalModelResult<Vec<ModelInfo>> {
            Err(LocalModelError::UnsupportedPlatform(
                "Ollama local models are not supported on wasm targets".to_string(),
            ))
        }

        async fn generate_completion(
            &self,
            _prompt: &str,
            _model: &str,
        ) -> LocalModelResult<String> {
            Err(LocalModelError::UnsupportedPlatform(
                "Ollama local models are not supported on wasm targets".to_string(),
            ))
        }
    }
}

pub use native::OllamaClient;
