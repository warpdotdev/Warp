use super::*;

#[test]
fn llm_info_deserializes_without_base_model_name() {
    let raw = r#"{
            "display_name": "gpt-4o",
            "id": "gpt-4o",
            "usage_metadata": {
                "request_multiplier": 1,
                "credit_multiplier": null
            },
            "description": null,
            "disable_reason": null,
            "vision_supported": false,
            "spec": null,
            "provider": "Unknown"
        }"#;

    let info: LLMInfo = serde_json::from_str(raw).expect("should deserialize");
    assert_eq!(info.display_name, "gpt-4o");
    assert_eq!(info.base_model_name, "gpt-4o");
}

#[test]
fn llm_info_deserializes_host_configs_as_vec() {
    // Wire format from server: host_configs is a Vec
    let raw = r#"{
            "display_name": "gpt-4o",
            "id": "gpt-4o",
            "usage_metadata": { "request_multiplier": 1, "credit_multiplier": null },
            "provider": "OpenAI",
            "host_configs": [
                { "enabled": true, "model_routing_host": "DirectApi" },
                { "enabled": false, "model_routing_host": "AwsBedrock" }
            ]
        }"#;

    let info: LLMInfo = serde_json::from_str(raw).expect("should deserialize vec format");
    assert_eq!(info.display_name, "gpt-4o");
    assert_eq!(info.host_configs.len(), 2);
    assert!(
        info.host_configs
            .get(&LLMModelHost::DirectApi)
            .unwrap()
            .enabled
    );
    assert!(
        !info
            .host_configs
            .get(&LLMModelHost::AwsBedrock)
            .unwrap()
            .enabled
    );
}

#[test]
fn available_harness_models_info_for_id_finds_match() {
    let models = AvailableHarnessModels {
        default_model_id: "opus".to_owned(),
        models: vec![
            HarnessModelInfo {
                id: "opus".to_owned(),
                display_name: "Claude Opus".to_owned(),
            },
            HarnessModelInfo {
                id: "sonnet".to_owned(),
                display_name: "Claude Sonnet".to_owned(),
            },
        ],
    };

    assert_eq!(
        models
            .info_for_id("sonnet")
            .map(|m| m.display_name.as_str()),
        Some("Claude Sonnet"),
    );
    assert!(models.info_for_id("haiku").is_none());
}

#[test]
fn available_harness_models_round_trip_serde() {
    // Cache write/read path serializes to JSON; ensure round-trip preserves values.
    let models = AvailableHarnessModels {
        default_model_id: "opus".to_owned(),
        models: vec![
            HarnessModelInfo {
                id: "opus".to_owned(),
                display_name: "Claude Opus".to_owned(),
            },
            HarnessModelInfo {
                id: "sonnet".to_owned(),
                display_name: "Claude Sonnet".to_owned(),
            },
        ],
    };
    let serialized = serde_json::to_string(&models).expect("should serialize");
    let round_tripped: AvailableHarnessModels =
        serde_json::from_str(&serialized).expect("should deserialize");
    assert_eq!(models, round_tripped);
}

#[test]
fn llm_info_round_trip_serializes_and_deserializes() {
    // Start with wire format (Vec)
    let wire_json = r#"{
            "display_name": "claude-3",
            "base_model_name": "claude-3",
            "id": "claude-3",
            "usage_metadata": { "request_multiplier": 2, "credit_multiplier": 1.5 },
            "description": "A powerful model",
            "vision_supported": true,
            "provider": "Anthropic",
            "host_configs": [
                { "enabled": true, "model_routing_host": "DirectApi" }
            ]
        }"#;

    // Deserialize from wire format
    let info: LLMInfo = serde_json::from_str(wire_json).expect("should deserialize");

    // Serialize (produces HashMap format)
    let serialized = serde_json::to_string(&info).expect("should serialize");

    // Deserialize again (from HashMap format)
    let round_tripped: LLMInfo =
        serde_json::from_str(&serialized).expect("should deserialize after round trip");

    assert_eq!(info, round_tripped);
}
