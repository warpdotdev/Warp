//! BYOP 模式下 chat completion + tool calling 适配层(基于 genai 0.5.3)。
//!
//! 把 `RequestParams` 翻译为 genai `ChatRequest`,通过 `Client::exec_chat_stream`
//! 调用用户配置的 provider,响应翻译回 `warp_multi_agent_api::ResponseEvent`,
//! controller 自家逻辑(权限/弹窗/执行/result 回写/触发下一轮)接管闭环。
//!
//! ## 5 种 API 协议显式路由
//!
//! 不再把所有 provider 当作 OpenAI 兼容硬塞,通过 `ServiceTargetResolver` 把
//! 用户在 settings UI 选的 `AgentProviderApiType` 一对一映射到 genai 的 `AdapterKind`:
//!
//! | ApiType        | AdapterKind  | 默认 endpoint                                  |
//! |----------------|--------------|------------------------------------------------|
//! | OpenAi         | OpenAI       | https://api.openai.com/v1                      |
//! | OpenAiResp     | OpenAIResp   | https://api.openai.com/v1 (走 /v1/responses)   |
//! | Gemini         | Gemini       | https://generativelanguage.googleapis.com/v1beta |
//! | Anthropic      | Anthropic    | https://api.anthropic.com                      |
//! | Ollama         | Ollama       | http://localhost:11434                         |
//!
//! 用户填的 `base_url` 始终覆盖默认。这样:
//! - DeepSeek / SiliconFlow / OpenRouter 等 OpenAI 兼容 provider 选 `OpenAi`,自定义 base_url
//! - 显式选定 adapter 完全绕过 genai 的"按模型名识别"默认行为,避免误识别
//!
//! ## 多轮 message 转换
//!
//! - system prompt: `ChatRequest::with_system()`(不进 messages 数组)
//! - user query: `ChatMessage::user(text)`
//! - assistant text: `ChatMessage::assistant(text)`
//! - assistant tool_calls: `ChatMessage::from(Vec<ToolCall>)`(自动 assistant role)
//! - tool result: `ChatMessage::from(ToolResponse::new(call_id, content))`(自动 tool role)
//!
//! ## 流式实现
//!
//! `Client::exec_chat_stream` 返回 `ChatStreamResponse`,其 `stream` 字段实现了
//! `futures_core::Stream<Item = Result<ChatStreamEvent>>`。事件:
//! - `Start` / `Chunk(text)` / `ReasoningChunk(text)` / `ToolCallChunk(tool_call)` / `End(StreamEnd)`
//!
//! 我们对 Chunk/ReasoningChunk 立即 emit `AppendToMessageContent`(打字机效果),
//! 对 ToolCallChunk 累积 buffer(按 call_id),流末统一 emit `Message::ToolCall`,
//! controller 自动接管。

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use instant::Instant;
use serde_json::{json, Value};
use uuid::Uuid;
use warp_multi_agent_api as api;

use genai::adapter::AdapterKind;
use genai::chat::{
    Binary, CacheControl, ChatMessage, ChatOptions, ChatRequest, ChatRole, ChatStreamEvent,
    ContentPart, MessageContent, Tool as GenaiTool, ToolCall, ToolResponse,
};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget, WebConfig};

use crate::ai::agent::api::{RequestParams, ResponseStream};
use crate::ai::agent::{AIAgentInput, RunningCommand, UserQueryMode};
use crate::ai::api_error::AIApiError;
use crate::ai::byop_compaction;
use crate::settings::AgentProviderApiType;
use ai::agent::convert::ConvertToAPITypeError;

use super::openai_compatible::OpenAiCompatibleError;
use super::tools;

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------
// system prompt 由 `prompt_renderer::render_system` 通过 minijinja 模板生成,
// 按 LLMId 模型族选 system/{anthropic,gpt,beast,gemini,kimi,codex,trinity,default}.j2,
// 并把 warp 客户端已经收集好的 AIAgentContext(env / git / skills / project_rules / codebase / current_time)
// 渲染进 system,让 BYOP 路径也能拥有跟 warp 自家路径相当的环境信息。

use super::attachment_caps;
use super::prompt_renderer;
use super::user_context;
use crate::ai::agent::AIAgentContext;

/// 从 input 中抽出最近一条 `UserQuery.context`(等价 warp `convert_to.rs::convert_input` 取的那条)。
fn latest_input_context(input: &[AIAgentInput]) -> &[AIAgentContext] {
    for i in input.iter().rev() {
        if let Some(ctx) = i.context() {
            return ctx;
        }
    }
    &[]
}

/// LRC tag-in 场景下渲染 `<attached_running_command>` XML 块,prepend 到 user message,
/// 让模型看到当前 PTY 的实际状态(命令、grid 内容、是否 alt-screen),从而正确选择
/// `write_to_long_running_shell_command` 工具发送对应键序列。
fn render_running_command_context(rc: &RunningCommand) -> String {
    format!(
        "<attached_running_command command_id=\"{}\" is_alt_screen_active=\"{}\">\n  \
         <command>{}</command>\n  \
         <snapshot>\n{}\n  </snapshot>\n  \
         <instructions>This command is already running in the user's terminal. \
         Use `read_shell_command_output` with this command_id to inspect it, and \
         `write_to_long_running_shell_command` with this command_id to operate the program \
         through its PTY (in raw mode, use tokens like `<ESC>` and `<ENTER>` for control \
         keys). This command_id is valid even if the process was started by the user \
         rather than by run_shell_command. Do NOT spawn a new shell to control the same TUI.\
         </instructions>\n\
         </attached_running_command>",
        xml_attr(rc.block_id.as_str()),
        rc.is_alt_screen_active,
        xml_text(&rc.command),
        xml_text(&rc.grid_contents),
    )
}

/// 简短回退版本:仅有 command_id(没拿到 RunningCommand 完整快照时),
/// 让模型至少知道目标 PTY 的 id,可以用 `read_shell_command_output` 自己取最新内容。
fn render_running_command_id_context(command_id: &str) -> String {
    format!(
        "<attached_running_command command_id=\"{}\">\n  \
         <instructions>This command is already running in the user's terminal. \
         Use `read_shell_command_output` with this command_id to inspect it, and \
         `write_to_long_running_shell_command` with this command_id to operate the program \
         through its PTY. Do NOT spawn a new shell to control the same TUI.</instructions>\n\
         </attached_running_command>",
        xml_attr(command_id),
    )
}

fn render_lrc_request_context(params: &RequestParams) -> Option<String> {
    params
        .lrc_running_command
        .as_ref()
        .map(render_running_command_context)
        .or_else(|| {
            params
                .lrc_command_id
                .as_deref()
                .map(render_running_command_id_context)
        })
}

/// OpenWarp:渲染 SSH 会话状态块,append 到 system prompt 末尾。
///
/// 触发条件:`SessionContext.is_legacy_ssh()` 为 true(用户在本地 PTY 手敲
/// `ssh xx@xx` 进入远端,远端没装 warp shell hook)。这种会话:
/// - `session_type` 仍是 `Local`
/// - 整段 system prompt 的 [Environment] 区块描述的是**本地客户端** OS / shell,
///   而 PTY 当前实际跑在**远端**
///
/// 不主动告知模型这一点,LLM 会按 system prompt 里的本地 OS 推断"目标在远端,
/// 我得先 ssh 过去",于是输出 `ssh xx@xx uname -a` 这种二次嵌套命令。
///
/// 注:warpified SSH(`SessionType::WarpifiedRemote`)不在这里处理 — 那条路径
/// 远端 shell hook 已重新 bootstrap,host_info / shell 都是远端真值,prompt 本身就对。
fn render_ssh_session_block(
    session_context: &crate::ai::blocklist::SessionContext,
) -> Option<String> {
    if !session_context.is_legacy_ssh() {
        return None;
    }
    let info = session_context.ssh_connection_info();
    let host = info
        .and_then(|i| i.host.as_deref())
        .map(xml_attr)
        .unwrap_or_else(|| "unknown".to_owned());
    let port = info
        .and_then(|i| i.port.as_deref())
        .map(xml_attr)
        .unwrap_or_else(|| "22".to_owned());

    Some(format!(
        "\n\n<ssh_session host=\"{host}\" port=\"{port}\">\n  \
         <fact>The active terminal PTY is currently inside an SSH session opened by the user from their local machine. \
         All shell commands you run via `run_shell_command` execute on the REMOTE host, not on the local client.</fact>\n  \
         <warning>The [Environment] block (OS / shell / working directory) above describes the LOCAL client and may not match the remote host. \
         If you need precise remote info, probe it directly (e.g. `uname -a`, `cat /etc/os-release`, `pwd`).</warning>\n  \
         <rules>\n    \
         - Run commands DIRECTLY (e.g. `uname -a`, `ls /`). Do NOT prepend `ssh {host} ...` — that opens a NESTED ssh session inside the current one.\n    \
         - Treat the working directory and home directory shown above with skepticism; they may reflect the local client.\n    \
         - When LRC tag-in mode is active (an `<attached_running_command>` block is present), prefer `write_to_long_running_shell_command` with that command_id to inject keystrokes into this same remote PTY. Spawning a new shell would create a separate local-side ssh client, not interact with the remote process the user is watching.\n  \
         </rules>\n\
         </ssh_session>"
    ))
}

/// XML 转义,同时 strip 所有非法/有问题的控制字符,避免 JSON 序列化失败。
///
/// `grid_contents`(从 `formatted_terminal_contents_for_input` 提取的 alt-screen 内容)
/// 可能含原始 ANSI escape 序列(0x1b)、CSI sequences、SGR codes、box-drawing chars 等。
/// 其中 `< 0x20` 的控制字符会让 Anthropic 解析 JSON 报 "invalid escaped character in string",
/// 因为 JSON RFC 7159 只接受 `\b \f \n \r \t \" \\ \/ \uXXXX` 这几种合法转义,
/// 其他 `\v` `\a` `\x1b` 之类直接 reject。
///
/// 处理:
/// - `\n` `\r` `\t` 保留(JSON 合法)
/// - 其它 `< 0x20` 控制字符替换成空格(纯展示给模型,不需要保留 ANSI 颜色等)
/// - `&` `<` `>` 转 XML entity
fn xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' | '\r' | '\t' => out.push(c),
            c if (c as u32) < 0x20 => out.push(' '),
            // DEL(0x7f)单独处理 — 也是控制字符
            '\u{7f}' => out.push(' '),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn xml_attr(s: &str) -> String {
    xml_text(s).replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Multi-turn message 转换
// ---------------------------------------------------------------------------

/// 累积同一 assistant turn 的 text + tool_calls + reasoning,然后 flush 成一个或两个
/// `ChatMessage`(text 一个,tool_calls 一个 — genai 把它们建模为分开的 message)。
///
/// **thinking-mode reasoning_content 回传 gate**(双向)。
///
/// `force_echo_reasoning` 同时控制两件事,语义统一为「这个 endpoint 接受/需要
/// `reasoning_content` 顶层字段」:
///
/// - `true`(DeepSeek api_type / OpenAI+kimi|moonshot):每条 assistant 必挂
///   `reasoning_content`(有真实 reasoning 用之,无则挂非空占位)— 满足
///   DeepSeek-v4-flash / Kimi 等 thinking-mode 服务端「字段必须存在」校验。
/// - `false`(其他):**即便 stream 收到了真实 reasoning_content,回放时也丢弃**,
///   不在历史 assistant 上挂 `with_reasoning_content`。
///
/// 为什么 `false` 时也要主动丢弃真实 reasoning:许多 OpenAI-strict provider 把
/// `messages[].reasoning_content` 视为非法字段并 400(`code: wrong_api_format`):
///
/// - **Cerebras**(zerx-lab/warp #25 元凶,zai-glm-4.7 第二轮 400)
/// - **Groq**(协议侧用 `reasoning_format` / `include_reasoning`,不接受 message 字段)
/// - **OpenRouter / Together AI / SambaNova / Anyscale / Replicate** 等中转/inference 厂商
/// - **OpenAI 官方**(GPT-5/o-series 走 OpenAIResp,o-series 用 `reasoning.encrypted_content`,不收 `reasoning_content`)
///
/// genai 0.6 OpenAI adapter `adapter_shared.rs:367,385-387` 见到
/// `ContentPart::ReasoningContent` 就**无条件** hoist 出顶层 `reasoning_content`
/// 字段,所以 gate 必须前移到 client 侧 — 即「不挂 ContentPart 就不会被序列化」。
///
/// Anthropic / Gemini adapter 序列化层会忽略 `ContentPart::ReasoningContent`
/// (各自走 thinking blocks / thought signature),不受这个 gate 影响,但保持一致仍走 `false` 分支不挂。
const REASONING_ECHO_PLACEHOLDER: &str = " ";

#[derive(Default)]
struct AssistantBuffer {
    text: Option<String>,
    tool_calls: Vec<ToolCall>,
    /// 上一轮 AgentReasoning(thinking 链)。flush 时挂到对应 assistant message
    /// 的 reasoning_content 字段(genai 内部按 adapter 序列化:DeepSeek/Kimi 走 reasoning_content,
    /// Anthropic 走 thinking blocks)。
    reasoning: Option<String>,
    /// thinking-mode adapter 强制回传 reasoning_content(非空占位)。由
    /// `super::reasoning::model_requires_reasoning_echo` 决定。
    force_echo_reasoning: bool,
}

impl AssistantBuffer {
    fn new(force_echo_reasoning: bool) -> Self {
        Self {
            force_echo_reasoning,
            ..Default::default()
        }
    }

    fn flush_into(&mut self, messages: &mut Vec<ChatMessage>) {
        let reasoning = self.reasoning.take();
        let has_tool_calls = !self.tool_calls.is_empty();
        // 决定本次 flush 要挂到 assistant message 上的 reasoning 字符串。
        //
        // **gate 反转**:`force_echo_reasoning = false` 时**一律不挂**,即使本 turn
        // stream 收到了真实 reasoning(zai-glm / qwen3-thinking 这类 thinking 模型走
        // OpenAI 兼容路径会 emit reasoning_content chunk)— 因为 Cerebras / Groq /
        // OpenRouter 等 OpenAI-strict provider 见到 `messages[].reasoning_content` 直接
        // 400 `wrong_api_format`(zerx-lab/warp #25)。
        //
        // `force_echo_reasoning = true` 时(DeepSeek api_type / OpenAI+kimi/moonshot):
        // - 有真实 reasoning → 用之
        // - 没有 → 非空占位(满足"字段必须存在"校验)
        let echo_reasoning: Option<String> = if self.force_echo_reasoning {
            match reasoning {
                Some(r) if !r.is_empty() => Some(r),
                _ => Some(REASONING_ECHO_PLACEHOLDER.to_owned()),
            }
        } else {
            // 注:即便 `reasoning` 是 Some(非空),也丢弃 — 见上方 gate 反转说明。
            None
        };
        if let Some(t) = self.text.take() {
            let mut msg = ChatMessage::assistant(t);
            if has_tool_calls {
                // DeepSeek thinking mode 要求每条 assistant message 都带
                // reasoning_content。text + tool_calls 被 genai 建模成两条
                // assistant 时,text 这条也必须补占位。
                if self.force_echo_reasoning {
                    msg = msg.with_reasoning_content(Some(REASONING_ECHO_PLACEHOLDER.to_owned()));
                }
            } else if let Some(r) = echo_reasoning.clone() {
                msg = msg.with_reasoning_content(Some(r));
            }
            messages.push(msg);
        }
        if has_tool_calls {
            // genai `From<Vec<ToolCall>> for ChatMessage` 自动产 assistant role +
            // MessageContent::from_tool_calls。
            let mut msg = ChatMessage::from(std::mem::take(&mut self.tool_calls));
            if let Some(r) = echo_reasoning {
                msg = msg.with_reasoning_content(Some(r));
            }
            messages.push(msg);
        }
    }
}

/// 构造一条 user `ChatMessage`,按 model capability 决定要不要切到
/// `MessageContent::Parts(Text + Binary[])` 多模态形态。
///
/// - 没有 binaries → 走老路 `ChatMessage::user(text)` 纯文本,与 P0 行为一致
/// - 有 binaries 且 model 支持对应 mime → `Parts(vec![Text(text), Binary(...), ...])`,
///   genai adapter 自动按线协议适配(OpenAI image_url/file、Anthropic image/document、
///   Gemini inline_data 等)
/// - binaries 但 model 不支持 → log warn 跳过该 part,降级为纯文本(prefix XML 里的
///   `<image .../>` / `<file binary=true .../>` 占位仍然在,LLM 至少知道用户附了什么)
fn build_user_message_with_binaries(
    text: String,
    binaries: Vec<user_context::UserBinary>,
    api_type: AgentProviderApiType,
    model_id: &str,
) -> ChatMessage {
    if binaries.is_empty() {
        return ChatMessage::user(text);
    }
    let caps = attachment_caps::caps_for(api_type, model_id);

    let mut parts: Vec<ContentPart> = Vec::with_capacity(1 + binaries.len());
    parts.push(ContentPart::Text(text));

    let mut error_replacements: Vec<(String, String)> = Vec::new();
    for bin in binaries {
        if !caps.supports_mime(&bin.content_type) {
            // OpenWarp 对齐 opencode `unsupportedParts`(packages/opencode/src/provider/transform.ts:305-341):
            // 模型不支持的 mime 不静默 drop,改成插入一条 ERROR 文本 part,让 LLM 自己告诉用户。
            // 文案严格照抄 opencode 的 `ERROR: Cannot read {name} (this model does not support
            // {modality} input). Inform the user.`,modality 由 mime 前缀映射,name 优先用文件名。
            let modality = mime_to_modality(&bin.content_type);
            let name = if bin.name.is_empty() {
                modality.to_string()
            } else {
                format!("\"{}\"", bin.name)
            };
            let err_text = format!(
                "ERROR: Cannot read {name} (this model does not support {modality} input). Inform the user."
            );
            error_replacements.push((bin.name.clone(), bin.content_type.clone()));
            parts.push(ContentPart::Text(err_text));
            continue;
        }
        parts.push(ContentPart::Binary(Binary::from_base64(
            bin.content_type,
            bin.data,
            Some(bin.name),
        )));
    }

    if !error_replacements.is_empty() {
        log::info!(
            "[byop] {} attachment(s) replaced with ERROR text — model {api_type:?}/{model_id} \
             does not support: {error_replacements:?}",
            error_replacements.len()
        );
    }

    // 若 binaries 全是被替换的 ERROR 文本(没有真正的 Binary part),仍保留 ERROR 文本 part
    // 让模型看到。退化情况(例如 text 为空 + 没有任何 part 加进来)兜底纯文本。
    if parts.len() == 1 {
        if let Some(ContentPart::Text(t)) = parts.into_iter().next() {
            return ChatMessage::user(t);
        }
        return ChatMessage::user("");
    }

    ChatMessage {
        role: ChatRole::User,
        content: MessageContent::from_parts(parts),
        options: None,
    }
}

/// MIME → modality 字符串映射。对齐 opencode `mimeToModality`
/// (packages/opencode/src/provider/transform.ts:12-18)。
fn mime_to_modality(mime: &str) -> &'static str {
    let lower = mime.trim().to_ascii_lowercase();
    if lower.starts_with("image/") {
        "image"
    } else if lower.starts_with("audio/") {
        "audio"
    } else if lower.starts_with("video/") {
        "video"
    } else if lower == "application/pdf" {
        "pdf"
    } else {
        "file"
    }
}

/// 把 RequestParams 翻译为 genai `ChatRequest`(含 system + messages + tools)。
///
/// `force_echo_reasoning`:由 `super::reasoning::model_requires_reasoning_echo`
/// 决定。true 时所有 assistant message 强制挂 reasoning_content(空串占位),
/// 修复 DeepSeek-v4-flash / Kimi 等收紧校验的 thinking-mode endpoint。
fn build_chat_request(
    params: &RequestParams,
    force_echo_reasoning: bool,
    api_type: AgentProviderApiType,
    model_id: &str,
) -> ChatRequest {
    let agent_ctx = latest_input_context(&params.input);
    let plan_mode = is_plan_mode_turn(&params.input);
    let tool_names = available_tool_names(params);
    let mut system_text =
        prompt_renderer::render_system(&params.model, agent_ctx, &tool_names, plan_mode);
    // OpenWarp:legacy SSH 会话画像补丁。`render_system` 走 AIAgentContext,
    // 拿到的 OS/shell 是本地客户端;legacy SSH 下 PTY 实际在远端,
    // 追加一段 SSH 状态块矫正 LLM 推断。
    if let Some(ssh_block) = render_ssh_session_block(&params.session_context) {
        system_text.push_str(&ssh_block);
    }
    // 注:LRC / 长命令的工具用法引导(write_to_long_running_shell_command + command_id +
    // 各种 mode 与 raw 字节序列)已经在 `prompts/system/default.j2:69-79` 完整覆盖。
    // 用户当前所处的具体 PTY 上下文(命令名 / alt-screen 标志 / grid 内容)通过
    // user message 前缀的 `<attached_running_command>` XML 块单独注入(见
    // `render_running_command_context` 与 build_chat_request 中的 UserQuery 分支)。
    // 不在 system 这层重复硬编码 TUI 退出键之类,避免与 default.j2 的标准引导冲突或冗余。

    let mut messages: Vec<ChatMessage> = Vec::new();

    // 收集所有 task 的 messages,按时间戳排序。
    let mut all_msgs: Vec<&api::Message> = params
        .tasks
        .iter()
        .flat_map(|t| t.messages.iter())
        .collect();
    all_msgs.sort_by_key(|m| {
        m.timestamp
            .as_ref()
            .map(|ts| (ts.seconds, ts.nanos))
            .unwrap_or((0, 0))
    });

    // OpenWarp BYOP 本地会话压缩:把 conversation.compaction_state 应用到 message 序列。
    //   1. 过滤已被某次压缩覆盖的 (user, assistant) 对(`hidden_message_ids`)
    //   2. 在被隐去区间的位置插入一对合成的 (user "已压缩,以下为摘要" + assistant 摘要文本) message —
    //      这一步通过 `summary_inserts` 索引在主 loop 里就近 emit
    //   3. ToolCallResult 的 marker.tool_output_compacted_at 不为空时,后面分支替换 content 为占位符
    //
    // 当前 input 是 `AIAgentInput::SummarizeConversation` 时:进一步用 select 算法把 messages
    // 切到 head(去掉 tail),最后 input loop 末尾会追加 `build_prompt(...)` 作为 user message
    // (走完整的 SUMMARY_TEMPLATE),让上游 LLM 输出结构化摘要。
    let is_summarization_request = params
        .input
        .iter()
        .any(|i| matches!(i, AIAgentInput::SummarizeConversation { .. }));
    let summarization_overflow = params.input.iter().any(|i| {
        matches!(
            i,
            AIAgentInput::SummarizeConversation { overflow: true, .. }
        )
    });
    let _ = summarization_overflow; // 当前在 input loop 内的 follow-up 文案分支会用,目前先 silence dead

    let summary_inserts: std::collections::HashMap<String, String> =
        if let Some(state) = params.compaction_state.as_ref() {
            // user_msg_id → summary_text;遇到该 user_msg_id 时(它本来要被 hidden)替换为合成的摘要对
            state
                .completed()
                .iter()
                .filter_map(|c| {
                    c.summary_text
                        .as_ref()
                        .map(|s| (c.user_msg_id.clone(), s.clone()))
                })
                .collect()
        } else {
            std::collections::HashMap::new()
        };
    let hidden_msg_ids: std::collections::HashSet<String> = params
        .compaction_state
        .as_ref()
        .map(|s| s.hidden_message_ids())
        .unwrap_or_default();
    let compacted_tool_msg_ids: std::collections::HashSet<String> = params
        .compaction_state
        .as_ref()
        .map(|s| {
            // 收集所有标记了 tool_output_compacted_at 的 ToolCallResult message_ids
            // 通过遍历 all_msgs 并查 marker 实现
            let mut out = std::collections::HashSet::new();
            for msg in &all_msgs {
                if let Some(api::message::Message::ToolCallResult(_)) = &msg.message {
                    if s.marker(&msg.id)
                        .and_then(|m| m.tool_output_compacted_at)
                        .is_some()
                    {
                        out.insert(msg.id.clone());
                    }
                }
            }
            out
        })
        .unwrap_or_default();

    // 摘要请求路径:用 byop_compaction::algorithm::select 切 head;tail 不送上游
    let summarize_head_end: Option<usize> = if is_summarization_request {
        // 临时投影成 WarpMessageView 算 select
        let state_for_select = params.compaction_state.clone().unwrap_or_default();
        let tool_names =
            byop_compaction::message_view::build_tool_name_lookup(all_msgs.iter().copied());
        let views =
            byop_compaction::message_view::project(&all_msgs, &state_for_select, &tool_names);
        let cfg = byop_compaction::CompactionConfig::default();
        let model_limit = byop_compaction::overflow::ModelLimit::FALLBACK;
        let result = byop_compaction::algorithm::select(&views, &cfg, model_limit, |slice| {
            slice
                .iter()
                .map(byop_compaction::algorithm::MessageRef::estimate_size)
                .sum()
        });
        // head_end 是 views 里"head 区间"上界,与 all_msgs 同序
        Some(result.head_end)
    } else {
        None
    };

    let mut buf = AssistantBuffer::new(force_echo_reasoning);
    // OpenWarp:历史里被 skip 掉的 subagent ToolCall 对应的 call_id —— 它们的
    // ToolCallResult 也必须 skip,否则会成为孤儿 tool_response,Anthropic 直接 400
    // `unexpected tool_use_id ... no corresponding tool_use block`。
    let mut skipped_subagent_call_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for (idx, msg) in all_msgs.iter().enumerate() {
        // 摘要请求:tail 区间不送上游(只送 head + 末尾追加 SUMMARY_TEMPLATE)
        if let Some(head_end) = summarize_head_end {
            if idx >= head_end {
                continue;
            }
        }
        if hidden_msg_ids.contains(&msg.id) {
            if let Some(summary_text) = summary_inserts.get(&msg.id) {
                buf.flush_into(&mut messages);
                messages.push(ChatMessage::user(
                    "Conversation history was compacted. Below is the structured summary of all prior turns.".to_string(),
                ));
                messages.push(ChatMessage::assistant(summary_text.clone()));
            }
            continue;
        }
        let Some(inner) = &msg.message else {
            continue;
        };
        match inner {
            api::message::Message::UserQuery(u) => {
                buf.flush_into(&mut messages);
                // OpenWarp:历史轮多模态保活。warp 自家路径靠云端 server 重注入 InputContext,
                // BYOP 直连没有那层,所以 `make_user_query_message` 持久化时把所有 binary
                // (image / pdf / audio)塞进了 `UserQuery.context.images`,这里反向恢复成
                // UserBinary 走 `build_user_message_with_binaries`,使后续轮模型仍能看到先前
                // 粘贴的多模态附件。模型 caps 不支持的 mime 由 build_user_message_with_binaries
                // 替换为 ERROR 文本(opencode unsupportedParts 风格),不会静默 drop。
                // 没有 binary → 退回老路 `ChatMessage::user(text)`,与修复前等价。
                let history_binaries: Vec<user_context::UserBinary> = u
                    .context
                    .as_ref()
                    .map(|ctx| {
                        ctx.images
                            .iter()
                            .filter(|b| !b.data.is_empty())
                            .enumerate()
                            .map(|(idx, b)| {
                                use base64::Engine;
                                user_context::UserBinary {
                                    name: format!("history-attachment-{}-{idx}", &msg.id),
                                    content_type: if b.mime_type.is_empty() {
                                        "application/octet-stream".to_string()
                                    } else {
                                        b.mime_type.clone()
                                    },
                                    data: base64::engine::general_purpose::STANDARD.encode(&b.data),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut history_prefixes: Vec<String> = Vec::new();
                if let Some(prefix) =
                    user_context::render_api_referenced_attachments(&u.referenced_attachments)
                {
                    history_prefixes.push(prefix);
                }
                let history_text = if history_prefixes.is_empty() {
                    u.query.clone()
                } else {
                    format!("{}\n\n{}", history_prefixes.join("\n\n"), u.query)
                };
                if history_binaries.is_empty() {
                    messages.push(ChatMessage::user(history_text));
                } else {
                    messages.push(build_user_message_with_binaries(
                        history_text,
                        history_binaries,
                        api_type,
                        model_id,
                    ));
                }
            }
            api::message::Message::AgentReasoning(r) => {
                // 把上一轮的 reasoning 挂到下一个要 flush 的 assistant message 上。
                // genai 0.6 的 with_reasoning_content 会按当前 adapter 序列化:
                // DeepSeek/Kimi → reasoning_content 字段;Anthropic → thinking blocks。
                // 多段 AgentReasoning 累加(同一 turn 可能 stream 出多个 reasoning chunk
                // 落地为多条 AgentReasoning)。
                let next = r.reasoning.clone();
                if !next.is_empty() {
                    match buf.reasoning.as_mut() {
                        Some(existing) => existing.push_str(&next),
                        None => buf.reasoning = Some(next),
                    }
                }
            }
            api::message::Message::AgentOutput(a) => {
                if buf.text.is_some() || !buf.tool_calls.is_empty() {
                    buf.flush_into(&mut messages);
                }
                buf.text = Some(a.text.clone());
            }
            api::message::Message::ToolCall(tc) => {
                // OpenWarp BYOP:**虚拟 subagent tool_call 不发给上游模型**。
                // LRC tag-in 场景下,我们在 chat_stream 流头合成 `Tool::Subagent { metadata: Cli }`
                // 写入 root.task.messages,只用于触发 conversation 创建 cli subtask + spawn 浮窗,
                // 它不是模型实际产出的工具调用,模型看到会 confused(多余 tool call + 没法回应)。
                // 同样它对应的 placeholder ToolResponse(由 sanitize_tool_call_pairs 补的)
                // 也要由下面 ToolCallResult 分支的 skip 逻辑配合过滤,避免出现
                // "tool_response 找不到匹配的 tool_call" 的不平衡。
                use crate::ai::agent::task::helper::ToolCallExt;
                if tc.subagent().is_some() {
                    skipped_subagent_call_ids.insert(tc.tool_call_id.clone());
                    continue;
                }
                let (name, args_json) = serialize_outgoing_tool_call(
                    tc,
                    params.mcp_context.as_ref(),
                    &msg.server_message_data,
                );
                buf.tool_calls.push(ToolCall {
                    call_id: tc.tool_call_id.clone(),
                    fn_name: name,
                    fn_arguments: args_json,
                    thought_signatures: None,
                });
            }
            api::message::Message::ToolCallResult(tcr) => {
                buf.flush_into(&mut messages);
                // OpenWarp:对应 ToolCall 已被 skip(subagent 虚拟 call)→ result 也 skip,
                // 否则留下孤儿 tool_response 导致上游 400。
                if skipped_subagent_call_ids.contains(&tcr.tool_call_id) {
                    continue;
                }
                // BYOP 持久化的 ToolCallResult 走 server_message_data(content 已是 JSON 字符串);
                // server 端 emit 走 result oneof 结构化 variant — 兼容两路。
                let content = if compacted_tool_msg_ids.contains(&msg.id) {
                    // 压缩投影:被 prune 的 tool output 替换为占位符,不送实际内容上游
                    r#"{"status":"compacted","note":"tool output was pruned by local compaction"}"#
                        .to_string()
                } else if tcr.result.is_some() {
                    tools::serialize_result(tcr)
                } else if !msg.server_message_data.is_empty() {
                    msg.server_message_data.clone()
                } else {
                    r#"{"status":"empty"}"#.to_owned()
                };
                messages.push(ChatMessage::from(ToolResponse::new(
                    tcr.tool_call_id.clone(),
                    content,
                )));
            }
            _ => {
                // 其他 message 类型(SystemQuery/UpdateTodos/...)BYOP 暂不送上游。
            }
        }
    }
    buf.flush_into(&mut messages);

    // 当前轮新输入 → 追加。
    for input in &params.input {
        match input {
            AIAgentInput::UserQuery {
                query,
                context,
                referenced_attachments,
                running_command,
                ..
            } => {
                // 当前轮 UserQuery 自带的附件类 context(Block / SelectedText / File / Image)
                // 严格对齐 warp 自家路径走 `api::InputContext.executed_shell_commands` 等字段
                // 上行后由后端注入 prompt 的语义。BYOP 没有后端这层,直接 prepend 到 user message。
                // 环境型 context(env / git / skills / ...)由 prompt_renderer 渲染进 system,
                // 与本路径不重叠。
                //
                // OpenWarp：LRC tag-in 场景下，`running_command: Some(...)` 含完整 PTY 上下文
                // （alt-screen grid_contents + command + is_alt_screen_active 标志），用
                // `render_running_command_context` 渲成 `<attached_running_command>` XML 块。
                // 模型据此决定调 write_to_long_running_shell_command。
                // 没填（普通对话或 controller 没注入）时回退到 `lrc_command_id` 简短上下文。
                //
                // **P1-10 prompt cache 优化**：LRC 上下文块**追加到 query 之后**而不
                // 是前缀。原因：
                //   - grid_contents 随 PTY 状态每秒变化，是 “高频变动” 内容。
                //   - 放到 query 前面会让 user message 头部不稳定→ messages 段末尾
                //     2 个 Anthropic breakpoint 写入的哈希总是不同，复用价值低。
                //   - 放到 query 之后，同一个 query (如 “退出 nvim”) 在不同 PTY 快照上仍
                //     共享前缀“user 问题”，提高跨调用复用可能。
                // 模型行为差别微小：指令在前还是 context 在前，模型都能正确理解。
                // user_attachments 的 prefix（如 SelectedText / Block）仍放前缀位，因为
                // 它们对应用户“明确选中”的内容，应作为问题背景而非实例补充。
                let mut suffixes: Vec<String> = Vec::new();
                let request_running_command = running_command
                    .as_ref()
                    .or(params.lrc_running_command.as_ref());
                if let Some(rc) = request_running_command {
                    suffixes.push(render_running_command_context(rc));
                } else if let Some(command_id) = params.lrc_command_id.as_deref() {
                    suffixes.push(render_running_command_id_context(command_id));
                }
                let mut prefixes: Vec<String> = Vec::new();
                let user_attachments = user_context::collect_user_attachments(context);
                if let Some(p) = &user_attachments.prefix {
                    prefixes.push(p.clone());
                }
                if let Some(p) = user_context::render_referenced_attachments(referenced_attachments)
                {
                    prefixes.push(p);
                }
                let full_text = match (prefixes.is_empty(), suffixes.is_empty()) {
                    (true, true) => query.clone(),
                    (false, true) => format!("{}\n\n{query}", prefixes.join("\n\n")),
                    (true, false) => format!("{query}\n\n{}", suffixes.join("\n\n")),
                    (false, false) => format!(
                        "{}\n\n{query}\n\n{}",
                        prefixes.join("\n\n"),
                        suffixes.join("\n\n"),
                    ),
                };
                log::info!(
                    "[byop-diag] build_chat_request UserQuery: query_len={} \
                     running_command={} prefixes={} suffixes={} full_text_len={} binaries={}",
                    query.len(),
                    match request_running_command {
                        Some(rc) => format!(
                            "Some(grid_len={} alt={})",
                            rc.grid_contents.len(),
                            rc.is_alt_screen_active
                        ),
                        None => "None".to_owned(),
                    },
                    prefixes.len(),
                    suffixes.len(),
                    full_text.len(),
                    user_attachments.binaries.len(),
                );
                messages.push(build_user_message_with_binaries(
                    full_text,
                    user_attachments.binaries,
                    api_type,
                    model_id,
                ));
            }
            AIAgentInput::ActionResult { result, .. } => {
                // 上一轮模型回了 tool_calls,client 端执行完后 result 走 `params.input`
                // 而不是 `params.tasks` 历史。必须在这里序列化为 ToolResponse,否则
                // genai/上游会因 tool_call_id 配对失败 400。
                let tool_call_id = result.id.to_string();
                let content = tools::serialize_action_result(result).unwrap_or_else(|| {
                    serde_json::json!({ "result": result.result.to_string() }).to_string()
                });
                messages.push(ChatMessage::from(ToolResponse::new(tool_call_id, content)));
            }
            AIAgentInput::InvokeSkill {
                skill, user_query, ..
            } => {
                let mut composed = format!(
                    "请按下面的技能 \"{}\" 指引执行任务:\n\n{}\n\n---\n",
                    skill.name, skill.content,
                );
                if let Some(uq) = user_query {
                    composed.push_str(&format!("用户进一步指令: {}", uq.query));
                }
                messages.push(ChatMessage::user(composed));
            }
            AIAgentInput::ResumeConversation { context } => {
                // BYOP 没有 server 端 resume prompt 注入层。LRC auto-resume 时必须显式
                // 重带当前 PTY 上下文,否则错误恢复轮会退化成普通对话并重新选择 shell 工具。
                let mut prefixes: Vec<String> = Vec::new();
                if let Some(lrc_prefix) = render_lrc_request_context(params) {
                    prefixes.push(lrc_prefix);
                }
                let user_attachments = user_context::collect_user_attachments(context);
                if let Some(p) = &user_attachments.prefix {
                    prefixes.push(p.clone());
                }
                if !prefixes.is_empty() {
                    let full_text = format!("{}\n\nContinue.", prefixes.join("\n\n"));
                    messages.push(build_user_message_with_binaries(
                        full_text,
                        user_attachments.binaries,
                        api_type,
                        model_id,
                    ));
                }
            }
            AIAgentInput::SummarizeConversation {
                prompt,
                overflow: _,
            } => {
                // OpenWarp BYOP 本地会话压缩入口 — 1:1 对齐 opencode `compaction.ts processCompaction`。
                //
                // 此前 messages loop 已根据 `summarize_head_end` 把序列切到 head(去掉 tail);
                // 这里追加最后一条 user message:`build_prompt(previous_summary, plugin_context)`,
                // 它包含 SUMMARY_TEMPLATE(9 段 Markdown 模板)+ 增量摘要锚点。
                //
                // 模型会 emit 一段结构化 Markdown 摘要文本,controller 接到 stream 完成
                // 后把它写回 conversation.compaction_state(参见 Phase 6 controller 改动)。
                let prev_summary = params
                    .compaction_state
                    .as_ref()
                    .and_then(|s| s.previous_summary())
                    .map(str::to_string);
                let mut anchor_context: Vec<String> = Vec::new();
                if let Some(custom) = prompt.as_ref().filter(|p| !p.is_empty()) {
                    // /compact <自定义指令> 走这里 — 把用户指令拼到 plugin_context 段
                    anchor_context
                        .push(format!("Additional instructions from the user:\n{custom}"));
                }
                let nextp =
                    byop_compaction::prompt::build_prompt(prev_summary.as_deref(), &anchor_context);
                messages.push(ChatMessage::user(nextp));
            }
            AIAgentInput::AutoCodeDiffQuery { .. }
            | AIAgentInput::CreateNewProject { .. }
            | AIAgentInput::CodeReview { .. } => {
                // 暂时忽略
            }
            _ => {}
        }
    }

    // 防御性 sanitize: 确保每个 assistant tool_calls 后面跟着对应每个 call_id 的
    // ToolResponse。warp 自家协议有时把 tool result 消化成下一轮 AgentOutput,
    // 上游若未保留 ToolCallResult,会让 tool_calls 配对失败。
    sanitize_tool_call_pairs(&mut messages);

    // 防御性 sanitize: 确保 messages 末尾不是 assistant。
    // Anthropic / 部分网关不接受末尾为 assistant 的请求(prefill 仅特定模型支持),
    // 而 warp 的 `AIAgentInput::ResumeConversation`(handoff/auto-resume after error 等)
    // 不附加新 user 消息,会让序列末尾停在历史 assistant 上。
    // 这里统一兜底:末尾若是 assistant,追加一条隐式 user 消息让上游继续。
    ensure_ends_with_user(&mut messages);

    let tools_array = build_tools_array(params);

    // 出站消息文本透传给 `serde_json` 处理 JSON escape,不再做激进的字符级
    // sanitize(参考 zed `into_anthropic` / opencode `provider/transform.ts`,
    // 两者都不在出站层打平控制字符或替换 `\` / `"`)。Anthropic / OpenAI / Gemini
    // 官方 API 与主流 BYOP 反代均能正确处理 `serde_json` 产出的合法 escape。

    // Prompt caching(1:1 移植自 opencode `provider/transform.ts::applyCaching`):
    // - opencode 选 first 2 system message + last 2 non-system message,统一打上
    //   anthropic.cacheControl / openaiCompatible.cache_control / bedrock.cachePoint
    //   等多 SDK 兼容标记。AI SDK 各 provider 实现读对应 key,无关 key 自动忽略。
    // - 我们走 rust-genai,Anthropic adapter 支持 per-message `cache_control`,
    //   OpenAI / OpenAiResp adapter 仅认 `ChatOptions` 级别的 prompt_cache_key /
    //   cache_control,DeepSeek / Gemini / Ollama 服务端隐式缓存,无需 client opt-in。
    // - 故在此只对 Anthropic 路径"per-message"打标:把 system 文本作为
    //   ChatRole::System message 推到 messages 头部并打 Ephemeral,再把末尾两条
    //   非 system message 也打 Ephemeral(对应 opencode 的 system+last 2 模式)。
    //   OpenAI 系的 `prompt_cache_key` / `cache_control` 在 `build_chat_options`
    //   里设置(请求级别),也来自 opencode 同一组规则的下游 fallback。
    let messages = if matches!(api_type, AgentProviderApiType::Anthropic) {
        let mut msgs: Vec<ChatMessage> = std::iter::once(ChatMessage::system(system_text.clone()))
            .chain(messages)
            .collect();
        apply_caching_anthropic(&mut msgs);
        msgs
    } else {
        messages
    };

    let mut req = ChatRequest::from_messages(messages);
    // Anthropic 路径 system 已经作为 ChatRole::System message 进 messages,
    // 不再设 `with_system`,避免 genai Anthropic adapter 的"first system 不能挂
    // cache_control"限制(`adapter_impl.rs::into_anthropic_request_parts` 注释)。
    if !matches!(api_type, AgentProviderApiType::Anthropic) {
        req = req.with_system(system_text);
    }
    if !tools_array.is_empty() {
        req = req.with_tools(tools_array);
    }
    req
}

/// 1:1 移植自 opencode `provider/transform.ts::applyCaching` 的 Anthropic 分支:
/// 给 first 2 个 system message + last 2 个 non-system message 打 cache 标记。
///
/// genai Anthropic adapter 在 `into_anthropic_request_parts` 内把
/// `MessageOptions::cache_control` 落到该 message 最后一个 content part 上,
/// 行为与 opencode 给 lastContent.providerOptions.anthropic.cacheControl 一致。
///
/// **TTL 选择 / P0-4**:统一使用 `Ephemeral1h`(1 小时 TTL)。
///
/// 决策依据(2026-05 Anthropic 官方文档):
/// - 5m TTL 适合「使用频率高于每 5 分钟」的场景,refresh 不另外计费。
/// - 1h TTL 官方原话:
///   > The 1-hour cache is best used in the following scenarios:
///   > * when an agentic side-agent will take longer than 5 minutes,
///   > * when storing a long chat conversation with a user and you generally
///   >   expect that user may not respond in the next 5 minutes.
///   > * When latency is important and your follow up prompts may be sent
///   >   beyond 5 minutes.
/// - OpenWarp BYOP 是终端 agent 场景:工具长任务、用户思考间隔、agent
///   step-by-step 都可能 > 5min;Claude Code CLI 在同类场景实际默认就是 1h。
///
/// **成本影响**:1h cache 写入价 2× base(对比 5m 的 1.25×),read 价不变
/// (0.1× base)。只要同一 prefix 在 1h 内复用 ≥ 1 次,就比 5m 反复 refresh 划算
/// (5m 模式每 5min 必须有请求触发 refresh,否则失效;1h 模式 1 小时内任意请求
/// 都直接命中)。
///
/// **TTL 排序约束**:Anthropic API 要求长 TTL 的 breakpoint 必须排在短 TTL 之前
/// (`https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching
///  #mixing-different-ttls`)。这里全部统一 1h,无混用,天然合规。
/// genai 在 `into_anthropic_request_parts` 内会按顺序检查,违反时 warn(见
/// `lib/rust-genai/src/adapter/adapters/anthropic/adapter_impl.rs`)。
///
/// **为什么不给 tools 打独立 breakpoint(P1-11 调研后决定推迟)**:
/// Anthropic API 原生支持 tools 数组末尾独立 cache_control,但 vendored
/// `lib/rust-genai` 的 `Tool` struct 不带 cache_control 字段,要启用需:
/// 1. 修改 genai `Tool` 加 `cache_control: Option<CacheControl>`
/// 2. 修改 `tool_to_anthropic_tool` 输出 `cache_control` 到 JSON
/// 3. 这里调整为“1 system + 2 messages tail + 1 tools” 总计 4 个 breakpoint
/// 实测现有 4 breakpoint(2 system + 2 tail)在典型场景中命中率已达 99.9%,
/// tools 段独立 breakpoint 仅在 system 字段频繁变动(如切 web_search 开关)
/// 时才能带来额外复用,边际收益小,暂不实施。如后期出现 tools 失效问题
/// 再走 P1-11 未使用路径(需同步 patch genai 上游)。
fn apply_caching_anthropic(messages: &mut Vec<ChatMessage>) {
    let n = messages.len();
    if n == 0 {
        return;
    }
    let mut tag = vec![false; n];

    // first 2 system messages
    let mut sys_seen = 0;
    for (i, m) in messages.iter().enumerate() {
        if matches!(m.role, ChatRole::System) {
            tag[i] = true;
            sys_seen += 1;
            if sys_seen >= 2 {
                break;
            }
        }
    }
    // last 2 non-system messages
    let mut tail_seen = 0;
    for (i, m) in messages.iter().enumerate().rev() {
        if !matches!(m.role, ChatRole::System) {
            tag[i] = true;
            tail_seen += 1;
            if tail_seen >= 2 {
                break;
            }
        }
    }

    let original = std::mem::take(messages);
    *messages = original
        .into_iter()
        .enumerate()
        .map(|(i, m)| {
            if tag[i] {
                // ChatMessage 没有直接的 with_cache_control;cache_control 挂在
                // `MessageOptions` 上,通过 `with_options` 注入。
                // `MessageOptions: From<CacheControl>` 由 genai 提供
                // (`chat_message.rs::impl From<CacheControl> for MessageOptions`)。
                // P0-4:全部统一 1h,顺序天然 system→messages 兼容 genai 排序约束。
                m.with_options(CacheControl::Ephemeral1h)
            } else {
                m
            }
        })
        .collect();
}

/// 重排 messages 中所有 Tool 消息,确保:
/// 每个含 tool_calls 的 Assistant message 后面紧跟**且仅紧跟**一条 Tool message,
/// 内含该 Assistant **每个** call_id 的 ToolResponse(按 tool_calls 顺序,缺失补 placeholder)。
///
/// **为什么需要重排,而不是只补 placeholder / 剔孤儿**:
/// `build_chat_request` 按时间戳 chronological 排序合并所有 task 的历史 messages。
/// 当模型在一轮内发起多个 tool_call,且这些 tool 的执行时长差异较大时(如 read_skill
/// 立即返回错误,而 git/PowerShell 命令稍慢),后到的 ToolCallResult 时间戳可能晚于
/// 模型紧接着发起的**下一轮** Assistant tool_calls,导致历史 messages 被排成:
///
/// ```text
/// Asst-X(tc_a, tc_b, tc_c)
/// Tool(tc_c real)         ← read_skill 错误,快
/// Asst-Y(tc_d, tc_e)      ← 模型基于 tc_c 错误立刻发了下一轮
/// Tool(tc_a real)         ← git 命令慢,落到 Asst-Y 后面
/// Tool(tc_b real)
/// ```
///
/// Anthropic API 把连续 Tool block 合并视作"上一条 Assistant 的 tool_results",
/// 于是 Asst-Y 后面的 Tool block 含 tc_a/tc_b 这种 Asst-Y 不认识的 call_id → 400
/// `unexpected tool_use_id ... no corresponding tool_use block in the previous message`。
///
/// 旧实现只做"剔孤儿(整序列匹配)+补 placeholder(向前看相邻 Tool)",前者不会剔除
/// 这类**位置错误但 call_id 合法**的 ToolResponse,后者也不会重定位 — 所以 400 重现。
///
/// 新实现:抽出所有 ToolResponse 进 `call_id → response` 表,然后按每个 Assistant
/// tool_calls 的 call_id 顺序重新组装紧随其后的 Tool message。剩余未消费的 ToolResponse
/// (call_id 完全不在历史 Assistant tool_calls 里)即真孤儿,丢弃。
fn sanitize_tool_call_pairs(messages: &mut Vec<ChatMessage>) {
    use std::collections::HashMap;

    // 抽取所有 ToolResponse(同 call_id 后到的覆盖前面,符合"最新结果优先"语义)。
    let mut response_by_call_id: HashMap<String, ToolResponse> = HashMap::new();
    let original = std::mem::take(messages);
    let mut non_tool_msgs: Vec<ChatMessage> = Vec::with_capacity(original.len());
    for msg in original {
        if msg.role == genai::chat::ChatRole::Tool {
            for resp in msg.content.tool_responses() {
                response_by_call_id.insert(resp.call_id.clone(), (*resp).clone());
            }
        } else {
            non_tool_msgs.push(msg);
        }
    }

    // 重组:每个 Assistant 含 tool_calls 后紧跟一条 Tool message,按 call_id 顺序绑定。
    let mut placeholders_inserted: Vec<String> = Vec::new();
    for msg in non_tool_msgs {
        let call_ids: Vec<String> = msg
            .content
            .tool_calls()
            .iter()
            .map(|tc| tc.call_id.clone())
            .collect();
        let is_assistant = msg.role == genai::chat::ChatRole::Assistant;
        messages.push(msg);

        if is_assistant && !call_ids.is_empty() {
            let bundled: Vec<ToolResponse> = call_ids
                .iter()
                .map(|cid| {
                    response_by_call_id.remove(cid).unwrap_or_else(|| {
                        placeholders_inserted.push(cid.clone());
                        ToolResponse::new(cid.clone(), "(tool 执行结果未保留)".to_owned())
                    })
                })
                .collect();
            messages.push(ChatMessage::from(bundled));
        }
    }

    // 剩余 response_by_call_id 是真孤儿(没有任何 Assistant tool_call 与之配对),丢弃。
    if !response_by_call_id.is_empty() {
        let orphan_ids: Vec<&String> = response_by_call_id.keys().collect();
        log::warn!(
            "[byop-diag] sanitize_tool_call_pairs: 丢弃 {} 个孤儿 ToolResponse: \
             orphan_call_ids={:?}",
            response_by_call_id.len(),
            orphan_ids
        );
    }
    if !placeholders_inserted.is_empty() {
        log::warn!(
            "[byop-diag] sanitize_tool_call_pairs: 给 {} 个 ToolCall 补 placeholder \
             ToolResponse: missing_call_ids={:?}",
            placeholders_inserted.len(),
            placeholders_inserted
        );
    }
}

/// 兜底:确保 messages 末尾是 user(或 tool 响应)。
///
/// 触发场景:`AIAgentInput::ResumeConversation` 不附加新 user 消息,直接重发历史。
/// Anthropic 原生 API 拒绝末尾为 assistant 的请求(`This model does not support
/// assistant message prefill. The conversation must end with a user message.`),
/// 重试 3 次都同 payload → UI 渲染 error block 触发 flex panic。
///
/// 末尾是 assistant 时追加 `ChatMessage::user("Continue.")`,提示模型继续即可。
/// Tool 角色作为 user 输入的一种(模型会把 tool 响应当作下一轮起点)不动。
/// 空 messages 不触发,避免给空对话凭空塞内容。
fn ensure_ends_with_user(messages: &mut Vec<ChatMessage>) {
    use genai::chat::ChatRole;
    if let Some(last) = messages.last() {
        if last.role == ChatRole::Assistant {
            messages.push(ChatMessage::user("Continue."));
        }
    }
}

/// 反向: 把内部 `tool_call::Tool` variant 序列化成 (function name, arguments JSON Value)
/// 用于多轮历史回放。这里的 (name, args) 必须与 `tools::REGISTRY` 中各 tool 的 `name`
/// 与 `from_args` 期望的 schema 严格对齐。
fn serialize_outgoing_tool_call(
    tc: &api::message::ToolCall,
    mcp_ctx: Option<&crate::ai::agent::MCPContext>,
    server_message_data: &str,
) -> (String, Value) {
    use api::message::tool_call::Tool;

    // BYOP from_args 解析失败 carrier 还原:由 make_tool_call_carrier_message 写入,
    // tool oneof = None,原始 `<fn_name>\n<args_str>` 编码在 server_message_data。
    // 必须在主 match 之前优先识别,否则会落到下面 None=>"warp_internal_empty",
    // 上游模型看到一个不存在的工具名会更困惑、也不知道是哪个 call 失败了。
    if tc.tool.is_none() {
        if let Some((fn_name, raw_args)) = server_message_data.split_once('\n') {
            if !fn_name.is_empty() {
                let args_value = serde_json::from_str(raw_args)
                    .unwrap_or_else(|_| Value::String(raw_args.to_owned()));
                return (fn_name.to_owned(), args_value);
            }
        }
    }

    // 大多数旧实现返回 (String, String);这里改成 (String, Value),把字符串再 parse 一次。
    let (name, args_str) = match &tc.tool {
        Some(Tool::CallMcpTool(c)) => tools::mcp::serialize_outgoing_call(c, mcp_ctx),
        Some(Tool::ReadMcpResource(r)) => tools::mcp::serialize_outgoing_read_resource(r, mcp_ctx),
        Some(Tool::RunShellCommand(c)) => (
            "run_shell_command".to_owned(),
            json!({
                "command": c.command,
                "is_read_only": c.is_read_only,
                "uses_pager": c.uses_pager,
                "is_risky": c.is_risky,
            })
            .to_string(),
        ),
        Some(Tool::ReadFiles(r)) => {
            let files: Vec<Value> = r
                .files
                .iter()
                .map(|f| {
                    json!({
                        "path": f.name,
                        "line_ranges": f.line_ranges.iter().map(|lr| json!({
                            "start": lr.start, "end": lr.end
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            (
                "read_files".to_owned(),
                json!({ "files": files }).to_string(),
            )
        }
        Some(Tool::Grep(g)) => (
            "grep".to_owned(),
            json!({ "queries": g.queries, "path": g.path }).to_string(),
        ),
        Some(Tool::AskUserQuestion(a)) => {
            let questions: Vec<Value> = a
                .questions
                .iter()
                .map(|q| {
                    let (options, recommended_index, multi_select, supports_other) =
                        match &q.question_type {
                            Some(
                                api::ask_user_question::question::QuestionType::MultipleChoice(mc),
                            ) => (
                                mc.options
                                    .iter()
                                    .map(|o| o.label.clone())
                                    .collect::<Vec<_>>(),
                                mc.recommended_option_index,
                                mc.is_multiselect,
                                mc.supports_other,
                            ),
                            None => (vec![], 0, false, false),
                        };
                    json!({
                        "question": q.question,
                        "options": options,
                        "recommended_index": recommended_index,
                        "multi_select": multi_select,
                        "supports_other": supports_other,
                    })
                })
                .collect();
            (
                "ask_user_question".to_owned(),
                json!({ "questions": questions }).to_string(),
            )
        }
        Some(Tool::FileGlobV2(g)) => (
            "file_glob".to_owned(),
            json!({
                "patterns": g.patterns,
                "search_dir": g.search_dir,
                "limit": g.max_matches,
            })
            .to_string(),
        ),
        Some(Tool::ApplyFileDiffs(a)) => {
            let mut operations: Vec<Value> = Vec::new();
            for d in &a.diffs {
                operations.push(json!({
                    "op": "edit",
                    "file_path": d.file_path,
                    "search": d.search,
                    "replace": d.replace,
                }));
            }
            for f in &a.new_files {
                operations.push(json!({
                    "op": "create",
                    "file_path": f.file_path,
                    "content": f.content,
                }));
            }
            for f in &a.deleted_files {
                operations.push(json!({
                    "op": "delete",
                    "file_path": f.file_path,
                }));
            }
            (
                "apply_file_diffs".to_owned(),
                json!({ "summary": a.summary, "operations": operations }).to_string(),
            )
        }
        Some(Tool::WriteToLongRunningShellCommand(w)) => {
            use api::message::tool_call::write_to_long_running_shell_command::mode::Mode as M;
            let mode = match w.mode.as_ref().and_then(|m| m.mode.as_ref()) {
                Some(M::Raw(_)) => "raw",
                Some(M::Block(_)) => "block",
                _ => "line",
            };
            (
                "write_to_long_running_shell_command".to_owned(),
                json!({
                    "command_id": w.command_id,
                    "input": String::from_utf8_lossy(&w.input).to_string(),
                    "mode": mode,
                })
                .to_string(),
            )
        }
        Some(Tool::ReadDocuments(r)) => {
            let docs: Vec<Value> = r
                .documents
                .iter()
                .map(|d| {
                    json!({
                        "document_id": d.document_id,
                        "line_ranges": d.line_ranges.iter().map(|lr| json!({
                            "start": lr.start, "end": lr.end
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            (
                "read_documents".to_owned(),
                json!({ "documents": docs }).to_string(),
            )
        }
        Some(Tool::EditDocuments(e)) => {
            let diffs: Vec<Value> = e
                .diffs
                .iter()
                .map(|d| {
                    json!({
                        "document_id": d.document_id,
                        "search": d.search,
                        "replace": d.replace,
                    })
                })
                .collect();
            (
                "edit_documents".to_owned(),
                json!({ "diffs": diffs }).to_string(),
            )
        }
        Some(Tool::CreateDocuments(c)) => {
            let new_documents: Vec<Value> = c
                .new_documents
                .iter()
                .map(|d| json!({ "title": d.title, "content": d.content }))
                .collect();
            (
                "create_documents".to_owned(),
                json!({ "new_documents": new_documents }).to_string(),
            )
        }
        Some(Tool::SuggestNewConversation(s)) => (
            "suggest_new_conversation".to_owned(),
            json!({ "message_id": s.message_id }).to_string(),
        ),
        Some(Tool::SuggestPrompt(s)) => {
            use api::message::tool_call::suggest_prompt::DisplayMode;
            let (prompt, label) = match &s.display_mode {
                Some(DisplayMode::PromptChip(c)) => (c.prompt.clone(), c.label.clone()),
                Some(DisplayMode::InlineQueryBanner(b)) => (b.query.clone(), b.title.clone()),
                None => (String::new(), String::new()),
            };
            (
                "suggest_prompt".to_owned(),
                json!({ "prompt": prompt, "label": label }).to_string(),
            )
        }
        Some(Tool::OpenCodeReview(_)) => ("open_code_review".to_owned(), "{}".to_owned()),
        Some(Tool::TransferShellCommandControlToUser(t)) => (
            "transfer_shell_command_control_to_user".to_owned(),
            json!({ "reason": t.reason }).to_string(),
        ),
        Some(Tool::ReadSkill(r)) => {
            use api::message::tool_call::read_skill::SkillReference;
            let path = match &r.skill_reference {
                Some(SkillReference::SkillPath(p)) => p.clone(),
                Some(SkillReference::BundledSkillId(id)) => format!("bundled:{id}"),
                None => String::new(),
            };
            (
                "read_skill".to_owned(),
                json!({ "skill_path": path }).to_string(),
            )
        }
        Some(Tool::ReadShellCommandOutput(r)) => {
            use api::message::tool_call::read_shell_command_output::Delay;
            let delay_seconds = match &r.delay {
                Some(Delay::Duration(d)) => Some(d.seconds),
                Some(Delay::OnCompletion(_)) | None => None,
            };
            let mut args = json!({ "command_id": r.command_id });
            if let Some(s) = delay_seconds {
                args["delay_seconds"] = json!(s);
            }
            ("read_shell_command_output".to_owned(), args.to_string())
        }
        Some(other) => {
            let variant_name = format!("{other:?}")
                .split('(')
                .next()
                .unwrap_or("UnknownVariant")
                .to_owned();
            (format!("warp_internal_{}", variant_name), "{}".to_owned())
        }
        None => ("warp_internal_empty".to_owned(), "{}".to_owned()),
    };
    let args_value: Value =
        serde_json::from_str(&args_str).unwrap_or(Value::Object(Default::default()));
    (name, args_value)
}

// ---------------------------------------------------------------------------
// Tools 数组
// ---------------------------------------------------------------------------

/// 本轮 input 是否含 `/plan` 触发的 `UserQueryMode::Plan`。
///
/// per-turn 语义:只看本轮 `params.input` 是否带 Plan 标记。历史 task message
/// 当前的持久化路径(`make_user_query_message`)用 `..Default::default()` 写入
/// 上游 proto,**不带 mode 字段**;所以 plan 状态不会自动跨轮 sticky,用户每条
/// 想保持只读的 query 都需重新加 `/plan ` 前缀。这是有意为之的 MVP 形态:
/// - 实施成本最低(无需改 proto schema、无需新会话级状态机)
/// - 与 Claude Code `EnterPlanMode` 的"显式进入/退出"语义一致 —— 只是这里把
///   退出动作隐含在"下一条不带 /plan"
fn is_plan_mode_turn(input: &[AIAgentInput]) -> bool {
    input.iter().any(|i| {
        matches!(
            i,
            AIAgentInput::UserQuery {
                user_query_mode: UserQueryMode::Plan,
                ..
            }
        )
    })
}

/// Plan Mode 下硬过滤的写/执行类内置工具名。
///
/// 逻辑兜底,即使模型无视 `partials/plan_mode.j2` 的引导也无法触发副作用 ——
/// 工具不在 tool list 里就调用不到(provider 协议层会直接拒绝 unknown function)。
///
/// **没被 BLOCK 的写类工具**:`create_documents` / `edit_documents`。它们只动
/// Warp Drive 本地文档存储(AIDocumentModel),不碰文件系统、不跑命令,语义上
/// 恰好是 Plan Mode 的产出归档动作 —— 模型把最终 plan 沉淀为 Drive 文档,
/// 用户后续可在 Drive UI 中查看 / 编辑 / 拖入自建的 PLAN 文件夹复用。
///
/// 留下的只读 + Drive 写子集:`read_files / grep / file_glob_v2 /
/// read_shell_command_output / ask_user_question / read_skill / read_documents /
/// create_documents / edit_documents / webfetch / websearch / mcp/*`。
const PLAN_MODE_BLOCKED_TOOLS: &[&str] = &[
    "run_shell_command",
    "apply_file_diffs",
    "write_to_long_running_shell_command",
    "open_code_review",
    "transfer_shell_command_control_to_user",
    "suggest_prompt",
];

/// 列出本轮真正会喂给上游模型的 tool name(内置 REGISTRY + 当前 MCP 工具),
/// 与 `build_tools_array` 共享同一套 gating(LRC / `web_search_enabled` /
/// `suggest_new_conversation` / `plan_mode`)。供 `prompt_renderer` 注入到
/// system prompt,让模板按实际可用列表动态渲染,不再硬编码白/黑名单。
pub fn available_tool_names(params: &RequestParams) -> Vec<String> {
    let is_lrc = params.lrc_command_id.is_some();
    let web_enabled = params.web_search_enabled;
    let plan_mode = is_plan_mode_turn(&params.input);
    let mut names: Vec<String> = tools::REGISTRY
        .iter()
        .filter(|t| {
            if is_lrc && t.name == "run_shell_command" {
                return false;
            }
            if !web_enabled
                && (t.name == tools::webfetch::TOOL_NAME || t.name == tools::websearch::TOOL_NAME)
            {
                return false;
            }
            if t.name == "suggest_new_conversation" {
                return false;
            }
            if plan_mode && PLAN_MODE_BLOCKED_TOOLS.contains(&t.name) {
                return false;
            }
            true
        })
        .map(|t| t.name.to_owned())
        .collect();
    if let Some(ctx) = params.mcp_context.as_ref() {
        for (name, _description, _parameters) in tools::mcp::build_mcp_tool_defs(ctx) {
            names.push(name);
        }
    }
    names
}

fn build_tools_array(params: &RequestParams) -> Vec<GenaiTool> {
    // OpenWarp A2:LRC tag-in 场景剔除 `run_shell_command`,迫使模型选 PTY 操作类工具。
    //
    // 在 alt-screen 长命令(nvim/htop)+ 用户 tag-in 状态下,**模型最容易犯的错**是
    // 调 `run_shell_command` 跑 `taskkill nvim` / `Stop-Process nvim`(开新进程),
    // 这跟当前正在跑的 PTY 没关系,杀不到目标。**正确做法**是
    // `write_to_long_running_shell_command(command_id, input=":q\n", mode=raw)`,
    // 直接给当前 PTY 发指令。
    //
    // 实测带 system prompt 引导 + RunningCommand context prefix 都不够强,
    // 模型仍然倾向 run_shell_command(更简单)。最干净的硬约束就是从 tools 列表
    // 直接移除该工具,模型只能在 PTY 操作类工具中选。
    //
    // 其他工具保留(read_files/grep/ask_user_question 等),允许模型做必要的
    // 信息收集和反问。
    let is_lrc = params.lrc_command_id.is_some();
    let web_enabled = params.web_search_enabled;
    let plan_mode = is_plan_mode_turn(&params.input);
    // OpenWarp BYOP:`suggest_prompt` chip UI 已通过 view 层订阅
    // PromptSuggestionExecutorEvent 恢复(见 `terminal/view.rs::
    // handle_suggest_prompt_executor_event`),可以暴露给模型。
    // `suggest_new_conversation` 仍 filter:UX 没有现成弹窗组件,executor 已改为
    // fast-fail Cancelled(见 `action_model/execute/suggest_new_conversation.rs`),
    // filter 是冗余防御以避免无效调用噪声。
    // 动态占位替换:某些工具描述含 `{{year}}`(如 websearch,对齐 opencode
    // websearch.ts:30-32 的 description getter),build 时替换成当前年份。
    // 模型每次看到的描述都带正确年份,不会被训练数据里的旧年份污染。
    let current_year = chrono::Local::now().format("%Y").to_string();
    let mut out: Vec<GenaiTool> = tools::REGISTRY
        .iter()
        .filter(|t| {
            if is_lrc && t.name == "run_shell_command" {
                return false;
            }
            // BYOP web 工具按 profile.web_search_enabled gating(用户已关闭隐私
            // 开关时不暴露给上游模型,避免误调外网请求)。
            if !web_enabled
                && (t.name == tools::webfetch::TOOL_NAME || t.name == tools::websearch::TOOL_NAME)
            {
                return false;
            }
            // suggest_new_conversation:无 UI 实现,executor 在 OpenWarp 改为
            // fast-fail Cancelled。这里 filter 掉避免模型调用产生无意义的
            // tool_call→cancelled 往返(纯 token 浪费)。
            if t.name == "suggest_new_conversation" {
                return false;
            }
            // Plan Mode:`/plan` 触发的只读模式硬护栏,移除写/执行类工具。
            // 与 system prompt 的 plan_mode.j2 引导双重保险 —— 即便模型无视
            // 提示词,工具不在列表里也无法触发副作用(provider 协议层
            // 会直接拒绝 unknown function)。
            if plan_mode && PLAN_MODE_BLOCKED_TOOLS.contains(&t.name) {
                return false;
            }
            true
        })
        .map(|t| {
            let description = if t.description.contains("{{year}}") {
                t.description.replace("{{year}}", &current_year)
            } else {
                t.description.to_owned()
            };
            GenaiTool::new(t.name)
                .with_description(description)
                .with_schema((t.parameters)())
        })
        .collect();

    if let Some(ctx) = params.mcp_context.as_ref() {
        for (name, description, parameters) in tools::mcp::build_mcp_tool_defs(ctx) {
            out.push(
                GenaiTool::new(name)
                    .with_description(description)
                    .with_schema(parameters),
            );
        }
    }
    if is_lrc {
        log::info!(
            "[byop] LRC tag-in: tools array filtered (removed run_shell_command), \
             total tools={}",
            out.len()
        );
    }
    if plan_mode {
        log::info!(
            "[byop] Plan Mode: tools array filtered (removed write/exec tools: {:?}), \
             total tools={}",
            PLAN_MODE_BLOCKED_TOOLS,
            out.len()
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Client / 路由
// ---------------------------------------------------------------------------

/// 把 `AgentProviderApiType` 一对一映射到 genai `AdapterKind`。
fn adapter_kind_for(api_type: AgentProviderApiType) -> AdapterKind {
    match api_type {
        AgentProviderApiType::OpenAi => AdapterKind::OpenAI,
        AgentProviderApiType::OpenAiResp => AdapterKind::OpenAIResp,
        AgentProviderApiType::Gemini => AdapterKind::Gemini,
        AgentProviderApiType::Anthropic => AdapterKind::Anthropic,
        AgentProviderApiType::Ollama => AdapterKind::Ollama,
        AgentProviderApiType::DeepSeek => AdapterKind::DeepSeek,
    }
}

/// 规范化用户填写的 `base_url`,产出供 genai adapter 拼接 service path 的 endpoint URL。
///
/// genai 0.6.x 所有 adapter 都假设 endpoint 以 `/` 结尾、且已经包含版本路径段:
/// - Anthropic:`format!("{base_url}messages")` 期望 `…/v1/`
/// - Gemini:`format!("{base_url}models/{m}:streamGenerateContent")` 期望 `…/v1beta/`
/// - OpenAI / OpenAIResp / DeepSeek:`Url::join("chat/completions" 或 "responses")` 期望 `…/v1/`
/// - Ollama:`format!("{base_url}api/chat")` 期望根路径 `…/`
///
/// 用户实际三种填法:
/// 1. 纯 host(`https://ai.zerx.dev`)— 早期默认行为只补尾 `/` 会拼成 `https://ai.zerx.dev/messages`
///    漏掉 `/v1/` 导致 404。**这里按 api_type 自动追加默认版本路径段**(Anthropic/OpenAI 系→`/v1/`,
///    Gemini→`/v1beta/`,Ollama 不补)。
/// 2. 完整带版本路径(`https://ai.zerx.dev/v1`)— 仅补尾 `/`,不动 path。
/// 3. 留空 — 用 [`AgentProviderApiType::default_base_url`]。
fn normalize_endpoint_url(api_type: AgentProviderApiType, base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return api_type.default_base_url().to_owned();
    }

    // 解析失败(用户填了畸形 URL)→ 退化到原"补尾 /"行为,让上游报错而不是这里 panic。
    let parsed = match url::Url::parse(trimmed) {
        Ok(u) => u,
        Err(_) => {
            let stripped = trimmed.trim_end_matches('/');
            return format!("{stripped}/");
        }
    };

    // path == "/" 或为空 → 用户只填了 host,自动补上 api_type 默认版本路径段。
    if parsed.path() == "/" || parsed.path().is_empty() {
        // 从 default_base_url 抽 path 部分(如 "/v1/" / "/v1beta/" / "/")。
        let default_path = url::Url::parse(api_type.default_base_url())
            .ok()
            .map(|u| u.path().to_owned())
            .unwrap_or_else(|| "/".to_owned());
        let host_part = trimmed.trim_end_matches('/');
        return format!("{host_part}{default_path}");
    }

    // 用户已自带 path → 仅确保尾随 `/`(genai format!/Url::join 都依赖)。
    let stripped = trimmed.trim_end_matches('/');
    format!("{stripped}/")
}

/// 构造 genai Client。每次请求新建(开销低 — Client 内部只是 reqwest::Client + adapter 表)。
/// `ServiceTargetResolver` capture 当前请求的 endpoint/key/api_type 后,把每次 exec_chat_stream
/// 都强制路由到指定 AdapterKind,完全绕过 genai 默认的"按模型名识别"。
pub(super) fn build_client(
    api_type: AgentProviderApiType,
    base_url: String,
    api_key: String,
) -> Client {
    let adapter_kind = adapter_kind_for(api_type);
    let endpoint_url = normalize_endpoint_url(api_type, &base_url);
    log::info!("[byop] build_client: adapter={adapter_kind:?} endpoint_url={endpoint_url}");
    let key_for_resolver = api_key.clone();
    let resolver = ServiceTargetResolver::from_resolver_fn(
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            let ServiceTarget { model, .. } = service_target;
            let endpoint = Endpoint::from_owned(endpoint_url.clone());
            let auth = AuthData::from_single(key_for_resolver.clone());
            // 用我们指定的 AdapterKind 覆盖 genai 的"按模型名"识别结果,
            // 但保留 model_name 以便上游服务正确寻址模型。
            let model = ModelIden::new(adapter_kind, model.model_name);
            Ok(ServiceTarget {
                endpoint,
                auth,
                model,
            })
        },
    );

    // OpenWarp BYOP:SSE 流必须不带 gzip。`Accept-Encoding: gzip` 会让 nginx
    // 类代理把响应压缩,server 必须 flush 完整 deflate frame 客户端才能解出
    // 明文,流式语义被破坏成 ~K 字节 burst,体感"几百毫秒一卡"。zed/opencode
    // 用 native fetch / std HTTP 不主动协商 gzip on SSE,所以同代理无问题。
    //
    // 这里显式构造 `WebConfig` 即使 genai default 已经 `gzip=false`(fork 修改)。
    //
    // User-Agent 动态绑定当前应用名(取自 `ChannelState::app_id().application_name()`,
    // 由入口 bin 注册:`bin/oss.rs` → "OpenWarp";其它 channel 自带各自名称)。
    // 这样上游服务能识别请求来自哪个分支构建,后续若改名也会自动跟随。
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(value) = build_user_agent_header() {
        headers.insert(reqwest::header::USER_AGENT, value);
    }
    let web_config = WebConfig {
        gzip: false,
        default_headers: Some(headers),
        ..WebConfig::default()
    };
    Client::builder()
        .with_web_config(web_config)
        .with_service_target_resolver(resolver)
        .build()
}

/// 构造 BYOP 出站请求的 `User-Agent` 头,值形如:
/// - `OpenWarp/<git-tag>` —— release 构建有 `GIT_RELEASE_TAG` 注入时
/// - `OpenWarp` —— Dev / 本地构建无版本时
///
/// 应用名一律从 `ChannelState::app_id().application_name()` 取,确保与入口 bin
/// 注册的 `AppId` 一致(`bin/oss.rs` 注册 "OpenWarp")。
fn build_user_agent_header(
) -> Result<reqwest::header::HeaderValue, reqwest::header::InvalidHeaderValue> {
    let app_name = warp_core::channel::ChannelState::app_id()
        .application_name()
        .to_owned();
    let ua = match warp_core::channel::ChannelState::app_version() {
        Some(v) if !v.is_empty() => format!("{app_name}/{v}"),
        _ => app_name,
    };
    reqwest::header::HeaderValue::from_str(&ua)
}

/// 判定是否给 DashScope(阿里云百炼,OpenAI 兼容路径)注入 `enable_thinking: true`。
///
/// 对齐 opencode `transform.ts:931-938`(provider/transform.ts L926+ 的注释):
/// 「DashScope 默认不开 thinking,qwen3 / qwq / deepseek-r1 / kimi-k2.5 / qwen-plus
/// 等 reasoning 模型必须显式 `enable_thinking: true` 才会输出 reasoning_content」。
///
/// 命中条件(全部满足):
/// 1. `api_type == OpenAi`(DashScope 走 OpenAI 兼容路径)
/// 2. `effort_setting != Off`(用户主动关思考时尊重之,不注入)
/// 3. base_url 含 `dashscope.aliyuncs.com` / `dashscope.cn` / `dashscope-intl.aliyuncs.com`
/// 4. model_id 不含 `kimi-k2-thinking`(opencode 排除,该模型默认就 thinking)
/// 5. model_id 命中 reasoning 子串白名单:`qwen3` / `qwq` / `deepseek-r1` / `kimi-k2.5` /
///    `kimi-k2-` / `qwen-plus`(避免给 qwen-turbo / qwen2.5 等纯 chat 模型乱塞)
fn dashscope_needs_enable_thinking(
    api_type: AgentProviderApiType,
    base_url: &str,
    model_id: &str,
    effort_setting: crate::settings::ReasoningEffortSetting,
) -> bool {
    if !matches!(api_type, AgentProviderApiType::OpenAi) {
        return false;
    }
    if matches!(effort_setting, crate::settings::ReasoningEffortSetting::Off) {
        return false;
    }
    let url = base_url.to_ascii_lowercase();
    let is_dashscope = url.contains("dashscope.aliyuncs.com")
        || url.contains("dashscope.cn")
        || url.contains("dashscope-intl.aliyuncs.com");
    if !is_dashscope {
        return false;
    }
    let id = model_id.to_ascii_lowercase();
    if id.contains("kimi-k2-thinking") {
        return false;
    }
    id.contains("qwen3")
        || id.contains("qwq")
        || id.contains("deepseek-r1")
        || id.contains("kimi-k2.5")
        || id.contains("kimi-k2-")
        || id.contains("qwen-plus")
}

/// 判断 OpenAI 某 model 是否支持 24h Extended Cache(`prompt_cache_retention="24h"`)。
///
/// 官方文档(2026-05):
/// - GPT-5 系列 / GPT-5.x / GPT-5-codex / GPT-4.1 / o-series:支持 24h
/// - GPT-5.5+:**不支持** `in_memory`,默认 24h(不传亦可,但显式下发体验更佳)
/// - 旧型号(GPT-4o / GPT-3.5):`in_memory`(默认 5-10min)
///
/// model_id 处理原则:
/// 1. 以在上游官方文档明确点名的名称为准
/// 2. 包含则走 24h(含 prefix 匹配:“gpt-5-mini” / "gpt-5.5-pro" 都命中 "gpt-5")
/// 3. 不识别的 model 默认不含在列表里 → 走 in_memory(低风险默认)
///
/// **跨云厂商使用同一 OpenAI 兼容 endpoint** 的情况(OpenRouter / vLLM / lm-studio /
/// Azure OpenAI 等):model_id 可能被重命名,这里仅能以字面匹配推断,
/// 未命中时走 in_memory 默认。后续可考虑提供设置项手动覆盖。
fn openai_supports_extended_cache(model_id: &str) -> bool {
    let m = model_id.to_ascii_lowercase();
    // 官方文档明确点名支持 24h 的型号前缀集合。
    // 顺序不重要(any),但要避免跨型号误匹配。
    const PREFIXES: &[&str] = &[
        "gpt-5",   // gpt-5, gpt-5-mini, gpt-5.5, gpt-5.5-pro, gpt-5-codex 均命中
        "gpt-4.1", // gpt-4.1, gpt-4.1-mini, gpt-4.1-nano
        "o3",      // o3, o3-mini, o3-pro
        "o4",      // o4, o4-mini
        "o1",      // o1, o1-mini, o1-preview
    ];
    PREFIXES
        .iter()
        .any(|p| m.starts_with(p) || m.contains(&format!("/{p}")))
}

fn build_chat_options(
    api_type: AgentProviderApiType,
    base_url: &str,
    model_id: &str,
    effort_setting: crate::settings::ReasoningEffortSetting,
    extra_headers: Vec<(String, String)>,
    conversation_id: Option<&str>,
) -> ChatOptions {
    let mut opts = ChatOptions::default()
        .with_capture_content(true)
        .with_capture_tool_calls(true)
        .with_capture_reasoning_content(true)
        .with_capture_usage(true)
        // 让 genai 把 DeepSeek-style 模型在 content 中夹带的 <think>...</think>
        // 段抽出来归到 reasoning chunk,UI 显示更干净。仅对支持该格式的 adapter 生效。
        .with_normalize_reasoning_content(true);

    // Prompt caching(对应 opencode `applyCaching` OpenAI 兼容路径)。
    // genai 的 OpenAI / OpenAiResp adapter 不读 per-message cache_control,
    // 只认 `ChatOptions::prompt_cache_key` 与 `ChatOptions::cache_control`:
    //   - prompt_cache_key:OpenAI 把同 key 的请求路由到同一缓存分片,提升命中
    //     (`prompt_cache_key` field,见 `adapter_shared.rs:194` /
    //     `openai_resp/adapter_impl.rs:238`);用 conversation_id 作为稳定 key。
    //   - cache_control → 序列化为 `prompt_cache_retention` 字段(genai
    //     `adapter_shared.rs:197-205`):
    //       * Memory / Ephemeral → "in_memory"(旧型号默认 5-10min)
    //       * Ephemeral24h         → "24h"(GPT-5 / 4.1 / o-series Extended Cache)
    //       * Ephemeral5m / 1h     → None(不下发字段)
    //
    // **P0-5**:按 model_id 推断 TTL
    // - 官方点名支持 24h 的型号(GPT-5/5.x/5-codex / GPT-4.1 / o-series) → 24h
    // - 旧型号 / 未识别 model → in_memory(保证代理 / 本地服务不报错)
    // - 官方明确点名:GPT-5.5+ 不支持 in_memory,仅 24h。不识别时 fallback in_memory
    //   在 GPT-5.5+ 上反而会被拒;但另一面,上面的 prefix 匹配 "gpt-5" 会提前
    //   拦截该型号走 24h,逻辑上不会遗漏。
    //
    // Anthropic 走 per-message cache_control(在 build_chat_request 里),不在此处。
    // DeepSeek / Gemini / Ollama 服务端隐式缓存,跳过。
    if matches!(
        api_type,
        AgentProviderApiType::OpenAi | AgentProviderApiType::OpenAiResp
    ) {
        if let Some(cid) = conversation_id {
            if !cid.is_empty() {
                opts = opts.with_prompt_cache_key(cid.to_owned());
            }
        }
        let cc = if openai_supports_extended_cache(model_id) {
            CacheControl::Ephemeral24h
        } else {
            CacheControl::Ephemeral
        };
        opts = opts.with_cache_control(cc);
    }

    // **思考深度档位下发**(对齐 Zed `LanguageModelRequest::thinking_allowed` 各
    // provider 的处理:`thinking_allowed=false` 时所有 provider 都不发任何 thinking
    // 字段,Anthropic / Google / Bedrock 服务端默认就是关思考)。
    //
    // - **Auto**:不传,让 genai 走"模型名后缀推断"(OpenAI/Anthropic adapter 内部)。
    // - **Off + Anthropic / Gemini**:**完全跳过 `with_reasoning_effort`**,等同
    //   Auto + 模型名无 thinking 后缀。genai adapter 走 `(model, None)` 推断分支,
    //   不调 `insert_anthropic_reasoning` / `thinkingConfig`,不发 thinking 字段。
    //   ★ 这正好绕开 vendor genai `claude-opus-4-6` / `claude-sonnet-4-6`
    //   `support_adaptive` 强行注入 `thinking:{type:adaptive}` 的 bug
    //   (`lib/rust-genai/src/adapter/adapters/anthropic/adapter_impl.rs:121-135`
    //   不读 effort 是否为 `None`)。
    // - **Off + DeepSeek**:服务端 `thinking_mode` 默认开启(deepseek-v4-flash 等),
    //   需要显式 `extra_body.thinking.type=disabled` 才能关闭。OpenWarp 本地 fork
    //   的 genai 已支持 `ChatOptions::extra_body` 顶层合并。
    // - **Off + OpenAI / OpenAiResp**:走 `reasoning_effort: "none"` 路径
    //   (GPT-5 / codex 接受 `none` 档;o-series 由能力表过滤)。
    // - **非 Off + 模型不支持 reasoning**:跳过,避免给 claude-3-5-haiku / gpt-4o /
    //   gemini-1.5-pro 等老模型注入 thinking 参数被上游 400 拒绝。
    use crate::settings::ReasoningEffortSetting as RE;
    match (api_type, effort_setting) {
        // Auto:不下发任何参数
        (_, RE::Auto) => {}

        // Anthropic + Off:不发 thinking 字段
        (AgentProviderApiType::Anthropic, RE::Off) => {
            log::info!(
                "[byop] Anthropic Off → skip reasoning_effort (model={model_id}); \
                 no thinking field sent"
            );
        }

        // Gemini + Off:不发 thinkingConfig
        (AgentProviderApiType::Gemini, RE::Off) => {
            log::info!(
                "[byop] Gemini Off → skip reasoning_effort (model={model_id}); \
                 no thinkingConfig sent"
            );
        }

        // DeepSeek + Off:显式 disabled
        (AgentProviderApiType::DeepSeek, RE::Off) => {
            log::info!(
                "[byop] DeepSeek Off → extra_body thinking.type=disabled (model={model_id})"
            );
            opts = opts.with_extra_body(json!({"thinking": {"type": "disabled"}}));
        }

        // 其他(OpenAI / OpenAiResp / Ollama / 各 provider 非 Off 档):
        // 走能力表过滤后的 reasoning_effort 注入路径
        _ => {
            if let Some(effort) = effort_setting.to_genai() {
                if super::reasoning::model_supports_reasoning(api_type, model_id) {
                    log::info!(
                        "[byop] reasoning_effort injected: model={model_id} setting={effort_setting:?}"
                    );
                    opts = opts.with_reasoning_effort(effort);
                } else {
                    log::info!(
                        "[byop] reasoning_effort SKIPPED: model={model_id} not in capability list \
                         (api_type={api_type:?} setting={effort_setting:?}); request sent without thinking params"
                    );
                }
            }
        }
    }

    // DashScope(阿里云百炼)OpenAI 兼容路径需显式 `enable_thinking: true` 才会
    // 输出 reasoning。详见 `dashscope_needs_enable_thinking` 注释。
    // 与上面 DeepSeek Off 的 extra_body 互斥(DeepSeek 走 DeepSeek api_type,
    // DashScope 走 OpenAI api_type),不会同时 fire。
    if dashscope_needs_enable_thinking(api_type, base_url, model_id, effort_setting) {
        log::info!(
            "[byop] DashScope reasoning model → extra_body enable_thinking=true \
             (model={model_id} setting={effort_setting:?})"
        );
        opts = opts.with_extra_body(json!({"enable_thinking": true}));
    }
    if !extra_headers.is_empty() {
        opts = opts.with_extra_headers(extra_headers);
    }

    opts
}

fn map_genai_error(err: genai::Error) -> OpenAiCompatibleError {
    use genai::Error as G;
    match err {
        // 真·解析失败:JSON 反序列化阶段
        G::StreamParse { .. }
        | G::SerdeJson(_)
        | G::JsonValueExt(_)
        | G::InvalidJsonResponseElement { .. } => OpenAiCompatibleError::Decode(format!("{err}")),

        // 网络/流式发送阶段失败(reqwest 连接、TLS、DNS、超时、流中断等)
        G::WebStream { .. } | G::WebAdapterCall { .. } | G::WebModelCall { .. } => {
            OpenAiCompatibleError::Stream(format!("{err}"))
        }

        // 服务端返回的 HTTP 错误状态
        G::HttpError {
            status,
            body,
            canonical_reason,
        } => OpenAiCompatibleError::Status {
            status: status.as_u16(),
            body: if canonical_reason.is_empty() {
                body
            } else {
                format!("{canonical_reason}: {body}")
            },
        },

        // 其余(请求构造、鉴权、能力不支持等)归为通用错误,避免误导成"解析失败"
        other => OpenAiCompatibleError::Other(format!("{other}")),
    }
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

/// 标题生成所需的 BYOP 配置。可能与主请求同 provider 也可能不同(用户在 Profile
/// Editor 里独立选了 title_model)。
pub struct TitleGenInput {
    pub base_url: String,
    pub api_key: String,
    pub model_id: String,
    pub api_type: AgentProviderApiType,
    pub reasoning_effort: crate::settings::ReasoningEffortSetting,
}

pub struct ByopOutputInput {
    pub params: RequestParams,
    pub base_url: String,
    pub api_key: String,
    pub model_id: String,
    pub api_type: AgentProviderApiType,
    pub reasoning_effort: crate::settings::ReasoningEffortSetting,
    pub extra_headers: Vec<(String, String)>,
    pub task_id: String,
    pub target_task_id: String,
    pub needs_create_task: bool,
    pub lrc_command_id: Option<String>,
    pub lrc_should_spawn_subagent: bool,
    pub context_window: Option<u32>,
    pub cancellation_rx: futures::channel::oneshot::Receiver<()>,
}

/// `task_id`: conversation 的 root task id(controller 端从 history model 取)。
/// `target_task_id`: 本轮模型输出应该写入的 task id;普通对话等于 root,
/// CLI subagent 后续轮为已有 subtask。
/// `needs_create_task`: 仅首轮(root 还是 Optimistic)需要 emit `CreateTask`。
pub async fn generate_byop_output(
    input: ByopOutputInput,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let ByopOutputInput {
        params,
        base_url,
        api_key,
        model_id,
        api_type,
        reasoning_effort,
        extra_headers,
        task_id,
        target_task_id,
        needs_create_task,
        lrc_command_id,
        lrc_should_spawn_subagent,
        context_window,
        cancellation_rx: _cancellation_rx,
    } = input;

    let force_echo_reasoning = super::reasoning::model_requires_reasoning_echo(api_type, &model_id);
    let chat_req = build_chat_request(&params, force_echo_reasoning, api_type, &model_id);
    let conversation_id = params
        .conversation_token
        .as_ref()
        .map(|t| t.as_str().to_string())
        .unwrap_or_default();
    let chat_opts = build_chat_options(
        api_type,
        &base_url,
        &model_id,
        reasoning_effort,
        extra_headers,
        if conversation_id.is_empty() {
            None
        } else {
            Some(conversation_id.as_str())
        },
    );
    let client = build_client(api_type, base_url, api_key);
    let request_id = Uuid::new_v4().to_string();
    let mcp_context = params.mcp_context.clone();

    // ⚠️ BYOP 持久化关键:warp 自家路径下,以下 ClientAction 都是 server 端 emit
    // 让 client 端把 UserQuery / ToolCallResult 等"非模型产出"的 message
    // 写回 task.messages,从而让下一轮请求的 `params.tasks` snapshot 完整。
    //
    // BYOP 去云化客户端自管,server 端不存在,必须我们自己 emit 这些写回事件,
    // 否则下一轮 `compute_active_tasks` 只看到模型产出(reasoning/output/tool_call),
    // 缺失对应的 user_query 和 tool_call_result,模型 context 严重断裂。
    //
    // 这里在流开始后 emit 两类事件:
    //   1. AddMessagesToTask{UserQuery}    ← 当前轮所有 UserQuery input
    //   2. AddMessagesToTask{ToolCallResult} ← 当前轮所有 ActionResult input
    //
    // emit 时机必须在 CreateTask 之后(任务已升级为 Server 状态),
    // 在模型响应开始之前(UI 顺序:user 显示 → thinking/answer)。
    // OpenWarp:历史轮多模态保活。除 query 文本外,把当前轮 UserQuery.context 里的所有
    // multimodal binary(image / pdf / audio / ...)一并打包进 `UserQuery.context.images`
    // 持久化(proto 字段叫 images,语义上是通用 BinaryFile —— `bytes data + mime_type`,
    // 跟 opencode FilePart 等价),使 build_chat_request 下一轮重建 messages 时能从历史
    // message 上恢复 binary,继续以 ContentPart::Binary 注入上游(模型不支持的 mime 由
    // build_user_message_with_binaries 替换为 ERROR 文本,与 opencode unsupportedParts 一致)。
    // 上游 warp 自家路径不需要这步因为云端 server 持有 InputContext;BYOP 直连必须客户端自管。
    let pending_user_queries: Vec<(String, Vec<user_context::UserBinary>)> = params
        .input
        .iter()
        .filter_map(|i| match i {
            AIAgentInput::UserQuery { query, context, .. } => {
                let attachments = user_context::collect_user_attachments(context);
                Some((query.clone(), attachments.binaries))
            }
            _ => None,
        })
        .collect();
    // ToolCallResult 持久化:用 `tools::serialize_action_result` 把 ActionResult
    // 序列化为 JSON 字符串,装进 Message 的 server_message_data 字段
    // (warp protobuf 的 `tool_call_result.result` oneof 都是结构化 variant,
    // 没有通用 string 兜底;但 server_message_data 是自由字符串字段,刚好够用)。
    // 下一轮 build_chat_request 在 ToolCallResult 分支:result=None 时从
    // server_message_data 读 content,result=Some 时走 tools::serialize_result。
    let pending_tool_results: Vec<(String, String)> = params
        .input
        .iter()
        .filter_map(|i| match i {
            AIAgentInput::ActionResult { result, .. } => {
                let id = result.id.to_string();
                let content = tools::serialize_action_result(result).unwrap_or_else(|| {
                    serde_json::json!({ "result": result.result.to_string() }).to_string()
                });
                Some((id, content))
            }
            _ => None,
        })
        .collect();

    // INFO 级别一行总览 + 每条 message 一行简报(role + 文本长度 + tool 计数 + reasoning 标记),
    // 默认日志配置即可看到,便于诊断"历史是否完整传上去"等问题。
    //
    // 注:Anthropic 路径下，`build_chat_request` 会把 system 文本作为 `ChatMessage::system`
    // 推到 messages[0] 以便打 `cache_control`，所以 `chat_req.system` 会是 None、`system_len`
    // 显示为 0；实际 system 内容仍然在 messages[0] 里(看下面逐条报告)。为避免误
    // 导诊断者，这里加上 `system_in_messages_head` 提示。
    let system_in_head = matches!(api_type, AgentProviderApiType::Anthropic)
        && chat_req
            .messages
            .first()
            .map(|m| matches!(m.role, ChatRole::System))
            .unwrap_or(false);
    log::info!(
        "[byop] adapter={:?} model={} system_len={} messages={} tools={} system_in_messages_head={}",
        adapter_kind_for(api_type),
        model_id,
        chat_req.system.as_deref().map(str::len).unwrap_or(0),
        chat_req.messages.len(),
        chat_req.tools.as_ref().map(|t| t.len()).unwrap_or(0),
        system_in_head,
    );
    for (idx, m) in chat_req.messages.iter().enumerate() {
        let role = format!("{:?}", m.role);
        let text_len = m.content.first_text().map(str::len).unwrap_or(0);
        let tc_count = m.content.tool_calls().len();
        let tr_count = m.content.tool_responses().len();
        // reasoning_content 检测 — genai 把它存为 ContentPart::ReasoningContent,
        // 没有公开 getter,这里通过 size() 与 first_text+tool_count 的和差异粗判。
        log::info!(
            "[byop]  [{idx}] role={role} text_len={text_len} tool_calls={tc_count} tool_responses={tr_count}"
        );
    }

    // 诊断:构造包含 system / messages / tools 的完整 ChatRequest JSON dump,保存到
    // stream 闭包。真实 Anthropic wire body 会由 genai adapter 再转换一层,但这里已经
    // 覆盖所有传入 BYOP 的原始字符串,足够定位非法 escape 来自 prompt、工具描述、
    // schema 还是 tool result。
    let diag_body_json = serde_json::to_string(&json!({
        "model": &model_id,
        "chat_request": &chat_req,
    }))
    .unwrap_or_default();
    log::info!("[byop] diag_body_approx_len={}", diag_body_json.len());
    log::info!("[byop-diag] full_request_json={diag_body_json}");

    // 主动扫描原始文本里的"可疑反斜杠序列":serde_json 把源字符串里的字面
    // `\` 序列化为 `\\`,所以 wire body 里出现"两个连续反斜杠 + u/x" 才意味着
    // 原文有字面 `\u` / `\x`,这是 proxy 误"还原 `\\u` → `\u`"触发 invalid escape
    // 的真实风险点。源字符串里的 `\n` / `\r` / `\t` 经 serde_json 输出为单个反斜杠 +
    // 字母,本身就是合法 JSON escape,proxy 不会再二次还原,不算可疑。
    fn scan_suspicious_backslash(label: &str, s: &str) {
        let bytes = s.as_bytes();
        let mut bs_hits: Vec<(usize, String)> = Vec::new();
        let mut ctrl_hits: Vec<(usize, u8)> = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            // 字面 `\\u` / `\\x` 序列(源字符串中含 `\u` / `\x`)。
            if b == b'\\'
                && i + 2 < bytes.len()
                && bytes[i + 1] == b'\\'
                && matches!(bytes[i + 2], b'u' | b'x')
            {
                let end = (i + 10).min(bytes.len());
                let snippet = String::from_utf8_lossy(&bytes[i..end]).to_string();
                if bs_hits.len() < 5 {
                    bs_hits.push((i, snippet));
                }
                // 跳过这一对,避免对同一位置触发多次。
                i += 3;
                continue;
            }
            // raw 控制字符(byte 0x00-0x08, 0x0B-0x0C, 0x0E-0x1F)。
            // serde_json 会 escape 为 `\u00XX`,合法 JSON;但部分 strict proxy
            // 或经过 base64 / 中间编码层时这些字节最容易出错。
            if (b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r')) && ctrl_hits.len() < 10 {
                ctrl_hits.push((i, b));
            }
            i += 1;
        }
        if !bs_hits.is_empty() {
            log::warn!("[byop] {label} suspicious literal '\\\\u'/'\\\\x' patterns: {bs_hits:?}");
        }
        if !ctrl_hits.is_empty() {
            log::warn!("[byop] {label} contains raw control chars (offset, byte): {ctrl_hits:?}");
        }
    }
    scan_suspicious_backslash("full_request_json", &diag_body_json);
    if let Some(sys) = chat_req.system.as_deref() {
        scan_suspicious_backslash("system", sys);
    }
    for (idx, m) in chat_req.messages.iter().enumerate() {
        if let Some(t) = m.content.first_text() {
            scan_suspicious_backslash(&format!("msg[{idx}]"), t);
        }
    }

    let stream = async_stream::stream! {
        // 1) StreamInit — 始终先发,UI 能立刻显示 "thinking..."
        yield Ok(api::ResponseEvent {
            r#type: Some(api::response_event::Type::Init(
                api::response_event::StreamInit {
                    request_id: request_id.clone(),
                    conversation_id,
                    run_id: String::new(),
                },
            )),
        });

        // 2) 首轮:CreateTask 升级 Optimistic root → Server。
        if needs_create_task {
            yield Ok(create_task_event(&task_id));
        }

        // 3) 持久化 input 里的 UserQuery / ToolCallResult 到 task.messages。
        //    (warp server 路径由后端 emit;BYOP 客户端必须自己 emit,见上方注释。)
        //    tag-in 首轮先写 root,再由下面的 spawn 分支复制到新 subtask;已有 CLI
        //    subagent 的后续轮直接写 target_task_id。
        let persistence_task_id = if lrc_should_spawn_subagent {
            task_id.as_str()
        } else {
            target_task_id.as_str()
        };
        let mut persistence_messages: Vec<api::Message> = Vec::new();
        for (q, imgs) in &pending_user_queries {
            persistence_messages.push(make_user_query_message(
                persistence_task_id,
                &request_id,
                q.clone(),
                imgs,
            ));
        }
        for (call_id, content) in &pending_tool_results {
            persistence_messages.push(make_tool_call_result_message(
                persistence_task_id,
                &request_id,
                call_id.clone(),
                content.clone(),
            ));
        }
        if !persistence_messages.is_empty() {
            yield Ok(make_add_messages_event(persistence_task_id, persistence_messages));
        }

        // 3.5) LRC subagent spawn(对齐上游云端的 cli subagent 注入路径)。
        //
        // 当请求来自 alt-screen + agent tagged-in 状态时,`lrc_command_id` 携带当前 LRC
        // block 的 id 字符串。此处客户端合成两条事件:
        //   a) AddMessagesToTask(root, [<虚拟 subagent tool_call>])
        //      在 root.messages 里挂一条 ToolCall::Subagent { task_id=<新 subtask>,
        //      metadata: Cli { command_id }, payload: "" }。
        //      conversation `Task::new_subtask` 会从 parent.messages 里按 task_id 匹配
        //      这条 subagent_call,提取出 SubagentParams 挂到 subtask。
        //   b) CreateTask(api::Task { id=<新 subtask>, dependencies.parent_task_id=root })
        //      触发 `apply_client_action::CreateTask`,因 parent_id 非空走 `new_subtask`,
        //      接着 emit `BlocklistAIHistoryEvent::CreatedSubtask` →
        //      `cli_controller::handle_history_model_event` 看到 cli_subagent_block_id
        //      非空,emit `CLISubagentEvent::SpawnedSubagent` → terminal_view 创建
        //      `CLISubagentView` 浮窗,挂进 `cli_subagent_views` map。
        //
        // 切换后续 chunk emit 的 task_id 到 subtask_id,让模型 reasoning/output/tool_call
        // 全部进 subtask,subagent_view 据此渲染浮窗内容。
        //
        // 时序约束:必须在 root CreateTask + UserQuery 持久化之后,模型流之前。
        // 否则 conversation 找不到 root task / 找不到 user query 引用对。
        let mut current_task_id = if lrc_should_spawn_subagent {
            task_id.clone()
        } else {
            target_task_id.clone()
        };
        if lrc_should_spawn_subagent {
            let Some(command_id) = lrc_command_id.clone() else {
                log::warn!("[byop] LRC spawn requested without command_id");
                yield Err(Arc::new(AIApiError::Other(anyhow::anyhow!(
                    "BYOP LRC spawn requested without command_id"
                ))));
                return;
            };
            let subtask_id = Uuid::new_v4().to_string();
            let tool_call_id = Uuid::new_v4().to_string();
            log::info!(
                "[byop] LRC tag-in: spawning cli subagent subtask={subtask_id} \
                 command_id={command_id} parent={task_id}"
            );

            let subagent_tool = api::message::tool_call::Tool::Subagent(
                api::message::tool_call::Subagent {
                    task_id: subtask_id.clone(),
                    payload: String::new(),
                    metadata: Some(
                        api::message::tool_call::subagent::Metadata::Cli(
                            api::message::tool_call::subagent::CliSubagent {
                                command_id,
                            },
                        ),
                    ),
                },
            );
            let subagent_msg = make_tool_call_message(
                &task_id,
                &request_id,
                &tool_call_id,
                subagent_tool,
            );
            // a) 把 subagent tool_call 挂到 root.messages,供 new_subtask 反查 SubagentParams。
            yield Ok(make_add_messages_event(&task_id, vec![subagent_msg]));
            // b) 创建带 parent_task_id 的 subtask;conversation 检测 parent_id 非空 →
            //    走 `Task::new_subtask` 路径,自动绑定 SubagentParams。
            yield Ok(create_subtask_event(&subtask_id, &task_id));

            // c) OpenWarp A1:把当前轮的 UserQuery 也复制一份到 subtask,初始化 subtask 的
            //    exchange.output.messages。否则 CLISubagentView 渲染时 subtask 的 exchanges
            //    output 为空,浮窗永远只显示 49.6 高度的空对话框,看不到任何内容。
            //    上游云端在 cli subagent 任务上有完整 ClientAction 序列填 exchange.output,
            //    BYOP 客户端自管必须显式注入。
            //
            //    只复制本轮 UserQuery(`pending_user_queries`),不动 root 的副本(root
            //    保留 user query 引用以避免 exchange.input 为空导致状态机错乱)。
            //    后续模型 chunks 走 `current_task_id = subtask_id`,append 到这个起点之后。
            if !pending_user_queries.is_empty() {
                let mut subtask_messages: Vec<api::Message> = Vec::new();
                for (q, imgs) in &pending_user_queries {
                    subtask_messages.push(make_user_query_message(
                        &subtask_id,
                        &request_id,
                        q.clone(),
                        imgs,
                    ));
                }
                yield Ok(make_add_messages_event(&subtask_id, subtask_messages));
            }

            // 后续 chunk emit 切到 subtask。
            current_task_id = subtask_id;
        }

        log::info!("[byop] opening stream: model={model_id}");
        let mut sdk_stream = match client
            .exec_chat_stream(&model_id, chat_req, Some(&chat_opts))
            .await
        {
            Ok(resp) => {
                log::info!("[byop] stream opened OK (HTTP request accepted)");
                resp.stream
            }
            Err(e) => {
                let mapped = map_genai_error(e);
                log::error!("[byop] open stream failed: {mapped:#}");
                yield Err(Arc::new(AIApiError::Other(anyhow::anyhow!(
                    "BYOP open stream failed: {mapped}"
                ))));
                return;
            }
        };

        // 流式状态:文本 / 推理各自的 message id 在第一次 chunk 到达时生成,
        // 之后的 chunk 走 AppendToMessageContent 增量追加。
        let mut text_msg_id: Option<String> = None;
        let mut reasoning_msg_id: Option<String> = None;
        // tool_call 按 call_id 累积 — genai 流式发的 ToolCallChunk 已带完整 ToolCall
        // (since 0.4.0 行为),但跨 chunk 同一 call_id 可能多次出现 args 增量,
        // 用 HashMap 按 id 累积后在流末统一 emit。
        let mut tool_bufs: HashMap<String, ToolCall> = HashMap::new();
        // call_id → 首帧占位 ToolCall message 的 id。
        // 首次 ToolCallChunk 到达且可解析时立即 emit 一条占位卡(让 UI 在 stream End
        // 之前就能看到"调用 X 工具"反馈),流末用 update_message 原地刷新为最终 args。
        // 不在表里的 call_id(首帧 parse 失败 / web 工具)走老路径在 End 后一次性 emit。
        let mut tool_msg_ids: HashMap<String, String> = HashMap::new();
        // call_id → 上次 update_message 增量刷新的时刻。
        // 长 args 工具(create_or_edit_document、长 grep query)args 跨多 chunk 累积时,
        // 节流 ≥ 200ms reparse + update,体感跟文本流一样连续而不是首帧定格到 End。
        let mut tool_last_update: HashMap<String, Instant> = HashMap::new();
        // 增量刷新节流阈值:小于此值的连续 chunk 不再 update_message,避免频繁 UI 重排。
        // 注:SDK stream 每个 ChatStreamEvent 独立 await,多 tool 并发时本就是顺序到达,
        // 同 tick batch emit 在此层意义不大;真正降抖在节流上,这条注释提醒后续不要瞎引入 batch。
        const TOOL_ARGS_UPDATE_THROTTLE_MS: u64 = 200;
        // 诊断:统计 stream 各类事件计数,流末打 INFO log。
        // 用于排查"消息静默消失"——如果 chunk_count=0 且 tool_count=0,说明上游返回空内容。
        let mut start_count: u32 = 0;
        let mut chunk_count: u32 = 0;
        let mut chunk_bytes: usize = 0;
        let mut reasoning_count: u32 = 0;
        let mut reasoning_bytes: usize = 0;
        let mut tool_chunk_count: u32 = 0;
        let mut end_count: u32 = 0;
        let mut other_count: u32 = 0;
        // 累积本轮 token 使用量。genai 在 ChatStreamEvent::End 事件里携带
        // captured_usage(Option<Usage>),其 prompt_tokens 是本轮整段 history
        // (Anthropic / OpenAI 都按"完整请求 prompt"计),completion_tokens 是模型输出。
        // 二者相加除以 context_window 即为"context 占用率",和 warp 自家 server 路径语义一致。
        let mut captured_prompt_tokens: i32 = 0;
        let mut captured_completion_tokens: i32 = 0;
        // P0-6 prompt cache 命中率监控:从 genai `Usage.prompt_tokens_details` 里拼
        // 出 Anthropic / OpenAI / Gemini 返回的 cache_read / cache_create 字段。
        // 详见 stream End 处理逻辑。DeepSeek / Ollama 本身不走 cache 字段,后续
        // 依然保持 0。
        let mut captured_cache_read_tokens: i32 = 0;
        let mut captured_cache_create_tokens: i32 = 0;

        while let Some(item) = sdk_stream.next().await {
            let event = match item {
                Ok(ev) => ev,
                Err(e) => {
                    let mapped = map_genai_error(e);
                    let err_text = format!("{mapped:#}");
                    log::error!("[byop] stream chunk error: {err_text}");
                    log::error!("[byop-diag] full_request_json_on_error={diag_body_json}");
                    // 从错误消息里 parse "column N",dump diag_body_json 该位置 ±200 char 上下文 + 字节 hex。
                    if let Some(col) = err_text
                        .split("column ")
                        .nth(1)
                        .and_then(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<usize>().ok())
                    {
                        let body = &diag_body_json;
                        let byte_len = body.len();
                        let start = col.saturating_sub(200).min(byte_len);
                        let end = (col + 200).min(byte_len);
                        let context = body.get(start..end).unwrap_or("(slice failed: 非 char 边界)");
                        log::error!(
                            "[byop] error column={col} diag_body_len={byte_len} context[{start}..{end}]={context:?}"
                        );
                        let hex_start = col.saturating_sub(20).min(byte_len);
                        let hex_end = (col + 20).min(byte_len);
                        if let Some(slice) = body.as_bytes().get(hex_start..hex_end) {
                            log::error!("[byop] error bytes[{hex_start}..{hex_end}] hex={slice:02x?}");
                        }
                    }
                    yield Err(Arc::new(AIApiError::Other(anyhow::anyhow!(
                        "BYOP stream error: {mapped}"
                    ))));
                    return;
                }
            };

            match event {
                ChatStreamEvent::Start => {
                    // unit event;UI 已通过 StreamInit 显示 thinking,这里 no-op
                    start_count += 1;
                }
                ChatStreamEvent::Chunk(c) if !c.content.is_empty() => {
                    chunk_count += 1;
                    chunk_bytes += c.content.len();
                    if let Some(id) = text_msg_id.clone() {
                        yield Ok(make_append_event(&current_task_id, &id, AppendKind::Text(c.content)));
                    } else {
                        let new_id = Uuid::new_v4().to_string();
                        let mut msg = make_agent_output_message(&current_task_id, &request_id, c.content);
                        msg.id = new_id.clone();
                        text_msg_id = Some(new_id);
                        yield Ok(make_add_messages_event(&current_task_id, vec![msg]));
                    }
                }
                ChatStreamEvent::Chunk(_) => {}
                ChatStreamEvent::ReasoningChunk(c) if !c.content.is_empty() => {
                    reasoning_count += 1;
                    reasoning_bytes += c.content.len();
                    if let Some(id) = reasoning_msg_id.clone() {
                        yield Ok(make_append_event(&current_task_id, &id, AppendKind::Reasoning(c.content)));
                    } else {
                        let new_id = Uuid::new_v4().to_string();
                        let mut msg = make_reasoning_message(&current_task_id, &request_id, c.content);
                        msg.id = new_id.clone();
                        reasoning_msg_id = Some(new_id);
                        yield Ok(make_add_messages_event(&current_task_id, vec![msg]));
                    }
                }
                ChatStreamEvent::ReasoningChunk(_) => {}
                ChatStreamEvent::ToolCallChunk(tc) => {
                    tool_chunk_count += 1;
                    let mut call = tc.tool_call;
                    // 极个别 provider(自建 ollama 代理等)不发 call_id,本地 uuid 兜底。
                    if call.call_id.is_empty() {
                        call.call_id = Uuid::new_v4().to_string();
                    }
                    // 首次见到该 call_id → 立即 push 占位 ToolCall 消息到 pending_placeholders,
                    // 让 UI 在 stream End 之前就出现"调用 X 工具"卡片。
                    // 多 tool 同 tick 内来时:本循环结束前统一 batch emit 一次 add_messages,
                    // 减少 view tree 重排次数。
                    // 已在表里(占位已发)且 args 又来新 chunk → 节流 ≥ 200ms reparse + update_message
                    // 增量刷新 args,长 args 工具(create_or_edit_document、长 grep 等)体感连续。
                    // web 工具(webfetch/websearch)走自己的 loading 帧链路(L2102 区域),
                    // 这里跳过避免双卡。
                    // todowrite 走 BYOP todo 拦截器,合成 Message::UpdateTodos 触发 chip,
                    // 这里也跳过占位避免出现一张无意义的"调用 todowrite"卡。
                    if call.fn_name != tools::webfetch::TOOL_NAME
                        && call.fn_name != tools::websearch::TOOL_NAME
                        && call.fn_name != tools::todowrite::TOOL_NAME
                    {
                        if let Some(msg_id) = tool_msg_ids.get(&call.call_id).cloned() {
                            // 已 emit 占位 → 节流增量刷新。
                            let now = Instant::now();
                            let last = tool_last_update.get(&call.call_id).copied();
                            let elapsed_ok = last
                                .map(|t| now.duration_since(t).as_millis() as u64 >= TOOL_ARGS_UPDATE_THROTTLE_MS)
                                .unwrap_or(true);
                            if elapsed_ok {
                                if let Ok(parsed) =
                                    parse_incoming_tool_call(&call, mcp_context.as_ref())
                                {
                                    let mut updated = make_tool_call_message(
                                        &current_task_id,
                                        &request_id,
                                        &call.call_id,
                                        parsed,
                                    );
                                    updated.id = msg_id;
                                    tool_last_update.insert(call.call_id.clone(), now);
                                    yield Ok(make_update_message_event(
                                        &current_task_id,
                                        updated,
                                        vec!["tool_call".to_owned()],
                                    ));
                                }
                                // reparse 失败(intermediate 状态):静默,等下次 chunk。
                            }
                        } else if let Ok(parsed) =
                            parse_incoming_tool_call(&call, mcp_context.as_ref())
                        {
                            // 首次 parse 成功 → 立即 emit 占位卡。
                            // 每个 chunk 在未 emit 占位前都会重 parse(即"retry on every
                            // chunk"),所以即便首帧 args 不全,后续任意 chunk 完整时
                            // 都会立刻触发占位 emit—— 这就是 P1-4 的覆盖路径,
                            // 不再需要 generic placeholder variant。
                            let msg_id = Uuid::new_v4().to_string();
                            let mut placeholder = make_tool_call_message(
                                &current_task_id,
                                &request_id,
                                &call.call_id,
                                parsed,
                            );
                            placeholder.id = msg_id.clone();
                            tool_msg_ids.insert(call.call_id.clone(), msg_id);
                            tool_last_update.insert(
                                call.call_id.clone(),
                                Instant::now(),
                            );
                            yield Ok(make_add_messages_event(
                                &current_task_id,
                                vec![placeholder],
                            ));
                        }
                        // 首帧 parse 失败(args 还不完整 / 未知工具):暂不 emit,
                        // 等下次 chunk 再尝试或 End 时走老路径,避免视觉抖动。
                    }
                    // 同一 call_id 多次 chunk:后到的覆盖(genai 已合并 args)。
                    tool_bufs.insert(call.call_id.clone(), call);
                }
                ChatStreamEvent::End(end) => {
                    end_count += 1;
                    // genai >= 0.4.0 的 captured_content 含 tool_calls。
                    // 优先用 captured_content 里的 tool_calls(更完整),
                    // 否则用 streaming 中累积的 tool_bufs。
                    if let Some(content) = end.captured_content.as_ref() {
                        for call in content.tool_calls() {
                            tool_bufs.entry(call.call_id.clone()).or_insert_with(|| call.clone());
                        }
                    }
                    if let Some(usage) = end.captured_usage.as_ref() {
                        // 多次 End 取最大值兜底(理论上单次 stream 只有一次 End)。
                        if let Some(p) = usage.prompt_tokens {
                            captured_prompt_tokens = captured_prompt_tokens.max(p);
                        }
                        if let Some(c) = usage.completion_tokens {
                            captured_completion_tokens = captured_completion_tokens.max(c);
                        }
                        // P0-6 prompt cache 命中率监控:Anthropic / OpenAI / Gemini 在
                        // `prompt_tokens_details` 中分别返回 `cache_read_input_tokens`
                        // (Anthropic) / `cached_tokens`(OpenAI) / `cachedContentTokenCount`
                        // (Gemini)。genai 已统一映射到 `cached_tokens`。
                        // 同样 `cache_creation_tokens` 仅 Anthropic 提供(写入计费提示)。
                        // 多次 End 取最大值兜底,语义同 prompt/completion。
                        if let Some(details) = usage.prompt_tokens_details.as_ref() {
                            if let Some(r) = details.cached_tokens {
                                captured_cache_read_tokens =
                                    captured_cache_read_tokens.max(r);
                            }
                            if let Some(w) = details.cache_creation_tokens {
                                captured_cache_create_tokens =
                                    captured_cache_create_tokens.max(w);
                            }
                        }
                    }
                }
                _ => {
                    other_count += 1;
                    // ThoughtSignatureChunk 等暂不处理(Gemini 3 thoughts 需要回传给后续轮次,
                    // 当前 BYOP 不持久化 thought_signatures,接受降级)
                }
            }
        }

        // 流统计 INFO log。chunk_count=0 && tool_count=0 时上游返回为空,
        // 大概率是 model_id 不被识别 / max_tokens 缺失 / Anthropic API 兼容代理返回 200 但 body 空。
        let total_tools = tool_bufs.len();
        log::info!(
            "[byop] stream stats: start={start_count} chunks={chunk_count} ({chunk_bytes}B) \
             reasoning={reasoning_count} ({reasoning_bytes}B) tool_chunks={tool_chunk_count} \
             ends={end_count} other={other_count} captured_tools={total_tools}"
        );
        // P0-6 prompt cache 命中率日志(只在 provider 返回 cache 字段时打)。
        // ratio = cache_read / (prompt_tokens.max(1)) 表示本轮 input 中有多少比例直接
        // 命中了缓存。create > 0 表示本轮有 cache write,write 价 ≈ 1.25x base(5m)或
        // 2x base(1h)。read 价 ≈ 0.1x base,长期看只要 ≥ 1 次复用就回本。
        // 用 ratio 判定 P0 优化是否生效:同一对话第 2+ 轮应当看到 ratio 显著上升。
        //
        // **P2-16**:额外拼一个 `compaction=` 标识。压缩本身会重写历史使 messages
        // prefix 跨压缩之前后不一致 → 压缩后首轮必然 cache miss。在日志里输出该
        // 信号让后期分析(`script/analyze-prompt-cache.ps1`)能区分“正常 miss”与
        // “压缩导致 miss”,避免误伤。
        if captured_cache_read_tokens > 0 || captured_cache_create_tokens > 0 {
            let denom = captured_prompt_tokens.max(1);
            let read_ratio = captured_cache_read_tokens as f32 / denom as f32;
            let create_ratio = captured_cache_create_tokens as f32 / denom as f32;
            // 压缩状态:none → 未启用 / inactive → 启用但本轮未变化 /
            // active(已 hide 的 message id 个数) → 本轮走了压缩路径。
            let compaction_label = match params.compaction_state.as_ref() {
                None => "none".to_owned(),
                Some(s) => {
                    let hidden = s.hidden_message_ids().len();
                    if hidden == 0 {
                        "inactive".to_owned()
                    } else {
                        format!("active(hidden={hidden})")
                    }
                }
            };
            log::info!(
                "[byop-cache] prompt_tokens={captured_prompt_tokens} \
                 cache_read={captured_cache_read_tokens} ({:.1}%) \
                 cache_create={captured_cache_create_tokens} ({:.1}%) \
                 model={model_id} compaction={compaction_label}",
                read_ratio * 100.0,
                create_ratio * 100.0,
            );
        }
        if chunk_count == 0 && reasoning_count == 0 && total_tools == 0 {
            log::warn!(
                "[byop] stream returned 0 content / 0 reasoning / 0 tool_calls — \
                 上游可能返回空响应(model_id 错? max_tokens 缺? proxy 异常?)"
            );
        }

        // 流结束:把累积的 tool_calls 一次性发出。
        let mut final_messages: Vec<api::Message> = Vec::new();
        for call in tool_bufs.into_values() {
            // 诊断:dump 模型实际发的 tool_call raw payload
            // (call_id / fn_name / fn_arguments JSON 原文 + 类型标注),
            // 便于核对模型是否按 schema 出入参(常见问题:bool 字段被字符串化、
            // 数字被加引号、嵌套对象塌成字符串等)。
            // debug 级:只在排查 schema 问题时开 RUST_LOG=debug,平时不污染 INFO。
            // info 级保留一行不带 args 的简短摘要,便于看流式时序。
            log::info!(
                "[byop] tool_call_in: name={} call_id={}",
                call.fn_name,
                call.call_id,
            );
            if log::log_enabled!(log::Level::Debug) {
                let args_repr = if call.fn_arguments.is_string() {
                    format!("string({:?})", call.fn_arguments.as_str().unwrap_or(""))
                } else {
                    format!(
                        "{}({})",
                        match &call.fn_arguments {
                            Value::Object(_) => "object",
                            Value::Array(_) => "array",
                            Value::Bool(_) => "bool",
                            Value::Number(_) => "number",
                            Value::Null => "null",
                            Value::String(_) => "string",
                        },
                        call.fn_arguments
                    )
                };
                log::debug!(
                    "[byop] tool_call_in_args: name={} call_id={} args={}",
                    call.fn_name,
                    call.call_id,
                    args_repr,
                );
            }

            // OpenWarp BYOP todowrite 拦截:不映射到 protobuf executor,合成
            // `Message::UpdateTodos` 直接写 conversation.todo_lists 触发 chip + popup
            // UI(对齐 server-side ClientAction::AddMessagesToTask::UpdateTodos 路径)。
            // 然后追加 carrier ToolCall + ToolCallResult 给模型 unblock。
            if call.fn_name == tools::todowrite::TOOL_NAME {
                let args_str = if call.fn_arguments.is_string() {
                    call.fn_arguments.as_str().unwrap_or("").to_owned()
                } else {
                    call.fn_arguments.to_string()
                };

                match tools::todowrite::build_update_todos_messages(
                    &args_str,
                    &current_task_id,
                    &request_id,
                ) {
                    Ok(todo_msgs) if !todo_msgs.is_empty() => {
                        // 直接 yield UpdateTodos 让 UI 实时更新 chip。
                        // 走 AddMessagesToTask:apply_client_action 路径会
                        // 命中 Message::UpdateTodos 分支 → update_todo_list_from_todo_op
                        // → emit BlocklistAIHistoryEvent::UpdatedTodoList,UI 自动刷新。
                        yield Ok(make_add_messages_event(&current_task_id, todo_msgs));
                        let result_payload =
                            tools::todowrite::success_result_to_json("todo list updated");
                        let result_content = serde_json::to_string(&result_payload)
                            .unwrap_or_else(|_| r#"{"status":"ok"}"#.to_owned());
                        final_messages.push(make_tool_call_carrier_message(
                            &current_task_id,
                            &request_id,
                            &call.call_id,
                            &call.fn_name,
                            &args_str,
                        ));
                        final_messages.push(make_tool_call_result_message(
                            &current_task_id,
                            &request_id,
                            call.call_id.clone(),
                            result_content,
                        ));
                    }
                    Ok(_) => {
                        // 空 todos 数组:不 emit UpdateTodos,但仍要给模型 result
                        // 否则下一轮 chat 会卡(模型等 tool_call_id 的 result)。
                        let result_payload = tools::todowrite::success_result_to_json("no todos");
                        let result_content = serde_json::to_string(&result_payload)
                            .unwrap_or_else(|_| r#"{"status":"ok","message":"no todos"}"#.to_owned());
                        final_messages.push(make_tool_call_carrier_message(
                            &current_task_id,
                            &request_id,
                            &call.call_id,
                            &call.fn_name,
                            &args_str,
                        ));
                        final_messages.push(make_tool_call_result_message(
                            &current_task_id,
                            &request_id,
                            call.call_id.clone(),
                            result_content,
                        ));
                    }
                    Err(e) => {
                        // args 解析失败:跟 from_args 失败一样,emit error tool_result。
                        log::warn!(
                            "[byop] todowrite args parse failed: call_id={} err={e:#}",
                            call.call_id
                        );
                        let error_payload = tools::todowrite::invalid_arguments_result_to_json(
                            e.to_string(),
                            &args_str,
                        );
                        let error_content = serde_json::to_string(&error_payload)
                            .unwrap_or_else(|_| r#"{"error":"invalid_arguments"}"#.to_owned());
                        final_messages.push(make_tool_call_carrier_message(
                            &current_task_id,
                            &request_id,
                            &call.call_id,
                            &call.fn_name,
                            &args_str,
                        ));
                        final_messages.push(make_tool_call_result_message(
                            &current_task_id,
                            &request_id,
                            call.call_id.clone(),
                            error_content,
                        ));
                    }
                }
                continue;
            }

            // OpenWarp BYOP web 工具拦截:webfetch / websearch 不映射到 protobuf
            // executor variant,在这里直接跑本地 HTTP,合成 (carrier ToolCall,
            // ToolCallResult) 一对消息,绕开 parse_incoming_tool_call。
            //
            // UI:对齐 cloud 模式,前后各 emit 一条 `Message::WebSearch` /
            // `Message::WebFetch` 状态消息,触发 inline_action `WebSearchView` /
            // `WebFetchView` 渲染:Searching/Fetching loading 卡片 → Success(URL 列表)
            // / Error 折叠卡。这两条不进 final_messages,直接 yield 让 UI 实时更新;
            // carrier + result 仍走 final_messages 给下一轮模型推理用。
            if call.fn_name == tools::webfetch::TOOL_NAME
                || call.fn_name == tools::websearch::TOOL_NAME
            {
                let args_str = if call.fn_arguments.is_string() {
                    call.fn_arguments.as_str().unwrap_or("").to_owned()
                } else {
                    call.fn_arguments.to_string()
                };
                let is_search = call.fn_name == tools::websearch::TOOL_NAME;

                // 预解析 args 抽 query / url 给 UI loading 卡。args 解析失败也要 emit
                // (用空字段兜底),保证 UI 至少看到一帧 loading,后续 dispatch
                // 仍会返回 invalid_arguments → 切到 Error 卡。
                let preview_query = if is_search {
                    serde_json::from_str::<tools::web_runtime::SearchToolArgs>(&args_str)
                        .map(|a| a.query)
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                let preview_urls: Vec<String> = if !is_search {
                    serde_json::from_str::<tools::web_runtime::FetchArgs>(&args_str)
                        .map(|a| vec![a.url])
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

                // Searching/Fetching loading 帧与最终 Success/Error 帧必须共用同一个
                // message.id —— `block.rs::handle_web_search_messages` 按 id 复用
                // WebSearchView,id 不同会创建两张独立卡。
                let web_msg_id = Uuid::new_v4().to_string();
                let mut loading_msg = if is_search {
                    make_web_search_searching_message(
                        &current_task_id,
                        &request_id,
                        preview_query.clone(),
                    )
                } else {
                    make_web_fetch_fetching_message(
                        &current_task_id,
                        &request_id,
                        preview_urls.clone(),
                    )
                };
                loading_msg.id = web_msg_id.clone();
                yield Ok(make_add_messages_event(&current_task_id, vec![loading_msg]));

                let result_json = dispatch_byop_web_tool(&call.fn_name, &args_str).await;

                let mut done_msg = if is_search {
                    make_web_search_status_from_result(
                        &current_task_id,
                        &request_id,
                        &preview_query,
                        &result_json,
                    )
                } else {
                    make_web_fetch_status_from_result(
                        &current_task_id,
                        &request_id,
                        &preview_urls,
                        &result_json,
                    )
                };
                done_msg.id = web_msg_id;
                // 第二帧不能再用 AddMessagesToTask —— 那会往 task.messages 追加第二条
                // 同 id 记录,`output.rs::WebSearch` 渲染分支按 message 数量 add_child,
                // 显示成两张并排卡。改用 UpdateTaskMessage + FieldMask:`task::upsert_message`
                // 找到同 id 现有 message 后走 FieldMaskOperation::update 原地合并,
                // task.messages 仍只有一条 → UI 一张卡 set_status 切换。
                let mask_path = if is_search { "web_search" } else { "web_fetch" };
                yield Ok(make_update_message_event(
                    &current_task_id,
                    done_msg,
                    vec![mask_path.to_owned()],
                ));

                let result_content = serde_json::to_string(&result_json)
                    .unwrap_or_else(|_| r#"{"status":"serialize_error"}"#.to_owned());
                final_messages.push(make_tool_call_carrier_message(
                    &current_task_id,
                    &request_id,
                    &call.call_id,
                    &call.fn_name,
                    &args_str,
                ));
                final_messages.push(make_tool_call_result_message(
                    &current_task_id,
                    &request_id,
                    call.call_id.clone(),
                    result_content,
                ));
                continue;
            }

            match parse_incoming_tool_call(&call, mcp_context.as_ref()) {
                Ok(warp_tool) => {
                    // 如果 ToolCallChunk 阶段已经 emit 过占位卡(同 call_id),
                    // 改用 update_message 原地刷新为最终 args(覆盖 chunk 中可能后到
                    // 的 args delta)。占位与终帧共用同一 message.id,
                    // task::upsert_message 走 FieldMaskOperation::update,
                    // task.messages 仍只有一条 → UI 一张卡 in-place 刷新,不双卡。
                    if let Some(msg_id) = tool_msg_ids.get(&call.call_id).cloned() {
                        let mut updated = make_tool_call_message(
                            &current_task_id,
                            &request_id,
                            &call.call_id,
                            warp_tool,
                        );
                        updated.id = msg_id;
                        yield Ok(make_update_message_event(
                            &current_task_id,
                            updated,
                            vec!["tool_call".to_owned()],
                        ));
                    } else {
                        final_messages.push(make_tool_call_message(
                            &current_task_id,
                            &request_id,
                            &call.call_id,
                            warp_tool,
                        ));
                    }
                }
                Err(e) => {
                    // 关键:不再把 from_args 失败吞成纯文本(原实现:emit AgentOutput),
                    // 因为模型那一轮以为自己调了 tool 在等 result,看到一段中文 assistant 文字
                    // 完全不知道是参数类型错,无法定向修正重试。
                    // 改成 emit 一对 ToolCall(carrier) + ToolCallResult(error JSON),
                    // 让模型在下一轮看到标准 tool_result error,可以按惯例改 args 重试或换工具。
                    //
                    // ToolCall 的 `tool` oneof 留 None(没有合适的结构化 variant),原始
                    // fn_name + args_str 通过 server_message_data 携带,
                    // serialize_outgoing_tool_call 的 carrier 分支会优先还原。
                    let args_str = if call.fn_arguments.is_string() {
                        call.fn_arguments.as_str().unwrap_or("").to_owned()
                    } else {
                        call.fn_arguments.to_string()
                    };
                    log::warn!(
                        "[byop] tool_call parse failed → emit synthetic error tool_result: \
                         tool={} call_id={} err={e:#}",
                        call.fn_name,
                        call.call_id
                    );
                    let error_payload = serde_json::json!({
                        "error": "invalid_arguments",
                        "detail": e.to_string(),
                        "tool": call.fn_name,
                        "received_args": &args_str,
                        "hint": "Arguments did not match the tool's JSON Schema. \
                                 Re-emit the tool call with corrected types / required fields, \
                                 or pick a different tool.",
                    });
                    let error_content = serde_json::to_string(&error_payload)
                        .unwrap_or_else(|_| r#"{"error":"invalid_arguments"}"#.to_owned());
                    final_messages.push(make_tool_call_carrier_message(
                        &current_task_id,
                        &request_id,
                        &call.call_id,
                        &call.fn_name,
                        &args_str,
                    ));
                    final_messages.push(make_tool_call_result_message(
                        &current_task_id,
                        &request_id,
                        call.call_id.clone(),
                        error_content,
                    ));
                }
            }
        }
        if !final_messages.is_empty() {
            yield Ok(make_add_messages_event(&current_task_id, final_messages));
        }

        // 把 captured token usage 折算成 ConversationUsageMetadata.context_window_usage
        // 注入 StreamFinished — controller 的 handle_response_stream_finished 会把它写到
        // conversation.conversation_usage_metadata,footer 监听 UpdatedStreamingExchange/
        // AppendedExchange 事件即在每轮末实时刷新 "X% context remaining" 工具提示。
        let usage_metadata = context_window.and_then(|cw| {
            if cw == 0 || (captured_prompt_tokens == 0 && captured_completion_tokens == 0) {
                return None;
            }
            let used = (captured_prompt_tokens + captured_completion_tokens).max(0) as f32;
            let pct = (used / cw as f32).clamp(0.0, 1.0);
            log::info!(
                "[byop] context usage: prompt={} completion={} window={} → {:.1}%",
                captured_prompt_tokens,
                captured_completion_tokens,
                cw,
                pct * 100.0
            );
            Some(api::response_event::stream_finished::ConversationUsageMetadata {
                context_window_usage: pct,
                summarized: false,
                credits_spent: 0.0,
                #[allow(deprecated)]
                token_usage: Vec::new(),
                tool_usage_metadata: None,
                warp_token_usage: std::collections::HashMap::new(),
                byok_token_usage: std::collections::HashMap::new(),
            })
        });
        yield Ok(make_finished_done(usage_metadata));
    };

    Ok(Box::pin(stream))
}

/// 用独立 BYOP 配置发一个短的非工具请求,让模型对首条 user query 生成会话标题。
/// 所有错误吞掉(返回 Err 让上游打 warn log,不影响主流程)。
///
/// 实现委托给 `oneshot::byop_oneshot_streaming_completion`,这里只负责拼 prompt 和清洗输出。
///
/// ## prompt 设计
///
/// - **system**: 见 `prompts/tasks/title_system.md`,结构化 task/rules/examples,
///   覆盖中英双语示例,显式禁止 "回答用户问题 / 拒绝 / 加引号"。
/// - **user**: 把原始 `user_query` 包在 `<user>...</user>` 里,前置一句明确的
///   "Generate a title for this conversation:",避免弱模型把 user 当主指令直接答复
///   (典型坏 case:user="你是谁" → 模型答"我是 Claude"被当作标题)。
/// - **temperature**: 0.3 — opencode title agent 用 0.5,这里更保守,降低跑题。
pub(crate) async fn generate_title_via_byop(
    tg: &TitleGenInput,
    user_query: &str,
) -> Result<Option<String>, anyhow::Error> {
    let cfg = super::oneshot::OneshotConfig {
        base_url: tg.base_url.clone(),
        api_key: tg.api_key.clone(),
        model_id: tg.model_id.clone(),
        api_type: tg.api_type,
        reasoning_effort: tg.reasoning_effort,
    };
    let system = include_str!("prompts/tasks/title_system.md");
    let user_prompt = format!(
        "Generate a title for this conversation:\n<user>{}</user>",
        user_query
    );
    let opts = super::oneshot::OneshotOptions {
        max_chars: Some(1000),
        temperature: Some(0.5),
        ..Default::default()
    };
    let raw = super::oneshot::byop_oneshot_completion(&cfg, system, &user_prompt, &opts).await?;
    Ok(sanitize_title(&raw))
}

/// 清洗 title 文本。空字符串 → None(让上游跳过 emit)。
///
/// 处理顺序:
/// 1. 剥 `<think>...</think>` / `<reasoning>...</reasoning>` 思考块(reasoning 模型常见前缀)。
/// 2. 取首行非空内容(模型常前置"好的,标题是:"再换行给标题)。
/// 3. 剥 `Title:` / `标题:` / `Thread:` / `Subject:` 等前缀(大小写不敏感)。
/// 4. 剥首尾引号 / 反引号(中英文)。
/// 5. 去尾标点。
/// 6. 50 字符截断(按 char,保护 CJK),超过则尾部加 `…`。
fn sanitize_title(raw: &str) -> Option<String> {
    // 1. 剥 reasoning 标签(可能有多个,DOTALL 模式)。
    let mut s = raw.to_owned();
    for tag in &["think", "reasoning", "thought", "scratchpad"] {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);
        while let (Some(start), Some(end_rel)) =
            (s.find(&open), s.find(&close).map(|e| e + close.len()))
        {
            if end_rel <= start {
                break;
            }
            s.replace_range(start..end_rel, "");
        }
    }

    // 2. 取首行非空。
    let first_line = s
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_owned();
    let mut s = first_line;

    // 3. 剥前缀(循环剥,处理 "Title: 标题: foo" 这类双前缀)。
    let prefixes = [
        "title:",
        "subject:",
        "thread:",
        "标题:",
        "标题：",
        "主题:",
        "主题：",
    ];
    loop {
        let lower = s.to_lowercase();
        let mut stripped = false;
        for p in &prefixes {
            if lower.starts_with(p) {
                s = s[p.len()..].trim_start().to_owned();
                stripped = true;
                break;
            }
        }
        if !stripped {
            break;
        }
    }

    // 4. 剥首尾引号(中英文)。
    let quotes = ['"', '\'', '`', '“', '”', '‘', '’', '《', '》', '「', '」'];
    while let Some(c) = s.chars().next() {
        if quotes.contains(&c) {
            s.remove(0);
        } else {
            break;
        }
    }
    while let Some(c) = s.chars().last() {
        if quotes.contains(&c) {
            let new_len = s.len() - c.len_utf8();
            s.truncate(new_len);
        } else {
            break;
        }
    }

    // 5. 去尾标点。
    while let Some(c) = s.chars().last() {
        if matches!(
            c,
            '.' | '。' | '!' | '！' | '?' | '？' | ',' | '，' | ';' | '；' | ':' | '：'
        ) {
            let new_len = s.len() - c.len_utf8();
            s.truncate(new_len);
        } else {
            break;
        }
    }

    let s = s.trim().to_owned();
    if s.is_empty() {
        return None;
    }

    // 6. 50 字符截断(按 char,保护 CJK)。超长加省略号。
    const MAX_CHARS: usize = 50;
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > MAX_CHARS {
        let mut truncated: String = chars.iter().take(MAX_CHARS - 1).collect();
        truncated.push('…');
        Some(truncated)
    } else {
        Some(s)
    }
}

// ---------------------------------------------------------------------------
// Event 构造辅助
// ---------------------------------------------------------------------------

enum AppendKind {
    Reasoning(String),
    Text(String),
}

fn make_add_messages_event(task_id: &str, messages: Vec<api::Message>) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::AddMessagesToTask(
                        api::client_action::AddMessagesToTask {
                            task_id: task_id.to_owned(),
                            messages,
                        },
                    )),
                }],
            },
        )),
    }
}

/// 用 `UpdateTaskMessage` + FieldMask 替换已有 message 的部分字段。controller
/// `conversation::Action::UpdateTaskMessage` → `task::upsert_message` →
/// `FieldMaskOperation::update` 原地合并,id 已存在则不会 push 重复记录。
/// 用于 BYOP web 工具 loading → success/error 状态切换(见拦截分支)。
fn make_update_message_event(
    task_id: &str,
    message: api::Message,
    mask_paths: Vec<String>,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::UpdateTaskMessage(
                        api::client_action::UpdateTaskMessage {
                            task_id: task_id.to_owned(),
                            message: Some(message),
                            mask: Some(prost_types::FieldMask { paths: mask_paths }),
                        },
                    )),
                }],
            },
        )),
    }
}

fn make_append_event(task_id: &str, message_id: &str, kind: AppendKind) -> api::ResponseEvent {
    let (msg_inner, mask_path) = match kind {
        AppendKind::Reasoning(r) => (
            api::message::Message::AgentReasoning(api::message::AgentReasoning {
                reasoning: r,
                finished_duration: None,
            }),
            "agent_reasoning.reasoning",
        ),
        AppendKind::Text(t) => (
            api::message::Message::AgentOutput(api::message::AgentOutput { text: t }),
            "agent_output.text",
        ),
    };
    let message = api::Message {
        id: message_id.to_owned(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(msg_inner),
        request_id: String::new(),
        timestamp: None,
    };
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::AppendToMessageContent(
                        api::client_action::AppendToMessageContent {
                            task_id: task_id.to_owned(),
                            message: Some(message),
                            mask: Some(prost_types::FieldMask {
                                paths: vec![mask_path.to_owned()],
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

/// BYOP web 工具(`webfetch` / `websearch`)的本地分发器。
///
/// 不通过 protobuf executor —— 直接在本地用 reqwest 跑 HTTP,把结构化结果
/// 序列化成 JSON Value 给上游 LLM。错误也序列化成 `{status:"error", ...}`,
/// 让模型看到标准 tool_result。
async fn dispatch_byop_web_tool(tool_name: &str, args_str: &str) -> Value {
    use tools::web_runtime;
    // 为 webfetch 构建带 SSRF 防护的 client：自定义重定向策略会校验每一跳目标。
    let client = match web_runtime::build_ssrf_safe_client() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[byop] reqwest client build failed: {e:#}");
            return web_runtime::error_to_json(tool_name, &anyhow::anyhow!(e.to_string()));
        }
    };
    if tool_name == tools::webfetch::TOOL_NAME {
        match serde_json::from_str::<web_runtime::FetchArgs>(args_str) {
            Ok(args) => match web_runtime::run_webfetch(&client, args).await {
                Ok(out) => web_runtime::fetch_output_to_json(&out),
                Err(e) => {
                    log::warn!("[byop][webfetch] error: {e:#}");
                    web_runtime::error_to_json(tool_name, &e)
                }
            },
            Err(e) => web_runtime::error_to_json(
                tool_name,
                &anyhow::anyhow!(format!("invalid arguments: {e}")),
            ),
        }
    } else {
        // websearch
        match serde_json::from_str::<web_runtime::SearchToolArgs>(args_str) {
            Ok(args) => {
                let api_key = std::env::var("EXA_API_KEY").ok();
                match web_runtime::run_websearch(&client, args, api_key.as_deref(), None).await {
                    Ok(out) => web_runtime::search_output_to_json(&out),
                    Err(e) => {
                        log::warn!("[byop][websearch] error: {e:#}");
                        web_runtime::error_to_json(tool_name, &e)
                    }
                }
            }
            Err(e) => web_runtime::error_to_json(
                tool_name,
                &anyhow::anyhow!(format!("invalid arguments: {e}")),
            ),
        }
    }
}

fn parse_incoming_tool_call(
    call: &ToolCall,
    mcp_ctx: Option<&crate::ai::agent::MCPContext>,
) -> anyhow::Result<api::message::tool_call::Tool> {
    // genai ToolCall.fn_arguments 是 Value;tools::* 的 from_args 期望 &str,
    // 把 Value 序列化回字符串后传入(原协议就是字符串 JSON)。
    let args_str = if call.fn_arguments.is_string() {
        call.fn_arguments.as_str().unwrap_or("").to_owned()
    } else {
        call.fn_arguments.to_string()
    };
    if tools::mcp::is_mcp_function(&call.fn_name) {
        return tools::mcp::parse_mcp_tool_call(&call.fn_name, &args_str, mcp_ctx);
    }
    let Some(tool) = tools::lookup(&call.fn_name) else {
        anyhow::bail!("unknown tool name: {}", call.fn_name);
    };
    match (tool.from_args)(&args_str) {
        Ok(t) => Ok(t),
        Err(e) => {
            // 第一次失败:大概率是模型把 bool/数字/数组 序列化成了字符串。
            // 拿工具自身的 schema 跑一次类型 coerce,再 retry。
            let schema = (tool.parameters)();
            if let Some(coerced) = tools::coerce::coerce_args_against_schema(&args_str, &schema) {
                match (tool.from_args)(&coerced) {
                    Ok(t) => {
                        log::info!(
                            "[byop] from_args coerced ok: tool={} original_err={e:#}",
                            call.fn_name
                        );
                        return Ok(t);
                    }
                    Err(e2) => {
                        log::warn!(
                            "[byop] from_args failed (after coerce): tool={} err={e2:#} original_err={e:#} coerced_args={coerced} args_str={args_str}",
                            call.fn_name
                        );
                        return Err(e2);
                    }
                }
            }
            // 诊断:解析失败时把 from_args 实际拿到的字符串原样打出来,
            // 配合上层 [byop] tool_call_in 的 args= 行可以判断:
            //   1. 是否模型出参类型错(bool→"true" / 数字→"1" 等)
            //   2. 是否 genai Value→string 转换中 escape 出问题
            //   3. 是否 fn_arguments 整段被字符串化(应该 object 却是 string)
            log::warn!(
                "[byop] from_args failed: tool={} err={e:#} args_str={args_str}",
                call.fn_name
            );
            Err(e)
        }
    }
}

fn make_reasoning_message(task_id: &str, request_id: &str, reasoning: String) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentReasoning(
            api::message::AgentReasoning {
                reasoning,
                finished_duration: None,
            },
        )),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

fn make_agent_output_message(task_id: &str, request_id: &str, text: String) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput { text },
        )),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

fn make_user_query_message(
    task_id: &str,
    request_id: &str,
    query: String,
    binaries: &[user_context::UserBinary],
) -> api::Message {
    // OpenWarp:把 multimodal binary(image / pdf / audio 等)写进 `UserQuery.context.images`
    // (InputContext 字段,proto Image 实际是 `bytes data + string mime_type` 通用容器,
    // 字段名叫 images 历史原因)。UserBinary.data 是 base64 字符串,proto.data 是 raw bytes,
    // 这里 decode 一次;decode 失败的条目跳过,不阻塞模型流(decode 失败本来就意味着这条
    // 当轮也没真送上游,丢就丢了,不影响 history 一致性)。
    let proto_binaries: Vec<api::input_context::Image> = binaries
        .iter()
        .filter_map(|b| {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(&b.data)
                .ok()
                .map(|bytes| api::input_context::Image {
                    data: bytes,
                    mime_type: b.content_type.clone(),
                })
        })
        .collect();
    let context = if proto_binaries.is_empty() {
        None
    } else {
        Some(api::InputContext {
            images: proto_binaries,
            ..Default::default()
        })
    };
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::UserQuery(api::message::UserQuery {
            query,
            context,
            ..Default::default()
        })),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

/// BYOP 拦截 websearch 时,emit `Message::WebSearch(Searching{query})`,UI 据此渲染
/// "Searching the web for \"query\"" loading 卡(`inline_action::web_search`)。
fn make_web_search_searching_message(
    task_id: &str,
    request_id: &str,
    query: String,
) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::WebSearch(api::message::WebSearch {
            status: Some(api::message::web_search::Status {
                r#type: Some(api::message::web_search::status::Type::Searching(
                    api::message::web_search::status::Searching { query },
                )),
            }),
        })),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

/// 从 exa MCP 返回的 results 字符串里抽 (url, title)。
///
/// 实际格式是行式 metadata block,以 `---` 分隔多条结果:
/// ```
/// Title: Announcing Rust 1.95.0 | Rust Blog
/// URL: https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/
/// Published: 2026-04-16T00:00:00.000Z
/// Author: N/A
/// Highlights:
/// ...
/// ---
/// Title: ...
/// ```
/// 扫到 `Title: X` 缓存 candidate,紧随的第一条 `URL: Y` 配对成 (Y, X) 入列,去重。
/// 兼容兜底:也扫 `[title](url)` markdown link 形式(若 exa 模板未来切换)。
fn extract_search_pages_from_exa_results(s: &str) -> Vec<(String, String)> {
    let mut pages = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 路线 1:Title:/URL: 行式
    let mut current_title: Option<String> = None;
    for line in s.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("Title:") {
            current_title = Some(rest.trim().to_owned());
        } else if let Some(rest) = trimmed.strip_prefix("URL:") {
            let url = rest.trim().to_owned();
            let title = current_title.take().unwrap_or_default();
            if (url.starts_with("http://") || url.starts_with("https://"))
                && seen.insert(url.clone())
            {
                pages.push((url, title));
            }
        }
    }

    // 路线 2:markdown link `[title](url)` 兜底(去重已生效,不会重复)
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(rel_close_text) = s[i + 1..].find("](") {
                let text_end = i + 1 + rel_close_text;
                let url_start = text_end + 2;
                if let Some(rel_close_url) = s[url_start..].find(')') {
                    let url_end = url_start + rel_close_url;
                    let title = s[i + 1..text_end].trim().to_owned();
                    let url = s[url_start..url_end].trim().to_owned();
                    if (url.starts_with("http://") || url.starts_with("https://"))
                        && seen.insert(url.clone())
                    {
                        pages.push((url, title));
                    }
                    i = url_end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    pages
}

/// BYOP websearch 完成后,根据 `result_json` 决定 Success / Error 状态。
///
/// `pages` 从 `result_json["results"]` 这段 exa 拼好的 markdown 里扫 `[title](url)` 抽。
fn make_web_search_status_from_result(
    task_id: &str,
    request_id: &str,
    query: &str,
    result_json: &Value,
) -> api::Message {
    let is_error = result_json.get("status").and_then(|v| v.as_str()) == Some("error");
    let r#type = if is_error {
        api::message::web_search::status::Type::Error(())
    } else {
        let pages = result_json
            .get("results")
            .and_then(|v| v.as_str())
            .map(extract_search_pages_from_exa_results)
            .unwrap_or_default()
            .into_iter()
            .map(
                |(url, title)| api::message::web_search::status::success::SearchedPage {
                    url,
                    title,
                },
            )
            .collect();
        api::message::web_search::status::Type::Success(api::message::web_search::status::Success {
            query: query.to_owned(),
            pages,
        })
    };
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::WebSearch(api::message::WebSearch {
            status: Some(api::message::web_search::Status {
                r#type: Some(r#type),
            }),
        })),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

/// BYOP 拦截 webfetch 时,emit `Message::WebFetch(Fetching{urls})`,UI 据此渲染
/// "Fetching N URLs" loading 卡(`inline_action::web_fetch`)。
fn make_web_fetch_fetching_message(
    task_id: &str,
    request_id: &str,
    urls: Vec<String>,
) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::WebFetch(api::message::WebFetch {
            status: Some(api::message::web_fetch::Status {
                r#type: Some(api::message::web_fetch::status::Type::Fetching(
                    api::message::web_fetch::status::Fetching { urls },
                )),
            }),
        })),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

/// BYOP webfetch 完成后,从 `FetchOutput` JSON 抽 `url` + HTTP `status` 组装 Success
/// 卡;status="error" 走 Error 卡。
fn make_web_fetch_status_from_result(
    task_id: &str,
    request_id: &str,
    fallback_urls: &[String],
    result_json: &Value,
) -> api::Message {
    let is_error = result_json.get("status").and_then(|v| v.as_str()) == Some("error");
    let r#type = if is_error {
        api::message::web_fetch::status::Type::Error(())
    } else {
        let url = result_json
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .unwrap_or_else(|| fallback_urls.first().cloned().unwrap_or_default());
        // FetchOutput.status 是 HTTP 状态码,2xx 算 success。
        let success = result_json
            .get("status")
            .and_then(|v| v.as_u64())
            .map(|c| (200..300).contains(&c))
            .unwrap_or(true);
        api::message::web_fetch::status::Type::Success(api::message::web_fetch::status::Success {
            pages: vec![api::message::web_fetch::status::success::FetchedPage {
                url,
                title: String::new(),
                success,
            }],
        })
    };
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::WebFetch(api::message::WebFetch {
            status: Some(api::message::web_fetch::Status {
                r#type: Some(r#type),
            }),
        })),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

fn make_tool_call_result_message(
    task_id: &str,
    request_id: &str,
    tool_call_id: String,
    content: String,
) -> api::Message {
    // ToolCallResult 持久化:warp protobuf 的 `tool_call_result.result` oneof 都是
    // 结构化 variant(RunShellCommand / Grep / ReadFiles / ...),没有通用的字符串
    // 兜底 variant。BYOP 已经在 chat_stream 自己把 result 序列化为 JSON 字符串,
    // 不再需要按 warp 协议结构化 — 直接把字符串存到 `server_message_data` 这个
    // 自由字符串字段,并把 `result` oneof 留 None。下一轮 build_chat_request 在
    // `Message::ToolCallResult` 分支需要特判:result=None 时从 server_message_data
    // 读 content(否则走 tools::serialize_result 反序列化结构化 variant)。
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: content,
        citations: vec![],
        message: Some(api::message::Message::ToolCallResult(
            api::message::ToolCallResult {
                tool_call_id,
                context: None,
                result: None,
            },
        )),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

/// BYOP `from_args` 解析失败时,emit 占位 ToolCall 作 carrier:
/// `tool` oneof 留 None(没有合适的结构化 variant),原始 fn_name + args_str 编码到
/// `server_message_data` 为 `<fn_name>\n<args_str>`。下一轮 build_chat_request →
/// `serialize_outgoing_tool_call` 的 carrier 分支据此还原,保证上游模型看到的
/// tool_use name / args 与原 call 一致(否则用 "warp_internal_empty" 占位会让模型
/// 困惑,也对不上紧随的 ToolCallResult error 上下文)。
fn make_tool_call_carrier_message(
    task_id: &str,
    request_id: &str,
    tool_call_id: &str,
    fn_name: &str,
    args_str: &str,
) -> api::Message {
    let carrier = format!("{}\n{}", fn_name, args_str);
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: carrier,
        citations: vec![],
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call_id.to_owned(),
            tool: None,
        })),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

fn make_tool_call_message(
    task_id: &str,
    request_id: &str,
    tool_call_id: &str,
    tool: api::message::tool_call::Tool,
) -> api::Message {
    api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call_id.to_owned(),
            tool: Some(tool),
        })),
        request_id: request_id.to_owned(),
        timestamp: None,
    }
}

fn create_task_event(task_id: &str) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::CreateTask(
                        api::client_action::CreateTask {
                            task: Some(api::Task {
                                id: task_id.to_owned(),
                                description: String::new(),
                                dependencies: None,
                                messages: vec![],
                                summary: String::new(),
                                server_data: String::new(),
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

/// 构造一条 `Action::CreateTask` 表示新 subtask,带 `dependencies.parent_task_id`。
/// conversation 在 `apply_client_action::CreateTask` 看到 `task.parent_id()` 非空 →
/// 走 `Task::new_subtask` 路径,从 parent.messages 找匹配的 subagent tool_call、
/// 抽 `SubagentParams` 挂到 subtask、emit `BlocklistAIHistoryEvent::CreatedSubtask`。
/// LRC tag-in 浮窗 spawn 链路依赖此事件。
fn create_subtask_event(subtask_id: &str, parent_task_id: &str) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::CreateTask(
                        api::client_action::CreateTask {
                            task: Some(api::Task {
                                id: subtask_id.to_owned(),
                                description: String::new(),
                                dependencies: Some(api::task::Dependencies {
                                    parent_task_id: parent_task_id.to_owned(),
                                }),
                                messages: vec![],
                                summary: String::new(),
                                server_data: String::new(),
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

fn make_finished_done(
    usage_metadata: Option<api::response_event::stream_finished::ConversationUsageMetadata>,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                reason: Some(api::response_event::stream_finished::Reason::Done(
                    api::response_event::stream_finished::Done {},
                )),
                conversation_usage_metadata: usage_metadata,
                token_usage: vec![],
                should_refresh_model_config: false,
                request_cost: None,
            },
        )),
    }
}

#[cfg(test)]
mod assistant_buffer_tests {
    use super::*;
    use genai::chat::{ChatRole, ToolCall};

    fn reasoning_part(msg: &ChatMessage) -> Option<&str> {
        for p in msg.content.parts() {
            if let ContentPart::ReasoningContent(r) = p {
                return Some(r.as_str());
            }
        }
        None
    }

    /// gate=false + 真实 reasoning → **丢弃**(zerx-lab/warp #25 修复点)。
    /// Cerebras / Groq / OpenRouter 等 OpenAI-strict provider 见到字段就 400。
    #[test]
    fn no_echo_drops_real_reasoning_text() {
        let mut buf = AssistantBuffer::new(false);
        buf.text = Some("Hi".to_string());
        buf.reasoning = Some("internal thought".to_string());
        let mut msgs = Vec::new();
        buf.flush_into(&mut msgs);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, ChatRole::Assistant);
        assert!(
            reasoning_part(&msgs[0]).is_none(),
            "must not echo reasoning"
        );
    }

    /// gate=false + tool_calls + 真实 reasoning → tool_calls 这条也不挂 reasoning。
    #[test]
    fn no_echo_drops_reasoning_on_tool_calls_message() {
        let mut buf = AssistantBuffer::new(false);
        buf.text = Some("calling".to_string());
        buf.tool_calls = vec![ToolCall {
            call_id: "c1".to_string(),
            fn_name: "echo".to_string(),
            fn_arguments: serde_json::json!({}),
            thought_signatures: None,
        }];
        buf.reasoning = Some("planning".to_string());
        let mut msgs = Vec::new();
        buf.flush_into(&mut msgs);
        assert_eq!(msgs.len(), 2, "text + tool_calls flush 成两条");
        for m in &msgs {
            assert!(
                reasoning_part(m).is_none(),
                "any-msg reasoning must be absent"
            );
        }
    }

    /// gate=true + 真实 reasoning → 挂真实值(DeepSeek / Kimi 路径)。
    #[test]
    fn echo_keeps_real_reasoning() {
        let mut buf = AssistantBuffer::new(true);
        buf.text = Some("ok".to_string());
        buf.reasoning = Some("thinking...".to_string());
        let mut msgs = Vec::new();
        buf.flush_into(&mut msgs);
        assert_eq!(msgs.len(), 1);
        assert_eq!(reasoning_part(&msgs[0]), Some("thinking..."));
    }

    /// gate=true + 无 reasoning → 挂占位符(满足"字段必须存在"校验)。
    #[test]
    fn echo_inserts_placeholder_when_empty() {
        let mut buf = AssistantBuffer::new(true);
        buf.text = Some("ok".to_string());
        buf.reasoning = None;
        let mut msgs = Vec::new();
        buf.flush_into(&mut msgs);
        assert_eq!(msgs.len(), 1);
        assert_eq!(reasoning_part(&msgs[0]), Some(REASONING_ECHO_PLACEHOLDER));
    }

    /// gate=true + tool_calls + 真实 reasoning → text 这条占位,tool_calls 那条挂真实值。
    #[test]
    fn echo_with_tool_calls_splits_correctly() {
        let mut buf = AssistantBuffer::new(true);
        buf.text = Some("calling".to_string());
        buf.tool_calls = vec![ToolCall {
            call_id: "c1".to_string(),
            fn_name: "echo".to_string(),
            fn_arguments: serde_json::json!({}),
            thought_signatures: None,
        }];
        buf.reasoning = Some("plan".to_string());
        let mut msgs = Vec::new();
        buf.flush_into(&mut msgs);
        assert_eq!(msgs.len(), 2);
        // text 这条:占位
        assert_eq!(reasoning_part(&msgs[0]), Some(REASONING_ECHO_PLACEHOLDER));
        // tool_calls 这条:真实 reasoning + 含 ToolCall part
        assert_eq!(reasoning_part(&msgs[1]), Some("plan"));
        assert!(
            !msgs[1].content.tool_calls().is_empty(),
            "second message must carry tool_calls"
        );
    }
}

#[cfg(test)]
mod dashscope_thinking_tests {
    use super::*;
    use crate::settings::ReasoningEffortSetting as R;

    const DASHSCOPE_CN: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/";
    const DASHSCOPE_INTL: &str = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/";

    #[test]
    fn dashscope_qwen3_triggers() {
        assert!(dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            DASHSCOPE_CN,
            "qwen3-235b-a22b",
            R::High
        ));
    }

    #[test]
    fn dashscope_qwq_triggers() {
        assert!(dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            DASHSCOPE_INTL,
            "qwq-32b",
            R::Medium
        ));
    }

    #[test]
    fn dashscope_deepseek_r1_triggers() {
        assert!(dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            DASHSCOPE_CN,
            "deepseek-r1",
            R::High
        ));
    }

    #[test]
    fn dashscope_kimi_k2_thinking_excluded() {
        // opencode 注释:kimi-k2-thinking 默认就开,不重复注入
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            DASHSCOPE_CN,
            "kimi-k2-thinking",
            R::High
        ));
    }

    #[test]
    fn dashscope_off_setting_skips() {
        // 用户主动关思考时尊重之
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            DASHSCOPE_CN,
            "qwen3-30b",
            R::Off
        ));
    }

    #[test]
    fn dashscope_non_reasoning_model_skips() {
        // qwen-turbo / qwen2.5 等纯 chat 模型不该被注入
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            DASHSCOPE_CN,
            "qwen-turbo",
            R::High
        ));
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            DASHSCOPE_CN,
            "qwen2.5-72b",
            R::High
        ));
    }

    #[test]
    fn non_dashscope_url_skips() {
        // OpenAI / Cerebras / Groq 等不是 DashScope 的 base_url
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            "https://api.openai.com/v1/",
            "qwen3-30b",
            R::High
        ));
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::OpenAi,
            "https://api.cerebras.ai/v1/",
            "qwen3-30b",
            R::High
        ));
    }

    #[test]
    fn non_openai_api_type_skips() {
        // Anthropic / Gemini / DeepSeek api_type 不走这条路径
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::Anthropic,
            DASHSCOPE_CN,
            "qwen3-30b",
            R::High
        ));
        assert!(!dashscope_needs_enable_thinking(
            AgentProviderApiType::DeepSeek,
            DASHSCOPE_CN,
            "deepseek-r1",
            R::High
        ));
    }
}

/// `build_chat_options` 中"思考深度档位下发"的回归测试。
///
/// 对齐 Zed `LanguageModelRequest::thinking_allowed=false` 在各 provider 的处理:
/// **Off 时所有 provider 都不能让服务端思考**。具体策略按 provider 不同:
/// - Anthropic / Gemini:不发 thinking 字段(跳过 `with_reasoning_effort`)
/// - DeepSeek:`extra_body.thinking.type=disabled`(服务端默认开启,需显式关)
/// - OpenAI / OpenAiResp:`reasoning_effort: "none"`(GPT-5 接受)
#[cfg(test)]
mod build_chat_options_off_tests {
    use super::*;
    use crate::settings::ReasoningEffortSetting as R;
    use genai::chat::ReasoningEffort as GE;

    fn opts(api_type: AgentProviderApiType, model: &str, effort: R) -> genai::chat::ChatOptions {
        build_chat_options(
            api_type,
            "https://example.com/v1/",
            model,
            effort,
            vec![],
            None,
        )
    }

    /// claude-sonnet-4-6(`SUPPORT_ADAPTTIVE_THINK_MODELS` 命中)+ Off 必须**完全
    /// 不传** `reasoning_effort`,否则 vendor genai adapter 会无条件插入
    /// `thinking:{type:adaptive}`(`adapter_impl.rs:121-135`)。
    #[test]
    fn anthropic_sonnet_4_6_off_skips_reasoning_effort() {
        let o = opts(AgentProviderApiType::Anthropic, "claude-sonnet-4-6", R::Off);
        assert!(
            o.reasoning_effort.is_none(),
            "Anthropic+Off 必须不传 reasoning_effort,避免 4.6 系强插 adaptive thinking"
        );
        assert!(
            o.extra_body.is_none(),
            "Anthropic+Off 也不应注入 extra_body"
        );
    }

    /// claude-opus-4-6 同上(双重命中 SUPPORT_EFFORT + SUPPORT_ADAPTIVE)。
    #[test]
    fn anthropic_opus_4_6_off_skips_reasoning_effort() {
        let o = opts(AgentProviderApiType::Anthropic, "claude-opus-4-6", R::Off);
        assert!(o.reasoning_effort.is_none());
        assert!(o.extra_body.is_none());
    }

    /// claude-opus-4-7+ + Off:虽然不在 adaptive 名单(本来就 OK),仍应一致跳过。
    #[test]
    fn anthropic_opus_4_7_off_skips_reasoning_effort() {
        let o = opts(AgentProviderApiType::Anthropic, "claude-opus-4-7", R::Off);
        assert!(o.reasoning_effort.is_none());
        assert!(o.extra_body.is_none());
    }

    /// Anthropic + High 仍走原 reasoning_effort 路径。
    #[test]
    fn anthropic_high_injects_reasoning_effort() {
        let o = opts(AgentProviderApiType::Anthropic, "claude-opus-4-7", R::High);
        assert!(matches!(o.reasoning_effort, Some(GE::High)));
    }

    /// Anthropic + Auto 不传任何参数。
    #[test]
    fn anthropic_auto_skips() {
        let o = opts(AgentProviderApiType::Anthropic, "claude-opus-4-7", R::Auto);
        assert!(o.reasoning_effort.is_none());
    }

    /// Gemini + Off:不发 thinkingConfig。
    #[test]
    fn gemini_off_skips_reasoning_effort() {
        let o = opts(AgentProviderApiType::Gemini, "gemini-2.5-pro", R::Off);
        assert!(o.reasoning_effort.is_none());
        assert!(o.extra_body.is_none());
    }

    /// Gemini + Medium 走 thinkingBudget 路径。
    #[test]
    fn gemini_medium_injects_reasoning_effort() {
        let o = opts(AgentProviderApiType::Gemini, "gemini-2.5-pro", R::Medium);
        assert!(matches!(o.reasoning_effort, Some(GE::Medium)));
    }

    /// DeepSeek + Off:必须发 `extra_body.thinking.type=disabled`,
    /// 而**不能**走 reasoning_effort=none(服务端 400 unknown variant)。
    #[test]
    fn deepseek_off_uses_extra_body_disabled() {
        let o = opts(AgentProviderApiType::DeepSeek, "deepseek-v4-flash", R::Off);
        assert!(
            o.reasoning_effort.is_none(),
            "DeepSeek+Off 不能走 reasoning_effort=none"
        );
        let body = o.extra_body.as_ref().expect("extra_body must be set");
        assert_eq!(
            body.pointer("/thinking/type"),
            Some(&serde_json::Value::String("disabled".to_string())),
            "DeepSeek+Off 必须发 thinking.type=disabled"
        );
    }

    /// DeepSeek + High 走 reasoning_effort 顶层字段。
    #[test]
    fn deepseek_high_injects_reasoning_effort() {
        let o = opts(AgentProviderApiType::DeepSeek, "deepseek-reasoner", R::High);
        assert!(matches!(o.reasoning_effort, Some(GE::High)));
        assert!(o.extra_body.is_none());
    }

    /// OpenAI(GPT-5)+ Off:走 reasoning_effort=none(GPT-5 接受 `none` 档)。
    #[test]
    fn openai_gpt5_off_uses_reasoning_effort_none() {
        let o = opts(AgentProviderApiType::OpenAi, "gpt-5", R::Off);
        assert!(
            matches!(o.reasoning_effort, Some(GE::None)),
            "OpenAI+GPT-5+Off 应发 reasoning_effort=none"
        );
    }

    /// 不支持 reasoning 的模型 + 任意非 Auto 档位:跳过(避免上游 400)。
    #[test]
    fn anthropic_haiku_3_5_off_skips() {
        let o = opts(
            AgentProviderApiType::Anthropic,
            "claude-3-5-haiku-20241022",
            R::Off,
        );
        assert!(o.reasoning_effort.is_none());
        assert!(o.extra_body.is_none());
    }

    #[test]
    fn openai_gpt4o_off_skips() {
        // gpt-4o 不在 reasoning 名单,Off 也跳过
        let o = opts(AgentProviderApiType::OpenAi, "gpt-4o", R::Off);
        assert!(o.reasoning_effort.is_none());
    }
}

/// `openai_supports_extended_cache` 的单元测试。
///
/// 官方 2026-05 点名支持 24h Extended Cache 的型号:GPT-5 系列 / GPT-5.x /
/// GPT-5-codex / GPT-4.1 / o-series。其他一律走 in_memory 低风险默认。
#[cfg(test)]
mod openai_extended_cache_tests {
    use super::*;

    #[test]
    fn gpt5_family_supports_24h() {
        assert!(openai_supports_extended_cache("gpt-5"));
        assert!(openai_supports_extended_cache("gpt-5-mini"));
        assert!(openai_supports_extended_cache("gpt-5-codex"));
        assert!(openai_supports_extended_cache("gpt-5.5"));
        assert!(openai_supports_extended_cache("gpt-5.5-pro"));
    }

    #[test]
    fn gpt41_family_supports_24h() {
        assert!(openai_supports_extended_cache("gpt-4.1"));
        assert!(openai_supports_extended_cache("gpt-4.1-mini"));
        assert!(openai_supports_extended_cache("gpt-4.1-nano"));
    }

    #[test]
    fn o_series_supports_24h() {
        assert!(openai_supports_extended_cache("o3"));
        assert!(openai_supports_extended_cache("o3-mini"));
        assert!(openai_supports_extended_cache("o4-mini"));
        assert!(openai_supports_extended_cache("o1-preview"));
    }

    #[test]
    fn legacy_models_default_in_memory() {
        assert!(!openai_supports_extended_cache("gpt-4o"));
        assert!(!openai_supports_extended_cache("gpt-4o-mini"));
        assert!(!openai_supports_extended_cache("gpt-4-turbo"));
        assert!(!openai_supports_extended_cache("gpt-3.5-turbo"));
    }

    #[test]
    fn case_insensitive() {
        assert!(openai_supports_extended_cache("GPT-5"));
        assert!(openai_supports_extended_cache("GPT-4.1-Mini"));
    }

    /// OpenRouter 等代理会把型号写成 "openai/gpt-5";`/<prefix>` 包含判定仅
    /// 在路径路由型型号上生效。
    #[test]
    fn openrouter_style_path_matches() {
        assert!(openai_supports_extended_cache("openai/gpt-5"));
        assert!(openai_supports_extended_cache("openai/gpt-4.1-mini"));
        assert!(openai_supports_extended_cache("vendor/o3-mini"));
    }

    /// 未识别 / 本地服务 → 不走 24h(低风险默认)。
    #[test]
    fn unknown_models_default_false() {
        assert!(!openai_supports_extended_cache("qwen-max"));
        assert!(!openai_supports_extended_cache("deepseek-chat"));
        assert!(!openai_supports_extended_cache("llama-3.1-70b"));
        assert!(!openai_supports_extended_cache(""));
    }
}

/// **端到端 cache 边界稳定性测试**:验证多轮对话模拟下,prompt cache
/// 需要的“前缀字节级一致”保证。这些测试并不调用上游 API,仅检查
/// `apply_caching_anthropic` 与 `build_chat_options` 输出的确定性。
///
/// 这是 cache 命中的**最低门槛**:如果同样输入跨调用输出不一致,
/// 上游哈希必不一致 → 100% miss。反之输出一致也不能保证命中。
#[cfg(test)]
mod cache_boundary_stability_tests {
    use super::*;
    use genai::chat::{ChatMessage, ChatRole};

    /// 构造一个典型的多轮对话 messages 序列:
    /// system + user_1 + assistant_1 + user_2 + assistant_2 + user_3
    /// (末尾是 user,与 `ensure_ends_with_user` 输出一致)。
    fn build_three_turn_conversation() -> Vec<ChatMessage> {
        vec![
            ChatMessage::system(
                "You are a helpful coding assistant for OpenWarp BYOP.\n\
                 Guidelines: be concise, prefer code over prose.",
            ),
            ChatMessage::user("What is rust borrow checker?"),
            ChatMessage::assistant("It enforces ownership rules at compile time."),
            ChatMessage::user("Show me a code example"),
            ChatMessage::assistant("```rust\nfn main() { let s = String::new(); }\n```"),
            ChatMessage::user("Explain the lifetime in that code"),
        ]
    }

    fn extract_cache_control(msg: &ChatMessage) -> Option<CacheControl> {
        // ChatMessage 的 cache_control 在 `options.cache_control` 上。
        msg.options.as_ref().and_then(|o| o.cache_control.clone())
    }

    fn cache_signature(msgs: &[ChatMessage]) -> Vec<(usize, ChatRole, Option<CacheControl>)> {
        msgs.iter()
            .enumerate()
            .map(|(i, m)| (i, m.role.clone(), extract_cache_control(m)))
            .collect()
    }

    /// **P0-4 主要验收**:apply_caching_anthropic 在同一输入上重复调用
    /// 产出的 cache 标记位置与 TTL 必须 byte-equal。
    #[test]
    fn apply_caching_anthropic_is_deterministic() {
        let mut a = build_three_turn_conversation();
        let mut b = build_three_turn_conversation();
        apply_caching_anthropic(&mut a);
        apply_caching_anthropic(&mut b);
        assert_eq!(
            cache_signature(&a),
            cache_signature(&b),
            "同输入 × 多次调用 cache 标记必须一致"
        );
    }

    /// **P0-4 TTL 验收**:全部走 1h(Ephemeral1h)而非旧版的 5m(Ephemeral)。
    #[test]
    fn anthropic_cache_uses_1h_ttl() {
        let mut msgs = build_three_turn_conversation();
        apply_caching_anthropic(&mut msgs);
        let tagged: Vec<_> = msgs.iter().filter_map(extract_cache_control).collect();
        assert!(!tagged.is_empty(), "必须至少打一个 breakpoint");
        for cc in &tagged {
            assert!(
                matches!(cc, CacheControl::Ephemeral1h),
                "P0-4 要求全部使用 1h TTL,实际={:?}",
                cc
            );
        }
    }

    /// **P0-4 覆盖面验收**:opencode 路子 first 2 system + last 2 non-system。
    /// 多轮对话(1 个 system + 5 个 non-system)应该打上 1+2=3 个标记。
    #[test]
    fn anthropic_marks_first_2_system_and_last_2_non_system() {
        let mut msgs = build_three_turn_conversation();
        apply_caching_anthropic(&mut msgs);
        let tagged_indices: Vec<usize> = msgs
            .iter()
            .enumerate()
            .filter(|(_, m)| extract_cache_control(m).is_some())
            .map(|(i, _)| i)
            .collect();
        // 验证 system(idx=0) 与末尾 2 个 non-system(idx=4, idx=5)都被打上。
        assert!(tagged_indices.contains(&0), "首 system 未被标记");
        assert!(tagged_indices.contains(&4), "倒数第 2 条未被标记");
        assert!(tagged_indices.contains(&5), "末条未被标记");
        assert_eq!(
            tagged_indices.len(),
            3,
            "总计 3 个 breakpoint(1 system + 2 tail)"
        );
    }

    /// **模拟多轮对话中的缓存 prefix 稳定性**:
    /// turn N 的 messages 是 turn N-1 的 messages + (N-1 轮 assistant) + (新 user)。
    /// 起始部分的 cache 标记(system + 中间轮)不应随轮数增长而漂移。
    #[test]
    fn cache_marks_stable_as_conversation_grows() {
        // turn 1
        let mut t1 = vec![ChatMessage::system("sys"), ChatMessage::user("q1")];
        apply_caching_anthropic(&mut t1);
        let sys_t1_cc = extract_cache_control(&t1[0]);

        // turn 2:增加 assistant_1 + user_2
        let mut t2 = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q1"),
            ChatMessage::assistant("a1"),
            ChatMessage::user("q2"),
        ];
        apply_caching_anthropic(&mut t2);
        let sys_t2_cc = extract_cache_control(&t2[0]);

        // 首 system 的 cache_control 跨轮一致 → 表示上游哈希不变 → 后续会命中。
        assert_eq!(
            sys_t1_cc, sys_t2_cc,
            "首 system breakpoint 的 TTL/位置跨轮应一致"
        );
        // turn 1 的 user 位置被打(末尾),turn 2 不再被打。
        assert!(extract_cache_control(&t1[1]).is_some());
        assert!(
            extract_cache_control(&t2[1]).is_none(),
            "turn 2 的旧 user 不再是 tail"
        );
    }

    /// **P0-5 主要验收**:OpenAI build_chat_options 下发的 cache_control
    /// 在同输入上跨调用一致(prompt_cache_key + cache_control 两个字段)。
    #[test]
    fn openai_chat_options_is_deterministic() {
        use crate::settings::ReasoningEffortSetting as R;
        let make = || {
            build_chat_options(
                AgentProviderApiType::OpenAi,
                "https://api.openai.com/v1/",
                "gpt-5-mini",
                R::Auto,
                vec![],
                Some("conv-abc-123"),
            )
        };
        let a = make();
        let b = make();
        assert_eq!(a.prompt_cache_key, b.prompt_cache_key);
        assert_eq!(a.cache_control, b.cache_control);
    }

    /// **P0-5 GPT-5 走 24h 最终路径验收**。
    #[test]
    fn openai_gpt5_path_lands_24h_cache_control() {
        use crate::settings::ReasoningEffortSetting as R;
        let opts = build_chat_options(
            AgentProviderApiType::OpenAi,
            "https://api.openai.com/v1/",
            "gpt-5-mini",
            R::Auto,
            vec![],
            Some("conv-1"),
        );
        assert_eq!(
            opts.cache_control,
            Some(CacheControl::Ephemeral24h),
            "GPT-5 系列必须下发 Ephemeral24h"
        );
        assert_eq!(
            opts.prompt_cache_key.as_deref(),
            Some("conv-1"),
            "prompt_cache_key 必须 = conversation_id"
        );
    }

    /// **P0-5 旧型号 fallback in_memory 路径验收**。
    #[test]
    fn openai_legacy_path_lands_in_memory_cache_control() {
        use crate::settings::ReasoningEffortSetting as R;
        let opts = build_chat_options(
            AgentProviderApiType::OpenAi,
            "https://api.openai.com/v1/",
            "gpt-4o-mini",
            R::Auto,
            vec![],
            Some("conv-2"),
        );
        assert_eq!(
            opts.cache_control,
            Some(CacheControl::Ephemeral),
            "旧型号 fallback Ephemeral(in_memory)"
        );
    }

    /// **conversation_id 为空不下发 prompt_cache_key**(避免跨会话误挂路由)。
    #[test]
    fn openai_empty_conversation_id_skips_cache_key() {
        use crate::settings::ReasoningEffortSetting as R;
        let opts = build_chat_options(
            AgentProviderApiType::OpenAi,
            "https://api.openai.com/v1/",
            "gpt-5",
            R::Auto,
            vec![],
            Some(""),
        );
        assert!(
            opts.prompt_cache_key.is_none(),
            "空 conversation_id 应跳过 prompt_cache_key"
        );
        // 但 cache_control 仍然走(只是没有路由哈希辅助)
        assert_eq!(opts.cache_control, Some(CacheControl::Ephemeral24h));
    }

    /// **Anthropic 路径 build_chat_options 不下发 cache_control**
    /// (Anthropic 走 per-message,不走 ChatOptions 级)。
    #[test]
    fn anthropic_chat_options_no_cache_control() {
        use crate::settings::ReasoningEffortSetting as R;
        let opts = build_chat_options(
            AgentProviderApiType::Anthropic,
            "https://api.anthropic.com/v1/",
            "claude-opus-4-7",
            R::Auto,
            vec![],
            Some("conv-3"),
        );
        assert!(
            opts.cache_control.is_none(),
            "Anthropic 的 ChatOptions 不能带 cache_control(走 per-message)"
        );
        assert!(
            opts.prompt_cache_key.is_none(),
            "Anthropic 不走 prompt_cache_key"
        );
    }

    /// **DeepSeek / Gemini / Ollama 服务端隐式缓存,不下发 cache_control**。
    #[test]
    fn implicit_cache_providers_no_cache_control() {
        use crate::settings::ReasoningEffortSetting as R;
        for api in [
            AgentProviderApiType::DeepSeek,
            AgentProviderApiType::Gemini,
            AgentProviderApiType::Ollama,
        ] {
            let opts = build_chat_options(
                api,
                "https://example.com/v1/",
                "some-model",
                R::Auto,
                vec![],
                Some("conv"),
            );
            assert!(
                opts.cache_control.is_none(),
                "{:?} 不应下发 cache_control",
                api
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 测试辅助: 给同一 crate 内的 `cache_stability_tests` 使用。
// ---------------------------------------------------------------------------

/// 测试专用包装:让同一 crate 内的其他测试模块能调用原本为文件私有的
/// `serialize_outgoing_tool_call`。仅在 `cfg(test)` 下暴露,不影响生产代码表面。
#[cfg(test)]
pub(super) fn serialize_outgoing_tool_call_for_test(
    tc: &api::message::ToolCall,
    mcp_ctx: Option<&crate::ai::agent::MCPContext>,
    server_message_data: &str,
) -> (String, Value) {
    serialize_outgoing_tool_call(tc, mcp_ctx, server_message_data)
}
