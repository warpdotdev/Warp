//! `webfetch` BYOP 工具 descriptor。
//!
//! 实际 HTTP 执行在 `web_runtime::run_webfetch`。本 descriptor 提供给 genai SDK
//! 用于把 tool 描述发给上游 LLM(name + description + JSON Schema)。
//!
//! ## 不走 protobuf executor
//!
//! `from_args` 永远返回 `Err("intercepted at byop layer")`,因为 `chat_stream::
//! parse_incoming_tool_call` 之前会按 name 命中并直接调 `web_runtime`。`result_to_json`
//! 同理永远返回 `None`(没有对应的 protobuf result variant)。这两个 stub 函数仅
//! 满足 `OpenAiTool` 结构体的字段约束。
//!
//! 参数 schema 与 opencode `webfetch.ts:12-20` 对齐。

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use warp_multi_agent_api as api;

use super::OpenAiTool;

pub const TOOL_NAME: &str = "webfetch";

fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "url": {
                "type": "string",
                "description": "The URL to fetch content from. Must use HTTPS (https://)."
            },
            "format": {
                "type": "string",
                "enum": ["markdown", "text", "html"],
                "description": "Output format. 'markdown' (default) converts HTML to Markdown. 'text' strips formatting. 'html' returns the raw HTML.",
                "default": "markdown"
            },
            "timeout": {
                "type": "integer",
                "description": "Optional timeout in seconds. Default 30, capped at 120.",
                "minimum": 1,
                "maximum": 120
            }
        },
        "required": ["url"],
        "additionalProperties": false
    })
}

fn from_args(_args: &str) -> Result<api::message::tool_call::Tool> {
    Err(anyhow!(
        "webfetch is intercepted by chat_stream BYOP web tool dispatcher; \
         from_args should never be called"
    ))
}

fn result_to_json(_result: &api::message::tool_call_result::Result) -> Option<Value> {
    None
}

pub static WEBFETCH: OpenAiTool = OpenAiTool {
    name: TOOL_NAME,
    description: include_str!("../prompts/tool_descriptions/webfetch.md"),
    parameters,
    from_args,
    result_to_json,
};
