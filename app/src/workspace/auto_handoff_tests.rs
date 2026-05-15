use super::{
    AutoCloudHandoffEligibility, AutoCloudHandoffSkipReason, AutoCloudHandoffTriggerSettings,
};
use crate::workspace::AutoCloudHandoffTrigger;

fn eligibility() -> AutoCloudHandoffEligibility {
    AutoCloudHandoffEligibility {
        is_empty: false,
        is_in_progress: true,
        has_server_conversation_token: true,
        is_viewing_shared_session: false,
        already_attempted: false,
    }
}

#[test]
fn eligible_running_synced_conversation_is_not_skipped() {
    assert_eq!(eligibility().skip_reason(), None);
}

#[test]
fn auto_handoff_skips_empty_conversations() {
    let eligibility = AutoCloudHandoffEligibility {
        is_empty: true,
        ..eligibility()
    };

    assert_eq!(
        eligibility.skip_reason(),
        Some(AutoCloudHandoffSkipReason::EmptyConversation)
    );
}

#[test]
fn auto_handoff_skips_idle_conversations() {
    let eligibility = AutoCloudHandoffEligibility {
        is_in_progress: false,
        ..eligibility()
    };

    assert_eq!(
        eligibility.skip_reason(),
        Some(AutoCloudHandoffSkipReason::NotInProgress)
    );
}

#[test]
fn auto_handoff_skips_unsynced_conversations() {
    let eligibility = AutoCloudHandoffEligibility {
        has_server_conversation_token: false,
        ..eligibility()
    };

    assert_eq!(
        eligibility.skip_reason(),
        Some(AutoCloudHandoffSkipReason::MissingServerConversationToken)
    );
}

#[test]
fn auto_handoff_skips_shared_session_viewers() {
    let eligibility = AutoCloudHandoffEligibility {
        is_viewing_shared_session: true,
        ..eligibility()
    };

    assert_eq!(
        eligibility.skip_reason(),
        Some(AutoCloudHandoffSkipReason::SharedSessionViewer)
    );
}

#[test]
fn auto_handoff_skips_already_attempted_conversations() {
    let eligibility = AutoCloudHandoffEligibility {
        already_attempted: true,
        ..eligibility()
    };

    assert_eq!(
        eligibility.skip_reason(),
        Some(AutoCloudHandoffSkipReason::AlreadyAttempted)
    );
}

fn trigger_settings(
    cloud_handoff_enabled: bool,
    auto_handoff_on_sleep_enabled: bool,
) -> AutoCloudHandoffTriggerSettings {
    AutoCloudHandoffTriggerSettings {
        cloud_handoff_enabled,
        auto_handoff_on_sleep_enabled,
    }
}

#[test]
fn sleep_trigger_requires_auto_handoff_on_sleep_setting() {
    let settings = trigger_settings(true, false);

    assert!(!settings.is_enabled_for(AutoCloudHandoffTrigger::MacOsSleep));
}

#[test]
fn uri_trigger_requires_auto_handoff_on_sleep_setting() {
    let settings = trigger_settings(true, false);
    assert!(!settings.is_enabled_for(AutoCloudHandoffTrigger::Uri));
}

#[test]
fn triggers_are_enabled_when_cloud_handoff_and_auto_handoff_on_sleep_are_enabled() {
    let settings = trigger_settings(true, true);

    assert!(settings.is_enabled_for(AutoCloudHandoffTrigger::MacOsSleep));
    assert!(settings.is_enabled_for(AutoCloudHandoffTrigger::Uri));
}

#[test]
fn triggers_require_cloud_handoff_setting() {
    let settings = trigger_settings(false, true);

    assert!(!settings.is_enabled_for(AutoCloudHandoffTrigger::MacOsSleep));
    assert!(!settings.is_enabled_for(AutoCloudHandoffTrigger::Uri));
}
