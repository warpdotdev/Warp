use std::path::Path;
use std::sync::Arc;

use crate::language_server_candidate::{LanguageServerCandidate, LanguageServerMetadata};
#[cfg(feature = "local_fs")]
use crate::supported_servers::CustomBinaryConfig;
use crate::CommandBuilder;
use async_trait::async_trait;

#[cfg(feature = "local_fs")]
use anyhow::Context;

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
    /// Path to the langserver JS entry point relative to the install directory,
    /// matching the layout published by the `vscode-json-languageserver` npm
    /// package (whose `package.json` declares `"main": "./out/node/jsonServerMain"`).
    #[cfg(feature = "local_fs")]
    const LANGSERVER_JS_PATH: &str =
        "node_modules/vscode-json-languageserver/out/node/jsonServerMain.js";

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
    ///
    /// `vscode-json-languageserver` only documents `--stdio`, `--node-ipc`,
    /// and `--socket=` as transport flags. `--version` and `--help` go through
    /// the language-server connection setup and exit with the missing
    /// connection-transport error, so we treat the presence of the
    /// npm-installed entry-point JS file as proof that the install completed.
    /// Any genuine failure surfaces through the LSP transport itself when
    /// `command_and_params` later spawns the server with `--stdio`.
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

        log::info!(
            "Found vscode-json-languageserver installation at {}",
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
        // `vscode-json-languageserver` only documents `--stdio`, `--node-ipc`,
        // and `--socket=` as transport flags. `--version`/`--help` enter
        // connection-transport setup and exit non-zero; spawning the server
        // with `--stdio` and EOF on stdin works but treats every spawnable
        // binary as healthy (including a corrupted install). Use a pure
        // filesystem PATH search instead — the binary either exists and is
        // executable or it doesn't, and we never inadvertently prefer a
        // broken global install over our (working) data_dir copy.
        binary_in_path("vscode-json-languageserver", executor.path_env_var())
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

/// Pure-filesystem search for an executable named `name` in any directory
/// listed by `path_env_var` (or the process's `PATH` if `None`).
///
/// We use this instead of spawning the binary with a probe flag for LSP
/// servers that have no documented version/help argument — running them
/// with arbitrary flags either errors during connection-transport setup
/// or hangs reading from stdin. A filesystem check has no such ambiguity:
/// the file either exists and is executable, or it doesn't.
///
/// On Windows we additionally try the standard executable extensions
/// (`.exe`, `.cmd`, `.bat`) since the `vscode-json-languageserver` npm
/// package ships a `.cmd` shim there.
///
/// On Unix the candidate must also have at least one executable mode bit
/// set; otherwise a leftover non-executable file in `~/bin/` would
/// falsely advertise availability and Warp would later prefer the broken
/// PATH entry over a working data_dir copy.
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
            if is_executable_file(&candidate) {
                return true;
            }
        }
    }
    false
}

/// Returns `true` iff `path` is a regular file *and* the OS would treat it
/// as runnable. On Unix that means at least one of the executable mode
/// bits (`0o111`) is set; on Windows we trust the extension match (the
/// `.exe`/`.cmd`/`.bat` suffix added by the caller is what makes
/// `CreateProcessW` willing to launch it).
#[cfg(feature = "local_fs")]
fn is_executable_file(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        match std::fs::metadata(path) {
            Ok(meta) => meta.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
#[cfg(feature = "local_fs")]
mod tests {
    use super::binary_in_path;
    use std::ffi::OsString;
    use std::fs::{self, File};
    use std::path::Path;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    /// Joins multiple path components into a PATH-formatted string using
    /// the platform separator (`:` on Unix, `;` on Windows). Wraps
    /// `std::env::join_paths` so the new tests work cross-platform with
    /// the matching split logic in `binary_in_path`.
    fn make_path_var<I, P>(parts: I) -> String
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let owned: Vec<OsString> = parts
            .into_iter()
            .map(|p| p.as_ref().as_os_str().to_owned())
            .collect();
        std::env::join_paths(owned)
            .expect("failed to join PATH")
            .into_string()
            .expect("PATH contained non-UTF-8 component")
    }

    /// On Windows, `binary_in_path` expects an executable to end in
    /// `.exe`/`.cmd`/`.bat`. The npm shim is a `.cmd`, so use that as
    /// the test artefact when running on Windows.
    fn binary_filename(stem: &str) -> String {
        #[cfg(windows)]
        {
            format!("{stem}.cmd")
        }
        #[cfg(not(windows))]
        {
            stem.to_string()
        }
    }

    fn touch_exe(dir: &Path, stem: &str) -> std::path::PathBuf {
        let path = dir.join(binary_filename(stem));
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
        touch_exe(tmp.path(), "vscode-json-languageserver");
        let path_var = make_path_var([tmp.path(), Path::new("/nonexistent/dir")]);
        assert!(binary_in_path(
            "vscode-json-languageserver",
            Some(&path_var)
        ));
    }

    #[test]
    fn rejects_when_binary_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path_var = make_path_var([tmp.path()]);
        assert!(!binary_in_path(
            "vscode-json-languageserver",
            Some(&path_var)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_executable_file_on_unix() {
        // A regular non-executable file at `~/bin/vscode-json-languageserver`
        // (e.g. left over from unpacking a tarball) must not pretend to be
        // an installed binary — otherwise Warp would prefer this broken
        // PATH entry over a working data_dir copy.
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("vscode-json-languageserver");
        File::create(&path).expect("create non-exec file");
        let perms = fs::metadata(&path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o111,
            0,
            "test setup failed: file unexpectedly executable",
        );
        let path_var = make_path_var([tmp.path()]);
        assert!(!binary_in_path(
            "vscode-json-languageserver",
            Some(&path_var)
        ));
    }
}
