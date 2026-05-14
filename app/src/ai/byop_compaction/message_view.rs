//! 把 warp `api::Message` 序列适配为 [`MessageRef`] trait,供 [`super::algorithm`] 操作。
//!
//! ## 与 opencode `MessageV2.WithParts` 的语义映射
//!
//! opencode:一条 user/assistant message 含多个 parts(text/tool/file/...);
//! warp:一条 protobuf `api::Message` 是细粒度的(UserQuery / AgentReasoning / AgentOutput / ToolCall / ToolCallResult 各自独立)。
//!
//! 本投影**一对一**把 warp 的每条 `api::Message` 视为一个 `MessageRef`,
//! turn 检测仍按 user message 边界切 — 一个 user message 后跟连续的非 user message 就是一个 turn。
//! 这不影响 [`super::algorithm::turns`] / [`super::algorithm::select`] 算法的正确性。
//!
//! prune 决策针对 `Role::Tool`(ToolCallResult)— 每条 ToolCallResult 自己是一个候选。
//! 调用方需提前把 conversation 内所有 ToolCall 的 `tool_call_id → tool_name` 索引到 [`ToolNameLookup`]。

use std::collections::HashMap;

use warp_multi_agent_api as api;

use super::algorithm::{MessageRef, Role, ToolOutputRef};
use super::state::CompactionState;

/// `tool_call_id → tool_name` 索引,投影时用于:
/// 1. 给 ToolCallResult 标注 tool_name(用于 PRUNE_PROTECTED_TOOLS 判断)
/// 2. 让 prune 决策跳过 protected 工具(如 `skill`)
pub type ToolNameLookup = HashMap<String, String>;

/// 给定一组 tasks,提取所有 ToolCall 的 `(tool_call_id, tool_name)` 对。
pub fn build_tool_name_lookup<'a, I>(messages: I) -> ToolNameLookup
where
    I: IntoIterator<Item = &'a api::Message>,
{
    let mut out = ToolNameLookup::new();
    for msg in messages {
        if let Some(api::message::Message::ToolCall(tc)) = &msg.message {
            // 直接用 protobuf tool_call.tool 的 enum variant 名
            let name = tool_name_for(tc).unwrap_or_default();
            out.insert(tc.tool_call_id.clone(), name);
        }
    }
    out
}

/// 从 protobuf ToolCall 拿"工具名"。
///
/// 本投影只需要识别 [`PRUNE_PROTECTED_TOOLS`](`super::consts::PRUNE_PROTECTED_TOOLS`) 里的工具
/// (目前只有 "skill",对应 warp 的 `Tool::ReadSkill`),其他工具返回空串 — 在 prune 决策里
/// 空串不匹配任何 protected entry,行为正确(允许被 prune)。
fn tool_name_for(tc: &api::message::ToolCall) -> Option<String> {
    use api::message::tool_call::Tool;
    let t = tc.tool.as_ref()?;
    let s = match t {
        Tool::ReadSkill(_) => "skill",
        _ => "",
    };
    Some(s.to_string())
}

/// 单条 `api::Message` 的视图。
#[derive(Clone, Copy)]
pub struct WarpMessageView<'a> {
    pub msg: &'a api::Message,
    pub state: &'a CompactionState,
    pub tool_names: &'a ToolNameLookup,
}

/// 估算单条 message 的 token 占用 — 累加可见文本字符数 / 4。
fn estimate_message(msg: &api::Message) -> usize {
    use super::token::estimate;
    use api::message::Message as M;
    let chars = msg
        .message
        .as_ref()
        .map(|inner| match inner {
            M::UserQuery(u) => u.query.chars().count(),
            M::AgentOutput(a) => a.text.chars().count(),
            M::AgentReasoning(r) => r.reasoning.chars().count(),
            M::ToolCall(_) => msg.server_message_data.chars().count().max(64),
            M::ToolCallResult(tcr) => {
                // 优先用 result oneof 的 estimate;fallback 用 server_message_data。
                // 简化:都按字符数算,result.estimate 走 Debug repr。
                let from_oneof = tcr
                    .result
                    .as_ref()
                    .map(|r| format!("{r:?}").chars().count())
                    .unwrap_or(0);
                from_oneof
                    .max(msg.server_message_data.chars().count())
                    .max(32)
            }
            _ => 0,
        })
        .unwrap_or(0);
    // 与 opencode 同算法:chars / 4 round。
    estimate(&" ".repeat(chars))
}

impl<'a> MessageRef for WarpMessageView<'a> {
    type Id = String;
    type CallId = String;

    fn id(&self) -> String {
        self.msg.id.clone()
    }

    fn role(&self) -> Role {
        use api::message::Message as M;
        match &self.msg.message {
            Some(M::UserQuery(_)) => Role::User,
            Some(M::ToolCallResult(_)) => Role::Tool,
            // AgentOutput / AgentReasoning / ToolCall / 其他 → Assistant
            _ => Role::Assistant,
        }
    }

    fn is_compaction_marker(&self) -> bool {
        // 只有 user 消息且带 compaction_trigger marker 才算
        if self.role() != Role::User {
            return false;
        }
        self.state
            .marker(&self.msg.id)
            .map(|m| m.compaction_trigger.is_some())
            .unwrap_or(false)
    }

    fn is_summary(&self) -> bool {
        // 只有 assistant message 才能是摘要
        if self.role() != Role::Assistant {
            return false;
        }
        self.state
            .marker(&self.msg.id)
            .map(|m| m.is_summary)
            .unwrap_or(false)
    }

    fn estimate_size(&self) -> usize {
        estimate_message(self.msg)
    }

    fn tool_outputs(&self) -> Vec<ToolOutputRef<String>> {
        let Some(api::message::Message::ToolCallResult(tcr)) = &self.msg.message else {
            return Vec::new();
        };
        let tool_name = self
            .tool_names
            .get(&tcr.tool_call_id)
            .cloned()
            .unwrap_or_default();
        let already_compacted = self
            .state
            .marker(&self.msg.id)
            .and_then(|m| m.tool_output_compacted_at)
            .is_some();
        // output_size 复用 estimate_message — ToolCallResult 路径会走 result/server_message_data 的字符数
        let output_size = estimate_message(self.msg);
        vec![ToolOutputRef {
            call_id: tcr.tool_call_id.clone(),
            tool_name,
            output_size,
            completed: tcr.result.is_some() || !self.msg.server_message_data.is_empty(),
            already_compacted,
        }]
    }
}

/// 把一组 messages 投影成 `Vec<WarpMessageView>`,按 timestamp 升序排序 —
/// 与 [`crate::ai::agent_providers::chat_stream::build_chat_request`] 的排序保持一致。
pub fn project<'a>(
    messages: &'a [&'a api::Message],
    state: &'a CompactionState,
    tool_names: &'a ToolNameLookup,
) -> Vec<WarpMessageView<'a>> {
    let mut sorted: Vec<&api::Message> = messages.to_vec();
    sorted.sort_by_key(|m| {
        m.timestamp
            .as_ref()
            .map(|ts| (ts.seconds, ts.nanos))
            .unwrap_or((0, 0))
    });
    sorted
        .into_iter()
        .map(|msg| WarpMessageView {
            msg,
            state,
            tool_names,
        })
        .collect()
}
