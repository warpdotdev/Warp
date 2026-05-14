mod glibc;

pub use glibc::{GlibcVersion, RemoteLibc};

use std::time::Duration;

use anyhow::{anyhow, Result};
use warp_core::channel::{Channel, ChannelState};

/// State machine for the remote server install → launch → initialize flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteServerSetupState {
    /// Checking if the binary exists on remote.
    Checking,
    /// Downloading and installing the binary for the first time on this host.
    Installing { progress_percent: Option<u8> },
    /// Replacing an existing install with a differently-versioned binary.
    /// Rendered as "Updating..." in the UI so the user understands this
    /// isn't a fresh install.
    Updating,
    /// Binary is launched, waiting for InitializeResponse.
    Initializing,
    /// Handshake complete. Ready.
    Ready,
    /// Something failed. Fall back to ControlMaster.
    Failed { error: String },
    /// Preinstall check classified the host as incompatible with the
    /// prebuilt remote-server binary. The controller treats this as a
    /// clean fall-back to the legacy ControlMaster-backed SSH flow,
    /// distinct from `Failed` (which is rendered as a real error).
    Unsupported { reason: UnsupportedReason },
}

impl RemoteServerSetupState {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported { .. })
    }

    pub fn is_terminal(&self) -> bool {
        self.is_ready() || self.is_failed() || self.is_unsupported()
    }

    pub fn is_in_progress(&self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Installing { .. } | Self::Updating | Self::Initializing
        )
    }

    pub fn is_connecting(&self) -> bool {
        matches!(
            self,
            Self::Installing { .. } | Self::Updating | Self::Initializing
        )
    }
}

/// Outcome of [`crate::transport::RemoteTransport::run_preinstall_check`].
///
/// The script runs over the existing SSH socket before any install UI
/// surfaces and reports whether the host can run the prebuilt
/// remote-server binary. The Rust side is intentionally a thin parser
/// over the script's structured stdout (see `preinstall_check.sh`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreinstallCheckResult {
    pub status: PreinstallStatus,
    pub libc: RemoteLibc,
    /// Verbatim, trimmed script stdout. Forwarded to telemetry for
    /// diagnosing `Unknown` outcomes on exotic distros.
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreinstallStatus {
    Supported,
    Unsupported {
        reason: UnsupportedReason,
    },
    /// Probe ran but couldn't classify the host. Treated as supported
    /// (fail open) by [`PreinstallCheckResult::is_supported`] so we keep
    /// today's install-and-try behavior on hosts where the probe is
    /// unreliable.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnsupportedReason {
    GlibcTooOld {
        detected: GlibcVersion,
        required: GlibcVersion,
    },
    NonGlibc {
        name: String,
    },
}

impl PreinstallCheckResult {
    /// Whether the host is supported. Both `Supported` and `Unknown`
    /// return true — only positive detection of an incompatible libc
    /// triggers the silent fall-back.
    pub fn is_supported(&self) -> bool {
        match self.status {
            PreinstallStatus::Supported | PreinstallStatus::Unknown => true,
            PreinstallStatus::Unsupported { .. } => false,
        }
    }

    /// Parses the structured `key=value` stdout emitted by
    /// `preinstall_check.sh`. Tolerates unknown keys and lines without
    /// `=` (forward-compatibility): future versions of the script can
    /// add new keys without coordinating a client release.
    pub fn parse(stdout: &str) -> Self {
        let mut status_str: Option<&str> = None;
        let mut reason_str: Option<&str> = None;
        let mut libc_family: Option<&str> = None;
        let mut libc_version: Option<&str> = None;
        let mut required_glibc: Option<&str> = None;

        for line in stdout.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "status" => status_str = Some(value.trim()),
                "reason" => reason_str = Some(value.trim()),
                "libc_family" => libc_family = Some(value.trim()),
                "libc_version" => libc_version = Some(value.trim()),
                "required_glibc" => required_glibc = Some(value.trim()),
                _ => {} // ignore unknown keys
            }
        }

        let libc = glibc::parse_libc(libc_family, libc_version);
        let status = parse_status(status_str, reason_str, &libc, required_glibc);

        Self {
            status,
            libc,
            raw: stdout.trim().to_string(),
        }
    }
}

fn parse_status(
    status: Option<&str>,
    reason: Option<&str>,
    libc: &RemoteLibc,
    required_glibc: Option<&str>,
) -> PreinstallStatus {
    match status {
        Some("supported") => PreinstallStatus::Supported,
        Some("unsupported") => match reason {
            Some("glibc_too_old") => {
                let detected = match libc {
                    RemoteLibc::Glibc(v) => Some(*v),
                    _ => None,
                };
                let required = required_glibc.and_then(GlibcVersion::parse);
                match (detected, required) {
                    (Some(detected), Some(required)) => PreinstallStatus::Unsupported {
                        reason: UnsupportedReason::GlibcTooOld { detected, required },
                    },
                    // The script said `unsupported` + `glibc_too_old` but we
                    // can't recover the numbers — fail open rather than
                    // surface a malformed reason.
                    _ => PreinstallStatus::Unknown,
                }
            }
            Some("non_glibc") => {
                let name = match libc {
                    RemoteLibc::NonGlibc { name } => name.clone(),
                    _ => "unknown".to_string(),
                };
                PreinstallStatus::Unsupported {
                    reason: UnsupportedReason::NonGlibc { name },
                }
            }
            _ => PreinstallStatus::Unknown,
        },
        // status=unknown, missing, or anything else → fail open.
        _ => PreinstallStatus::Unknown,
    }
}

/// The bundled preinstall check script. Loaded as a string so the SSH
/// transport can pipe it through the existing ControlMaster socket via
/// [`crate::ssh::run_ssh_script`].
///
/// The script is intentionally self-contained — the supported-glibc
/// floor is hardcoded inside the script (see `preinstall_check.sh`)
/// rather than templated from Rust.
pub const PREINSTALL_CHECK_SCRIPT: &str = include_str!("preinstall_check.sh");

/// Detected remote platform from `uname -sm` output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotePlatform {
    pub os: RemoteOs,
    pub arch: RemoteArch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteOs {
    Linux,
    MacOs,
}

impl RemoteOs {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::MacOs => "macos",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteArch {
    X86_64,
    Aarch64,
}

impl RemoteArch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }
}

/// Parse `uname -sm` output into a `RemotePlatform`.
///
/// The expected format is `<os> <arch>`, e.g. `Linux x86_64` or `Darwin arm64`.
/// Takes the last line to skip any shell initialization output.
pub fn parse_uname_output(output: &str) -> Result<RemotePlatform> {
    let line = output
        .lines()
        .last()
        .ok_or_else(|| anyhow!("empty uname output"))?
        .trim();

    let mut parts = line.split_whitespace();
    let os_str = parts
        .next()
        .ok_or_else(|| anyhow!("missing OS in uname output: {line}"))?;
    let arch_str = parts
        .next()
        .ok_or_else(|| anyhow!("missing arch in uname output: {line}"))?;

    let os = match os_str {
        "Linux" => RemoteOs::Linux,
        "Darwin" => RemoteOs::MacOs,
        other => return Err(anyhow!("unsupported OS: {other}")),
    };

    let arch = match arch_str {
        "x86_64" => RemoteArch::X86_64,
        "aarch64" | "arm64" | "armv8l" => RemoteArch::Aarch64,
        other => return Err(anyhow!("unsupported arch: {other}")),
    };

    Ok(RemotePlatform { os, arch })
}

/// 返回远端二进制安装目录,按 channel 隔离。
///
/// - stable:      `~/.warp/remote-server`
/// - preview:     `~/.warp-preview/remote-server`
/// - dev:         `~/.warp-dev/remote-server`
/// - local:       `~/.warp-local/remote-server`
/// - integration: `~/.warp-dev/remote-server`
/// - warp-oss:    `~/.openwarp/remote-server`
pub fn remote_server_dir() -> String {
    let warp_dir = match ChannelState::channel() {
        Channel::Stable => ".warp",
        Channel::Preview => ".warp-preview",
        Channel::Dev | Channel::Integration => ".warp-dev",
        Channel::Local => ".warp-local",
        Channel::Oss => ".openwarp",
    };
    format!("~/{warp_dir}/remote-server")
}

/// 返回可安全放入路径的 remote-server identity key 目录名。
///
/// identity key 不是密钥,但可能包含路径中不安全或有歧义的字节。
/// 保留 ASCII 字母数字以及 `-` / `_`,其他 UTF-8 字节做百分号编码。
pub fn remote_server_identity_dir_name(identity_key: &str) -> String {
    if identity_key.is_empty() {
        return "empty".to_string();
    }

    let mut encoded = String::with_capacity(identity_key.len());
    for byte in identity_key.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// 返回按 identity 隔离的远端目录,用于 daemon socket 和 PID 文件。
pub fn remote_server_daemon_dir(identity_key: &str) -> String {
    format!(
        "{}/{}",
        remote_server_dir(),
        remote_server_identity_dir_name(identity_key)
    )
}

/// 返回远端 remote-server 二进制文件名。
pub fn binary_name() -> &'static str {
    ChannelState::channel().cli_command_name()
}

/// 返回当前 channel 和客户端版本对应的远端二进制完整路径。
///
/// Local 构建保留无版本后缀路径,以便 `script/deploy_remote_server`
/// 覆盖同一个开发 slot。OpenWarp release 构建带 `GIT_RELEASE_TAG`
/// 时使用版本后缀,这样新版本会自然触发重新安装;源码本地构建没有
/// release tag,仍使用无后缀路径。
pub fn remote_server_binary() -> String {
    let dir = remote_server_dir();
    let name = binary_name();
    match ChannelState::channel() {
        Channel::Local => format!("{dir}/{name}"),
        Channel::Oss if ChannelState::app_version().is_none() => format!("{dir}/{name}"),
        Channel::Oss => format!("{dir}/{name}-{}", pinned_version()),
        Channel::Stable | Channel::Preview | Channel::Dev | Channel::Integration => {
            format!("{dir}/{name}-{}", pinned_version())
        }
    }
}

/// 返回检查远端 remote-server 二进制存在且可执行的 shell 命令。
///
/// 与上游一致,这里实际运行 `--version`,而不只是 `test -x`;
/// 这样可以把损坏或无法解析参数的二进制提前识别出来。
pub fn binary_check_command() -> String {
    format!("{} --version", remote_server_binary())
}

/// 返回用于版本化安装路径的版本号。优先使用编译时注入的
/// `GIT_RELEASE_TAG`;没有 release tag 时回退到 `CARGO_PKG_VERSION`,
/// 让需要版本化路径的 channel 保持确定性,并在缺少对应 release 资产时
/// 清晰失败,而不是误用无版本路径。
fn pinned_version() -> &'static str {
    ChannelState::app_version().unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// 安装脚本模板独立放在 `.sh` 文件里方便维护。
/// `{download_base_url}` 等占位符由 [`install_script`] 替换。
const INSTALL_SCRIPT_TEMPLATE: &str = include_str!("install_remote_server.sh");

/// 返回安装脚本。`staging_tarball_path` 非空时,脚本跳过远端下载,
/// 改为解压客户端通过 SCP 预上传的 tarball。
pub fn install_script(staging_tarball_path: Option<&str>) -> String {
    let version_suffix = version_suffix();
    INSTALL_SCRIPT_TEMPLATE
        .replace("{download_base_url}", &download_url())
        .replace("{install_dir}", &remote_server_dir())
        .replace("{binary_name}", binary_name())
        .replace("{version_suffix}", &version_suffix)
        .replace("{staging_tarball_path}", staging_tarball_path.unwrap_or(""))
}

/// 构造 OpenWarp CLI release 资产下载基址。
fn download_url() -> String {
    let release_path = match ChannelState::app_version() {
        Some(tag) => format!("download/{tag}"),
        None => "latest/download".to_string(),
    };
    format!("https://github.com/zerx-lab/warp/releases/{release_path}")
}

fn version_suffix() -> String {
    match ChannelState::channel() {
        Channel::Local => String::new(),
        Channel::Oss if ChannelState::app_version().is_none() => String::new(),
        Channel::Oss | Channel::Stable | Channel::Preview | Channel::Dev | Channel::Integration => {
            format!("-{}", pinned_version())
        }
    }
}

/// 返回指定远端平台对应的 OpenWarp CLI tarball URL。
pub fn download_tarball_url(platform: &RemotePlatform) -> String {
    format!(
        "{}/openwarp-{}-{}.tar.gz",
        download_url(),
        platform.os.as_str(),
        platform.arch.as_str(),
    )
}

/// 检查二进制是否存在的超时。
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// 常规远端安装脚本超时。
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(60);

/// SCP fallback 包含本地下载、上传和远端解压,给它更宽松的超时。
pub const SCP_INSTALL_TIMEOUT: Duration = Duration::from_secs(120);

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
