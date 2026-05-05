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

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub struct PhpIntelephenseCandidate {
    client: Arc<http_client::Client>,
}

impl PhpIntelephenseCandidate {
    /// Path to the language server JS file relative to the intelephense install directory.
    #[cfg(feature = "local_fs")]
    const LANGSERVER_JS_PATH: &str = "node_modules/intelephense/lib/intelephense.js";

    pub fn new(client: Arc<http_client::Client>) -> Self {
        Self { client }
    }

    /// Finds the configuration for running intelephense from our custom installation.
    ///
    /// Instead of running the `intelephense` wrapper script (which has a shebang
    /// requiring node in PATH), we run node directly with the intelephense.js file.
    /// This is the same pattern used by pyright.
    ///
    /// Intelephense can be installed with either system node or our custom node. This function
    /// checks for both cases:
    /// 1. First tries our custom node installation
    /// 2. Falls back to system node if custom node isn't available
    ///
    /// # Arguments
    /// * `path_env_var` - The PATH environment variable to use when checking for system node.
    #[cfg(feature = "local_fs")]
    pub async fn find_installed_binary_config(
        path_env_var: Option<&str>,
    ) -> Option<CustomBinaryConfig> {
        let install_dir = warp_core::paths::data_dir().join("intelephense");
        let langserver_js = install_dir.join(Self::LANGSERVER_JS_PATH);

        if !langserver_js.is_file() {
            log::info!(
                "Intelephense language server JS not found at {}",
                langserver_js.display()
            );
            return None;
        }

        let node_binary = node_runtime::find_working_node_binary(path_env_var).await?;

        // Verify the intelephense installation works by running `node intelephense.js --version`
        let mut cmd = Command::new(&node_binary);
        // Propagate PATH so "node" (bare name) resolves when using system node
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
                log::warn!("Failed to run intelephense version check: {}", e);
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
impl LanguageServerCandidate for PhpIntelephenseCandidate {
    async fn should_suggest_for_repo(&self, path: &Path, _executor: &CommandBuilder) -> bool {
        path.join("composer.json").exists()
            || path.join("composer.lock").exists()
            || path.join(".php_cs").exists()
            || path.join("phpunit.xml").exists()
            || path.join("phpunit.xml.dist").exists()
            || path.join("artisan").exists()
            || std::fs::read_dir(path).map_or(false, |entries| {
                entries.flatten().any(|entry| {
                    let file_path = entry.path();
                    file_path.is_file()
                        && file_path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .is_some_and(|ext| {
                                matches!(ext, "php" | "phtml" | "php3" | "php4" | "php5" | "phps")
                            })
                })
            })
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

        // First, check if system node is available and meets requirements
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
impl LanguageServerCandidate for PhpIntelephenseCandidate {
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
