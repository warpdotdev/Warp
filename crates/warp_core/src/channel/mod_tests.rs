use super::Channel;

#[test]
fn server_url_override_policy_matches_channel_matrix() {
    let cases = [
        (Channel::Stable, false),
        (Channel::Preview, false),
        (Channel::Dev, true),
        (Channel::Local, true),
        (Channel::Oss, true),
        (Channel::Integration, true),
    ];

    for (channel, expected) in cases {
        assert_eq!(
            channel.allows_server_url_overrides(),
            expected,
            "unexpected override policy for {channel:?}"
        );
    }
}
