use axum::Router;

use crate::server::ShimState;

pub(crate) mod ai;
pub(crate) mod auth;
pub(crate) mod graphql;
pub(crate) mod public_api;
pub(crate) mod rtc;

pub(crate) fn router() -> Router<ShimState> {
    Router::new()
        .merge(ai::router())
        .merge(graphql::router())
        .merge(auth::router())
        .merge(public_api::router())
        .merge(rtc::router())
}
