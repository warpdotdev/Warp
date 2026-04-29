use super::*;

#[test]
fn test_should_hide_ai_block_query_and_header_for_optimistic_prompt() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(should_hide_ai_block_query_and_header(true, true, false));
}

#[test]
fn test_should_not_hide_ai_block_query_and_header_during_replay() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(!should_hide_ai_block_query_and_header(true, true, true));
}

#[test]
fn test_should_not_hide_ai_block_query_and_header_for_untracked_prompt() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(!should_hide_ai_block_query_and_header(false, true, false));
}

#[test]
fn test_should_not_hide_ai_block_query_and_header_outside_shared_session() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(!should_hide_ai_block_query_and_header(true, false, false));
}
