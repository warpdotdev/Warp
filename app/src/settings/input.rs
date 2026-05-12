use serde::{Deserialize, Serialize};
/// TODO: move alias_expansion setting into this group.
use settings::{define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud};
use std::collections::HashMap;
use warpui::{AppContext, SingletonEntity};

use crate::terminal::input::inline_menu::InlineMenuType;
use crate::terminal::session_settings::SessionSettings;
use settings::Setting as _;

pub const MAX_TIMES_TO_SHOW_AUTOSUGGESTION_HINT: i8 = 2;

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "Terminal input style.", rename_all = "snake_case")]
pub enum InputBoxType {
    /// AI-first input
    Universal,

    #[default]
    /// Terminal-first input
    Classic,
}

define_settings_group!(InputSettings,
    settings: [
        show_hint_text: ShowHintText {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.show_hint_text",
            description: "Whether hint text is shown in the terminal input.",
        },
        classic_completions_mode: ClassicCompletionsMode {
            type: bool,
            default: false,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.classic_completions_mode",
            description: "Whether classic completions mode is enabled.",
        },
        completions_open_while_typing: CompletionsOpenWhileTyping {
            type: bool,
            default: false,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.completions_open_while_typing",
            description: "Whether the completions menu opens automatically while typing.",
        },
        error_underlining: ErrorUnderliningEnabled {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::DESKTOP,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.error_underlining_enabled",
            description: "Whether command errors are underlined in the input.",
        },
        syntax_highlighting: SyntaxHighlighting {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::DESKTOP,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.syntax_highlighting",
            description: "Whether syntax highlighting is enabled in the terminal input.",
        },
        command_corrections: CommandCorrections {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.command_corrections",
            description: "Whether command corrections are suggested for mistyped commands.",
        },
        workflows_box_expanded: WorkflowsBoxExpanded {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: true,
            storage_key: "WorkflowsBoxOpen",
        },
        autosuggestion_accepted_count: AutosuggestionAcceptedCount {
            type: i8,
            default: 0,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: true,
        },
        input_box_type: InputBoxTypeSetting {
            type: InputBoxType,
            default: InputBoxType::Classic,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.input_box_type_setting",
            description: "The terminal input style.",
        },
        at_context_menu_in_terminal_mode: AtContextMenuInTerminalMode {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.at_context_menu_in_terminal_mode",
            description: "Whether the @ context menu is available in terminal mode.",
        },
        enable_slash_commands_in_terminal: EnableSlashCommandsInTerminal {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.enable_slash_commands_in_terminal",
            description: "Whether slash commands are available in the terminal input.",
        },
        outline_codebase_symbols_for_at_context_menu: OutlineCodebaseSymbolsForAtContextMenu {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.outline_codebase_symbols_for_at_context_menu",
            description: "Whether codebase symbols appear in the @ context menu.",
        },
        completions_menu_width: CompletionsMenuWidth {
            type: f32,
            default: 330.,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            private: true,
        },
        completions_menu_height: CompletionsMenuHeight {
            type: f32,
            default: 185.,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            private: true,
        },
        show_agent_tips: ShowAgentTips {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "agents.warp_agent.input.show_agent_tips",
            description: "Whether agent tips are displayed in the input.",
        },
        // Whether to show the terminal input message bar (contextual hints at the bottom of terminal input).
        // Only applicable when FeatureFlag::AgentView is enabled.
        show_terminal_input_message_bar: ShowTerminalInputMessageBar {
            type: bool,
            default: true,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.show_terminal_input_message_bar",
            description: "Whether the terminal input message bar is shown.",
        },
        // Per-menu custom content heights set by drag-to-resize. Not user-visible.
        inline_menu_custom_content_heights: InlineMenuCustomContentHeights {
            type: HashMap<InlineMenuType, f32>,
            default: HashMap::default(),
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            private: true,
        },
        command_input_min_lines: CommandInputMinLines {
            type: u8,
            default: 1,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "terminal.input.command_input_min_lines",
            description: "Minimum number of lines reserved for the command input field. Values are clamped to 1..=20.",
        },
    ]
);

impl CommandInputMinLines {
    /// Minimum allowed value for `command_input_min_lines`.
    pub const MIN: u8 = 1;
    /// Maximum allowed value for `command_input_min_lines`.
    ///
    /// Capped at 20 lines so that overly large values cannot crowd out the
    /// terminal block list. The render path also constrains the field to at
    /// most half the visible terminal height regardless of this setting.
    pub const MAX: u8 = 20;

    fn validate(&self, new_value: u8) -> u8 {
        clamp_command_input_min_lines(new_value)
    }

    /// Returns the validated current value of the setting.
    ///
    /// The stored value is always validated on load and on update, but reading
    /// through this accessor guarantees callers never see an out-of-range value
    /// even if persisted state ever drifts.
    pub fn effective(&self) -> u8 {
        clamp_command_input_min_lines(**self)
    }
}

/// Clamp a raw `command_input_min_lines` value into the supported range.
///
/// Exposed at module scope so it can be exercised in unit tests without
/// constructing a full settings group.
pub fn clamp_command_input_min_lines(value: u8) -> u8 {
    value.clamp(CommandInputMinLines::MIN, CommandInputMinLines::MAX)
}

/// Compute the pixel height to reserve for the input field given the user's
/// configured minimum and the editor's line height.
///
/// This is a pure function so the layout math can be unit-tested without
/// constructing an `AppContext`.
pub fn command_input_min_height_px(min_lines: u8, line_height_px: f32) -> f32 {
    let clamped = clamp_command_input_min_lines(min_lines);
    f32::from(clamped) * line_height_px.max(0.0)
}

impl InputSettings {
    pub fn input_type(&self, app: &AppContext) -> InputBoxType {
        let stored_input_type_value = &self.input_box_type;

        // Check if the user has explicitly set the InputBoxTypeSetting
        let computed_input_type_value = if stored_input_type_value.is_value_explicitly_set() {
            // User has explicitly set the value, use it
            **stored_input_type_value
        } else {
            // User hasn't set it explicitly, use our computed default.
            // If the user is in Preview or isn't using PS1, default to UDI.
            // TODO(CORE-3752): migrate unit and integration tests to pass with UDI instead of Classic
            let should_default_to_universal = (cfg!(feature = "preview_channel")
                || !*SessionSettings::as_ref(app).honor_ps1.value())
                && !cfg!(feature = "integration_tests")
                && !cfg!(test);

            if should_default_to_universal {
                InputBoxType::Universal
            } else {
                InputBoxType::Classic
            }
        };

        // PS1 input is only valid when honor_ps1 is active. If the user has PS1 selected
        // but the shell has not signalled PS1 support, fall back to Warp input.
        let is_ps1_enabled = *SessionSettings::as_ref(app).honor_ps1
            && computed_input_type_value == InputBoxType::Classic;
        if is_ps1_enabled {
            InputBoxType::Classic
        } else {
            InputBoxType::Universal
        }
    }

    pub fn is_universal_developer_input_enabled(&self, app: &AppContext) -> bool {
        self.input_type(app) == InputBoxType::Universal
    }

    pub fn is_classic_input_enabled(&self, app: &AppContext) -> bool {
        self.input_type(app) == InputBoxType::Classic
    }

    pub fn is_terminal_input_message_bar_enabled(&self) -> bool {
        *self.show_terminal_input_message_bar
    }
}

#[cfg(test)]
mod command_input_min_lines_tests {
    use super::*;

    #[test]
    fn clamp_keeps_value_within_default_range() {
        assert_eq!(
            clamp_command_input_min_lines(CommandInputMinLines::MIN),
            CommandInputMinLines::MIN,
        );
        assert_eq!(clamp_command_input_min_lines(5), 5);
        assert_eq!(
            clamp_command_input_min_lines(CommandInputMinLines::MAX),
            CommandInputMinLines::MAX,
        );
    }

    #[test]
    fn clamp_raises_subrange_value_to_min() {
        // u8 min is 0, but our setting min is 1.
        assert_eq!(clamp_command_input_min_lines(0), CommandInputMinLines::MIN);
    }

    #[test]
    fn clamp_caps_oversized_value_to_max() {
        assert_eq!(
            clamp_command_input_min_lines(CommandInputMinLines::MAX + 1),
            CommandInputMinLines::MAX,
        );
        assert_eq!(
            clamp_command_input_min_lines(u8::MAX),
            CommandInputMinLines::MAX,
        );
    }

    #[test]
    fn min_height_uses_clamped_lines() {
        let line_height = 18.0_f32;

        // Default (1 line) reserves a single line of height.
        assert_eq!(command_input_min_height_px(1, line_height), 18.0);

        // 5 lines reserves 5 lines of height.
        assert_eq!(command_input_min_height_px(5, line_height), 90.0);

        // Values above MAX get clamped down to MAX before multiplying.
        assert_eq!(
            command_input_min_height_px(u8::MAX, line_height),
            f32::from(CommandInputMinLines::MAX) * line_height,
        );

        // Values below MIN get clamped up to MIN.
        assert_eq!(command_input_min_height_px(0, line_height), 18.0);
    }

    #[test]
    fn min_height_handles_nonpositive_line_height() {
        // Defensive: a non-positive line height should yield 0, never negative.
        assert_eq!(command_input_min_height_px(3, 0.0), 0.0);
        assert_eq!(command_input_min_height_px(3, -10.0), 0.0);
    }
}
