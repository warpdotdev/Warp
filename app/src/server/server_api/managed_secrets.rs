// OpenWarp:`ManagedSecretsClient` 已本地化为 stub。
// 历史职责:通过 warp.dev 后端 GraphQL 维护"团队/用户托管密钥"——
// CRUD(create/update/delete)、列表(list_secrets)、配置查询
// (get_managed_secret_configs)、按云端任务取密钥(get_task_secrets)、
// 以及给 BYOH cloud agent task 颁发 OIDC 身份令牌
// (issue_task_identity_token,后端再由 AWS STS / GCP STS 兑换 federated
// credentials)。OpenWarp 已切除 cloud agent/任务运行链路,所有云端
// 密钥相关动作无服务端可达,这里全部本地化:
//   - 查询/列表类返回空集合,避免 UI 入口(若残留)崩溃。
//   - 写动作 + 颁发 OIDC token 返回 `disabled in OpenWarp` 错误。
// 保留:`ManagedSecretsClient` trait 路径、模块导出(`ManagedSecretConfigs`、
// `ManagedSecretsClient`),crates/managed_secrets 上层 manager 与 agent_sdk
// 多处直接消费。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use warp_graphql::managed_secrets::{ManagedSecret, ManagedSecretType};
use warp_graphql::queries::task_secrets::ManagedSecretValue;
use warp_managed_secrets::client::{SecretOwner, TaskIdentityToken};

pub use warp_managed_secrets::client::{ManagedSecretConfigs, ManagedSecretsClient};

pub struct DisabledManagedSecretsClient;

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
        _workload_token: String,
    ) -> Result<HashMap<String, ManagedSecretValue>> {
        // BYOP 无云端任务密钥,返回空映射,允许 agent_sdk 路径继续走本地 secrets。
        Ok(HashMap::new())
    }

    async fn issue_task_identity_token(
        &self,
        _options: warp_managed_secrets::client::IdentityTokenOptions,
    ) -> Result<TaskIdentityToken> {
        // OpenWarp 无服务端 OIDC issuer,Bedrock/Vertex federated 凭据路径
        // 直接失败;BYOP AWS 用户走 access key / SSO,与本路径无关。
        Err(anyhow!("Task identity token issuance disabled in OpenWarp"))
    }
}
