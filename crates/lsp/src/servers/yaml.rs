use std::path::Path;

use crate::language_server_candidate::{LanguageServerCandidate, LanguageServerMetadata};
use crate::CommandBuilder;
use async_trait::async_trait;

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub struct YamlLanguageServerCandidate;

impl YamlLanguageServerCandidate {
    pub fn new() -> Self {
        Self
    }

#[async_trait]
#[cfg(feature = "local_fs")]
impl LanguageServerCandidate for YamlLanguageServerCandidate {
    async fn should_suggest_for_repo(&self, path: &Path, executor: &CommandBuilder) -> bool {
        // Only suggest YAML server if yaml-language-server is available on PATH
        // and there are YAML files present
        let has_yaml_files = if let Ok(entries) = std::fs::read_dir(path) {
            entries.flatten().any(|entry| {
                entry.file_name()
                    .to_str()
                    .map(|name| name.ends_with(".yaml") || name.ends_with(".yml"))
                    .unwrap_or(false)
            })
        } else {
            false
        };

        has_yaml_files && self.is_installed_on_path(executor).await
    }

    async fn is_installed_in_data_dir(&self, _executor: &CommandBuilder) -> bool {
        // yaml-language-server doesn't support custom installation yet
        false
    }

    async fn is_installed_on_path(&self, executor: &CommandBuilder) -> bool {
        executor
            .command("yaml-language-server")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn install(
        &self,
        _metadata: LanguageServerMetadata,
        _executor: &CommandBuilder,
    ) -> anyhow::Result<()> {
        anyhow::bail!("YAML language server installation is not yet supported. Please install yaml-language-server via npm: npm install -g yaml-language-server");
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        // yaml-language-server is an npm package, not a GitHub release
        // Return placeholder metadata since we don't support automatic installation
        Ok(LanguageServerMetadata {
            version: "unknown".to_string(),
            url: None,
            digest: None,
        })
    }
}

#[async_trait]
#[cfg(not(feature = "local_fs"))]
impl LanguageServerCandidate for YamlLanguageServerCandidate {
    async fn should_suggest_for_repo(&self, _path: &Path, _executor: &CommandBuilder) -> bool {
        false
    }

    async fn is_installed_in_data_dir(&self, _executor: &CommandBuilder) -> bool {
        false
    }

    async fn is_installed_on_path(&self, _executor: &CommandBuilder) -> bool {
        false
    }

    async fn install(
        &self,
        _metadata: LanguageServerMetadata,
        _executor: &CommandBuilder,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        todo!()
    }
}
