use anyhow::Result;

use crate::config::SecretMapping;
use crate::provider::{Secret, SecretProvider};

pub async fn fetch_secrets(
    provider: &dyn SecretProvider,
    mappings: &[SecretMapping],
) -> Result<Vec<Secret>> {
    let mut secrets = Vec::new();
    for mapping in mappings {
        let secret = provider.fetch(&mapping.path, &mapping.env_var).await?;
        secrets.push(secret);
    }
    Ok(secrets)
}
