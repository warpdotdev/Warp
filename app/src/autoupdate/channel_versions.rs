use std::{env, fs::read_to_string};

use anyhow::{Context as _, Result};
use channel_versions::{ChannelChangelogs, ChannelVersion, ChannelVersions, VersionInfo};

use crate::{
    channel::{Channel, ChannelState},
    server::server_api::FETCH_CHANNEL_VERSIONS_TIMEOUT,
};

// Fetches channel versions asynchronously from the Warp server. If the Warp server request fails,
// then fetches from GCP JSON storage as a fallback.
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
    if matches!(
        ChannelState::channel(),
        Channel::Stable | Channel::Preview | Channel::Dev
    ) {
        return Ok(local_channel_versions(include_changelogs));
    }

    fetch_channel_versions_from_json_storage(client, nonce).await
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

// Synchronously fetches updated Warp [`ChannelVersions`] from GCP JSON storage. This will soon
// be deprecated in favor of retrieving updated channel versions from the Warp Server.
// Note, in order to run against a test file you can use the "channel_versions_test.json" file
// and update the file using gsutil cp channel_versions_test.json gs://warp-releases/channel_versions_test.json
async fn fetch_channel_versions_from_json_storage(
    client: &http_client::Client,
    nonce: &str,
) -> Result<ChannelVersions> {
    log::info!("Fetching channel versions from GCP JSON storage");
    let res = client
        .get(
            format!(
                "{}/channel_versions.json?r={}",
                ChannelState::releases_base_url(),
                nonce
            )
            .as_str(),
        )
        .timeout(FETCH_CHANNEL_VERSIONS_TIMEOUT)
        .send()
        .await?;
    let versions: ChannelVersions = res.json().await?;
    log::info!("Received channel versions from GCP JSON storage: {versions}");
    Ok(versions)
}
