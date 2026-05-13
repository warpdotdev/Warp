//! This module should houses all horizontal/cross-cutting AI functionality throughout
//! Warp (including Agent Mode).
//!
//! The side panel Warp AI implementation lives in `super::ai_assistant`.
pub(crate) mod agent;
pub(crate) mod agent_conversations_model;
pub(crate) mod agent_events;
pub(crate) mod agent_providers;
pub(crate) mod agent_tips;
pub(crate) mod ai_document_view;
pub mod ambient_agents;
pub(crate) mod api_error;
pub(crate) mod artifact_download;
pub mod artifacts;
pub(crate) mod attachment_utils;
#[cfg(not(target_family = "wasm"))]
pub mod aws_credentials;
pub(crate) mod block_context;
pub(crate) mod blocklist;
pub(crate) mod byop_compaction;
pub mod control_code_parser;
pub(crate) mod conversation_navigation;
pub(crate) mod conversation_status_ui;
pub(crate) mod conversation_utils;
pub(crate) mod document;
pub(crate) mod harness_display;
pub(crate) mod llms;
pub mod onboarding;
pub(crate) mod predict;
pub(crate) mod project_rules_persister;
pub mod request_usage_model;
pub(crate) mod restored_conversations;
pub(crate) mod skills;
pub(crate) mod voice;
pub use agent_tips::*;
pub use request_usage_model::*;
use warpui::AppContext;
#[cfg(not(target_family = "wasm"))]
pub mod agent_sdk;
// OpenWarp Wave 7-3:`cloud_agent_settings` 随 Cloud Mode UI 子系统物理删。
// OpenWarp Wave 7-2:Cloud environments 的 CLI / 表单 / 环境准备链路已删；
// 本地对象数据类型仍暂存于此，供 ObjectStoreModel 反序列化与现有视图过滤使用。
pub mod execution_profiles;
pub mod facts;
// OpenWarp Wave 6-8:`generate_block_title` 随 `BlockClient::generate_shared_block_title`
// stub 一同移除 —— 唯一消费点是 BlockClient trait 签名,本地无其他代码路径。
pub(crate) mod loading;
pub mod mcp;

pub(crate) use ai::paths;

pub fn init(app: &mut AppContext) {
    blocklist::keyboard_navigable_buttons::init(app);
    blocklist::block::number_shortcut_buttons::init(app);
    blocklist::toggleable_items::init(app);
    blocklist::suggested_agent_mode_workflow_modal::init(app);
    blocklist::suggested_rule_modal::init(app);
    ai_document_view::init(app);
}
