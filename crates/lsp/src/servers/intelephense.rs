use std::path::Path;
use std::sync::Arc;

use crate::language_server_candidate::{LanguageServerCandidate, LanguageServerMetadata};
#[cfg(feature = "local_fs")]
use crate::supported_servers::CustomBinaryConfig;
use crate::CommandBuilder;
use async_trait::async_trait;

#[cfg(feature = "local_fs")]
use anyhow::Context;
#[cfg(feature = "local_fs")]
use command::r#async::Command;

/// Language server candidate for [Intelephense](https://intelephense.com/),
/// the de-facto standard PHP language server (used by Zed, VS Code, Neovim,
/// Helix, Sublime LSP, and Emacs lsp-mode by default).
///
/// Intelephense is distributed as the [`intelephense`](https://www.npmjs.com/package/intelephense)
/// npm package. The package ships a Node.js entry point at
/// `node_modules/intelephense/lib/intelephense.js`. Premium licensing (rename,
/// code actions, etc.) is honoured via `~/intelephense/licence.txt`, which
/// Intelephense reads itself — Warp does not need to plumb it explicitly.
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub struct IntelephenseCandidate {
    client: Arc<http_client::Client>,
}

impl IntelephenseCandidate {
    /// Path to the langserver JS entry point relative to the install directory.
    /// Mirrors the layout produced by `npm install intelephense`.
    #[cfg(feature = "local_fs")]
    const LANGSERVER_JS_PATH: &str = "node_modules/intelephense/lib/intelephense.js";

    pub fn new(client: Arc<http_client::Client>) -> Self {
        Self { client }
    }

    /// Finds the configuration for running Intelephense from our custom installation.
    ///
    /// Like Pyright, we run node directly with the langserver JS file rather than
    /// relying on the `intelephense` wrapper script (which has a node shebang).
    /// First tries our custom node installation, then falls back to system node.
    #[cfg(feature = "local_fs")]
    pub async fn find_installed_binary_config(
        path_env_var: Option<&str>,
    ) -> Option<CustomBinaryConfig> {
        let install_dir = warp_core::paths::data_dir().join("intelephense");
        let langserver_js = install_dir.join(Self::LANGSERVER_JS_PATH);

        if !langserver_js.is_file() {
            log::info!(
                "Intelephense entry point not found at {}",
                langserver_js.display()
            );
            return None;
        }

        let node_binary = node_runtime::find_working_node_binary(path_env_var).await?;

        // Verify the install works by spawning `node intelephense.js --version`.
        // Intelephense exits 0 with a version string on stdout; if the install
        // is broken, the spawn fails or the exit is non-zero.
        let mut cmd = Command::new(&node_binary);
        if let Some(path) = path_env_var {
            cmd.env("PATH", path);
        }
        cmd.arg(&langserver_js).arg("--version");
        match cmd.output().await {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                log::info!("Verified intelephense installation: {}", version.trim());
            }
            Ok(output) => {
                log::warn!(
                    "Intelephense version check failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return None;
            }
            Err(e) => {
                log::warn!("Failed to run intelephense version check: {e}");
                return None;
            }
        }

        Some(CustomBinaryConfig {
            binary_path: node_binary,
            prepend_args: vec![langserver_js.to_string_lossy().to_string()],
        })
    }
}

#[async_trait]
#[cfg(feature = "local_fs")]
impl LanguageServerCandidate for IntelephenseCandidate {
    async fn should_suggest_for_repo(&self, path: &Path, _executor: &CommandBuilder) -> bool {
        // Common PHP project indicators across Composer, Laravel, and WordPress.
        path.join("composer.json").exists()
            || path.join("composer.lock").exists()
            || path.join("artisan").exists()
            || path.join("wp-config.php").exists()
    }

    async fn is_installed_in_data_dir(&self, executor: &CommandBuilder) -> bool {
        Self::find_installed_binary_config(executor.path_env_var())
            .await
            .is_some()
    }

    async fn is_installed_on_path(&self, executor: &CommandBuilder) -> bool {
        executor
            .command("intelephense")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn install(
        &self,
        metadata: LanguageServerMetadata,
        executor: &CommandBuilder,
    ) -> anyhow::Result<()> {
        log::info!("Installing intelephense version {}", metadata.version);

        let install_dir = warp_core::paths::data_dir().join("intelephense");

        async_fs::create_dir_all(&install_dir)
            .await
            .context("Failed to create intelephense installation directory")?;

        // Prefer the user's system node when it meets the runtime requirement;
        // otherwise install Warp's bundled node alongside the package.
        let use_system_node = match executor.path_env_var() {
            Some(path) => node_runtime::detect_system_node(path).await.is_ok(),
            None => false,
        };

        let custom_node_paths = if use_system_node {
            log::info!("Using system Node.js for intelephense installation");
            None
        } else {
            log::info!("System Node.js not found or too old, installing custom Node.js");
            node_runtime::install_npm(&self.client).await?;
            Some((
                node_runtime::node_binary_path()?,
                node_runtime::npm_binary_path()?,
            ))
        };

        log::info!("Installing intelephense@{} using npm", metadata.version);

        let mut cmd = if let Some((node_path, npm_path)) = &custom_node_paths {
            let mut c = executor.command(node_path);
            c.arg(npm_path);
            c
        } else {
            executor.command("npm")
        };

        cmd.arg("install")
            .arg("--ignore-scripts")
            .arg(format!("intelephense@{}", metadata.version))
            .current_dir(&install_dir);

        let output = cmd.output().await.context("Failed to run npm install")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to install intelephense via npm: {}", stderr);
        }

        log::info!("Intelephense installed successfully");
        Ok(())
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        let version = node_runtime::fetch_npm_package_version(&self.client, "intelephense")
            .await
            .context("Failed to fetch intelephense version from npm registry")?;

        Ok(LanguageServerMetadata {
            version,
            url: None,
            digest: None,
        })
    }
}

#[async_trait]
#[cfg(not(feature = "local_fs"))]
impl LanguageServerCandidate for IntelephenseCandidate {
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
