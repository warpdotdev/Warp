use super::{
    decode_file_edits_policy_denied_reason, encode_file_edits_policy_denied_message,
    StartAgentResult, StartAgentVersion, FILE_EDITS_POLICY_DENIED_MARKER,
    FILE_EDITS_POLICY_DENIED_PREFIX,
};

#[test]
fn decodes_file_edit_policy_denial_marker() {
    let message = encode_file_edits_policy_denied_message("protected path");

    assert_eq!(
        decode_file_edits_policy_denied_reason(&message).as_deref(),
        Some("protected path")
    );
}

#[test]
fn file_edit_policy_denial_decoder_rejects_human_prefix() {
    let message = format!("{FILE_EDITS_POLICY_DENIED_PREFIX}protected path");

    assert_eq!(decode_file_edits_policy_denied_reason(&message), None);
}

#[test]
fn file_edit_policy_denial_decoder_rejects_unexpected_fields() {
    let message = serde_json::json!({
        "marker": FILE_EDITS_POLICY_DENIED_MARKER,
        "reason": "protected path",
        "error": "diff failed",
    })
    .to_string();

    assert_eq!(decode_file_edits_policy_denied_reason(&message), None);
}

#[test]
fn deserializes_legacy_start_agent_success_without_version_as_v1() {
    let result: StartAgentResult =
        serde_json::from_value(serde_json::json!({ "Success": { "agent_id": "agent-1" } }))
            .expect("legacy start-agent success should deserialize");

    assert_eq!(
        result,
        StartAgentResult::Success {
            agent_id: "agent-1".to_string(),
            version: StartAgentVersion::V1,
        }
    );
}

#[test]
fn deserializes_legacy_start_agent_error_without_version_as_v1() {
    let result: StartAgentResult =
        serde_json::from_value(serde_json::json!({ "Error": { "error": "boom" } }))
            .expect("legacy start-agent error should deserialize");

    assert_eq!(
        result,
        StartAgentResult::Error {
            error: "boom".to_string(),
            version: StartAgentVersion::V1,
        }
    );
}

#[test]
fn deserializes_legacy_start_agent_cancelled_without_version_as_v1() {
    let result: StartAgentResult = serde_json::from_value(serde_json::json!({ "Cancelled": {} }))
        .expect("legacy start-agent cancellation should deserialize");

    assert_eq!(
        result,
        StartAgentResult::Cancelled {
            version: StartAgentVersion::V1,
        }
    );
}
