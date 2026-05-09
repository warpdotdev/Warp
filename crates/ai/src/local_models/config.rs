use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalModelProvider {
    #[default]
    None,
    Ollama,
    LMStudio,
}

impl LocalModelProvider {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Ollama => "ollama",
            Self::LMStudio => "lmstudio",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "ollama" => Self::Ollama,
            "lmstudio" | "lm_studio" | "lm studio" => Self::LMStudio,
            _ => Self::None,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::None => "Disabled",
            Self::Ollama => "Ollama",
            Self::LMStudio => "LM Studio",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelParams {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalModelConfig {
    pub provider: LocalModelProvider,
    pub ollama: OllamaConfig,
    pub lmstudio: LMStudioConfig,
    pub selected_model: Option<String>,
    pub model_params: ModelParams,
}

impl Default for LocalModelConfig {
    fn default() -> Self {
        Self {
            provider: LocalModelProvider::None,
            ollama: OllamaConfig::default(),
            lmstudio: LMStudioConfig::default(),
            selected_model: None,
            model_params: ModelParams::default(),
        }
    }
}
