//! Telemetry events for the in-app notification mailbox / toast stack.
//!
//! 这是 002ce467 cloud-removal 时一并删掉的 `AgentManagementTelemetryEvent` 的最小裁剪版,
//! 仅保留通知中心(`item_rendering.rs`)实际仍在用的 variant —— artifact 点击事件 +
//! tombstone 已经不存在但保留 schema 以维持向后兼容/未来重建。

use serde::Serialize;

/// 通知 artifact 类型(用于 telemetry)。
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Plan,
    Branch,
    PullRequest,
}

/// 通知中心相关的 telemetry 事件。
#[derive(Serialize, Debug)]
pub enum NotificationsTelemetryEvent {
    /// 用户在通知项里点击了 artifact 按钮(plan / branch / PR)
    ArtifactClicked { artifact_type: ArtifactType },
}
