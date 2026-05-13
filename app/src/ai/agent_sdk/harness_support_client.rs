// OpenWarp:HarnessSupportClient 已本地化为 stub。
// 历史职责:通过 warp.dev 后端 `/public_api/harness-support/*` REST 端点支撑
// "cloud agent harness"——即在远端机房跑 Claude Code/Gemini/Codex CLI 的 BYOH
// 协议(resolve_prompt / report_artifact / notify_user / finish_task)。OpenWarp 只跑本地
// BYOP harness,无云端 harness 需求。
//
// 保留:
//   - `HarnessSupportClient` trait 本身、仍被调用的方法签名、相关 request/response 数据类型
//     (`ResolvePromptAttachedSkill`、`ResolvePromptRequest`、`ResolvedHarnessPrompt`、
//     `ReportArtifactResponse`):被 agent_sdk/driver/harness/*、agent_sdk/harness_support.rs
//     等多处导入使用,trait 路径不可断。
// 改造:
//   - `DisabledHarnessSupportClient` 所有方法返回
//     "Cloud harness support disabled in OpenWarp" 错误。
//   - 删 conversation/transcript/block snapshot 上传相关云端方法。

#![cfg_attr(target_family = "wasm", expect(dead_code))]

use anyhow::{anyhow, Result};
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

use crate::ai::artifacts::Artifact;

/// Skill attached to a resolve-prompt request,
/// used when invoking a third-party harness with a skill
/// via the CLI.
#[derive(serde::Serialize)]
pub struct ResolvePromptAttachedSkill {
    pub name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ResolvePromptRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<ResolvePromptAttachedSkill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments_dir: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct ResolvedHarnessPrompt {
    pub prompt: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Optional user-turn preamble for resumed third-party harness sessions.
    #[serde(default)]
    pub resumption_prompt: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ReportArtifactResponse {
    pub artifact_uid: String,
}

/// Trait for API endpoints used to support third-party agent harnesses in Oz.
#[cfg_attr(test, automock)]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait HarnessSupportClient: 'static + Send + Sync {
    /// Resolve the prompt for a third-party harness run for a task stored on the server.
    async fn resolve_prompt(&self, request: ResolvePromptRequest) -> Result<ResolvedHarnessPrompt>;

    /// Report an artifact created by a third-party harness back to the Oz platform.
    async fn report_artifact(&self, artifact: &Artifact) -> Result<ReportArtifactResponse>;

    /// Send a progress notification to the task's originating platform.
    async fn notify_user(&self, message: &str) -> Result<()>;

    /// Report task completion or failure.
    async fn finish_task(&self, success: bool, summary: &str) -> Result<()>;
}

pub struct DisabledHarnessSupportClient;

impl DisabledHarnessSupportClient {
    pub fn new() -> Self {
        Self
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl HarnessSupportClient for DisabledHarnessSupportClient {
    async fn resolve_prompt(
        &self,
        _request: ResolvePromptRequest,
    ) -> Result<ResolvedHarnessPrompt> {
        Err(anyhow!("Cloud harness support disabled in OpenWarp"))
    }

    async fn report_artifact(&self, _artifact: &Artifact) -> Result<ReportArtifactResponse> {
        Err(anyhow!("Cloud harness support disabled in OpenWarp"))
    }

    async fn notify_user(&self, _message: &str) -> Result<()> {
        Err(anyhow!("Cloud harness support disabled in OpenWarp"))
    }

    async fn finish_task(&self, _success: bool, _summary: &str) -> Result<()> {
        Err(anyhow!("Cloud harness support disabled in OpenWarp"))
    }
}
