use super::{StartAgentResult, StartAgentVersion};

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
