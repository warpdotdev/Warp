use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use flate2::read::GzDecoder;
use futures::AsyncWriteExt as _;
use futures::TryStreamExt as _;
use http_client::StatusCode;
use tempfile::TempDir;
use walkdir::WalkDir;

use remote_server::setup::RemotePlatform;
use remote_server::transport::Error;

const REMOTE_SERVER_TARBALL_CACHE_FILE_NAME: &str = "oz.tar.gz";

const REMOTE_SERVER_TARBALL_DOWNLOAD_ATTEMPTS: usize = 3;
// The local SCP fallback download can run over slow or captive networks. Match
// the install-script timeout so slow client-side downloads have the same budget
// as remote-host downloads.
const REMOTE_SERVER_TARBALL_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);

// Keep retry backoff short because retries only cover transient HTTP failures;
// the longer timeout above handles slow successful downloads.
const REMOTE_SERVER_TARBALL_DOWNLOAD_RETRY_DELAY: Duration = Duration::from_millis(250);

/// Exit codes where SCP fallback would not help because the failure is on the
/// remote host itself, not a network/download issue.
pub(super) fn should_try_install(error: &Error) -> bool {
    !matches!(error, Error::ScriptFailed { exit_code, .. } if *exit_code == 2)
}

/// Installs the remote server via SCP fallback.
///
/// The tarball is downloaded or reused from the local cache first, extracted
/// locally to a temp directory, and the resulting binary is uploaded directly
/// to its final remote path. This bypasses `install_remote_server.sh` on the
/// fallback path entirely, so the remote host does not need `tar`, `gzip`, or
/// extraction-time disk quota for an unpacked tarball.
pub(super) async fn install(socket_path: &Path) -> Result<(), Error> {
    let platform = super::super::detect_remote_platform(socket_path).await?;

    let client_tarball_path = cached_remote_server_tarball(&platform)
        .await
        .map_err(Error::Other)?;
    let timeout = remote_server::setup::SCP_INSTALL_TIMEOUT;
    let install_dir = remote_server::setup::remote_server_dir();
    let remote_binary_path = remote_server::setup::remote_server_binary();
    let remote_staging_path = format!("{install_dir}/.oz-upload-{}", uuid::Uuid::new_v4());

    let extraction = extract_tarball_locally(&client_tarball_path)
        .await
        .map_err(Error::Other)?;
    let local_binary_path = find_extracted_binary(extraction.dir.path()).map_err(Error::Other)?;

    // The remote install dir is normally created by install_remote_server.sh.
    // Direct-upload bypasses that script, so create it explicitly here.
    let mkdir_output = remote_server::ssh::run_ssh_command(
        socket_path,
        &format!("mkdir -p {install_dir}"),
        remote_server::setup::CHECK_TIMEOUT,
    )
    .await
    .map_err(Error::from)?;
    if !mkdir_output.status.success() {
        let code = mkdir_output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&mkdir_output.stderr).to_string();
        return Err(Error::ScriptFailed {
            exit_code: code,
            stderr,
        });
    }

    log::info!("Uploading remote-server binary to remote at {remote_staging_path}");
    remote_server::ssh::scp_upload(
        socket_path,
        &local_binary_path,
        &remote_staging_path,
        timeout,
    )
    .await
    .map_err(Error::Other)?;

    // chmod +x the staged upload and atomically move it into place. Going
    // through a staging path keeps concurrent installs from observing a
    // partial file at `remote_binary_path` if the SCP transfer is interrupted.
    let finalize_cmd =
        format!("chmod +x {remote_staging_path} && mv {remote_staging_path} {remote_binary_path}");
    let finalize = remote_server::ssh::run_ssh_command(
        socket_path,
        &finalize_cmd,
        remote_server::setup::CHECK_TIMEOUT,
    )
    .await
    .map_err(Error::from)?;
    if !finalize.status.success() {
        let code = finalize.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&finalize.stderr).to_string();
        // Best-effort cleanup of the staged upload so a failed install does
        // not leave dead files in the install dir. Ignore failures: surface
        // the finalize error instead.
        let _ = remote_server::ssh::run_ssh_command(
            socket_path,
            &format!("rm -f {remote_staging_path}"),
            remote_server::setup::CHECK_TIMEOUT,
        )
        .await;
        return Err(Error::ScriptFailed {
            exit_code: code,
            stderr,
        });
    }

    Ok(())
}

/// Owns the local temp directory used for tarball extraction so it lives long
/// enough for the binary to be uploaded before getting cleaned up.
struct LocalExtraction {
    dir: TempDir,
}

/// Extracts a cached tarball into a fresh local temp directory.
///
/// Tar / gzip decoding is offloaded to a blocking task because both APIs are
/// synchronous and can take noticeable time on slow disks or large archives.
async fn extract_tarball_locally(tarball_path: &Path) -> anyhow::Result<LocalExtraction> {
    let tarball_path = tarball_path.to_owned();
    let dir = tempfile::Builder::new()
        .prefix("oz-remote-server-")
        .tempdir()
        .context("Failed to create temp directory for local tarball extraction")?;
    let extract_to = dir.path().to_owned();

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let file = std::fs::File::open(&tarball_path).with_context(|| {
            format!(
                "Failed to open cached tarball '{}' for local extraction",
                tarball_path.display()
            )
        })?;
        let gz = GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        archive.unpack(&extract_to).with_context(|| {
            format!(
                "Failed to extract tarball '{}' into '{}'",
                tarball_path.display(),
                extract_to.display()
            )
        })?;
        Ok(())
    })
    .await
    .context("Local tarball extraction task panicked")??;

    Ok(LocalExtraction { dir })
}

/// Locates the remote-server binary inside an extracted tarball directory.
///
/// Mirrors the matcher used by `install_remote_server.sh`:
/// `find "$tmpdir" -type f -name 'oz*' ! -name '*.tar.gz'`. The exclusion is
/// widened to any name containing `.tar` so an archive that happens to be
/// shipped inside the tarball (or the tarball itself, if a future caller
/// extracts into the same directory) can never win over the real binary.
fn find_extracted_binary(dir: &Path) -> anyhow::Result<PathBuf> {
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = entry.with_context(|| {
            format!(
                "Failed to traverse extracted tarball directory '{}'",
                dir.display()
            )
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str() else {
            continue;
        };
        if !name.starts_with("oz") || name.contains(".tar") {
            continue;
        }
        return Ok(entry.into_path());
    }
    anyhow::bail!(
        "No remote-server binary found in extracted tarball directory '{}'",
        dir.display()
    );
}

fn remote_server_tarball_cache_root() -> PathBuf {
    warp_core::paths::cache_dir()
        .join("remote-server")
        .join("tarballs")
}

fn remote_server_tarball_cache_temp_dir() -> PathBuf {
    remote_server_tarball_cache_root().join(".tmp")
}

fn current_remote_server_tarball_cache_version() -> &'static str {
    remote_server::setup::remote_server_artifact_version()
}

fn remote_server_tarball_cache_path(platform: &RemotePlatform) -> PathBuf {
    remote_server_tarball_cache_root()
        .join(current_remote_server_tarball_cache_version())
        .join(format!(
            "{}-{}",
            platform.os.as_str(),
            platform.arch.as_str()
        ))
        .join(REMOTE_SERVER_TARBALL_CACHE_FILE_NAME)
}

async fn is_valid_cached_tarball(path: &Path) -> bool {
    async_fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

/// Returns a local tarball for the remote platform.
///
/// Reuses an existing cached tarball when available; otherwise downloads the
/// tarball into the cache and returns the newly cached path.
async fn cached_remote_server_tarball(platform: &RemotePlatform) -> anyhow::Result<PathBuf> {
    let cache_path = remote_server_tarball_cache_path(platform);
    if is_valid_cached_tarball(&cache_path).await {
        log::info!(
            "Using cached remote-server tarball at {}",
            cache_path.display()
        );
        return Ok(cache_path);
    }

    if async_fs::metadata(&cache_path).await.is_ok() {
        let _ = async_fs::remove_file(&cache_path).await;
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
    async_fs::create_dir_all(parent).await.with_context(|| {
        format!(
            "Failed to create remote-server tarball cache directory '{}'",
            parent.display()
        )
    })?;
    let temp_dir = remote_server_tarball_cache_temp_dir();
    async_fs::create_dir_all(&temp_dir).await.with_context(|| {
        format!(
            "Failed to create remote-server tarball cache temp directory '{}'",
            temp_dir.display()
        )
    })?;

    // Download into a unique temp path first so a failed or partial download
    // never appears at the shared cache path that other installs may reuse.
    let temp_path = temp_dir.join(format!(
        ".{REMOTE_SERVER_TARBALL_CACHE_FILE_NAME}.{}.tmp",
        uuid::Uuid::new_v4()
    ));

    if let Err(e) = download_remote_server_tarball_with_retries(url, &temp_path).await {
        let _ = async_fs::remove_file(&temp_path).await;
        return Err(e);
    }
    if !is_valid_cached_tarball(&temp_path).await {
        let _ = async_fs::remove_file(&temp_path).await;
        anyhow::bail!("Downloaded remote-server tarball from {url} was empty");
    }

    if is_valid_cached_tarball(cache_path).await {
        let _ = async_fs::remove_file(&temp_path).await;
        return Ok(());
    }

    // Publish the validated temp file to the shared cache path. If another
    // concurrent fallback populated the cache after the check above, that valid
    // cache hit is good enough for this install, so discard our temp file.
    match async_fs::rename(&temp_path, cache_path).await {
        Ok(()) => Ok(()),
        Err(e) if is_valid_cached_tarball(cache_path).await => {
            let _ = async_fs::remove_file(&temp_path).await;
            Ok(())
        }
        Err(e) => {
            let _ = async_fs::remove_file(&temp_path).await;
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
        match download_remote_server_tarball_internal(&http_client, url, temp_path).await {
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

async fn download_remote_server_tarball_internal(
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

    let mut file = async_fs::File::create(temp_path).await.map_err(|e| {
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

#[cfg(test)]
#[path = "scp_fallback_tests.rs"]
mod tests;
