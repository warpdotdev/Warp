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

/// Language server candidate for the VS Code [JSON language server][upstream],
/// distributed on npm as [`vscode-json-languageserver`][npm].
///
/// This is the same JSON LSP that ships inside VS Code; it powers schema-aware
/// validation, hover, completion, and `$ref` go-to-definition for `.json` and
/// `.jsonc` files. It is also the LSP recommended by the issue tracker for
/// closing the "Language support is unavailable for this file type" gap on
/// JSON in Warp's editor.
///
/// [upstream]: https://github.com/microsoft/vscode/tree/main/extensions/json-language-features/server
/// [npm]: https://www.npmjs.com/package/vscode-json-languageserver
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub struct VsCodeJsonLanguageServerCandidate {
    client: Arc<http_client::Client>,
}

impl VsCodeJsonLanguageServerCandidate {
    /// Path to the langserver JS entry point relative to the install directory.
    /// Mirrors the layout produced by `npm install vscode-json-languageserver`.
    #[cfg(feature = "local_fs")]
    const LANGSERVER_JS_PATH: &str =
        "node_modules/vscode-json-languageserver/dist/node/jsonServerMain.js";

    pub fn new(client: Arc<http_client::Client>) -> Self {
        Self { client }
    }

    /// Finds the configuration for running the JSON language server from our
    /// custom installation.
    ///
    /// Like Pyright, we run node directly with the langserver JS file rather
    /// than relying on the `vscode-json-languageserver` wrapper script (which
    /// has a node shebang). First tries our custom node, then falls back to
    /// system node.
    #[cfg(feature = "local_fs")]
    pub async fn find_installed_binary_config(
        path_env_var: Option<&str>,
    ) -> Option<CustomBinaryConfig> {
        let install_dir = warp_core::paths::data_dir().join("vscode-json-languageserver");
        let langserver_js = install_dir.join(Self::LANGSERVER_JS_PATH);

        if !langserver_js.is_file() {
            log::info!(
                "vscode-json-languageserver entry point not found at {}",
                langserver_js.display()
            );
            return None;
        }

        let node_binary = node_runtime::find_working_node_binary(path_env_var).await?;

        // Verify the install works by spawning `node jsonServerMain.js --help`.
        // The server prints usage on --help and exits 0 even though it has no
        // dedicated --version flag.
        let mut cmd = Command::new(&node_binary);
        if let Some(path) = path_env_var {
            cmd.env("PATH", path);
        }
        cmd.arg(&langserver_js).arg("--help");
        match cmd.output().await {
            Ok(output) if output.status.success() => {
                log::info!("Verified vscode-json-languageserver installation");
            }
            Ok(output) => {
                log::warn!(
                    "vscode-json-languageserver health check failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return None;
            }
            Err(e) => {
                log::warn!("Failed to run vscode-json-languageserver health check: {e}");
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
impl LanguageServerCandidate for VsCodeJsonLanguageServerCandidate {
    async fn should_suggest_for_repo(&self, path: &Path, _executor: &CommandBuilder) -> bool {
        // Almost every meaningfully-structured repo has at least one JSON
        // config file. Use the most common ones as the trigger so we don't
        // recommend the JSON server for repos that just happen to contain a
        // single `package-lock.json`-style artifact.
        path.join("package.json").exists()
            || path.join("tsconfig.json").exists()
            || path.join("composer.json").exists()
            || path.join(".vscode").join("settings.json").exists()
    }

    async fn is_installed_in_data_dir(&self, executor: &CommandBuilder) -> bool {
        Self::find_installed_binary_config(executor.path_env_var())
            .await
            .is_some()
    }

    async fn is_installed_on_path(&self, executor: &CommandBuilder) -> bool {
        executor
            .command("vscode-json-languageserver")
            .arg("--help")
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
        log::info!(
            "Installing vscode-json-languageserver version {}",
            metadata.version
        );

        let install_dir = warp_core::paths::data_dir().join("vscode-json-languageserver");

        async_fs::create_dir_all(&install_dir)
            .await
            .context("Failed to create vscode-json-languageserver install directory")?;

        let use_system_node = match executor.path_env_var() {
            Some(path) => node_runtime::detect_system_node(path).await.is_ok(),
            None => false,
        };

        let custom_node_paths = if use_system_node {
            log::info!("Using system Node.js for vscode-json-languageserver installation");
            None
        } else {
            log::info!("System Node.js not found or too old, installing custom Node.js");
            node_runtime::install_npm(&self.client).await?;
            Some((
                node_runtime::node_binary_path()?,
                node_runtime::npm_binary_path()?,
            ))
        };

        log::info!(
            "Installing vscode-json-languageserver@{} using npm",
            metadata.version
        );

        let mut cmd = if let Some((node_path, npm_path)) = &custom_node_paths {
            let mut c = executor.command(node_path);
            c.arg(npm_path);
            c
        } else {
            executor.command("npm")
        };

        cmd.arg("install")
            .arg("--ignore-scripts")
            .arg(format!("vscode-json-languageserver@{}", metadata.version))
            .current_dir(&install_dir);

        let output = cmd.output().await.context("Failed to run npm install")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to install vscode-json-languageserver via npm: {}",
                stderr
            );
        }

        log::info!("vscode-json-languageserver installed successfully");
        Ok(())
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        let version =
            node_runtime::fetch_npm_package_version(&self.client, "vscode-json-languageserver")
                .await
                .context("Failed to fetch vscode-json-languageserver version from npm registry")?;

        Ok(LanguageServerMetadata {
            version,
            url: None,
            digest: None,
        })
    }
}

#[async_trait]
#[cfg(not(feature = "local_fs"))]
impl LanguageServerCandidate for VsCodeJsonLanguageServerCandidate {
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
