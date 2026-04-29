use prost::Message as _;
use warp_multi_agent_api as api;

pub(crate) fn decode_request(bytes: &[u8]) -> Result<api::Request, prost::DecodeError> {
    api::Request::decode(bytes)
}
