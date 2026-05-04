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

/// Returns the remote directory where the binary is installed, keyed by channel.
///
/// - stable:      `~/.warp/remote-server`
/// - preview:     `~/.warp-preview/remote-server`
/// - dev:         `~/.warp-dev/remote-server`
/// - local:       `~/.warp-local/remote-server`
/// - integration: `~/.warp-dev/remote-server`
/// - warp-oss:    `~/.warp-oss/remote-server`
pub fn remote_server_dir() -> String {
    let warp_dir = match ChannelState::channel() {
        Channel::Stable => ".warp",
        Channel::Preview => ".warp-preview",
        Channel::Dev | Channel::Integration => ".warp-dev",
        Channel::Local => ".warp-local",
        Channel::Oss => {
            // TODO(alokedesai): need to figure out how remote server works with warp-oss
            // For now, return what Dev returns.
            ".warp-dev"
        }
    };
    format!("~/{warp_dir}/remote-server")
}

/// Returns a filesystem-safe directory name for a remote-server identity key.
///
/// The identity key is not secret, but it can contain bytes that are unsafe or
/// ambiguous in paths. Keep ASCII alphanumeric characters plus `-` and `_`;
/// percent-encode all other UTF-8 bytes.
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

/// Returns the identity-scoped remote directory used for the daemon socket
/// and PID file.
pub fn remote_server_daemon_dir(identity_key: &str) -> String {
    format!(
        "{}/{}",
        remote_server_dir(),
        remote_server_identity_dir_name(identity_key)
    )
}

/// Returns the binary name, keyed by channel.
///
/// Matches the CLI command names: `oz` (stable), `oz-preview`, `oz-dev`.
pub fn binary_name() -> &'static str {
    ChannelState::channel().cli_command_name()
}

/// Returns the full remote binary path for the current channel and client
/// version.
///
/// The path-versioning rule is keyed strictly off [`Channel`]:
///
/// - [`Channel::Local`] and [`Channel::Oss`] always use the bare
///   `{binary_name}` path. For `Local` this is the slot
///   `script/deploy_remote_server` writes to; `Oss` is treated the
///   same way because it has no release-pinned CDN artifact and is
///   expected to be deployed/managed locally.
/// - Every other channel always uses `{binary_name}-{version}`, where
///   `version` is the baked-in `GIT_RELEASE_TAG` when present and falls
///   back to `CARGO_PKG_VERSION` otherwise. The fallback keeps the path
///   deterministic for misconfigured `cargo run --bin {dev,preview,...}`
///   builds; the resulting `&version=...` query is expected to 404 against
///   `/download/cli` and surface a clean `SetupFailed` rather than silently
///   writing to a path that doesn't follow the rule.
pub fn remote_server_binary() -> String {
    let dir = remote_server_dir();
    let name = binary_name();
    match ChannelState::channel() {
        Channel::Local | Channel::Oss => format!("{dir}/{name}"),
        Channel::Stable | Channel::Preview | Channel::Dev | Channel::Integration => {
            format!("{dir}/{name}-{}", pinned_version())
        }
    }
}

/// Returns the shell command to check if the remote server binary exists and
/// is executable.
pub fn binary_check_command() -> String {
    format!("test -x {}", remote_server_binary())
}

/// Returns the version string used to pin remote-server installs on
/// channels that take the versioned path (i.e. everything except
/// [`Channel::Local`] and [`Channel::Oss`]). Prefers the baked-in
/// `GIT_RELEASE_TAG` from [`ChannelState::app_version`]; falls back to
/// `CARGO_PKG_VERSION` so the path / install URL is deterministic even on
/// dev `cargo run` builds without a release tag. The `CARGO_PKG_VERSION`
/// fallback is not expected to map to a real `/download/cli` artifact —
/// it exists to produce a clean install-time failure rather than silently
/// fall through to the unversioned (Local/Oss-only) path.
fn pinned_version() -> &'static str {
    ChannelState::app_version().unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// The install script template, loaded from a standalone `.sh` file for
/// readability. Placeholders like `{download_base_url}` are substituted by
/// [`install_script`].
const INSTALL_SCRIPT_TEMPLATE: &str = include_str!("install_remote_server.sh");

/// Returns the install script that downloads and installs the CLI binary
/// at the current client version.
///
/// The script detects the remote architecture via `uname -m`, downloads
/// the correct Oz CLI tarball from the download URL, and installs it at
/// the path returned by [`remote_server_binary`] so repeat invocations
/// are idempotent. The `version_query` / `version_suffix` substitutions
/// follow the same rule as [`remote_server_binary`]: empty on
/// [`Channel::Local`] and [`Channel::Oss`] (so the install lands at
/// the unversioned path used by `script/deploy_remote_server`); pinned to
/// `&version={v}` / `-{v}` on every other channel, where `v` falls back
/// to `CARGO_PKG_VERSION` when no release tag is baked in.
pub fn install_script() -> String {
    let (version_query, version_suffix) = match ChannelState::channel() {
        Channel::Local | Channel::Oss => (String::new(), String::new()),
        Channel::Stable | Channel::Preview | Channel::Dev | Channel::Integration => {
            let v = pinned_version();
            (format!("&version={v}"), format!("-{v}"))
        }
    };
    INSTALL_SCRIPT_TEMPLATE
        .replace("{download_base_url}", &download_url())
        .replace("{channel}", download_channel())
        .replace("{install_dir}", &remote_server_dir())
        .replace("{binary_name}", binary_name())
        .replace("{version_query}", &version_query)
        .replace("{version_suffix}", &version_suffix)
}

/// Construct the download URL from the server root URL.
///
/// For example, given `https://app.warp.dev`, returns
/// `https://app.warp.dev/download/cli`.
fn download_url() -> String {
    let base = ChannelState::server_root_url();
    let base = base.trim_end_matches('/');
    format!("{base}/download/cli")
}

/// Maps the client's [`Channel`] to the server's download channel parameter.
///
/// The server recognises `"stable"`, `"preview"`, and `"dev"`.  Local and
/// Integration builds map to `"dev"` so they fetch dogfood artifacts.
fn download_channel() -> &'static str {
    match ChannelState::channel() {
        Channel::Stable => "stable",
        Channel::Preview => "preview",
        Channel::Dev | Channel::Local | Channel::Integration => "dev",
        Channel::Oss => {
            // TODO(alokedesai): need to figure out how remote server works with warp-oss
            // For now, return what Dev returns.
            "dev"
        }
    }
}

/// Timeout for the binary existence check.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for the install script.
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
