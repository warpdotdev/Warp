use super::*;
use remote_server::setup::{RemoteArch, RemoteOs};
use warpui::r#async::BoxFuture;

fn static_auth_context() -> Arc<RemoteServerAuthContext> {
    Arc::new(RemoteServerAuthContext::new(
        || -> BoxFuture<'static, Option<String>> { Box::pin(async { None }) },
        || "user id/with spaces".to_string(),
        String::new(),
        String::new(),
        true,
    ))
}

#[test]
fn remote_proxy_command_quotes_identity_key() {
    let transport = SshTransport::new(
        PathBuf::from("/tmp/control-master.sock"),
        static_auth_context(),
    );

    let command = transport.remote_proxy_command();

    assert!(command.contains("remote-server-proxy --identity-key"));
    assert!(command.contains("'user id/with spaces'"));
}

#[test]
fn detected_platform_cache_is_shared_between_transport_clones() {
    let transport = SshTransport::new(
        PathBuf::from("/tmp/control-master.sock"),
        static_auth_context(),
    );
    let clone = transport.clone();
    let platform = RemotePlatform {
        os: RemoteOs::Linux,
        arch: RemoteArch::X86_64,
    };

    transport.cache_detected_platform(platform.clone());

    assert_eq!(clone.cached_detected_platform(), Some(platform));
}

#[tokio::test]
async fn scp_fallback_reuses_cached_platform_without_uname_probe() {
    let cached = RemotePlatform {
        os: RemoteOs::Linux,
        arch: RemoteArch::X86_64,
    };

    let platform = platform_for_scp_fallback(
        Path::new("/tmp/nonexistent-control-master.sock"),
        Some(cached.clone()),
    )
    .await
    .unwrap();

    assert_eq!(platform, cached);
}
