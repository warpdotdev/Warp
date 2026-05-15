//! 全局 HTTP 网络代理设置。
//!
//! 见 Issue #72。提供一个用户可配置的全局代理项,值会注入到 `http_client::Client`
//! 与 `websocket` 两个出口,从而覆盖所有 BYOP 调用、autoupdate、对话加载、
//! MCP OAuth、cloud workflow fetch 等出站 HTTP/WS 请求。
//!
//! 三个字段:
//! - `proxy_mode`: `system` / `custom` / `off`(默认 `system`,等价 reqwest 的
//!   既有行为)。
//! - `proxy_url`:`Custom` 模式下使用,例如 `http://proxy.corp:8080`。
//! - `proxy_no_proxy`:逗号分隔的 host 例外列表,例如 `localhost,127.0.0.1,.internal`。
//!
//! 用户名 / 密码不在这里:用户名将放到一个独立 setting(或在 URL 里写),
//! 密码走 `managed_secrets`(与 BYOP API key 同模式),由 UI 单独管理。
//!
//! 为简化第一版,这里也提供了 username 字段;password 仍由 managed_secrets 管理。

use serde::{Deserialize, Serialize};
use settings::{macros::define_settings_group, SupportedPlatforms, SyncToCloud};

/// 用户可见的代理模式。
///
/// 与 `http_client::ProxyMode` / `websocket::ProxyMode` 一一对应;之所以单独
/// 定义一份是为了配置层与基础设施层解耦,且本类型需要实现 `JsonSchema` 等
/// settings 体系要求的 trait。
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "HTTP 代理模式: off 完全禁用(默认);system 沿用系统/环境;custom 使用显式 URL。",
    rename_all = "snake_case"
)]
pub enum ProxyMode {
    /// 强制禁用代理,包括环境变量。默认项;避免 reqwest 探测出的意外系统代理干扰本地调用。
    #[default]
    Off,
    /// 跟随系统代理 / 环境变量(reqwest 默认行为)。
    System,
    /// 使用用户填写的 URL。
    Custom,
}

impl ProxyMode {
    /// 转换为 `http_client::ProxyMode`。
    pub fn to_http_client_mode(self) -> http_client::ProxyMode {
        match self {
            ProxyMode::System => http_client::ProxyMode::System,
            ProxyMode::Custom => http_client::ProxyMode::Custom,
            ProxyMode::Off => http_client::ProxyMode::Off,
        }
    }

    /// 转换为 `websocket::ProxyMode`(独立镜像,见 websocket/proxy.rs 顶部注释)。
    pub fn to_websocket_mode(self) -> websocket::ProxyMode {
        match self {
            ProxyMode::System => websocket::ProxyMode::System,
            ProxyMode::Custom => websocket::ProxyMode::Custom,
            ProxyMode::Off => websocket::ProxyMode::Off,
        }
    }
}

define_settings_group!(NetworkSettings, settings: [
    proxy_mode: ProxyModeSetting {
        type: ProxyMode,
        default: ProxyMode::Off,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "network.proxy_mode",
        description: "HTTP 代理模式:off (默认) / system / custom。",
    },
    proxy_url: ProxyUrlSetting {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "network.proxy_url",
        description: "Custom 模式下的代理 URL,例:http://proxy.corp:8080。",
    },
    proxy_username: ProxyUsernameSetting {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "network.proxy_username",
        description: "Custom 模式下的代理用户名;为空表示无 basic auth 或无 username。",
    },
    proxy_no_proxy: ProxyNoProxySetting {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "network.proxy_no_proxy",
        description: "逗号分隔的 host 例外列表,例:localhost,127.0.0.1,.internal。",
    },
]);
