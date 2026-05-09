use std::{env, fs::read_to_string, sync::Arc};

use anyhow::{Context as _, Result};
use channel_versions::ChannelVersions;

use crate::{
    channel::{Channel, ChannelState},
    report_error,
    server::server_api::{ServerApi, FETCH_CHANNEL_VERSIONS_TIMEOUT},
};

// Fetches channel versions asynchronously from the Warp server. If the Warp server request fails,
// then fetches from GCP JSON storage as a fallback.
pub async fn fetch_channel_versions(
    nonce: &str,
    server_api: Arc<ServerApi>,
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

    if should_fetch_channel_versions_from_manifest_directly() {
        log::info!(
            "Bypassing Warp server for channel versions on channel {}; fetching manifest directly",
            ChannelState::channel()
        );
        return fetch_channel_versions_from_json_storage(server_api.http_client(), nonce).await;
    }

    let channel_versions = server_api
        .fetch_channel_versions(include_changelogs, is_daily)
        .await
        .context("Failed to retrieve channel versions from Warp server");
    match channel_versions {
        channel_versions @ Ok(_) => channel_versions,
        Err(err) => {
            match ChannelState::channel() {
                // Only log an error on Dev and Preview -- if this is failing, its likely to be
                // failing for all users, and Stable has too many users (this error would flood
                // our Sentry logs).
                Channel::Dev | Channel::Preview => report_error!(err),
                _ => log::warn!(
                    "Failed to retrieve channel versions from Warp server, falling \
                back to GCP JSON storage."
                ),
            }
            fetch_channel_versions_from_json_storage(server_api.http_client(), nonce).await
        }
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
    let manifest_url = channel_versions_manifest_url(nonce);
    let res = client
        .get(manifest_url.as_str())
        .timeout(FETCH_CHANNEL_VERSIONS_TIMEOUT)
        .send()
        .await?;
    let versions: ChannelVersions = res.json().await?;
    log::info!("Received channel versions from GCP JSON storage: {versions}");
    Ok(versions)
}

/// Returns the manifest URL used to fetch channel version metadata.
fn channel_versions_manifest_url(nonce: &str) -> String {
    if let Some(url) = ChannelState::channel_versions_url() {
        format!("{url}?r={nonce}")
    } else {
        format!(
            "{}/channel_versions.json?r={nonce}",
            ChannelState::releases_base_url()
        )
    }
}

/// Returns whether the current channel should skip Warp's `/client_version` API entirely.
fn should_fetch_channel_versions_from_manifest_directly() -> bool {
    ChannelState::channel_versions_url().is_some()
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::should_fetch_channel_versions_from_manifest_directly;
    use crate::channel::{Channel, ChannelState};
    use warp_core::{
        channel::{AutoupdateConfig, ChannelConfig, OzConfig, WarpServerConfig},
        AppId,
    };

    /// Configures a temporary OSS channel state with or without a direct manifest URL.
    fn set_test_channel_state(channel_versions_url: Option<&str>) {
        ChannelState::set(ChannelState::new(
            Channel::Oss,
            ChannelConfig {
                app_id: AppId::new("dev", "warp", "WarpOss"),
                logfile_name: "warp-oss.log".into(),
                server_config: WarpServerConfig::production(),
                oz_config: OzConfig::production(),
                telemetry_config: None,
                crash_reporting_config: None,
                autoupdate_config: Some(AutoupdateConfig {
                    releases_base_url: "https://github.com/example/warp/releases".into(),
                    channel_versions_url: channel_versions_url.map(|url| url.to_string().into()),
                    show_autoupdate_menu_items: true,
                }),
                mcp_static_config: None,
            },
        ));
    }

    #[test]
    #[serial]
    fn direct_manifest_fetch_is_enabled_when_channel_versions_url_exists() {
        set_test_channel_state(Some(
            "https://github.com/example/warp/releases/latest/download/channel_versions.json",
        ));

        assert!(should_fetch_channel_versions_from_manifest_directly());
    }

    #[test]
    #[serial]
    fn direct_manifest_fetch_is_disabled_without_channel_versions_url() {
        set_test_channel_state(None);

        assert!(!should_fetch_channel_versions_from_manifest_directly());
    }
}
