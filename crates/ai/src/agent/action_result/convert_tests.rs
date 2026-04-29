use super::*;

#[test]
fn orchestrate_cancelled_converts_to_ignore_error() {
    // Per QUALITY-569 PRODUCT.md §invariants and TECH.md §5: when the user
    // clicks Reject, the client transitions the card to its Cancelled
    // post-action state but emits NOTHING on the wire. The convert path
    // surfaces this by returning `ConvertToAPITypeError::Ignore`, which the
    // request-input layer drops silently. The server's input interceptor
    // synthesizes the generic `ToolCallResult.Cancel` marker on the next
    // user input via `cancelledResultsForIncompleteToolCallsInLastResponse`.
    let result =
        api::request::input::tool_call_result::Result::try_from(OrchestrateActionResult::Cancelled);
    assert!(matches!(result, Err(ConvertToAPITypeError::Ignore)));
}

#[test]
fn orchestrate_launched_preserves_per_agent_outcome_order() {
    // Per PRODUCT.md §invariants: the `launched` result MUST report per-agent
    // outcomes in the same order as the input `agent_run_configs[]`,
    // regardless of which `CreateAgentTask` returned first.
    let result = api::request::input::tool_call_result::Result::try_from(
        OrchestrateActionResult::Launched {
            model_id: "auto".to_string(),
            harness: "oz".to_string(),
            execution_mode: OrchestrateExecutionMode::Local,
            agents: vec![
                OrchestrateAgentOutcomeEntry {
                    name: "alpha".to_string(),
                    outcome: OrchestrateAgentOutcome::Launched {
                        agent_id: "agent-1".to_string(),
                    },
                },
                OrchestrateAgentOutcomeEntry {
                    name: "beta".to_string(),
                    outcome: OrchestrateAgentOutcome::Failed {
                        error: "boom".to_string(),
                    },
                },
                OrchestrateAgentOutcomeEntry {
                    name: "gamma".to_string(),
                    outcome: OrchestrateAgentOutcome::Launched {
                        agent_id: "agent-3".to_string(),
                    },
                },
            ],
        },
    )
    .expect("Launched should convert");

    let api::request::input::tool_call_result::Result::OrchestrateResult(api_result) = result
    else {
        panic!("expected orchestrate result");
    };
    let Some(api::orchestrate_result::Outcome::Launched(launched)) = api_result.outcome else {
        panic!("expected launched outcome");
    };
    assert_eq!(launched.agents.len(), 3);
    assert_eq!(launched.agents[0].name, "alpha");
    assert_eq!(launched.agents[1].name, "beta");
    assert_eq!(launched.agents[2].name, "gamma");
    assert!(matches!(
        launched.agents[0].result,
        Some(api::orchestrate_result::agent_outcome::Result::Launched(_))
    ));
    assert!(matches!(
        launched.agents[1].result,
        Some(api::orchestrate_result::agent_outcome::Result::Failed(_))
    ));
    assert!(matches!(
        launched.agents[2].result,
        Some(api::orchestrate_result::agent_outcome::Result::Launched(_))
    ));
}

#[test]
fn orchestrate_launch_denied_emits_launch_denied_outcome() {
    let result = api::request::input::tool_call_result::Result::try_from(
        OrchestrateActionResult::LaunchDenied,
    )
    .expect("LaunchDenied should convert");
    let api::request::input::tool_call_result::Result::OrchestrateResult(api_result) = result
    else {
        panic!("expected orchestrate result");
    };
    assert!(matches!(
        api_result.outcome,
        Some(api::orchestrate_result::Outcome::LaunchDenied(_))
    ));
}

#[test]
fn orchestrate_failure_emits_failure_outcome_with_error() {
    let result =
        api::request::input::tool_call_result::Result::try_from(OrchestrateActionResult::Failure {
            error: "network drop".to_string(),
        })
        .expect("Failure should convert");
    let api::request::input::tool_call_result::Result::OrchestrateResult(api_result) = result
    else {
        panic!("expected orchestrate result");
    };
    let Some(api::orchestrate_result::Outcome::Failure(failure)) = api_result.outcome else {
        panic!("expected failure outcome");
    };
    assert_eq!(failure.error, "network drop");
}

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
