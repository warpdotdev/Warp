//! `proxy` 模块的单元测试。
//!
//! reqwest 0.12 没有公开 API 让我们查询 `ClientBuilder` 上已注册的 `Proxy`,
//! 因此这里只能从可观察行为(`apply` 之后构造出的 `Client` 是否成功)上做最小验证。
//! 更细的"实际是否走代理"留给集成测试(需要本地起 mitm)。
//!
//! 注意:reqwest 在 `rustls-tls-native-roots-no-provider` features 下,
//! `.build()` 需要一个全局 crypto provider 已安装,否则 panic。生产代码由
//! `app/src/lib.rs::init_common` 安装,单测进程里需要我们自己装上。

use super::*;
use std::sync::Once;

static INSTALL_CRYPTO_PROVIDER: Once = Once::new();

/// 在运行 reqwest `.build()` 的测试前调用,仅第一次生效。
fn ensure_crypto_provider() {
    INSTALL_CRYPTO_PROVIDER.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

/// 构造一个关闭原生 CA 加载的 builder,避免在难拿到系统证书的环境里 build 失败。
fn test_builder() -> reqwest::ClientBuilder {
    ensure_crypto_provider();
    reqwest::ClientBuilder::new()
        .tls_built_in_native_certs(false)
        .tls_built_in_root_certs(false)
}

#[test]
fn proxy_mode_from_str_lenient_handles_variants() {
    assert_eq!(ProxyMode::from_str_lenient("system"), ProxyMode::System);
    assert_eq!(ProxyMode::from_str_lenient("SYSTEM"), ProxyMode::System);
    assert_eq!(ProxyMode::from_str_lenient("custom"), ProxyMode::Custom);
    assert_eq!(ProxyMode::from_str_lenient("off"), ProxyMode::Off);
    assert_eq!(ProxyMode::from_str_lenient("disabled"), ProxyMode::Off);
    assert_eq!(ProxyMode::from_str_lenient("none"), ProxyMode::Off);
    // 未知值退回 Off,与默认项一致,避免意外走系统代理。
    assert_eq!(ProxyMode::from_str_lenient("wat"), ProxyMode::Off);
}

#[test]
fn proxy_mode_as_str_roundtrip() {
    for mode in [ProxyMode::System, ProxyMode::Custom, ProxyMode::Off] {
        let s = mode.as_str();
        assert_eq!(ProxyMode::from_str_lenient(s), mode);
    }
}

#[test]
fn apply_system_returns_default_builder() {
    let cfg = ProxyConfig {
        mode: ProxyMode::System,
        ..Default::default()
    };
    // 验证不会 panic 且能成功 build。
    let builder = cfg.apply(test_builder()).no_proxy();
    // 上面再叠一个 no_proxy 只是为了避免 build 时去真正解析系统代理;
    // 核心断言是 apply 不 panic。
    let _client = builder.build().expect("System 模式应可成功 build");
}

#[test]
fn apply_off_disables_proxy_without_error() {
    let cfg = ProxyConfig {
        mode: ProxyMode::Off,
        ..Default::default()
    };
    let builder = cfg.apply(test_builder());
    let _client = builder.build().expect("Off 模式应可成功 build");
}

#[test]
fn apply_custom_with_valid_url_succeeds() {
    let cfg = ProxyConfig {
        mode: ProxyMode::Custom,
        url: "http://proxy.corp:8080".to_string(),
        ..Default::default()
    };
    let builder = cfg.apply(test_builder());
    let _client = builder
        .build()
        .expect("Custom 模式 + 合法 URL 应可成功 build");
}

#[test]
fn apply_custom_with_basic_auth_succeeds() {
    let cfg = ProxyConfig {
        mode: ProxyMode::Custom,
        url: "http://proxy.corp:8080".to_string(),
        username: "alice".to_string(),
        password: "s3cret".to_string(),
        ..Default::default()
    };
    let builder = cfg.apply(test_builder());
    let _client = builder.build().expect("Custom + auth 应可成功 build");
}

#[test]
fn apply_custom_with_no_proxy_list_succeeds() {
    let cfg = ProxyConfig {
        mode: ProxyMode::Custom,
        url: "http://proxy.corp:8080".to_string(),
        no_proxy: "localhost,127.0.0.1,.internal".to_string(),
        ..Default::default()
    };
    let builder = cfg.apply(test_builder());
    let _client = builder.build().expect("Custom + no_proxy 应可成功 build");
}

#[test]
fn apply_custom_with_empty_url_falls_back_silently() {
    let cfg = ProxyConfig {
        mode: ProxyMode::Custom,
        url: String::new(),
        ..Default::default()
    };
    // 不该 panic,等价于退回 System(reqwest 默认)。
    let builder = cfg.apply(test_builder()).no_proxy();
    let _client = builder.build().expect("空 URL 应静默退回");
}

#[test]
fn apply_custom_with_invalid_url_falls_back_silently() {
    let cfg = ProxyConfig {
        mode: ProxyMode::Custom,
        url: "://not a url".to_string(),
        ..Default::default()
    };
    let builder = cfg.apply(test_builder()).no_proxy();
    let _client = builder.build().expect("非法 URL 应静默退回");
}

#[test]
fn set_and_read_global_config_roundtrip() {
    // 注意:OnceLock 全局,测试间不能假设隔离;这里只验证 set 之后读到的就是写入的。
    let cfg = ProxyConfig {
        mode: ProxyMode::Custom,
        url: "http://test-proxy:1234".to_string(),
        username: "u".to_string(),
        password: "p".to_string(),
        no_proxy: "a,b".to_string(),
    };
    set_global_proxy_config(cfg.clone());
    let read_back = current_proxy_config();
    assert_eq!(read_back.mode, cfg.mode);
    assert_eq!(read_back.url, cfg.url);
    assert_eq!(read_back.username, cfg.username);
    assert_eq!(read_back.password, cfg.password);
    assert_eq!(read_back.no_proxy, cfg.no_proxy);

    // 重置回默认,避免污染其他测试。
    set_global_proxy_config(ProxyConfig::default());
}
