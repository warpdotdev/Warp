// OpenWarp:HarnessSupportClient impl 已本地化为 stub。
// 历史职责:通过 warp.dev 后端 `/public_api/harness-support/*` REST 端点支撑
// "cloud agent harness"——即在远端机房跑 Claude Code/Gemini/Codex CLI 的 BYOH
// 协议(resolve_prompt / report_artifact / notify_user / finish_task /
// get_snapshot_upload_targets)。OpenWarp 只跑本地 BYOP harness,无云端 harness 需求。
//
// 保留:
//   - `HarnessSupportClient` trait 本身、仍被调用的方法签名、相关 request/response 数据类型
//     (`UploadTarget`、`SnapshotUploadRequest`、`SnapshotFileInfo`、`SnapshotUploadResponse`、
//     `ResolvePromptAttachedSkill`、`ResolvePromptRequest`、`ResolvedHarnessPrompt`、
//     `ReportArtifactResponse`):被 agent_sdk/driver/{snapshot,harness/*}、agent_sdk/harness_support.rs
//     等多处导入使用,trait 路径不可断。
//   - 顶层 `upload_to_target` 包装:维持公共 API 供 agent_sdk/driver/snapshot.rs
//     调用,内部委托给 presigned_upload(同样已 stub 返回错误)。
// 改造:
//   - `DisabledHarnessSupportClient` 所有方法返回
//     "Cloud harness support disabled in OpenWarp" 错误。
//   - 删 conversation/transcript/block snapshot 上传相关云端方法。

#![cfg_attr(target_family = "wasm", expect(dead_code))]

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

use crate::ai::artifacts::Artifact;

/// A presigned upload target returned by the server.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadTarget {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
}

/// Request body for upload-snapshot upload targets.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotUploadRequest {
    pub files: Vec<SnapshotFileInfo>,
}

/// Describes a single file in a snapshot upload request.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotFileInfo {
    pub filename: String,
    pub mime_type: String,
}

/// Response from the upload-snapshot endpoint.
///
/// The `uploads` list is aligned by index with the [`SnapshotUploadRequest::files`]
/// list in the request, so callers match each upload target back to the filename
/// they requested by position.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SnapshotUploadResponse {
    pub uploads: Vec<UploadTarget>,
}

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

    /// Get presigned upload targets for a workspace state snapshot.
    async fn get_snapshot_upload_targets(
        &self,
        request: &SnapshotUploadRequest,
    ) -> Result<Vec<UploadTarget>>;

    /// Get an HTTP client to use with [`UploadTarget`]s for saving blobs.
    fn http_client(&self) -> &http_client::Client;
}

pub struct DisabledHarnessSupportClient {
    client: http_client::Client,
}

impl DisabledHarnessSupportClient {
    pub fn new() -> Self {
        Self {
            client: http_client::Client::new(),
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            client: http_client::Client::new_for_test(),
        }
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

    async fn get_snapshot_upload_targets(
        &self,
        _request: &SnapshotUploadRequest,
    ) -> Result<Vec<UploadTarget>> {
        Err(anyhow!("Cloud harness support disabled in OpenWarp"))
    }

    fn http_client(&self) -> &http_client::Client {
        &self.client
    }
}

/// Upload a blob to a presigned upload target.
///
/// OpenWarp:转发到已 stub 化的 `presigned_upload::upload_to_target`,返回
/// "Presigned upload disabled in OpenWarp" 错误。保留入口以维持 agent_sdk
/// 内 `snapshot.rs` 对 `harness_support::upload_to_target` 的 import 路径。
pub async fn upload_to_target(
    http_client: &http_client::Client,
    target: &UploadTarget,
    body: impl Into<reqwest::Body>,
) -> Result<()> {
    super::presigned_upload::upload_to_target(http_client, target, body).await
}

#[cfg(test)]
#[path = "harness_support_tests.rs"]
mod tests;
