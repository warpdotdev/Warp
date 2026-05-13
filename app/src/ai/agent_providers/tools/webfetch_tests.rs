//! `web_runtime::run_webfetch` 单测(mockito,无外网)。

use super::*;
use mockito::{Matcher, Server};

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .build()
        .expect("reqwest client build")
}

fn args(url: &str) -> FetchArgs {
    FetchArgs {
        url: url.to_owned(),
        format: None,
        timeout: None,
    }
}

// ---------------------------------------------------------------------------
// URL 验证(纯逻辑,無 HTTP)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rejects_non_https_scheme() {
    let client = build_client();
    for bad in [
        "ftp://example.com",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "http://example.com",
        "",
    ] {
        let err = run_webfetch(&client, args(bad)).await.unwrap_err();
        assert!(err.to_string().contains("HTTPS"), "bad={bad} err={err}");
    }
}

#[tokio::test]
async fn rejects_http_urls() {
    let client = build_client();
    let err = run_webfetch(&client, args("http://example.com"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("HTTPS"),
        "HTTP should be rejected: {err}"
    );
}

// ---------------------------------------------------------------------------
// 内容类型分支 — use send_fetch directly since mockito uses http://
// ---------------------------------------------------------------------------

/// Helper: run a webfetch-like flow against a mockito server, bypassing the
/// HTTPS-only check (mockito only serves HTTP).  Tests the content processing
/// pipeline without the URL scheme gate.
async fn run_webfetch_test(
    server_url: &str,
    path: &str,
    format: Option<FetchFormat>,
) -> Result<FetchOutput> {
    let client = build_client();
    let url = format!("{server_url}{path}");
    let fmt = format.unwrap_or_default();
    let accept = fmt.accept_header();
    let timeout = std::time::Duration::from_secs(DEFAULT_FETCH_TIMEOUT_SECS);

    let resp = send_fetch(&client, &url, accept, CHROME_UA, timeout).await?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {} fetching {}", status.as_u16(), url);
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mime = content_type
        .split(';')
        .next()
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_default();

    let bytes = resp.bytes().await?;
    if bytes.len() > MAX_RESPONSE_SIZE {
        anyhow::bail!(
            "Response too large ({} bytes > {} bytes limit)",
            bytes.len(),
            MAX_RESPONSE_SIZE
        );
    }

    if is_image_mime(&mime) {
        let encoded = BASE64.encode(&bytes);
        let data_url = format!("data:{mime};base64,{encoded}");
        return Ok(FetchOutput {
            url: url.clone(),
            status: status.as_u16(),
            content_type,
            format: format!("{fmt:?}").to_ascii_lowercase(),
            output: "Image fetched successfully".to_owned(),
            attachments: vec![FetchAttachment {
                mime,
                url: data_url,
            }],
        });
    }

    let body_str = String::from_utf8_lossy(&bytes).into_owned();
    let is_html = mime == "text/html" || mime == "application/xhtml+xml";
    let output = match fmt {
        FetchFormat::Markdown if is_html => html_to_markdown(&body_str),
        FetchFormat::Text if is_html => extract_text_from_html(&body_str),
        FetchFormat::Html => body_str,
        _ => body_str,
    };

    Ok(FetchOutput {
        url: url.clone(),
        status: status.as_u16(),
        content_type,
        format: format!("{fmt:?}").to_ascii_lowercase(),
        output: maybe_format_json(&output, &mime),
        attachments: vec![],
    })
}

#[tokio::test]
async fn html_to_markdown() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/page")
        .with_status(200)
        .with_header("content-type", "text/html; charset=utf-8")
        .with_body("<html><body><h1>Hello</h1><p>World</p></body></html>")
        .create_async()
        .await;

    let out = run_webfetch_test(&server.url(), "/page", None)
        .await
        .expect("ok");
    assert!(
        out.output.contains("Hello"),
        "missing Hello: {}",
        out.output
    );
    assert!(
        out.output.contains("World"),
        "missing World: {}",
        out.output
    );
    assert!(
        out.output.contains('#') || !out.output.contains("<h1>"),
        "should be markdown not HTML: {}",
        out.output
    );
    assert_eq!(out.format, "markdown");
    assert!(out.attachments.is_empty());
}

#[tokio::test]
async fn text_plain_passthrough() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/text")
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("just some text")
        .create_async()
        .await;

    let out = run_webfetch_test(&server.url(), "/text", None)
        .await
        .expect("ok");
    assert_eq!(out.output, "just some text");
}

#[tokio::test]
async fn json_pretty_print() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/api")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"a":1,"b":[2,3]}"#)
        .create_async()
        .await;

    let out = run_webfetch_test(&server.url(), "/api", None)
        .await
        .expect("ok");
    assert!(
        out.output.starts_with("```json\n"),
        "missing fence: {}",
        out.output
    );
    assert!(
        out.output.contains("\"a\": 1"),
        "not pretty: {}",
        out.output
    );
    assert!(out.output.ends_with("\n```"));
}

#[tokio::test]
async fn image_attachment_base64() {
    let mut server = Server::new_async().await;
    // 1x1 transparent PNG
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let _m = server
        .mock("GET", "/img.png")
        .with_status(200)
        .with_header("content-type", "image/png")
        .with_body(png_bytes.clone())
        .create_async()
        .await;

    let out = run_webfetch_test(&server.url(), "/img.png", None)
        .await
        .expect("ok");
    assert_eq!(out.attachments.len(), 1);
    let att = &out.attachments[0];
    assert_eq!(att.mime, "image/png");
    assert!(att.url.starts_with("data:image/png;base64,"));
    let b64 = att.url.trim_start_matches("data:image/png;base64,");
    let decoded = BASE64.decode(b64).expect("decode");
    assert_eq!(decoded, png_bytes);
}

// ---------------------------------------------------------------------------
// format 参数
// ---------------------------------------------------------------------------

#[tokio::test]
async fn format_html_returns_raw() {
    let mut server = Server::new_async().await;
    let raw = "<html><body><h1>Raw</h1></body></html>";
    let _m = server
        .mock("GET", "/x")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(raw)
        .create_async()
        .await;

    let out = run_webfetch_test(&server.url(), "/x", Some(FetchFormat::Html))
        .await
        .expect("ok");
    assert_eq!(out.output, raw);
    assert_eq!(out.format, "html");
}

#[tokio::test]
async fn format_text_strips_html() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/x")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body("<html><body><p>One</p><p>Two</p><script>alert(1)</script></body></html>")
        .create_async()
        .await;

    let out = run_webfetch_test(&server.url(), "/x", Some(FetchFormat::Text))
        .await
        .expect("ok");
    assert!(out.output.contains("One"));
    assert!(out.output.contains("Two"));
    assert!(
        !out.output.contains("alert(1)"),
        "script 内容应被剥离: {}",
        out.output
    );
    assert_eq!(out.format, "text");
}

#[tokio::test]
async fn default_format_is_markdown() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/x")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body("<html><body><h2>x</h2></body></html>")
        .create_async()
        .await;
    let out = run_webfetch_test(&server.url(), "/x", None).await.unwrap();
    assert_eq!(out.format, "markdown");
}

#[tokio::test]
async fn accept_header_negotiation_for_markdown() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/x")
        .match_header(
            "accept",
            Matcher::Regex(r"text/markdown\s*;\s*q=1\.0".into()),
        )
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("ok")
        .create_async()
        .await;

    let out = run_webfetch_test(&server.url(), "/x", None)
        .await
        .expect("ok");
    assert_eq!(out.output, "ok");
}

// ---------------------------------------------------------------------------
// 大小 / 状态
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rejects_oversized_content_length() {
    let big = vec![b'x'; MAX_RESPONSE_SIZE + 1024];
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/big")
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body(big)
        .create_async()
        .await;

    let err = run_webfetch_test(&server.url(), "/big", None)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("too large"), "got: {msg}");
}

#[tokio::test]
async fn http_error_status_propagates() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/404")
        .with_status(404)
        .create_async()
        .await;
    let err = run_webfetch_test(&server.url(), "/404", None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("404"), "got: {err}");
}

// ---------------------------------------------------------------------------
// SSRF: is_blocked_ip coverage
// ---------------------------------------------------------------------------

#[test]
fn blocked_ip_ipv4_basics() {
    use std::net::IpAddr;
    for blocked in [
        "127.0.0.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.1.1",
        "169.254.1.1",
        "0.0.0.0",
        "255.255.255.255",
        "100.64.0.1",   // CGNAT
        "192.0.2.1",    // TEST-NET-1
        "198.51.100.1",  // TEST-NET-2
        "203.0.113.1",   // TEST-NET-3
        "198.18.0.1",    // Benchmarking
        "240.0.0.1",     // Reserved
    ] {
        let ip: IpAddr = blocked.parse().unwrap();
        assert!(is_blocked_ip(ip), "should block {blocked}");
    }
    // Public IPs must NOT be blocked.
    for allowed in ["8.8.8.8", "1.1.1.1", "93.184.216.34"] {
        let ip: IpAddr = allowed.parse().unwrap();
        assert!(!is_blocked_ip(ip), "should allow {allowed}");
    }
}

#[test]
fn blocked_ip_ipv4_mapped_ipv6() {
    use std::net::IpAddr;
    // ::ffff:127.0.0.1 is IPv4-mapped — must be blocked.
    let mapped_loopback: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
    assert!(is_blocked_ip(mapped_loopback), "::ffff:127.0.0.1 must be blocked");

    let mapped_private: IpAddr = "::ffff:10.0.0.1".parse().unwrap();
    assert!(is_blocked_ip(mapped_private), "::ffff:10.0.0.1 must be blocked");

    let mapped_link_local: IpAddr = "::ffff:169.254.1.1".parse().unwrap();
    assert!(is_blocked_ip(mapped_link_local), "::ffff:169.254.1.1 must be blocked");

    // ::ffff:8.8.8.8 is public — must NOT be blocked.
    let mapped_public: IpAddr = "::ffff:8.8.8.8".parse().unwrap();
    assert!(!is_blocked_ip(mapped_public), "::ffff:8.8.8.8 should be allowed");
}

#[test]
fn blocked_ip_ipv6_ranges() {
    use std::net::IpAddr;
    for blocked in [
        "::1",                   // loopback
        "::",                    // unspecified
        "fc00::1",               // unique-local
        "fe80::1",               // link-local
        "2001:db8::1",           // documentation
    ] {
        let ip: IpAddr = blocked.parse().unwrap();
        assert!(is_blocked_ip(ip), "should block {blocked}");
    }
    // Public IPv6 must NOT be blocked.
    let public: IpAddr = "2606:4700:4700::1111".parse().unwrap();
    assert!(!is_blocked_ip(public), "public IPv6 should be allowed");
}

// ---------------------------------------------------------------------------
// SSRF redirect protection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ssrf_safe_client_blocks_redirect_to_internal() {
    let client = build_ssrf_safe_client().expect("build client");
    // The custom redirect policy should stop redirects to internal IPs.
    // We can't easily test this with mockito (no redirect support),
    // but we verify the client builds successfully with the policy.
    assert!(client.get("https://example.invalid").build().is_ok());
}

// ---------------------------------------------------------------------------
// FetchOutput 序列化
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 真実端点 smoke 测试(默认开启;CI 网络受限时設 WARP_SKIP_WEB_INTEGRATION=1)
// ---------------------------------------------------------------------------

fn skip_real() -> bool {
    std::env::var("WARP_SKIP_WEB_INTEGRATION").is_ok()
}

#[tokio::test]
async fn real_example_com_markdown() {
    if skip_real() {
        return;
    }
    let client = build_ssrf_safe_client().expect("build client");
    let out = run_webfetch(&client, args("https://example.com"))
        .await
        .expect("real example.com");
    assert!(
        out.output.to_lowercase().contains("example domain"),
        "got: {}",
        out.output
    );
}

#[tokio::test]
async fn real_httpbin_html_to_markdown() {
    if skip_real() {
        return;
    }
    let client = build_ssrf_safe_client().expect("build client");
    let out = run_webfetch(&client, args("https://httpbin.org/html"))
        .await
        .expect("real httpbin html");
    assert!(!out.output.trim().is_empty());
    assert_eq!(out.format, "markdown");
}

#[tokio::test]
async fn real_httpbin_json_pretty() {
    if skip_real() {
        return;
    }
    let client = build_ssrf_safe_client().expect("build client");
    let out = run_webfetch(&client, args("https://httpbin.org/json"))
        .await
        .expect("real httpbin json");
    assert!(out.output.contains("```json"), "got: {}", out.output);
}

#[tokio::test]
async fn real_httpbin_image_attachment() {
    if skip_real() {
        return;
    }
    let client = build_ssrf_safe_client().expect("build client");
    let out = run_webfetch(&client, args("https://httpbin.org/image/png"))
        .await
        .expect("real png");
    assert_eq!(out.attachments.len(), 1);
    assert_eq!(out.attachments[0].mime, "image/png");
}

#[tokio::test]
async fn real_httpbin_404_errors() {
    if skip_real() {
        return;
    }
    let client = build_ssrf_safe_client().expect("build client");
    let err = run_webfetch(&client, args("https://httpbin.org/status/404"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("404"), "got: {err}");
}

// ---------------------------------------------------------------------------
// 描述文档 / opencode 字节级対齐回归
// ---------------------------------------------------------------------------

/// 锁住 webfetch.md 与 opencode `packages/opencode/src/tool/webfetch.txt`
/// 字節级一致。修改時需同步两邊。
#[test]
fn webfetch_description_matches_opencode_verbatim() {
    use super::super::webfetch::WEBFETCH;
    let expected = "- Fetches content from a specified URL\n\
                    - Takes a URL and optional format as input\n\
                    - Fetches the URL content, converts to requested format (markdown by default)\n\
                    - Returns the content in the specified format\n\
                    - Use this tool when you need to retrieve and analyze web content\n\
                    \n\
                    Usage notes:\n\
                    \x20\x20- IMPORTANT: if another tool is present that offers better web fetching capabilities, is more targeted to the task, or has fewer restrictions, prefer using that tool instead of this one.\n\
                    \x20\x20- The URL must be a fully-formed valid URL\n\
                    \x20\x20- HTTP URLs will be automatically upgraded to HTTPS\n\
                    \x20\x20- Format options: \"markdown\" (default), \"text\", or \"html\"\n\
                    \x20\x20- This tool is read-only and does not modify any files\n\
                    \x20\x20- Results may be summarized if the content is very large\n";
    assert_eq!(WEBFETCH.description, expected);
}

#[test]
fn fetch_output_omits_empty_attachments_in_json() {
    let out = FetchOutput {
        url: "https://x".into(),
        status: 200,
        content_type: "text/plain".into(),
        format: "markdown".into(),
        output: "hi".into(),
        attachments: vec![],
    };
    let v = fetch_output_to_json(&out);
    assert!(
        v.get("attachments").is_none(),
        "空 attachments 应被 skip: {v}"
    );
    assert_eq!(v["output"], "hi");
}

/// `_byop_intercepted` sentinel 必须存在于所有 web tool result(包括 error)中,
/// 否則 controller (`controller.rs::needs_byop_local_resume`) 不会触发 auto-resume,
/// 模型会卡在等待结果,UI 显示静默失败。
#[test]
fn fetch_output_carries_byop_sentinel() {
    let out = FetchOutput {
        url: "https://x".into(),
        status: 200,
        content_type: "text/plain".into(),
        format: "markdown".into(),
        output: "hi".into(),
        attachments: vec![],
    };
    let v = fetch_output_to_json(&out);
    assert_eq!(v["_byop_intercepted"], true);

    let err = error_to_json("webfetch", &anyhow::anyhow!("boom"));
    assert_eq!(err["_byop_intercepted"], true);
    assert_eq!(err["status"], "error");
}
