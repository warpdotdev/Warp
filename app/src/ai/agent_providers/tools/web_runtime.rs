//! BYOP `webfetch` 与 `websearch` 工具的本地执行逻辑。
//!
//! 这两个 BYOP 工具不走 protobuf executor(`warp_multi_agent_api` 没有对应 variant),
//! 由 `chat_stream.rs::handle_byop_web_tool_intercept` 在 `parse_incoming_tool_call`
//! 之前直接调用本模块,把结果合成 `(ToolCall carrier, ToolCallResult)` 一对消息推回流。
//!
//! ## 与 opencode 对齐
//!
//! - `webfetch` 镜像 `packages/opencode/src/tool/webfetch.ts`:
//!   * UA 默认 Chrome,403 + `cf-mitigated: challenge` → 切回 `OpenWarp` UA 重试一次
//!   * `Accept` 头按 format 参数 q 优先级协商
//!   * Content-Length 预检 + 实读字节双检,5 MB 上限
//!   * timeout 默认 30s,上限 120s
//!   * 图片 mime 自动 base64 → output.attachments
//! - `websearch` 镜像 `packages/opencode/src/tool/{websearch,mcp-exa}.ts`:
//!   * 默认匿名 `https://mcp.exa.ai/mcp`,`EXA_API_KEY` 环境变量存在则拼到 querystring
//!   * 25s timeout
//!   * SSE 响应 → `result.content[0].text`

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_LENGTH, CONTENT_TYPE, USER_AGENT};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use std::time::Duration;

use super::exa;

// ---------------------------------------------------------------------------
// 常量(对齐 opencode webfetch.ts:8-10)
// ---------------------------------------------------------------------------

pub const MAX_RESPONSE_SIZE: usize = 5 * 1024 * 1024; // 5 MB
pub const DEFAULT_FETCH_TIMEOUT_SECS: u64 = 30;
pub const MAX_FETCH_TIMEOUT_SECS: u64 = 120;
pub const SEARCH_TIMEOUT_SECS: u64 = 25;

pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";
pub const FALLBACK_UA: &str = "OpenWarp";

// ---------------------------------------------------------------------------
// webfetch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FetchFormat {
    Markdown,
    Text,
    Html,
}

impl Default for FetchFormat {
    fn default() -> Self {
        Self::Markdown
    }
}

impl FetchFormat {
    fn accept_header(&self) -> &'static str {
        match self {
            Self::Markdown => {
                "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, \
                 text/html;q=0.7, */*;q=0.1"
            }
            Self::Text => "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1",
            Self::Html => {
                "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, \
                 text/markdown;q=0.7, */*;q=0.1"
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetchArgs {
    pub url: String,
    #[serde(default)]
    pub format: Option<FetchFormat>,
    /// 单位:秒。`None` → 30s;上限 120s,超过被 clamp。
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FetchAttachment {
    pub mime: String,
    /// `data:<mime>;base64,<...>` 形式(对齐 opencode)。
    pub url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FetchOutput {
    pub url: String,
    pub status: u16,
    pub content_type: String,
    pub format: String,
    pub output: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<FetchAttachment>,
}

/// Returns `true` if the IP address is private, loopback, link-local, or
/// otherwise should not be reachable from a webfetch tool (SSRF protection).
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4),
        IpAddr::V6(v6) => {
            // IPv4-mapped IPv6 (::ffff:x.x.x.x) — apply IPv4 rules.
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ipv4(mapped);
            }
            v6.is_loopback()               // ::1
                || v6.is_unspecified()      // ::
                || is_ipv6_unique_local(v6) // fc00::/7
                || is_ipv6_link_local(v6)   // fe80::/10
                || is_ipv6_documentation(v6) // 2001:db8::/32
        }
    }
}

fn is_blocked_ipv4(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    v4.is_loopback()          // 127.0.0.0/8
        || v4.is_private()    // 10/8, 172.16/12, 192.168/16
        || v4.is_link_local() // 169.254.0.0/16
        || v4.is_unspecified() // 0.0.0.0
        || v4.is_broadcast()  // 255.255.255.255
        || (Ipv4Addr::new(100, 64, 0, 0) <= v4 && v4 <= Ipv4Addr::new(100, 127, 255, 255))
            // CGNAT 100.64/10
        || (o[0] == 192 && o[1] == 0 && o[2] == 2)   // TEST-NET-1  192.0.2.0/24
        || (o[0] == 198 && o[1] == 51 && o[2] == 100) // TEST-NET-2  198.51.100.0/24
        || (o[0] == 203 && o[1] == 0 && o[2] == 113)  // TEST-NET-3  203.0.113.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18)       // Benchmarking 198.18.0.0/15
        || o[0] >= 240 // Reserved    240.0.0.0/4
}

fn is_ipv6_unique_local(v6: Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xfe00) == 0xfc00
}

fn is_ipv6_link_local(v6: Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xffc0) == 0xfe80
}

fn is_ipv6_documentation(v6: Ipv6Addr) -> bool {
    v6.segments()[0] == 0x2001 && v6.segments()[1] == 0x0db8
}

/// Validate a URL for SSRF safety: reject private/internal IP ranges after DNS
/// resolution.
fn validate_url_not_internal(url_str: &str) -> Result<()> {
    let parsed = url::Url::parse(url_str).context("invalid URL")?;
    let host = parsed.host_str().context("URL has no host")?;

    // If the host is already an IP literal, check directly.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_blocked_ip(ip) {
            bail!("URL targets a blocked IP address range");
        }
    }

    // Also try resolving the hostname to catch DNS rebinding to internal IPs.
    // Use std::net (blocking) — this runs in an async context but the resolution
    // is fast for legitimate hosts and the alternative (tokio::net) would add a
    // dependency.  We resolve with port 0 since we only need the address.
    if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(host, 0)) {
        for addr in addrs {
            if is_blocked_ip(addr.ip()) {
                bail!("URL resolves to a blocked IP address range");
            }
        }
    }

    Ok(())
}

/// DNS resolver that filters out blocked (internal/private) IPs at resolution
/// time, eliminating the TOCTOU gap between pre-validation and connection.
///
/// Only available on non-WASM targets — reqwest's `dns` module (and
/// `ClientBuilder::dns_resolver`) is not exposed on WebAssembly.
#[cfg(not(target_arch = "wasm32"))]
struct SsrfSafeResolver;

#[cfg(not(target_arch = "wasm32"))]
impl reqwest::dns::Resolve for SsrfSafeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            use std::net::ToSocketAddrs;
            let addrs: Vec<std::net::SocketAddr> = (host.as_str(), 0)
                .to_socket_addrs()
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                .filter(|addr| !is_blocked_ip(addr.ip()))
                .collect();
            if addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("DNS for '{host}' resolved to blocked IPs (SSRF protection)"),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Maximum number of redirects before stopping (matches reqwest's default).
const MAX_REDIRECT_HOPS: usize = 10;

/// Build a reqwest client with:
/// - A custom DNS resolver that blocks connections to internal IPs
/// - A redirect policy that enforces HTTPS, validates each hop, and limits
///   the total number of redirects (reqwest's default limits are not inherited
///   by `Policy::custom`)
pub fn build_ssrf_safe_client() -> Result<reqwest::Client> {
    let policy = Policy::custom(|attempt| {
        // Enforce redirect hop limit (Policy::custom does not inherit
        // reqwest's default loop/max-hop protections).
        if attempt.previous().len() >= MAX_REDIRECT_HOPS {
            return attempt.stop();
        }
        let url = attempt.url();
        // Enforce HTTPS on redirect targets (prevent HTTPS→HTTP downgrade).
        if url.scheme() != "https" {
            return attempt.stop();
        }
        // Validate redirect target is not internal (defense-in-depth on top
        // of the DNS resolver, catches IP-literal redirect URLs immediately).
        if validate_url_not_internal(url.as_str()).is_err() {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });
    let builder = reqwest::Client::builder()
        .redirect(policy)
        .pool_idle_timeout(Duration::from_secs(30));
    // Wire the SSRF-safe DNS resolver only on non-WASM targets (reqwest
    // does not expose the dns module on WebAssembly).
    #[cfg(not(target_arch = "wasm32"))]
    let builder = builder.dns_resolver(Arc::new(SsrfSafeResolver));
    builder.build().context("build SSRF-safe reqwest client")
}

/// 入口:执行一次 webfetch,返回结构化 output(由 caller `serde_json::to_value` 喂给上游 LLM)。
pub async fn run_webfetch(client: &reqwest::Client, args: FetchArgs) -> Result<FetchOutput> {
    if !args.url.starts_with("https://") {
        bail!("URL must use HTTPS");
    }
    validate_url_not_internal(&args.url)?;
    let format = args.format.clone().unwrap_or_default();
    let timeout_secs = args
        .timeout
        .unwrap_or(DEFAULT_FETCH_TIMEOUT_SECS)
        .min(MAX_FETCH_TIMEOUT_SECS);
    let timeout = Duration::from_secs(timeout_secs);

    let accept = format.accept_header();
    let resp = match send_fetch(&client, &args.url, accept, CHROME_UA, timeout).await {
        Ok(r) => r,
        Err(e) => return Err(e),
    };

    // Cloudflare 挑战:Chrome UA 第一轮 403 + cf-mitigated: challenge → 换 UA 重试一次。
    let resp = if resp.status() == StatusCode::FORBIDDEN
        && resp
            .headers()
            .get("cf-mitigated")
            .and_then(|v| v.to_str().ok())
            == Some("challenge")
    {
        log::info!("[webfetch] cloudflare challenge detected → retry with fallback UA");
        send_fetch(&client, &args.url, accept, FALLBACK_UA, timeout).await?
    } else {
        resp
    };

    let status = resp.status();
    if !status.is_success() {
        bail!("HTTP {} fetching {}", status.as_u16(), args.url);
    }

    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mime = content_type
        .split(';')
        .next()
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_default();

    // Content-Length 预检
    if let Some(len_str) = resp
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(len) = len_str.parse::<usize>() {
            if len > MAX_RESPONSE_SIZE {
                bail!(
                    "Response too large (Content-Length {} > {} bytes limit)",
                    len,
                    MAX_RESPONSE_SIZE
                );
            }
        }
    }

    let bytes = resp.bytes().await.context("read response body")?;
    if bytes.len() > MAX_RESPONSE_SIZE {
        bail!(
            "Response too large ({} bytes > {} bytes limit)",
            bytes.len(),
            MAX_RESPONSE_SIZE
        );
    }

    // 图片 → base64 attachment
    if is_image_mime(&mime) {
        let encoded = BASE64.encode(&bytes);
        let data_url = format!("data:{mime};base64,{encoded}");
        return Ok(FetchOutput {
            url: args.url.clone(),
            status: status.as_u16(),
            content_type,
            format: format!("{format:?}").to_ascii_lowercase(),
            output: "Image fetched successfully".to_owned(),
            attachments: vec![FetchAttachment {
                mime,
                url: data_url,
            }],
        });
    }

    let body_str = String::from_utf8_lossy(&bytes).into_owned();
    let is_html = mime == "text/html" || mime == "application/xhtml+xml";

    let output = match format {
        FetchFormat::Markdown if is_html => html_to_markdown(&body_str),
        FetchFormat::Text if is_html => extract_text_from_html(&body_str),
        FetchFormat::Html => body_str,
        // markdown / text 但 mime 不是 html → 透传(已经是 text 类)
        _ => body_str,
    };

    Ok(FetchOutput {
        url: args.url.clone(),
        status: status.as_u16(),
        content_type,
        format: format!("{format:?}").to_ascii_lowercase(),
        output: maybe_format_json(&output, &mime),
        attachments: vec![],
    })
}

async fn send_fetch(
    client: &reqwest::Client,
    url: &str,
    accept: &str,
    ua: &str,
    timeout: Duration,
) -> Result<reqwest::Response> {
    client
        .get(url)
        .header(USER_AGENT, ua)
        .header(ACCEPT, accept)
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .timeout(timeout)
        .send()
        .await
        .with_context(|| format!("HTTP GET {url}"))
}

fn is_image_mime(mime: &str) -> bool {
    mime.starts_with("image/")
}

/// 若 mime 是 application/json,且 content 是合法 JSON,美化为 ```json``` 代码块
/// (对齐 zed fetch_tool.rs 的 JSON 处理)。
fn maybe_format_json(content: &str, mime: &str) -> String {
    if mime != "application/json" {
        return content.to_owned();
    }
    match serde_json::from_str::<Value>(content) {
        Ok(v) => match serde_json::to_string_pretty(&v) {
            Ok(pretty) => format!("```json\n{pretty}\n```"),
            Err(_) => content.to_owned(),
        },
        Err(_) => content.to_owned(),
    }
}

fn html_to_markdown(html: &str) -> String {
    // htmd 默认配置已对齐 Turndown 的常见输出风格(atx 标题、fenced code block 等)。
    // 预先剥 script / style / noscript / iframe 内容(htmd 默认会把这些标签内的
    // 文本当作普通文本保留,污染 markdown 输出)。
    let pre = strip_unsafe_blocks(html);
    match std::panic::catch_unwind(|| htmd::convert(&pre)) {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            log::warn!("[webfetch] htmd convert error: {e}, falling back to text extraction");
            naive_html_strip(&pre)
        }
        Err(_) => {
            log::warn!("[webfetch] htmd panicked, falling back to text extraction");
            naive_html_strip(&pre)
        }
    }
}

/// 删 `<script>...</script>` / `<style>...</style>` / `<noscript>...</noscript>` /
/// `<iframe>...</iframe>` 整段(大小写不敏感,允许 attribute)。
fn strip_unsafe_blocks(html: &str) -> String {
    let mut out = html.to_owned();
    for tag in &["script", "style", "noscript", "iframe", "object", "embed"] {
        out = strip_tag_block(&out, tag);
    }
    out
}

fn strip_tag_block(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(rel_open) = lower[cursor..].find(&open) {
        let abs_open = cursor + rel_open;
        // 必须接着 `>` 或空白(避免误吞 <scriptlike>)
        let after = abs_open + open.len();
        match html.as_bytes().get(after) {
            Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'/') => {}
            _ => {
                out.push_str(&html[cursor..=abs_open]);
                cursor = abs_open + 1;
                continue;
            }
        }
        out.push_str(&html[cursor..abs_open]);
        // 找闭合
        match lower[after..].find(&close) {
            Some(rel_close) => {
                cursor = after + rel_close + close.len();
            }
            None => {
                // 没闭合 → 整段丢弃
                cursor = html.len();
                break;
            }
        }
    }
    out.push_str(&html[cursor..]);
    out
}

/// 极简 HTML→纯文本兜底:正则 strip 所有 tag。仅 htmd 失败时用。
fn naive_html_strip(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// HTML → 纯文本:先用 htmd 转 markdown,再剥 markdown 标记。
///
/// 简化路径,避免再引入 html5ever DOM 遍历依赖(`markup5ever_rcdom`)。htmd 内部
/// 已经过滤了 script/style/noscript 等不可见标签,纯文本输出对 text 模式足够。
fn extract_text_from_html(html: &str) -> String {
    let md = html_to_markdown(html);
    strip_markdown(&md)
}

fn strip_markdown(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut last_blank = false;
    for raw_line in md.lines() {
        let mut line = raw_line.trim().to_owned();
        // 标题前缀 # ## ###
        while line.starts_with('#') {
            line.remove(0);
        }
        let line = line.trim_start();
        // 列表 / 引用 / 水平线 prefix
        let line = line
            .trim_start_matches(|c: char| c == '-' || c == '*' || c == '>' || c == '+')
            .trim_start();
        // ![alt](url) → 删整段
        let line = strip_pattern(line, "![", ")");
        // [text](url) → 保留 text
        let line = unwrap_links(&line);
        // `code` / **bold** / *em* / _em_ — 保守地把 ` * _ 删掉
        let cleaned: String = line
            .chars()
            .filter(|c| !matches!(c, '`' | '*' | '_'))
            .collect();
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            if !last_blank && !out.is_empty() {
                out.push('\n');
                last_blank = true;
            }
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(trimmed);
        last_blank = false;
    }
    out
}

fn strip_pattern(s: &str, start: &str, end: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(start) {
        out.push_str(&rest[..i]);
        let after = &rest[i + start.len()..];
        match after.find(end) {
            Some(j) => rest = &after[j + end.len()..],
            None => {
                // 没闭合,保留剩余
                rest = after;
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `[text](url)` → `text`
fn unwrap_links(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // 找 ]( 然后 )
            if let Some(close_text) = s[i + 1..].find("](") {
                let text_end = i + 1 + close_text;
                if let Some(close_url) = s[text_end + 2..].find(')') {
                    let url_end = text_end + 2 + close_url;
                    out.push_str(&s[i + 1..text_end]);
                    i = url_end + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// websearch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct SearchToolArgs {
    pub query: String,
    #[serde(rename = "numResults", default)]
    pub num_results: Option<u32>,
    #[serde(default)]
    pub livecrawl: Option<String>,
    #[serde(rename = "type", default)]
    pub search_type: Option<String>,
    #[serde(rename = "contextMaxCharacters", default)]
    pub context_max_characters: Option<u32>,
}

impl SearchToolArgs {
    pub fn into_exa_args(self) -> exa::SearchArgs {
        let mut a = exa::SearchArgs::with_defaults(self.query);
        if let Some(n) = self.num_results {
            a.num_results = n;
        }
        if let Some(s) = self.livecrawl {
            a.livecrawl = s;
        }
        if let Some(t) = self.search_type {
            a.search_type = t;
        }
        if let Some(c) = self.context_max_characters {
            a.context_max_characters = Some(c);
        }
        a
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchOutput {
    pub query: String,
    /// Exa 返回的人类可读 / LLM-optimized context 字符串。
    pub results: String,
}

const EMPTY_FALLBACK: &str = "No search results found. Please try a different query.";

/// 入口:执行一次 Exa websearch。
///
/// `endpoint_override`:测试用,默认走 `exa::endpoint_url(api_key)`。
/// `api_key`:`None` → 匿名;`Some(...)` → 拼到 querystring。
pub async fn run_websearch(
    client: &reqwest::Client,
    args: SearchToolArgs,
    api_key: Option<&str>,
    endpoint_override: Option<&str>,
) -> Result<SearchOutput> {
    let query = args.query.clone();
    let exa_args = args.into_exa_args();
    let body = exa::build_request_body(exa::SEARCH_TOOL_NAME, &exa_args);

    let url = endpoint_override
        .map(|s| s.to_owned())
        .unwrap_or_else(|| exa::endpoint_url(api_key));

    let resp = client
        .post(&url)
        .header(ACCEPT, "application/json, text/event-stream")
        .header(CONTENT_TYPE, "application/json")
        .timeout(Duration::from_secs(SEARCH_TIMEOUT_SECS))
        .json(&body)
        .send()
        .await
        .with_context(|| format!("Exa POST {url}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        bail!("Exa returned HTTP {} ({})", status.as_u16(), body_text);
    }
    let body_text = resp.text().await.context("read Exa SSE body")?;

    let parsed = exa::parse_sse_body(&body_text)?;
    let results = parsed.unwrap_or_else(|| EMPTY_FALLBACK.to_owned());
    Ok(SearchOutput { query, results })
}

/// 把 webfetch / websearch 的结构化结果序列化为 JSON Value(给上游 LLM 看的字符串)。
///
/// 所有 BYOP 本地拦截工具的 tool_result 必须带 `"_byop_intercepted":true` sentinel,
/// 否则 controller (`controller.rs:2693+`) 不会触发 auto-resume,模型会卡在等结果。
/// 见 `chat_stream::dispatch_byop_web_tool` 与 controller 的 `needs_byop_local_resume` 检测。
pub fn fetch_output_to_json(out: &FetchOutput) -> Value {
    let mut v = serde_json::to_value(out).unwrap_or_else(|_| json!({"status": "serialize_error"}));
    if let Some(obj) = v.as_object_mut() {
        obj.insert("_byop_intercepted".to_owned(), Value::Bool(true));
    }
    v
}
pub fn search_output_to_json(out: &SearchOutput) -> Value {
    let mut v = serde_json::to_value(out).unwrap_or_else(|_| json!({"status": "serialize_error"}));
    if let Some(obj) = v.as_object_mut() {
        obj.insert("_byop_intercepted".to_owned(), Value::Bool(true));
    }
    v
}
pub fn error_to_json(tool: &str, e: &anyhow::Error) -> Value {
    json!({
        "_byop_intercepted": true,
        "status": "error",
        "tool": tool,
        "message": format!("{e:#}"),
    })
}

#[cfg(test)]
#[path = "webfetch_tests.rs"]
mod webfetch_tests;
#[cfg(test)]
#[path = "websearch_tests.rs"]
mod websearch_tests;
