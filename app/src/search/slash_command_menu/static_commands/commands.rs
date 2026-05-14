use std::{collections::HashMap, sync::LazyLock};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp_core::features::FeatureFlag;

use crate::search::slash_command_menu::{static_commands::Argument, StaticCommand};
use crate::t_static;

use super::Availability;

pub static AGENT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/agent",
    description: t_static!("slash-cmd-agent-desc"),
    icon_path: "bundled/svg/oz.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub static ADD_MCP: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/add-mcp",
    description: t_static!("slash-cmd-add-mcp-desc"),
    icon_path: "bundled/svg/dataflow.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static PR_COMMENTS: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/pr-comments",
    description: t_static!("slash-cmd-pr-comments-desc"),
    icon_path: "bundled/svg/github.svg",
    availability: Availability::REPOSITORY.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
});

pub static CREATE_DOCKER_SANDBOX: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/docker-sandbox",
    description: t_static!("slash-cmd-docker-sandbox-desc"),
    icon_path: "bundled/svg/docker.svg",
    availability: Availability::LOCAL.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
});

pub static CREATE_NEW_PROJECT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/create-new-project",
    description: t_static!("slash-cmd-create-new-project-desc"),
    icon_path: "bundled/svg/plus.svg",
    availability: Availability::LOCAL | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(
        Argument::required().with_hint_text(t_static!("slash-cmd-create-new-project-hint")),
    ),
});

pub static EDIT_SKILL: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-skill",
    description: t_static!("slash-cmd-open-skill-desc"),
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static INVOKE_SKILL: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/skills",
    description: t_static!("slash-cmd-skills-desc"),
    icon_path: "bundled/svg/stars-01.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static ADD_PROMPT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/add-prompt",
    description: t_static!("slash-cmd-add-prompt-desc"),
    icon_path: if FeatureFlag::AgentView.is_enabled() {
        "bundled/svg/prompt.svg"
    } else {
        "bundled/svg/agentmode.svg"
    },
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static ADD_RULE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/add-rule",
    description: t_static!("slash-cmd-add-rule-desc"),
    icon_path: "bundled/svg/book-open.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static EDIT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-file",
    description: t_static!("slash-cmd-open-file-desc"),
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_hint_text(t_static!("slash-cmd-open-file-hint"))),
});

pub static RENAME_TAB: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/rename-tab",
    description: t_static!("slash-cmd-rename-tab-desc"),
    icon_path: "bundled/svg/pencil-line.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: Some(Argument::required().with_hint_text(t_static!("slash-cmd-rename-tab-hint"))),
});

pub static FORK: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/fork",
    description: t_static!("slash-cmd-fork-desc"),
    icon_path: "bundled/svg/arrow-split.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text(t_static!("slash-cmd-fork-hint"))),
});

pub static OPEN_CODE_REVIEW: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-code-review",
    description: t_static!("slash-cmd-open-code-review-desc"),
    icon_path: "bundled/svg/diff.svg",
    availability: Availability::REPOSITORY,
    auto_enter_ai_mode: false,
    argument: None,
});

pub const INIT_NAME: &str = "/init";

pub static INIT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: INIT_NAME,
    description: t_static!("slash-cmd-init-desc"),
    icon_path: "bundled/svg/warp-2.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional()),
});

pub static OPEN_PROJECT_RULES: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-project-rules",
    description: t_static!("slash-cmd-open-project-rules-desc"),
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::REPOSITORY.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
});

pub static OPEN_MCP_SERVERS: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-mcp-servers",
    description: t_static!("slash-cmd-open-mcp-servers-desc"),
    icon_path: "bundled/svg/dataflow.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static OPEN_SETTINGS_FILE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-settings-file",
    description: t_static!("slash-cmd-open-settings-file-desc"),
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static CHANGELOG: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/changelog",
    description: t_static!("slash-cmd-changelog-desc"),
    icon_path: "bundled/svg/book-open.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static OPEN_REPO: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-repo",
    description: t_static!("slash-cmd-open-repo-desc"),
    icon_path: "bundled/svg/folder.svg",
    availability: Availability::LOCAL.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
});

pub static OPEN_RULES: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-rules",
    description: t_static!("slash-cmd-open-rules-desc"),
    icon_path: "bundled/svg/book-open.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static NEW: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/new",
    description: t_static!("slash-cmd-new-desc"),
    icon_path: "bundled/svg/new-conversation.svg",
    availability: Availability::NO_LRC_CONTROL | Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub static MODEL: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/model",
    description: t_static!("slash-cmd-model-desc"),
    icon_path: "bundled/svg/oz.svg",
    availability: Availability::AGENT_VIEW | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: None,
});

pub static PROFILE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/profile",
    description: t_static!("slash-cmd-profile-desc"),
    icon_path: "bundled/svg/psychology.svg",
    availability: Availability::AGENT_VIEW | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: None,
});

pub const PLAN_NAME: &str = "/plan";

pub static PLAN: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: PLAN_NAME,
    description: t_static!("slash-cmd-plan-desc"),
    icon_path: "bundled/svg/file-06.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text(t_static!("slash-cmd-plan-hint"))),
});

/// If `query` starts with the given command `name` followed by a space,
/// returns the remainder of the query. Otherwise returns `None`.
pub fn strip_command_prefix(query: &str, name: &str) -> Option<String> {
    query
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix(' '))
        .map(|rest| rest.to_string())
}

pub static COMPACT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/compact",
    description: t_static!("slash-cmd-compact-desc"),
    icon_path: "bundled/svg/collapse_content.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text(t_static!("slash-cmd-compact-hint"))),
});

pub static COMPACT_AND: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/compact-and",
    description: t_static!("slash-cmd-compact-and-desc"),
    icon_path: "bundled/svg/collapse_content.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text(t_static!("slash-cmd-compact-and-hint"))),
});

pub static QUEUE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/queue",
    description: t_static!("slash-cmd-queue-desc"),
    icon_path: "bundled/svg/clock-plus.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::required().with_hint_text(t_static!("slash-cmd-queue-hint"))),
});

pub static FORK_AND_COMPACT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/fork-and-compact",
    description: t_static!("slash-cmd-fork-and-compact-desc"),
    icon_path: "bundled/svg/fork_and_compact.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(
        Argument::optional().with_hint_text(t_static!("slash-cmd-fork-and-compact-hint")),
    ),
});

pub static FORK_FROM: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/fork-from",
    description: t_static!("slash-cmd-fork-from-desc"),
    icon_path: "bundled/svg/arrow-split.svg",
    availability: Availability::AGENT_VIEW
        .union(Availability::NO_LRC_CONTROL)
        .union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
});

pub static CONVERSATIONS: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/conversations",
    description: t_static!("slash-cmd-conversations-desc"),
    icon_path: "bundled/svg/conversation.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static PROMPTS: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/prompts",
    description: t_static!("slash-cmd-prompts-desc"),
    icon_path: "bundled/svg/prompt.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static REWIND: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/rewind",
    description: t_static!("slash-cmd-rewind-desc"),
    icon_path: "bundled/svg/clock-rewind.svg",
    availability: Availability::AGENT_VIEW.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
});

pub static EXPORT_TO_CLIPBOARD: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/export-to-clipboard",
    description: t_static!("slash-cmd-export-to-clipboard-desc"),
    icon_path: "bundled/svg/copy.svg",
    availability: Availability::AGENT_VIEW.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
});

pub static EXPORT_TO_FILE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/export-to-file",
    description: t_static!("slash-cmd-export-to-file-desc"),
    icon_path: "bundled/svg/download-01.svg",
    availability: Availability::AGENT_VIEW | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text(t_static!("slash-cmd-export-to-file-hint"))),
});

pub static COMMAND_REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

/// A unique identifier for a static slash command.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SlashCommandId(Uuid);

impl SlashCommandId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SlashCommandId {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Registry {
    commands: HashMap<SlashCommandId, StaticCommand>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        let mut commands = HashMap::new();
        for command in all_commands().into_iter() {
            debug_assert!(
                !command
                    .availability
                    .contains(Availability::TERMINAL_VIEW | Availability::AGENT_VIEW),
                "command `{}` sets both TERMINAL_VIEW and AGENT_VIEW, which is unsatisfiable",
                command.name,
            );
            commands.insert(SlashCommandId::new(), command);
        }
        Self { commands }
    }

    pub fn all_commands_by_id(&self) -> impl Iterator<Item = (SlashCommandId, &StaticCommand)> {
        self.commands.iter().map(|(id, cmd)| (*id, cmd))
    }

    pub fn all_commands(&self) -> impl Iterator<Item = &StaticCommand> {
        self.commands.values()
    }

    pub fn get_command(&self, id: &SlashCommandId) -> Option<&StaticCommand> {
        self.commands.get(id)
    }

    pub fn get_command_with_name(&self, name: &str) -> Option<&StaticCommand> {
        self.commands.values().find(|command| command.name == name)
    }

    #[cfg(test)]
    pub fn get_command_id_with_name(&self, name: &str) -> Option<&SlashCommandId> {
        self.commands
            .iter()
            .find(|(_, command)| command.name == name)
            .map(|(id, _)| id)
    }
}

fn all_commands() -> Vec<StaticCommand> {
    let mut commands = vec![
        ADD_MCP.clone(),
        ADD_PROMPT.clone(),
        ADD_RULE.clone(),
        INIT.clone(),
        OPEN_PROJECT_RULES.clone(),
        OPEN_MCP_SERVERS.clone(),
        OPEN_RULES.clone(),
        AGENT.clone(),
        NEW.clone(),
        PLAN.clone(),
        RENAME_TAB.clone(),
        CONVERSATIONS.clone(),
        EXPORT_TO_CLIPBOARD.clone(),
        MODEL.clone(),
    ];

    if FeatureFlag::LocalDockerSandbox.is_enabled() {
        commands.push(CREATE_DOCKER_SANDBOX.clone());
    }

    if FeatureFlag::Changelog.is_enabled() {
        commands.push(CHANGELOG.clone());
    }

    if FeatureFlag::AgentView.is_enabled() {
        commands.push(PROMPTS.clone());
    }

    commands.push(OPEN_CODE_REVIEW.clone());

    if FeatureFlag::CreateProjectFlow.is_enabled() {
        commands.push(CREATE_NEW_PROJECT.clone());
    }

    if FeatureFlag::SummarizationConversationCommand.is_enabled() {
        commands.push(COMPACT.clone());
        commands.push(COMPACT_AND.clone());
    }

    if FeatureFlag::QueueSlashCommand.is_enabled() {
        commands.push(QUEUE.clone());
    }

    if !cfg!(target_family = "wasm") {
        commands.extend([FORK.clone(), FORK_AND_COMPACT.clone()]);

        if FeatureFlag::ForkFromCommand.is_enabled() {
            commands.push(FORK_FROM.clone());
        }
    }

    if !cfg!(target_family = "wasm") {
        commands.extend([EDIT.clone(), EXPORT_TO_FILE.clone()]);
    }

    if FeatureFlag::ListSkills.is_enabled() && !cfg!(target_family = "wasm") {
        commands.push(EDIT_SKILL.clone());
        commands.push(INVOKE_SKILL.clone());
    }

    if FeatureFlag::PRCommentsSlashCommand.is_enabled()
        && !FeatureFlag::PRCommentsSkill.is_enabled()
    {
        commands.push(PR_COMMENTS.clone());
    }

    if FeatureFlag::InlineProfileSelector.is_enabled() {
        commands.push(PROFILE.clone());
    }

    if FeatureFlag::RevertToCheckpoints.is_enabled() && FeatureFlag::RewindSlashCommand.is_enabled()
    {
        commands.push(REWIND.clone());
    }

    if FeatureFlag::InlineRepoMenu.is_enabled() && !cfg!(target_family = "wasm") {
        commands.push(OPEN_REPO.clone());
    }

    if FeatureFlag::SettingsFile.is_enabled() && cfg!(feature = "local_fs") {
        commands.push(OPEN_SETTINGS_FILE.clone());
    }

    commands
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn command_names_are_unique() {
        let names = COMMAND_REGISTRY.all_commands().map(|command| command.name);
        let mut seen = HashSet::new();
        for name in names {
            assert!(seen.insert(name), "duplicate slash command name: {name}");
        }
    }

    #[test]
    fn rename_tab_command_requires_argument() {
        // hint_text 走 i18n,初始化 loader 后取真实英文文案
        crate::i18n::init(Some("en"));
        let command = COMMAND_REGISTRY
            .get_command_with_name(RENAME_TAB.name)
            .expect("expected /rename-tab to be registered");
        let argument = command
            .argument
            .as_ref()
            .expect("expected /rename-tab to require an argument");

        assert!(!argument.is_optional);
        assert!(!argument.should_execute_on_selection);
        assert_eq!(argument.hint_text, Some("<tab name>"));
    }

    #[test]
    fn strip_command_prefix_no_match() {
        let result = strip_command_prefix("just a normal query", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_empty() {
        let result = strip_command_prefix("", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_no_trailing_space() {
        // "/plan" alone (no trailing space) should NOT be stripped
        let result = strip_command_prefix("/plan", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_trailing_space_only() {
        // "/plan " with nothing after should strip to empty string
        let result = strip_command_prefix("/plan ", "/plan");
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn strip_command_prefix_substring_not_matched() {
        // "/planning" should not match "/plan"
        let result = strip_command_prefix("/planning something", "/plan");
        assert_eq!(result, None);
    }
}
