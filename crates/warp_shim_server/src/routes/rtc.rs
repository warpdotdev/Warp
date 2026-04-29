use std::convert::Infallible;

use axum::{
    Router,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use futures_util::{Stream, stream};

use crate::server::ShimState;

pub(crate) fn router() -> Router<ShimState> {
    Router::new().route("/api/v1/agent/events/stream", get(idle_agent_events_stream))
}

async fn idle_agent_events_stream() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    tracing::info!("opened idle local shim agent events stream");
    Sse::new(stream::pending::<Result<Event, Infallible>>()).keep_alive(KeepAlive::default())
}
