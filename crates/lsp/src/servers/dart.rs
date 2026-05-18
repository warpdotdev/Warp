use std::path::Path;
use std::sync::Arc;

use crate::language_server_candidate::{LanguageServerCandidate, LanguageServerMetadata};
use crate::CommandBuilder;
use async_trait::async_trait;

/// The Dart Analysis Server is built into the Dart SDK and invoked via
/// `dart language-server`. It communicates over stdin/stdout using LSP.
///
/// Custom installation (downloading the Dart SDK) is not yet supported;
/// the Dart SDK must be installed separately and available on PATH.

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub struct DartAnalysisServerCandidate {
    #[allow(dead_code)]
    client: Arc<http_client::Client>,
}

impl DartAnalysisServerCandidate {
    pub fn new(client: Arc<http_client::Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
#[cfg(feature = "local_fs")]
impl LanguageServerCandidate for DartAnalysisServerCandidate {
    async fn should_suggest_for_repo(&self, path: &Path, executor: &CommandBuilder) -> bool {
        // Check for pubspec.yaml — the standard Dart/Flutter project marker
        if !path.join("pubspec.yaml").exists() {
            return false;
        }

        // Also verify the Dart SDK is available
        executor
            .command("dart")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn is_installed_in_data_dir(&self, _executor: &CommandBuilder) -> bool {
        // Custom Dart SDK installation is not yet supported
        false
    }

    async fn is_installed_on_path(&self, executor: &CommandBuilder) -> bool {
        // Verify the Dart SDK is available by running `dart --version`
        executor
            .command("dart")
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
        // Custom installation is not yet supported.
        // The user must install the Dart SDK manually from https://dart.dev/get-dart
        anyhow::bail!(
            "Dart SDK installation is not yet automated. \
             Install the Dart SDK from https://dart.dev/get-dart and ensure \
             `dart` is available on your PATH."
        )
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        // The Dart Analysis Server is bundled with the Dart SDK — there is no
        // separate server version to track here. Return a placeholder.
        Ok(LanguageServerMetadata {
            version: "bundled".to_string(),
            url: None,
            digest: None,
        })
    }
}

#[async_trait]
#[cfg(not(feature = "local_fs"))]
impl LanguageServerCandidate for DartAnalysisServerCandidate {
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
