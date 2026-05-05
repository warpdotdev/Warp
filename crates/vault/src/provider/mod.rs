pub mod aws;

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;

use anyhow::{bail, Result};
use async_trait::async_trait;

pub struct Secret {
    env_var: String,
    value: zeroize::Zeroizing<String>,
}

impl Secret {
    pub fn new(env_var: String, value: String) -> Result<Self> {
        if !is_valid_env_var(&env_var) {
            bail!(
                "vault: invalid environment variable name '{}' — must contain only letters, digits, and underscores",
                env_var
            );
        }
        Ok(Self {
            env_var,
            value: zeroize::Zeroizing::new(value),
        })
    }

    pub fn env_var(&self) -> &str {
        &self.env_var
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

pub fn is_valid_env_var(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn fetch(&self, path: &str, env_var: &str) -> Result<Secret>;
}
