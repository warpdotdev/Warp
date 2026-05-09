use serde::{Deserialize, Serialize};

/// Version of the local model config schema.
/// Bump this when making breaking changes so callers can migrate gracefully.
pub const LOCAL_MODEL_CONFIG_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalModelProvider {
    #[default]
    None,
    Ollama,
    LMStudio,
    /// Any OpenAI-compatible endpoint (e.g. vLLM, llama.cpp server, own server)
    CustomOpenAICompatible,
}

impl LocalModelProvider {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Ollama => "ollama",
            Self::LMStudio => "lmstudio",
            Self::CustomOpenAICompatible => "custom_openai_compatible",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "ollama" => Self::Ollama,
            "lmstudio" | "lm_studio" | "lm studio" => Self::LMStudio,
            "custom_openai_compatible" | "custom" => Self::CustomOpenAICompatible,
            _ => Self::None,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::None => "Disabled",
            Self::Ollama => "Ollama",
            Self::LMStudio => "LM Studio",
            Self::CustomOpenAICompatible => "Custom Server",
        }
    }
}

// ---------------------------------------------------------------------------
// Model selection mode
// ---------------------------------------------------------------------------

/// Controls how Warp routes between local and cloud models.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionMode {
    /// Always use Warp cloud models (default behaviour, no change from upstream).
    #[default]
    CloudOnly,
    /// Try a local model first; fall back to cloud if the context is too long
    /// or the local provider returns an error.
    LocalFirst,
    /// Only ever use local / self-hosted models. External API calls are
    /// hard-blocked – required for GDPR / data-sovereignty compliance.
    LocalOnly,
}

impl ModelSelectionMode {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::CloudOnly => "Cloud only",
            Self::LocalFirst => "Local first (cost efficient)",
            Self::LocalOnly => "Local only (GDPR)",
        }
    }
}

// ---------------------------------------------------------------------------
// Model tags
// ---------------------------------------------------------------------------

/// Optional hints used by the routing logic to pick the best available model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTag {
    /// Optimised for low latency / short prompts.
    Fast,
    /// Supports a large context window; preferred when the prompt is long.
    HighContext,
    /// Specialised for code generation / editing tasks.
    Coding,
    /// This model must NEVER be replaced by a cloud fallback (hard GDPR guard).
    LocalOnly,
}

// ---------------------------------------------------------------------------
// Per-model parameters
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelParams {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}

// ---------------------------------------------------------------------------
// Configured model entry (one entry per model the user has added)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfiguredModel {
    /// Unique identifier used internally (e.g. "llama3:8b", "deepseek-coder").
    pub id: String,
    /// Human-readable label shown in the model picker.
    pub display_name: String,
    /// Which provider serves this model.
    pub provider: LocalModelProvider,
    /// Base URL of the provider for this specific model.
    /// Must point to localhost or a private network address.
    /// Validated by [`is_local_url`] before use.
    pub base_url: String,
    /// Per-model generation parameters (overrides global defaults when set).
    pub params: ModelParams,
    /// Maximum context window in tokens. Used by LocalFirst routing to decide
    /// whether to fall back to a cloud model for long prompts.
    pub max_context_tokens: Option<u32>,
    /// Optional routing hints.
    #[serde(default)]
    pub tags: Vec<ModelTag>,
}

impl ConfiguredModel {
    /// Returns `true` if this model must never be replaced by a cloud fallback.
    pub fn is_local_only(&self) -> bool {
        self.tags.contains(&ModelTag::LocalOnly)
    }
}

// ---------------------------------------------------------------------------
// Provider-level connection configs (kept for connection UI / defaults)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub timeout_seconds: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            timeout_seconds: 30,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LMStudioConfig {
    pub base_url: String,
    pub timeout_seconds: u64,
}

impl Default for LMStudioConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1234/v1".to_string(),
            timeout_seconds: 30,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalModelConfig {
    /// Schema version – bump `LOCAL_MODEL_CONFIG_VERSION` on breaking changes.
    #[serde(default)]
    pub version: u32,
    /// How Warp routes between local and cloud models.
    #[serde(default)]
    pub selection_mode: ModelSelectionMode,
    /// Default Ollama connection settings (used when adding Ollama models).
    pub ollama: OllamaConfig,
    /// Default LM Studio connection settings.
    pub lmstudio: LMStudioConfig,
    /// All models the user has explicitly configured.
    #[serde(default)]
    pub configured_models: Vec<ConfiguredModel>,
    /// ID of the model currently selected in the picker (`None` = no local model active).
    pub active_model_id: Option<String>,
    /// Global fallback parameters used when a `ConfiguredModel` has no per-model params.
    #[serde(default)]
    pub model_params: ModelParams,
}

impl Default for LocalModelConfig {
    fn default() -> Self {
        Self {
            version: LOCAL_MODEL_CONFIG_VERSION,
            selection_mode: ModelSelectionMode::default(),
            ollama: OllamaConfig::default(),
            lmstudio: LMStudioConfig::default(),
            configured_models: Vec::new(),
            active_model_id: None,
            model_params: ModelParams::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// URL safety validation (GDPR helper)
// ---------------------------------------------------------------------------

/// Returns `true` when `url` points to localhost or a RFC-1918 private address.
///
/// This is used to prevent accidental external API calls when
/// [`ModelSelectionMode::LocalOnly`] is active.
pub fn is_local_url(url: &str) -> bool {
    let url = url.trim();
    // localhost variants
    if url.starts_with("http://localhost") || url.starts_with("https://localhost") {
        return true;
    }
    // IPv4 loopback
    if url.starts_with("http://127.") || url.starts_with("https://127.") {
        return true;
    }
    // IPv6 loopback
    if url.starts_with("http://[::1]") || url.starts_with("https://[::1]") {
        return true;
    }
    // RFC-1918 private ranges
    for prefix in &[
        "http://10.",
        "https://10.",
        "http://192.168.",
        "https://192.168.",
        "http://172.16.",
        "https://172.16.",
        "http://172.17.",
        "https://172.17.",
        "http://172.18.",
        "https://172.18.",
        "http://172.19.",
        "https://172.19.",
        "http://172.20.",
        "https://172.20.",
        "http://172.21.",
        "https://172.21.",
        "http://172.22.",
        "https://172.22.",
        "http://172.23.",
        "https://172.23.",
        "http://172.24.",
        "https://172.24.",
        "http://172.25.",
        "https://172.25.",
        "http://172.26.",
        "https://172.26.",
        "http://172.27.",
        "https://172.27.",
        "http://172.28.",
        "https://172.28.",
        "http://172.29.",
        "https://172.29.",
        "http://172.30.",
        "https://172.30.",
        "http://172.31.",
        "https://172.31.",
    ] {
        if url.starts_with(prefix) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_urls_are_accepted() {
        assert!(is_local_url("http://localhost:11434"));
        assert!(is_local_url("http://127.0.0.1:11434"));
        assert!(is_local_url("http://[::1]:11434"));
        assert!(is_local_url("http://192.168.1.100:1234"));
        assert!(is_local_url("http://10.0.0.5:8080"));
        assert!(is_local_url("http://172.16.0.1:11434"));
    }

    #[test]
    fn external_urls_are_rejected() {
        assert!(!is_local_url("https://api.openai.com"));
        assert!(!is_local_url("https://api.anthropic.com"));
        assert!(!is_local_url("http://8.8.8.8:11434"));
        assert!(!is_local_url(""));
    }
}
