use super::*;
use std::collections::HashMap;

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

#[test]
fn open_router_model_maps_to_llm_info() {
    let info = OpenRouterModel {
        id: "anthropic/claude-sonnet-4.5".to_string(),
        name: Some("Claude Sonnet 4.5".to_string()),
        description: Some("OpenRouter hosted model".to_string()),
        architecture: Some(OpenRouterArchitecture {
            input_modalities: vec!["text".to_string(), "image".to_string()],
        }),
    }
    .into_llm_info();

    assert_eq!(info.id.to_string(), "anthropic/claude-sonnet-4.5");
    assert_eq!(info.display_name, "Claude Sonnet 4.5");
    assert_eq!(info.provider, LLMProvider::OpenRouter);
    assert!(info.vision_supported);
    assert!(info
        .host_configs
        .get(&LLMModelHost::DirectApi)
        .is_some_and(|config| config.enabled));
}

#[test]
fn merge_open_router_models_replaces_existing_open_router_choices() {
    let mut models_by_feature = ModelsByFeature::default();
    models_by_feature.agent_mode.choices.push(LLMInfo {
        display_name: "Old OpenRouter Model".to_string(),
        base_model_name: "Old OpenRouter Model".to_string(),
        id: "old/openrouter".to_string().into(),
        reasoning_level: None,
        usage_metadata: LLMUsageMetadata {
            request_multiplier: 1,
            credit_multiplier: None,
        },
        description: None,
        disable_reason: None,
        vision_supported: false,
        spec: None,
        provider: LLMProvider::OpenRouter,
        host_configs: HashMap::new(),
        discount_percentage: None,
    });

    let replacement = OpenRouterModel {
        id: "openai/gpt-4o".to_string(),
        name: Some("GPT-4o".to_string()),
        description: None,
        architecture: None,
    }
    .into_llm_info();

    merge_open_router_models(&mut models_by_feature, vec![replacement]);

    assert!(models_by_feature
        .agent_mode
        .choices
        .iter()
        .any(|info| info.id.to_string() == "openai/gpt-4o"));
    assert!(models_by_feature
        .agent_mode
        .choices
        .iter()
        .all(|info| info.id.to_string() != "old/openrouter"));
}
