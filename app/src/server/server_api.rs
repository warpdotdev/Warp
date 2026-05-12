pub mod ai;
// OpenWarp Wave 3-1:`server_api/auth.rs`(AuthClient trait + impl)整文件物理删,
// `AuthManager` 改为本地 stub。两个 HTTP header 常量直接迁入本文件,供 ambient agent
// 路径继续使用(实际运行时永远不命中,因 OpenWarp 已无云端 ambient workload)。
// OpenWarp Wave 6-8:`server_api/block.rs`(BlockClient trait + impl)与
// `server_api/referral.rs`(ReferralsClient trait + impl)整文件物理删 —— 两个
// trait 全部 stub Err / 空列表,对应的 `ShowBlocksView` / `ReferralsPageView`
// 设置页一并移除。
pub mod harness_support;
pub mod integrations;
pub mod managed_secrets;
pub(crate) mod presigned_upload;
// OpenWarp(Wave 3-2):`team` / `workspace` 两个 client trait 与 impl 已物理删,
// 在 app/ 外 0 消费,UserWorkspaces / TeamUpdateManager 已在 Phase 5 本地化为 no-op。

use crate::ai::ambient_agents::AmbientAgentTaskId;
use warpui::ModelContext;

// OpenWarp Wave 5-3:原 `AMBIENT_WORKLOAD_TOKEN_HEADER` 随 `generate_multi_agent_output` 云端
// SSE 路径 stub 化后在全仓库 0 消费,物理删。`get_or_create_ambient_workload_token`
// 在 W3-1 后永返 `None`,代码中不再有 header 注入点。

use crate::settings_view;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use instant::Instant;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use warp_core::errors::{AnyhowErrorExt, ErrorExt};
use warp_core::register_error;
use warpui::Entity;
use warpui::SingletonEntity;

use super::experiments::ServerExperiment;
use super::experiments::ServerExperiments;
pub const FETCH_CHANNEL_VERSIONS_TIMEOUT: std::time::Duration = Duration::from_secs(60);

// openWarp 闭源遥测剥离 P4b:`X-Warp-Experiment-Id` HTTP header 原本携带 anonymous_id
// 注入到 GraphQL 等请求,服务端用于实验分组 +
// 跨会话追踪。P0 后值已是 nil-UUID,P4b 直接删 header 注入,服务端见到的请求里就
// 不再有这个字段。注入点(共 3 处)同步删除。

/// We use a special error code header `X-Warp-Error-Code` to allow the server to send
/// more specific error code information, so that the client can discern between different
/// errors with the same error code.
/// See errors/http_error_codes.go on the server for possible values.
const WARP_ERROR_CODE_HEADER: &str = "X-Warp-Error-Code";

/// An error indicating the user is out of credits. The server sends 429s to communicate this
/// state, but if Cloud Run is overloaded, it can also send 429s that aren't credit-related.
/// So we use this to distinguish between the two cases.
const WARP_ERROR_CODE_OUT_OF_CREDITS: &str = "OUT_OF_CREDITS";

/// Error code indicating the user has reached their cloud agent concurrency limit.
const WARP_ERROR_CODE_AT_CAPACITY: &str = "AT_CLOUD_AGENT_CAPACITY";

/// ResponseType received by Client
#[derive(thiserror::Error, Debug, Serialize, Deserialize)]
#[error("{error}")]
pub struct ClientError {
    pub error: String,
    // We unconditionally check for GitHub auth errors in any public API response. It'd be much better
    // to have the server return error codes that we can parse, but this isn't yet supported.
    // See REMOTE-666
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,
}

/// Error when the user is at their cloud agent concurrency limit.
#[derive(thiserror::Error, Debug, Clone, Deserialize)]
#[error("{error} (running agents: {running_agents})")]
pub struct CloudAgentCapacityError {
    pub error: String,
    pub running_agents: i32,
}

// OpenWarp Wave 5-3:`TimeResponse` 随云端 `/current_time` GET 接口 stub 化后 0 消费,物理删。

#[derive(Debug, Clone)]
pub struct ServerTime {
    time_at_fetch: DateTime<FixedOffset>,
    fetched_at: Instant,
}

impl ServerTime {
    pub fn local_now() -> Self {
        Self {
            time_at_fetch: chrono::Utc::now().into(),
            fetched_at: Instant::now(),
        }
    }

    pub fn current_time(&self) -> DateTime<FixedOffset> {
        let elapsed = chrono::Duration::from_std(self.fetched_at.elapsed())
            .expect("duration should not be bigger than limit");
        self.time_at_fetch + elapsed
    }
}

/// Wrapper for deserialization errors. This covers both:
/// * Using `serde` directly
/// * Using `reqwest` decoding utilities
#[derive(thiserror::Error, Debug)]
pub enum DeserializationError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Transport(reqwest::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum AIApiError {
    #[error("Request failed due to lack of AI quota.")]
    QuotaLimit,

    #[error("Warp is currently overloaded. Please try again later.")]
    ServerOverloaded,

    #[error("Internal error occurred at transport layer.")]
    Transport(#[source] reqwest::Error),

    #[error("Failed to deserialize API response.")]
    Deserialization(#[source] DeserializationError),

    #[error("No context found on context search.")]
    NoContextFound,

    #[error("Failed with status code {0}: {1}")]
    ErrorStatus(http::StatusCode, String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error("Got error when streaming {stream_type}: {source:#}")]
    Stream {
        stream_type: &'static str,
        #[source]
        source: anyhow::Error,
    },
}

impl From<http_client::ResponseError> for AIApiError {
    fn from(err: http_client::ResponseError) -> Self {
        Self::from_response_error(err.source, &err.headers)
    }
}

impl From<reqwest::Error> for AIApiError {
    fn from(err: reqwest::Error) -> Self {
        Self::from_transport_error(err)
    }
}

impl From<serde_json::Error> for AIApiError {
    fn from(err: serde_json::Error) -> Self {
        AIApiError::Deserialization(err.into())
    }
}

impl AIApiError {
    /// Converts a reqwest error to an AIApiError, using response headers to distinguish
    /// between different types of 429 errors.
    fn from_response_error(err: reqwest::Error, headers: &::http::HeaderMap) -> Self {
        // For HTTP 429 errors, check the X-Warp-Error-Code header to distinguish
        // between out-of-credits and server-overload.
        if err.status() == Some(http::StatusCode::TOO_MANY_REQUESTS) {
            return Self::error_for_429(headers);
        }

        Self::from_transport_error(err)
    }

    /// Converts a transport-level reqwest error (no HTTP response) to an AIApiError.
    fn from_transport_error(err: reqwest::Error) -> Self {
        // Unfortunately, `reqwest` reports some non-decoding errors as decoding errors (e.g.
        // unexpected disconnects or timeouts while deserializing a response body). Since we
        // render deserialization and transport errors differently, we try to detect those cases
        // here.
        if err.is_timeout() {
            return AIApiError::Transport(err);
        }
        if err.is_decode() {
            #[cfg(not(target_family = "wasm"))]
            {
                use std::error::Error as _;
                let mut source = err.source();
                while let Some(underlying) = source {
                    if underlying.is::<hyper::Error>() {
                        return AIApiError::Transport(err);
                    }

                    source = underlying.source();
                }
            }

            return AIApiError::Deserialization(DeserializationError::Transport(err));
        }

        AIApiError::Transport(err)
    }

    /// Returns the appropriate error for a 429 response by checking the X-Warp-Error-Code header.
    fn error_for_429(headers: &::http::HeaderMap) -> Self {
        if headers
            .get(WARP_ERROR_CODE_HEADER)
            .and_then(|v| v.to_str().ok())
            == Some(WARP_ERROR_CODE_OUT_OF_CREDITS)
        {
            AIApiError::QuotaLimit
        } else {
            AIApiError::ServerOverloaded
        }
    }

    /// Format a stream error into a human-readable error message. This will read the response
    /// body if there is one.
    async fn from_stream_error(stream_type: &'static str, err: reqwest_eventsource::Error) -> Self {
        match err {
            reqwest_eventsource::Error::InvalidStatusCode(
                http::StatusCode::TOO_MANY_REQUESTS,
                ref res,
            ) => Self::error_for_429(res.headers()),
            reqwest_eventsource::Error::InvalidStatusCode(status, res) => Self::ErrorStatus(
                status,
                res.text()
                    .await
                    .unwrap_or_else(|e| format!("(no response body: {e:#})")),
            ),
            reqwest_eventsource::Error::Transport(err) => Self::from_transport_error(err),
            err => AIApiError::Stream {
                stream_type,
                // On WASM, `reqwest_eventsource::Error` doesn't implement `Into<anyhow::Error>` or
                // `Send` because it may contain a `wasm_bindgen` JS value.
                #[cfg(target_family = "wasm")]
                source: anyhow!("{err:#?}"),
                #[cfg(not(target_family = "wasm"))]
                source: anyhow!(err),
            },
        }
    }

    /// Returns whether or not the error can be retried.
    pub fn is_retryable(&self) -> bool {
        // Don't retry client errors, except for timeouts and quota limits.
        fn is_retryable_status(status: http::StatusCode) -> bool {
            !status.is_client_error()
                || status == http::StatusCode::REQUEST_TIMEOUT
                || status == http::StatusCode::TOO_MANY_REQUESTS
        }

        match self {
            AIApiError::ErrorStatus(status, _) => is_retryable_status(*status),
            AIApiError::Transport(e) => {
                if let Some(status) = e.status() {
                    return is_retryable_status(status);
                }
                true
            }
            // By default, retry on error.
            _ => true,
        }
    }
}

impl ErrorExt for AIApiError {
    fn is_actionable(&self) -> bool {
        match self {
            AIApiError::Deserialization(_) => true,
            AIApiError::Transport(error) => error.is_actionable(),
            AIApiError::Other(error) => error.is_actionable(),
            AIApiError::Stream { source, .. } => source.is_actionable(),
            AIApiError::ErrorStatus(_, _) => self.is_retryable(),
            AIApiError::QuotaLimit | AIApiError::ServerOverloaded | AIApiError::NoContextFound => {
                false
            }
        }
    }
}
register_error!(AIApiError);

#[derive(thiserror::Error, Debug)]
pub enum TranscribeError {
    #[error("Request failed due to lack of Voice quota.")]
    QuotaLimit,

    #[error("Warp is currently overloaded. Please try again later.")]
    ServerOverloaded,

    #[error("Internal error occurred at transport layer.")]
    Transport,

    #[error("Failed to deserialize JSON.")]
    Deserialization,

    /// OpenWarp 已禁用语音转写(BYOP genai 协议无法承载音频)。
    #[error("Voice transcription is unavailable in OpenWarp.")]
    Disabled,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// OpenWarp 仍保留的本地 server API 窄接口:
/// - 复用长生命周期 HTTP client
/// - 管理 ambient task header 上下文
///
/// 这两项被 BYOP/本地 harness 继续使用,但不再向调用方暴露整颗云 RPC 壳。
pub trait LocalServerApiClient: 'static + Send + Sync {
    fn set_ambient_agent_task_id(&self, task_id: Option<AmbientAgentTaskId>);
    fn http_client(&self) -> &http_client::Client;
}

impl<T> LocalServerApiClient for Arc<T>
where
    T: LocalServerApiClient + ?Sized,
{
    fn set_ambient_agent_task_id(&self, task_id: Option<AmbientAgentTaskId>) {
        self.as_ref().set_ambient_agent_task_id(task_id);
    }

    fn http_client(&self) -> &http_client::Client {
        self.as_ref().http_client()
    }
}

struct OpenWarpLocalServerApiClient {
    client: http_client::Client,
    ambient_agent_task_id: RwLock<Option<AmbientAgentTaskId>>,
}

impl OpenWarpLocalServerApiClient {
    fn new(_agent_source: Option<ai::AgentSource>) -> Self {
        Self {
            client: http_client::Client::new(),
            ambient_agent_task_id: RwLock::new(None),
        }
    }

    #[cfg(test)]
    fn new_for_test() -> Self {
        Self {
            client: http_client::Client::new_for_test(),
            ambient_agent_task_id: RwLock::new(None),
        }
    }
}

impl LocalServerApiClient for OpenWarpLocalServerApiClient {
    fn set_ambient_agent_task_id(&self, task_id: Option<AmbientAgentTaskId>) {
        *self.ambient_agent_task_id.write() = task_id;
    }

    fn http_client(&self) -> &http_client::Client {
        &self.client
    }
}

/// OpenWarp 下仍需保留的本地 agent 事件流入口。
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
pub trait AgentEventStreamClient: 'static + Send + Sync {
    async fn stream_agent_events(
        &self,
        run_ids: &[String],
        since_sequence: i64,
    ) -> Result<http_client::EventSourceStream>;
}

pub struct DisabledAgentEventStreamClient;

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
impl AgentEventStreamClient for DisabledAgentEventStreamClient {
    async fn stream_agent_events(
        &self,
        _run_ids: &[String],
        _since_sequence: i64,
    ) -> Result<http_client::EventSourceStream> {
        Err(anyhow!(
            "Cloud agent event stream disabled in OpenWarp — RTC endpoint is removed"
        ))
    }
}

/// A singleton entity that provides access to the few server-facing facades still retained in OpenWarp.
pub struct ServerApiProvider {
    local_client: Arc<OpenWarpLocalServerApiClient>,
    harness_support_client: Arc<harness_support::DisabledHarnessSupportClient>,
    agent_event_stream_client: Arc<DisabledAgentEventStreamClient>,
}

impl ServerApiProvider {
    /// Constructs a new ServerApiProvider.
    pub fn new(agent_source: Option<ai::AgentSource>) -> Self {
        Self {
            local_client: Arc::new(OpenWarpLocalServerApiClient::new(agent_source)),
            harness_support_client: Arc::new(harness_support::DisabledHarnessSupportClient::new()),
            agent_event_stream_client: Arc::new(DisabledAgentEventStreamClient),
        }
    }

    /// Handles fetching server-side experiments by updating the appropriate app state.
    pub fn handle_experiments_fetched(
        &self,
        experiments: Vec<ServerExperiment>,
        ctx: &mut ModelContext<Self>,
    ) {
        ServerExperiments::handle(ctx).update(ctx, |state, ctx| {
            state.apply_latest_state(experiments, ctx);
        });

        settings_view::handle_experiment_change(ctx);
    }

    /// Constructs a new SeverApiProvider for tests.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            local_client: Arc::new(OpenWarpLocalServerApiClient::new_for_test()),
            harness_support_client: Arc::new(
                harness_support::DisabledHarnessSupportClient::new_for_test(),
            ),
            agent_event_stream_client: Arc::new(DisabledAgentEventStreamClient),
        }
    }

    /// 返回 BYOP/本地 harness 仍需的最小本地接口:
    /// ambient task header 上下文 + 共享 HTTP client。
    pub fn get_local_client(&self) -> Arc<dyn LocalServerApiClient> {
        self.local_client.clone()
    }

    /// 兼容仍未迁出的本地 transport 调用点。新增代码应优先使用窄接口。
    pub fn get_harness_support_client(&self) -> Arc<dyn harness_support::HarnessSupportClient> {
        self.harness_support_client.clone()
    }

    pub fn get_agent_event_stream_client(&self) -> Arc<dyn AgentEventStreamClient> {
        self.agent_event_stream_client.clone()
    }
}

impl Entity for ServerApiProvider {
    type Event = ();
}

impl SingletonEntity for ServerApiProvider {}
