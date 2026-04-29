use std::{collections::BTreeMap, net::Ipv4Addr, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cynic::GraphQlResponse;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tower::ServiceExt;
use url::Url;

use crate::{
    config::{FeatureConfig, ModelMapping, ServerConfig, ShimConfig, UpstreamConfig},
    stubs::graphql_payloads,
};

fn test_config() -> ShimConfig {
    let mut upstreams = BTreeMap::new();
    upstreams.insert(
        "default".to_string(),
        UpstreamConfig {
            base_url: Url::parse("http://127.0.0.1:11434/v1").unwrap(),
            api_key: None,
            api_key_env: None,
            timeout_secs: 180,
            streaming: true,
        },
    );

    let mut models = BTreeMap::new();
    models.insert(
        "auto".to_string(),
        ModelMapping {
            upstream: "default".to_string(),
            model: "llama3.1".to_string(),
        },
    );
    models.insert(
        "cli-agent-auto".to_string(),
        ModelMapping {
            upstream: "default".to_string(),
            model: "llama3.1".to_string(),
        },
    );
    models.insert(
        "coding-auto".to_string(),
        ModelMapping {
            upstream: "default".to_string(),
            model: "qwen2.5-coder".to_string(),
        },
    );
    models.insert(
        "computer-use-agent-auto".to_string(),
        ModelMapping {
            upstream: "default".to_string(),
            model: "llama3.1".to_string(),
        },
    );

    ShimConfig {
        config_path: None,
        server: ServerConfig {
            host: Ipv4Addr::LOCALHOST.into(),
            port: 4444,
            public_base_url: "http://127.0.0.1:4444".to_string(),
        },
        upstreams,
        models,
        features: FeatureConfig::default(),
    }
}

fn decode<T>(value: Value) -> GraphQlResponse<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).unwrap()
}

#[test]
fn get_feature_model_choices_round_trips_through_graphql_response_type() {
    let response: GraphQlResponse<
        warp_graphql::queries::get_feature_model_choices::GetFeatureModelChoices,
    > = decode(graphql_payloads::get_feature_model_choices(&test_config()));
    let data = response.data.unwrap();

    let warp_graphql::queries::get_feature_model_choices::UserResult::UserOutput(output) =
        data.user
    else {
        panic!("expected UserOutput");
    };
    let workspace = output.user.workspaces.into_iter().next().unwrap();
    let choices = workspace.feature_model_choice.agent_mode.choices;

    assert_eq!(workspace.feature_model_choice.agent_mode.default_id, "auto");
    assert_eq!(choices.len(), 4);
    assert!(choices.iter().any(|choice| choice.id == "coding-auto"));
    assert!(matches!(
        choices[0].host_configs[0].model_routing_host,
        warp_graphql::queries::get_feature_model_choices::LlmModelHost::DirectApi
    ));
}

#[test]
fn free_available_models_round_trips_through_graphql_response_type() {
    let response: GraphQlResponse<
        warp_graphql::queries::free_available_models::FreeAvailableModels,
    > = decode(graphql_payloads::free_available_models(&test_config()));
    let data = response.data.unwrap();

    let warp_graphql::queries::free_available_models::FreeAvailableModelsResult::FreeAvailableModelsOutput(output) =
        data.free_available_models
    else {
        panic!("expected FreeAvailableModelsOutput");
    };

    assert_eq!(
        output.feature_model_choice.cli_agent.default_id,
        "cli-agent-auto"
    );
    assert_eq!(output.feature_model_choice.cli_agent.choices.len(), 4);
}

#[test]
fn get_user_round_trips_through_graphql_response_type() {
    let response: GraphQlResponse<warp_graphql::queries::get_user::GetUser> =
        decode(graphql_payloads::get_user(&test_config()));
    let data = response.data.unwrap();

    let warp_graphql::queries::get_user::UserResult::UserOutput(output) = data.user else {
        panic!("expected UserOutput");
    };

    assert_eq!(
        output.principal_type,
        Some(warp_graphql::queries::get_user::PrincipalType::User)
    );
    assert_eq!(output.user.profile.uid, "local-shim-user");
    assert_eq!(
        output.user.profile.email.as_deref(),
        Some("local@warp-shim")
    );
    assert_eq!(output.user.llms.agent_mode.default_id, "auto");
}

#[test]
fn get_user_settings_round_trips_through_graphql_response_type() {
    let response: GraphQlResponse<warp_graphql::queries::get_user_settings::GetUserSettings> =
        decode(graphql_payloads::get_user_settings());
    let data = response.data.unwrap();

    let warp_graphql::queries::get_user_settings::UserResult::UserOutput(output) = data.user else {
        panic!("expected UserOutput");
    };
    let settings = output.user.settings.unwrap();

    assert!(!settings.is_cloud_conversation_storage_enabled);
    assert!(!settings.is_crash_reporting_enabled);
    assert!(!settings.is_telemetry_enabled);
}

#[test]
fn get_workspaces_metadata_for_user_round_trips_through_graphql_response_type() {
    let response: GraphQlResponse<
        warp_graphql::queries::get_workspaces_metadata_for_user::GetWorkspacesMetadataForUser,
    > = decode(graphql_payloads::get_workspaces_metadata_for_user(
        &test_config(),
    ));
    let data = response.data.unwrap();

    let warp_graphql::queries::get_workspaces_metadata_for_user::UserResult::UserOutput(output) =
        data.user
    else {
        panic!("expected UserOutput");
    };
    let workspace = output.user.workspaces.into_iter().next().unwrap();

    assert_eq!(workspace.uid.into_inner(), "local-shim-workspace");
    assert!(workspace.settings.llm_settings.enabled);
    assert_eq!(workspace.settings.llm_settings.host_configs.len(), 1);
    assert!(matches!(
        workspace.settings.llm_settings.host_configs[0].host,
        warp_graphql::workspace::LlmModelHost::DirectApi
    ));
    assert!(
        workspace.settings.llm_settings.host_configs[0]
            .settings
            .enabled
    );

    let warp_graphql::queries::get_workspaces_metadata_for_user::PricingInfoResult::PricingInfoOutput(pricing_output) =
        data.pricing_info
    else {
        panic!("expected PricingInfoOutput");
    };
    assert!(pricing_output.pricing_info.plans.is_empty());
    assert!(pricing_output.pricing_info.addon_credits_options.is_empty());
}

#[tokio::test]
async fn graphql_post_uses_operation_name_from_json_body_when_query_param_is_missing() {
    let response = crate::server::router(Arc::new(test_config()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/graphql/v2")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "operationName": "GetUserSettings",
                        "query": "query GetUserSettings { user { __typename } }"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let decoded: GraphQlResponse<warp_graphql::queries::get_user_settings::GetUserSettings> =
        serde_json::from_slice(&body).unwrap();
    let data = decoded.data.unwrap();
    let warp_graphql::queries::get_user_settings::UserResult::UserOutput(output) = data.user else {
        panic!("expected UserOutput");
    };
    let settings = output.user.settings.unwrap();

    assert!(!settings.is_telemetry_enabled);
}

#[tokio::test]
async fn unknown_graphql_operation_returns_ok_with_null_data() {
    let response = crate::server::router(Arc::new(test_config()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/graphql/v2?op=TotallyUnknown")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"query":"query TotallyUnknown { ping }"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(value, json!({ "data": null }));
}
