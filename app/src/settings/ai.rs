//! Settings for Blocklist AI.
//!
//! These settings are currently used to configure the underlying model/API used to power the AI
//! UX, as well as small UX configurations.

use std::collections::HashMap;
use std::path::PathBuf;

use indexmap::IndexMap;

use crate::ai::request_usage_model::RequestLimitInfo;
use crate::auth::AuthStateProvider;
use crate::report_if_error;
use crate::terminal::CLIAgent;
use crate::workspaces::user_workspaces::UserWorkspaces;
use cfg_if::cfg_if;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use warpui::platform::OperatingSystem;
use warpui::{
    platform::keyboard::KeyCode, AppContext, Entity, ModelContext, SingletonEntity, UpdateModel,
};

use settings::{
    define_settings_group, RespectUserSyncSetting, Setting, SupportedPlatforms, SyncToCloud,
};
use warp_core::execution_mode::AppExecutionMode;
use warp_core::features::FeatureFlag;

use serde::{de::Deserializer, Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub enum FocusedTerminalInfoEvent {
    TerminalInfoUpdated,
}

/// Singleton model that is used to track the remote sessions in the terminal.
/// Useful for organizations that have restrictions on using AI in sessions in
/// remote sessions.
#[derive(Default, Clone, Debug)]
pub struct FocusedTerminalInfo {
    contains_any_remote_blocks: bool,
    contains_any_restored_remote_blocks: bool,
}

impl FocusedTerminalInfo {
    pub fn new(_: &mut ModelContext<Self>) -> Self {
        Self {
            contains_any_remote_blocks: false,
            contains_any_restored_remote_blocks: false,
        }
    }

    pub fn contains_any_remote_blocks(&self) -> bool {
        self.contains_any_remote_blocks
    }

    pub fn contains_any_restored_remote_blocks(&self) -> bool {
        self.contains_any_restored_remote_blocks
    }

    /// Updates both remote blocks and restored blocks status in a single atomic operation.
    /// Only emits a TerminalInfoUpdated event if either value changes.
    /// Returns true if the event was emitted.
    pub fn update(
        &mut self,
        contains_any_remote_blocks: bool,
        contains_any_restored_remote_blocks: bool,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let remote_changed = self.contains_any_remote_blocks != contains_any_remote_blocks;
        let restored_changed =
            self.contains_any_restored_remote_blocks != contains_any_restored_remote_blocks;

        if remote_changed || restored_changed {
            self.contains_any_remote_blocks = contains_any_remote_blocks;
            self.contains_any_restored_remote_blocks = contains_any_restored_remote_blocks;
            ctx.emit(FocusedTerminalInfoEvent::TerminalInfoUpdated);
            return true;
        }

        false
    }
}

impl Entity for FocusedTerminalInfo {
    type Event = FocusedTerminalInfoEvent;
}

impl SingletonEntity for FocusedTerminalInfo {}

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    EnumIter,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Physical key used to toggle voice input.",
    rename_all = "snake_case"
)]
pub enum VoiceInputToggleKey {
    #[default]
    #[schemars(description = "No toggle key assigned.")]
    None,
    /// Fn key is default toggle key for Mac, when the feature is toggled on.
    #[schemars(description = "Fn key.")]
    Fn,
    /// Alt or Option key (left side).
    #[schemars(description = "Alt or Option key (left side).")]
    AltLeft,
    /// Alt or Option key (right side). Used as default toggle
    /// key for Windows and Linux, , when the feature is toggled on.
    #[schemars(description = "Alt or Option key (right side).")]
    AltRight,
    #[schemars(description = "Control key (left side).")]
    ControlLeft,
    #[schemars(description = "Control key (right side).")]
    ControlRight,
    /// The Windows, ⌘, Command, or other OS symbol key.
    #[schemars(description = "Super, Windows, or Command key (left side).")]
    SuperLeft,
    /// The Windows, ⌘, Command, or other OS symbol key.
    #[schemars(description = "Super, Windows, or Command key (right side).")]
    SuperRight,
    #[schemars(description = "Shift key (left side).")]
    ShiftLeft,
    #[schemars(description = "Shift key (right side).")]
    ShiftRight,
}

settings::macros::implement_setting_for_enum!(
    VoiceInputToggleKey,
    AISettings,
    SupportedPlatforms::DESKTOP,
    // Never sync to cloud to allow users to use different toggle keys on different devices,
    // especially given platform differences.
    SyncToCloud::Never,
    private: false,
    toml_path: "agents.voice.voice_input_toggle_key",
    description: "The key used to toggle voice input.",
);

impl VoiceInputToggleKey {
    pub fn all_possible_values() -> Vec<VoiceInputToggleKey> {
        let all_keys = VoiceInputToggleKey::iter().collect();
        match OperatingSystem::get() {
            OperatingSystem::Mac => all_keys,
            // For non-Mac platforms, we exclude the `Fn` key since it may not be correctly identified by winit.
            // In particular, we saw it is unidentified for a ThinkPad with a standard keyboard.
            OperatingSystem::Windows | OperatingSystem::Linux | OperatingSystem::Other(_) => {
                all_keys
                    .into_iter()
                    .filter(|key| *key != VoiceInputToggleKey::Fn)
                    .collect()
            }
        }
    }

    /// Display name for choosing key from the AI settings page.
    pub fn display_name(&self) -> &'static str {
        // We use the underlying host OS to determine the correct key name to display.
        let (super_key_name, alt_key_name): (&'static str, &'static str) =
            match OperatingSystem::get() {
                OperatingSystem::Mac => ("Command", "Option"),
                OperatingSystem::Windows => ("Windows", "Alt"),
                OperatingSystem::Linux | OperatingSystem::Other(_) => ("Super", "Alt"),
            };

        match self {
            VoiceInputToggleKey::None => "None",
            VoiceInputToggleKey::Fn => "Fn",
            VoiceInputToggleKey::AltLeft => {
                Box::leak(format!("{alt_key_name} (Left)").into_boxed_str())
            }
            VoiceInputToggleKey::AltRight => {
                Box::leak(format!("{alt_key_name} (Right)").into_boxed_str())
            }
            VoiceInputToggleKey::ControlLeft => "Control (Left)",
            VoiceInputToggleKey::ControlRight => "Control (Right)",
            VoiceInputToggleKey::SuperLeft => {
                Box::leak(format!("{super_key_name} (Left)").into_boxed_str())
            }
            VoiceInputToggleKey::SuperRight => {
                Box::leak(format!("{super_key_name} (Right)").into_boxed_str())
            }
            VoiceInputToggleKey::ShiftLeft => "Shift (Left)",
            VoiceInputToggleKey::ShiftRight => "Shift (Right)",
        }
    }

    pub fn to_key_code(&self) -> Option<KeyCode> {
        match self {
            VoiceInputToggleKey::None => None,
            VoiceInputToggleKey::Fn => Some(KeyCode::Fn),
            VoiceInputToggleKey::AltLeft => Some(KeyCode::AltLeft),
            VoiceInputToggleKey::AltRight => Some(KeyCode::AltRight),
            VoiceInputToggleKey::ControlLeft => Some(KeyCode::ControlLeft),
            VoiceInputToggleKey::ControlRight => Some(KeyCode::ControlRight),
            VoiceInputToggleKey::SuperLeft => Some(KeyCode::SuperLeft),
            VoiceInputToggleKey::SuperRight => Some(KeyCode::SuperRight),
            VoiceInputToggleKey::ShiftLeft => Some(KeyCode::ShiftLeft),
            VoiceInputToggleKey::ShiftRight => Some(KeyCode::ShiftRight),
        }
    }

    /// Converts the voice input toggle key to a Keystroke representation.
    /// Since these are standalone modifier keys, we construct the Keystroke directly
    /// rather than using `parse()` (which always requires a non-modifier key to be included).
    pub fn keystroke(&self) -> Option<warpui::keymap::Keystroke> {
        use warpui::keymap::Keystroke;

        let keystroke = match self {
            VoiceInputToggleKey::None => return None,
            VoiceInputToggleKey::Fn => Keystroke {
                key: "fn".to_string(),
                ..Default::default()
            },
            VoiceInputToggleKey::AltLeft | VoiceInputToggleKey::AltRight => Keystroke {
                alt: true,
                ..Default::default()
            },
            VoiceInputToggleKey::ControlLeft | VoiceInputToggleKey::ControlRight => Keystroke {
                ctrl: true,
                ..Default::default()
            },
            VoiceInputToggleKey::SuperLeft | VoiceInputToggleKey::SuperRight => Keystroke {
                cmd: true,
                ..Default::default()
            },
            VoiceInputToggleKey::ShiftLeft | VoiceInputToggleKey::ShiftRight => Keystroke {
                shift: true,
                ..Default::default()
            },
        };
        Some(keystroke)
    }

    pub fn tooltip_message(&self) -> String {
        match self.keystroke() {
            Some(keystroke) => {
                let symbol = keystroke.displayed();
                let side = match self {
                    VoiceInputToggleKey::AltLeft
                    | VoiceInputToggleKey::ControlLeft
                    | VoiceInputToggleKey::SuperLeft
                    | VoiceInputToggleKey::ShiftLeft => Some("Left"),
                    VoiceInputToggleKey::AltRight
                    | VoiceInputToggleKey::ControlRight
                    | VoiceInputToggleKey::SuperRight
                    | VoiceInputToggleKey::ShiftRight => Some("Right"),
                    VoiceInputToggleKey::None | VoiceInputToggleKey::Fn => None,
                };
                let key_name = match side {
                    Some(side) => format!("{side} {symbol}"),
                    None => symbol,
                };
                format!("Voice input (hold {key_name} key)")
            }
            None => "Voice input".to_string(),
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, VoiceInputToggleKey::None)
    }
}

/// The default mode for new terminal sessions.
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    EnumIter,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Default mode for new sessions.",
    rename_all = "snake_case"
)]
pub enum DefaultSessionMode {
    /// New sessions start in the terminal mode (default).
    #[default]
    Terminal,
    /// New sessions start in agent view.
    Agent,
    /// New sessions start in cloud (ambient) agent mode.
    CloudAgent,
    /// New sessions open a user-defined tab config.
    /// The specific config is identified by the companion `default_tab_config_path` setting.
    TabConfig,
    /// New sessions open in a local Docker sandbox.
    /// Requires the `LocalDockerSandbox` feature flag; falls back to `Terminal` when disabled.
    DockerSandbox,
}

settings::macros::implement_setting_for_enum!(
    DefaultSessionMode,
    AISettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "general.default_session_mode",
    description: "The default mode for new terminal sessions.",
);

impl DefaultSessionMode {
    /// Display name for the settings dropdown.
    pub fn display_name(&self) -> &'static str {
        match self {
            DefaultSessionMode::Terminal => "Terminal",
            DefaultSessionMode::Agent => "Agent",
            DefaultSessionMode::CloudAgent => "Cloud Oz",
            DefaultSessionMode::TabConfig => "Tab Config",
            DefaultSessionMode::DockerSandbox => "Local Docker Sandbox",
        }
    }
}

/// Controls how agent thinking/reasoning traces are displayed after streaming.
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    EnumIter,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Controls how agent thinking is displayed after streaming.",
    rename_all = "snake_case"
)]
pub enum ThinkingDisplayMode {
    /// Show reasoning blocks while streaming, then collapse them when complete (default).
    #[default]
    ShowAndCollapse,
    /// Always keep reasoning blocks expanded, even after streaming finishes.
    AlwaysShow,
    /// Never show reasoning blocks.
    NeverShow,
}

settings::macros::implement_setting_for_enum!(
    ThinkingDisplayMode,
    AISettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "agents.warp_agent.other.thinking_display_mode",
    description: "Controls how agent thinking traces are displayed after streaming.",
);

impl ThinkingDisplayMode {
    /// Display name for the settings dropdown.
    pub fn display_name(&self) -> &'static str {
        match self {
            ThinkingDisplayMode::ShowAndCollapse => "Show & collapse",
            ThinkingDisplayMode::AlwaysShow => "Always show",
            ThinkingDisplayMode::NeverShow => "Never show",
        }
    }

    pub fn command_palette_description(&self) -> String {
        match self {
            ThinkingDisplayMode::ShowAndCollapse => {
                crate::t!("agent-thinking-display-show-collapse")
            }
            ThinkingDisplayMode::AlwaysShow => crate::t!("agent-thinking-display-always-show"),
            ThinkingDisplayMode::NeverShow => crate::t!("agent-thinking-display-never-show"),
        }
    }

    pub fn should_render(&self) -> bool {
        !matches!(self, ThinkingDisplayMode::NeverShow)
    }

    pub fn should_keep_expanded(&self) -> bool {
        matches!(self, ThinkingDisplayMode::AlwaysShow)
    }
}

/// Tracks the state of the quota reset banner
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    PartialEq,
    Default,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "State of the quota reset banner.")]
pub struct BannerState {
    #[serde(default)]
    #[schemars(description = "Whether the banner has been dismissed.")]
    pub dismissed: bool,
}

/// Tracks information about a single billing cycle for AI request usage
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    PartialEq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "Information about a single billing cycle.")]
pub struct CycleInfo {
    /// End date of the billing cycle
    #[schemars(description = "End date of the billing cycle.")]
    pub end_date: DateTime<Utc>,
    /// Whether the quota was exceeded in this cycle
    #[schemars(description = "Whether the usage quota was exceeded in this cycle.")]
    pub was_quota_exceeded: bool,
    /// State of the quota reset banner
    #[schemars(description = "State of the quota reset banner for this cycle.")]
    pub banner_state: BannerState,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Default,
    PartialEq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "AI usage quota information across billing cycles.")]
pub struct AIRequestQuotaInfo {
    /// History of billing cycles and their usage.
    ///
    /// Note that these are only populated going forward from when this setting
    /// was introduced.
    #[schemars(description = "History of billing cycles and their quota usage.")]
    pub cycle_history: Vec<CycleInfo>,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Default,
    PartialEq,
    EnumIter,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "File read permission level for the agent.",
    rename_all = "snake_case"
)]
pub enum AgentModeCodingPermissionsType {
    /// Agent Mode must ask for explicit permission for any type of file read.
    #[default]
    AlwaysAskBeforeReading,
    /// Agent Mode can always read files without explicit consent.
    AlwaysAllowReading,
    /// Agent Mode can only read certain files without explicit consent.
    ///
    /// The specific filepaths are backed by the
    /// [`AISettings::agent_mode_coding_file_read_allowlist`] setting.
    AllowReadingSpecificFiles,
}

/// Predicate types to match commands that can be executed by Agent Mode.
#[derive(Debug, Serialize, Deserialize, Clone)]
enum AgentModeCommandExecutionPredicateType {
    /// A regex with start (`^`) and end (`$`) anchors.
    ///
    /// We want regex rules to apply to the entire cmd string so we anchor them
    /// (there isn't any efficient way to apply to the entire cmd string at match-time).
    #[serde(with = "serde_regex")]
    AnchoredRegex(Regex),
}

impl AgentModeCommandExecutionPredicateType {
    fn new_regex(regex: &str) -> Result<Self, regex::Error> {
        // Redundant anchors aren't a problem so we can unconditionally add them.
        let anchored_regex = Regex::new(&format!("^{regex}$"))?;
        Ok(Self::AnchoredRegex(anchored_regex))
    }

    fn matches(&self, cmd: &str) -> bool {
        match self {
            Self::AnchoredRegex(regex) => regex.is_match(cmd),
        }
    }
}

impl PartialEq for AgentModeCommandExecutionPredicateType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AnchoredRegex(a), Self::AnchoredRegex(b)) => {
                // Indexing should be safe since they're guaranteed to have at least
                // the anchors around them.
                let a_unanchored = &a.as_str()[1..a.as_str().len() - 1];
                let b_unanchored = &b.as_str()[1..b.as_str().len() - 1];
                a_unanchored == b_unanchored
            }
        }
    }
}

impl std::fmt::Display for AgentModeCommandExecutionPredicateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AnchoredRegex(regex) => {
                write!(f, "{}", &regex.as_str()[1..regex.as_str().len() - 1])
            }
        }
    }
}

/// A wrapper around [`AgentModeCommandExecutionPredicateType`] to enforce
/// the use of the provided constructors rather than direct construction of the variants.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(transparent)]
pub struct AgentModeCommandExecutionPredicate(AgentModeCommandExecutionPredicateType);

impl schemars::JsonSchema for AgentModeCommandExecutionPredicate {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("AgentModeCommandExecutionPredicate")
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        // In the settings file, predicates are serialized as plain regex strings.
        gen.subschema_for::<String>()
    }
}

impl AgentModeCommandExecutionPredicate {
    pub fn new_regex(regex: &str) -> Result<Self, regex::Error> {
        Ok(Self(AgentModeCommandExecutionPredicateType::new_regex(
            regex,
        )?))
    }

    pub fn matches(&self, cmd: &str) -> bool {
        self.0.matches(cmd)
    }
}

impl std::fmt::Display for AgentModeCommandExecutionPredicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl settings_value::SettingsValue for AgentModeCommandExecutionPredicate {
    fn to_file_value(&self) -> serde_json::Value {
        serde_json::Value::String(self.to_string())
    }

    fn from_file_value(value: &serde_json::Value) -> Option<Self> {
        value.as_str().and_then(|s| Self::new_regex(s).ok())
    }
}

lazy_static! {
    // Matches optional args / options for a top-level command.
    static ref OPTIONAL_ARGS_REGEX: Regex = Regex::new(r"(\s.*)?").expect("Can parse optional args regex");
}

cfg_if! {
    // Compiling the regexes for the default command execution allowlist/denylist can be slow
    // in an unoptimized build, so we use empty lists in unit tests.
    if #[cfg(test)] {
        lazy_static! {
            pub static ref DEFAULT_COMMAND_EXECUTION_ALLOWLIST: Vec<AgentModeCommandExecutionPredicate> = vec![];
            pub static ref DEFAULT_COMMAND_EXECUTION_DENYLIST: Vec<AgentModeCommandExecutionPredicate> = vec![];
        }
    } else {
        lazy_static! {
            pub static ref DEFAULT_COMMAND_EXECUTION_ALLOWLIST: Vec<AgentModeCommandExecutionPredicate> = vec![
                AgentModeCommandExecutionPredicate::new_regex(&format!("cat{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default cat rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("echo{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default echo rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex("find .*").expect("Can parse default find rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("grep{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default grep rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("ls{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default ls rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex("which .*").expect("Can parse default which rule into regex"),
            ];

            pub static ref DEFAULT_COMMAND_EXECUTION_DENYLIST: Vec<AgentModeCommandExecutionPredicate> = vec![
                AgentModeCommandExecutionPredicate::new_regex(&format!("bash{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default bash rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("fish{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default fish rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("pwsh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default pwsh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("sh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default sh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("zsh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default zsh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("curl{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default curl rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("eval{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default eval rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("exec{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default exec rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("source{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default source rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("wget{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default wget rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("dig{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default dig rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("nslookup{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default nslookup rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("host{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default host rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("ssh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default ssh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("scp{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default scp rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("rsync{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default rsync rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("telnet{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default telnet rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("rm{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default rm rule into regex"),
            ];
        }
    }
}

/// Maps custom toolbar command regex patterns to CLI agent names.
// ---------------------------------------------------------------------------
// 自定义 Agent 提供商配置(进程内 Provider)
// ---------------------------------------------------------------------------

/// Agent 提供商支持的协议类型。
///
/// 第一阶段仅支持 OpenAI 兼容协议(适用于 OpenAI、DeepSeek、智谱 GLM、
/// Moonshot、通义千问 DashScope-OpenAI 兼容端点、SiliconFlow、OpenRouter、
/// 任何 OpenAI 兼容的本地服务等)。后续可在此扩展 Anthropic、Google、Bedrock。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentProviderKind {
    /// OpenAI 兼容的 Chat Completions / `/v1/models` 协议。
    OpenAiCompatible,
}

impl Default for AgentProviderKind {
    fn default() -> Self {
        Self::OpenAiCompatible
    }
}

/// BYOP provider 实际使用的 API 协议类型 — 显式指定,
/// 由 chat_stream 通过 genai `ServiceTargetResolver` 一对一映射到对应的
/// `AdapterKind`,完全绕过"按模型名识别"的默认行为,避免误识别。
///
/// **注意**:这是相对 [`AgentProviderKind`] 的更细粒度维度。
/// `AgentProviderKind` 目前只有 `OpenAiCompatible`(语义"用户自管 endpoint"),
/// `AgentProviderApiType` 决定 genai 用哪种原生协议序列化请求 / 解析响应。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AgentProviderApiType {
    /// OpenAI Chat Completions(`POST /v1/chat/completions`)。
    /// 适用于:OpenAI 官方、DeepSeek、SiliconFlow、OpenRouter、智谱 GLM、
    /// Moonshot、DashScope-OpenAI 兼容、本地 vLLM/llama.cpp 等。
    OpenAi,
    /// OpenAI Responses API(`POST /v1/responses`)。
    /// 适用于:GPT-5 / Codex / Pro 等较新模型。
    OpenAiResp,
    /// Google Gemini 原生协议(generativelanguage.googleapis.com)。
    Gemini,
    /// Anthropic Messages API 原生协议(api.anthropic.com)。
    Anthropic,
    /// Ollama 原生协议(本地或自建 Ollama)。
    Ollama,
    /// DeepSeek 原生协议。与 OpenAI 兼容相比:多轮 thinking 模式必须把
    /// `reasoning_content` 字段带回服务端(否则 400),仅 genai DeepSeek
    /// adapter 处理这个非标字段。`deepseek-reasoner / deepseek-v4-flash` 等
    /// thinking-mode 模型必须选这个类型,普通 chat 模型(`deepseek-chat`)
    /// 选 OpenAI 也可以工作。
    DeepSeek,
}

impl Default for AgentProviderApiType {
    fn default() -> Self {
        Self::OpenAi
    }
}

/// Provider 级别的 reasoning effort(思考深度)偏好。
///
/// 语义说明:
/// - `Auto`(默认):不向 genai 传 effort。OpenAI / Anthropic adapter 内部会按
///   模型名后缀(`-low` / `-high` / `-zero` 等)自动推断;Gemini / DeepSeek 不推断。
/// - `Off`:对支持 reasoning 的模型显式发送 `none`,关闭思考链。
/// - 其他档位:client 端先用 `reasoning::model_supports_reasoning` 判定,**仅在该
///   模型支持时**注入,避免向 claude-3-5-haiku / gpt-4o / gemini-1.5-pro 等老模型
///   注入 thinking 参数被上游 400 拒绝。
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffortSetting {
    #[default]
    Auto,
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ReasoningEffortSetting {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Off => "Off",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "XHigh",
            Self::Max => "Max",
        }
    }

    /// 转成 genai `ReasoningEffort`。`Auto` 返回 None(调用方不要 set)。
    pub fn to_genai(self) -> Option<genai::chat::ReasoningEffort> {
        use genai::chat::ReasoningEffort as GE;
        Some(match self {
            Self::Auto => return None,
            Self::Off => GE::None,
            Self::Minimal => GE::Minimal,
            Self::Low => GE::Low,
            Self::Medium => GE::Medium,
            Self::High => GE::High,
            Self::XHigh => GE::XHigh,
            Self::Max => GE::Max,
        })
    }
}

impl AgentProviderApiType {
    /// 设置 UI dropdown 显示文字。
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::OpenAiResp => "OpenAI-Response",
            Self::Gemini => "Gemini",
            Self::Anthropic => "Anthropic",
            Self::Ollama => "Ollama",
            Self::DeepSeek => "DeepSeek",
        }
    }

    /// 反向解析 Debug 格式名(`OpenAi` / `DeepSeek` 等),用于 BYOPLastUsedReasoningMap
    /// 这种 `<api_type>:<model_id>` 复合 key 的 hydrate 场景。未知字符串返回 None。
    pub fn from_debug_str(s: &str) -> Option<Self> {
        Some(match s {
            "OpenAi" => Self::OpenAi,
            "OpenAiResp" => Self::OpenAiResp,
            "Gemini" => Self::Gemini,
            "Anthropic" => Self::Anthropic,
            "Ollama" => Self::Ollama,
            "DeepSeek" => Self::DeepSeek,
            _ => return None,
        })
    }

    /// 当用户没填 base_url 时使用的默认 endpoint。新建 provider / 切换 ApiType
    /// 时,UI 可调用此方法预填,便于新手。
    ///
    /// **必须以 `/` 结尾**:genai 0.6.x 的 adapter 内部用 `format!("{base_url}messages")` /
    /// `Url::join` 拼接 service path,缺尾随 `/` 会拼出乱地址(Anthropic 是 `.devmessages` 直接连)
    /// 或被 `Url::join` 吃掉 path 最后一段。client 端 `build_client` 也会兜底补 `/`,
    /// 这里依然要求显式尾随 `/`,保证 UI 预填值落到 settings.toml 后即使绕过 client 兜底也是对的。
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1/",
            Self::OpenAiResp => "https://api.openai.com/v1/",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/",
            Self::Anthropic => "https://api.anthropic.com/v1/",
            Self::Ollama => "http://localhost:11434/v1/",
            Self::DeepSeek => "https://api.deepseek.com/v1/",
        }
    }
}

/// 一条用户自定义的 Agent 提供商配置。
///
/// `api_key` **不**保存在这里,而是保存在 `AgentProviderSecrets` 单例(secure storage),
/// 通过 `id` 关联。这样设置文件 (settings.toml) 不会泄漏敏感信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AgentProvider {
    /// 提供商唯一 ID,首次创建时生成,持久化到设置中作为 secret 的关联键。
    #[serde(default = "AgentProvider::default_id")]
    pub id: String,

    /// 用户给这个提供商起的显示名(例如 "DeepSeek 官方"、"本地 Ollama")。
    pub name: String,

    /// 协议类型,目前固定为 OpenAI 兼容(语义"用户自管 endpoint")。
    /// 实际请求/响应序列化协议由 [`AgentProvider::api_type`] 决定。
    #[serde(default)]
    pub kind: AgentProviderKind,

    /// 显式指定的 API 协议类型(OpenAI / OpenAI-Response / Gemini / Anthropic / Ollama)。
    /// 老配置(无此字段)反序列化为 `OpenAi` 兼容老语义。
    #[serde(default)]
    pub api_type: AgentProviderApiType,

    /// API base URL,例如 `https://api.deepseek.com/v1`、`http://localhost:11434/v1`。
    /// 不要带尾随斜杠,但代码侧会做容错。
    pub base_url: String,

    /// 用户配置的、希望暴露给 Agent 选择的模型列表。
    /// 每条同时含 `name`(显示名) 与 `id`(发送给上游 API 的 model 字段值)。
    #[serde(default)]
    pub models: Vec<AgentProviderModel>,

    /// 附加 HTTP 请求头,发给上游 provider 时逐条 merge 进请求。
    /// 用于需要额外路由头的 gateway(如 Portkey 的 `x-portkey-provider`)。
    /// `api_key` 仍走 `Authorization: Bearer` 标准路径。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_headers: Vec<(String, String)>,
}

impl AgentProvider {
    fn default_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// 构造一个新的、空的提供商。
    pub fn new_empty() -> Self {
        Self {
            id: Self::default_id(),
            name: String::new(),
            kind: AgentProviderKind::default(),
            api_type: AgentProviderApiType::default(),
            base_url: String::new(),
            models: Vec::new(),
            extra_headers: Vec::new(),
        }
    }
}

impl settings_value::SettingsValue for AgentProvider {}

/// 单条模型条目: `name` 是用户在 model picker 中看到的显示名,
/// `id` 是真正发给上游 OpenAI 兼容 API 的 `model` 字段值。
///
/// 序列化为 toml 时形如:
/// ```toml
/// [[agent_providers.models]]
/// name = "DS V3 通用"
/// id   = "deepseek-chat"
/// ```
///
/// 反序列化兼容老格式 `models = ["deepseek-chat", "deepseek-coder"]`
/// (每个字符串视为 `{ name = id, id = id }`),便于现有用户无痛升级。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct AgentProviderModel {
    pub name: String,
    pub id: String,

    /// 上下文窗口(tokens)。来源:用户填或 models.dev 自动带入。
    /// 0 表示未知 — chat_stream 退化到不做主动截断,完全交给上游服务报错。
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub context_window: u32,

    /// 单次最大输出 tokens。0 表示未指定。
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub max_output_tokens: u32,

    /// 是否支持 reasoning(思考/CoT)输出。
    #[serde(default, skip_serializing_if = "is_false")]
    pub reasoning: bool,

    /// 是否支持 function/tool calling。
    /// 默认 `true` — 老配置升级 + 用户手填新 model 时不要默认禁工具,
    /// 不支持工具调用的模型由 models.dev 数据带入显式 false。
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub tool_call: bool,

    // ----- 多模态附件 capability,三态语义:
    // - `None`(toml 字段缺省)= Auto: 运行时按 models.dev catalog → substring fallback 推断
    // - `Some(true)` = Force-On: 用户强制开,绕过推断
    // - `Some(false)` = Force-Off: 用户强制关
    //
    // 字段命名故意用 `image` 而非 `vision`,跟 models.dev `modalities.input: ["image"]`
    // 字面对应,语义最窄不歧义(避免用户误以为 vision = image+pdf+...)。
    /// 是否支持图像输入(image/* MIME)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<bool>,
    /// 是否支持 PDF 文档输入(application/pdf)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf: Option<bool>,
    /// 是否支持音频输入(audio/* MIME)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn is_true(v: &bool) -> bool {
    *v
}
fn default_true() -> bool {
    true
}

impl AgentProviderModel {
    pub fn from_id(id: String) -> Self {
        Self {
            name: id.clone(),
            id,
            context_window: 0,
            max_output_tokens: 0,
            reasoning: false,
            tool_call: true,
            image: None,
            pdf: None,
            audio: None,
        }
    }
}

impl<'de> Deserialize<'de> for AgentProviderModel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Plain(String),
            Full {
                #[serde(default)]
                name: String,
                id: String,
                #[serde(default)]
                context_window: u32,
                #[serde(default)]
                max_output_tokens: u32,
                #[serde(default)]
                reasoning: bool,
                #[serde(default = "default_true")]
                tool_call: bool,
                #[serde(default)]
                image: Option<bool>,
                #[serde(default)]
                pdf: Option<bool>,
                #[serde(default)]
                audio: Option<bool>,
            },
        }
        match Either::deserialize(deserializer)? {
            Either::Plain(id) => Ok(AgentProviderModel::from_id(id)),
            Either::Full {
                name,
                id,
                context_window,
                max_output_tokens,
                reasoning,
                tool_call,
                image,
                pdf,
                audio,
            } => {
                let name = if name.is_empty() { id.clone() } else { name };
                Ok(AgentProviderModel {
                    name,
                    id,
                    context_window,
                    max_output_tokens,
                    reasoning,
                    tool_call,
                    image,
                    pdf,
                    audio,
                })
            }
        }
    }
}

impl settings_value::SettingsValue for AgentProviderModel {}

/// Keys are regex patterns (insertion-ordered), values are serialized CLIAgent names (e.g. "Claude").
/// An empty string value means "Any CLI Agent" (CLIAgent::Unknown).
///
/// Uses `IndexMap` to preserve insertion order so the settings UI list is deterministic.
/// Supports backward-compatible deserialization from the legacy `Vec<String>` format,
/// where each string is converted to a key with an empty agent value.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ToolbarCommandMap(IndexMap<String, String>);

impl ToolbarCommandMap {
    pub(crate) fn new(map: IndexMap<String, String>) -> Self {
        Self(map)
    }
}

impl<'de> Deserialize<'de> for ToolbarCommandMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum MapOrVec {
            Map(IndexMap<String, String>),
            Vec(Vec<String>),
        }

        match MapOrVec::deserialize(deserializer) {
            Ok(MapOrVec::Map(map)) => Ok(ToolbarCommandMap::new(map)),
            Ok(MapOrVec::Vec(vec)) => {
                let map = vec
                    .into_iter()
                    .map(|pattern| (pattern, String::new()))
                    .collect();
                Ok(ToolbarCommandMap::new(map))
            }
            Err(e) => Err(e),
        }
    }
}

impl schemars::JsonSchema for ToolbarCommandMap {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ToolbarCommandMap")
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        gen.subschema_for::<HashMap<String, String>>()
    }
}

impl std::ops::Deref for ToolbarCommandMap {
    type Target = IndexMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl settings_value::SettingsValue for ToolbarCommandMap {
    fn to_file_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or_default()
    }

    fn from_file_value(value: &serde_json::Value) -> Option<Self> {
        // Try map format first (using from_value to preserve insertion order), then legacy array format.
        if value.is_object() {
            if let Ok(map) = serde_json::from_value::<IndexMap<String, String>>(value.clone()) {
                return Some(ToolbarCommandMap::new(map));
            }
        }
        if let Some(arr) = value.as_array() {
            let result: IndexMap<String, String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| (s.to_string(), String::new())))
                .collect();
            return Some(ToolbarCommandMap::new(result));
        }
        None
    }
}

/// 持久化记忆"上次某 (api_type, model) 用过的 reasoning effort 档位"。
/// key 形式:`<api_type>:<model_id>`,例如 `DeepSeek:deepseek-v4-pro`。
/// value 是 `ReasoningEffortSetting` 枚举(serde_json snake_case)。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BYOPLastUsedReasoningMap(pub IndexMap<String, ReasoningEffortSetting>);

impl BYOPLastUsedReasoningMap {
    pub fn new(map: IndexMap<String, ReasoningEffortSetting>) -> Self {
        Self(map)
    }

    /// 拼 key:`<api_type>:<model_id>`。api_type 用 Debug 拼出 `DeepSeek` 等驼峰名,
    /// 跟 ReasoningEffortSetting 的 serde 形式无关。
    pub fn make_key(api_type: AgentProviderApiType, model_id: &str) -> String {
        format!("{api_type:?}:{model_id}")
    }
}

impl std::ops::Deref for BYOPLastUsedReasoningMap {
    type Target = IndexMap<String, ReasoningEffortSetting>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl schemars::JsonSchema for BYOPLastUsedReasoningMap {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "BYOPLastUsedReasoningMap".into()
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        gen.subschema_for::<HashMap<String, String>>()
    }
}

impl settings_value::SettingsValue for BYOPLastUsedReasoningMap {
    fn to_file_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or_default()
    }

    fn from_file_value(value: &serde_json::Value) -> Option<Self> {
        if value.is_object() {
            if let Ok(map) =
                serde_json::from_value::<IndexMap<String, ReasoningEffortSetting>>(value.clone())
            {
                return Some(Self::new(map));
            }
        }
        None
    }
}

define_settings_group!(AISettings, settings: [
    // If `false`, all AI features are disabled.
    is_any_ai_enabled: IsAnyAIEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::No),
        private: false,
        toml_path: "agents.warp_agent.is_any_ai_enabled",
        description: "Controls whether all AI features are enabled.",
    },
    // This field should not be referenced directly to lookup active AI enablement -- use the
    // `is_active_ai_enabled()` getter.
    is_active_ai_enabled_internal: IsActiveAIEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::No),
        private: false,
        toml_path: "agents.warp_agent.active_ai.enabled",
        description: "Controls whether proactive AI features like suggestions are enabled.",
    },
    // This field should not be referenced directly to lookup autodetection enablement -- use the
    // `is_ai_autodetection_enabled()` getter.
    ai_autodetection_enabled_internal: AIAutoDetectionEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.input.ai_auto_detection_enabled",
        description: "Controls whether AI automatically detects natural language input.",
    },
    // This field should not be referenced directly -- use the
    // `is_nld_in_terminal_enabled()` getter.
    // Controls whether natural language detection is enabled in the terminal input.
    //
    // This is only used when `FeatureFlag::AgentView` is enabled.
    nld_in_terminal_enabled_internal: NLDInTerminalEnabled {
        // openWarp:NLD in terminal 默认开。HeuristicClassifier 命中 CJK / 自然语言时
        // 自动切到 AI 输入,这是 openWarp 中文用户能直接在终端写中文当 prompt 的前提。
        // 上游默认 false 是因为 cloud 路线下用户先进 AgentView 全屏,在 terminal mode
        // 不期望自动切换;openWarp 没有 cloud 全屏入口,terminal 即主输入区。
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.input.nld_in_terminal_enabled",
        description: "Controls whether natural language detection is enabled in the terminal input.",
    },
    autodetection_command_denylist: AICommandDenylist {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.input.ai_command_denylist",
        description: "Commands to exclude from AI natural language autodetection.",
    },
    // This field should not be referenced directly to lookup intelligent autosuggestion enablement
    // -- use the `is_intelligent_autosuggestions_enabled()` getter.
    intelligent_autosuggestions_enabled_internal: IntelligentAutosuggestionsEnabled {
        type: bool,
        default: true, // TODO(roland): revisit this when launched to stable
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.active_ai.intelligent_autosuggestions_enabled",
        description: "Controls whether AI-powered intelligent autosuggestions are enabled.",
    }
    // This field should not be referenced directly to lookup Prompt Suggestions
    // enablement -- use the `is_prompt_suggestions_enabled()` getter.
    // Note that AgentModeQuerySuggestionsEnabled is a legacy name (the feature was initially named Agent
    // Mode Query Suggestions), however, we do not want to change the name of the setting key to avoid
    // breaking existing user settings.
    prompt_suggestions_enabled_internal: AgentModeQuerySuggestionsEnabled {
        type: bool,
        default: true, // TODO(advait): revisit this when launched to stable
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.active_ai.agent_mode_query_suggestions_enabled",
        description: "Controls whether prompt suggestions are shown in agent mode.",
    }

    // This field should not be referenced directly to lookup Code Suggestions
    // enablement -- use the `is_code_suggestions_enabled()` getter.
    code_suggestions_enabled_internal: CodeSuggestionsEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.active_ai.code_suggestions_enabled",
        description: "Controls whether AI code suggestions are enabled.",
    }
    // This field should not be referenced directly to lookup natural language autosuggestions
    // enablement -- use the `is_natural_language_autosuggestions_enabled()` getter.
    // This feature refers to ghosted text for AI input queries.
    natural_language_autosuggestions_enabled_internal: NaturalLanguageAutosuggestionsEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.active_ai.natural_language_autosuggestions_enabled",
        description: "Controls whether ghosted text autosuggestions are shown for AI input queries.",
        feature_flag: FeatureFlag::PredictAMQueries,
    }
    // This field should not be referenced directly to lookup git operations AI autogen
    // enablement -- use the `is_git_operations_autogen_enabled()` getter.
    git_operations_autogen_enabled_internal: GitOperationsAutogenEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.active_ai.git_operations_autogen_enabled",
        description: "Controls whether AI auto-generates commit messages and PR title/body in the code review dialogs.",
    }
    // This field should not be referenced directly to lookup Rule Suggestions
    // enablement -- use the `is_rule_suggestions_enabled()` getter.
    rule_suggestions_enabled_internal: RuleSuggestionsEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.active_ai.rule_suggestions_enabled",
        description: "Controls whether the agent suggests rules to save after responses.",
        feature_flag: FeatureFlag::SuggestedRules,
    }
    // This field should not be referenced directly to lookup Voice AI enablement -- use the
    // `is_voice_input_enabled()` getter.
    voice_input_enabled_internal: VoiceInputEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.voice.voice_input_enabled",
        description: "Controls whether voice input is enabled for AI interactions.",
    },
    // The number of times the user has entered Agent Mode.
    // Not a user-visible setting. We model it so we can show the voice input new feature popup
    // the correct number of times.
    entered_agent_mode_num_times: EnteredAgentModeNumTimes {
        type: usize,
        default: 0,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    },
    // Whether or not the user has manually dismissed the voice input new feature popup.
    dismissed_voice_input_new_feature_popup: DismissedVoiceInputNewFeaturePopup {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    },
    // This field is used to store the key used for voice input toggling.
    // Note this is not the named key, but rather corresponds to the physical key.
    voice_input_toggle_key: VoiceInputToggleKey,
    // This is not a user-visible setting - it's merely a one-time flag to track if the user has
    // explicitly interacted with voice input. We use this to determine whether we should show a toast
    // to inform the user about voice input and auto-set the keybinding.
    explicitly_interacted_with_voice: ExplicitlyInteractedWithVoice {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        // Never sync to cloud to keep state separate across devices, since microphone access is per-device.
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },
    // Predicates that Agent Mode can use to decide if it can execute
    // a command without explicit user consent.
    //
    // Prefer [`BlocklistAIPermissions::can_autoexecute_command`] to
    // interpret this allowlist.
    agent_mode_command_execution_allowlist: AgentModeCommandExecutionAllowlist {
        type: Vec<AgentModeCommandExecutionPredicate>,
        default: DEFAULT_COMMAND_EXECUTION_ALLOWLIST.clone(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.profiles.agent_mode_command_execution_allowlist",
        description: "Commands that the agent can execute without explicit permission.",
    },
    // Predicates that Agent Mode can use to decide if a command must
    // be executed by the user.
    //
    // Prefer [`BlocklistAIPermissions::can_autoexecute_command`] to
    // interpret this denylist.
    agent_mode_command_execution_denylist: AgentModeCommandExecutionDenylist {
        type: Vec<AgentModeCommandExecutionPredicate>,
        default: DEFAULT_COMMAND_EXECUTION_DENYLIST.clone(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.profiles.agent_mode_command_execution_denylist",
        description: "Commands that the agent must always ask before executing.",
    },
    // Enabled iff Agent Mode can execute readonly commands without explicit user consent.
    //
    // Prefer [`BlocklistAIPermissions::can_autoexecute_command`] to
    // interpret this setting.
    agent_mode_execute_read_only_commands: AgentModeExecuteReadonlyCommands {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.profiles.agent_mode_execute_readonly_commands",
        description: "Whether the agent can auto-execute read-only commands without asking.",
    },
    // Determines coding permissions that Agent Mode has.
    // Note that if Agent Mode has permissions to execute readonly commands,
    // that automatically gives Agent Mode the ability to also _read_ files for coding
    // tasks, including codebase search.
    //
    // Prefer [`BlocklistAIPermissions::can_read_file`] to interpret this setting.
    agent_mode_coding_permissions: AgentModeCodingPermissions {
        type: AgentModeCodingPermissionsType,
        default: AgentModeCodingPermissionsType::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.profiles.agent_mode_coding_permissions",
        description: "The file read permission level for the agent.",
    }
    // Specific filepaths that Agent Mode can read without asking for additional permissions.
    // These should be persisted as absolute filepaths to avoid ambiguity.
    //
    // This is used in conjunction with [`AgentModeCodingPermissionsType::AllowReadingSpecificFiles`]
    // but modelled as a separate setting because it is not cloud-synced.
    //
    // Prefer [`BlocklistAIPermissions::can_read_file`] to interpret this setting.
    agent_mode_coding_file_read_allowlist: AgentModeCodingFileReadAllowlist {
        type: Vec<PathBuf>,
        default: vec![],
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.profiles.agent_mode_coding_file_read_allowlist",
        description: "File paths the agent can read without asking for permission.",
    }
    // Whether or not the profile-level command autoexecution speedbump has been shown.
    //
    // Not a user-visible setting - we model it as a setting so we can track how often
    // it's shown across devices.
    has_shown_agent_mode_profile_command_autoexecution_speedbump: HasShownAgentModeProfileCommandAutoexecutionSpeedbump {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }
    // Whether or not we should show the speedbump for auto-executing readonly cmds.
    //
    // Not a user-visible settings - we model it as a setting so we can track how often
    // it's shown across devices.
    should_show_agent_mode_autoexecute_readonly_commands_speedbump: ShouldShowAgentModeModelExecuteReadonlyCommandsSpeedbump {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }
    // Whether or not we should show the speedbump for auto-writing to the PTY.
    //
    // Not a user-visible settings - we model it as a setting so we can track how often
    // it's shown across devices.
    should_show_agent_mode_write_to_pty_speedbump: ShouldShowAgentModeWriteToPtySpeedbump {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }
    // Whether or not we should show the speedbump for auto-reading files.
    //
    // Not a user-visible settings - we model it as a setting so we can track how often
    // it's shown across devices.
    should_show_agent_mode_autoread_files_speedbump: ShouldShowAgentModeCodingReadPermissionsNudge {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }
    // Whether to use locally loaded AWS credentials for Bedrock-enabled requests.
    aws_bedrock_credentials_enabled: AwsBedrockCredentialsEnabled {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_credentials_enabled",
        description: "Whether Warp should use your local AWS credentials for Bedrock-enabled requests.",
    }
    // Whether to automatically run the AWS login command when Bedrock credentials are expired.
    //
    // When true, the configured login command will be run automatically without asking.
    // When false (default), a prompt will be shown asking for permission.
    aws_bedrock_auto_login: AwsBedrockAutoLogin {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_auto_login",
        description: "Whether to automatically run the AWS login command when Bedrock credentials expire.",
    }
    // Command to run to refresh AWS credentials when using Bedrock auto-login.
    aws_bedrock_auth_refresh_command: AwsBedrockAuthRefreshCommand {
        type: String,
        default: "aws login".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_auth_refresh_command",
        description: "The command to run to refresh AWS credentials for Bedrock.",
    }
    // AWS profile name to use when loading credentials from the local AWS credential/config chain.
    aws_bedrock_profile: AwsBedrockProfile {
        type: String,
        default: "default".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.aws_bedrock_profile",
        description: "The AWS profile name to use for Bedrock credentials.",
    }
    // Whether the AWS Bedrock login banner has been permanently dismissed.
    //
    // Not a user-visible setting - we model it as a setting so we can track state.
    aws_bedrock_login_banner_dismissed: AwsBedrockLoginBannerDismissed {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }
    // Whether or not the user wants agent mode requests to use their saved rules.
    memory_enabled: MemoryEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.knowledge.rules_enabled",
        description: "Whether the agent uses your saved rules during requests.",
    }
    // Whether warp drive context should be included in AI requests
    warp_drive_context_enabled: WarpDriveContextEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.knowledge.warp_drive_context_enabled",
        description: "Whether Warp Drive context is included in AI requests.",
    }

    // Whether the agent mode setup banner has been shown for a given repo path.
    // Once shown, it will not be shown again for that repo.
    //
    // Not a user-visible settings - we model it as a setting so we can track state.
    agent_mode_setup_banner_shown_for_repo_paths: AgentModeSetupBannerShownForRepoPaths {
        type: Vec<PathBuf>,
        default: vec![],
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    }

    // Information about AI request quotas and usage across billing cycles
    ai_request_quota_info: AIRequestQuotaInfoSetting {
        type: AIRequestQuotaInfo,
        default: AIRequestQuotaInfo::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    },

    // Whether or not we should show the speedbump for showing code suggestion banners.
    // This includes both passive code diffs and suggested prompts (passive unit tests).
    //
    // Not a user-visible settings - we model it as a setting so we can track if the speedbump has already been shown or not.
    show_code_suggestion_speedbump: ShouldShowCodeSuggestionSpeedbump {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }

    mcp_execution_path: MCPExecutionPath {
        type: Option<String>,
        default: None,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },

    // This is not a user-visible setting - its merely a one-time flag to track if the agents 3 launch modal
    // has been shown to the user.
    //
    // We model it as a setting so it's only shown once to a given user regardless of the number of
    // devices they use.
    did_check_to_trigger_agents_3_launch_modal: DidShowAgents3LaunchModal {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::No),
        private: true,
    }

    // Whether or not the user has enabled the ability to use Warp credits even when providing
    // their own LLM provider API key.
    can_use_warp_credits_with_byok: CanUseWarpCreditsWithByok {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cloud_platform.third_party_api_keys.can_use_warp_credits_with_byok",
        description: "Whether Warp credits can be used even when providing your own API key.",
    }

    should_render_use_agent_footer_for_user_commands: ShouldRenderUseAgentToolbarForUserCommands {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.other.should_render_use_agent_toolbar_for_user_commands",
        description: "Whether to show the \"Use Agent\" footer for terminal commands.",
    }

    // Whether to render the CLI agent footer for commands like Claude, Codex, Gemini, etc.
    // This is independent of the "Use Agent" footer setting.
    should_render_cli_agent_footer: ShouldRenderCLIAgentToolbar {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.should_render_cli_agent_toolbar",
        description: "Whether to show the CLI agent footer for coding agent commands.",
    }
    // When enabled and a CLI agent session has a plugin listener, rich input
    // auto-closes when the session enters a Blocked state (the agent requires
    // direct keyboard interaction) and auto-opens when it leaves Blocked.
    auto_toggle_rich_input: AutoToggleRichInput {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.auto_toggle_composer",
        description: "Whether CLI agent Rich Input automatically closes and reopens based on the agent's blocked state.",
    }

    // When enabled and a CLI agent session has a plugin listener, rich input
    // auto-opens once when the session starts or when the listener is registered.
    auto_open_rich_input_on_cli_agent_start: AutoOpenRichInputOnCLIAgentStart {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.auto_open_composer_on_cli_agent_start",
        description: "Whether CLI agent Rich Input automatically opens when a CLI agent session starts.",
    }

    // When enabled and a CLI agent session does NOT have a plugin listener,
    // rich input auto-closes after the user submits a prompt.
    // When the plugin IS present, this setting has no effect (auto-show/hide
    // from auto_toggle_rich_input handles rich input lifecycle).
    auto_dismiss_rich_input_after_submit: AutoDismissRichInputAfterSubmit {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.auto_dismiss_composer_after_submit",
        description: "Whether CLI agent Rich Input automatically closes after the user submits a prompt.",
    }

    // Maps custom toolbar command regex patterns to specific CLI agents.
    // Keys are regex patterns matched against the full command string.
    // Values are serialized CLIAgent names (empty string = any agent).
    // Supports migration from the legacy Vec<String> format.
    cli_agent_footer_enabled_commands: CLIAgentToolbarEnabledCommands {
        type: ToolbarCommandMap,
        default: ToolbarCommandMap::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.third_party.cli_agent_toolbar_enabled_commands",
        max_table_depth: 1,
        description: "Maps custom toolbar command patterns to specific CLI agents.",
    }

    // This is not a user-visible setting - it tracks whether a paid user has dismissed the
    // agent management help page by clicking "View Agents".
    //
    // When false and user is on a paid plan, the help page is shown.
    // When true, the help page is hidden (user dismissed it).
    // Free users never see the help page by default regardless of this setting.
    did_dismiss_cloud_setup_guide: DidDismissAgentManagementHelpPage {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }

    // This is not a user-visible setting - it tracks whether the FTU model picker callout
    // has been shown to the user. We set this to `true` as soon as the callout is first
    // displayed (not when it's dismissed), so it never re-appears.
    //
    // Note: this setting was originally named "dismissed" but we now use it to mean "shown".
    // We kept the same setting key so that users who already dismissed the callout on an
    // older client don't see it again.
    ftu_model_callout_dismissed: FtuModelCalloutDismissed {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }

    // Tracks whether we've done the one-time auto-open of the conversation list for discoverability.
    // Once set to true, the conversation list visibility will be restored from workspace state.
    has_auto_opened_conversation_list: HasAutoOpenedConversationList {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }

    // Whether the ambient agent trial widget has been dismissed by the user.
    //
    // Not a user-visible setting - we model it as a setting so we can track state.
    ambient_agent_trial_widget_dismissed: AmbientAgentTrialWidgetDismissed {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }

    // The raw stored default mode for new sessions. Use `default_session_mode()` to retrieve the
    // effective value, which is gated on AI availability.
    default_session_mode_internal: DefaultSessionMode,

    // The file path of the tab config used when default_session_mode_internal is TabConfig.
    // Only read when mode is TabConfig; ignored for all other modes.
    // Machine-local (tab config paths vary per machine), so never synced to cloud.
    default_tab_config_path: DefaultTabConfigPath {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "general.default_tab_config_path",
    }

    // Whether multi-agent orchestration is enabled. When enabled, the agent can
    // spawn and coordinate parallel sub-agents via StartAgent / SendMessageToAgent
    // tools. This setting is only effective when FeatureFlag::Orchestration is also
    // enabled.
    orchestration_enabled: OrchestrationEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.other.orchestration_enabled",
        description: "Whether multi-agent orchestration is enabled.",
        feature_flag: FeatureFlag::Orchestration,
    }

    // Whether file-based MCP servers from third-party AI tools (e.g. Claude, Codex) should
    // be automatically detected and spawned. Warp-native config files (.warp/.mcp.json) are
    // always detected and spawned, regardless of this setting.
    file_based_mcp_enabled: FileBasedMcpEnabled {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.mcp_servers.file_based_mcp_enabled",
        description: "Whether third-party file-based MCP servers are automatically detected.",
    }

    // Controls how agent thinking/reasoning traces are displayed.
    thinking_display_mode: ThinkingDisplayMode,

    // Whether agent-executed shell commands should be included in command history
    // (up-arrow, Ctrl-R search, inline history menu).
    // When false, commands run by the AI agent are excluded from history.
    include_agent_commands_in_history: IncludeAgentCommandsInHistory {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.input.include_agent_commands_in_history",
        description: "Whether agent-executed commands are included in command history.",
    }

    // Controls whether the conversation history view appears in the tools panel.
    show_conversation_history: ShowConversationHistory {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.other.show_conversation_history",
        description: "Whether conversation history appears in the tools panel.",
    }


    // Controls whether agent notifications (mailbox button, toasts, notification items) are shown.
    show_agent_notifications: ShowAgentNotifications {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.other.show_agent_notifications",
        description: "Whether agent notifications are shown.",
    }

    // OpenWarp T1-2:已完成工具卡默认隐藏(对齐 opencode TUI showDetails 行为)。
    // true → 默认隐藏 status.is_done() 的 RequestCommandOutput / ReadFiles /
    // Grep / FileGlob / RequestFileEdits 等卡片,只保留 in-progress + error,
    // 长 session 不被历史卡片堆积淹没新内容。folded 状态可由外观设置面板切换。
    hide_completed_tool_cards: HideCompletedToolCards {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.warp_agent.appearance.hide_completed_tool_cards",
        description: "When true, completed tool action cards (read files, grep, search codebase, requested commands, etc.) are hidden after they finish. In-progress and errored cards are always shown. Useful for long sessions to keep focus on the latest activity.",
    }

    // Per-agent, per-host tracking of whether the user dismissed the plugin install chip.
    // Keys are "<agent_prefix>" for local sessions or "<agent_prefix>@<host>" for remote.
    // Local-only so dismissal doesn't sync across devices.
    plugin_install_chip_dismissed_map: PluginInstallChipDismissedMap {
        type: HashMap<String, bool>,
        default: HashMap::default(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    }

    // Per-agent, per-host tracking of the MINIMUM_PLUGIN_VERSION for which the user
    // dismissed the plugin update chip. Empty/absent means not dismissed.
    // Keys are "<agent_prefix>" for local sessions or "<agent_prefix>@<host>" for remote.
    // Local-only so dismissal doesn't sync across devices.
    plugin_update_chip_dismissed_for_version_map: PluginUpdateChipDismissedForVersionMap {
        type: HashMap<String, String>,
        default: HashMap::default(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    }

    // 用户自定义 Agent 提供商列表。第一阶段仅支持 OpenAI 兼容协议。
    //
    // 注意: 提供商的 `api_key` 不在这里持久化,见 `AgentProviderSecrets`。
    agent_providers: AgentProviders {
        type: Vec<AgentProvider>,
        default: Vec::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.warp_agent.providers",
        description: "User-configured custom Agent providers (OpenAI-compatible).",
    }

    // OpenWarp BYOP 本地会话压缩 — 1:1 对齐 opencode `Config.compaction.auto`。
    // true 时按 token-overflow 自动触发摘要;false 仅手动 /compact /compact-and 触发。
    byop_compaction_auto: ByopCompactionAuto {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop_compaction.auto",
        description: "Enable BYOP automatic conversation compaction on context overflow.",
    }

    // OpenWarp BYOP 本地会话压缩 — 1:1 对齐 opencode `Config.compaction.prune`。
    // true 时每次 LLM 请求前清旧 tool output(替换为占位符)。
    byop_compaction_prune: ByopCompactionPrune {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop_compaction.prune",
        description: "Auto-prune older tool outputs to free BYOP context.",
    }

    // OpenWarp BYOP 本地会话压缩 — 1:1 对齐 opencode `Config.compaction.tail_turns`(默认 2)。
    // 保留最近 N 个 user turn 作 tail,前面的进入 head 给摘要 LLM。0 关闭压缩。
    byop_compaction_tail_turns: ByopCompactionTailTurns {
        type: u32,
        default: 2u32,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop_compaction.tail_turns",
        description: "Number of recent user turns to keep verbatim during compaction.",
    }

    // OpenWarp BYOP 本地会话压缩 — 1:1 对齐 `Config.compaction.preserve_recent_tokens`。
    // 0 = 自动按公式算(min(MAX=8000, max(MIN=2000, usable * 0.25)));> 0 强制覆盖。
    byop_compaction_preserve_recent_tokens: ByopCompactionPreserveRecentTokens {
        type: u32,
        default: 0u32,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop_compaction.preserve_recent_tokens",
        description: "Override the recent-tokens preservation budget (0 = auto).",
    }

    // OpenWarp BYOP 本地会话压缩 — 1:1 对齐 `Config.compaction.reserved`。
    // overflow 判定时 usable = input_limit - reserved。0 = 自动按 min(20_000, max_output) 算。
    byop_compaction_reserved: ByopCompactionReserved {
        type: u32,
        default: 0u32,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop_compaction.reserved",
        description: "Reserved buffer tokens for compaction overflow check (0 = auto).",
    }

    // OpenWarp BYOP 本地会话压缩 — 摘要专用模型(可选)。
    // 设置后:摘要 LLM 调用走这个 provider+model 而非当前 conversation 模型。
    // 留空两个字段 = 用 conversation 当前模型。
    byop_compaction_model_provider_id: ByopCompactionModelProviderId {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop_compaction.model.provider_id",
        description: "Optional dedicated provider id for compaction LLM calls.",
    }

    byop_compaction_model_id: ByopCompactionModelId {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop_compaction.model.model_id",
        description: "Optional dedicated model id for compaction LLM calls.",
    }

    // OpenWarp BYOP 模型 + 思考深度持久化(picker 切换后立即写入,新 tab/重启沿用)。
    // 模型用 LLMId 字符串形式;空串 = 没有 last_used,落回 profile 默认。
    byop_last_used_model_id: ByopLastUsedModelId {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop.last_used_model_id",
        description: "Last selected BYOP model id (picker hydrates new tabs/sessions from this).",
    }

    // OpenWarp BYOP per-(api_type, model) 思考深度记忆。
    // key = `<api_type>:<model_id>`,value = ReasoningEffortSetting。picker 切换写入。
    byop_last_used_reasoning: ByopLastUsedReasoning {
        type: BYOPLastUsedReasoningMap,
        default: BYOPLastUsedReasoningMap::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "agents.byop.last_used_reasoning",
        max_table_depth: 1,
        description: "Per-(api_type, model) reasoning effort memory for BYOP picker.",
    }
]);

impl AISettings {
    pub fn register_and_subscribe_to_events(app: &mut AppContext) {
        Self::register(app);
        app.add_singleton_model(FocusedTerminalInfo::new);
        CompiledCommandsForCodingAgentToolbar::register(app);

        app.update_model(&Self::handle(app), |_me, ctx| {
            ctx.subscribe_to_model(&FocusedTerminalInfo::handle(ctx), |_me, event, ctx| {
                if matches!(event, FocusedTerminalInfoEvent::TerminalInfoUpdated) {
                    // Pipe the event so that any view that listens for settings changes will be notified.
                    ctx.emit(AISettingsChangedEvent::IsAnyAIEnabled {
                        change_event_reason: ChangeEventReason::LocalChange,
                    });
                }
            });
        });
    }

    pub fn is_ai_disabled_due_to_remote_session_org_policy(&self, app: &AppContext) -> bool {
        let contains_remote_blocks = FocusedTerminalInfo::as_ref(app).contains_any_remote_blocks();

        let contains_restored_remote_blocks =
            FocusedTerminalInfo::as_ref(app).contains_any_restored_remote_blocks();

        let is_ai_allowed_in_remote_sessions =
            UserWorkspaces::as_ref(app).is_ai_allowed_in_remote_sessions();

        if is_ai_allowed_in_remote_sessions {
            return false;
        }

        contains_remote_blocks || contains_restored_remote_blocks
    }

    pub fn is_any_ai_enabled(&self, app: &AppContext) -> bool {
        // Disable AI for anonymous and logged-out users.
        let is_anonymous_or_logged_out = AuthStateProvider::as_ref(app)
            .get()
            .is_anonymous_or_logged_out();

        *self.is_any_ai_enabled
            && !is_anonymous_or_logged_out
            && !self.is_ai_disabled_due_to_remote_session_org_policy(app)
    }

    pub fn default_session_mode(&self, app: &AppContext) -> DefaultSessionMode {
        let mode = *self.default_session_mode_internal.value();
        match mode {
            // Terminal and TabConfig don't require AI.
            DefaultSessionMode::Terminal | DefaultSessionMode::TabConfig => mode,
            // Agent and CloudAgent require AI to be enabled.
            DefaultSessionMode::Agent | DefaultSessionMode::CloudAgent => {
                if self.is_any_ai_enabled(app) {
                    mode
                } else {
                    DefaultSessionMode::Terminal
                }
            }
            // DockerSandbox is gated on its feature flag; fall back to Terminal
            // when disabled so a stale stored value doesn't wedge the user.
            DefaultSessionMode::DockerSandbox => {
                if FeatureFlag::LocalDockerSandbox.is_enabled() {
                    mode
                } else {
                    DefaultSessionMode::Terminal
                }
            }
        }
    }

    /// Returns the stored default tab config path (only meaningful when mode is `TabConfig`).
    pub fn default_tab_config_path(&self) -> &str {
        &self.default_tab_config_path
    }

    /// Looks up the `TabConfig` matching the stored `default_tab_config_path`.
    /// Returns `None` if the path is empty or no loaded config matches.
    pub fn resolved_default_tab_config(
        &self,
        app: &AppContext,
    ) -> Option<crate::tab_configs::TabConfig> {
        let path_str = self.default_tab_config_path.as_str();
        if path_str.is_empty() {
            return None;
        }
        let path = std::path::Path::new(path_str);
        crate::user_config::WarpConfig::as_ref(app)
            .tab_configs()
            .iter()
            .find(|config| config.source_path.as_deref().is_some_and(|p| p == path))
            .cloned()
    }

    pub fn is_active_ai_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_any_ai_enabled(app)
            && *self.is_active_ai_enabled_internal
            && AppExecutionMode::as_ref(app).allows_active_ai()
    }

    pub fn is_prompt_suggestions_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_active_ai_enabled(app) && *self.prompt_suggestions_enabled_internal
    }

    pub fn is_rule_suggestions_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_active_ai_enabled(app) && *self.rule_suggestions_enabled_internal
    }

    pub fn is_code_suggestions_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_active_ai_enabled(app) && *self.code_suggestions_enabled_internal
    }

    pub fn is_natural_language_autosuggestions_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_active_ai_enabled(app) && *self.natural_language_autosuggestions_enabled_internal
    }

    pub fn is_git_operations_autogen_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_active_ai_enabled(app) && *self.git_operations_autogen_enabled_internal
    }

    pub fn is_intelligent_autosuggestions_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_active_ai_enabled(app) && *self.intelligent_autosuggestions_enabled_internal
    }

    pub fn is_voice_input_enabled(&self, app: &warpui::AppContext) -> bool {
        // Voice input is conditionally-compiled because it requires additional dependencies on some platforms.
        cfg!(feature = "voice_input")
            && self.is_any_ai_enabled(app)
            && *self.voice_input_enabled_internal
    }

    /// Returns `true` if input autodetection is enabled.
    ///
    /// If `FeatureFlag::AgentView` is enabled, this specifically gates NLD enablement in the agent
    /// view only.
    pub fn is_ai_autodetection_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_any_ai_enabled(app) && *self.ai_autodetection_enabled_internal
    }

    /// Returns `true` if NLD is enabled in the terminal.
    ///
    /// This is only used when `FeatureFlag::AgentView` is enabled.
    /// If the user has not explicitly set this setting, it defaults to the value of
    /// `ai_autodetection_enabled_internal`.
    pub fn is_nld_in_terminal_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_any_ai_enabled(app) && *self.nld_in_terminal_enabled_internal
    }

    pub fn is_memory_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_any_ai_enabled(app) && *self.memory_enabled
    }

    pub fn is_warp_drive_context_enabled(&self, app: &warpui::AppContext) -> bool {
        self.is_any_ai_enabled(app) && *self.warp_drive_context_enabled
    }

    pub fn is_file_based_mcp_enabled(&self, app: &warpui::AppContext) -> bool {
        if !FeatureFlag::FileBasedMcp.is_enabled() || !self.is_any_ai_enabled(app) {
            return false;
        }
        // NOTE: we intentionally do not force-enable this in Cloud Mode. Previously
        // we auto-spawned file-based MCPs in autonomous execution, but that bypassed
        // the user's explicit opt-in and let any MCP config checked into a repo run
        // arbitrary commands as part of a cloud agent run. Respecting the toggle
        // closes that attack surface; cloud agents that need project-scoped MCP
        // servers should surface an explicit, auditable opt-in. A more robust
        // solution (e.g. per-environment allowlisting, signed configs) should be
        // explored in the future.
        *self.file_based_mcp_enabled
    }

    pub fn is_orchestration_enabled(&self, app: &warpui::AppContext) -> bool {
        FeatureFlag::Orchestration.is_enabled()
            && self.is_any_ai_enabled(app)
            && *self.orchestration_enabled
    }

    /// Determines whether a quota reset banner should be displayed to the user.
    ///
    /// The banner should be shown if the most recent completed billing cycle had
    /// quota exceeded and the banner was not manually dismissed.
    pub fn should_display_quota_reset_banner(&self) -> bool {
        let quota_info = &self.ai_request_quota_info;

        let most_recent_completed_cycle = quota_info
            .cycle_history
            .iter()
            .rev()
            .find(|cycle| cycle.end_date < Utc::now());

        if let Some(cycle) = most_recent_completed_cycle {
            if cycle.was_quota_exceeded && !cycle.banner_state.dismissed {
                return true;
            }
        }

        false
    }

    /// Marks the banner as dismissed for all completed cycles.
    pub fn mark_quota_banner_as_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        let mut cycle_history = self.ai_request_quota_info.cycle_history.clone();

        for cycle in cycle_history.iter_mut() {
            if cycle.end_date < Utc::now() {
                cycle.banner_state.dismissed = true;
            }
        }

        report_if_error!(self
            .ai_request_quota_info
            .set_value(AIRequestQuotaInfo { cycle_history }, ctx));
    }

    /// Updates the quota info based on the latest RequestLimitInfo.
    ///
    /// This method finds or creates the appropriate CycleInfo based on the
    /// request_limit_info's next refresh time and updates its fields accordingly.
    pub fn update_quota_info(
        &mut self,
        request_limit_info: &RequestLimitInfo,
        ctx: &mut ModelContext<Self>,
    ) {
        // Convert ServerTimestamp to DateTime<Utc>
        let next_refresh_time = request_limit_info.next_refresh_time.utc();
        let now = Utc::now();

        // Check if request_limit_info has unlimited requests
        let is_quota_exceeded = !request_limit_info.is_unlimited
            && request_limit_info.num_requests_used_since_refresh >= request_limit_info.limit;

        let mut cycle_history = self.ai_request_quota_info.cycle_history.clone();

        // Track if we updated an existing cycle
        let mut updated_existing_cycle = false;

        // Find or create a cycle that matches the current period
        if let Some(current_cycle) = cycle_history.last_mut() {
            if now <= current_cycle.end_date {
                // Update existing cycle
                current_cycle.was_quota_exceeded = is_quota_exceeded;
                updated_existing_cycle = true;
            }
        }

        // Only create a new cycle if we didn't update an existing one
        if !updated_existing_cycle {
            // Create a new cycle
            let new_cycle = CycleInfo {
                end_date: next_refresh_time,
                was_quota_exceeded: is_quota_exceeded,
                banner_state: BannerState::default(),
            };

            cycle_history.push(new_cycle);
        }

        report_if_error!(self
            .ai_request_quota_info
            .set_value(AIRequestQuotaInfo { cycle_history }, ctx));
    }

    pub fn is_command_denylist_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_execute_commands_denylist();

        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_command_allowlist_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_execute_commands_allowlist();

        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_directory_allowlist_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_read_files_allowlist();

        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_execute_commands_permissions_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_execute_commands();

        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_write_to_pty_permissions_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_write_to_pty();
        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_computer_use_permissions_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_computer_use();
        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_read_files_permissions_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_read_files();

        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_code_diffs_permissions_editable(&self, app: &AppContext) -> bool {
        let set_by_workspace = UserWorkspaces::as_ref(app)
            .ai_autonomy_settings()
            .has_override_for_code_diffs();

        self.is_any_ai_enabled(app) && !set_by_workspace
    }

    pub fn is_ask_user_question_permissions_editable(&self, app: &AppContext) -> bool {
        self.is_any_ai_enabled(app)
    }

    pub fn is_mcp_permission_editable(&self, app: &AppContext) -> bool {
        // TODO: Allow workspace overrides on MCP permissions.
        self.is_any_ai_enabled(app)
    }

    pub fn show_code_suggestion_speedbump(&self, app: &AppContext) -> bool {
        self.is_any_ai_enabled(app) && *self.show_code_suggestion_speedbump
    }

    /// Handles first-time voice input setup when user clicks the voice button.
    ///
    /// If the user hasn't explicitly interacted with voice yet:
    /// - Sets the default voice input toggle key based on the OS
    /// - Marks `explicitly_interacted_with_voice` as true
    /// - Returns `Some(toggle_key)` so the caller can show a toast
    ///
    /// If the user has already interacted with voice, returns `None`.
    pub fn maybe_setup_first_time_voice(
        &mut self,
        ctx: &mut ModelContext<Self>,
    ) -> Option<VoiceInputToggleKey> {
        if *self.explicitly_interacted_with_voice.value() {
            return None;
        }

        let voice_input_toggle_key = match OperatingSystem::get() {
            OperatingSystem::Mac => VoiceInputToggleKey::Fn,
            OperatingSystem::Windows | OperatingSystem::Linux | OperatingSystem::Other(_) => {
                VoiceInputToggleKey::AltRight
            }
        };

        report_if_error!(self
            .voice_input_toggle_key
            .set_value(voice_input_toggle_key, ctx));

        report_if_error!(self.explicitly_interacted_with_voice.set_value(true, ctx));

        Some(voice_input_toggle_key)
    }

    pub fn add_cli_agent_footer_enabled_command(
        &mut self,
        command: &str,
        ctx: &mut ModelContext<Self>,
    ) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        if self
            .cli_agent_footer_enabled_commands
            .value()
            .contains_key(command)
        {
            return;
        }

        let mut map = self.cli_agent_footer_enabled_commands.value().0.clone();
        map.insert(command.to_string(), String::new());
        report_if_error!(self
            .cli_agent_footer_enabled_commands
            .set_value(ToolbarCommandMap::new(map), ctx));
    }

    pub fn remove_cli_agent_footer_enabled_command(
        &mut self,
        command: &str,
        ctx: &mut ModelContext<Self>,
    ) {
        let command = command.trim();
        let mut map = self.cli_agent_footer_enabled_commands.value().0.clone();
        map.shift_remove(command);
        report_if_error!(self
            .cli_agent_footer_enabled_commands
            .set_value(ToolbarCommandMap::new(map), ctx));
    }

    pub fn set_cli_agent_for_command(
        &mut self,
        pattern: &str,
        agent: Option<CLIAgent>,
        ctx: &mut ModelContext<Self>,
    ) {
        let mut map = self.cli_agent_footer_enabled_commands.value().0.clone();
        if !map.contains_key(pattern) {
            return;
        }
        let value = agent.map(|a| a.to_serialized_name()).unwrap_or_default();
        map.insert(pattern.to_string(), value);
        report_if_error!(self
            .cli_agent_footer_enabled_commands
            .set_value(ToolbarCommandMap::new(map), ctx));
    }

    /// Whether the plugin install chip was dismissed for the given agent/host.
    pub fn is_plugin_install_chip_dismissed(&self, key: &str) -> bool {
        self.plugin_install_chip_dismissed_map
            .get(key)
            .copied()
            .unwrap_or(false)
    }

    /// Mark the plugin install chip as dismissed for the given agent/host.
    pub fn dismiss_plugin_install_chip(&mut self, key: &str, ctx: &mut ModelContext<Self>) {
        let mut map = self.plugin_install_chip_dismissed_map.clone();
        map.insert(key.to_owned(), true);
        report_if_error!(self.plugin_install_chip_dismissed_map.set_value(map, ctx));
    }

    /// Returns the minimum plugin version for which the update chip was dismissed
    /// for the given agent/host, or an empty string if not dismissed.
    pub fn plugin_update_chip_dismissed_version(&self, key: &str) -> &str {
        self.plugin_update_chip_dismissed_for_version_map
            .get(key)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Record that the user dismissed the update chip for the given agent/host at
    /// the specified minimum version.
    pub fn dismiss_plugin_update_chip(
        &mut self,
        key: &str,
        version: String,
        ctx: &mut ModelContext<Self>,
    ) {
        let mut map = self.plugin_update_chip_dismissed_for_version_map.clone();
        map.insert(key.to_owned(), version);
        report_if_error!(self
            .plugin_update_chip_dismissed_for_version_map
            .set_value(map, ctx));
    }
}

/// Singleton model that caches compiled regexes for the `cli_agent_footer_enabled_commands`
/// setting. Each entry pairs a compiled regex with the CLI agent it maps to.
pub struct CompiledCommandsForCodingAgentToolbar {
    regexes: Vec<(Regex, CLIAgent)>,
}

impl CompiledCommandsForCodingAgentToolbar {
    fn parse(app: &AppContext) -> Vec<(Regex, CLIAgent)> {
        AISettings::as_ref(app)
            .cli_agent_footer_enabled_commands
            .value()
            .iter()
            .filter_map(|(pattern, agent_name)| {
                let regex = Regex::new(pattern).ok()?;
                let agent = CLIAgent::from_serialized_name(agent_name);
                Some((regex, agent))
            })
            .collect()
    }

    fn register(app: &mut AppContext) {
        let handle = app.add_singleton_model(|ctx| Self {
            regexes: Self::parse(ctx),
        });
        let ai_settings = AISettings::handle(app);
        app.subscribe_to_model(&ai_settings, move |_, event, ctx| {
            if matches!(
                event,
                AISettingsChangedEvent::CLIAgentToolbarEnabledCommands { .. }
            ) {
                let regexes = Self::parse(ctx);
                handle.update(ctx, |me, _| {
                    me.regexes = regexes;
                });
            }
        });
    }

    /// Returns the CLI agent assigned to the first matching pattern, or `None`
    /// if no pattern matches the command.
    pub fn matched_agent(app: &AppContext, command: &str) -> Option<CLIAgent> {
        Self::as_ref(app)
            .regexes
            .iter()
            .find(|(regex, _)| regex.is_match(command))
            .map(|(_, agent)| *agent)
    }
}

impl Entity for CompiledCommandsForCodingAgentToolbar {
    type Event = ();
}

impl SingletonEntity for CompiledCommandsForCodingAgentToolbar {}

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
