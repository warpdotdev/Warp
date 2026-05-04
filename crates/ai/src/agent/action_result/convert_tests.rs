use super::*;

#[test]
fn ask_user_question_skipped_by_auto_approve_converts_to_skipped_answers() {
    let result = api::request::input::tool_call_result::Result::from(
        AskUserQuestionResult::SkippedByAutoApprove {
            question_ids: vec!["q1".to_string(), "q2".to_string()],
        },
    );

    let api::request::input::tool_call_result::Result::AskUserQuestion(result) = result else {
        panic!("expected ask_user_question result");
    };

    let Some(api::ask_user_question_result::Result::Success(success)) = result.result else {
        panic!("expected success result");
    };

    assert_eq!(success.answers.len(), 2);
    assert_eq!(success.answers[0].question_id, "q1");
    assert_eq!(success.answers[1].question_id, "q2");
    assert!(matches!(
        success.answers[0].answer,
        Some(AskUserQuestionAnswer::Skipped(()))
    ));
    assert!(matches!(
        success.answers[1].answer,
        Some(AskUserQuestionAnswer::Skipped(()))
    ));
}

#[test]
fn policy_denied_shell_result_preserves_policy_reason_without_denylist_label() {
    let result = api::request::input::tool_call_result::Result::try_from(
        RequestCommandOutputResult::PolicyDenied {
            command: "rm -rf target".to_string(),
            reason: "blocked by org policy".to_string(),
        },
    )
    .unwrap();

    let api::request::input::tool_call_result::Result::RunShellCommand(result) = result else {
        panic!("expected run_shell_command result");
    };
    let Some(api::run_shell_command_result::Result::PermissionDenied(permission_denied)) =
        result.result
    else {
        panic!("expected permission_denied result");
    };

    assert_eq!(result.command, "rm -rf target");
    #[allow(deprecated)]
    let output = &result.output;
    assert_eq!(
        decode_command_policy_denied_reason(output).as_deref(),
        Some("blocked by org policy")
    );
    assert!(!output.starts_with(COMMAND_POLICY_DENIED_PREFIX));
    assert!(permission_denied.reason.is_none());
}

#[test]
fn policy_denied_file_edit_result_converts_to_policy_marker_message() {
    let result = api::request::input::tool_call_result::Result::try_from(
        RequestFileEditsResult::PolicyDenied {
            reason: "protected path".to_string(),
        },
    )
    .unwrap();

    let api::request::input::tool_call_result::Result::ApplyFileDiffs(result) = result else {
        panic!("expected apply_file_diffs result");
    };
    let Some(api::apply_file_diffs_result::Result::Error(error)) = result.result else {
        panic!("expected error result");
    };

    assert_eq!(
        decode_file_edits_policy_denied_reason(&error.message).as_deref(),
        Some("protected path")
    );
    assert!(!error.message.starts_with(FILE_EDITS_POLICY_DENIED_PREFIX));
}

#[test]
fn policy_denied_write_to_shell_result_converts_to_policy_marker() {
    let result = api::request::input::tool_call_result::Result::try_from(
        WriteToLongRunningShellCommandResult::PolicyDenied {
            reason: "interactive write blocked".to_string(),
        },
    )
    .unwrap();

    let api::request::input::tool_call_result::Result::WriteToLongRunningShellCommand(result) =
        result
    else {
        panic!("expected write_to_long_running_shell_command result");
    };
    let Some(api::write_to_long_running_shell_command_result::Result::CommandFinished(finished)) =
        result.result
    else {
        panic!("expected command_finished result");
    };

    assert_eq!(finished.command_id, WRITE_TO_SHELL_POLICY_DENIED_COMMAND_ID);
    assert_eq!(finished.exit_code, WRITE_TO_SHELL_POLICY_DENIED_EXIT_CODE);
    assert_eq!(
        finished.output,
        "Write to long-running shell command blocked by host policy: interactive write blocked"
    );
}
