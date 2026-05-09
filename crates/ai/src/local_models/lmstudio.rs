use super::api_client::{LocalModelClient, ModelInfo};
use super::config::{LocalModelProvider, ModelParams};
use super::{LocalModelError, LocalModelResult};

#[cfg(not(target_family = "wasm"))]
mod native {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    #[derive(Clone)]
    pub struct LMStudioClient {
        base_url: String,
        model_params: ModelParams,
        http_client: reqwest::Client,
    }

    impl LMStudioClient {
        pub fn new(
            base_url: &str,
            model_params: Option<ModelParams>,
            timeout_seconds: Option<u64>,
        ) -> LocalModelResult<Self> {
            let base_url = normalize_base_url(base_url);
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

        fn models_endpoint(&self) -> String {
            format!("{}/models", self.base_url)
        }

        fn chat_completions_endpoint(&self) -> String {
            format!("{}/chat/completions", self.base_url)
        }
    }

    fn normalize_base_url(base_url: &str) -> String {
        let trimmed = base_url.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            return String::new();
        }
        if trimmed.ends_with("/v1") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/v1")
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
    struct OpenAIModelsResponse {
        #[serde(default)]
        data: Vec<OpenAIModel>,
    }

    #[derive(Debug, Deserialize)]
    struct OpenAIModel {
        id: String,
    }

    #[derive(Debug, Serialize)]
    struct ChatCompletionsRequest<'a> {
        model: &'a str,
        messages: Vec<ChatMessage<'a>>,
        stream: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        temperature: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        top_p: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_tokens: Option<u32>,
    }

    #[derive(Debug, Serialize)]
    struct ChatMessage<'a> {
        role: &'a str,
        content: &'a str,
    }

    #[derive(Debug, Deserialize)]
    struct ChatCompletionsResponse {
        #[serde(default)]
        choices: Vec<ChatChoice>,
    }

    #[derive(Debug, Deserialize)]
    struct ChatChoice {
        message: ChatResponseMessage,
    }

    #[derive(Debug, Deserialize)]
    struct ChatResponseMessage {
        #[serde(default)]
        content: String,
    }

    #[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
    #[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
    impl LocalModelClient for LMStudioClient {
        fn provider(&self) -> LocalModelProvider {
            LocalModelProvider::LMStudio
        }

        async fn check_connection(&self) -> LocalModelResult<()> {
            self.http_client
                .get(self.models_endpoint())
                .send()
                .await
                .map_err(|e| map_reqwest_error("Failed to connect to LM Studio", e))?
                .error_for_status()
                .map_err(|e| map_reqwest_error("LM Studio health check failed", e))?;
            Ok(())
        }

        async fn list_models(&self) -> LocalModelResult<Vec<ModelInfo>> {
            let response = self
                .http_client
                .get(self.models_endpoint())
                .send()
                .await
                .map_err(|e| map_reqwest_error("Failed to list LM Studio models", e))?
                .error_for_status()
                .map_err(|e| map_reqwest_error("Failed to list LM Studio models", e))?;

            let payload = response
                .json::<OpenAIModelsResponse>()
                .await
                .map_err(|e| LocalModelError::SerializationError(e.to_string()))?;

            let models: Vec<ModelInfo> = payload
                .data
                .into_iter()
                .map(|m| ModelInfo::new(m.id.clone(), m.id))
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

            let response = self
                .http_client
                .post(self.chat_completions_endpoint())
                .json(&ChatCompletionsRequest {
                    model,
                    messages: vec![ChatMessage {
                        role: "user",
                        content: prompt,
                    }],
                    stream: false,
                    temperature: self.model_params.temperature,
                    top_p: self.model_params.top_p,
                    max_tokens: self.model_params.max_tokens,
                })
                .send()
                .await
                .map_err(|e| map_reqwest_error("Failed to call LM Studio completions endpoint", e))?
                .error_for_status()
                .map_err(|e| map_reqwest_error("LM Studio completion request failed", e))?;

            let payload = response
                .json::<ChatCompletionsResponse>()
                .await
                .map_err(|e| LocalModelError::SerializationError(e.to_string()))?;

            let content = payload
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .unwrap_or_default();

            if content.trim().is_empty() {
                return Err(LocalModelError::InvalidResponse(
                    "LM Studio returned an empty completion".to_string(),
                ));
            }

            Ok(content)
        }
    }
}

#[cfg(target_family = "wasm")]
mod native {
    use super::*;

    #[derive(Clone)]
    pub struct LMStudioClient;

    impl LMStudioClient {
        pub fn new(
            _base_url: &str,
            _model_params: Option<ModelParams>,
            _timeout_seconds: Option<u64>,
        ) -> LocalModelResult<Self> {
            Err(LocalModelError::UnsupportedPlatform(
                "LM Studio local models are not supported on wasm targets".to_string(),
            ))
        }
    }

    #[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
    #[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
    impl LocalModelClient for LMStudioClient {
        fn provider(&self) -> LocalModelProvider {
            LocalModelProvider::LMStudio
        }

        async fn check_connection(&self) -> LocalModelResult<()> {
            Err(LocalModelError::UnsupportedPlatform(
                "LM Studio local models are not supported on wasm targets".to_string(),
            ))
        }

        async fn list_models(&self) -> LocalModelResult<Vec<ModelInfo>> {
            Err(LocalModelError::UnsupportedPlatform(
                "LM Studio local models are not supported on wasm targets".to_string(),
            ))
        }

        async fn generate_completion(
            &self,
            _prompt: &str,
            _model: &str,
        ) -> LocalModelResult<String> {
            Err(LocalModelError::UnsupportedPlatform(
                "LM Studio local models are not supported on wasm targets".to_string(),
            ))
        }
    }
}

pub use native::LMStudioClient;
