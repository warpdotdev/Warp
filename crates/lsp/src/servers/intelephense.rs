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
        // First narrow the PATH search to a healthy-looking, executable
        // file (skips dangling symlinks, regular text files, etc.). This
        // alone is not sufficient — a stale npm shim or missing Node
        // would still pass — so we then probe-spawn the server with the
        // documented `--stdio` transport and stdin redirected to /dev/null.
        // intelephense's connection layer reads zero bytes, sees EOF, and
        // exits cleanly when the install is healthy. We require both the
        // file to be present *and* the spawn to exit with success status,
        // so a broken global shim never displaces Warp's data_dir copy.
        if !binary_in_path("intelephense", executor.path_env_var()) {
            return false;
        }
        executor
            .command("intelephense")
            .arg("--stdio")
            .stdin(std::process::Stdio::null())
            .output()
            .await
            .map(|output| output.status.success())
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
///
/// On Unix the candidate must also have at least one executable mode bit
/// set; otherwise a leftover `intelephense` source-tarball or stray text
/// file in `~/bin/` would falsely advertise availability and Warp would
/// later prefer the broken PATH entry over a working data_dir copy.
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

    /// Joins multiple path components into a single PATH-formatted string
    /// using the platform separator (`:` on Unix, `;` on Windows). Wraps
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
    /// `.exe`/`.cmd`/`.bat`. The `intelephense` npm shim is a `.cmd` so
    /// use that as the test artefact when running on Windows.
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

    /// Creates a fake executable at `dir/<name><ext>`. On Unix it gets
    /// the executable bit; on Windows the `.cmd`/`.exe`/`.bat` suffix is
    /// what makes it count as runnable.
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
        touch_exe(tmp.path(), "intelephense");
        // Use platform-appropriate separator via `std::env::join_paths`
        // so this test passes both on Unix (`:`) and Windows (`;`).
        let path_var = make_path_var([tmp.path(), Path::new("/nonexistent/dir")]);
        assert!(binary_in_path("intelephense", Some(&path_var)));
    }

    #[test]
    fn rejects_when_binary_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path_var = make_path_var([tmp.path()]);
        assert!(!binary_in_path("intelephense", Some(&path_var)));
    }

    #[test]
    fn skips_empty_path_segments() {
        // Empty PATH segments arise from `:::` (Unix) or `;;` (Windows).
        // `std::env::join_paths` rejects empty components, so build the
        // string manually but use the right separator per platform.
        let tmp = tempfile::tempdir().expect("tempdir");
        touch_exe(tmp.path(), "intelephense");
        let separator = if cfg!(windows) { ';' } else { ':' };
        let path_var = format!("{separator}{}", tmp.path().display());
        assert!(binary_in_path("intelephense", Some(&path_var)));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_executable_file_on_unix() {
        // A regular text file at `~/bin/intelephense` (e.g. left over from
        // unpacking a tarball) must not pretend to be an installed binary —
        // otherwise Warp would prefer this broken PATH entry over a working
        // data_dir copy.
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("intelephense");
        File::create(&path).expect("create non-exec file");
        // Default permissions: 0o644 (no executable bits).
        let perms = fs::metadata(&path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o111,
            0,
            "test setup failed: file unexpectedly executable",
        );
        let path_var = make_path_var([tmp.path()]);
        assert!(!binary_in_path("intelephense", Some(&path_var)));
    }
}
