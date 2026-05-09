//! Local LLM Model Support
//!
//! This module provides integration with local language models through:
//! - Ollama (https://ollama.ai)
//! - LMStudio (https://lmstudio.ai)
//!
//! The module exposes a unified interface for model discovery, health checks,
//! and completions generation.

pub mod api_client;
pub mod config;
pub mod lmstudio;
pub mod ollama;
pub mod provider;

pub use api_client::{LocalModelClient, ModelInfo};
pub use config::{LMStudioConfig, LocalModelConfig, LocalModelProvider, ModelParams, OllamaConfig};
pub use provider::ProviderFactory;

use thiserror::Error;

/// Errors that can occur when working with local models
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

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type LocalModelResult<T> = Result<T, LocalModelError>;
