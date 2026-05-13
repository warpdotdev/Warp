use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::AppId;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChannelConfig {
    /// The application ID for this channel.
    pub app_id: AppId,

    /// The name of the file to which logs should be written.
    pub logfile_name: Cow<'static, str>,

    /// Configuration for autoupdate functionality.
    pub autoupdate_config: Option<AutoupdateConfig>,
    /// Configuration for crash reporting.
    pub crash_reporting_config: Option<CrashReportingConfig>,
    /// Configuration for statically-bundled MCP OAuth credentials.
    pub mcp_static_config: Option<McpStaticConfig>,
}

pub(crate) const DISABLED_HTTP_SENTINEL: &str = "http://192.0.2.0:9";

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
    fn disabled_http_sentinel_matches_legacy_literal() {
        assert_eq!(DISABLED_HTTP_SENTINEL, "http://192.0.2.0:9");
    }
}
