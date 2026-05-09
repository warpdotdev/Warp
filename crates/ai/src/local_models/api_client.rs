use super::config::LocalModelProvider;
use super::LocalModelResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ModelInfo — returned by list_models()
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub id: String,
}

impl ModelInfo {
    pub fn new(name: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: id.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ModelPickerEntry — sent to the UI for the model selector dropdown
// ---------------------------------------------------------------------------

/// Represents a single entry in the model picker UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPickerEntry {
    /// Internal model id (e.g. "llama3:8b").
    pub id: String,
    /// Human-readable label shown in the picker.
    pub display_name: String,
    /// Which provider serves this model.
    pub provider: LocalModelProvider,
    /// Whether this model is reachable (cached, updated every 30 s).
    pub status: ConnectionStatus,
    /// true = local/self-hosted, false = Warp cloud or external API
    pub is_local: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    /// Connection check succeeded.
    Online,
    /// Connection check failed or not yet attempted.
    Offline,
    /// Check is in progress.
    Checking,
}

// ---------------------------------------------------------------------------
// LocalModelClient trait
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait LocalModelClient: Send + Sync {
    fn provider(&self) -> LocalModelProvider;

    async fn check_connection(&self) -> LocalModelResult<()>;

    async fn list_models(&self) -> LocalModelResult<Vec<ModelInfo>>;

    async fn generate_completion(&self, prompt: &str, model: &str) -> LocalModelResult<String>;
}
