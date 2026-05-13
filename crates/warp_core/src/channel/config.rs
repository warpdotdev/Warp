use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::AppId;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChannelConfig {
    /// The application ID for this channel.
    pub app_id: AppId,

    /// The name of the file to which logs should be written.
    pub logfile_name: Cow<'static, str>,

    /// Configuration for talking to Warp's servers.
    pub server_config: WarpServerConfig,
    /// Configuration for Oz/ambient agents.
    pub oz_config: OzConfig,
    /// Configuration for telemetry sending, or [`None`] if telemetry should be
    /// disabled for this build.
    pub telemetry_config: Option<TelemetryConfig>,
    /// Configuration for autoupdate functionality.
    pub autoupdate_config: Option<AutoupdateConfig>,
    /// Configuration for crash reporting.
    pub crash_reporting_config: Option<CrashReportingConfig>,
    /// Configuration for statically-bundled MCP OAuth credentials.
    pub mcp_static_config: Option<McpStaticConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WarpServerConfig {
    /// The root URL for the standard server pool.
    pub server_root_url: Cow<'static, str>,
    /// The URL for the RTC server, which serves real-time updates for Warp Drive objects.
    pub rtc_server_url: Cow<'static, str>,
}

impl WarpServerConfig {
    pub fn production() -> Self {
        Self::disabled()
    }

    pub fn disabled() -> Self {
        Self {
            server_root_url: DISABLED_HTTP_SENTINEL.into(),
            rtc_server_url: DISABLED_WS_SENTINEL.into(),
        }
    }

    /// Returns true when this config is the openWarp disabled stub (no real
    /// cloud endpoints). Phase 0 of the cloud-removal plan uses this as the
    /// canonical guard so subsequent phases can short-circuit cloud init
    /// without spreading hard-coded IP checks across the codebase.
    pub fn is_disabled(&self) -> bool {
        self.server_root_url == DISABLED_HTTP_SENTINEL
    }
}

/// RFC 5737 TEST-NET-1 sentinel used to mark openWarp's no-op cloud config.
/// Hard-coded in [`WarpServerConfig::disabled`] / [`OzConfig::disabled`];
/// matched by [`WarpServerConfig::is_disabled`] / [`OzConfig::is_disabled`].
pub(crate) const DISABLED_HTTP_SENTINEL: &str = "http://192.0.2.0:9";
pub(crate) const DISABLED_WS_SENTINEL: &str = "ws://192.0.2.0:9";

#[derive(Debug, Deserialize, Serialize)]
pub struct OzConfig {
    /// Root URL for the Oz (ambient agent management) dashboard.
    pub oz_root_url: Cow<'static, str>,

    /// URL to use as the audience when issuing workload identity tokens. If [`None`], falls back
    /// to [`WarpServerConfig::server_root_url`]. This exists so the audience is not overridden
    /// when a custom server root URL is provided (e.g. an ngrok URL for local development).
    pub workload_audience_url: Option<Cow<'static, str>>,
}

impl OzConfig {
    pub fn production() -> Self {
        Self::disabled()
    }

    pub fn disabled() -> Self {
        Self {
            oz_root_url: DISABLED_HTTP_SENTINEL.into(),
            workload_audience_url: Some(DISABLED_HTTP_SENTINEL.into()),
        }
    }

    pub fn is_disabled(&self) -> bool {
        self.oz_root_url == DISABLED_HTTP_SENTINEL
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TelemetryConfig {
    /// The name of the file in which not-yet-sent telemetry events will be stored.
    pub telemetry_file_name: Cow<'static, str>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AutoupdateConfig {
    /// The base URL for fetching autoupdate versions and updated release bundles.
    pub releases_base_url: Cow<'static, str>,
    /// Whether or not to display menu items relating to autoupdate.
    pub show_autoupdate_menu_items: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CrashReportingConfig {
    /// The URL/DSN for sending error logs and crash reports to Sentry.
    pub sentry_url: Cow<'static, str>,
}

/// Configuration for statically-bundled MCP OAuth credentials.
///
/// These are credentials for OAuth providers where dynamic client registration
/// is not supported and we instead ship pre-registered client IDs and secrets.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct McpStaticConfig {
    /// Per-provider OAuth credentials.
    pub providers: Vec<McpOAuthProviderConfig>,
}

/// A single OAuth provider's credentials for MCP authentication.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct McpOAuthProviderConfig {
    /// The issuer URL of the OAuth provider (e.g. `https://github.com/login/oauth`).
    pub issuer: Cow<'static, str>,
    /// The OAuth client ID registered for this channel.
    pub client_id: Cow<'static, str>,
    /// The OAuth client secret registered for this channel.
    pub client_secret: Cow<'static, str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_server_config_disabled_is_disabled() {
        assert!(WarpServerConfig::disabled().is_disabled());
    }

    #[test]
    fn warp_server_config_production_is_disabled_in_openwarp() {
        assert!(WarpServerConfig::production().is_disabled());
    }

    #[test]
    fn oz_config_disabled_is_disabled() {
        assert!(OzConfig::disabled().is_disabled());
    }

    #[test]
    fn oz_config_production_is_disabled_in_openwarp() {
        assert!(OzConfig::production().is_disabled());
    }

    #[test]
    fn disabled_sentinels_match_legacy_literals() {
        // Lock the sentinel strings: any future change here is a breaking
        // change for the cloud-removal short-circuit and must be intentional.
        assert_eq!(DISABLED_HTTP_SENTINEL, "http://192.0.2.0:9");
        assert_eq!(DISABLED_WS_SENTINEL, "ws://192.0.2.0:9");

        let server = WarpServerConfig::disabled();
        assert_eq!(server.server_root_url, "http://192.0.2.0:9");
        assert_eq!(server.rtc_server_url, "ws://192.0.2.0:9");

        let oz = OzConfig::disabled();
        assert_eq!(oz.oz_root_url, "http://192.0.2.0:9");
        assert_eq!(
            oz.workload_audience_url.as_deref(),
            Some("http://192.0.2.0:9")
        );
    }
}
