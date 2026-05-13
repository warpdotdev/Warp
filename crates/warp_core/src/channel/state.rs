use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::{borrow::Cow, collections::HashSet};
use url::{Origin, Url};

use crate::AppId;
use crate::{
    channel::config::{ChannelConfig, McpOAuthProviderConfig, DISABLED_HTTP_SENTINEL},
    features::FeatureFlag,
};

use super::Channel;

lazy_static! {
    static ref CHANNEL_STATE: Mutex<ChannelState> = Mutex::new(ChannelState::init());
}

#[cfg(feature = "test-util")]
lazy_static! {
    static ref MOCK_SERVER: mockito::ServerGuard = mockito::Server::new();
    static ref MOCK_SERVER_URL: String = MOCK_SERVER.url();
    static ref APP_VERSION: Mutex<Option<&'static str>> = Mutex::new(None);
}

#[derive(Debug)]
pub struct ChannelState {
    channel: Channel,

    /// The set of additional features to enable (on top of default-enabled ones).
    additional_features: HashSet<FeatureFlag>,

    config: ChannelConfig,
}

impl ChannelState {
    pub fn init() -> Self {
        let channel = Channel::Oss;
        let app_id = AppId::new("dev", "openwarp", "OpenWarp");
        Self {
            channel,
            additional_features: Default::default(),
            config: ChannelConfig {
                app_id,
                logfile_name: "".into(),
                autoupdate_config: None,
                mcp_static_config: None,
            },
        }
    }

    pub fn new(channel: Channel, mut config: ChannelConfig) -> Self {
        if let Some(app_id) = app_id_from_bundle() {
            config.app_id = app_id;
        }
        Self {
            channel,
            additional_features: Default::default(),
            config,
        }
    }

    pub fn with_additional_features(mut self, overrides: &[FeatureFlag]) -> Self {
        self.additional_features.extend(overrides);
        self
    }

    pub fn set(state: ChannelState) {
        *CHANNEL_STATE.lock() = state;
    }

    pub fn is_release_bundle() -> bool {
        cfg!(feature = "release_bundle")
    }

    pub fn enable_debug_features() -> bool {
        cfg!(debug_assertions) || matches!(Self::channel(), Channel::Local | Channel::Dev)
    }

    /// Returns the canonical identifier for the application.
    ///
    /// This should not be used for namespacing persisted data - such use cases
    /// should make use of [`Self::data_domain`] instead.
    pub fn app_id() -> AppId {
        CHANNEL_STATE.lock().config.app_id.clone()
    }

    /// Returns a profile name for isolating user data. This should be used to
    /// sandbox how user data is stored.
    ///
    /// This is a debugging tool for isolating development instances of Warp, and is not
    /// supported in release builds.
    pub fn data_profile() -> Option<String> {
        if cfg!(debug_assertions) {
            std::env::var("WARP_DATA_PROFILE").ok()
        } else {
            None
        }
    }

    /// Returns a value that should be used for namespacing persisted data.
    ///
    /// In release builds, this is identical to the app ID; in debug builds,
    /// it optionally includes a suffix derived from the `WARP_DATA_PROFILE`
    /// environment variable.
    pub fn data_domain() -> String {
        match Self::data_profile() {
            Some(profile) => format!("{}-{profile}", Self::app_id()),
            None => Self::app_id().to_string(),
        }
    }

    /// Returns the data domain if overridden from the default, otherwise None.
    pub fn data_domain_if_not_default() -> Option<String> {
        Self::data_profile().map(|_| Self::data_domain())
    }

    pub fn additional_features() -> HashSet<FeatureFlag> {
        CHANNEL_STATE
            .lock()
            .additional_features
            .iter()
            .cloned()
            .collect()
    }

    pub fn debug_str() -> String {
        let state = CHANNEL_STATE.lock();
        format!(
            "ChannelState {{ channel: {:?}, additional_features: {:?}, app_id: {:?}, autoupdate_configured: {}, mcp_static_configured: {} }}",
            state.channel,
            state.additional_features,
            state.config.app_id,
            state.config.autoupdate_config.is_some(),
            state.config.mcp_static_config.is_some()
        )
    }

    pub fn logfile_name() -> Cow<'static, str> {
        CHANNEL_STATE.lock().config.logfile_name.clone()
    }

    pub fn telemetry_file_name() -> Cow<'static, str> {
        Cow::default()
    }

    pub fn is_telemetry_available() -> bool {
        false
    }

    pub fn is_crash_reporting_available() -> bool {
        false
    }

    pub fn releases_base_url() -> Cow<'static, str> {
        CHANNEL_STATE
            .lock()
            .config
            .autoupdate_config
            .as_ref()
            .map(|ac| ac.releases_base_url.clone())
            .unwrap_or_default()
    }

    pub fn server_root_url() -> Cow<'static, str> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "test-util")] {
                Cow::Owned(MOCK_SERVER_URL.clone())
            } else {
                Cow::Borrowed(DISABLED_HTTP_SENTINEL)
            }
        }
    }

    // Returns the origin url, with scheme, domain, and ports (if any)
    pub fn server_root_domain() -> Origin {
        Url::parse(&Self::server_root_url())
            .expect("Server root URL should be valid")
            .origin()
    }

    pub fn channel() -> Channel {
        CHANNEL_STATE.lock().channel
    }

    pub fn is_cloud_disabled() -> bool {
        true
    }

    #[cfg(feature = "test-util")]
    pub fn app_version() -> Option<&'static str> {
        let version = APP_VERSION.lock();

        version.or_else(|| option_env!("GIT_RELEASE_TAG"))
    }

    #[cfg(feature = "test-util")]
    pub fn set_app_version(version: Option<&'static str>) {
        *APP_VERSION.lock() = version;
    }

    #[cfg(not(feature = "test-util"))]
    pub fn app_version() -> Option<&'static str> {
        option_env!("GIT_RELEASE_TAG")
    }

    pub fn show_autoupdate_menu_items() -> bool {
        CHANNEL_STATE
            .lock()
            .config
            .autoupdate_config
            .as_ref()
            .map(|ac| ac.show_autoupdate_menu_items)
            .unwrap_or_default()
    }

    /// Returns the MCP OAuth provider config matching the given client ID, if any.
    pub fn mcp_oauth_provider_by_client_id(client_id: &str) -> Option<McpOAuthProviderConfig> {
        CHANNEL_STATE
            .lock()
            .config
            .mcp_static_config
            .as_ref()
            .and_then(|c| c.providers.iter().find(|p| p.client_id == client_id))
            .cloned()
    }

    /// Returns the MCP OAuth provider config matching the given issuer URL, if any.
    pub fn mcp_oauth_provider_by_issuer(issuer: &str) -> Option<McpOAuthProviderConfig> {
        CHANNEL_STATE
            .lock()
            .config
            .mcp_static_config
            .as_ref()
            .and_then(|c| c.providers.iter().find(|p| p.issuer == issuer))
            .cloned()
    }

    pub fn url_scheme() -> &'static str {
        match Self::channel() {
            Channel::Stable => "warp",
            Channel::Preview => "warppreview",
            Channel::Dev => "warpdev",
            // Dummy value--integration tests shouldn't support URL schemes.
            Channel::Integration => "warpintegration",
            Channel::Local => "warplocal",
            Channel::Oss => "openwarp",
        }
    }
}

/// OpenWarp Wave 5-5：`derive_http_origin_from_ws_url` 随 `rtc_http_url()` 一同物理删。

#[cfg(all(test, not(feature = "test-util")))]
#[path = "state_tests.rs"]
mod tests;

fn app_id_from_bundle() -> Option<AppId> {
    // On macOS, attempt to determine the app ID from the containing bundle,
    // falling back to the channel-keyed "default" ID if we cannot retrieve
    // bundle information.
    //
    // We skip this for tests, as the call to `mainBundle` can take 30+ms,
    // which is a significant portion of the total test runtime.
    #[cfg(all(target_os = "macos", not(feature = "test-util")))]
    #[allow(deprecated)]
    unsafe {
        use cocoa::{
            base::{id, nil},
            foundation::NSBundle,
        };
        use objc::{msg_send, sel, sel_impl};
        use warpui::platform::mac::utils::nsstring_as_str;

        let bundle = id::mainBundle();
        if bundle != nil {
            let nsstring: id = msg_send![bundle, bundleIdentifier];
            if nsstring != nil {
                let app_id = nsstring_as_str(nsstring)
                    .expect("bundle IDs should always be valid UTF-8 strings");

                if !app_id.is_empty() {
                    return Some(
                        AppId::parse(app_id)
                            .expect("macOS bundle identifier has an unexpected format"),
                    );
                }
            }
        }
    }

    None
}
