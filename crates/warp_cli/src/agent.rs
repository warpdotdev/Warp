use std::{fmt, path::PathBuf};

use clap::{Args, Subcommand, ValueEnum};

use crate::{
    config_file::ConfigFileArgs, mcp::MCPSpec, model::ModelArgs, share::ShareArgs, skill::SkillSpec,
};

/// Output format for agent results.
#[derive(Debug, Copy, Clone, ValueEnum, Eq, PartialEq, Default)]
pub enum OutputFormat {
    /// Output as JSON.
    #[value(name = "json")]
    Json,
    /// Output as newline-delimited JSON.
    #[value(name = "ndjson")]
    Ndjson,
    /// Output as human-readable text.
    #[default]
    #[value(name = "pretty")]
    Pretty,
    /// Output as plain text.
    #[value(name = "text")]
    Text,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.to_possible_value().expect("no values are skipped");
        f.write_str(value.get_name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    PlainText(String),
    SavedPrompt(String),
}

impl fmt::Display for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prompt::PlainText(text) => write!(f, "Prompt: {text}"),
            Prompt::SavedPrompt(id) => write!(f, "Saved Prompt ID: {id}"),
        }
    }
}

/// Prompt arguments - mutually exclusive prompt or saved-prompt.
/// The required constraint is enforced at the command level via ArgGroup.
#[derive(Debug, Clone, Args)]
#[group(multiple = false)]
pub struct PromptArg {
    /// Prompt for the agent to carry out.
    #[arg(long = "prompt", short = 'p')]
    pub prompt: Option<String>,
    /// The saved AI prompt to run, identified by id.
    #[arg(long = "saved-prompt")]
    pub saved_prompt: Option<String>,
}

impl PromptArg {
    pub fn to_prompt(&self) -> Option<Prompt> {
        match (self.prompt.as_ref(), self.saved_prompt.as_ref()) {
            (Some(prompt), None) => Some(Prompt::PlainText(prompt.clone())),
            (None, Some(saved_prompt)) => Some(Prompt::SavedPrompt(saved_prompt.clone())),
            _ => None,
        }
    }
}

/// Shared CLI args for controlling computer use capabilities.
#[derive(Debug, Clone, Args, Default)]
pub struct ComputerUseArgs {
    /// Enable computer use capabilities for this agent run.
    #[arg(long = "computer-use", conflicts_with = "no_computer_use")]
    pub computer_use: bool,

    /// Disable computer use capabilities for this agent run.
    #[arg(long = "no-computer-use", conflicts_with = "computer_use")]
    pub no_computer_use: bool,
}

impl ComputerUseArgs {
    /// Returns the computer use override based on CLI flags.
    /// - `Some(true)` if `--computer-use` was specified
    /// - `Some(false)` if `--no-computer-use` was specified
    /// - `None` if neither was specified (use default behavior)
    pub fn computer_use_override(&self) -> Option<bool> {
        match (self.computer_use, self.no_computer_use) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }
}

/// Hidden variant of [`ComputerUseArgs`] for commands where computer use flags
/// should be accepted but not shown in help output.
#[derive(Debug, Clone, Args, Default)]
pub struct HiddenComputerUseArgs {
    /// Enable computer use capabilities for this agent run.
    #[arg(long = "computer-use", conflicts_with = "no_computer_use", hide = true)]
    pub computer_use: bool,

    /// Disable computer use capabilities for this agent run.
    #[arg(long = "no-computer-use", conflicts_with = "computer_use", hide = true)]
    pub no_computer_use: bool,
}

impl HiddenComputerUseArgs {
    pub fn computer_use_override(&self) -> Option<bool> {
        match (self.computer_use, self.no_computer_use) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }
}
/// The execution harness for an agent run.
#[derive(Debug, Copy, Clone, ValueEnum, Eq, PartialEq, Default)]
pub enum Harness {
    /// Use Warp's built-in MAA infrastructure (default).
    #[default]
    #[value(name = "oz")]
    Oz,
    /// Delegate to the `claude` CLI.
    #[value(name = "claude", alias = "claude-code")]
    Claude,
    /// Delegate to the `opencode` CLI.
    #[value(name = "opencode", alias = "open-code")]
    OpenCode,
    /// Delegate to the `gemini` CLI.
    #[value(name = "gemini")]
    Gemini,
    /// A harness produced by a newer client/server that this client doesn't
    /// recognize. Surfaced via deserialization fallbacks (e.g. unknown serialized
    /// enum values, unknown `harness_type` strings); never selectable from the
    /// CLI or harness dropdown.
    #[value(skip)]
    Unknown,
}

impl Harness {
    pub fn parse_orchestration_harness(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        <Self as ValueEnum>::from_str(&normalized, true).ok()
    }

    pub fn parse_local_child_harness(value: &str) -> Option<Self> {
        match Self::parse_orchestration_harness(value) {
            Some(harness @ (Self::Claude | Self::OpenCode)) => Some(harness),
            Some(Self::Oz) | Some(Self::Gemini) | Some(Self::Unknown) | None => None,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Oz => "Oz",
            Self::Claude => "Claude Code",
            Self::OpenCode => "OpenCode",
            Self::Gemini => "Gemini CLI",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for Harness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Harness::Oz => "oz",
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
            Harness::Gemini => "gemini",
            Harness::Unknown => "unknown",
        };
        f.write_str(name)
    }
}

/// Profile subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum AgentProfileCommand {
    /// List available agent profiles.
    List,
}

/// Agent-related subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum AgentCommand {
    /// Run a new Oz agent.
    Run(Box<RunAgentArgs>),
    /// Manage agent profiles.
    #[command(subcommand)]
    Profile(AgentProfileCommand),
    /// List all available agents.
    List(ListAgentConfigsArgs),
}

#[derive(Debug, Clone, Args)]
#[command(
    visible_alias = "r",
    group(
        clap::ArgGroup::new("prompt_group")
            .required(true)
            .multiple(true)
            .args(["prompt", "saved_prompt", "skill"])
    )
)]
pub struct RunAgentArgs {
    #[command(flatten)]
    pub prompt_arg: PromptArg,

    #[command(flatten)]
    pub model: ModelArgs,

    #[command(flatten)]
    pub config_file: ConfigFileArgs,

    /// Use a skill as the base prompt for the agent.
    ///
    /// Format: `skill_name`, `repo:skill_name`, or `org/repo:skill_name`
    ///
    /// Skills are searched in `.agents/skills/`, `.warp/skills/`, `.claude/skills/`, and `.codex/skills/` directories.
    /// If a repo is specified, searches only that repo. If org is also specified,
    /// validates the repo's git remote matches the expected org.
    ///
    /// When used with --prompt, the skill provides the base context and the prompt is the task.
    ///
    /// To automate a skill on a schedule, use `oz schedule create --skill <SPEC>`.
    #[arg(long = "skill", value_name = "SPEC")]
    pub skill: Option<SkillSpec>,

    /// Name for this agent task.
    #[arg(long = "name", short = 'n')]
    pub name: Option<String>,
    /// Working directory for the agent
    #[arg(short = 'C', long = "cwd")]
    pub cwd: Option<PathBuf>,
    /// Display agent progress in the Warp interface.
    #[arg(long = "gui", hide = true)]
    pub gui: bool,
    #[command(flatten)]
    pub share: ShareArgs,
    /// MCP servers to start before executing the agent.
    ///
    /// Can be specified as:
    /// - A path to a JSON file containing MCP configuration
    /// - Inline JSON with MCP server configuration
    ///
    /// Can be specified multiple times to include multiple servers.
    #[arg(long = "mcp", value_name = "SPEC")]
    pub mcp_specs: Vec<MCPSpec>,
    /// LEGACY: MCP servers to start before executing the agent, identified by UUID.
    #[arg(long = "mcp-server", value_name = "UUID", hide = true)]
    pub mcp_servers: Vec<uuid::Uuid>,
    // OpenWarp Wave 7-2:`--environment` 参数 随 cloud ambient agent 主体子系统物理删。
    /// Keep the agent's session open after the conversation completes.
    ///
    /// This is useful when you want to keep the session alive for follow-up interactions.
    ///
    /// You can optionally provide a duration (e.g. `--idle-on-complete 10m`).
    #[arg(
        long = "idle-on-complete",
        value_name = "DURATION",
        num_args = 0..=1,
        default_missing_value = "45m",
        hide = true
    )]
    pub idle_on_complete: Option<humantime::Duration>,

    /// Whether we are running the agent in a sandboxed environment.
    #[arg(long = "sandboxed", hide = true)]
    pub sandboxed: bool,
    /// IAM role ARN to use for federated AWS Bedrock credentials for this run.
    #[arg(long = "bedrock-inference-role", value_name = "ROLE_ARN", hide = true)]
    pub bedrock_inference_role: Option<String>,

    #[command(flatten)]
    pub computer_use: HiddenComputerUseArgs,

    /// Agent profile to configure the terminal session.
    #[arg(long = "profile", value_name = "ID")]
    pub profile: Option<String>,

    /// Execution harness for the agent run.
    ///
    /// "oz" (default) uses Warp's built-in agent infrastructure.
    /// "claude" delegates to the `claude` CLI.
    #[arg(long = "harness", value_name = "HARNESS", default_value_t = Harness::Oz, hide = true)]
    pub harness: Harness,
}

impl RunAgentArgs {
    /// Combine `mcp_specs` with legacy `mcp_servers` (UUIDs) into a single list.
    pub fn all_mcp_specs(&self) -> Vec<MCPSpec> {
        let mut specs = self.mcp_specs.clone();
        specs.extend(self.mcp_servers.iter().cloned().map(MCPSpec::Uuid));
        specs
    }
}

/// Arguments for listing available agents.
#[derive(Debug, Clone, Args)]
pub struct ListAgentConfigsArgs {
    /// List skills from a specific GitHub repository.
    ///
    /// Format: `owner/repo` or `https://github.com/owner/repo`
    ///
    /// When provided, lists skills from this repo instead of from your environments.
    /// Any environments that include this repo will still be shown in the results.
    #[arg(long = "repo", short = 'r', value_name = "REPO")]
    pub repo: Option<String>,
}
