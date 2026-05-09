use super::config::LocalModelProvider;
use super::LocalModelResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait LocalModelClient: Send + Sync {
    fn provider(&self) -> LocalModelProvider;

    async fn check_connection(&self) -> LocalModelResult<()>;

    async fn list_models(&self) -> LocalModelResult<Vec<ModelInfo>>;

    async fn generate_completion(&self, prompt: &str, model: &str) -> LocalModelResult<String>;
}
