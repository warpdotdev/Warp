use std::path::Path;
use std::sync::Arc;

use crate::language_server_candidate::{LanguageServerCandidate, LanguageServerMetadata};
#[cfg(feature = "local_fs")]
use crate::supported_servers::CustomBinaryConfig;
use crate::CommandBuilder;
use async_trait::async_trait;

#[cfg(feature = "local_fs")]
use anyhow::Context;

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
    ///
    /// Unlike Pyright, Intelephense does not document a `--version` flag — its
    /// only documented arguments are LSP transport flags (`--stdio`,
    /// `--node-ipc`, `--socket=N`, `--pipe=X`). Running it with `--version`
    /// would either error out or hang waiting for LSP messages, so we treat
    /// the presence of the npm-installed entry-point JS file as the proof
    /// that the install completed. The next call to `command_and_params`
    /// will spawn the server with `--stdio` and any real failure (corrupted
    /// install, broken node, etc.) surfaces through the LSP transport itself.
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

        log::info!(
            "Found intelephense installation at {}",
            langserver_js.display()
        );

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
        // Intelephense's CLI surface is only LSP transport flags (`--stdio`,
        // `--node-ipc`, `--socket=N`, `--pipe=X`); `--version` is not
        // documented and `--help` enters connection-transport setup, so any
        // spawn-based probe is unreliable — exit codes don't distinguish a
        // healthy install from a broken one. Use a pure filesystem PATH
        // search instead: locate an executable named `intelephense` in any
        // PATH entry. If it isn't there we fall through to the data_dir
        // install path (which we control), so a broken global install
        // never prevents Warp from using a known-good copy.
        binary_in_path("intelephense", executor.path_env_var())
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

/// Pure-filesystem search for an executable named `name` in any directory
/// listed by `path_env_var` (or the process's `PATH` if `None`).
///
/// We use this instead of spawning the binary with a probe flag for LSP
/// servers that have no documented version/help argument — running them with
/// arbitrary flags either errors during connection-transport setup or hangs
/// reading from stdin, which would cause `is_installed_on_path` to either
/// reject working installs or accept broken ones based on noise. A
/// filesystem check has no such ambiguity: the file either exists and is
/// executable, or it doesn't.
///
/// On Windows we additionally try the standard executable extensions
/// (`.exe`, `.cmd`, `.bat`) since intelephense is shipped via npm as a
/// `.cmd` shim there.
#[cfg(feature = "local_fs")]
fn binary_in_path(name: &str, path_env_var: Option<&str>) -> bool {
    let owned;
    let path_str = match path_env_var {
        Some(p) => p,
        None => match std::env::var("PATH") {
            Ok(p) => {
                owned = p;
                owned.as_str()
            }
            Err(_) => return false,
        },
    };
    let separator = if cfg!(windows) { ';' } else { ':' };
    #[cfg(windows)]
    let extensions: &[&str] = &["", ".exe", ".cmd", ".bat"];
    #[cfg(not(windows))]
    let extensions: &[&str] = &[""];

    for dir in path_str.split(separator) {
        if dir.is_empty() {
            continue;
        }
        let dir_path = std::path::Path::new(dir);
        for ext in extensions {
            let candidate = dir_path.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
#[cfg(feature = "local_fs")]
mod tests {
    use super::binary_in_path;
    use std::fs::{self, File};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    /// Creates a fake executable file at `dir/name`. On Unix it gets the
    /// executable bit; on Windows the suffix decides resolution.
    fn touch_exe(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        File::create(&path).expect("create test binary");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    #[test]
    fn finds_binary_in_first_path_entry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        touch_exe(tmp.path(), "intelephense");
        let path_var = format!("{}:{}", tmp.path().display(), "/nonexistent/dir");
        assert!(binary_in_path("intelephense", Some(&path_var)));
    }

    #[test]
    fn rejects_when_binary_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path_var = tmp.path().display().to_string();
        assert!(!binary_in_path("intelephense", Some(&path_var)));
    }

    #[test]
    fn skips_empty_path_segments() {
        let tmp = tempfile::tempdir().expect("tempdir");
        touch_exe(tmp.path(), "intelephense");
        // Leading empty segment must not blow up or short-circuit.
        let path_var = format!(":{}", tmp.path().display());
        assert!(binary_in_path("intelephense", Some(&path_var)));
    }
}
