use std::{env, fs::read_to_string};

use anyhow::{Context as _, Result};
use channel_versions::{ChannelChangelogs, ChannelVersion, ChannelVersions, VersionInfo};

use crate::channel::ChannelState;

// 只从本地状态加载通道版本。OpenWarp 不再向 Warp 或 GCP 请求 release-channel 元数据。
pub async fn fetch_channel_versions(
    nonce: &str,
    client: &http_client::Client,
    include_changelogs: bool,
    is_daily: bool,
) -> Result<ChannelVersions> {
    if let Ok(path) = env::var("WARP_CHANNEL_VERSIONS_PATH") {
        // Load channel versions from local filesystem. Used for testing both
        // autoupdate and changelog behavior.
        let path = shellexpand::tilde(&path);
        let channel_versions_string = read_to_string::<&str>(&path)?;
        return serde_json::from_str(channel_versions_string.as_str())
            .context("Failed to parse channel versions JSON");
    }

    let _ = (nonce, client, is_daily);
    Ok(local_channel_versions(include_changelogs))
}

fn local_channel_versions(include_changelogs: bool) -> ChannelVersions {
    let version = ChannelState::app_version()
        .unwrap_or("v0.local.testing.string_00")
        .to_string();
    let channel_version = ChannelVersion::new(VersionInfo::new(version));
    let changelogs = include_changelogs.then(|| ChannelChangelogs {
        dev: std::collections::HashMap::new(),
        preview: std::collections::HashMap::new(),
        stable: std::collections::HashMap::new(),
    });
    ChannelVersions {
        dev: channel_version.clone(),
        preview: channel_version.clone(),
        stable: channel_version,
        changelogs,
    }
}
