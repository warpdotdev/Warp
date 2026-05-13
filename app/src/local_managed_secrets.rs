//! OpenWarp 本地 managed-secrets client。
//!
//! Warp 上游这里原本通过 server_api 调云端接口维护团队/用户托管密钥。
//! OpenWarp 保留 `warp_managed_secrets` crate 供本地功能复用,但所有云端托管密钥
//! 动作都不可达:查询返回空集合,写动作和 OIDC token 颁发返回 disabled 错误。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use warp_managed_secrets::client::{
    ManagedSecretConfigs, ManagedSecretsClient, SecretOwner, TaskIdentityToken,
};
use warp_managed_secrets::{ManagedSecret, ManagedSecretType, ManagedSecretValue};

pub(crate) struct DisabledManagedSecretsClient;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ManagedSecretsClient for DisabledManagedSecretsClient {
    async fn get_managed_secret_configs(&self) -> Result<ManagedSecretConfigs> {
        Ok(ManagedSecretConfigs {
            user_secrets: None,
            team_secrets: HashMap::new(),
        })
    }

    async fn create_managed_secret(
        &self,
        _owner: SecretOwner,
        _name: String,
        _secret_type: ManagedSecretType,
        _encrypted_value: String,
        _description: Option<String>,
    ) -> Result<ManagedSecret> {
        Err(anyhow!("Cloud managed secrets disabled in OpenWarp"))
    }

    async fn delete_managed_secret(&self, _owner: SecretOwner, _name: String) -> Result<()> {
        Err(anyhow!("Cloud managed secrets disabled in OpenWarp"))
    }

    async fn update_managed_secret(
        &self,
        _owner: SecretOwner,
        _name: String,
        _encrypted_value: Option<String>,
        _description: Option<String>,
    ) -> Result<ManagedSecret> {
        Err(anyhow!("Cloud managed secrets disabled in OpenWarp"))
    }

    async fn list_secrets(&self) -> Result<Vec<ManagedSecret>> {
        Ok(Vec::new())
    }

    async fn get_task_secrets(
        &self,
        _task_id: String,
    ) -> Result<HashMap<String, ManagedSecretValue>> {
        Ok(HashMap::new())
    }

    async fn issue_task_identity_token(
        &self,
        _options: warp_managed_secrets::client::IdentityTokenOptions,
    ) -> Result<TaskIdentityToken> {
        Err(anyhow!("Task identity token issuance disabled in OpenWarp"))
    }
}
