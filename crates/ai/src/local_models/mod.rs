//! Local LLM Model Support
//!
//! This module provides integration with local language models through:
//! - Ollama (https://ollama.ai)
//! - LMStudio (https://lmstudio.ai)
//! - Any OpenAI-compatible endpoint (vLLM, llama.cpp, own server, …)
//!
//! The module exposes a unified interface for model discovery, health checks,
//! and completions generation.
//!
//! # GDPR / Data-sovereignty
//!
//! When [`config::ModelSelectionMode::LocalOnly`] is active **no request must
//! ever reach an external API**. Callers are responsible for checking
//! [`config::is_local_url`] before constructing a client, and for returning
//! [`LocalModelError::ExternalCallBlocked`] if the guard would be violated.

pub mod api_client;
pub mod config;
pub mod lmstudio;
pub mod ollama;
pub mod provider;

pub use api_client::{LocalModelClient, ModelInfo};
pub use config::{
    is_local_url, ConfiguredModel, LMStudioConfig, LocalModelConfig, LocalModelProvider,
    ModelParams, ModelSelectionMode, ModelTag, OllamaConfig, LOCAL_MODEL_CONFIG_VERSION,
};
pub use provider::ProviderFactory;

use thiserror::Error;

/// Errors that can occur when working with local models.
#[derive(Error, Debug)]
pub enum LocalModelError {
    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Request failed: {0}")]
    RequestFailed(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("No models available")]
    NoModelsAvailable,

    #[error("Provider not configured")]
    ProviderNotConfigured,

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Timeout")]
    Timeout,

    /// Returned when a cloud/external API call is attempted while
    /// [`ModelSelectionMode::LocalOnly`] is active. This is the hard
    /// GDPR guard – must never be silently swallowed.
    #[error("External call blocked (LocalOnly mode): {0}")]
    ExternalCallBlocked(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type LocalModelResult<T> = Result<T, LocalModelError>;
