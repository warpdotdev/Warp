use anyhow::{Context, Result};
use async_trait::async_trait;
use aws_sdk_secretsmanager::Client;

use super::{Secret, SecretProvider};

pub struct AwsProvider {
    client: Client,
}

impl AwsProvider {
    pub async fn new(region: Option<String>) -> Result<Self> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(region) = region {
            config_loader = config_loader.region(aws_config::Region::new(region));
        }
        let config = config_loader.load().await;
        Ok(Self {
            client: Client::new(&config),
        })
    }
}

#[async_trait]
impl SecretProvider for AwsProvider {
    async fn fetch(&self, path: &str, env_var: &str) -> Result<Secret> {
        let response = self
            .client
            .get_secret_value()
            .secret_id(path)
            .send()
            .await
            .with_context(|| format!("vault: secret '{path}' not found"))?;

        let value = response
            .secret_string()
            .context("vault: secret has no string value")?
            .to_string();

        Ok(Secret::new(env_var.to_string(), value))
    }
}
