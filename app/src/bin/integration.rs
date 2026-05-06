use anyhow::Result;
use warp_core::channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig};
use warp_core::AppId;

fn main() -> Result<()> {
    ChannelState::set(ChannelState::new(
        Channel::Integration,
        ChannelConfig {
            app_id: AppId::new(
                "dev",
                "warp",
                if cfg!(target_os = "macos") {
                    "Warp-Integration"
                } else {
                    "WarpIntegration"
                },
            ),
            logfile_name: "warp_integration.log".into(),
            server_config: WarpServerConfig {
                firebase_auth_api_key: "".into(),
                // Use the reserved .invalid TLD with the TCP discard port to avoid
                // sending integration-test traffic to real services.
                server_root_url: "http://warp.invalid:9".into(),
                rtc_server_url: "ws://warp.invalid:9/graphql/v2".into(),
                session_sharing_server_url: None,
            },
            oz_config: OzConfig {
                // Use the reserved .invalid TLD with the TCP discard port to avoid
                // sending integration-test traffic to real services.
                oz_root_url: "http://warp.invalid:9".into(),
                workload_audience_url: None,
            },
            telemetry_config: None,
            crash_reporting_config: None,
            autoupdate_config: None,
            mcp_static_config: None,
        },
    ));

    warp::run()
}