use super::{TaskStatusErrorCode, TaskStatusMessage};

#[test]
fn task_status_error_code_deserializes_public_api_casing() {
    let message: TaskStatusMessage = serde_json::from_str(
        "{\"message\":\"setup failed\",\"error_code\":\"environment_setup_failed\"}",
    )
    .unwrap();

    assert_eq!(
        message.error_code,
        Some(TaskStatusErrorCode::EnvironmentSetupFailed)
    );
    assert!(message.is_environment_setup_failure());
}

#[test]
fn task_status_error_code_deserializes_graphql_casing() {
    let message: TaskStatusMessage = serde_json::from_str(
        "{\"message\":\"setup failed\",\"errorCode\":\"ENVIRONMENT_SETUP_FAILED\"}",
    )
    .unwrap();

    assert_eq!(
        message.error_code,
        Some(TaskStatusErrorCode::EnvironmentSetupFailed)
    );
    assert!(message.is_environment_setup_failure());
}

#[test]
fn task_status_error_code_deserializes_unknown_codes() {
    let message: TaskStatusMessage =
        serde_json::from_str("{\"message\":\"failed\",\"error_code\":\"new_error\"}").unwrap();

    assert_eq!(message.error_code, Some(TaskStatusErrorCode::Unknown));
    assert!(!message.is_environment_setup_failure());
    assert!(message.should_hide_continue_actions());
}

#[test]
fn platform_error_codes_hide_continue_actions() {
    let codes = [
        "authentication_required",
        "budget_exceeded",
        "conflict",
        "content_policy_violation",
        "environment_setup_failed",
        "external_authentication_required",
        "feature_not_available",
        "insufficient_credits",
        "integration_disabled",
        "integration_not_configured",
        "internal_error",
        "invalid_request",
        "not_authorized",
        "operation_not_supported",
        "resource_not_found",
        "resource_unavailable",
        "infrastructure_timeout",
        "agent_process_failed",
    ];

    for code in codes {
        let json = format!("{{\"message\":\"failed\",\"error_code\":\"{code}\"}}");
        let message: TaskStatusMessage = serde_json::from_str(&json).unwrap();

        assert!(message.should_hide_continue_actions(), "code {code}");
    }
}

#[test]
fn platform_error_codes_deserialize_graphql_casing() {
    let message: TaskStatusMessage = serde_json::from_str(
        "{\"message\":\"capacity full\",\"errorCode\":\"RESOURCE_UNAVAILABLE\"}",
    )
    .unwrap();

    assert_eq!(
        message.error_code,
        Some(TaskStatusErrorCode::ResourceUnavailable)
    );
    assert!(message.should_hide_continue_actions());
}
