use axum::response::sse::Event;
use base64::{Engine as _, prelude::BASE64_URL_SAFE};
use prost::Message as _;
use warp_multi_agent_api as api;

pub(crate) fn encode_response_event_data(event: &api::ResponseEvent) -> String {
    BASE64_URL_SAFE.encode(event.encode_to_vec())
}

pub(crate) fn encode_response_event_for_sse(event: &api::ResponseEvent) -> Event {
    Event::default().data(encode_response_event_data(event))
}

#[cfg(test)]
pub(crate) fn decode_response_event_data_like_client(
    data: &str,
) -> anyhow::Result<api::ResponseEvent> {
    let decoded = BASE64_URL_SAFE.decode(data.trim_matches('"'))?;
    Ok(api::ResponseEvent::decode(decoded.as_slice())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::response_builder::ResponseBuilder;

    #[test]
    fn response_event_data_round_trips_through_client_decode_logic() {
        let event = ResponseBuilder::new(
            "conversation-1".to_string(),
            "request-1".to_string(),
            "run-1".to_string(),
        )
        .finished_success(None);

        let encoded = encode_response_event_data(&event);
        let decoded = decode_response_event_data_like_client(&encoded).unwrap();

        assert_eq!(decoded, event);
    }
}
