use crate::api_keys::{ApiKeys, CustomApiEndpoint, ProviderType};

fn create_test_endpoint(id: &str, name: &str, url: &str) -> CustomApiEndpoint {
    CustomApiEndpoint {
        id: id.to_string(),
        name: name.to_string(),
        url: url.to_string(),
        api_key: Some("test-key".to_string()),
        provider_type: ProviderType::OpenAI,
        models: vec!["gpt-4".to_string()],
    }
}

#[test]
fn test_custom_endpoint_creation() {
    let endpoint = create_test_endpoint("test-id", "Test Endpoint", "https://api.test.com/v1");

    assert_eq!(endpoint.id, "test-id");
    assert_eq!(endpoint.name, "Test Endpoint");
    assert_eq!(endpoint.url, "https://api.test.com/v1");
    assert_eq!(endpoint.provider_type, ProviderType::OpenAI);
    assert!(endpoint.api_key.is_some());
    assert_eq!(endpoint.models.len(), 1);
}

#[test]
fn test_provider_type_default() {
    let provider_type = ProviderType::default();
    assert_eq!(provider_type, ProviderType::OpenAI);
}

#[test]
fn test_api_keys_empty_custom_endpoints() {
    let keys = ApiKeys::default();
    assert!(keys.custom_endpoints.is_empty());
}

#[test]
fn test_api_keys_with_custom_endpoints() {
    let endpoint1 = create_test_endpoint("id1", "Endpoint 1", "https://api1.com/v1");
    let endpoint2 = create_test_endpoint("id2", "Endpoint 2", "https://api2.com/v1");

    let keys = ApiKeys {
        google: None,
        anthropic: None,
        openai: Some("openai-key".to_string()),
        open_router: None,
        custom_endpoints: vec![endpoint1, endpoint2],
    };

    assert!(keys.has_any_key());
    assert_eq!(keys.custom_endpoints.len(), 2);
}

#[test]
fn test_api_keys_has_any_key_with_custom_endpoint_only() {
    let endpoint = create_test_endpoint("id1", "Endpoint 1", "https://api1.com/v1");

    let keys = ApiKeys {
        google: None,
        anthropic: None,
        openai: None,
        open_router: None,
        custom_endpoints: vec![endpoint],
    };

    assert!(keys.has_any_key());
}

#[test]
fn test_api_keys_serialization_roundtrip() {
    let endpoint = create_test_endpoint("id1", "Test Endpoint", "https://api.test.com/v1");

    let keys = ApiKeys {
        google: Some("google-key".to_string()),
        anthropic: Some("anthropic-key".to_string()),
        openai: Some("openai-key".to_string()),
        open_router: None,
        custom_endpoints: vec![endpoint],
    };

    let json = serde_json::to_string(&keys).expect("Failed to serialize");
    let deserialized: ApiKeys = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(keys, deserialized);
}

#[test]
fn test_api_keys_backward_compatibility_missing_custom_endpoints() {
    // Old format without custom_endpoints field should deserialize with empty vec
    let old_json = r#"{
        "google": null,
        "anthropic": "anthropic-key",
        "openai": null,
        "open_router": null
    }"#;

    let keys: ApiKeys = serde_json::from_str(old_json).expect("Failed to deserialize old format");

    assert!(keys.custom_endpoints.is_empty());
    assert_eq!(keys.anthropic, Some("anthropic-key".to_string()));
}

#[test]
fn test_custom_endpoint_provider_type_serialization() {
    let endpoint_openai = CustomApiEndpoint {
        id: "1".to_string(),
        name: "OpenAI Endpoint".to_string(),
        url: "https://api.openai.com/v1".to_string(),
        api_key: None,
        provider_type: ProviderType::OpenAI,
        models: vec![],
    };

    let endpoint_anthropic = CustomApiEndpoint {
        id: "2".to_string(),
        name: "Anthropic Endpoint".to_string(),
        url: "https://api.anthropic.com/v1".to_string(),
        api_key: None,
        provider_type: ProviderType::Anthropic,
        models: vec![],
    };

    let endpoint_custom = CustomApiEndpoint {
        id: "3".to_string(),
        name: "Custom Endpoint".to_string(),
        url: "https://custom.llm.com/v1".to_string(),
        api_key: None,
        provider_type: ProviderType::Custom,
        models: vec![],
    };

    // Test serialization
    let json_openai = serde_json::to_string(&endpoint_openai.provider_type).unwrap();
    assert_eq!(json_openai, "\"OpenAI\"");

    let json_anthropic = serde_json::to_string(&endpoint_anthropic.provider_type).unwrap();
    assert_eq!(json_anthropic, "\"Anthropic\"");

    let json_custom = serde_json::to_string(&endpoint_custom.provider_type).unwrap();
    assert_eq!(json_custom, "\"Custom\"");

    // Test deserialization
    let deserialized: ProviderType = serde_json::from_str(&json_openai).unwrap();
    assert_eq!(deserialized, ProviderType::OpenAI);
}
