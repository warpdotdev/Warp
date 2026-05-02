use crate::config::{ProviderType, VaultConfig};

#[test]
fn test_parse_valid_config() {
    let toml = r#"
        [provider]
        type = "aws"
        region = "us-east-1"

        [mappings]
        "prod/api-key" = "API_KEY"
        "prod/db-password" = "DB_PASSWORD"
    "#;

    let config: VaultConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.provider.provider_type, ProviderType::Aws);
    assert_eq!(config.provider.region, Some("us-east-1".to_string()));
    assert_eq!(config.mappings.len(), 2);
    assert_eq!(config.mappings.get("prod/api-key").unwrap(), "API_KEY");
}

#[test]
fn test_parse_config_no_region() {
    let toml = r#"
        [provider]
        type = "aws"
    "#;

    let config: VaultConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.provider.region, None);
}

#[test]
fn test_parse_config_empty_mappings() {
    let toml = r#"
        [provider]
        type = "aws"
    "#;

    let config: VaultConfig = toml::from_str(toml).unwrap();
    assert!(config.mappings().is_empty());
}

#[test]
fn test_mappings_returns_correct_pairs() {
    let toml = r#"
        [provider]
        type = "aws"

        [mappings]
        "prod/secret" = "MY_SECRET"
    "#;

    let config: VaultConfig = toml::from_str(toml).unwrap();
    let mappings = config.mappings();
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].path, "prod/secret");
    assert_eq!(mappings[0].env_var, "MY_SECRET");
}

#[test]
fn test_parse_invalid_provider_type() {
    let toml = r#"
        [provider]
        type = "gcp"
    "#;

    let result: Result<VaultConfig, _> = toml::from_str(toml);
    assert!(result.is_err());
}
