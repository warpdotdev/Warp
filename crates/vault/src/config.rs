#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VaultConfig {
    pub provider: ProviderConfig,
    #[serde(default)]
    pub mappings: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub region: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Aws,
}

pub struct SecretMapping {
    pub path: String,
    pub env_var: String,
}

impl VaultConfig {
    pub fn load() -> Result<Self> {
        let path = config_path();
        let contents = fs::read_to_string(&path).with_context(|| {
            format!(
                "no config found at {} — run 'oz vault init' to get started",
                path.display()
            )
        })?;
        toml::from_str(&contents).context("failed to parse vault config")
    }

    pub fn mappings(&self) -> Vec<SecretMapping> {
        self.mappings
            .iter()
            .map(|(path, env_var)| SecretMapping {
                path: path.clone(),
                env_var: env_var.clone(),
            })
            .collect()
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".warp")
        .join("vault.toml")
}
