use super::{
    AgentOnboardingVersion, AskAISource, ContextMenuAction, OnboardingIntention, OnboardingVersion,
    TerminalAction,
};

use crate::ai::predict::prompt_suggestions::ACCEPT_PROMPT_SUGGESTION_KEYBINDING;
use crate::server::telemetry::ToggleBlockFilterSource;

use crate::server::telemetry::InteractionSource;
use crate::settings_view::flags;
use crate::terminal::input::{
    SET_INPUT_MODE_AGENT_ACTION_NAME, SET_INPUT_MODE_TERMINAL_ACTION_NAME,
};
use crate::terminal::ssh::error::{SshErrorBlockAction, SSH_ERROR_BLOCK_VISIBLE_KEY};
use crate::terminal::view::passive_suggestions::PromptSuggestionResolution;
use crate::terminal::view::{
    LONG_RUNNING_AGENT_REQUESTED_COMMAND_CONTEXT_KEY,
    LONG_RUNNING_AGENT_REQUESTED_COMMAND_USER_TOOK_OVER_CONTEXT_KEY,
};
use crate::util::bindings;
use crate::util::bindings::{cmd_or_ctrl_shift, is_binding_pty_compliant};
use crate::{
    channel::{Channel, ChannelState},
    features::FeatureFlag,
    terminal::model::{
        escape_sequences::{self, EscCodes},
        selection::SelectionDirection,
    },
    terminal::shared_session::SharedSessionStatus,
    terminal::TerminalView,
    util::bindings::CustomAction,
};
use warpui::keymap::ContextPredicate;
use warpui::keymap::{BindingDescription, PerPlatformKeystroke};
use warpui::platform::OperatingSystem;
use warpui::{
    keymap::{EditableBinding, FixedBinding},
    units::IntoLines,
    AppContext,
};

pub const TOGGLE_BLOCK_FILTER_KEYBINDING: &str =
    "terminal:toggle_block_filter_on_selected_or_last_block";

pub const CANCEL_COMMAND_KEYBINDING: &str = "terminal:cancel_command";
pub const TOGGLE_AUTOEXECUTE_MODE_KEYBINDING: &str = "terminal:toggle_autoexecute_mode";
pub const TOGGLE_QUEUE_NEXT_PROMPT_KEYBINDING: &str = "terminal:toggle_queue_next_prompt";
pub const TOGGLE_HIDE_CLI_RESPONSES_KEYBINDING: &str = "terminal:toggle_hide_cli_responses";
pub const OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING: &str = "terminal:open_cli_agent_rich_input";

const SELECT_NEXT_BLOCK_ACTION_NAME: &str = "terminal:select_next_block";
pub const SELECT_PREVIOUS_BLOCK_ACTION_NAME: &str = "terminal:select_previous_block";

pub const CAN_RESUME_CONVERSATION_KEY: &str = "CanResumeConversation";
pub const CAN_FORK_FROM_LAST_KNOWN_GOOD_STATE_KEY: &str = "CanForkFromLastKnownGoodState";

pub const INPUT_BOX_VISIBLE_KEY: &str = "InputVisible";
pub const KEYBOARD_PROTOCOL_ENABLED_KEY: &str = "KeyboardProtocolEnabled";
pub const CLI_AGENT_SESSION_ACTIVE_KEY: &str = "CLIAgentSessionActive";
pub const ROOT_AMBIENT_AGENT_PANE_KEY: &str = "RootAmbientAgentPane";

/// Some keybindings will do different things in different contexts. We break
/// these into their own function to ensure we pay special attention to
/// these overlaps, and ensure only 1 action is taken.
fn init_overlapping_keybindings(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    let escape_key: &str = "escape";
    let cmd_or_ctrl_enter: &str = "cmdorctrl-enter";

    // No Active Block Context
    app.register_fixed_bindings([FixedBinding::new(
        escape_key,
        TerminalAction::MaybeDismissToolTip {
            from_keybinding: true,
        },
        !id!(SSH_ERROR_BLOCK_VISIBLE_KEY) & id!("Terminal"),
    )]);

    let block_action_context = || id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand");

    // SSH Error Block Context
    app.register_fixed_bindings([
        FixedBinding::new(
            escape_key,
            TerminalAction::NotifySshErrorBlock(SshErrorBlockAction::ContinueWithoutWarpification),
            id!(SSH_ERROR_BLOCK_VISIBLE_KEY) & block_action_context(),
        ),
        FixedBinding::new(
            cmd_or_ctrl_enter,
            TerminalAction::NotifySshErrorBlock(SshErrorBlockAction::ContinueWithoutWarpification),
            id!(SSH_ERROR_BLOCK_VISIBLE_KEY) & block_action_context(),
        ),
    ]);
}

/// Register keybindings for [`TerminalView`] actions.
pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_binding_validator::<TerminalView>(is_binding_pty_compliant);

    init_overlapping_keybindings(app);
    // Register input mode bindings before warpify bindings so ctrl-i warpifies
    // instead of opening inline agent when a warpify banner is visible.
    register_input_mode_bindings(app);

    app.register_fixed_bindings([
        FixedBinding::new("up", TerminalAction::Up, id!("Terminal") & !id!("IMEOpen")),
        FixedBinding::new(
            "down",
            TerminalAction::Down,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "left",
            TerminalAction::UserInputSequence(vec![EscCodes::ARROW_LEFT]),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "right",
            TerminalAction::UserInputSequence(vec![EscCodes::ARROW_RIGHT]),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "home",
            TerminalAction::Home,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "end",
            TerminalAction::End,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "shift-enter",
            TerminalAction::KeyDown("\n".to_owned()),
            id!("Terminal")
                & !id!("IMEOpen")
                & (id!("LongRunningCommand") | id!("AltScreen"))
                & !id!(KEYBOARD_PROTOCOL_ENABLED_KEY),
        ),
        FixedBinding::new(
            "numpadenter",
            TerminalAction::KeyDown("\r".to_owned()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "backspace",
            TerminalAction::ControlSequence("\x7f".as_bytes().to_vec()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "insert",
            TerminalAction::ControlSequence("\x1b[2~".as_bytes().to_vec()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "delete",
            TerminalAction::ControlSequence("\x1b[3~".as_bytes().to_vec()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "pageup",
            TerminalAction::PageUp,
            id!("Terminal") & !id!("IMEOpen") & !id!("EditorFocused"),
        )
        .with_command_description(crate::t!("keybinding-desc-terminal-scroll-up-one-page")),
        FixedBinding::new(
            "pagedown",
            TerminalAction::PageDown,
            id!("Terminal") & !id!("IMEOpen") & !id!("EditorFocused"),
        )
        .with_command_description(crate::t!("keybinding-desc-terminal-scroll-down-one-page")),
        // Resume conversation keybinding
        FixedBinding::new_per_platform(
            PerPlatformKeystroke {
                mac: "cmd-shift-R",
                linux_and_windows: "ctrl-alt-r",
            },
            TerminalAction::ResumeConversation,
            id!("Terminal") & !id!("IMEOpen") & id!(CAN_RESUME_CONVERSATION_KEY),
        ),
        // Fork from the last known good exchange keybinding
        FixedBinding::new_per_platform(
            PerPlatformKeystroke {
                mac: "cmd-alt-y",
                linux_and_windows: "ctrl-alt-y",
            },
            TerminalAction::ForkConversationFromLastKnownGoodState,
            id!("Terminal") & !id!("IMEOpen") & id!(CAN_FORK_FROM_LAST_KNOWN_GOOD_STATE_KEY),
        ),
        // Toggle AI document pane
        FixedBinding::new(
            "cmdorctrl-alt-p",
            TerminalAction::ToggleAIDocumentPane,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        // On the web, we get pastes from system paste events.
        #[cfg(target_family = "wasm")]
        FixedBinding::standard(
            warpui::actions::StandardAction::Paste,
            TerminalAction::Paste,
            id!("Terminal") & !id!("IMEOpen"),
        ),
    ]);
    if cfg!(target_os = "macos") {
        // On MacOS, if the user has the 'Option as meta' setting enabled, the cmd-alt-y binding
        // above will not match.
        app.register_fixed_bindings([FixedBinding::new(
            "cmd-meta-y",
            TerminalAction::ForkConversationFromLastKnownGoodState,
            id!("Terminal") & !id!("IMEOpen") & id!(CAN_FORK_FROM_LAST_KNOWN_GOOD_STATE_KEY),
        )]);
    }

    // Register binding to toggle plans in agent conversations.
    {
        app.register_fixed_bindings([FixedBinding::new(
            "cmdorctrl-alt-p",
            TerminalAction::ToggleAIDocumentPane,
            id!("Terminal") & !id!("IMEOpen"),
        )]);
        if cfg!(target_os = "macos") {
            // On MacOS, if the user has the 'Option as meta' setting enabled, the cmd-alt-p binding
            // above will not match.
            //
            // TODO(zachbai): Consider if, for the purposes of fixed bindings, alt/meta should work
            // fungibly regardless of underlying setting.
            app.register_fixed_bindings([FixedBinding::new(
                "cmd-meta-p",
                TerminalAction::ToggleAIDocumentPane,
                id!("Terminal") & !id!("IMEOpen"),
            )]);
        }
    }

    if ChannelState::channel() == Channel::Integration {
        app.register_fixed_bindings([
            // Hack: Add explicit bindings for the tests, since the tests' injected
            // keypresses won't trigger Mac menu items. Unfortunately we can't use
            // cfg[test] because we are a separate process!
            FixedBinding::new(
                cmd_or_ctrl_shift("l"),
                TerminalAction::FocusInputAndClearSelection,
                id!("Terminal"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("f"),
                TerminalAction::ShowFindBar,
                id!("Terminal"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("k"),
                TerminalAction::ClearBuffer,
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("d"),
                TerminalAction::SplitRight(None),
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new_per_platform(
                PerPlatformKeystroke {
                    mac: "cmd-shift-D",
                    linux_and_windows: "ctrl-shift-E",
                },
                TerminalAction::SplitDown(None),
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("v"),
                TerminalAction::Paste,
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("c"),
                TerminalAction::Copy,
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("i"),
                TerminalAction::SetInputModeAgent,
                id!("Terminal")
                    & !id!("IMEOpen")
                    & (!id!(flags::AGENT_VIEW_ENABLED)
                        | id!(flags::ACTIVE_AGENT_VIEW)
                        | id!(flags::ACTIVE_INLINE_AGENT_VIEW)),
            ),
        ]);
    }

    // By default, Windows Terminal recognizes both `ctrl-v` and `ctrl-shift-v` to paste into the
    // terminal. It also allows users to disable it, so we also make this an EditableBinding.
    #[cfg(windows)]
    app.register_editable_bindings([EditableBinding::new(
        "terminal:alternate_terminal_paste",
        crate::t!("keybinding-desc-terminal-alternate-paste"),
        TerminalAction::Paste,
    )
    .with_key_binding("ctrl-v")
    .with_context_predicate(id!("Terminal") & !id!("IMEOpen"))]);

    app.register_fixed_bindings([
        FixedBinding::new(
            "shift-left",
            TerminalAction::KeyboardSelectText(SelectionDirection::Left),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
        FixedBinding::new(
            "shift-right",
            TerminalAction::KeyboardSelectText(SelectionDirection::Right),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
        FixedBinding::new(
            "shift-up",
            TerminalAction::KeyboardSelectText(SelectionDirection::Up),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
        FixedBinding::new(
            "shift-down",
            TerminalAction::KeyboardSelectText(SelectionDirection::Down),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
    ]);

    app.register_editable_bindings([
        // Ctrl-G: open rich input for CLI agents before the keystroke reaches the PTY.
        EditableBinding::new(
            OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING,
            crate::t!("keybinding-desc-terminal-toggle-cli-agent-rich-input"),
            TerminalAction::OpenCLIAgentRichInput,
        )
        .with_key_binding("ctrl-g")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & (id!("LongRunningCommand") | id!("AltScreen"))
                & id!(flags::CLI_AGENT_FOOTER_ENABLED)
                & id!(flags::CLI_AGENT_RICH_INPUT_CHIP_ENABLED),
        ),
        EditableBinding::new(
            "terminal:warpify_subshell",
            crate::t!("keybinding-desc-terminal-warpify-subshell"),
            TerminalAction::TriggerSubshellBootstrap,
        )
        .with_key_binding("ctrl-i")
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand") & id!("SubshellBanner"),
        ),
        EditableBinding::new(
            "terminal:warpify_ssh_session",
            crate::t!("keybinding-desc-terminal-warpify-ssh-session"),
            TerminalAction::WarpifySSHSession,
        )
        .with_key_binding("ctrl-i")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & id!("LongRunningCommand")
                & id!("SshWarpificationBanner"),
        ),
        EditableBinding::new(
            ACCEPT_PROMPT_SUGGESTION_KEYBINDING,
            crate::t!("keybinding-desc-terminal-accept-prompt-suggestion"),
            TerminalAction::ResolvePromptSuggestion(PromptSuggestionResolution::Accept {
                interaction_source: InteractionSource::Keybinding,
            }),
        )
        .with_mac_key_binding(if FeatureFlag::AgentView.is_enabled() {
            "ctrl-enter"
        } else {
            "cmd-enter"
        })
        .with_linux_or_windows_key_binding(if FeatureFlag::AgentView.is_enabled() {
            "alt-shift-enter"
        } else {
            "ctrl-shift-enter"
        })
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & id!(flags::HAS_PENDING_PROMPT_SUGGESTION),
        ),
        EditableBinding::new(
            CANCEL_COMMAND_KEYBINDING,
            if cfg!(windows) {
                crate::t!("keybinding-desc-terminal-cancel-process-windows")
            } else {
                crate::t!("keybinding-desc-terminal-cancel-process")
            },
            TerminalAction::CtrlC,
        )
        .with_key_binding("ctrl-c")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:focus_input",
            crate::t!("keybinding-desc-terminal-focus-input"),
            TerminalAction::FocusInputAndClearSelection,
        )
        .with_custom_action(CustomAction::FocusInput)
        .with_context_predicate(id!("Terminal")),
        // Paste is not rebindable on the web.
        #[cfg(not(target_family = "wasm"))]
        EditableBinding::new(
            "terminal:paste",
            crate::t!("keybinding-desc-terminal-paste"),
            TerminalAction::Paste,
        )
        .with_custom_action(CustomAction::Paste)
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:copy",
            crate::t!("keybinding-desc-terminal-copy"),
            TerminalAction::Copy,
        )
        .with_custom_action(CustomAction::Copy)
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:reinput_commands",
            crate::t!("keybinding-desc-terminal-reinput-commands"),
            TerminalAction::ReinputCommands,
        )
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:reinput_commands_with_sudo",
            crate::t!("keybinding-desc-terminal-reinput-commands-sudo"),
            TerminalAction::ReinputCommandsWithSudo,
        )
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:find",
            crate::t!("keybinding-desc-terminal-find"),
            TerminalAction::ShowFindBar,
        )
        .with_key_binding(cmd_or_ctrl_shift("f"))
        .with_custom_action(CustomAction::Find)
        .with_context_predicate(id!("Terminal")),
        EditableBinding::new(
            "terminal:select_bookmark_up",
            crate::t!("keybinding-desc-terminal-select-bookmark-up"),
            TerminalAction::SelectBookmarkUp,
        )
        .with_key_binding("alt-up")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:select_bookmark_down",
            crate::t!("keybinding-desc-terminal-select-bookmark-down"),
            TerminalAction::SelectBookmarkDown,
        )
        .with_key_binding("alt-down")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:open_block_list_context_menu_via_keybinding",
            crate::t!("keybinding-desc-terminal-open-block-context-menu"),
            TerminalAction::OpenBlockListContextMenu,
        )
        .with_mac_key_binding("ctrl-m")
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:toggle_teams_modal",
            crate::t!("keybinding-desc-terminal-toggle-team-workflows-modal"),
            TerminalAction::OpenWorkflowModal,
        )
        .with_key_binding(cmd_or_ctrl_shift("s"))
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy_git_branch",
            crate::t!("keybinding-desc-terminal-copy-git-branch"),
            TerminalAction::CopyGitBranch,
        )
        .with_context_predicate(
            id!("Terminal")
                & (eq!("TerminalView_BlockSelectionCardinality", "One")
                    | eq!("TerminalView_BlockSelectionCardinality", "None")),
        ),
        EditableBinding::new(
            "terminal:clear_blocks",
            crate::t!("keybinding-desc-terminal-clear-blocks"),
            TerminalAction::ClearBuffer,
        )
        .with_custom_action(CustomAction::ClearBlocks)
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
        ),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_word_left",
            crate::t!("keybinding-desc-terminal-cursor-word-left"),
            TerminalAction::ControlSequence(Vec::from(EscCodes::WORD_LEFT)),
        )
        .with_mac_key_binding("alt-left")
        .with_linux_or_windows_key_binding("ctrl-left")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_word_right",
            crate::t!("keybinding-desc-terminal-cursor-word-right"),
            TerminalAction::ControlSequence(Vec::from(EscCodes::WORD_RIGHT)),
        )
        .with_mac_key_binding("alt-right")
        .with_linux_or_windows_key_binding("ctrl-right")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_home",
            crate::t!("keybinding-desc-terminal-cursor-home"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::SOH]),
        )
        // We already have bindings for home/end (the keybindings for this on Linux and Mac) that
        // send the correct control sequence to the PTY.
        .with_mac_key_binding("cmd-left")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_end",
            crate::t!("keybinding-desc-terminal-cursor-end"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::ENQ]),
        )
        .with_mac_key_binding("cmd-right")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_delete_word_left",
            crate::t!("keybinding-desc-terminal-delete-word-left"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::ETB]),
        )
        .with_mac_key_binding("alt-backspace")
        .with_linux_or_windows_key_binding("ctrl-backspace")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_delete_line_start",
            crate::t!("keybinding-desc-terminal-delete-line-start"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::NAK]),
        )
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand"))
        // Set this for mac-only. The default binding for this on Linux / Windows is `ctrl-y`, which
        // we can't hijack because it is already reserved for the PTY.
        .with_mac_key_binding("cmd-backspace"),
        EditableBinding::new(
            "terminal:executing_command_delete_line_end",
            crate::t!("keybinding-desc-terminal-delete-line-end"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::VT]),
        )
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand"))
        // Set this for mac-only since the corresponding editor action is also Mac-only.
        .with_mac_key_binding("cmd-delete"),
        EditableBinding::new(
            "terminal:backward_tabulation",
            crate::t!("keybinding-desc-terminal-backward-tabulation"),
            TerminalAction::ControlSequence(EscCodes::build_escape_sequence_with_c1(
                escape_sequences::C1::CSI,
                EscCodes::BACKWARD_TABULATION,
            )),
        )
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & (id!("LongRunningCommand") | id!("AltScreen")),
        )
        .with_key_binding("shift-tab"),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            SELECT_PREVIOUS_BLOCK_ACTION_NAME,
            crate::t!("keybinding-desc-terminal-select-previous-block"),
            TerminalAction::SelectPriorBlock,
        )
        .with_custom_action(CustomAction::SelectBlockAbove)
        .with_context_predicate(
            id!("Terminal") & id!("TerminalView_NonEmptyBlockList") & !id!("AltScreen"),
        ),
        EditableBinding::new(
            SELECT_NEXT_BLOCK_ACTION_NAME,
            crate::t!("keybinding-desc-terminal-select-next-block"),
            TerminalAction::SelectNextBlock,
        )
        .with_custom_action(CustomAction::SelectBlockBelow)
        .with_context_predicate(
            id!("Terminal") & id!("TerminalView_NonEmptyBlockList") & !id!("AltScreen"),
        ),
        // OpenWarp:删除 terminal:open_share_block_modal keybinding(云端 share block)
        EditableBinding::new(
            "terminal:bookmark_selected_block",
            crate::t!("keybinding-desc-terminal-bookmark-selected-block"),
            TerminalAction::BookmarkSelectedBlock,
        )
        .with_custom_action(CustomAction::ToggleBookmarkBlock)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:find",
            crate::t!("keybinding-desc-terminal-find-within-selected-block"),
            TerminalAction::ShowFindBar,
        )
        .with_custom_action(CustomAction::FindWithinBlock)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy",
            crate::t!("keybinding-desc-terminal-copy-command-and-output"),
            TerminalAction::Copy,
        )
        .with_custom_action(CustomAction::CopyBlock)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy_outputs",
            crate::t!("keybinding-desc-terminal-copy-command-output"),
            TerminalAction::CopyOutputs,
        )
        .with_custom_action(CustomAction::CopyBlockOutput)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy_commands",
            crate::t!("keybinding-desc-terminal-copy-command"),
            TerminalAction::CopyCommands,
        )
        .with_custom_action(CustomAction::CopyBlockCommand)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:scroll_up_one_line",
            crate::t!("keybinding-desc-terminal-scroll-up-one-line"),
            TerminalAction::Scroll {
                delta: 1.0.into_lines(),
            },
        )
        .with_context_predicate(id!("Terminal") & id!("TerminalView_NonEmptyBlockList")),
        EditableBinding::new(
            "terminal:scroll_down_one_line",
            crate::t!("keybinding-desc-terminal-scroll-down-one-line"),
            TerminalAction::Scroll {
                delta: -(1.0.into_lines()),
            },
        )
        .with_context_predicate(id!("Terminal") & id!("TerminalView_NonEmptyBlockList")),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:scroll_up_one_page",
            crate::t!("keybinding-desc-terminal-scroll-up-one-page"),
            TerminalAction::PageUp,
        )
        .with_key_binding("pageup")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & id!("TerminalView_NonEmptyBlockList")
                & !id!("EditorFocused"),
        ),
        EditableBinding::new(
            "terminal:scroll_down_one_page",
            crate::t!("keybinding-desc-terminal-scroll-down-one-page"),
            TerminalAction::PageDown,
        )
        .with_key_binding("pagedown")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & id!("TerminalView_NonEmptyBlockList")
                & !id!("EditorFocused"),
        ),
    ]);

    app.register_editable_bindings([EditableBinding::new(
        "terminal:scroll_to_top_of_selected_block",
        crate::t!("keybinding-desc-terminal-scroll-to-top-of-block"),
        TerminalAction::ScrollToTopOfSelectedBlocks,
    )
    .with_custom_action(CustomAction::ScrollToTopOfSelectedBlocks)
    .with_context_predicate(
        id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
    )]);
    app.register_editable_bindings([EditableBinding::new(
        "terminal:scroll_to_bottom_of_selected_block",
        crate::t!("keybinding-desc-terminal-scroll-to-bottom-of-block"),
        TerminalAction::ScrollToBottomOfSelectedBlocks,
    )
    .with_custom_action(CustomAction::ScrollToBottomOfSelectedBlocks)
    .with_context_predicate(
        id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
    )]);

    // Register a mac only keybinding for selecting all blocks that uses the "Select All" mac menu
    // item. We don't want this registered on Linux/Windows since this would mean the binding needs
    // to be "PTY compliant", which would end up making select all have a binding of `ctrl-shift-a`
    // instead of `ctrl-a` within the editor view.
    if OperatingSystem::get().is_mac() {
        app.register_editable_bindings([
            // Note that we register a separate action for SelectAll blocks
            // that always works, regardless of context - this one is triggered
            // from the menus and doesn't conflict with cmd-A in the editor.
            EditableBinding::new(
                "terminal:select_all_blocks",
                crate::t!("keybinding-desc-terminal-select-all-blocks"),
                TerminalAction::SelectAllBlocks,
            )
            .with_context_predicate(
                id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
            )
            .with_custom_action(CustomAction::SelectAll),
            EditableBinding::new(
                "terminal:select_all_blocks",
                crate::t!("keybinding-desc-terminal-select-all-blocks"),
                TerminalAction::SelectAllBlocks,
            )
            .with_context_predicate(
                id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
            )
            .with_custom_action(CustomAction::SelectAllBlocks),
        ]);
    } else {
        app.register_editable_bindings([EditableBinding::new(
            "terminal:select_all_blocks",
            crate::t!("keybinding-desc-terminal-select-all-blocks"),
            TerminalAction::SelectAllBlocks,
        )
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
        )])
    }

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:expand_block_selection_above",
            crate::t!("keybinding-desc-terminal-expand-blocks-above"),
            TerminalAction::ExpandBlockSelectionAbove,
        )
        .with_key_binding("shift-up")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & !id!("ActiveBlockTextSelection")
                & !id!("AltScreen"),
        ),
        EditableBinding::new(
            "terminal:expand_block_selection_below",
            crate::t!("keybinding-desc-terminal-expand-blocks-below"),
            TerminalAction::ExpandBlockSelectionBelow,
        )
        .with_key_binding("shift-down")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & !id!("ActiveBlockTextSelection")
                & !id!("AltScreen"),
        ),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:ask_ai_assistant",
            BindingDescription::new(crate::t!(
                "keybinding-desc-terminal-attach-block-as-context"
            ))
            .with_custom_description(
                bindings::MAC_MENUS_CONTEXT,
                crate::t!("keybinding-desc-terminal-attach-as-context-menu"),
            ),
            TerminalAction::ContextMenu(ContextMenuAction::AskAI(AskAISource::SelectedBlocks)),
        )
        .with_enabled(|| FeatureFlag::AgentMode.is_enabled())
        .with_custom_action(CustomAction::AttachSelectionAsAgentModeContext)
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        // When possible, prioritize the text selection action over attaching a block as
        // context.
        .with_context_predicate(
            id!("Terminal")
                & ne!("TerminalView_BlockSelectionCardinality", "None")
                & !id!("ActiveBlockTextSelection")
                & !id!("ActiveAltScreenSelection")
                & id!(flags::IS_ANY_AI_ENABLED),
        ),
        EditableBinding::new(
            "terminal:ask_ai_assistant",
            BindingDescription::new(crate::t!("keybinding-desc-terminal-attach-text-as-context"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    crate::t!("keybinding-desc-terminal-attach-as-context-menu"),
                ),
            TerminalAction::ContextMenu(ContextMenuAction::AskAI(
                AskAISource::SelectedTerminalText,
            )),
        )
        .with_enabled(|| FeatureFlag::AgentMode.is_enabled())
        .with_custom_action(CustomAction::AttachSelectionAsAgentModeContext)
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(
            id!("Terminal")
                & (id!("ActiveBlockTextSelection") | id!("ActiveAltScreenSelection"))
                & id!(flags::IS_ANY_AI_ENABLED),
        ),
        // We register a single binding for either a selected block or selected text
        // to avoid cluttering the keybindings UI. At the end of the day, these
        // map to the same logic, and we should be able to distinguish whether
        // this is a block selection or text selection later on.
        EditableBinding::new(
            "terminal:ask_ai_assistant",
            crate::t!("keybinding-desc-terminal-ask-ai-selection"),
            TerminalAction::ContextMenu(ContextMenuAction::AskAI(AskAISource::SelectedBlockOrText)),
        )
        .with_enabled(|| !FeatureFlag::AgentMode.is_enabled())
        .with_custom_action(CustomAction::AttachSelectionAsAgentModeContext)
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(
            id!("Terminal")
                & id!(flags::IS_ANY_AI_ENABLED)
                & (eq!("TerminalView_BlockSelectionCardinality", "One")
                    | id!("ActiveBlockTextSelection")
                    | id!("ActiveAltScreenSelection")),
        ),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:ask_ai_assistant_last_block",
            crate::t!("keybinding-desc-terminal-ask-ai-last-block"),
            TerminalAction::ContextMenu(ContextMenuAction::AskAI(AskAISource::LastBlock)),
        )
        .with_enabled(|| !FeatureFlag::AgentMode.is_enabled())
        .with_key_binding("ctrl-shift->")
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(
            id!("Terminal") & id!("TerminalView_NonEmptyBlockList") & id!(flags::IS_ANY_AI_ENABLED),
        ),
        EditableBinding::new(
            "terminal:ask_ai_assistant",
            crate::t!("keybinding-desc-terminal-ask-ai"),
            TerminalAction::ContextMenu(ContextMenuAction::AskAI(AskAISource::SelectedInputText)),
        )
        .with_enabled(|| !FeatureFlag::AgentMode.is_enabled())
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_key_binding("ctrl-shift-space")
        .with_context_predicate(id!("Input") & id!(flags::IS_ANY_AI_ENABLED)),
    ]);

    if FeatureFlag::CommandCorrectionKey.is_enabled() {
        app.register_editable_bindings([EditableBinding::new(
            "input:insert_command_correction",
            crate::t!("keybinding-desc-terminal-insert-command-correction"),
            TerminalAction::InsertMostRecentCommandCorrection,
        )
        .with_context_predicate(id!("Terminal"))]);
    }

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:onboarding_flow",
            crate::t!("keybinding-desc-terminal-setup-guide"),
            TerminalAction::OnboardingFlow(OnboardingVersion::Legacy),
        )
        .with_context_predicate(
            id!("Terminal") & id!(SharedSessionStatus::NotShared.as_keymap_context()),
        ),
        // UniversalInput callout debug bindings
        EditableBinding::new(
            "terminal:agent_onboarding_flow_legacy_terminal",
            crate::t!("keybinding-desc-terminal-onboarding-warp-input-terminal"),
            TerminalAction::OnboardingFlow(OnboardingVersion::Legacy),
        )
        .with_enabled(|| {
            FeatureFlag::AgentOnboarding.is_enabled() && ChannelState::enable_debug_features()
        })
        .with_context_predicate(
            id!("Terminal") & id!(SharedSessionStatus::NotShared.as_keymap_context()),
        ),
        EditableBinding::new(
            "terminal:agent_onboarding_flow_universal_input_project",
            crate::t!("keybinding-desc-terminal-onboarding-warp-input-project"),
            TerminalAction::OnboardingFlow(OnboardingVersion::Agent(
                AgentOnboardingVersion::UniversalInput { has_project: true },
            )),
        )
        .with_enabled(|| {
            FeatureFlag::AgentOnboarding.is_enabled() && ChannelState::enable_debug_features()
        })
        .with_context_predicate(
            id!("Terminal") & id!(SharedSessionStatus::NotShared.as_keymap_context()),
        ),
        EditableBinding::new(
            "terminal:agent_onboarding_flow_universal_input_no_project",
            crate::t!("keybinding-desc-terminal-onboarding-warp-input-no-project"),
            TerminalAction::OnboardingFlow(OnboardingVersion::Agent(
                AgentOnboardingVersion::UniversalInput { has_project: false },
            )),
        )
        .with_enabled(|| {
            FeatureFlag::AgentOnboarding.is_enabled() && ChannelState::enable_debug_features()
        })
        .with_context_predicate(
            id!("Terminal") & id!(SharedSessionStatus::NotShared.as_keymap_context()),
        ),
        // AgentModality callout debug bindings
        EditableBinding::new(
            "terminal:agent_onboarding_flow_modality_project",
            crate::t!("keybinding-desc-terminal-onboarding-modality-project"),
            TerminalAction::OnboardingFlow(OnboardingVersion::Agent(
                AgentOnboardingVersion::AgentModality {
                    has_project: true,
                    intention: OnboardingIntention::AgentDrivenDevelopment,
                },
            )),
        )
        .with_enabled(|| {
            FeatureFlag::AgentOnboarding.is_enabled() && ChannelState::enable_debug_features()
        })
        .with_context_predicate(
            id!("Terminal") & id!(SharedSessionStatus::NotShared.as_keymap_context()),
        ),
        EditableBinding::new(
            "terminal:agent_onboarding_flow_modality_no_project",
            crate::t!("keybinding-desc-terminal-onboarding-modality-no-project"),
            TerminalAction::OnboardingFlow(OnboardingVersion::Agent(
                AgentOnboardingVersion::AgentModality {
                    has_project: false,
                    intention: OnboardingIntention::AgentDrivenDevelopment,
                },
            )),
        )
        .with_enabled(|| {
            FeatureFlag::AgentOnboarding.is_enabled() && ChannelState::enable_debug_features()
        })
        .with_context_predicate(
            id!("Terminal") & id!(SharedSessionStatus::NotShared.as_keymap_context()),
        ),
        EditableBinding::new(
            "terminal:agent_onboarding_flow_modality_terminal",
            crate::t!("keybinding-desc-terminal-onboarding-modality-terminal"),
            TerminalAction::OnboardingFlow(OnboardingVersion::Agent(
                AgentOnboardingVersion::AgentModality {
                    has_project: false,
                    intention: OnboardingIntention::Terminal,
                },
            )),
        )
        .with_enabled(|| {
            FeatureFlag::AgentOnboarding.is_enabled() && ChannelState::enable_debug_features()
        })
        .with_context_predicate(
            id!("Terminal") & id!(SharedSessionStatus::NotShared.as_keymap_context()),
        ),
    ]);

    app.register_editable_bindings([EditableBinding::new(
        "workspace:open_settings_import_page",
        crate::t!("keybinding-desc-terminal-import-external-settings"),
        TerminalAction::ImportSettings,
    )
    .with_context_predicate(id!("Terminal") & id!(flags::HAS_SETTINGS_TO_IMPORT_FLAG))]);

    // OpenWarp:删除 terminal:share_current_session / terminal:stop_sharing_current_session keybindings(云端 shared session)

    app.register_editable_bindings([EditableBinding::new(
        TOGGLE_BLOCK_FILTER_KEYBINDING,
        crate::t!("keybinding-desc-terminal-toggle-block-filter"),
        TerminalAction::ToggleBlockFilterOnSelectedOrLastBlock(ToggleBlockFilterSource::Binding),
    )
    .with_mac_key_binding("shift-alt-F")
    .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & !id!("AltScreen"))]);

    app.register_editable_bindings([EditableBinding::new(
        "terminal:toggle_snackbar_in_active_pane",
        crate::t!("keybinding-desc-terminal-toggle-sticky-command-header"),
        TerminalAction::ToggleSnackbarInActivePane,
    )
    .with_context_predicate(id!("Terminal"))]);

    app.register_editable_bindings([
        EditableBinding::new(
            TOGGLE_AUTOEXECUTE_MODE_KEYBINDING,
            crate::t!("keybinding-desc-terminal-toggle-autoexecute-mode"),
            TerminalAction::ToggleAutoexecuteMode,
        )
        .with_key_binding("cmdorctrl-shift-I")
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(id!(flags::IS_ANY_AI_ENABLED) & id!("Terminal"))
        .with_enabled(|| FeatureFlag::FastForwardAutoexecuteButton.is_enabled()),
        EditableBinding::new(
            TOGGLE_QUEUE_NEXT_PROMPT_KEYBINDING,
            crate::t!("keybinding-desc-terminal-toggle-queue-next-prompt"),
            TerminalAction::ToggleQueueNextPrompt,
        )
        .with_key_binding("cmdorctrl-shift-J")
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(id!(flags::IS_ANY_AI_ENABLED) & id!("Terminal"))
        .with_enabled(|| FeatureFlag::QueueSlashCommand.is_enabled()),
    ]);

    app.register_fixed_bindings(vec![
        FixedBinding::new(
            "cmdorctrl-1",
            TerminalAction::SelectAgenticSuggestion(1),
            id!("Terminal") & id!("OnboardingAgenticSuggestionsBlock"),
        ),
        FixedBinding::new(
            "cmdorctrl-2",
            TerminalAction::SelectAgenticSuggestion(2),
            id!("Terminal") & id!("OnboardingAgenticSuggestionsBlock"),
        ),
        FixedBinding::new(
            "cmdorctrl-3",
            TerminalAction::SelectAgenticSuggestion(3),
            id!("Terminal") & id!("OnboardingAgenticSuggestionsBlock"),
        ),
        FixedBinding::new(
            "cmdorctrl-4",
            TerminalAction::SelectAgenticSuggestion(4),
            id!("Terminal") & id!("OnboardingAgenticSuggestionsBlock"),
        ),
    ]);

    app.register_editable_bindings([EditableBinding::new(
        "terminal:load_agent_mode_conversation",
        crate::t!("keybinding-desc-terminal-load-agent-conversation"),
        TerminalAction::LoadAgentModeConversation,
    )
    .with_enabled(ChannelState::enable_debug_features)
    .with_context_predicate(id!("Terminal"))]);

    app.register_editable_bindings([EditableBinding::new(
        "terminal:toggle_session_recording",
        crate::t!("keybinding-desc-terminal-toggle-session-recording"),
        TerminalAction::ToggleSessionRecording,
    )
    .with_enabled(|| cfg!(feature = "local_fs") && ChannelState::enable_debug_features())
    .with_context_predicate(id!("Terminal"))]);

    app.register_editable_bindings([EditableBinding::new(
        "workspace:add_current_dir_as_project",
        BindingDescription::new(crate::t!("keybinding-desc-workspace-add-current-folder")),
        TerminalAction::AddProjectAtCurrentDirectory,
    )
    .with_enabled(|| FeatureFlag::Projects.is_enabled())
    .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))]);
}

/// Registers bindings related to input modes.
fn register_input_mode_bindings(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    // A context predicate that matches when the input mode bindings are
    // available for use. Disabled when a CLI agent session is active — the
    // Warp agent should not be tagged into a CLI agent's command, and the
    // `!` prefix is the only way to toggle shell mode in the rich input.
    let base_context = id!(flags::IS_ANY_AI_ENABLED)
        & (id!("Input") | id!("Terminal"))
        & !id!("SubshellBanner")
        & !id!(CLI_AGENT_SESSION_ACTIVE_KEY);

    // A context predicate that is active when the user can switch input to agent mode.
    let agent_mode_predicate = base_context.clone()
        & ContextPredicate::Or(
            Box::new(id!(flags::TERMINAL_MODE_INPUT)),
            Box::new(ContextPredicate::Or(
                Box::new(
                    !id!(flags::TERMINAL_MODE_INPUT)
                        & id!(LONG_RUNNING_AGENT_REQUESTED_COMMAND_USER_TOOK_OVER_CONTEXT_KEY),
                ),
                Box::new(id!("LongRunningCommand") | id!("AltScreen")),
            )),
        );

    // A context predicate that is active when the user could switch input to shell mode.
    // This matches when in AI mode AND either:
    // - AgentView feature is disabled, OR
    // - In an active agent view, OR
    // - Input is unlocked (autodetected) (implying the input is autodetected as AI in terminal mode)
    let terminal_mode_predicate = base_context.clone()
        & id!(flags::AGENT_MODE_INPUT)
        & (!id!(flags::AGENT_VIEW_ENABLED)
            | id!(flags::ACTIVE_AGENT_VIEW)
            | id!(flags::ACTIVE_INLINE_AGENT_VIEW)
            | !id!(flags::LOCKED_INPUT));

    app.register_fixed_bindings([FixedBinding::new_per_platform(
        PerPlatformKeystroke {
            mac: "cmd-enter",
            linux_and_windows: "ctrl-shift-enter",
        },
        TerminalAction::SetInputModeAgent,
        agent_mode_predicate.clone()
            & !id!("Input")
            & !id!(ROOT_AMBIENT_AGENT_PANE_KEY)
            & !id!(flags::HAS_PENDING_PROMPT_SUGGESTION)
            & !id!(SSH_ERROR_BLOCK_VISIBLE_KEY),
    )
    .with_enabled(|| FeatureFlag::AgentView.is_enabled())]);

    app.register_editable_bindings([
        EditableBinding::new(
            SET_INPUT_MODE_AGENT_ACTION_NAME,
            crate::t!("keybinding-desc-set-input-mode-agent"),
            TerminalAction::SetInputModeAgent,
        )
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(agent_mode_predicate)
        .with_mac_key_binding("cmd-i")
        .with_linux_or_windows_key_binding("ctrl-i"),
        EditableBinding::new(
            SET_INPUT_MODE_TERMINAL_ACTION_NAME,
            crate::t!("keybinding-desc-set-input-mode-terminal"),
            TerminalAction::SetInputModeTerminal,
        )
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(terminal_mode_predicate)
        .with_mac_key_binding("cmd-i")
        .with_linux_or_windows_key_binding("ctrl-i"),
        EditableBinding::new(
            TOGGLE_HIDE_CLI_RESPONSES_KEYBINDING,
            crate::t!("keybinding-desc-toggle-hide-cli-responses"),
            TerminalAction::ToggleHideCliResponses,
        )
        .with_key_binding("cmdorctrl-g")
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(
            id!(flags::IS_ANY_AI_ENABLED) & !id!(LONG_RUNNING_AGENT_REQUESTED_COMMAND_CONTEXT_KEY),
        ),
    ]);
}
