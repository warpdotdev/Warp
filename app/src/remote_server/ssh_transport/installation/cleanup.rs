use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Context as _;
use warpui::r#async::executor;

use super::{
    cache_component, current_remote_server_tarball_cache_version, remote_server_tarball_cache_root,
};

const STALE_REMOTE_SERVER_TARBALL_TEMP_FILE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub(crate) fn schedule_remote_server_tarball_cache_cleanup(executor: Arc<executor::Background>) {
    executor
        .spawn(async move {
            if let Err(e) = cleanup_remote_server_tarball_cache().await {
                log::warn!("Failed to clean up remote-server tarball cache: {e:#}");
            }
        })
        .detach();
}

async fn cleanup_remote_server_tarball_cache() -> anyhow::Result<()> {
    let current_version = cache_component(current_remote_server_tarball_cache_version());
    cleanup_remote_server_tarball_cache_at(
        &remote_server_tarball_cache_root(),
        &current_version,
        SystemTime::now(),
        STALE_REMOTE_SERVER_TARBALL_TEMP_FILE_AGE,
    )
    .await
}

async fn cleanup_remote_server_tarball_cache_at(
    root: &Path,
    current_version: &str,
    now: SystemTime,
    stale_temp_file_age: Duration,
) -> anyhow::Result<()> {
    let Ok(metadata) = tokio::fs::metadata(root).await else {
        return Ok(());
    };
    if !metadata.is_dir() {
        return Ok(());
    }

    cleanup_stale_remote_server_tarball_temp_files(root, now, stale_temp_file_age).await?;

    let mut channels = tokio::fs::read_dir(root).await.with_context(|| {
        format!(
            "Failed to read remote-server cache root '{}'",
            root.display()
        )
    })?;
    while let Some(channel_entry) = channels.next_entry().await? {
        let channel_path = channel_entry.path();
        if !channel_entry.file_type().await.is_ok_and(|ty| ty.is_dir()) {
            continue;
        }

        let mut versions = match tokio::fs::read_dir(&channel_path).await {
            Ok(versions) => versions,
            Err(e) => {
                log::warn!(
                    "Failed to read remote-server cache channel '{}': {e}",
                    channel_path.display()
                );
                continue;
            }
        };
        while let Some(version_entry) = versions.next_entry().await? {
            let version_path = version_entry.path();
            let version_name = version_entry.file_name();
            let version_name = version_name.to_string_lossy();
            if version_entry.file_type().await.is_ok_and(|ty| ty.is_dir())
                && version_name != current_version
            {
                if let Err(e) = tokio::fs::remove_dir_all(&version_path).await {
                    log::warn!(
                        "Failed to remove stale remote-server cache version '{}': {e}",
                        version_path.display()
                    );
                }
            }
        }
    }

    Ok(())
}

async fn cleanup_stale_remote_server_tarball_temp_files(
    root: &Path,
    now: SystemTime,
    stale_temp_file_age: Duration,
) -> anyhow::Result<()> {
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) => {
                log::warn!(
                    "Failed to read remote-server cache directory '{}': {e}",
                    dir.display()
                );
                continue;
            }
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if file_type.is_dir() {
                dirs.push(path);
                continue;
            }
            if !file_type.is_file()
                || !path
                    .file_name()
                    .is_some_and(|file_name| file_name.to_string_lossy().ends_with(".tmp"))
            {
                continue;
            }

            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            let is_stale = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age >= stale_temp_file_age);
            if is_stale {
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    log::warn!(
                        "Failed to remove stale remote-server temp cache file '{}': {e}",
                        path.display()
                    );
                }
            }
        }
    }

    Ok(())
}
