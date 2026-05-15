//! HTTP proxy support for WebSocket connections.
//!
//! 优先读取 `http_client::current_proxy_config()` 这个全局单例。如果是
//! `ProxyMode::Custom` 或 `Off`,则直接应用;如果是 `ProxyMode::System`,则
//! fallback 到原有的环境变量解析逻辑(`HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` /
//! `NO_PROXY`)。这样设置页的 Custom URL / Off 能覆盖到 WebSocket。见 Issue #72。
//!
//! TODO: Switch to tungstenite's native proxy support once it is available and remove this
//! module: <https://github.com/snapview/tungstenite-rs/pull/530>

use std::env;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use anyhow::{bail, Context};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use http_body_util::Empty;
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use percent_encoding::percent_decode_str;
use tokio::net::TcpStream;
use tokio::time::timeout;
use url::Url;

/// 代理模式镜像。与 `http_client::ProxyMode` 一一对应,只是为了避免
/// `websocket -> http_client -> warp_core -> websocket` 的循环依赖,这里本地镜像一份。
/// `app` 启动 / 设置变更时同时调用 `http_client::set_global_proxy_config` 和
/// `websocket::set_global_proxy_config` 保证两路保持一致。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxyMode {
    /// 与 `http_client::ProxyMode` 一致:默认 `Off`。
    #[default]
    Off,
    System,
    Custom,
}

/// `http_client::ProxyConfig` 的镜像,参见 [`ProxyMode`] 的说明。
#[derive(Clone, Debug, Default)]
pub struct ProxyConfig {
    pub mode: ProxyMode,
    pub url: String,
    pub username: String,
    pub password: String,
    pub no_proxy: String,
}

static GLOBAL_PROXY_CONFIG: OnceLock<RwLock<ProxyConfig>> = OnceLock::new();

fn slot() -> &'static RwLock<ProxyConfig> {
    GLOBAL_PROXY_CONFIG.get_or_init(|| RwLock::new(ProxyConfig::default()))
}

/// 安装全局 WebSocket 代理配置。应在启动与设置变更时同时调用
/// `http_client::set_global_proxy_config`。
pub fn set_global_proxy_config(cfg: ProxyConfig) {
    if let Ok(mut guard) = slot().write() {
        *guard = cfg;
    } else {
        log::error!("写入 WebSocket 代理配置失败:RwLock 已 poison");
    }
}

fn current_proxy_config() -> ProxyConfig {
    match slot().read() {
        Ok(guard) => guard.clone(),
        Err(err) => {
            log::error!("读取 WebSocket 代理配置失败:RwLock 已 poison({err})");
            ProxyConfig::default()
        }
    }
}

const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const PROXY_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Proxy connection info parsed from environment variables.
#[derive(Debug)]
pub struct ProxyInfo {
    pub host: String,
    pub port: u16,
    /// Base64-encoded `user:password` for `Proxy-Authorization: Basic` header.
    pub basic_auth: Option<String>,
}

/// Returns proxy info if a proxy should be used for the given target URI.
///
/// 优先级:
/// 1. 读取 `http_client::current_proxy_config()`:
///    - `Custom`: 使用设置页填入的 URL / auth / no_proxy。
///    - `Off`: 返回 `None`。
///    - `System`: 落到环境变量解析。
/// 2. 环境变量解析(保持向后兼容):
///    - For TLS targets (`wss://`): `HTTPS_PROXY` / `https_proxy`, then `ALL_PROXY` / `all_proxy`.
///    - For plain targets (`ws://`): `HTTP_PROXY` / `http_proxy`, then `ALL_PROXY` / `all_proxy`.
///    - `NO_PROXY` / `no_proxy` is checked to bypass the proxy for specific hosts.
pub fn resolve_proxy(uri: &http::Uri) -> anyhow::Result<Option<ProxyInfo>> {
    let target_host = uri.host().unwrap_or_default();

    // 优先走全局设置(与 http_client 镇 mirror,避免循环依赖)。
    let global_cfg = current_proxy_config();
    match global_cfg.mode {
        ProxyMode::Off => return Ok(None),
        ProxyMode::Custom => {
            // 重用 settings 中的 no_proxy 列表(以逗号分隔)。
            if !global_cfg.no_proxy.trim().is_empty()
                && host_matches_no_proxy_list(target_host, &global_cfg.no_proxy)
            {
                return Ok(None);
            }
            let trimmed = global_cfg.url.trim();
            if trimmed.is_empty() {
                // Custom 但 URL 为空: 与 http_client 一致,静默退回环境变量解析。
                log::warn!(
                    "WebSocket: HTTP 代理设置为 Custom 但 URL 为空,退回环境变量"
                );
            } else {
                let info = parse_proxy_url_with_optional_auth(
                    trimmed,
                    &global_cfg.username,
                    &global_cfg.password,
                )
                .context("Invalid custom proxy URL configured in settings")?;
                return Ok(Some(info));
            }
        }
        ProxyMode::System => {
            // 落入环境变量解析。
        }
    }

    let is_tls = uri.scheme_str() == Some("wss") || uri.scheme_str() == Some("https");
    let proxy_env = if is_tls {
        read_env_var("HTTPS_PROXY").or_else(|| read_env_var("ALL_PROXY"))
    } else {
        read_env_var("HTTP_PROXY").or_else(|| read_env_var("ALL_PROXY"))
    };

    let Some((proxy_env_name, proxy_url)) = proxy_env else {
        return Ok(None);
    };

    if is_no_proxy(target_host) {
        return Ok(None);
    }

    parse_proxy_url(&proxy_url)
        .with_context(|| format!("Invalid proxy URL configured in {proxy_env_name}"))
        .map(Some)
}

/// 拆分逗号分隔的 no_proxy 列表并多 apply `is_no_proxy` 中的同样规则。
fn host_matches_no_proxy_list(target_host: &str, no_proxy: &str) -> bool {
    let target = target_host.to_lowercase();
    for entry in no_proxy.split(',') {
        let entry = entry.trim().to_lowercase();
        if entry.is_empty() {
            continue;
        }
        if entry == "*" {
            return true;
        }
        if target == entry {
            return true;
        }
        if entry.starts_with('.') && target.ends_with(&entry) {
            return true;
        }
        if target.ends_with(&format!(".{entry}")) {
            return true;
        }
    }
    false
}

/// `parse_proxy_url` 的变体,附加 settings 中的显式 username / password(覆盖 URL 中的)。
fn parse_proxy_url_with_optional_auth(
    raw: &str,
    extra_user: &str,
    extra_pass: &str,
) -> anyhow::Result<ProxyInfo> {
    let mut info = parse_proxy_url(raw)?;
    if !extra_user.is_empty() || !extra_pass.is_empty() {
        let userinfo = format!("{extra_user}:{extra_pass}");
        info.basic_auth = Some(BASE64.encode(userinfo));
    }
    Ok(info)
}

/// Establishes a TCP connection through an HTTP proxy using the CONNECT method.
///
/// Uses hyper's HTTP/1 client to send the CONNECT request and then extracts
/// the underlying `TcpStream` via the upgrade mechanism.
pub async fn connect_via_proxy(
    proxy: &ProxyInfo,
    target_uri: &http::Uri,
) -> anyhow::Result<TcpStream> {
    let target_host = target_uri.host().context("Target URI has no host")?;
    let is_tls = target_uri.scheme_str() == Some("wss") || target_uri.scheme_str() == Some("https");
    let default_port: u16 = if is_tls { 443 } else { 80 };
    let target_port = target_uri.port_u16().unwrap_or(default_port);

    // 1. TCP connect to the proxy.
    let stream = timeout(
        PROXY_CONNECT_TIMEOUT,
        TcpStream::connect((&*proxy.host, proxy.port)),
    )
    .await
    .context("Timed out connecting to proxy")?
    .with_context(|| format!("Failed to connect to proxy {}:{}", proxy.host, proxy.port))?;

    // 2. HTTP/1 handshake over the proxy TCP stream.
    let (mut sender, conn) = timeout(
        PROXY_HANDSHAKE_TIMEOUT,
        hyper::client::conn::http1::handshake(TokioIo::new(stream)),
    )
    .await
    .context("Timed out during HTTP handshake with proxy")?
    .context("HTTP handshake with proxy failed")?;

    // Drive the connection in the background with upgrade support.
    tokio::spawn(async move {
        if let Err(err) = conn.with_upgrades().await {
            log::warn!("Proxy connection driver error: {err}");
        }
    });

    // 3. Build and send the CONNECT request.
    let authority = format!("{target_host}:{target_port}");
    let mut req = hyper::Request::builder()
        .method(hyper::Method::CONNECT)
        .uri(&authority)
        .header(hyper::header::HOST, &authority)
        .body(Empty::<Bytes>::new())
        .context("Failed to build CONNECT request")?;

    if let Some(credentials) = &proxy.basic_auth {
        req.headers_mut().insert(
            "proxy-authorization",
            format!("Basic {credentials}")
                .parse()
                .context("Invalid Proxy-Authorization header value")?,
        );
    }

    let response = timeout(PROXY_HANDSHAKE_TIMEOUT, sender.send_request(req))
        .await
        .context("Timed out waiting for CONNECT response from proxy")?
        .context("Failed to send CONNECT request to proxy")?;

    if !response.status().is_success() {
        bail!("Proxy CONNECT failed with status: {}", response.status());
    }

    // 4. Upgrade the connection to get the raw stream.
    let upgraded = hyper::upgrade::on(response)
        .await
        .context("Failed to upgrade proxy connection after CONNECT")?;

    // 5. Downcast back to the underlying TcpStream.
    let downcast = upgraded.downcast::<TokioIo<TcpStream>>().map_err(|_| {
        anyhow::anyhow!("Failed to downcast upgraded proxy connection to TcpStream")
    })?;

    Ok(downcast.io.into_inner())
}

/// Reads an environment variable by its canonical (uppercase) name, falling back to lowercase.
fn read_env_var(uppercase_name: &str) -> Option<(String, String)> {
    env::var(uppercase_name)
        .ok()
        .filter(|v| !v.is_empty())
        .map(|value| (uppercase_name.to_string(), value))
        .or_else(|| {
            let lowercase_name = uppercase_name.to_lowercase();
            env::var(&lowercase_name)
                .ok()
                .filter(|v| !v.is_empty())
                .map(|value| (lowercase_name, value))
        })
}

/// Returns `true` if `target_host` matches any entry in `NO_PROXY` / `no_proxy`.
///
/// Supported patterns:
/// - `*` matches all hosts.
/// - Exact match (case-insensitive).
/// - Suffix match with leading `.` (e.g. `.example.com` matches `foo.example.com`).
/// - Suffix match without leading `.` (e.g. `example.com` matches `foo.example.com`).
fn is_no_proxy(target_host: &str) -> bool {
    let no_proxy = read_env_var("NO_PROXY")
        .map(|(_, value)| value)
        .unwrap_or_default();
    if no_proxy.is_empty() {
        return false;
    }

    let target = target_host.to_lowercase();
    for entry in no_proxy.split(',') {
        let entry = entry.trim().to_lowercase();
        if entry.is_empty() {
            continue;
        }
        if entry == "*" {
            return true;
        }
        if target == entry {
            return true;
        }
        // Suffix match: ".example.com" matches "foo.example.com"
        if entry.starts_with('.') && target.ends_with(&entry) {
            return true;
        }
        // Suffix match without leading dot: "example.com" matches "foo.example.com"
        if target.ends_with(&format!(".{entry}")) {
            return true;
        }
    }

    false
}

/// Parses a proxy URL string into a `ProxyInfo`.
fn parse_proxy_url(raw: &str) -> anyhow::Result<ProxyInfo> {
    // Many proxy URLs are specified without a scheme (e.g. "proxy.corp:8080").
    // Prepend "http://" if no scheme is present so the URL parser can handle it.
    let normalized = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{raw}")
    };
    let url = Url::parse(&normalized).context("failed to parse proxy URL")?;
    match url.scheme() {
        "http" => {}
        "https" => bail!("HTTPS proxy URLs are not supported"),
        scheme => bail!("Unsupported proxy scheme '{scheme}'"),
    }

    let host = url
        .host_str()
        .context("proxy URL is missing a host")?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(8080);

    let username = percent_decode_str(url.username())
        .decode_utf8()
        .context("proxy username contains invalid percent-encoding")?
        .into_owned();
    let password = url
        .password()
        .map(|password| {
            percent_decode_str(password)
                .decode_utf8()
                .context("proxy password contains invalid percent-encoding")
        })
        .transpose()?
        .map(|password| password.into_owned());

    let basic_auth = if !username.is_empty() || password.is_some() {
        let userinfo = format!("{username}:{}", password.unwrap_or_default());
        Some(BASE64.encode(userinfo))
    } else {
        None
    };

    Ok(ProxyInfo {
        host,
        port,
        basic_auth,
    })
}

#[cfg(test)]
#[path = "proxy_tests.rs"]
mod tests;
