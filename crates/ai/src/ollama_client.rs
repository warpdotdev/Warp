//! Ollama API client for local model inference.
//!
//! This module provides a client for interacting with Ollama's API,
//! allowing Warp to use locally-hosted models.
//!
//! API Reference: https://github.com/ollama/ollama/blob/main/docs/api.md

use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub use crate::llm_id::LLMId;

/// Default Ollama base URL.
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Default request timeout for Ollama API calls.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// A message in an Ollama chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Request payload for Ollama chat API.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Response from Ollama chat API (non-streaming).
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub model: String,
    pub message: ChatMessage,
    #[serde(default)]
    pub done_reason: Option<String>,
    pub done: bool,
}

/// A streaming chunk from Ollama's chat API.
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "StreamChunkHelper")]
pub enum StreamChunk {
    Partial {
        model: String,
        message: ChatMessage,
        done: bool,
    },
    Complete {
        model: String,
        message: ChatMessage,
        done: bool,
        #[serde(default)]
        done_reason: Option<String>,
        #[serde(default)]
        total_duration: Option<u64>,
        #[serde(default)]
        eval_count: Option<u64>,
        #[serde(default)]
        eval_duration: Option<u64>,
        #[serde(default)]
        load_duration: Option<u64>,
        #[serde(default)]
        prompt_eval_count: Option<u64>,
        #[serde(default)]
        prompt_eval_duration: Option<u64>,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct StreamChunkHelper {
    done: bool,
    model: String,
    message: ChatMessage,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    eval_duration: Option<u64>,
    #[serde(default)]
    load_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>,
}

impl From<StreamChunkHelper> for StreamChunk {
    fn from(h: StreamChunkHelper) -> Self {
        if h.done {
            StreamChunk::Complete {
                model: h.model,
                message: h.message,
                done: h.done,
                done_reason: h.done_reason,
                total_duration: h.total_duration,
                eval_count: h.eval_count,
                eval_duration: h.eval_duration,
                load_duration: h.load_duration,
                prompt_eval_count: h.prompt_eval_count,
                prompt_eval_duration: h.prompt_eval_duration,
            }
        } else {
            StreamChunk::Partial {
                model: h.model,
                message: h.message,
                done: h.done,
            }
        }
    }
}

/// Model info returned by Ollama's /api/tags endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub digest: Option<String>,
}

/// Response from Ollama's /api/tags endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct ListModelsResponse {
    pub models: Vec<ModelInfo>,
}

/// Error types for Ollama client operations.
#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error("Connection failed: {0}")]
    ConnectionError(#[from] reqwest::Error),

    #[error("Ollama server returned error: {0}")]
    ServerError(String),

    #[error("Ollama server not running at {0}")]
    ServerNotRunning(String),

    #[error("Parse error: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Result type for Ollama operations.
pub type OllamaResult<T> = std::result::Result<T, OllamaError>;

/// Client for interacting with Ollama API.
#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: String,
    http_client: Client,
}

impl OllamaClient {
    /// Create a new Ollama client with the default base URL.
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_OLLAMA_URL)
    }

    /// Create a new Ollama client with a custom base URL.
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let http_client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http_client,
        }
    }

    /// Check if Ollama server is running and accessible.
    pub async fn health_check(&self) -> OllamaResult<bool> {
        match self
            .http_client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
        {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                if e.is_connect() {
                    Ok(false)
                } else {
                    Err(OllamaError::ConnectionError(e))
                }
            }
        }
    }

    /// List all available models on the Ollama server.
    pub async fn list_models(&self) -> OllamaResult<Vec<ModelInfo>> {
        let response = self
            .http_client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(OllamaError::ServerError(format!(
                "Server returned status: {}",
                status
            )));
        }

        let body = response.text().await?;
        let result: ListModelsResponse = serde_json::from_str(&body)?;
        Ok(result.models)
    }

    /// Get a list of model names available on the server.
    pub async fn available_model_names(&self) -> OllamaResult<Vec<String>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().map(|m| m.name).collect())
    }

    /// Send a chat request and get a non-streaming response.
    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> OllamaResult<ChatResponse> {
        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: Some(false),
        };

        let response = self
            .http_client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OllamaError::ServerError(format!(
                "Server returned status {}: {}",
                status, body
            )));
        }

        let body = response.text().await?;
        let result: ChatResponse = serde_json::from_str(&body)?;
        Ok(result)
    }

    /// Send a chat request and stream responses.
    pub async fn chat_streaming(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> OllamaResult<impl futures::Stream<Item = OllamaResult<StreamChunk>>> {
        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: Some(true),
        };

        let response = self
            .http_client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OllamaError::ServerError(format!(
                "Server returned status {}: {}",
                status, body
            )));
        }

        let stream = response.bytes_stream().map(|chunk_result| {
            chunk_result
                .map_err(OllamaError::ConnectionError)
                .and_then(|bytes| {
                    let text = String::from_utf8_lossy(&bytes);
                    // Ollama sends newline-delimited JSON - each line is a separate JSON object
                    let mut accumulated = String::new();
                    for ch in text.chars() {
                        if ch == '\n' {
                            let line = accumulated.trim();
                            if !line.is_empty() {
                                match serde_json::from_str::<StreamChunk>(line) {
                                    Ok(chunk) => return Ok(chunk),
                                    Err(e) => {
                                        log::debug!("Failed to parse Ollama stream chunk: {}", e);
                                    }
                                }
                            }
                            accumulated.clear();
                        } else {
                            accumulated.push(ch);
                        }
                    }
                    // Handle any remaining data without trailing newline
                    let line = accumulated.trim();
                    if !line.is_empty() {
                        match serde_json::from_str::<StreamChunk>(line) {
                            Ok(chunk) => return Ok(chunk),
                            Err(e) => {
                                log::debug!("Failed to parse Ollama stream chunk: {}", e);
                            }
                        }
                    }
                    // If no valid chunk found, skip
                    Ok(StreamChunk::Partial {
                        model: String::new(),
                        message: ChatMessage {
                            role: String::new(),
                            content: String::new(),
                        },
                        done: false,
                    })
                })
        });

        Ok(stream)
    }

    /// Create a chat message from a role and content.
    pub fn message(role: impl Into<String>, content: impl Into<String>) -> ChatMessage {
        ChatMessage {
            role: role.into(),
            content: content.into(),
        }
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for easily creating messages.
pub trait MessageExt {
    fn user(content: impl Into<String>) -> ChatMessage;
    fn assistant(content: impl Into<String>) -> ChatMessage;
    fn system(content: impl Into<String>) -> ChatMessage;
}

impl MessageExt for ChatMessage {
    fn user(content: impl Into<String>) -> ChatMessage {
        Self::message("user", content)
    }

    fn assistant(content: impl Into<String>) -> ChatMessage {
        Self::message("assistant", content)
    }

    fn system(content: impl Into<String>) -> ChatMessage {
        Self::message("system", content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_creation() {
        let msg = ChatMessage::user("Hello, world!");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello, world!");
    }

    #[test]
    fn test_chat_message_from_trait() {
        let msg = ChatMessage::system("You are a helpful assistant.");
        assert_eq!(msg.role, "system");
    }

    #[test]
    fn test_serialize_chat_request() {
        let request = ChatRequest {
            model: "llama3".to_string(),
            messages: vec![ChatMessage::user("Hi")],
            stream: Some(true),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"llama3\""));
        assert!(json.contains("\"stream\":true"));
    }

    #[tokio::test]
    async fn test_client_creation() {
        let client = OllamaClient::new();
        assert_eq!(client.base_url, DEFAULT_OLLAMA_URL);
    }

    #[tokio::test]
    async fn test_custom_base_url() {
        let client = OllamaClient::with_base_url("http://192.168.1.100:11434");
        assert_eq!(client.base_url, "http://192.168.1.100:11434");
    }

    #[test]
    fn test_parse_stream_chunk() {
        let json = r#"{"model":"llama3","message":{"role":"assistant","content":"Hello"},"done":false}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert!(matches!(chunk, StreamChunk::Partial { .. }));
    }

    #[test]
    fn test_parse_complete_chunk() {
        let json = r#"{"model":"llama3","message":{"role":"assistant","content":"Hello"},"done":true,"done_reason":"stop","eval_count":5}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert!(matches!(chunk, StreamChunk::Complete { .. }));
    }

    #[test]
    fn test_parse_model_info() {
        let json = r#"{"name":"llama3:latest","model":"llama3","modified_at":"2024-01-01T00:00:00Z","size":3826793472,"digest":"sha256:..."}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.name, "llama3:latest");
    }
}