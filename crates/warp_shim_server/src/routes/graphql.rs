use axum::{
    Json, Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{config::ShimConfig, server::ShimState, stubs::graphql_payloads};

#[derive(Debug, Deserialize)]
pub(crate) struct GraphqlParams {
    op: Option<String>,
}

pub(crate) fn router() -> Router<ShimState> {
    Router::new().route("/graphql/v2", post(post_graphql).get(get_graphql))
}

async fn post_graphql(
    State(state): State<ShimState>,
    Query(params): Query<GraphqlParams>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let op = params
        .op
        .as_deref()
        .and_then(non_empty)
        .map(str::to_string)
        .or_else(|| operation_name_from_body(&body))
        .unwrap_or_default();
    let auth_present = headers.contains_key(axum::http::header::AUTHORIZATION);
    let payload = dispatch(&state.config, &op, &body, auth_present);
    (StatusCode::OK, Json(payload))
}

async fn get_graphql(Query(params): Query<GraphqlParams>) -> impl IntoResponse {
    let op = params.op.as_deref().unwrap_or("<missing>");
    tracing::info!(%op, "received GET for GraphQL endpoint in local shim mode");
    (StatusCode::OK, "warp-shim GraphQL endpoint")
}

fn dispatch(config: &ShimConfig, op: &str, body: &[u8], auth_present: bool) -> Value {
    match op {
        "GetFeatureModelChoices" => graphql_payloads::get_feature_model_choices(config),
        "FreeAvailableModels" => graphql_payloads::free_available_models(config),
        "GetUser" => graphql_payloads::get_user(config),
        "GetUserSettings" => graphql_payloads::get_user_settings(),
        "GetWorkspacesMetadataForUser" => {
            graphql_payloads::get_workspaces_metadata_for_user(config)
        }
        _ => {
            let request_fields = request_field_names(body);
            let logged_op = if op.is_empty() { "<missing>" } else { op };
            tracing::info!(
                op = %logged_op,
                auth_present,
                request_fields = ?request_fields,
                "unsupported GraphQL operation in local shim mode"
            );
            graphql_payloads::unknown_operation()
        }
    }
}

fn operation_name_from_body(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("operationName")?
                .as_str()
                .and_then(non_empty)
                .map(str::to_string)
        })
}

fn request_field_names(body: &[u8]) -> Option<Vec<String>> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .as_object()
                .map(|object| object.keys().cloned().collect())
        })
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}
