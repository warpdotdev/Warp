use super::ChannelState;

// OpenWarp Wave 5-5：`derive_http_origin_from_ws_url` 调用 + 3 个 wss/ws 路径测试随
// `ChannelState::rtc_http_url()` 一同物理删。

/// `ChannelState::init()` (the static default for OSS builds) must satisfy
/// the cloud-disabled predicate; the cloud-removal plan's Phase 5 short-circuit
/// depends on this invariant.
#[test]
fn default_oss_state_is_cloud_disabled() {
    assert!(ChannelState::is_cloud_disabled());
}
