//! 模型 reasoning(思考链)能力的启发式判定。
//!
//! 背景:genai 0.6 各 adapter 内部**不**对模型做 capability gate ——
//! 只要 `ChatOptions::reasoning_effort` 非空就照样注入 thinking 参数。
//! 这对**不支持 reasoning 的模型**(claude-3-5-haiku / gpt-4o / gemini-1.5-pro)
//! 会让上游 API 直接 400,所以 client 端必须自己判定。
//!
//! 判定策略沿用 opencode `provider/transform.ts::variants()` 的"硬编码 + 子串匹配":
//! BYOP 用户填的 model id 是任意字符串,无法靠 registry 元数据,只能匹配命名约定。
//!
//! 参考:
//! - genai 0.6 anthropic adapter 的 SUPPORT_EFFORT_MODELS / SUPPORT_ADAPTTIVE_THINK_MODELS
//! - opencode v5 的 anthropicAdaptiveEfforts / OPENAI_EFFORTS 名单
//! - 各 provider 官方文档的 thinking-mode model 列表

use crate::settings::{AgentProviderApiType, ReasoningEffortSetting};
use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

/// 返回指定 (api_type, model_id) 实际可用的 reasoning effort 档位列表。
///
/// 列表为空 → picker 整个隐藏(不支持 reasoning 或 client 无法可靠注入)。
/// 列表首项 → 该模型的推荐默认档(picker 第一次出现时的初值)。
/// 末项恒为 [`ReasoningEffortSetting::Off`],表示"明确关闭思考"(对支持 effort 的模型
/// 会发 `none` 档,对 budget 系列会跳过 thinking 字段)。
///
/// 设计参照 opencode `provider/transform.ts::variants()` —— 各家档位是硬编码的,
/// 不来自 models.dev。models.dev 只给"是否支持 reasoning"布尔,具体档位由 client 内置。
pub fn model_reasoning_variants(
    api_type: AgentProviderApiType,
    model_id: &str,
) -> Vec<ReasoningEffortSetting> {
    use ReasoningEffortSetting as R;
    let id = strip_effort_suffix(&model_id.to_ascii_lowercase()).to_string();

    match api_type {
        AgentProviderApiType::Anthropic => {
            if is_opus_4_7_or_higher(&id) {
                // Opus 4.7+: adaptive thinking + xhigh + max(genai 已适配)
                return vec![R::High, R::Low, R::Medium, R::XHigh, R::Max, R::Off];
            }
            if id.contains("claude-opus-4-6") || id.contains("claude-sonnet-4-6") {
                // 4.6 系: adaptive thinking + max
                return vec![R::High, R::Low, R::Medium, R::Max, R::Off];
            }
            if is_anthropic_reasoning_model(&id) {
                // 4.5 / 3.7-sonnet 等 legacy budget,无 max
                return vec![R::High, R::Low, R::Medium, R::Off];
            }
            vec![]
        }
        AgentProviderApiType::OpenAi | AgentProviderApiType::OpenAiResp => {
            if id.contains("gpt-5") || id.contains("codex") {
                // GPT-5 / codex: minimal + xhigh 都可用
                return vec![R::Medium, R::Minimal, R::Low, R::High, R::XHigh, R::Off];
            }
            if is_openai_reasoning_model(&id) {
                // o-series: 仅 low/medium/high
                return vec![R::Medium, R::Low, R::High, R::Off];
            }
            vec![]
        }
        AgentProviderApiType::Gemini => {
            if is_gemini_reasoning_model(&id) {
                // genai 0.6 统一发 thinkingBudget 数值,2.5/3.x 不区分档位
                return vec![R::Medium, R::Low, R::High, R::Off];
            }
            vec![]
        }
        // DeepSeek thinking-mode 模型(deepseek-reasoner / v4 / thinking / r1)。
        // OpenWarp 本地 fork(`lib/rust-genai`)放宽了 adapter_shared.rs 的注入条件,
        // 让 `reasoning_effort` 顶层字段按 DeepSeek thinking_mode 文档下发。
        //
        // Ollama 后端模型 id 任意,保守留空。
        AgentProviderApiType::DeepSeek => {
            if is_deepseek_thinking_model(&id) {
                // DeepSeek 官方思考深度只有 high / max 两档(low/medium/xhigh
                // 即便服务端 deserializer 接受也只是同档别名,picker 不暴露冗余项)。
                // Off 档走"关闭思考":本地 fork genai 已支持 ChatOptions::extra_body,
                // chat_stream 在 DeepSeek+Off 时改发
                // `extra_body = {"thinking": {"type": "disabled"}}` 顶层合并。
                vec![R::High, R::Max, R::Off]
            } else {
                vec![]
            }
        }
        AgentProviderApiType::Ollama => vec![],
    }
}

/// 该模型的推荐默认档(picker 首次出现时的初值);None 表示模型不支持 reasoning。
pub fn default_reasoning_for(
    api_type: AgentProviderApiType,
    model_id: &str,
) -> Option<ReasoningEffortSetting> {
    model_reasoning_variants(api_type, model_id)
        .first()
        .copied()
}

/// Opus 4.7 及更高版本(`claude-opus-4-7` / `claude-opus-5-0` ...)。
/// 与 genai anthropic adapter 的 `is_opus_4_7_or_higher` regex 同语义。
fn is_opus_4_7_or_higher(model_name: &str) -> bool {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"claude-opus-(\d+)-(\d+)").ok());
    let Some(re) = re.as_ref() else {
        return false;
    };
    let Some(caps) = re.captures(model_name) else {
        return false;
    };
    let major = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
    let minor = caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
    matches!((major, minor), (Some(major), Some(minor)) if (major, minor) >= (4, 7))
}

/// 判定指定 (api_type, model_name) 组合是否支持 reasoning(思考链)。
///
/// 仅当返回 `true` 时才向 genai 注入 `reasoning_effort`,否则按原样发送
/// 普通 chat 请求,避免向旧模型(如 claude-3-5-haiku / gpt-4o)注入 thinking
/// 参数被上游拒绝。
///
/// 命名约定按各家 model id 风格(全转 lowercase 后子串匹配):
/// - **Anthropic**:`claude-opus-4` / `claude-sonnet-4` / `claude-haiku-4` /
///   `claude-3-7-sonnet`(extended thinking 起点)及更新版本
/// - **OpenAI / OpenAIResp**:`o1` / `o3` / `o4` 系列、`gpt-5`、`codex`
/// - **Gemini**:`gemini-2.5*` / `gemini-3*`(2.5 起 thinking,3.x 全系)
/// - **DeepSeek**:`deepseek-reasoner` / `deepseek-r1` / `deepseek-v4*` /
///   `deepseek-thinking`(官方两档:high / max 走 `reasoning_effort` 顶层字段,
///   Off 档走 `extra_body.thinking.type=disabled` 关闭思考)
/// - **Ollama**:走 OpenAI 兼容路径,后端模型 id 不可控,**保守返回 `false`**
///   (用户若确实在跑 thinking 模型,可在 Settings 显式调档,后续再放宽)
pub fn model_supports_reasoning(api_type: AgentProviderApiType, model_id: &str) -> bool {
    !model_reasoning_variants(api_type, model_id).is_empty()
}

fn strip_effort_suffix(id: &str) -> &str {
    if let Some((prefix, last)) = id.rsplit_once('-') {
        if matches!(
            last,
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "zero"
        ) {
            return prefix;
        }
    }
    id
}

fn is_anthropic_reasoning_model(id: &str) -> bool {
    // claude-3-7-sonnet 是 extended thinking 的起点(2025-02 发布)。
    if id.contains("claude-3-7-sonnet") {
        return true;
    }
    // claude-opus-4* / claude-sonnet-4* / claude-haiku-4* 全系支持。
    // 同时兼容 `4.5` / `4-5` / `4_5` 三种点号风格。
    let four_series = ["claude-opus-4", "claude-sonnet-4", "claude-haiku-4"];
    if four_series.iter().any(|prefix| id.contains(prefix)) {
        return true;
    }
    false
}

fn is_openai_reasoning_model(id: &str) -> bool {
    // o-series reasoning 模型(o1 / o1-mini / o1-pro / o3 / o3-mini / o4 / o4-mini)。
    // 注意 `o1-mini` 在 opencode azure case 被排除,但 OpenAI 官方接受 reasoning_effort,
    // 这里按上游 OpenAI 行为保留。
    let o_series_prefixes = ["o1", "o3", "o4"];
    for prefix in o_series_prefixes {
        if id == prefix
            || id.starts_with(&format!("{prefix}-"))
            || id.starts_with(&format!("{prefix}_"))
        {
            return true;
        }
    }
    // GPT-5 系列(全系 reasoning)+ codex 变体(gpt-5-codex / codex-* / o*-codex 等)。
    if id.contains("gpt-5") || id.contains("codex") {
        return true;
    }
    false
}

fn is_deepseek_thinking_model(id: &str) -> bool {
    // DeepSeek thinking-mode 模型名约定:reasoner / r1 / v4* / *-thinking。
    // `deepseek-v4` 子串覆盖 `deepseek-v4-flash` 等后续变体。
    id.contains("deepseek-reasoner")
        || id.contains("deepseek-v4")
        || id.contains("deepseek-thinking")
        || id.contains("deepseek-r1")
}

fn is_gemini_reasoning_model(id: &str) -> bool {
    // gemini-2.5-* 起 thinking 模式(flash-thinking-exp / pro / pro-thinking)。
    // gemini-3.* 全系(opencode 在 levels 上区分 3 / 3.1)。
    if id.contains("gemini-2.5") || id.contains("gemini-3") {
        return true;
    }
    // 历史 thinking exp 通道(2.0 flash-thinking-exp 也算)。
    if id.contains("thinking") {
        return true;
    }
    false
}

/// 对齐 opencode `model.capabilities.interleaved.field`(`provider/provider.ts:1182-1187`、
/// `provider/transform.ts:217-249`):某些 thinking-mode 模型要求把历史 reasoning 以特定
/// 字段名挂回 assistant message。
///
/// opencode 的两个合法取值是 `"reasoning_content"` 和 `"reasoning_details"`:
/// - `reasoning_content`:绝大多数国产 OpenAI 兼容 thinking 模型(DeepSeek/Kimi/MiMo/Qwen3/
///   GLM-thinking/MiniMax/Hunyuan/Ernie/Doubao …)使用的顶层字符串字段。
/// - `reasoning_details`:OpenRouter 等聚合 provider 的 array 形式;genai 0.6 OpenAI adapter
///   暂未支持(只能 hoist 顶层 `reasoning_content` 字符串)— 留作 enum 占位,
///   命中时退化按 `ReasoningContent` 序列化(够覆盖大多数兼容端点)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReasoningInterleavedField {
    /// 顶层 `reasoning_content` 字符串字段。
    ReasoningContent,
    /// 顶层 `reasoning_details` 数组字段(预留,当前序列化路径走 fallback)。
    ReasoningDetails,
}

/// 国产 / 第三方 OpenAI 兼容 thinking 模型 model_id 子串匹配表。
///
/// 设计照 opencode `models.dev` 的 `capabilities.interleaved` 数据字段 —— 每条
/// thinking 模型在 catalog 里显式声明 field,client 端按 model 查表决定回传形态。
/// warp 没有外置 catalog,把表硬编码进来,后续可改为可配置覆盖。
///
/// 规则:**小写 model_id 子串包含 needle 即命中**。顺序无关(短串与长串不互相覆盖,
/// 第一个命中即可)。维护时只需在表里加一行,不改控制流。
const INTERLEAVED_RULES: &[(&str, ReasoningInterleavedField)] = {
    use ReasoningInterleavedField::ReasoningContent as RC;
    &[
        // DeepSeek 全系 thinking(用户常把官方 OpenAI 兼容端点配为 OpenAi api_type)
        ("deepseek-reasoner", RC),
        ("deepseek-v4", RC),
        ("deepseek-r1", RC),
        ("deepseek-thinking", RC),
        // Moonshot Kimi 系列
        ("kimi", RC),
        ("moonshot", RC),
        // 小米 MiMo(报错 issue 来源:`mimo-v2.5-pro`)
        ("mimo", RC),
        // 阿里 Qwen thinking / QwQ(DashScope OpenAI 兼容端点 + enable_thinking)
        ("qwen3", RC),
        ("qwq", RC),
        // 智谱 GLM thinking(z.ai / 智谱开放平台)
        ("zai-glm", RC),
        ("glm-4.5-thinking", RC),
        ("glm-4.6-thinking", RC),
        ("glm-4.7", RC),
        // MiniMax M1 thinking
        ("minimax-m1", RC),
        // 腾讯混元 T1 thinking
        ("hunyuan-t1", RC),
        // 百度文心 X1 / thinking
        ("ernie-x1", RC),
        ("ernie-thinking", RC),
        // 阶跃 Step thinking
        ("step-r-mini", RC),
        ("step-thinking", RC),
        // 字节豆包 thinking
        ("doubao-thinking", RC),
        ("doubao-1-5-thinking", RC),
        // 零一 Yi thinking
        ("yi-thinking", RC),
    ]
};

/// 运行时 latch 集合:记录哪些 (api_type, model_id) 在某次 stream 里发过
/// `ReasoningChunk` —— 即"该 endpoint 服务端认识 reasoning_content 字段"的
/// 精准启发式信号。
///
/// 这是和 opencode 的关键差异:opencode 用 `models.dev` 外置 catalog 静态声明
/// `capabilities.interleaved`,warp 没有 catalog,改用 stream 探测 —— 发过 reasoning
/// chunk 的 endpoint 必然认 reasoning_content,**Cerebras / Groq / OpenRouter
/// / Together AI / SambaNova**等不发该 chunk 的 strict provider 永远不会被 latch,
/// 自动避开 zerx-lab/warp #25 那类误挂 400。
///
/// 信号只跨 stream/turn 在内存里保留,进程重启清空(下次见到 reasoning chunk
/// 会重新 latch)。仅对 OpenAi / OpenAiResp api_type 有意义 —— DeepSeek 整个
/// adapter 默认 echo;Anthropic / Gemini 各自走 thinking blocks / thought
/// signatures,即便 stream 出 reasoning chunk 也不需要顶层 `reasoning_content` 字段。
static REASONING_ECHO_LATCH: OnceLock<RwLock<HashSet<(AgentProviderApiType, String)>>> =
    OnceLock::new();

fn latch_set() -> &'static RwLock<HashSet<(AgentProviderApiType, String)>> {
    REASONING_ECHO_LATCH.get_or_init(|| RwLock::new(HashSet::new()))
}

/// 在 stream 收到 `ReasoningChunk` 时调用,把 (api_type, lowercased model_id) 标记为
/// "需要回传 reasoning_content"。下一轮 [`model_reasoning_interleaved`] /
/// [`model_requires_reasoning_echo`] 查询时优先返回 `Some(ReasoningContent)` /
/// `true`,无论是否在静态 [`INTERLEAVED_RULES`] 表内。
///
/// 仅对 OpenAi / OpenAiResp api_type 真正落地写入(其他 api_type 早就有原生
/// reasoning 通道,latch 无收益且会污染 set);其余路径快速 return。
pub fn note_reasoning_seen(api_type: AgentProviderApiType, model_id: &str) {
    if !matches!(
        api_type,
        AgentProviderApiType::OpenAi | AgentProviderApiType::OpenAiResp
    ) {
        return;
    }
    let key = (api_type, model_id.to_ascii_lowercase());
    if let Ok(s) = latch_set().read() {
        if s.contains(&key) {
            return;
        }
    }
    if let Ok(mut s) = latch_set().write() {
        s.insert(key);
    }
}

fn latch_contains(api_type: AgentProviderApiType, model_id_lower: &str) -> bool {
    latch_set()
        .read()
        .map(|s| s.contains(&(api_type, model_id_lower.to_string())))
        .unwrap_or(false)
}

/// 测试用:清空 latch。生产代码不应调用。
#[cfg(test)]
fn reset_reasoning_latch() {
    if let Ok(mut s) = latch_set().write() {
        s.clear();
    }
}

/// 查表得到模型应使用的 reasoning interleaved 字段;`None` 表示该 endpoint 不应回传
/// `reasoning_content` —— 即便 stream 收到了真实 reasoning,回放时也丢弃,避免被
/// **Cerebras / Groq / OpenRouter / Together AI / SambaNova / OpenAI 官方**等
/// 严格 schema provider 用 400 `wrong_api_format` 拒绝。
///
/// 对齐 opencode `provider/transform.ts:217-249` 的 `capabilities.interleaved` 语义,
/// 增强为两段决策(精度优先 → 召回率兜底):
///
/// 1. **运行时 latch**(精准):此 (api_type, model_id) 在历史 stream 中发过
///    `ReasoningChunk` → 该 endpoint 服务端必然认 reasoning_content 字段 →
///    返回 `Some(ReasoningContent)`。覆盖 [`INTERLEAVED_RULES`] 表外的任意国产 /
///    第三方 thinking 模型,无需维护白名单。
/// 2. **静态 hint**(冷启动):latch 未命中时回退查 [`INTERLEAVED_RULES`] 子串表
///    与 api_type 默认值:
///    - **DeepSeek api_type**:整个 adapter 即 DeepSeek 专属,全模型 echo
///      (与 opencode 默认值 `apiID.includes("deepseek") → { field: "reasoning_content" }` 一致)
///    - **OpenAI / OpenAiResp**:走子串表,覆盖国内主流 thinking 模型
///    - **Anthropic / Gemini / Ollama**:`None`(Anthropic 走 thinking blocks,
///      Gemini 走 thought signatures,Ollama 走原生 reasoning;均不需要这个 echo)
pub fn model_reasoning_interleaved(
    api_type: AgentProviderApiType,
    model_id: &str,
) -> Option<ReasoningInterleavedField> {
    use AgentProviderApiType as T;
    let id = model_id.to_ascii_lowercase();
    // (1) 运行时 latch —— 上一轮 stream 发过 reasoning chunk 就锁定 echo
    if matches!(api_type, T::OpenAi | T::OpenAiResp) && latch_contains(api_type, &id) {
        return Some(ReasoningInterleavedField::ReasoningContent);
    }
    // (2) 静态 hint —— 冷启动 / 首轮(尚未 stream 过)的兜底
    match api_type {
        T::DeepSeek => Some(ReasoningInterleavedField::ReasoningContent),
        T::OpenAi | T::OpenAiResp => INTERLEAVED_RULES
            .iter()
            .find(|(needle, _)| id.contains(needle))
            .map(|(_, f)| *f),
        T::Anthropic | T::Gemini | T::Ollama => None,
    }
}

/// 判定指定 (api_type, model_id) 是否需要在每条 assistant message 上回传
/// `reasoning_content` 字段(包括空串占位)。等价于 [`model_reasoning_interleaved`]
/// `.is_some()`,保留旧名以兼容已有调用点。
///
/// 背景:`deepseek-v4-flash` / `mimo-v2.5-pro` 等新一代 thinking-mode 模型把
/// server-side 校验从"仅含 tool_calls 的 assistant 必须带 reasoning_content"收紧到
/// "thinking-mode 下每条 assistant 必须带 reasoning_content,缺失即 400
/// `The reasoning_content in the thinking mode must be passed back to the API`"。
/// genai 0.6 序列化层(`adapter_shared.rs:368-373`)只 echo 已有的
/// `ContentPart::ReasoningContent`,**不会自动补缺**,所以 client 层必须强制挂上
/// 占位字段(空串也行 — genai 原样 insert,服务端只校验字段存在性)。
pub fn model_requires_reasoning_echo(api_type: AgentProviderApiType, model_id: &str) -> bool {
    model_reasoning_interleaved(api_type, model_id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_supported() {
        let t = AgentProviderApiType::Anthropic;
        assert!(model_supports_reasoning(t, "claude-opus-4-5"));
        assert!(model_supports_reasoning(t, "claude-sonnet-4-6"));
        assert!(model_supports_reasoning(t, "claude-opus-4-7"));
        assert!(model_supports_reasoning(t, "claude-3-7-sonnet-20250219"));
        // 后缀不影响判定
        assert!(model_supports_reasoning(t, "claude-sonnet-4-5-high"));
        assert!(model_supports_reasoning(t, "claude-opus-4-7-max"));
    }

    #[test]
    fn anthropic_unsupported() {
        let t = AgentProviderApiType::Anthropic;
        assert!(!model_supports_reasoning(t, "claude-3-5-haiku-20241022"));
        assert!(!model_supports_reasoning(t, "claude-3-5-sonnet-20241022"));
        assert!(!model_supports_reasoning(t, "claude-3-opus-20240229"));
        assert!(!model_supports_reasoning(t, "claude-2.1"));
    }

    #[test]
    fn openai_supported() {
        let t = AgentProviderApiType::OpenAi;
        assert!(model_supports_reasoning(t, "o1"));
        assert!(model_supports_reasoning(t, "o1-mini"));
        assert!(model_supports_reasoning(t, "o3-mini"));
        assert!(model_supports_reasoning(t, "o4-mini"));
        assert!(model_supports_reasoning(t, "gpt-5"));
        assert!(model_supports_reasoning(t, "gpt-5-codex"));
        assert!(model_supports_reasoning(t, "gpt-5-codex-high"));
    }

    #[test]
    fn openai_unsupported() {
        let t = AgentProviderApiType::OpenAi;
        assert!(!model_supports_reasoning(t, "gpt-4o"));
        assert!(!model_supports_reasoning(t, "gpt-4-turbo"));
        assert!(!model_supports_reasoning(t, "gpt-3.5-turbo"));
    }

    #[test]
    fn gemini_supported() {
        let t = AgentProviderApiType::Gemini;
        assert!(model_supports_reasoning(t, "gemini-2.5-pro"));
        assert!(model_supports_reasoning(t, "gemini-2.5-flash"));
        assert!(model_supports_reasoning(t, "gemini-3-pro"));
        assert!(model_supports_reasoning(t, "gemini-2.0-flash-thinking-exp"));
    }

    #[test]
    fn gemini_unsupported() {
        let t = AgentProviderApiType::Gemini;
        assert!(!model_supports_reasoning(t, "gemini-1.5-pro"));
        assert!(!model_supports_reasoning(t, "gemini-1.5-flash"));
        assert!(!model_supports_reasoning(t, "gemini-2.0-flash"));
    }

    #[test]
    fn deepseek_thinking_models_supported() {
        let t = AgentProviderApiType::DeepSeek;
        assert!(model_supports_reasoning(t, "deepseek-reasoner"));
        assert!(model_supports_reasoning(t, "deepseek-v4"));
        assert!(model_supports_reasoning(t, "deepseek-v4-flash"));
        assert!(model_supports_reasoning(t, "deepseek-thinking"));
        assert!(model_supports_reasoning(t, "deepseek-r1"));
        // 普通 chat 模型不带 thinking
        assert!(!model_supports_reasoning(t, "deepseek-chat"));
        assert!(!model_supports_reasoning(t, "deepseek-coder"));
    }

    #[test]
    fn ollama_always_false() {
        assert!(!model_supports_reasoning(
            AgentProviderApiType::Ollama,
            "qwq-32b"
        ));
    }

    #[test]
    fn requires_reasoning_echo_deepseek() {
        // DeepSeek api_type 一律 echo,不挑 model
        assert!(model_requires_reasoning_echo(
            AgentProviderApiType::DeepSeek,
            "deepseek-v4-flash"
        ));
        assert!(model_requires_reasoning_echo(
            AgentProviderApiType::DeepSeek,
            "deepseek-chat"
        ));
        assert!(model_requires_reasoning_echo(
            AgentProviderApiType::DeepSeek,
            "deepseek-reasoner"
        ));
    }

    #[test]
    fn requires_reasoning_echo_kimi_via_openai() {
        let t = AgentProviderApiType::OpenAi;
        assert!(model_requires_reasoning_echo(t, "kimi-k2-thinking"));
        assert!(model_requires_reasoning_echo(t, "moonshot-v1-32k"));
        assert!(model_requires_reasoning_echo(
            AgentProviderApiType::OpenAiResp,
            "Kimi-Latest"
        ));
        // 普通 OpenAI 模型不 echo
        assert!(!model_requires_reasoning_echo(t, "gpt-5"));
        assert!(!model_requires_reasoning_echo(t, "o3-mini"));
    }

    #[test]
    fn requires_reasoning_echo_deepseek_via_openai() {
        // DeepSeek 官方端点是 OpenAI-compatible 的,用户常把它配成 OpenAI api_type 的
        // BYOP provider。thinking 模型必须回 echo `reasoning_content`,否则 400。
        let t = AgentProviderApiType::OpenAi;
        assert!(model_requires_reasoning_echo(t, "deepseek-v4-flash"));
        assert!(model_requires_reasoning_echo(t, "deepseek-v4"));
        assert!(model_requires_reasoning_echo(t, "deepseek-reasoner"));
        assert!(model_requires_reasoning_echo(t, "deepseek-r1"));
        assert!(model_requires_reasoning_echo(t, "deepseek-thinking"));
        // 大小写不敏感
        assert!(model_requires_reasoning_echo(t, "DeepSeek-V4-Flash"));
        // OpenAiResp 同源
        assert!(model_requires_reasoning_echo(
            AgentProviderApiType::OpenAiResp,
            "deepseek-r1"
        ));
        // 非 thinking 的 DeepSeek 模型(deepseek-chat / deepseek-coder)走 OpenAI
        // 兼容路径时不进 thinking-mode 校验,无需 echo
        assert!(!model_requires_reasoning_echo(t, "deepseek-chat"));
        assert!(!model_requires_reasoning_echo(t, "deepseek-coder"));
    }

    #[test]
    fn opus_4_7_variants_have_xhigh_and_max() {
        let v =
            model_reasoning_variants(AgentProviderApiType::Anthropic, "claude-opus-4-7-20260101");
        assert!(v.contains(&ReasoningEffortSetting::XHigh));
        assert!(v.contains(&ReasoningEffortSetting::Max));
        assert_eq!(v.first().copied(), Some(ReasoningEffortSetting::High));
        assert_eq!(v.last().copied(), Some(ReasoningEffortSetting::Off));
    }

    #[test]
    fn opus_5_0_variants_treated_as_4_7_plus() {
        let v = model_reasoning_variants(AgentProviderApiType::Anthropic, "claude-opus-5-0");
        assert!(v.contains(&ReasoningEffortSetting::XHigh));
        assert!(v.contains(&ReasoningEffortSetting::Max));
    }

    #[test]
    fn sonnet_4_6_variants_have_max_no_xhigh() {
        let v = model_reasoning_variants(AgentProviderApiType::Anthropic, "claude-sonnet-4-6");
        assert!(v.contains(&ReasoningEffortSetting::Max));
        assert!(!v.contains(&ReasoningEffortSetting::XHigh));
    }

    #[test]
    fn sonnet_4_5_variants_legacy_no_max_no_xhigh() {
        let v = model_reasoning_variants(AgentProviderApiType::Anthropic, "claude-sonnet-4-5");
        assert!(!v.contains(&ReasoningEffortSetting::Max));
        assert!(!v.contains(&ReasoningEffortSetting::XHigh));
        assert!(v.contains(&ReasoningEffortSetting::High));
    }

    #[test]
    fn claude_3_5_haiku_variants_empty() {
        let v =
            model_reasoning_variants(AgentProviderApiType::Anthropic, "claude-3-5-haiku-20241022");
        assert!(v.is_empty());
    }

    #[test]
    fn gpt_5_variants_have_minimal_and_xhigh() {
        let v = model_reasoning_variants(AgentProviderApiType::OpenAi, "gpt-5");
        assert!(v.contains(&ReasoningEffortSetting::Minimal));
        assert!(v.contains(&ReasoningEffortSetting::XHigh));
        assert_eq!(v.first().copied(), Some(ReasoningEffortSetting::Medium));
    }

    #[test]
    fn o3_variants_no_minimal_no_xhigh() {
        let v = model_reasoning_variants(AgentProviderApiType::OpenAi, "o3-mini");
        assert!(!v.contains(&ReasoningEffortSetting::Minimal));
        assert!(!v.contains(&ReasoningEffortSetting::XHigh));
        assert!(v.contains(&ReasoningEffortSetting::High));
    }

    #[test]
    fn gpt_4o_variants_empty() {
        let v = model_reasoning_variants(AgentProviderApiType::OpenAi, "gpt-4o");
        assert!(v.is_empty());
    }

    #[test]
    fn gemini_2_5_variants_three_levels() {
        let v = model_reasoning_variants(AgentProviderApiType::Gemini, "gemini-2.5-pro");
        assert_eq!(v.len(), 4); // Medium, Low, High, Off
        assert!(v.contains(&ReasoningEffortSetting::Off));
    }

    #[test]
    fn gemini_1_5_variants_empty() {
        let v = model_reasoning_variants(AgentProviderApiType::Gemini, "gemini-1.5-pro");
        assert!(v.is_empty());
    }

    #[test]
    fn deepseek_thinking_variants_two_levels_plus_off() {
        let v = model_reasoning_variants(AgentProviderApiType::DeepSeek, "deepseek-reasoner");
        // DeepSeek 官方:仅 high / max 两档 + Off
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], ReasoningEffortSetting::High);
        assert_eq!(v[1], ReasoningEffortSetting::Max);
        assert_eq!(v[2], ReasoningEffortSetting::Off);
        // 不应暴露冗余别名
        assert!(!v.contains(&ReasoningEffortSetting::Medium));
        assert!(!v.contains(&ReasoningEffortSetting::Low));
        assert!(!v.contains(&ReasoningEffortSetting::XHigh));
    }

    #[test]
    fn deepseek_chat_variants_empty() {
        assert!(
            model_reasoning_variants(AgentProviderApiType::DeepSeek, "deepseek-chat").is_empty()
        );
    }

    #[test]
    fn ollama_variants_empty() {
        assert!(model_reasoning_variants(AgentProviderApiType::Ollama, "qwq-32b").is_empty());
    }

    #[test]
    fn default_reasoning_for_consistency() {
        // default 应等于 variants 列表第一项
        assert_eq!(
            default_reasoning_for(AgentProviderApiType::Anthropic, "claude-opus-4-7"),
            Some(ReasoningEffortSetting::High)
        );
        assert_eq!(
            default_reasoning_for(AgentProviderApiType::OpenAi, "gpt-5"),
            Some(ReasoningEffortSetting::Medium)
        );
        assert_eq!(
            default_reasoning_for(AgentProviderApiType::OpenAi, "gpt-4o"),
            None
        );
    }

    #[test]
    fn supports_reasoning_consistent_with_variants() {
        // 单一来源:supports == !variants.is_empty()
        for (t, m) in [
            (AgentProviderApiType::Anthropic, "claude-opus-4-7"),
            (AgentProviderApiType::Anthropic, "claude-3-5-haiku"),
            (AgentProviderApiType::OpenAi, "gpt-5"),
            (AgentProviderApiType::OpenAi, "gpt-4o"),
            (AgentProviderApiType::Gemini, "gemini-2.5-pro"),
            (AgentProviderApiType::Gemini, "gemini-1.5-pro"),
            (AgentProviderApiType::DeepSeek, "deepseek-reasoner"),
        ] {
            assert_eq!(
                model_supports_reasoning(t, m),
                !model_reasoning_variants(t, m).is_empty(),
                "{t:?}/{m}"
            );
        }
    }

    #[test]
    fn requires_reasoning_echo_domestic_thinking_models() {
        // 国产 OpenAI 兼容 thinking 模型必须 echo `reasoning_content`,
        // 否则服务端 400 `The reasoning_content in the thinking mode must be passed back`。
        // 测试在 OpenAi api_type 下命中(用户最常见的 BYOP 配法)。
        let t = AgentProviderApiType::OpenAi;
        // 小米 MiMo(本次 issue 触发模型)
        assert!(model_requires_reasoning_echo(t, "mimo-v2.5-pro"));
        assert!(model_requires_reasoning_echo(t, "mimo-vl-7b"));
        // 阿里 Qwen3 thinking / QwQ
        assert!(model_requires_reasoning_echo(t, "qwen3-235b-a22b-thinking-2507"));
        assert!(model_requires_reasoning_echo(t, "qwq-32b-preview"));
        // 智谱 GLM thinking
        assert!(model_requires_reasoning_echo(t, "zai-glm-4.7"));
        assert!(model_requires_reasoning_echo(t, "glm-4.6-thinking"));
        assert!(model_requires_reasoning_echo(t, "glm-4.5-thinking"));
        // MiniMax / 混元 / 文心 / 阶跃 / 豆包 / Yi
        assert!(model_requires_reasoning_echo(t, "minimax-m1-80k"));
        assert!(model_requires_reasoning_echo(t, "hunyuan-t1-latest"));
        assert!(model_requires_reasoning_echo(t, "ernie-x1-turbo-32k"));
        assert!(model_requires_reasoning_echo(t, "step-r-mini"));
        assert!(model_requires_reasoning_echo(t, "doubao-1-5-thinking-pro"));
        assert!(model_requires_reasoning_echo(t, "yi-thinking-v1"));
        // OpenAiResp 同源
        let r = AgentProviderApiType::OpenAiResp;
        assert!(model_requires_reasoning_echo(r, "MiMo-V2.5-Pro"));
        assert!(model_requires_reasoning_echo(r, "Qwen3-Coder-Thinking"));
    }

    #[test]
    fn reasoning_interleaved_field_for_domestic_models() {
        // model_reasoning_interleaved 必须返回 ReasoningContent(目前所有 INTERLEAVED_RULES
        // 都是 ReasoningContent;ReasoningDetails 是预留 enum 占位)。
        let t = AgentProviderApiType::OpenAi;
        assert_eq!(
            model_reasoning_interleaved(t, "mimo-v2.5-pro"),
            Some(ReasoningInterleavedField::ReasoningContent)
        );
        assert_eq!(
            model_reasoning_interleaved(t, "deepseek-v4-flash"),
            Some(ReasoningInterleavedField::ReasoningContent)
        );
        // DeepSeek api_type 全模型(包括非 thinking 的 chat / coder)都返回 ReasoningContent —
        // adapter 即 DeepSeek 专属,与 opencode `apiID.includes("deepseek") →
        // { field: "reasoning_content" }` 默认对齐。
        let d = AgentProviderApiType::DeepSeek;
        assert_eq!(
            model_reasoning_interleaved(d, "deepseek-chat"),
            Some(ReasoningInterleavedField::ReasoningContent)
        );
        // 无声明的模型 / 非 OpenAI 系 → None
        assert_eq!(model_reasoning_interleaved(t, "gpt-5"), None);
        assert_eq!(model_reasoning_interleaved(t, "gpt-4o"), None);
        assert_eq!(
            model_reasoning_interleaved(AgentProviderApiType::Anthropic, "claude-opus-4-7"),
            None
        );
        assert_eq!(
            model_reasoning_interleaved(AgentProviderApiType::Gemini, "gemini-2.5-pro"),
            None
        );
        assert_eq!(
            model_reasoning_interleaved(AgentProviderApiType::Ollama, "qwq-32b"),
            None
        );
    }

    #[test]
    fn requires_reasoning_echo_strict_providers_excluded() {
        // OpenAI 官方 / Anthropic / Gemini / 普通 OpenAI 模型 → 不挂 reasoning_content,
        // 避免 Cerebras / Groq / OpenRouter 等 strict OpenAI provider 400 `wrong_api_format`
        // (zerx-lab/warp #25)。
        let t = AgentProviderApiType::OpenAi;
        assert!(!model_requires_reasoning_echo(t, "gpt-5"));
        assert!(!model_requires_reasoning_echo(t, "gpt-4o"));
        assert!(!model_requires_reasoning_echo(t, "o3-mini"));
        // 名字里既不含已知 thinking 子串又不是 DeepSeek api_type 的随便 BYOP 模型
        assert!(!model_requires_reasoning_echo(t, "llama-3.3-70b-instruct"));
        assert!(!model_requires_reasoning_echo(t, "mistral-large-2407"));
    }

    #[test]
    fn runtime_latch_overrides_static_table() {
        // 任意未在 INTERLEAVED_RULES 内的国产/第三方 thinking 模型,
        // 一旦 stream 发过 reasoning chunk → 下一轮起自动 echo。
        // 用一个故意"不存在"的 model id 验证 latch 是真起作用的。
        let t = AgentProviderApiType::OpenAi;
        let exotic = "totally-new-thinking-model-2099";
        reset_reasoning_latch();
        assert!(
            !model_requires_reasoning_echo(t, exotic),
            "未 latch 前白名单外模型应不 echo"
        );
        note_reasoning_seen(t, exotic);
        assert!(
            model_requires_reasoning_echo(t, exotic),
            "latch 后必须 echo"
        );
        assert_eq!(
            model_reasoning_interleaved(t, exotic),
            Some(ReasoningInterleavedField::ReasoningContent)
        );
        // 大小写不敏感
        assert!(model_requires_reasoning_echo(t, "Totally-New-Thinking-Model-2099"));
        // OpenAiResp 与 OpenAi 是独立 key —— 但同 endpoint 类别都应 latch 各自
        let r = AgentProviderApiType::OpenAiResp;
        assert!(!model_requires_reasoning_echo(r, exotic), "另一 api_type 不串");
        note_reasoning_seen(r, exotic);
        assert!(model_requires_reasoning_echo(r, exotic));
        reset_reasoning_latch();
    }

    #[test]
    fn runtime_latch_never_writes_for_strict_api_types() {
        // Anthropic / Gemini / Ollama 各自走原生 reasoning 通道,即使有人误调
        // note_reasoning_seen 也不能污染 latch(否则跨 api_type 共用 model_id
        // 可能在 OpenAi 路径误命中 —— 我们用 (api_type, id) 复合 key,本来就隔离,
        // 但语义上额外保险:这些 api_type 不进 latch)。
        reset_reasoning_latch();
        for at in [
            AgentProviderApiType::Anthropic,
            AgentProviderApiType::Gemini,
            AgentProviderApiType::Ollama,
            AgentProviderApiType::DeepSeek,
        ] {
            note_reasoning_seen(at, "some-model");
        }
        // 任何 OpenAi/OpenAiResp 查询都不应被这些 noise 命中
        assert!(!model_requires_reasoning_echo(
            AgentProviderApiType::OpenAi,
            "some-model"
        ));
        assert!(!model_requires_reasoning_echo(
            AgentProviderApiType::OpenAiResp,
            "some-model"
        ));
        reset_reasoning_latch();
    }

    #[test]
    fn requires_reasoning_echo_others_false() {
        assert!(!model_requires_reasoning_echo(
            AgentProviderApiType::Anthropic,
            "claude-opus-4-7"
        ));
        assert!(!model_requires_reasoning_echo(
            AgentProviderApiType::Gemini,
            "gemini-2.5-pro"
        ));
        assert!(!model_requires_reasoning_echo(
            AgentProviderApiType::Ollama,
            "qwq-32b"
        ));
    }
}
