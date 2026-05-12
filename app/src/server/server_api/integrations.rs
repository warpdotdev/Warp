// OpenWarp:IntegrationsClient impl 已收敛为仅保留 BYOP OAuth flow 所需的 OAuth
// transaction 状态轮询能力。
//
// 历史职责:通过 GraphQL `create_simple_integration` / `get_simple_integrations` /
// `get_integrations_using_environment` / `suggest_cloud_environment_image` /
// `user_github_info` / `user_repo_auth_status` 与 warp.dev 后端往返,服务于:
//   - 云端 Simple Integration(Linear / Slack 等)CRUD
//   - 云端 cloud agent environment 的 Docker image 建议 + GitHub 仓库授权检查
//   - 设置→Environments 页面拉取用户的 GitHub 已连接仓库
// OpenWarp 不需要这些云端腿,本地用户走 BYOP OAuth provider flow 即可,故:
//   - trait 上仅保留 `poll_oauth_connect_status`(BYOP 本地 OAuth flow,
//     消费点 `app/src/ai/agent_sdk/oauth_flow.rs::poll_oauth_until_terminal`)
//   - 其它方法全部从 trait 删除;原 CLI 入口 (`warp agent integration *`)
//     已在同 PR 物理删除 `app/src/ai/agent_sdk/integration*.rs`
// 相关 GraphQL operation 已在同一 PR 物理删除:
//   crates/graphql/src/api/mutations/create_simple_integration.rs
//   crates/graphql/src/api/queries/{get_simple_integrations,
//     get_integrations_using_environment,suggest_cloud_environment_image,
//     user_github_info,user_repo_auth_status}.rs

use anyhow::{anyhow, Result};
use async_trait::async_trait;

#[cfg(test)]
use mockall::automock;

#[derive(Clone, Copy, Debug)]
pub enum OauthConnectTxStatus {
    Completed,
    Expired,
    Failed,
    InProgress,
    Pending,
}

#[cfg(not(target_family = "wasm"))]
pub trait IntegrationsClientBounds: Send + Sync {}

#[cfg(not(target_family = "wasm"))]
impl<T: 'static + Send + Sync> IntegrationsClientBounds for T {}

#[cfg(target_family = "wasm")]
pub trait IntegrationsClientBounds {}

#[cfg(target_family = "wasm")]
impl<T: 'static> IntegrationsClientBounds for T {}

#[cfg_attr(test, automock)]
#[cfg_attr(target_family = "wasm", allow(dead_code))]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
pub trait IntegrationsClient: 'static + IntegrationsClientBounds {
    /// Polls the status of an OAuth connect transaction.
    ///
    /// OpenWarp:本地环境该路径已不再发起远端轮询,消费方当前保留签名兼容。
    ///
    /// # Arguments
    /// * `tx_id` - The transaction ID returned from the OAuth start request
    ///
    /// # Returns
    /// * `Ok(OauthConnectTxStatus)` - The current status of the transaction
    /// * `Err` - If the transaction is not found or polling fails
    async fn poll_oauth_connect_status(&self, tx_id: String) -> Result<OauthConnectTxStatus>;
}

pub struct DisabledIntegrationsClient;

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
impl IntegrationsClient for DisabledIntegrationsClient {
    async fn poll_oauth_connect_status(&self, _tx_id: String) -> Result<OauthConnectTxStatus> {
        Err(anyhow!(
            "OpenWarp local mode has no cloud OAuth connect polling endpoint"
        ))
    }
}
