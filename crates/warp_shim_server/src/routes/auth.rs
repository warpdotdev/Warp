use axum::{
    Json, Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};

use crate::{server::ShimState, stubs::rest_payloads};

pub(crate) fn router() -> Router<ShimState> {
    Router::new()
        .route("/client/login", post(client_login))
        .route("/proxy/token", post(firebase_token))
        .route("/proxy/customToken", post(firebase_token))
        .route("/login", get(local_shim_auth_page))
        .route("/login/{*path}", get(local_shim_auth_page))
        .route("/signup", get(local_shim_auth_page))
        .route("/signup/{*path}", get(local_shim_auth_page))
        .route("/upgrade", get(local_shim_auth_page))
        .route("/login_options", get(local_shim_auth_page))
        .route("/login_options/{*path}", get(local_shim_auth_page))
        .route("/link_sso", get(local_shim_auth_page))
}

async fn client_login() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn firebase_token() -> impl IntoResponse {
    (StatusCode::OK, Json(rest_payloads::firebase_token()))
}

async fn local_shim_auth_page() -> Html<String> {
    Html(rest_payloads::local_shim_auth_page())
}
