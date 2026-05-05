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
    env_var: String,
}

impl SecretMapping {
    pub fn new(path: String, env_var: String) -> anyhow::Result<Self> {
        if !crate::provider::is_valid_env_var(&env_var) {
            anyhow::bail!(
                "vault: invalid environment variable name '{}' — must contain only letters, digits, and underscores",
                env_var
            );
        }
        Ok(Self { path, env_var })
    }

    pub fn env_var(&self) -> &str {
        &self.env_var
    }
}

impl VaultConfig {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let contents = fs::read_to_string(&path).with_context(|| {
            format!(
                "no config found at {}. Create it with:\n\n  [provider]\n  type = \"aws\"\n  region = \"us-east-1\"\n\n  [mappings]\n  \"your/secret/path\" = \"ENV_VAR_NAME\"",
                path.display()
            )
        })?;
        toml::from_str(&contents).context("failed to parse vault config")
    }

    pub fn mappings(&self) -> anyhow::Result<Vec<SecretMapping>> {
        self.mappings
            .iter()
            .map(|(path, env_var)| SecretMapping::new(path.clone(), env_var.clone()))
            .collect()
    }
}

fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("vault: could not determine home directory"))?;
    Ok(home.join(".warp").join("vault.toml"))
}
