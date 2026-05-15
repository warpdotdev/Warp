//! 全局 HTTP 代理配置。
//!
//! 见 Issue #72:OpenWarp 需要一个全局可配置的代理设置,统一覆盖所有出站 HTTP
//! 请求(BYOP 拉模型列表、autoupdate、对话加载等)。
//!
//! 设计要点:
//! - 三档 [`ProxyMode`]:`System` / `Custom` / `Off`。
//! - `System` 退回 reqwest 默认行为;workspace 的 reqwest 已启用
//!   `system-proxy` + `macos-system-configuration` features,因此 macOS 读
//!   SystemConfiguration、Windows 读 WinINET、Linux 读 `HTTP_PROXY` 等环境变量,
//!   无需自己实现。
//! - `Custom` 显式指定 URL / basic auth / no_proxy 列表。
//! - `Off` 调用 [`reqwest::ClientBuilder::no_proxy`],完全禁用代理(含环境变量)。
//!
//! 应用通过 [`set_global_proxy_config`] 在启动 / 设置变更时注入配置,
//! 后续所有 [`crate::Client::new`] 调用会读取该全局值并应用到 reqwest。
//!
//! reqwest 不支持已构造 `Client` 的运行时代理切换,因此调用方在变更设置后必须
//! 重建 Client 实例(例如 `AutoupdateState::new(http_client::Client::new())`)。

use std::sync::{OnceLock, RwLock};

/// 全局代理模式。
///
/// 默认项为 `Off`:避免在 app 层 settings 还未注入之前,冷启动期间构造的
/// `Client` 走上 reqwest 探测到的意外系统代理。app::ProxyMode 同一默认。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxyMode {
    /// 禁用代理,包括环境变量。默认项。
    #[default]
    Off,
    /// 完全跟随系统 / 环境变量(reqwest 默认行为)。
    System,
    /// 使用 [`ProxyConfig::url`] 中显式配置的代理。
    Custom,
}

impl ProxyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ProxyMode::System => "system",
            ProxyMode::Custom => "custom",
            ProxyMode::Off => "off",
        }
    }

    pub fn from_str_lenient(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "system" => ProxyMode::System,
            "custom" => ProxyMode::Custom,
            // off / disabled / none / 未知 都退回 Off(默认),避免意外走系统代理。
            _ => ProxyMode::Off,
        }
    }
}

/// 已解析的全局代理配置。
///
/// `username` 在 settings.toml 中明文存储,`password` 通过 `managed_secrets`
/// 单独保存(与 BYOP API key 同模式),由调用方在装配本结构前注入到 [`Self::password`]。
#[derive(Clone, Debug, Default)]
pub struct ProxyConfig {
    pub mode: ProxyMode,
    /// 例:`http://proxy.corp:8080`。仅在 [`ProxyMode::Custom`] 下生效。
    pub url: String,
    pub username: String,
    pub password: String,
    /// 逗号分隔的 host 列表;空字符串表示无例外。
    pub no_proxy: String,
}

impl ProxyConfig {
    /// 将本配置应用到 `reqwest::ClientBuilder`。
    ///
    /// 出错时(`Custom` 模式但 URL 不合法)在日志中告警并退回 reqwest 默认行为,
    /// 不让 `Client::new()` panic。
    pub fn apply(&self, mut builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        match self.mode {
            ProxyMode::System => builder,
            ProxyMode::Off => builder.no_proxy(),
            ProxyMode::Custom => {
                let trimmed = self.url.trim();
                if trimmed.is_empty() {
                    log::warn!(
                        "HTTP 代理设置为 Custom 但 URL 为空,退回 reqwest 默认(读系统代理)"
                    );
                    return builder;
                }

                let proxy_result = reqwest::Proxy::all(trimmed);
                let mut proxy = match proxy_result {
                    Ok(p) => p,
                    Err(err) => {
                        log::warn!(
                            "HTTP 代理 URL '{trimmed}' 无效({err}),退回 reqwest 默认"
                        );
                        return builder;
                    }
                };

                if !self.username.is_empty() || !self.password.is_empty() {
                    proxy = proxy.basic_auth(&self.username, &self.password);
                }

                if !self.no_proxy.trim().is_empty() {
                    if let Some(no_proxy) = reqwest::NoProxy::from_string(self.no_proxy.trim()) {
                        proxy = proxy.no_proxy(Some(no_proxy));
                    }
                }

                builder = builder.proxy(proxy);
                builder
            }
        }
    }
}

static GLOBAL_PROXY_CONFIG: OnceLock<RwLock<ProxyConfig>> = OnceLock::new();

fn slot() -> &'static RwLock<ProxyConfig> {
    GLOBAL_PROXY_CONFIG.get_or_init(|| RwLock::new(ProxyConfig::default()))
}

/// 安装新的全局代理配置。
///
/// 仅影响该调用之后构造的 `Client`。`reqwest::Client` 一旦构造完成无法切换
/// 代理,因此应用层在变更设置后需要重建所有共享的 Client 实例。
pub fn set_global_proxy_config(cfg: ProxyConfig) {
    let lock = slot();
    if let Ok(mut guard) = lock.write() {
        *guard = cfg;
    } else {
        log::error!("写入全局 HTTP 代理配置失败:RwLock 已 poison");
    }
}

/// 读取当前的全局代理配置(若未设置则返回默认值)。
pub fn current_proxy_config() -> ProxyConfig {
    let lock = slot();
    match lock.read() {
        Ok(guard) => guard.clone(),
        Err(err) => {
            log::error!("读取全局 HTTP 代理配置失败:RwLock 已 poison({err})");
            ProxyConfig::default()
        }
    }
}

#[cfg(test)]
#[path = "proxy_tests.rs"]
mod tests;
