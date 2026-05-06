use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EndpointModel {
    /// The model ID to send in API requests (e.g. "llama3", "gpt-4").
    pub model_id: String,
    /// User-facing display name for this model (e.g. "Llama 3").
    pub alias: String,
}

impl EndpointModel {
    /// Returns true if this model has a non-empty alias.
    pub fn has_alias(&self) -> bool {
        !self.alias.is_empty()
    }

    /// Returns the display name: alias if set, otherwise model_id.
    pub fn display_name(&self) -> &str {
        if self.has_alias() {
            &self.alias
        } else {
            &self.model_id
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(from = "OpenAiCompatibleEndpointHelper")]
pub struct OpenAiCompatibleEndpoint {
    /// Unique identifier for this endpoint (e.g. "endpoint-2").
    pub id: String,
    /// Human-readable label shown in the UI (e.g. "My Ollama Server").
    pub display_name: String,
    /// Base URL of the API server (e.g. "http://localhost:11434").
    pub base_url: String,
    /// Whether an API key is stored in secure storage for this endpoint.
    #[serde(default)]
    pub has_api_key: bool,
    /// Runtime-only API key, loaded from secure storage. Not serialized to TOML.
    #[serde(skip)]
    pub api_key: Option<String>,
    /// Models available on this endpoint.
    pub models: Vec<EndpointModel>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct OpenAiCompatibleEndpointHelper {
    id: String,
    display_name: String,
    base_url: String,
    #[serde(default)]
    #[deprecated = "API keys are now stored in OS keychain via secure storage. This field only exists for backward-compatible TOML migration."]
    api_key: Option<String>,
    #[serde(default)]
    has_api_key: bool,
    #[serde(default)]
    #[schemars(skip)]
    model_id: Option<String>,
    #[serde(default)]
    models: Option<Vec<EndpointModel>>,
}

impl From<OpenAiCompatibleEndpointHelper> for OpenAiCompatibleEndpoint {
    fn from(helper: OpenAiCompatibleEndpointHelper) -> Self {
        let models = helper.models.unwrap_or_else(|| {
            let model_id = helper.model_id.unwrap_or_default();
            vec![EndpointModel {
                model_id,
                alias: String::new(),
            }]
        });
        let has_api_key = helper.has_api_key || helper.api_key.as_ref().is_some_and(|k| !k.is_empty());
        Self {
            id: helper.id,
            display_name: helper.display_name,
            base_url: helper.base_url,
            has_api_key,
            api_key: helper.api_key,
            models,
        }
    }
}

impl OpenAiCompatibleEndpoint {
    /// The prefix used for LLMId values derived from custom endpoints.
    pub const ID_PREFIX: &'static str = "custom:";

    /// Returns the LLMId for a specific model on this endpoint
    /// (e.g. "custom:endpoint-2:model-0").
    pub fn llm_id_for_model(&self, model_index: usize) -> crate::LLMId {
        format!("{}{}:model-{}", Self::ID_PREFIX, self.id, model_index).into()
    }

    /// Extracts the endpoint ID and model index from an LLMId.
    /// Returns (endpoint_id, model_index) if the format matches.
    pub fn parse_llm_id(llm_id: &str) -> Option<(String, usize)> {
        let after_prefix = llm_id.strip_prefix(Self::ID_PREFIX)?;
        let parts: Vec<&str> = after_prefix.splitn(2, ':').collect();
        if parts.len() == 2 {
            let endpoint_id = parts[0].to_string();
            let model_str = parts[1].strip_prefix("model-")?;
            let model_index = model_str.parse::<usize>().ok()?;
            Some((endpoint_id, model_index))
        } else {
            None
        }
    }

    /// Returns the full URL for the chat completions endpoint.
    pub fn chat_completions_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else if base.ends_with("/v1")
            || base.ends_with("/v1/")
            || base.ends_with("/v2")
            || base.ends_with("/v2/")
        {
            format!("{}/chat/completions", base.trim_end_matches('/'))
        } else {
            format!("{}/v1/chat/completions", base)
        }
    }

    /// Returns true if this endpoint has an API key configured.
    pub fn has_api_key(&self) -> bool {
        self.has_api_key || self.api_key.as_ref().is_some_and(|k| !k.is_empty())
    }

    pub fn secure_storage_key(endpoint_id: &str) -> String {
        format!("CustomEndpoint:{}:api_key", endpoint_id)
    }

    /// Generates a unique model ID for a new model on this endpoint.
    pub fn generate_unique_model_index(&self) -> usize {
        self.models.len()
    }
}

/// A collection of user-configured OpenAI-compatible endpoints.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OpenAiCompatibleEndpoints(pub Vec<OpenAiCompatibleEndpoint>);

impl OpenAiCompatibleEndpoints {
    /// Finds an endpoint by its ID.
    pub fn get_by_id(&self, id: &str) -> Option<&OpenAiCompatibleEndpoint> {
        self.0.iter().find(|e| e.id == id)
    }

    /// Finds an endpoint and model by LLMId (e.g. "custom:endpoint-2:model-0").
    /// Returns (endpoint, model_index).
    pub fn get_by_llm_id(&self, llm_id: &str) -> Option<(&OpenAiCompatibleEndpoint, usize)> {
        let (endpoint_id, model_index) = OpenAiCompatibleEndpoint::parse_llm_id(llm_id)?;
        let endpoint = self.get_by_id(&endpoint_id)?;
        if model_index < endpoint.models.len() {
            Some((endpoint, model_index))
        } else {
            None
        }
    }

    /// Returns true if there are any endpoints configured.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over the endpoints.
    pub fn iter(&self) -> impl Iterator<Item = &OpenAiCompatibleEndpoint> {
        self.0.iter()
    }

    /// Generates a unique ID for a new endpoint.
    pub fn generate_unique_id(&self) -> String {
        let mut idx = self.0.len() + 1;
        while self.0.iter().any(|e| e.id == format!("endpoint-{idx}")) {
            idx += 1;
        }
        format!("endpoint-{idx}")
    }
}

impl settings_value::SettingsValue for OpenAiCompatibleEndpoints {}
