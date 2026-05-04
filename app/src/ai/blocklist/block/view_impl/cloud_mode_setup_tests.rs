use super::*;

#[test]
fn test_should_hide_ai_block_query_and_header_for_initial_cloud_prompt() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(should_hide_ai_block_query_and_header(
        true, false, true, true, false
    ));
}

#[test]
fn test_should_hide_ai_block_query_and_header_for_optimistic_followup_prompt() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(should_hide_ai_block_query_and_header(
        false, true, true, false, false
    ));
}

#[test]
fn test_should_not_hide_ai_block_query_and_header_during_replay() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(!should_hide_ai_block_query_and_header(
        true, true, true, true, true
    ));
}

#[test]
fn test_should_not_hide_ai_block_query_and_header_for_untracked_prompt() {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    assert!(!should_hide_ai_block_query_and_header(
        false, false, true, false, false
    ));
}
