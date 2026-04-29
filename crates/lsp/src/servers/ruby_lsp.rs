use std::path::Path;
use std::sync::Arc;

use crate::language_server_candidate::{LanguageServerCandidate, LanguageServerMetadata};
use crate::CommandBuilder;
use async_trait::async_trait;

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub struct RubyLspCandidate {
    #[allow(dead_code)]
    client: Arc<http_client::Client>,
}

impl RubyLspCandidate {
    pub fn new(client: Arc<http_client::Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
#[cfg(feature = "local_fs")]
impl LanguageServerCandidate for RubyLspCandidate {
    async fn should_suggest_for_repo(&self, path: &Path, _executor: &CommandBuilder) -> bool {
        if path.join("Gemfile").exists()
            || path.join("Rakefile").exists()
            || path.join(".ruby-version").exists()
            || path.join("config.ru").exists()
        {
            return true;
        }

        std::fs::read_dir(path)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry.path().extension().and_then(|s| s.to_str()) == Some("gemspec")
                })
            })
            .unwrap_or(false)
    }

    async fn is_installed_in_data_dir(&self, _executor: &CommandBuilder) -> bool {
        false
    }

    async fn is_installed_on_path(&self, executor: &CommandBuilder) -> bool {
        executor
            .command("ruby-lsp")
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
        anyhow::bail!("Install ruby-lsp manually: `gem install ruby-lsp`")
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        anyhow::bail!("Auto-install not supported; install via `gem install ruby-lsp`")
    }
}

#[async_trait]
#[cfg(not(feature = "local_fs"))]
impl LanguageServerCandidate for RubyLspCandidate {
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
