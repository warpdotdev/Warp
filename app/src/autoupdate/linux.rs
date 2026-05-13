use std::io::Write;
use std::path::PathBuf;

use anyhow::{bail, Context as _, Result};
use channel_versions::VersionInfo;
use instant::Duration;
use warp_core::channel::{Channel, ChannelState};

use super::release_assets_directory_url;
use super::{DownloadReady, ReadyForRelaunch};

lazy_static::lazy_static! {
    /// Stores the path to the current executable.
    ///
    /// We cache this before running auto-update because the returned path for
    /// a deleted file includes " (deleted)" _in the file name_, which breaks
    /// the relaunch logic.
    static ref CURRENT_EXE: std::io::Result<PathBuf> = std::env::current_exe();
}

pub(super) async fn download_update_and_cleanup(
    version_info: &VersionInfo,
    _update_id: &str,
    client: &http_client::Client,
) -> Result<DownloadReady> {
    match UpdateMethod::detect() {
        UpdateMethod::Unknown => Ok(DownloadReady::NeedsAuthorization),
        UpdateMethod::AppImage(appimage_path) => {
            appimage::download_update_and_cleanup(version_info, &appimage_path, client).await
        }
        UpdateMethod::PackageManager(package_manager) => {
            log::info!("Detected that Warp was installed using {package_manager:?}");
            Ok(DownloadReady::NeedsAuthorization)
        }
    }
}

pub(super) fn apply_update() -> Result<ReadyForRelaunch> {
    // Make sure CURRENT_EXE is initialized before we actually apply the update.
    let _ = CURRENT_EXE.as_ref();

    match UpdateMethod::detect() {
        UpdateMethod::Unknown => bail!("Cannot apply update for unknown update method!"),
        UpdateMethod::AppImage(_) => Ok(ReadyForRelaunch::Yes),
        UpdateMethod::PackageManager(package_manager) => bail!(
            "OpenWarp does not support package-manager autoupdate for {package_manager}; install the new release manually"
        ),
    }
}

pub(super) fn relaunch() -> Result<()> {
    match UpdateMethod::detect() {
        UpdateMethod::Unknown => bail!("Don't know how to relaunch for an unknown update method!"),
        UpdateMethod::AppImage(appimage_path) => appimage::relaunch(&appimage_path),
        UpdateMethod::PackageManager(_) => package_manager::relaunch(),
    }
}

mod appimage {
    use std::path::Path;

    use super::*;

    pub(super) async fn download_update_and_cleanup(
        version_info: &VersionInfo,
        appimage_path: &Path,
        client: &http_client::Client,
    ) -> Result<DownloadReady> {
        const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

        // Compute the URL where we can download the new release.
        let Some(appimage_name) = option_env!("APPIMAGE_NAME") else {
            bail!("APPIMAGE_NAME environment variable was not set at compile time!");
        };

        let url = format!(
            "{}/{}",
            release_assets_directory_url(ChannelState::channel(), &version_info.version),
            appimage_name
        );

        // Create a temporary file that we'll write the download into.
        let mut new_appimage = tempfile::NamedTempFile::new()?;

        log::info!("Downloading {url} to {}...", new_appimage.path().display());

        let response = client
            .get(&url)
            .timeout(DOWNLOAD_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        new_appimage
            .as_file_mut()
            .write_all(&response.bytes().await?)?;

        log::info!(
            "Copying downloaded AppImage from {} to {}",
            new_appimage.path().display(),
            appimage_path.display()
        );

        // Copy permissions to new app before moving it to ensure we don't leave it
        // in a bad state if the move succeeds but we are unable to update the
        // permissions afterwards.
        new_appimage
            .as_file_mut()
            .set_permissions(appimage_path.metadata()?.permissions())?;

        // Move new AppImage over the one that launched the current Warp instance.
        let new_appimage_path = new_appimage.into_temp_path();
        let mv_status = command::r#async::Command::new("mv")
            .arg(new_appimage_path.as_os_str())
            .arg(appimage_path)
            .output()
            .await?
            .status;
        if !mv_status.success() {
            bail!("Failed to move new AppImage over the old one: {mv_status}");
        }

        // Ensure we don't accidentally drop `new_appimage_path` before we finish
        // moving it to its final location.
        let _ = new_appimage_path;

        Ok(DownloadReady::Yes)
    }

    pub(super) fn relaunch(appimage_path: &Path) -> Result<()> {
        let mut command = command::blocking::Command::new(appimage_path);
        // Pass a flag to the app to let it know it was restarted as part of the
        // autoupdate process.
        command.arg(warp_cli::finish_update_flag());
        // 测试本地通道版本 JSON 时，让新启动的二进制继续引用同一个文件，
        // 以便验证自动更新后的 changelog 展示。
        if let Ok(path) = std::env::var("WARP_CHANNEL_VERSIONS_PATH") {
            command.env("WARP_CHANNEL_VERSIONS_PATH", path);
        }

        log::info!("Relaunching warp for update...");
        command.spawn()?;
        Ok(())
    }
}

mod package_manager {
    use super::*;

    pub(super) fn relaunch() -> Result<()> {
        let Ok(program) = CURRENT_EXE.as_ref() else {
            bail!(
                "Failed to get path to current executable to relaunch after completing auto-update"
            );
        };
        log::info!("Relaunching using path: {program:?}");
        let mut command = command::blocking::Command::new(program);
        // Add any arguments that were passed to warp, skipping the first
        // argument (the name of the executable) and dropping the flag for
        // finishing an update.
        let finish_update_flag = warp_cli::finish_update_flag();
        command.args(
            std::env::args()
                .skip(1)
                .filter(|arg| arg != &finish_update_flag),
        );
        // Pass a flag to the app to let it know it was restarted as part of the
        // autoupdate process.
        command.arg(finish_update_flag);
        // 测试本地通道版本 JSON 时，让新启动的二进制继续引用同一个文件，
        // 以便验证自动更新后的 changelog 展示。
        if let Ok(path) = std::env::var("WARP_CHANNEL_VERSIONS_PATH") {
            command.env("WARP_CHANNEL_VERSIONS_PATH", path);
        }

        log::info!("Relaunching warp for update...");
        command.spawn()?;
        Ok(())
    }
}

/// Returns which method should be used to update Warp.
#[derive(Debug)]
pub(crate) enum UpdateMethod {
    /// We don't know how to update Warp.
    Unknown,
    /// Warp is running as an AppImage and should be updated in-place.
    AppImage(PathBuf),
    /// Warp can be updated using the given package manager.
    PackageManager(PackageManager),
}

impl UpdateMethod {
    pub(crate) fn detect() -> Self {
        if let Some(appimage_path) = std::env::var_os("APPIMAGE").map(PathBuf::from) {
            return Self::AppImage(appimage_path);
        }
        if let Ok(package_manager) = PackageManager::detect() {
            return Self::PackageManager(package_manager);
        }
        Self::Unknown
    }
}

/// Package managers that we understand and can assist with auto-update
/// for.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Yum,
    Dnf,
    Zypper,
    Pacman,
}

impl PackageManager {
    fn package_name() -> &'static str {
        package_name(ChannelState::channel())
    }

    fn detect() -> Result<Self> {
        let package_name = Self::package_name();

        let detect_script = r#"
            command -p pacman -Qi $PACKAGE_NAME >/dev/null 2>/dev/null
            if [ $? -eq 0 ]; then
              echo "pacman"
              exit
            fi

            command -p zypper search --match-exact --installed-only $PACKAGE_NAME >/dev/null 2>/dev/null
            if [ $? -eq 0 ]; then
              echo "zypper"
              exit
            fi

            command -p dnf list --installed $PACKAGE_NAME >/dev/null 2>/dev/null
            if [ $? -eq 0 ]; then
              echo "dnf"
              exit
            fi

            command -p yum list installed $PACKAGE_NAME >/dev/null 2>/dev/null
            if [ $? -eq 0 ]; then
              echo "yum"
              exit
            fi

            if [ "$(command -p dpkg-query --show --showformat='${db:Status-Status}' $PACKAGE_NAME 2>/dev/null)" = "installed" ]; then
              echo "apt"
              exit
            fi

            exit 1
        "#;

        let output = command::blocking::Command::new("sh")
            .args(["-c", detect_script])
            .env("PACKAGE_NAME", package_name)
            .output();
        match output {
            Ok(output) => {
                if !output.status.success() {
                    bail!("Failed to determine which package manager was used to install warp");
                }
                let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
                    bail!("Could not parse package manager detection script output as UTF-8");
                };
                match stdout.trim() {
                    "pacman" => Ok(Self::Pacman),
                    "zypper" => Ok(Self::Zypper),
                    "dnf" => Ok(Self::Dnf),
                    "yum" => Ok(Self::Yum),
                    "apt" => Ok(Self::Apt),
                    _ => bail!(
                        "Received unexpected output from the package manager detection script"
                    ),
                }
            }
            Err(err) => Err(err).context("Failed to run package manager detection script"),
        }
    }
}

impl std::fmt::Display for PackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageManager::Apt => write!(f, "apt"),
            PackageManager::Yum => write!(f, "yum"),
            PackageManager::Dnf => write!(f, "dnf"),
            PackageManager::Zypper => write!(f, "zypper"),
            PackageManager::Pacman => write!(f, "pacman"),
        }
    }
}

fn package_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => "warp-terminal",
        Channel::Preview => "warp-terminal-preview",
        Channel::Dev => "warp-terminal-dev",
        Channel::Integration => "warp-terminal-integration",
        Channel::Local => "warp-terminal-local",
        Channel::Oss => "warp-oss",
    }
}

#[cfg(test)]
#[path = "linux_test.rs"]
mod tests;
