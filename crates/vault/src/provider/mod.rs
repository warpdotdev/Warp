pub mod aws;

use anyhow::Result;
use async_trait::async_trait;

pub struct Secret {
    pub env_var: String,
    value: zeroize::Zeroizing<String>,
}

impl Secret {
    pub fn new(env_var: String, value: String) -> Self {
        Self {
            env_var,
            value: zeroize::Zeroizing::new(value),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn fetch(&self, path: &str, env_var: &str) -> Result<Secret>;
}
