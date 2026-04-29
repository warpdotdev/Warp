use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{server::ShimState, stubs::rest_payloads};

pub(crate) fn router() -> Router<ShimState> {
    Router::new()
        .route("/api/v1/agent/run", post(spawn_agent_unsupported))
        .route("/api/v1/agent/runs", get(list_agent_runs))
        .route("/api/v1/agent/runs/{id}", get(agent_run_not_found))
        .route(
            "/api/v1/agent/runs/{id}/conversation",
            get(agent_run_conversation_not_found),
        )
        .route("/api/v1/agent/events", get(list_agent_events))
        .route("/api/v1/agent/events/{run_id}", post(report_agent_event))
        .route(
            "/api/v1/harness-support/external-conversation",
            post(create_external_conversation),
        )
        .route(
            "/api/v1/harness-support/transcript",
            post(upload_target).get(fetch_transcript_not_found),
        )
        .route(
            "/api/v1/harness-support/block-snapshot",
            post(upload_target),
        )
        .route(
            "/api/v1/harness-support/upload-snapshot",
            post(upload_snapshot_targets),
        )
        .route(
            "/api/v1/harness-support/upload/{id}",
            put(discard_upload).post(discard_upload),
        )
        .route(
            "/api/v1/harness-support/resolve-prompt",
            post(resolve_prompt),
        )
        .route(
            "/api/v1/harness-support/report-artifact",
            post(report_artifact),
        )
        .route("/api/v1/harness-support/notify-user", post(no_content))
        .route("/api/v1/harness-support/finish-task", post(no_content))
        .route("/api/v1/harness-support/{*path}", post(harness_fallback))
}

async fn spawn_agent_unsupported() -> impl IntoResponse {
    tracing::info!("unsupported local shim endpoint: cloud-agent spawn");
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(rest_payloads::error(
            "unsupported in local shim: cloud agents are not available",
        )),
    )
}

async fn list_agent_runs() -> impl IntoResponse {
    (StatusCode::OK, Json(rest_payloads::list_runs()))
}

async fn agent_run_not_found(Path(id): Path<String>) -> impl IntoResponse {
    tracing::info!(%id, "local shim cloud-agent run not found");
    (
        StatusCode::NOT_FOUND,
        Json(rest_payloads::error("run not found in local shim")),
    )
}

async fn agent_run_conversation_not_found(Path(id): Path<String>) -> impl IntoResponse {
    tracing::info!(%id, "local shim cloud-agent run conversation not found");
    (
        StatusCode::NOT_FOUND,
        Json(rest_payloads::error(
            "run conversation not found in local shim",
        )),
    )
}

async fn list_agent_events() -> impl IntoResponse {
    (StatusCode::OK, Json(rest_payloads::list_agent_events()))
}

async fn report_agent_event(Path(run_id): Path<String>) -> impl IntoResponse {
    tracing::info!(%run_id, "accepted local shim cloud-agent event report");
    (StatusCode::OK, Json(rest_payloads::report_agent_event()))
}

async fn create_external_conversation() -> impl IntoResponse {
    (StatusCode::OK, Json(rest_payloads::external_conversation()))
}

async fn upload_target(State(state): State<ShimState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(rest_payloads::upload_target(
            &state.config.server.public_base_url,
        )),
    )
}

async fn upload_snapshot_targets(State(state): State<ShimState>, body: Bytes) -> impl IntoResponse {
    let file_count = serde_json::from_slice::<SnapshotUploadRequest>(&body)
        .map(|request| request.files.len())
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(rest_payloads::snapshot_uploads(
            &state.config.server.public_base_url,
            file_count,
        )),
    )
}

async fn discard_upload(Path(id): Path<String>, body: Bytes) -> impl IntoResponse {
    tracing::info!(%id, bytes = body.len(), "discarded local shim harness upload");
    StatusCode::NO_CONTENT
}

async fn resolve_prompt() -> impl IntoResponse {
    (StatusCode::OK, Json(rest_payloads::resolved_prompt()))
}

async fn report_artifact() -> impl IntoResponse {
    (StatusCode::OK, Json(rest_payloads::report_artifact()))
}

async fn no_content() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn fetch_transcript_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(rest_payloads::error("no transcript in local shim")),
    )
}

async fn harness_fallback(uri: Uri) -> impl IntoResponse {
    tracing::info!(path = %uri.path(), "accepted unknown local shim harness-support POST");
    (StatusCode::OK, Json(Value::Object(Default::default())))
}

#[derive(Deserialize)]
struct SnapshotUploadRequest {
    #[serde(default)]
    files: Vec<Value>,
}
