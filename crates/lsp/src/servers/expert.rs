use std::path::Path;
use std::sync::Arc;

use crate::language_server_candidate::{LanguageServerCandidate, LanguageServerMetadata};
use crate::CommandBuilder;
use async_trait::async_trait;

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub struct ExpertCandidate {
    #[allow(dead_code)]
    client: Arc<http_client::Client>,
}

impl ExpertCandidate {
    pub fn new(client: Arc<http_client::Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
#[cfg(feature = "local_fs")]
impl LanguageServerCandidate for ExpertCandidate {
    async fn should_suggest_for_repo(&self, path: &Path, executor: &CommandBuilder) -> bool {
        (path.join("mix.exs").exists()
            || path.join("mix.lock").exists()
            || path.join(".formatter.exs").exists())
            && self.is_installed_on_path(executor).await
    }

    async fn is_installed_in_data_dir(&self, _executor: &CommandBuilder) -> bool {
        false
    }

    async fn is_installed_on_path(&self, executor: &CommandBuilder) -> bool {
        executor
            .command("expert")
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
        anyhow::bail!(
            "Install Expert manually: download the binary for your platform (e.g. `expert_darwin_arm64`) from https://github.com/expert-lsp/expert/releases, rename it to `expert`, and place it on your PATH"
        )
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        anyhow::bail!(
            "Auto-install not supported; download from https://github.com/expert-lsp/expert/releases"
        )
    }
}

#[async_trait]
#[cfg(not(feature = "local_fs"))]
impl LanguageServerCandidate for ExpertCandidate {
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
