use crate::palette::PaletteMode;
use crate::server::telemetry::PaletteSource;
use crate::settings::AISettings;
use crate::terminal::input::SET_INPUT_MODE_AGENT_ACTION_NAME;
use crate::terminal::view::init::{
    CANCEL_COMMAND_KEYBINDING, SELECT_PREVIOUS_BLOCK_ACTION_NAME,
    TOGGLE_AUTOEXECUTE_MODE_KEYBINDING,
};
use crate::util::bindings::trigger_to_keystroke;
use crate::workspace::view::{
    TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME, TOGGLE_RIGHT_PANEL_BINDING_NAME,
};
use crate::workspace::WorkspaceAction;
use crate::workspaces::user_workspaces::UserWorkspaces;

use markdown_parser::FormattedTextFragment;

use std::sync::LazyLock;
use std::time::Duration;
use warpui::keymap::Keystroke;
use warpui::r#async::SpawnedFutureHandle;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

/// Trait for tip implementations that can be displayed to users.
/// Tips provide helpful information with optional links and keybindings.
pub trait AITip: Clone {
    /// Returns the keystroke for this tip, if applicable.
    fn keystroke(&self, app: &AppContext) -> Option<Keystroke>;

    /// Returns the documentation link for this tip, if available.
    fn link(&self) -> Option<String>;

    /// Returns the raw description text for this tip.
    fn description(&self) -> &str;

    /// Converts the tip to formatted text fragments for rendering.
    /// Default implementation adds "Tip: " prefix and parses backtick-wrapped text as inline code.
    fn to_formatted_text(&self, _app: &AppContext) -> Vec<FormattedTextFragment> {
        let text = format!("{} {}", crate::t!("agent-tip-prefix"), self.description());

        // Style backtick-wrapped text as inline code
        let parts: Vec<&str> = text.split('`').collect();
        let mut fragments = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if i % 2 == 0 {
                fragments.push(FormattedTextFragment::plain_text(part.to_string()));
            } else {
                fragments.push(FormattedTextFragment::inline_code(part.to_string()));
            }
        }
        fragments
    }

    /// Checks if this tip is applicable in the current context.
    /// Default implementation returns true (tip is always applicable).
    fn is_tip_applicable(
        &self,
        _current_working_directory: Option<&str>,
        _app: &AppContext,
    ) -> bool {
        true
    }
}

/// Kinds of agent tips for organizing and filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentTipKind {
    WarpDrive,
    General,
    Mcp,
    SlashCommands,
    /// Tips about adding context (files, blocks, URLs, images, @-mentions, rules)
    Context,
    /// Tips about code editors, file trees, and code review panes
    Code,
}

static DEFAULT_TIPS: LazyLock<Vec<AgentTip>> = LazyLock::new(|| {
    vec![
        AgentTip {
            description: crate::t!("agent-tip-slash-menu"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/slash-commands".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: crate::t!("agent-tip-toggle-input-mode"),
            link: Some("https://docs.warp.dev/terminal/input/universal-input#input-modes".to_string()),
            binding_name: Some(SET_INPUT_MODE_AGENT_ACTION_NAME),
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-plan"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/planning".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: crate::t!("agent-tip-command-palette"),
            link: Some("https://docs.warp.dev/terminal/command-palette".to_string()),
            binding_name: Some(TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME),
            action: Some(WorkspaceAction::OpenPalette {
                mode: PaletteMode::Command,
                source: PaletteSource::AgentTip,
                query: None,
            }),
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-warp-drive"),
            link: Some("https://docs.warp.dev/knowledge-and-collaboration/warp-drive".to_string()),
            binding_name: None,
            action: Some(WorkspaceAction::OpenWarpDrive),
            kind: AgentTipKind::WarpDrive,
        },
        AgentTip {
            description: crate::t!("agent-tip-redirect-running-agent"),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-add-context"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/using-to-add-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: crate::t!("agent-tip-attach-prior-output"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/blocks-as-context#attaching-blocks-as-context".to_string()),
            binding_name: Some(SELECT_PREVIOUS_BLOCK_ACTION_NAME),
            action: None,
            kind: AgentTipKind::Context,
        },

        AgentTip {
            description: crate::t!("agent-tip-agent-profiles"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/agent-profiles-permissions".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-fork-block"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents/conversation-forking".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-copy-output"),
            link: Some("https://docs.warp.dev/terminal/blocks/block-actions#copy-input-output-of-block".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-drag-image"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/images-as-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: crate::t!("agent-tip-interactive-tools"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/full-terminal-use".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-code-review-panel"),
            link: Some("https://docs.warp.dev/code/code-review".to_string()),
            binding_name: Some(TOGGLE_RIGHT_PANEL_BINDING_NAME),
            action: None,
            kind: AgentTipKind::Code,
        },
        AgentTip {
            description: crate::t!("agent-tip-add-mcp"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/mcp".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Mcp,
        },
        AgentTip {
            description: crate::t!("agent-tip-open-mcp-servers"),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::Mcp,
        },
        AgentTip {
            description: crate::t!("agent-tip-add-prompt"),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::WarpDrive,
        },
        AgentTip {
            description: crate::t!("agent-tip-add-rule"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/rules".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: crate::t!("agent-tip-fork"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents/conversation-forking".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: crate::t!("agent-tip-open-code-review"),
            link: None,
            binding_name: None,
            action: Some(WorkspaceAction::ToggleRightPanel),
            kind: AgentTipKind::Code,
        },
        AgentTip {
            description: crate::t!("agent-tip-new-conversation"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: crate::t!("agent-tip-compact"),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: crate::t!("agent-tip-usage"),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-oz-headless"),
            link: Some("https://docs.warp.dev/reference/cli".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-selected-text-context"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/blocks-as-context#attaching-blocks-as-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: crate::t!("agent-tip-project-rules"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/rules#project-rules-1".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: crate::t!("agent-tip-url-context"),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/urls-as-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: crate::t!("agent-tip-warpify-ssh"),
            link: Some("https://docs.warp.dev/terminal/warpify".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-switch-profiles"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/agent-profiles-permissions".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-init-rules"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/rules".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: crate::t!("agent-tip-auto-approve"),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/full-terminal-use#session-level-approvals".to_string()),
            binding_name: Some(TOGGLE_AUTOEXECUTE_MODE_KEYBINDING),
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-desktop-notifications"),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: crate::t!("agent-tip-cancel-task"),
            link: None,
            binding_name: Some(CANCEL_COMMAND_KEYBINDING),
            action: None,
            kind: AgentTipKind::General,
        },
    ]
});

#[derive(Clone, Debug)]
pub struct AgentTip {
    /// The text that will be displayed to the user. This is parsed such that:
    /// "Tip: " is added as a prefix,
    /// "<keybinding>" is replaced with user-defined and platform-specific keybinding referenced by binding_name,
    /// `text` that is wrapped in backticks is formatted as inline code
    pub description: String,
    pub link: Option<String>,
    pub binding_name: Option<&'static str>,
    pub action: Option<WorkspaceAction>,
    /// The kind of the tip, used for filtering and organization
    pub kind: AgentTipKind,
}

impl AITip for AgentTip {
    fn keystroke(&self, app: &AppContext) -> Option<Keystroke> {
        let binding_name = self.binding_name?;

        // Special case: voice input uses settings, not editable bindings
        if binding_name == "FN" {
            return AISettings::as_ref(app).voice_input_toggle_key.keystroke();
        }

        if let Some(binding) = app.editable_bindings().find(|b| b.name == binding_name) {
            return trigger_to_keystroke(binding.trigger);
        }
        None
    }

    fn link(&self) -> Option<String> {
        self.link.clone()
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn to_formatted_text(&self, app: &AppContext) -> Vec<FormattedTextFragment> {
        let mut text = format!("{} {}", crate::t!("agent-tip-prefix"), self.description);

        // Replace <keybinding> with the actual keybinding string
        if let Some(keystroke) = self.keystroke(app) {
            text = text.replace("<keybinding>", &keystroke.displayed());
        }

        // Style backtick-wrapped text as inline code
        let parts: Vec<&str> = text.split('`').collect();
        let mut fragments = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if i % 2 == 0 {
                fragments.push(FormattedTextFragment::plain_text(part.to_string()));
            } else {
                fragments.push(FormattedTextFragment::inline_code(part.to_string()));
            }
        }

        fragments
    }

    fn is_tip_applicable(
        &self,
        _current_working_directory: Option<&str>,
        _app: &AppContext,
    ) -> bool {
        true
    }
}

impl WorkspaceAction {
    pub fn display_text(&self) -> Option<String> {
        match self {
            WorkspaceAction::OpenPalette { .. } => Some(crate::t!("agent-tip-action-open-palette")),
            WorkspaceAction::OpenWarpDrive => Some(crate::t!("agent-tip-action-warp-drive")),
            WorkspaceAction::ToggleRightPanel => Some(crate::t!("agent-tip-action-show-diff-view")),
            _ => None,
        }
    }
}

/// Helper function to build the list of agent tips, including the voice tip if enabled.
pub fn get_agent_tips(ctx: &AppContext) -> Vec<AgentTip> {
    let mut tips = DEFAULT_TIPS.clone();

    if cfg!(feature = "voice_input")
        && UserWorkspaces::as_ref(ctx).is_voice_enabled()
        && AISettings::as_ref(ctx).is_voice_input_enabled(ctx)
    {
        tips.push(AgentTip {
            description: crate::t!("agent-tip-voice-input"),
            link: Some(
                "https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents/voice"
                    .to_string(),
            ),
            binding_name: Some("FN"),
            action: None,
            kind: AgentTipKind::General,
        });
    }

    tips
}

/// A model for managing tips with cooldown logic.
/// Generic over any type implementing the AITip trait.
pub struct AITipModel<T: AITip> {
    tips: Vec<T>,
    current_tip: Option<T>,
    cooldown_handle: Option<SpawnedFutureHandle>,
}

impl<T: AITip + 'static> AITipModel<T> {
    /// Creates a new AITipModel with the given tips.
    /// Selects a random initial tip from the provided tips, if any.
    pub fn new(tips: Vec<T>) -> Self {
        use rand::seq::SliceRandom;

        let mut rng = rand::thread_rng();
        let current_tip = tips.choose(&mut rng).cloned();

        Self {
            tips,
            current_tip,
            cooldown_handle: None,
        }
    }

    /// Returns the current tip, if one has been selected.
    pub fn current_tip(&self) -> Option<&T> {
        self.current_tip.as_ref()
    }
}

impl<T: AITip + 'static> Entity for AITipModel<T> {
    type Event = ();
}

// Specific implementation for AgentTip
impl AITipModel<AgentTip> {
    /// Creates a new AITipModel for AgentTips.
    /// This is the constructor used for the singleton model.
    pub fn new_for_agent_tips(ctx: &AppContext) -> Self {
        let tips = get_agent_tips(ctx);
        Self::new(tips)
    }

    /// Refreshes the current tip with a new random selection that is applicable
    /// for the given working directory.
    /// Only updates if not in cooldown period (60 seconds).
    pub fn maybe_refresh_tip(
        &mut self,
        current_working_directory: Option<&str>,
        ctx: &mut ModelContext<Self>,
    ) {
        // Don't update if cooldown is active
        if self.cooldown_handle.is_some() {
            return;
        }

        use rand::seq::SliceRandom;

        // Filter applicable tips based on working directory
        let available_tips: Vec<AgentTip> = self
            .tips
            .iter()
            .filter(|tip| tip.is_tip_applicable(current_working_directory, ctx))
            .cloned()
            .collect();

        // Select a random tip
        let mut rng = rand::thread_rng();
        self.current_tip = available_tips.choose(&mut rng).cloned();

        // Start 60-second cooldown
        let handle = ctx.spawn(
            async {
                warpui::r#async::Timer::after(Duration::from_secs(60)).await;
            },
            |me, _, _| {
                me.cooldown_handle = None;
            },
        );
        self.cooldown_handle = Some(handle);
        ctx.notify();
    }
}

impl SingletonEntity for AITipModel<AgentTip> {}

// Specific implementation for AmbientAgentTip
impl AITipModel<crate::terminal::view::ambient_agent::AmbientAgentTip> {
    /// Refreshes the current tip with a new random selection.
    /// Only updates if not in cooldown period (60 seconds).
    pub fn maybe_refresh_tip(&mut self, ctx: &mut ModelContext<Self>) {
        // Don't update if cooldown is active
        if self.cooldown_handle.is_some() {
            return;
        }

        use rand::seq::SliceRandom;

        // Select a random tip
        let mut rng = rand::thread_rng();
        self.current_tip = self.tips.choose(&mut rng).cloned();

        // Start 60-second cooldown
        let handle = ctx.spawn(
            async {
                warpui::r#async::Timer::after(Duration::from_secs(60)).await;
            },
            |me, _, _| {
                me.cooldown_handle = None;
            },
        );
        self.cooldown_handle = Some(handle);
        ctx.notify();
    }

    /// Resets the cooldown timer without changing the current tip.
    /// This ensures the current tip will be shown for the full cooldown period.
    pub fn reset_cooldown(&mut self, ctx: &mut ModelContext<Self>) {
        // Cancel any existing cooldown
        if let Some(handle) = self.cooldown_handle.take() {
            handle.abort();
        }

        // Start a new 60-second cooldown
        let handle = ctx.spawn(
            async {
                warpui::r#async::Timer::after(Duration::from_secs(60)).await;
            },
            |me, _, _| {
                me.cooldown_handle = None;
            },
        );
        self.cooldown_handle = Some(handle);
    }
}
