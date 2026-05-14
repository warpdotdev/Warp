#[path = "installation/cleanup.rs"]
pub(crate) mod cleanup;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result};
use futures::TryStreamExt as _;
use http_client::StatusCode;
use tokio::io::AsyncWriteExt as _;

use remote_server::setup::RemotePlatform;
use remote_server::ssh::SshCommandError;
use remote_server::transport::{Error, InstallOutcome, InstallSource};

pub(super) const REMOTE_SERVER_TARBALL_CACHE_FILE_NAME: &str = "oz.tar.gz";
const REMOTE_SERVER_TARBALL_CACHE_VERSION_UNPINNED: &str = "unversioned";
const REMOTE_SERVER_TARBALL_DOWNLOAD_ATTEMPTS: usize = 3;
const REMOTE_SERVER_TARBALL_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);
const REMOTE_SERVER_TARBALL_DOWNLOAD_RETRY_DELAY: Duration = Duration::from_millis(250);

pub(super) fn cache_component(raw: &str) -> String {
    if raw.is_empty() {
        return "empty".to_string();
    }

    let mut encoded = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

pub(super) fn remote_server_tarball_cache_root() -> PathBuf {
    warp_core::paths::cache_dir()
        .join("remote-server")
        .join("tarballs")
}

pub(super) fn current_remote_server_tarball_cache_version() -> &'static str {
    remote_server::setup::remote_server_artifact_version()
        .unwrap_or(REMOTE_SERVER_TARBALL_CACHE_VERSION_UNPINNED)
}

fn remote_server_tarball_cache_path(platform: &RemotePlatform) -> PathBuf {
    remote_server_tarball_cache_root()
        .join(cache_component(
            remote_server::setup::remote_server_download_channel(),
        ))
        .join(cache_component(
            current_remote_server_tarball_cache_version(),
        ))
        .join(cache_component(&format!(
            "{}-{}",
            platform.os.as_str(),
            platform.arch.as_str()
        )))
        .join(REMOTE_SERVER_TARBALL_CACHE_FILE_NAME)
}

async fn is_valid_cached_tarball(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

async fn cached_remote_server_tarball(platform: &RemotePlatform) -> anyhow::Result<PathBuf> {
    let cache_path = remote_server_tarball_cache_path(platform);
    if is_valid_cached_tarball(&cache_path).await {
        log::info!(
            "Using cached remote-server tarball at {}",
            cache_path.display()
        );
        return Ok(cache_path);
    }

    if tokio::fs::metadata(&cache_path).await.is_ok() {
        let _ = tokio::fs::remove_file(&cache_path).await;
    }

    let url = remote_server::setup::download_tarball_url(platform);
    log::info!(
        "Downloading remote-server tarball from {url} into cache at {}",
        cache_path.display()
    );
    download_remote_server_tarball_to_cache(&url, &cache_path).await?;
    Ok(cache_path)
}

async fn download_remote_server_tarball_to_cache(
    url: &str,
    cache_path: &Path,
) -> anyhow::Result<()> {
    let parent = cache_path
        .parent()
        .context("remote-server tarball cache path has no parent directory")?;
    tokio::fs::create_dir_all(parent).await.with_context(|| {
        format!(
            "Failed to create remote-server tarball cache directory '{}'",
            parent.display()
        )
    })?;

    let temp_path = parent.join(format!(
        ".{REMOTE_SERVER_TARBALL_CACHE_FILE_NAME}.{}.tmp",
        uuid::Uuid::new_v4()
    ));

    if let Err(e) = download_remote_server_tarball_with_retries(url, &temp_path).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(e);
    }
    if !is_valid_cached_tarball(&temp_path).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        anyhow::bail!("Downloaded remote-server tarball from {url} was empty");
    }

    if is_valid_cached_tarball(cache_path).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Ok(());
    }

    match tokio::fs::rename(&temp_path, cache_path).await {
        Ok(()) => Ok(()),
        Err(_e) if is_valid_cached_tarball(cache_path).await => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            Ok(())
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            Err(e).with_context(|| {
                format!(
                    "Failed to move remote-server tarball into cache at '{}'",
                    cache_path.display()
                )
            })
        }
    }
}

async fn download_remote_server_tarball_with_retries(
    url: &str,
    temp_path: &Path,
) -> anyhow::Result<()> {
    let http_client = http_client::Client::new();
    let mut last_retryable_error = None;

    for attempt in 1..=REMOTE_SERVER_TARBALL_DOWNLOAD_ATTEMPTS {
        match download_remote_server_tarball_once(&http_client, url, temp_path).await {
            Ok(()) => return Ok(()),
            Err(DownloadAttemptError::Permanent(e)) => return Err(e),
            Err(DownloadAttemptError::Retryable(e)) => {
                last_retryable_error = Some(e);
                if attempt < REMOTE_SERVER_TARBALL_DOWNLOAD_ATTEMPTS {
                    log::warn!("Remote-server tarball download attempt {attempt} failed; retrying");
                    tokio::time::sleep(REMOTE_SERVER_TARBALL_DOWNLOAD_RETRY_DELAY).await;
                }
            }
        }
    }

    Err(last_retryable_error.unwrap_or_else(|| {
        anyhow::anyhow!("Remote-server tarball download failed without an error")
    }))
}

enum DownloadAttemptError {
    Retryable(anyhow::Error),
    Permanent(anyhow::Error),
}

async fn download_remote_server_tarball_once(
    http_client: &http_client::Client,
    url: &str,
    temp_path: &Path,
) -> Result<(), DownloadAttemptError> {
    let response = http_client
        .get(url)
        .timeout(REMOTE_SERVER_TARBALL_DOWNLOAD_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            DownloadAttemptError::Retryable(anyhow::anyhow!(
                "Failed to download remote-server tarball from {url}: {e}"
            ))
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let error =
            anyhow::anyhow!("Remote-server tarball download failed with status {status}: {body}");
        return if is_retryable_download_status(status) {
            Err(DownloadAttemptError::Retryable(error))
        } else {
            Err(DownloadAttemptError::Permanent(error))
        };
    }

    let mut file = tokio::fs::File::create(temp_path).await.map_err(|e| {
        DownloadAttemptError::Permanent(anyhow::anyhow!(
            "Failed to create remote-server tarball cache file '{}': {e}",
            temp_path.display()
        ))
    })?;
    let mut bytes_stream = response.bytes_stream();
    while let Some(chunk) = bytes_stream.try_next().await.map_err(|e| {
        DownloadAttemptError::Retryable(anyhow::anyhow!(
            "Failed to read remote-server tarball response body from {url}: {e}"
        ))
    })? {
        file.write_all(&chunk).await.map_err(|e| {
            DownloadAttemptError::Permanent(anyhow::anyhow!(
                "Failed to write remote-server tarball cache file '{}': {e}",
                temp_path.display()
            ))
        })?;
    }
    file.sync_data().await.map_err(|e| {
        DownloadAttemptError::Permanent(anyhow::anyhow!(
            "Failed to sync remote-server tarball cache file '{}': {e}",
            temp_path.display()
        ))
    })?;

    Ok(())
}

fn is_retryable_download_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS
    ) || status.is_server_error()
}

/// Runs the binary install sequence for the SSH transport. It first asks the
/// remote host to download directly, then falls back to uploading a cached
/// client-side tarball over SCP when the remote download path fails.
pub(super) async fn install_binary(socket_path: &Path) -> InstallOutcome {
    let binary_path = remote_server::setup::remote_server_binary();
    log::info!("Installing remote server binary to {binary_path}");
    let mut outcome = match install_on_server(socket_path).await {
        Ok(()) => InstallOutcome {
            source: Some(InstallSource::Server),
            result: Ok(()),
        },
        Err(server_err) => {
            let should_try_scp = !should_skip_scp_fallback(&server_err);

            if should_try_scp {
                log::info!("Remote server has no curl/wget, falling back to SCP upload");
                match scp_install_fallback(socket_path).await {
                    Ok(()) => InstallOutcome {
                        source: Some(InstallSource::Client),
                        result: Ok(()),
                    },
                    Err(e) => InstallOutcome {
                        source: Some(InstallSource::Client),
                        result: Err(Error::Other(e)),
                    },
                }
            } else {
                InstallOutcome {
                    source: Some(InstallSource::Server),
                    result: Err(server_err),
                }
            }
        }
    };

    // Post-install verification: confirm the binary actually landed at the
    // expected path and is functional. This catches silent install failures
    // that would otherwise surface as a cryptic IPC handshake error.
    if outcome.result.is_ok() {
        log::info!("Running post-install verification for {binary_path}");
        let check_cmd = remote_server::setup::binary_check_command();
        let verify = remote_server::ssh::run_ssh_command(
            socket_path,
            &check_cmd,
            remote_server::setup::CHECK_TIMEOUT,
        )
        .await;
        match verify {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                outcome.result = Err(Error::Other(anyhow::anyhow!(
                    "Post-install verification failed: binary not found or not \
                     executable at {binary_path} (exit {code}): {stderr}"
                )));
            }
            Err(e) => {
                outcome.result = Err(Error::Other(anyhow::anyhow!(
                    "Post-install verification failed: {e}"
                )));
            }
        }
    }

    outcome
}

/// Exit codes where SCP fallback would not help because the failure is on the
/// remote host itself, not a network/download issue.
fn should_skip_scp_fallback(error: &Error) -> bool {
    match error {
        Error::UnsupportedOs { .. } | Error::UnsupportedArch { .. } => true,
        Error::ScriptFailed { exit_code, .. } => *exit_code == 2,
        _ => false,
    }
}

fn classify_install_script_error(exit_code: i32, stderr: String) -> Error {
    if exit_code == 2 {
        for line in stderr.lines().map(str::trim) {
            if let Some(arch) = line.strip_prefix("unsupported arch:") {
                return Error::UnsupportedArch {
                    arch: arch.trim().to_string(),
                };
            }
            if let Some(os) = line.strip_prefix("unsupported OS:") {
                return Error::UnsupportedOs {
                    os: os.trim().to_string(),
                };
            }
        }
    }

    Error::ScriptFailed { exit_code, stderr }
}

/// Runs the install script on the remote host to download and install the
/// binary directly from the CDN.
async fn install_on_server(socket_path: &Path) -> Result<(), Error> {
    let script = remote_server::setup::install_script(None);
    match remote_server::ssh::run_ssh_script(
        socket_path,
        &script,
        remote_server::setup::INSTALL_TIMEOUT,
    )
    .await
    {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(classify_install_script_error(exit_code, stderr))
        }
        Err(SshCommandError::TimedOut { .. }) => Err(Error::TimedOut),
        Err(e) => Err(Error::Other(e.into())),
    }
}

/// SCP install fallback: downloads the tarball locally (reusing the local
/// cache when possible), uploads it to the remote via SCP, then re-invokes the
/// install script with the staging path baked in so the shared extraction tail
/// runs.
async fn scp_install_fallback(socket_path: &Path) -> anyhow::Result<()> {
    let platform = super::detect_remote_platform(socket_path)
        .await
        .map_err(|e| anyhow::anyhow!("SCP fallback: {e:#}"))?;

    let client_tarball_path = cached_remote_server_tarball(&platform).await?;
    let remote_tarball_path = format!(
        "{}/oz-upload.tar.gz",
        remote_server::setup::remote_server_dir()
    );
    let timeout = remote_server::setup::SCP_INSTALL_TIMEOUT;

    log::info!("Uploading tarball to remote at {remote_tarball_path}");
    remote_server::ssh::scp_upload(
        socket_path,
        &client_tarball_path,
        &remote_tarball_path,
        timeout,
    )
    .await?;

    log::info!("Running extraction via install script with tarball at {remote_tarball_path}");
    let script = remote_server::setup::install_script(Some(&remote_tarball_path));

    let output = remote_server::ssh::run_ssh_script(socket_path, &script, timeout).await?;
    if output.status.success() {
        Ok(())
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "Extraction script failed (exit {code}): {stderr}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_unsupported_arch_script_failure() {
        let error = classify_install_script_error(2, "unsupported arch: armv7l\n".to_string());
        assert!(matches!(
            error,
            Error::UnsupportedArch { arch } if arch == "armv7l"
        ));
    }

    #[test]
    fn classifies_unsupported_os_script_failure() {
        let error =
            classify_install_script_error(2, "unsupported OS: CYGWIN_NT-10.0-22621\n".to_string());
        assert!(matches!(
            error,
            Error::UnsupportedOs { os } if os == "CYGWIN_NT-10.0-22621"
        ));
    }

    #[test]
    fn leaves_unrecognized_exit_two_as_script_failure() {
        let error = classify_install_script_error(2, "some other failure\n".to_string());
        assert!(matches!(
            error,
            Error::ScriptFailed { exit_code: 2, stderr } if stderr == "some other failure\n"
        ));
    }

    #[test]
    fn skips_scp_fallback_for_structured_unsupported_platform_errors() {
        assert!(should_skip_scp_fallback(&Error::UnsupportedArch {
            arch: "armv7l".into(),
        }));
        assert!(should_skip_scp_fallback(&Error::UnsupportedOs {
            os: "CYGWIN_NT-10.0-22621".into(),
        }));
    }
}
