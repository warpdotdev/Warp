use anyhow::Result;

use crate::config::SecretMapping;
use crate::provider::{Secret, SecretProvider};

pub async fn inject_secrets(
    provider: &dyn SecretProvider,
    mappings: &[SecretMapping],
) -> Result<Vec<Secret>> {
    let mut secrets = Vec::new();
    for mapping in mappings {
        let secret = provider.fetch(&mapping.path, &mapping.env_var).await?;
        unsafe {
            std::env::set_var(&secret.env_var, secret.value());
        }
        secrets.push(secret);
    }
    Ok(secrets)
}
