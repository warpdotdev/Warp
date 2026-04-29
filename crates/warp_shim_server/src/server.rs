use std::{
    future::Future,
    sync::{Arc, Once},
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::State,
    http::{Method, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
};
use tokio::net::TcpListener;
use url::Url;

use crate::{
    config::ShimConfig, conversation::store::ConversationStore, routes, stubs::rest_payloads,
};

#[derive(Clone)]
pub(crate) struct ShimState {
    pub(crate) config: Arc<ShimConfig>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) conversations: ConversationStore,
}

pub async fn serve(config: ShimConfig) -> Result<()> {
    let bind_addr = config.bind_addr();
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind warp shim server to {bind_addr}"))?;

    serve_listener(listener, config, shutdown_signal()).await
}

pub async fn serve_listener<F>(listener: TcpListener, config: ShimConfig, shutdown: F) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let local_addr = listener
        .local_addr()
        .context("failed to read warp shim listener address")?;
    let config = Arc::new(config);
    let default_upstream = config.default_upstream()?;

    tracing::info!(
        bind_addr = %config.bind_addr(),
        %local_addr,
        public_base_url = %config.server.public_base_url,
        config_path = ?config.config_path,
        upstream_url = %upstream_url_for_log(&default_upstream.base_url),
        upstream_timeout_secs = default_upstream.timeout_secs,
        upstream_streaming = default_upstream.streaming,
        upstream_has_api_key = default_upstream.api_key.is_some(),
        upstream_api_key_env_configured = default_upstream.api_key_env.is_some(),
        model_mappings = %config.model_mappings_for_log(),
        tools_enabled = config.features.tools_enabled,
        mcp_tools_enabled = config.features.mcp_tools_enabled,
        passive_suggestions_enabled = config.features.passive_suggestions_enabled,
        "starting warp shim server"
    );

    tracing::info!(%local_addr, "warp shim server listening");

    axum::serve(listener, router(config))
        .with_graceful_shutdown(shutdown)
        .await
        .context("warp shim server failed")
}

pub(crate) fn router(config: Arc<ShimConfig>) -> Router {
    init_tls_provider();

    let state = ShimState {
        config,
        http_client: reqwest::Client::new(),
        conversations: ConversationStore::default(),
    };

    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(healthz))
        .merge(routes::router())
        .fallback(fallback)
        .with_state(state)
}

async fn healthz(State(state): State<ShimState>) -> impl IntoResponse {
    tracing::trace!(bind_addr = %state.config.bind_addr(), "health check");
    (StatusCode::OK, "OK")
}

async fn fallback(method: Method, uri: Uri) -> impl IntoResponse {
    let path = uri.path();
    if method == Method::POST && is_telemetry_like_path(path) {
        tracing::info!(%path, "accepted telemetry-like POST in local shim mode");
        return StatusCode::NO_CONTENT.into_response();
    }

    tracing::info!(%method, %path, "unsupported local shim endpoint");
    (
        StatusCode::NOT_FOUND,
        Json(rest_payloads::unsupported_endpoint()),
    )
        .into_response()
}

pub(crate) fn upstream_url_for_log(url: &Url) -> String {
    let mut sanitized = url.clone();
    let _ = sanitized.set_username("");
    let _ = sanitized.set_password(None);
    sanitized.set_query(None);
    sanitized.set_fragment(None);
    sanitized.to_string()
}

fn init_tls_provider() {
    static TLS_PROVIDER: Once = Once::new();
    TLS_PROVIDER.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

fn is_telemetry_like_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.contains("telemetry")
        || path.contains("metrics")
        || path.contains("analytics")
        || path.contains("event")
        || path.contains("log")
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(%error, "failed to listen for shutdown signal");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_url_for_log_strips_userinfo_query_and_fragment() {
        let url = Url::parse(
            "https://user:super-secret@example.com:8443/v1?api_key=secret&token=secret#frag",
        )
        .unwrap();

        let logged = upstream_url_for_log(&url);

        assert_eq!(logged, "https://example.com:8443/v1");
        assert!(!logged.contains("super-secret"));
        assert!(!logged.contains("api_key"));
        assert!(!logged.contains("token"));
    }
}
