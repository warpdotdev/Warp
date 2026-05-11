use super::*;

fn make_manager(keys: ApiKeys) -> ApiKeyManager {
    ApiKeyManager {
        keys,
        aws_credentials_state: AwsCredentialsState::Missing,
        aws_credentials_refresh_strategy: AwsCredentialsRefreshStrategy::default(),
    }
}

fn endpoint(
    name: &str,
    url: &str,
    api_key: &str,
    models: &[(&str, Option<&str>)],
) -> CustomEndpoint {
    CustomEndpoint {
        name: name.into(),
        url: url.into(),
        api_key: api_key.into(),
        models: models
            .iter()
            .map(|(n, a)| CustomEndpointModel {
                name: (*n).into(),
                alias: a.map(|s| s.into()),
            })
            .collect(),
    }
}

// ── serde round-trip ────────────────────────────────────────────

#[test]
fn serde_round_trip_empty() {
    let keys = ApiKeys::default();
    let json = serde_json::to_string(&keys).unwrap();
    let deser: ApiKeys = serde_json::from_str(&json).unwrap();
    assert_eq!(keys, deser);
}

#[test]
fn serde_round_trip_with_provider_keys() {
    let keys = ApiKeys {
        openai: Some("sk-openai".into()),
        anthropic: Some("sk-ant-abc".into()),
        google: Some("AIzaSy123".into()),
        open_router: Some("sk-or-xxx".into()),
        custom_inference: None,
        custom_endpoints: vec![],
    };
    let json = serde_json::to_string(&keys).unwrap();
    let deser: ApiKeys = serde_json::from_str(&json).unwrap();
    assert_eq!(keys, deser);
}

#[test]
fn serde_round_trip_with_custom_endpoints() {
    let keys = ApiKeys {
        openai: None,
        anthropic: None,
        google: None,
        open_router: None,
        custom_inference: None,
        custom_endpoints: vec![
            endpoint("ep1", "https://a.io/v1", "key1", &[("gpt-4", Some("fast"))]),
            endpoint(
                "ep2",
                "https://b.io/v1",
                "key2",
                &[("llama-70b", None), ("mixtral", Some("mix"))],
            ),
        ],
    };
    let json = serde_json::to_string(&keys).unwrap();
    let deser: ApiKeys = serde_json::from_str(&json).unwrap();
    assert_eq!(keys, deser);
}

#[test]
fn serde_round_trip_with_legacy_custom_inference() {
    let keys = ApiKeys {
        custom_inference: Some(CustomInference {
            endpoint: "https://legacy.io".into(),
            model: "legacy-model".into(),
            api_key: "legacy-key".into(),
        }),
        ..Default::default()
    };
    let json = serde_json::to_string(&keys).unwrap();
    let deser: ApiKeys = serde_json::from_str(&json).unwrap();
    assert_eq!(keys, deser);
}

#[test]
fn serde_ignores_unknown_fields() {
    let json = r#"{"openai":"sk-x","unknown_field":"value","custom_endpoints":[]}"#;
    let keys: ApiKeys = serde_json::from_str(json).unwrap();
    assert_eq!(keys.openai, Some("sk-x".into()));
    assert!(keys.custom_endpoints.is_empty());
}

// ── has_any_key ─────────────────────────────────────────────────

#[test]
fn has_any_key_false_when_empty() {
    assert!(!ApiKeys::default().has_any_key());
}

#[test]
fn has_any_key_true_for_openai_only() {
    let keys = ApiKeys {
        openai: Some("sk-x".into()),
        ..Default::default()
    };
    assert!(keys.has_any_key());
}

#[test]
fn has_any_key_true_for_custom_endpoints_only() {
    let keys = ApiKeys {
        custom_endpoints: vec![endpoint("ep", "https://a.io", "key", &[("m", None)])],
        ..Default::default()
    };
    assert!(keys.has_any_key());
}

#[test]
fn has_any_key_false_for_endpoint_with_empty_api_key() {
    let keys = ApiKeys {
        custom_endpoints: vec![endpoint("ep", "https://a.io", "", &[("m", None)])],
        ..Default::default()
    };
    assert!(!keys.has_any_key());
}

#[test]
fn has_any_key_true_for_legacy_custom_inference() {
    let keys = ApiKeys {
        custom_inference: Some(CustomInference {
            endpoint: "".into(),
            model: "".into(),
            api_key: "secret".into(),
        }),
        ..Default::default()
    };
    assert!(keys.has_any_key());
}

// ── has_custom_endpoints ────────────────────────────────────────

#[test]
fn has_custom_endpoints_false_when_empty() {
    assert!(!ApiKeys::default().has_custom_endpoints());
}

#[test]
fn has_custom_endpoints_true_when_present() {
    let keys = ApiKeys {
        custom_endpoints: vec![endpoint("ep", "https://a.io", "k", &[("m", None)])],
        ..Default::default()
    };
    assert!(keys.has_custom_endpoints());
}

// ── user_provided_llm_endpoint_for_request ──────────────────────

#[test]
fn user_provided_llm_endpoint_none_when_empty() {
    let mgr = make_manager(ApiKeys::default());
    assert!(mgr.user_provided_llm_endpoint_for_request(true).is_none());
}

#[test]
fn user_provided_llm_endpoint_none_when_byo_disabled() {
    let mgr = make_manager(ApiKeys {
        custom_endpoints: vec![endpoint("ep", "https://a.io", "k", &[("m", None)])],
        ..Default::default()
    });
    assert!(mgr.user_provided_llm_endpoint_for_request(false).is_none());
}

#[test]
fn user_provided_llm_endpoint_populates_from_endpoint() {
    let mgr = make_manager(ApiKeys {
        custom_endpoints: vec![endpoint(
            "My EP",
            "https://custom.io/v1",
            "ep-key",
            &[("big-model", Some("alias"))],
        )],
        ..Default::default()
    });
    let endpoint = mgr.user_provided_llm_endpoint_for_request(true).unwrap();
    assert_eq!(endpoint.base_url, "https://custom.io/v1");
    assert_eq!(endpoint.model_id, "big-model");
    assert_eq!(endpoint.api_key, "ep-key");
}

#[test]
fn user_provided_llm_endpoint_prefers_legacy_custom_inference_over_endpoint() {
    let mgr = make_manager(ApiKeys {
        custom_inference: Some(CustomInference {
            endpoint: "https://legacy.io".into(),
            model: "legacy-m".into(),
            api_key: "legacy-k".into(),
        }),
        custom_endpoints: vec![endpoint(
            "ep",
            "https://new.io",
            "new-k",
            &[("new-m", None)],
        )],
        ..Default::default()
    });
    let endpoint = mgr.user_provided_llm_endpoint_for_request(true).unwrap();
    assert_eq!(endpoint.base_url, "https://legacy.io");
    assert_eq!(endpoint.model_id, "legacy-m");
}

// ── api_keys_for_request ────────────────────────────────────────

#[test]
fn api_keys_for_request_none_when_empty() {
    let mgr = make_manager(ApiKeys::default());
    assert!(mgr.api_keys_for_request(true, false).is_none());
}

#[test]
fn api_keys_for_request_populates_provider_keys() {
    let mgr = make_manager(ApiKeys {
        openai: Some("sk-o".into()),
        anthropic: Some("sk-a".into()),
        ..Default::default()
    });
    let result = mgr.api_keys_for_request(true, false).unwrap();
    assert_eq!(result.openai, "sk-o");
    assert_eq!(result.anthropic, "sk-a");
    assert!(result.google.is_empty());
}

#[test]
fn api_keys_for_request_omits_keys_when_byo_disabled() {
    let mgr = make_manager(ApiKeys {
        openai: Some("sk-o".into()),
        ..Default::default()
    });
    // With BYO disabled and no other credentials, returns None.
    assert!(mgr.api_keys_for_request(false, false).is_none());
}

#[test]
fn api_keys_for_request_none_for_custom_endpoints_only() {
    let mgr = make_manager(ApiKeys {
        custom_endpoints: vec![endpoint("ep", "https://a.io", "k", &[("m", None)])],
        ..Default::default()
    });
    assert!(mgr.api_keys_for_request(true, false).is_none());
}
