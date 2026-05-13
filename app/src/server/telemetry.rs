// OpenWarp:telemetry 发送层与 context provider 已删除。
// 这里仅保留 `TelemetryEvent` 枚举及其辅助类型,作为大量 UI/模型调用点的类型壳。

use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use warp_completer::completer::MatchType;
use warp_core::command::ExitCode;
use warpui::keymap::Keystroke;
use warpui::notification::{NotificationSendError, RequestPermissionsOutcome};
use warpui::rendering::ThinStrokes;

use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent::AIAgentActionId;
use crate::ai::agent::AIAgentExchangeId;
use crate::ai::agent::AIAgentInput as FullAIAgentInput;
use crate::ai::agent::AIIdentifiers;
use crate::ai::agent::EntrypointType;
use crate::ai::agent::PassiveSuggestionTrigger;
use crate::ai::agent::ServerOutputId;
use crate::ai::agent::SuggestedLoggingId;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::blocklist::agent_view::AgentViewEntryOrigin;
use crate::ai::blocklist::CommandExecutionPermissionAllowedReason;
use crate::ai::blocklist::InputType;
use crate::ai::mcp::TemplateVariable;
use crate::ai::predict::generate_ai_input_suggestions::GenerateAIInputSuggestionsRequest;
use crate::ai::predict::generate_ai_input_suggestions::GenerateAIInputSuggestionsResponseV2;
use crate::ai::predict::next_command_model::HistoryBasedAutosuggestionState;
use crate::auth::LoginGatedFeature;
use crate::cloud_object::{
    model::generic_string_model::GenericStringObjectId, GenericStringObjectFormat, ObjectType,
    Space,
};
#[cfg(feature = "local_fs")]
use crate::code::editor_management::CodeSource;
use crate::drive::DriveSortOrder;
use crate::drive::ObjectTypeAndId;
use crate::launch_configs::save_modal::SaveState;
use crate::notebooks::telemetry::NotebookTelemetryAction;
use crate::notebooks::NotebookId;
use crate::notebooks::NotebookLocation;
use crate::palette::PaletteMode;
use crate::pane_group::PaneDragDropLocation;
use crate::prompt::editor_modal::OpenSource as PromptEditorOpenSource;
use crate::search::command_search::searcher::CommandSearchItemAction;
use crate::search::QueryFilter;
use crate::server::ids::ObjectUid;
use crate::server::ids::ServerId;
use crate::settings::import::config::ParsedTerminalSetting;
use crate::settings::import::config::SettingType;
use crate::settings::import::model::TerminalType;
use crate::settings::AgentModeCodingPermissionsType;
use crate::settings_view::TeamsInviteOption;
use crate::tab::TabTelemetryAction;
use crate::terminal::block_list_viewport::InputMode;
use crate::terminal::cli_agent_sessions::CLIAgentInputEntrypoint;
use crate::terminal::cli_agent_sessions::CLIAgentRichInputCloseReason;
use crate::terminal::input::TelemetryInputSuggestionsMode;
use crate::terminal::model::ansi::WarpificationUnavailableReason;
use crate::terminal::model::block::BlockId;
use crate::terminal::model::session::SessionId;
use crate::terminal::model::terminal_model::BlockSelectionCardinality;
use crate::terminal::model::terminal_model::TmuxInstallationState;
use crate::terminal::settings::AltScreenPaddingMode;
use crate::terminal::shell::ShellType;
use crate::terminal::ssh::ssh_detection::SshInteractiveSessionDetected;
use crate::terminal::view::block_onboarding::onboarding_agentic_suggestions_block::OnboardingChipType;
use crate::terminal::view::inline_banner::ZeroStatePromptSuggestionTriggeredFrom;
use crate::terminal::view::inline_banner::ZeroStatePromptSuggestionType;
use crate::terminal::view::BlockEntity;
use crate::terminal::view::BlockSelectionDetails;
use crate::terminal::view::ContextMenuInfo;
use crate::terminal::view::GridHighlightedLink;
use crate::terminal::view::PromptPart;
use crate::terminal::view::{
    NotificationsDiscoveryBannerAction, NotificationsErrorBannerAction, NotificationsTrigger,
};
use crate::tips::WelcomeTipFeature;
#[cfg(feature = "local_fs")]
use crate::util::file::external_editor::settings::EditorLayout;
#[cfg(feature = "local_fs")]
use crate::util::openable_file_type::FileTarget;
use crate::workflows::WorkflowId;
use crate::workflows::WorkflowSelectionSource;
use crate::workflows::WorkflowSource;
use crate::workspace::tab_settings::TabCloseButtonPosition;
use crate::workspace::tab_settings::WorkspaceDecorationVisibility;
use crate::workspace::TabMovement;
use warp_core::interval_timer::TimingDataPoint;

#[derive(Clone, Serialize, Deserialize)]
pub struct BootstrappingInfo {
    pub shell: &'static str,
    pub is_ssh: bool,
    pub is_subshell: bool,
    pub is_wsl: bool,
    pub is_msys2: bool,
    /// `true` if the bootstrapping process was triggered by an RC file snippet.
    ///
    /// This should only be true if `is_subshell` is true.
    pub was_triggered_by_rc_file: bool,
    /// The total time it took to bootstrap the shell, in seconds.
    pub bootstrap_duration_seconds: Option<f64>,
    /// The time it took to source the user's rcfiles, in seconds.  May be None
    /// if we weren't able to get that information from the shell.
    pub rcfiles_duration_seconds: Option<f64>,
    /// The difference between the total bootstrap time and the rcfile sourcing
    /// time, which roughly equals the time cost of running our bootstrap
    /// script.  Will be None if `bootstrap_duration_seconds` or
    /// `rcfiles_duration_seconds` is None.
    pub warp_attributed_bootstrap_duration_seconds: Option<f64>,
    pub shell_version: Option<String>,
    pub terminal_session_id: Option<SessionId>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SlowBootstrapInfo {
    pub shell: &'static str,
    pub is_ssh: bool,
    pub is_subshell: bool,
    pub is_wsl: bool,
    pub is_msys2: bool,
    /// Contents of the bootstrap block when the slow bootstrap was detected.
    /// This includes both command and output content from the block.
    pub bootstrap_block_contents: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppStartupInfo {
    pub is_session_restoration_on: bool,
    /// Whether or not a screen reader is enabled at the time the app is
    /// launched.  Should be set to None if we do not know for sure.
    pub is_screen_reader_enabled: Option<bool>,
    pub from_relaunch: bool,
    pub is_crash_reporting_enabled: bool,
    pub timing_data: Vec<TimingDataPoint>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlockLatencyInfo {
    pub command: &'static str,
    pub shell: &'static str,
    pub is_ssh: bool,
    pub execution_ms: u64,
}

// Compatibility metadata for local Warp Drive object event shells.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryObjectType {
    Workflow,
    Notebook,
    Folder,
    GenericStringObject(GenericStringObjectFormat),
}

impl From<&ObjectTypeAndId> for TelemetryObjectType {
    fn from(object_type_and_id: &ObjectTypeAndId) -> Self {
        match object_type_and_id {
            ObjectTypeAndId::Notebook(_) => Self::Notebook,
            ObjectTypeAndId::Workflow(_) => Self::Workflow,
            ObjectTypeAndId::Folder(_) => Self::Folder,
            ObjectTypeAndId::GenericStringObject { object_type, .. } => {
                Self::GenericStringObject(*object_type)
            }
        }
    }
}

/// Compatibility metadata for how an object is scoped locally.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TelemetrySpace {
    /// The object is owned by the current user.
    Personal,
    /// The object is owned by a team the user is on.
    Team,
    /// The object was shared with the user.
    Shared,
}

impl From<Space> for TelemetrySpace {
    fn from(space: Space) -> Self {
        match space {
            Space::Personal => Self::Personal,
            Space::Team { .. } => Self::Team,
            Space::Shared => Self::Shared,
        }
    }
}

/// Common metadata retained for local Warp Drive event call sites that act on a specific object.
/// Events that only apply to a single object type may use specific metadata like [`WorkflowTelemetryMetadata`],
/// [`NotebookTelemetryMetadata`], or [`EnvVarTelemetryMetadata`] instead.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectTelemetryMetadata {
    pub object_type: TelemetryObjectType,
    /// Legacy server UID slot. OpenWarp keeps it optional while object-event call sites are being
    /// localized.
    pub object_uid: Option<ServerId>,
    /// The space through which the user has access to the object.
    pub space: Option<TelemetrySpace>,
    /// If the object is owned by a team, this is the owning team's UID. For shared objects, the
    /// user might not be on the team.
    pub team_uid: Option<ServerId>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorkflowTelemetryMetadata {
    pub workflow_categories: Option<Vec<String>>,
    pub workflow_source: WorkflowSource,
    pub workflow_space: Option<TelemetrySpace>,
    pub workflow_selection_source: WorkflowSelectionSource,
    // This field is populated when the workflow has a local object ID.
    pub workflow_id: Option<WorkflowId>,
    // Any referenced local workflow enum IDs.
    pub enum_ids: Vec<GenericStringObjectId>,
}

/// Metadata to include in all notebook telemetry events.
///
/// There are 4 expected configurations:
/// * Legacy personal notebooks: `notebook_id` is `Some`, `team_uid` is `None`, and location is `PersonalCloud`
/// * Legacy team notebooks: `notebook_id` is `Some`, `team_uid` is `Some`, and location is `Team`
/// * Local file-based notebooks: `notebook_id` and `team_uid` are `None`, and location is `LocalFile`
/// * Remote file-based notebooks: `notebook_id` and `team_uid` are `None`, and location is `RemoteFile`
///
/// This representation allows for invalid combinations, but makes querying the data easier (for
/// example, to find all notebook events for a given team).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NotebookTelemetryMetadata {
    /// Legacy notebook ID, only available for migrated/synced notebook records.
    pub notebook_id: Option<NotebookId>,
    /// Legacy team UID, only available for migrated/shared-team records.
    pub team_uid: Option<ServerId>,
    pub space: Option<TelemetrySpace>,
    /// Where the notebook is canonically located.
    pub location: NotebookLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown_table_count: Option<usize>,
}

impl NotebookTelemetryMetadata {
    pub fn new(
        notebook_id: impl Into<Option<NotebookId>>,
        team_uid: impl Into<Option<ServerId>>,
        location: impl Into<NotebookLocation>,
        space: Option<TelemetrySpace>,
    ) -> Self {
        Self {
            notebook_id: notebook_id.into(),
            team_uid: team_uid.into(),
            location: location.into(),
            space,
            markdown_table_count: None,
        }
    }

    pub fn with_markdown_table_count(mut self, markdown_table_count: usize) -> Self {
        self.markdown_table_count = Some(markdown_table_count);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookActionEvent {
    #[serde(flatten)]
    pub action: NotebookTelemetryAction,
    #[serde(flatten)]
    pub metadata: NotebookTelemetryMetadata,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvVarTelemetryMetadata {
    /// Legacy object ID, only available for migrated env-var records.
    pub object_id: Option<GenericStringObjectId>,
    /// Legacy team UID, only available for migrated/shared-team records.
    pub team_uid: Option<ServerId>,
    pub space: TelemetrySpace,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MCPServerTelemetryMetadata {
    pub object_id: GenericStringObjectId,
    pub name: String,
    pub transport_type: MCPServerTelemetryTransportType,
    /// The MCP server string extracted from '@modelcontextprotocol/<...>'.
    pub mcp_server: Option<String>,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum MCPTemplateCreationSource {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "conversion")]
    Conversion,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum MCPTemplateInstallationSource {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "shared")]
    Shared,
    #[serde(rename = "gallery")]
    Gallery,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum MCPServerModel {
    #[serde(rename = "legacy")]
    Legacy,
    #[serde(rename = "templatable")]
    Templatable,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MCPServerTelemetryTransportType {
    CLIServer,
    ServerSentEvents,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum MCPServerTelemetryError {
    Initialization(String),
    RequestCancelled,
    ResponseError(String),
    SerializationError(String),
    CapabilityUnsupported(String),
    InternalError(String),
    TransportError(String),
}

#[cfg(not(target_family = "wasm"))]
impl From<rmcp::RmcpError> for MCPServerTelemetryError {
    fn from(err: rmcp::RmcpError) -> Self {
        match err {
            rmcp::RmcpError::ClientInitialize(err) => Self::Initialization(err.to_string()),
            rmcp::RmcpError::ServerInitialize(err) => Self::Initialization(err.to_string()),
            rmcp::RmcpError::TransportCreation { error, .. } => {
                Self::TransportError(error.to_string())
            }
            rmcp::RmcpError::Runtime(err) => Self::InternalError(err.to_string()),
            rmcp::RmcpError::Service(err) => match err {
                rmcp::ServiceError::McpError(_) => Self::ResponseError(err.to_string()),
                rmcp::ServiceError::TransportSend(_) => Self::TransportError(err.to_string()),
                rmcp::ServiceError::TransportClosed => Self::TransportError(err.to_string()),
                rmcp::ServiceError::UnexpectedResponse => Self::ResponseError(err.to_string()),
                rmcp::ServiceError::Cancelled { .. } => Self::InternalError(err.to_string()),
                rmcp::ServiceError::Timeout { .. } => Self::TransportError(err.to_string()),
                // The enum is marked as non-exhaustive, so we need a catch-all.
                _ => Self::InternalError(err.to_string()),
            },
        }
    }
}

// OpenWarp Phase 2a: `OpenedSharingDialogEvent` + `SharingDialogSource` and
// the corresponding `OpenedSharingDialog` `TelemetryEvent` variant removed
// along with the sharing dialog UI.

#[derive(Clone, Serialize, Deserialize)]
pub enum TabRenameEvent {
    OpenedEditor,
    CustomNameSet,
    CustomNameCleared,
}

/// The possible sources notifications can turned on from.
#[derive(Clone, Serialize, Deserialize)]
pub enum NotificationsTurnedOnSource {
    Settings,
    Banner,
}

/// The possible types of toggles in the find bar
#[derive(Clone, Serialize, Deserialize)]
pub enum FindOption {
    CaseSensitive,
    FindInBlock,
    Regex,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum LinkOpenMethod {
    CmdClick,
    ToolTip,
    MiddleClick,
}

/// The possible ways to trigger command x-ray
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandXRayTrigger {
    Hover,
    Keystroke,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum PaletteSource {
    PrefixChange,
    Keybinding,
    CtrlTab { shift_pressed_initially: bool },
    WarpDrive,
    QuitModal,
    LogOutModal,
    IntegrationTest,
    ConversationManager,
    ContextChip,
    PaneHeader,
    RecentsViewAll,
    AgentTip,
    TitleBarSearchBar,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum FileTreeSource {
    /// Opened from the pane header toolbelt button.
    PaneHeader,
    Keybinding,
    LeftPanelToolbelt,
    ForceOpened,
    /// Opened from the CLI agent view footer (e.g., Claude Code).
    CLIAgentView,
}

#[cfg(feature = "local_fs")]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodePanelsFileOpenEntrypoint {
    CodeReview,
    ProjectExplorer,
    GlobalSearch,
}

/// The CLI agent being used (for telemetry purposes).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CLIAgentType {
    Claude,
    Gemini,
    Codex,
    Amp,
    Droid,
    OpenCode,
    Copilot,
    Pi,
    Auggie,
    Cursor,
    Goose,
    DeepSeek,
    Unknown,
}

/// The kind of plugin chip shown or dismissed (for telemetry purposes).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginChipTelemetryKind {
    Install,
    Update,
}

/// Identifies the agent variant that triggered a notification (for telemetry purposes).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAgentVariant {
    /// Warp's built-in agent (Oz).
    Oz,
    /// A CLI agent (e.g., Claude Code, Gemini CLI, etc.).
    CLIAgent(CLIAgentType),
}

impl From<crate::notifications::NotificationSourceAgent> for NotificationAgentVariant {
    fn from(agent: crate::notifications::NotificationSourceAgent) -> Self {
        match agent {
            crate::notifications::NotificationSourceAgent::Oz => Self::Oz,
            crate::notifications::NotificationSourceAgent::CLI(cli_agent) => {
                Self::CLIAgent(cli_agent.into())
            }
        }
    }
}

/// The action taken on a plugin chip (for telemetry purposes).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginChipTelemetryAction {
    /// User clicked the auto-install button.
    Install,
    /// User clicked the auto-update button.
    Update,
    /// User clicked the manual install instructions button.
    InstallInstructions,
    /// User clicked the manual update instructions button.
    UpdateInstructions,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WarpDriveSource {
    Legacy,
    LeftPanelToolbelt,
    ForceOpened,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CommandCorrectionAcceptedType {
    /// TODO: We don't use the Autosuggestion variant yet. We need to wire through
    /// when an autosuggestion is accepted to be able to check this.
    Autosuggestion,
    Banner,
    Keybinding,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CommandCorrectionEvent {
    Proposed {
        rule: &'static str,
    },
    Accepted {
        via: CommandCorrectionAcceptedType,
        rule: &'static str,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CommandSearchResultType {
    History,
    Workflow,
    OpenWarpAI,
    TranslateUsingWarpAI,
    Notebook,
    EnvVarCollection,
    ViewInWarpDrive,
    AIQuery,
    Project,
}

impl From<&CommandSearchItemAction> for CommandSearchResultType {
    fn from(action: &CommandSearchItemAction) -> Self {
        use crate::search::command_search::searcher::CommandSearchItemAction::*;
        match action {
            AcceptHistory(_) | ExecuteHistory(_) => Self::History,
            AcceptWorkflow(_) => Self::Workflow,
            AcceptNotebook(_) => Self::Notebook,
            AcceptEnvVarCollection(_) => Self::EnvVarCollection,
            OpenWarpAI => Self::OpenWarpAI,
            TranslateUsingWarpAI => Self::TranslateUsingWarpAI,
            AcceptAIQuery(_) | RunAIQuery(_) => Self::AIQuery,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CloseTarget {
    App,
    Window,
    Tab,
    Pane,
    EditorTab,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum PtySpawnMode {
    /// The pty was spawned using the terminal server.
    TerminalServer,
    /// We tried to spawn the pty using the terminal server, but something went
    /// wrong so we fell back to spawning it directly.
    FallbackToDirect,
    /// The terminal server is not in use, and we spawned the pty directly
    /// (in tests, for example).
    Direct,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum OpenedWarpAISource {
    GlobalEntryButton,
    HelpWithBlock,
    HelpWithTextSelection,
    FromAICommandSearch,
    WarmWelcome,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WarpAIRequestResult {
    Succeeded { latency_ms: i64, truncated: bool },
    OutOfRequests,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WarpAIActionType {
    CopyTranscript,
    Restart,
    CopyAnswer,
    CopyCode,
    InsertIntoInput,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SaveAsWorkflowModalSource {
    Block,
    Input,
    WarpAIWorkflowCard,
    WarpAIPanel,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum LaunchConfigUiLocation {
    CommandPalette,
    AppMenu,
    TabMenu,
    Uri,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AICommandSearchEntrypoint {
    ShortHandTrigger,
    Keybinding,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SecretInteraction {
    RevealSecret,
    HideSecret,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AnonymousUserSignupEntrypoint {
    HitDriveObjectLimit,
    LoginGatedFeature,
    SignUpButton,
    RenotificationBlock,
    SignUpAIPrompt,
    NextCommandSuggestionsUpgradeBanner,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum UndoCloseItemType {
    Window,
    Tab,
    Pane,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromptChoice {
    PS1,
    Default,
    Custom { builtin_chips: Vec<String> },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ToggleBlockFilterSource {
    /// This includes the keybinding and the command palette items.
    Binding,
    ContextMenu,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TierLimitHitEvent {
    pub team_uid: ServerId,
    pub feature: String,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum KnowledgePaneEntrypoint {
    /// Triggered by either the command palette or the mac menus
    #[serde(rename = "global")]
    Global,

    #[serde(rename = "settings")]
    Settings,

    #[serde(rename = "warp_drive")]
    WarpDrive,

    #[serde(rename = "ai_blocklist")]
    AIBlocklist,

    #[serde(rename = "slash_command")]
    SlashCommand,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum MCPServerCollectionPaneEntrypoint {
    /// Triggered by either the command palette or the mac menus
    #[serde(rename = "global")]
    Global,

    #[serde(rename = "settings")]
    Settings,

    #[serde(rename = "warp_drive")]
    WarpDrive,

    #[serde(rename = "slash_command")]
    SlashCommand,

    #[serde(rename = "mcp_settings_tab")]
    MCPSettingsTab,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeEntrypointSelectionType {
    /// User entered Agent Mode by taking action on a blocklist text selection.
    Text,

    /// User entered Agent Mode by taking action on a block selection.
    Block,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeEntrypoint {
    /// The stars icon button in the tab bar.
    #[serde(rename = "tab_bar")]
    TabBar,

    /// This corresponds to _both_ triggering from the command palette and via keybinding.
    ///
    /// Unfortunately due to the way the command palette automatically surfaces any editable
    /// keybinding as an action, we don't have enough information to discern if the binding was
    /// triggered by the palette or keyboard.
    #[serde(rename = "new_pane_binding")]
    NewPaneBinding,

    /// The stars button in the hoverable block "toolbelt".
    #[serde(rename = "block_toolbelt")]
    BlockToolbelt,

    /// The "Ask Agent Mode" option from AI command search.
    #[serde(rename = "ai_command_search")]
    AICommandSearch,

    /// Context menu item(s) that attach a blocklist selection as context to an Agent Mode query.
    #[serde(rename = "context_menu")]
    ContextMenu {
        selection_type: AgentModeEntrypointSelectionType,
    },

    /// The Agent Mode chip in the prompt.
    #[serde(rename = "prompt_chip")]
    PromptChip,

    /// The Agent Management popup, where you can see all the most recent tasks for each terminal
    /// pane across all windows/tabs/panes.
    #[serde(rename = "agent_management_popup")]
    AgentManagementPopup,

    /// User manually switched between terminal and AI input modes in UDI interface
    #[serde(rename = "udi_terminal_input_switcher")]
    UDITerminalInputSwitcher,

    /// The agent management view, where you can see both local interactive and ambient agent tasks
    #[serde(rename = "agent_management_view")]
    AgentManagementView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AutonomySettingToggleSource {
    Speedbump,
    SettingsPage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToggleCodeSuggestionsSettingSource {
    Speedbump,
    Settings,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InteractionSource {
    Button,
    Keybinding,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PromptSuggestionViewType {
    TerminalView,
    AgentView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeAttachContextMethod {
    #[serde(rename = "keyboard")]
    Keyboard,

    #[serde(rename = "mouse")]
    Mouse,
}

/// The entrypoint from which the rewind dialog was opened.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AgentModeRewindEntrypoint {
    /// The rewind button in the AI block header.
    Button,
    /// The context menu item "Rewind to before here".
    ContextMenu,
    /// The /rewind slash command.
    SlashCommand,
}

/// Reasons why we fell back to a prompt suggestion from a suggested code diff.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum PromptSuggestionFallbackReason {
    /// Code file had too many lines, hence we stopped triggering the suggested code diff.
    #[serde(rename = "file_too_many_lines")]
    FileTooManyLines,
    /// Code file had too many bytes, hence we stopped triggering the suggested code diff.
    #[serde(rename = "file_too_many_bytes")]
    FileTooManyBytes,
    /// Missing file, when looking up filepaths in local file system.
    #[serde(rename = "missing_file")]
    MissingFile,
    /// Failed to retrieve file from local file system.
    #[serde(rename = "failed_to_retrieve_file")]
    FailedToRetrieveFile,
    /// In an SSH/remote session.
    #[serde(rename = "ssh_remote_session")]
    SSHRemoteSession,
    /// No read files permission.
    #[serde(rename = "no_read_files_permission")]
    NoReadFilesPermission,
    /// AI query timeout.
    #[serde(rename = "ai_query_timeout")]
    AIQueryTimeout,
    /// Failed to send AI request.
    #[serde(rename = "failed_to_send_ai_request")]
    FailedToSendAIRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeSetupProjectScopedRulesActionType {
    #[serde(rename = "link_from_existing")]
    LinkFromExisting(String),
    #[serde(rename = "generate_warp_md")]
    GenerateWarpMd,
    #[serde(rename = "skip_rules")]
    SkipRules,
    #[serde(rename = "regenerate_warp_md")]
    RegenerateWarpMd,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CpuUsageStats {
    /// The number of logical CPUs on the system.
    pub num_cpus: usize,

    /// The maximum CPU usage over the measurement interval.
    ///
    /// This number is in the range [0, num_cpus].  The CPU utilization, as a
    /// percentage, can be determined via `max_usage / num_cpus * 100`.
    pub max_usage: f32,

    /// The average CPU usage over the measurement interval.
    ///
    /// This number is in the range [0, num_cpus].  The CPU utilization, as a
    /// percentage, can be determined via `avg_usage / num_cpus * 100`.
    pub avg_usage: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryUsageStats {
    pub total_application_usage_bytes: usize,
    pub total_blocks: usize,
    pub total_lines: usize,

    /// Statistics about blocks that have been seen in the past 5 minutes.
    pub active_block_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [5m, 1h).
    pub inactive_5m_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [1h, 24h).
    pub inactive_1h_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [24h, ..).
    pub inactive_24h_stats: BlockMemoryUsageStats,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockMemoryUsageStats {
    pub num_blocks: usize,
    pub num_lines: usize,
    pub estimated_memory_usage_bytes: usize,
}

/// Entrypoints to toggle the input auto-detection setting for Agent Mode.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AgentModeAutoDetectionSettingOrigin {
    /// The "speed bump" banner shown that's shown to the user when input is autodetected.
    #[serde(rename = "banner")]
    Banner,

    /// The AI settings page.
    #[serde(rename = "settings_page")]
    SettingsPage,
}

/// Payload for the [`AgentModePotentialAutodetectionFalsePositive`] event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentModeAutoDetectionFalsePositivePayload {
    /// Payload includes input text for dogfood channels.
    InternalDogfoodUsers { input_text: String },

    /// Do not include the misclassified input text in stable channels due to privacy concerns.
    ExternalUsers,
}

/// How the user triggered the [`AgentModeCodeFilesNavigated`] event.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum AgentModeCodeFileNavigationSource {
    /// User used the next/previous actions.
    NavigationCommand,
    /// User directly selected the file's tab.
    SelectedFileTab,
}

/// How the user triggered the [`AddTabWithShell`] event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum AddTabWithShellSource {
    CommandPalette,
    ShellSelectorMenu,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeContextDestination {
    Pty,
    AgentInput,
    RichInput,
}

#[derive(Clone, Debug, Serialize)]
pub enum AgentModeCitation {
    WarpDriveObject {
        object_type: ObjectType,
        uid: ObjectUid,
    },
    WarpDocs {
        page: String,
    },
    WebPage {
        // Don't serialize the URL to avoid leaking sensitive information.
        #[serde(skip_serializing)]
        url: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum ImageProtocol {
    Kitty,
    ITerm,
}

#[derive(Clone, Copy, Debug, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputUXChangeOrigin {
    #[default]
    Settings,
    ADELaunchModal,
}

#[derive(Clone, Debug, Serialize)]
pub enum AIAgentInput {
    UserQuery { query: String },
    AutoCodeDiffQuery { query: String },
    ResumeConversation,
    InitProjectRules { display_query: Option<String> },
    TriggerSuggestPrompt { trigger: PassiveSuggestionTrigger },
    ActionResult { action_id: AIAgentActionId },
    CreateNewProject { query: String },
    CloneRepository { url: String },
    CodeReview,
    FetchReviewComments,
    SummarizeConversation,
    InvokeSkill { skill_name: String },
    StartFromAmbientRunPrompt,
    MessagesReceivedFromAgents { message_count: usize },
    EventsFromAgents { event_count: usize },
    PassiveSuggestionResult,
}

impl From<FullAIAgentInput> for AIAgentInput {
    fn from(input: FullAIAgentInput) -> Self {
        match input {
            FullAIAgentInput::UserQuery { query, .. } => Self::UserQuery { query },
            FullAIAgentInput::AutoCodeDiffQuery { query, .. } => Self::AutoCodeDiffQuery { query },
            FullAIAgentInput::ResumeConversation { .. } => Self::ResumeConversation,
            FullAIAgentInput::InitProjectRules { display_query, .. } => {
                Self::InitProjectRules { display_query }
            }
            FullAIAgentInput::TriggerPassiveSuggestion { trigger, .. } => {
                Self::TriggerSuggestPrompt { trigger }
            }
            FullAIAgentInput::ActionResult { result, .. } => Self::ActionResult {
                action_id: result.id,
            },
            FullAIAgentInput::CreateNewProject { query, .. } => Self::CreateNewProject { query },
            FullAIAgentInput::CloneRepository { clone_repo_url, .. } => Self::CloneRepository {
                url: clone_repo_url.into_url(),
            },
            FullAIAgentInput::CodeReview { .. } => Self::CodeReview,
            FullAIAgentInput::FetchReviewComments { .. } => Self::FetchReviewComments,
            FullAIAgentInput::SummarizeConversation { .. } => Self::SummarizeConversation,
            FullAIAgentInput::InvokeSkill { skill, .. } => Self::InvokeSkill {
                skill_name: skill.name.clone(),
            },
            FullAIAgentInput::StartFromAmbientRunPrompt { .. } => Self::StartFromAmbientRunPrompt,
            FullAIAgentInput::MessagesReceivedFromAgents { messages } => {
                Self::MessagesReceivedFromAgents {
                    message_count: messages.len(),
                }
            }
            FullAIAgentInput::EventsFromAgents { events } => Self::EventsFromAgents {
                event_count: events.len(),
            },
            FullAIAgentInput::PassiveSuggestionResult { .. } => Self::PassiveSuggestionResult,
        }
    }
}

/// The origin of an agent view entry, for telemetry purposes.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryAgentViewEntryOrigin {
    Input { was_prompt_autodetected: bool },
    ConversationSelector,
    AgentModeHomepage,
    AgentViewBlock,
    AIDocument,
    AutoFollowUp,
    RestoreExistingConversation,
    SharedSessionSelection,
    AgentRequestedNewConversation,
    AcceptedPromptSuggestion,
    AcceptedUnitTestSuggestion,
    AcceptedPassiveCodeDiff,
    InlineCodeReview,
    AmbientAgent,
    Cli,
    ImageAdded,
    SlashCommand,
    CodeReviewContext,
    ContinueConversationButton,
    ViewPassiveCodeDiffDetails,
    ResumeConversationButton,
    CodexModal,
    LongRunningCommand,
    HistoryMenu,
    InlineConversationMenu,
    PromptChip,
    OnboardingCallout,
    ConversationListView,
    Onboarding,
    Keybinding,
    SlashInit,
    ProjectEntry,
    ClearBuffer,
    DefaultSessionMode,
    ChildAgent,
    LinearDeepLink,
    ThirdPartyAmbientAgent,
}

impl From<AgentViewEntryOrigin> for TelemetryAgentViewEntryOrigin {
    fn from(origin: AgentViewEntryOrigin) -> Self {
        match origin {
            AgentViewEntryOrigin::Input {
                was_prompt_autodetected,
            } => Self::Input {
                was_prompt_autodetected,
            },
            AgentViewEntryOrigin::ConversationSelector => Self::ConversationSelector,
            AgentViewEntryOrigin::AgentModeHomepage => Self::AgentModeHomepage,
            AgentViewEntryOrigin::AgentViewBlock => Self::AgentViewBlock,
            AgentViewEntryOrigin::AIDocument => Self::AIDocument,
            AgentViewEntryOrigin::AutoFollowUp => Self::AutoFollowUp,
            AgentViewEntryOrigin::RestoreExistingConversation => Self::RestoreExistingConversation,
            AgentViewEntryOrigin::SharedSessionSelection => Self::SharedSessionSelection,
            AgentViewEntryOrigin::AgentRequestedNewConversation => {
                Self::AgentRequestedNewConversation
            }
            AgentViewEntryOrigin::AcceptedPromptSuggestion => Self::AcceptedPromptSuggestion,
            AgentViewEntryOrigin::AcceptedUnitTestSuggestion => Self::AcceptedUnitTestSuggestion,
            AgentViewEntryOrigin::AcceptedPassiveCodeDiff => Self::AcceptedPassiveCodeDiff,
            AgentViewEntryOrigin::InlineCodeReview => Self::InlineCodeReview,
            AgentViewEntryOrigin::CloudAgent => Self::AmbientAgent,
            AgentViewEntryOrigin::ThirdPartyCloudAgent => Self::ThirdPartyAmbientAgent,
            AgentViewEntryOrigin::Cli => Self::Cli,
            AgentViewEntryOrigin::ImageAdded => Self::ImageAdded,
            AgentViewEntryOrigin::SlashCommand { .. } => Self::SlashCommand,
            AgentViewEntryOrigin::CodeReviewContext => Self::CodeReviewContext,
            AgentViewEntryOrigin::LongRunningCommand => Self::LongRunningCommand,
            AgentViewEntryOrigin::ContinueConversationButton => Self::ContinueConversationButton,
            AgentViewEntryOrigin::ViewPassiveCodeDiffDetails => Self::ViewPassiveCodeDiffDetails,
            AgentViewEntryOrigin::ResumeConversationButton => Self::ResumeConversationButton,
            AgentViewEntryOrigin::CodexModal => Self::CodexModal,
            AgentViewEntryOrigin::InlineHistoryMenu => Self::HistoryMenu,
            AgentViewEntryOrigin::InlineConversationMenu => Self::InlineConversationMenu,
            AgentViewEntryOrigin::PromptChip => Self::PromptChip,
            AgentViewEntryOrigin::OnboardingCallout => Self::OnboardingCallout,
            AgentViewEntryOrigin::ConversationListView => Self::ConversationListView,
            AgentViewEntryOrigin::Onboarding => Self::Onboarding,
            AgentViewEntryOrigin::Keybinding => Self::Keybinding,
            AgentViewEntryOrigin::SlashInit => Self::SlashInit,
            AgentViewEntryOrigin::ProjectEntry => Self::ProjectEntry,
            AgentViewEntryOrigin::ClearBuffer => Self::ClearBuffer,
            AgentViewEntryOrigin::DefaultSessionMode => Self::DefaultSessionMode,
            AgentViewEntryOrigin::ChildAgent => Self::ChildAgent,
            AgentViewEntryOrigin::LinearDeepLink => Self::LinearDeepLink,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum SlashMenuSource {
    SlashButton,
    UserTyped,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginEventSource {
    OnboardingSlide,
    AuthModal,
}

/// Details about which type of slash command was accepted
#[derive(Clone, Debug, Serialize)]
pub enum SlashCommandAcceptedDetails {
    /// A built-in static command with its specific name (e.g., "/init", "/diff-review")
    StaticCommand { command_name: String },
    /// A user-created saved prompt/workflow
    SavedPrompt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AutoReloadModalAction {
    #[serde(rename = "dismissed")]
    Dismissed,
    #[serde(rename = "enabled_auto_reload")]
    EnabledAutoReload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OutOfCreditsBannerAction {
    #[serde(rename = "dismissed")]
    Dismissed,
    #[serde(rename = "credits_purchased")]
    CreditsPurchased,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CLISubagentControlState {
    AgentInControl,
    UserInControl,
    AgentTaggedIn,
    AgentTaggedOut,
}

#[derive(Clone)]
pub enum TelemetryEvent {
    AutosuggestionInserted {
        insertion_length: usize,
        buffer_length: usize,
    },
    BlockCompleted {
        block_finished_to_precmd_delay_ms: u64,
        honor_ps1_enabled: bool,
        num_secrets_redacted: usize,
        /// The number of lines in the block's output grid when it was
        /// finished.
        num_output_lines: u64,
        /// The number of lines of output that were truncated while the block
        /// was active and receiving output.
        num_output_lines_truncated: u64,
        terminal_session_id: Option<SessionId>,
        is_udi_enabled: bool,
        /// Whether the command was executed while in an active agent view.
        is_in_agent_view: bool,
    },
    /// This is identical to the `BlockCompleted` event, but includes extra fields for
    /// the command run / time it took the block to complete / exit code.
    /// That sort of telemetry should *NEVER* be sent in production, so
    /// DO NOT SEND THIS IN NON-DOGFOOD ENVIRONMENTS!
    BlockCompletedOnDogfoodOnly {
        block_finished_to_precmd_delay_ms: u64,
        honor_ps1_enabled: bool,
        num_secrets_redacted: usize,
        /// The number of lines in the block's output grid when it was
        /// finished.
        num_output_lines: u64,
        /// The number of lines of output that were truncated while the block
        /// was active and receiving output.
        num_output_lines_truncated: u64,
        command: String,
        duration: Duration,
        exit_code: ExitCode,
        terminal_session_id: Option<SessionId>,
    },
    /// A new block of background output was started and added to the block list.
    BackgroundBlockStarted,
    /// User-perceptible latency (i.e. from hitting enter to first frame after command finishes) for
    /// a number of commands that perform minimal work we use as a baseline.
    BaselineCommandLatency(BlockLatencyInfo),
    SessionCreation,
    Login,
    OpenSuggestionsMenu(TelemetryInputSuggestionsMode),
    ConfirmSuggestion {
        mode: TelemetryInputSuggestionsMode,
        match_type: MatchType,
    },
    OpenContextMenu {
        context_menu_info: ContextMenuInfo,
    },
    /// Copy command, output or both for some number of blocks.
    ContextMenuCopy(BlockEntity, BlockSelectionCardinality),
    ContextMenuFindWithinBlocks(BlockSelectionCardinality),
    ContextMenuCopyPrompt {
        part: PromptPart,
    },
    ContextMenuToggleGitPromptDirtyIndicator {
        enabled: bool,
    },
    ContextMenuInsertSelectedText,
    ContextMenuCopySelectedText,
    /// The user opened the prompt editor modal.
    OpenPromptEditor {
        entrypoint: PromptEditorOpenSource,
    },
    /// The user's prompt was edited via the prompt editor modal.
    PromptEdited {
        prompt: PromptChoice,
        entrypoint: String,
    },
    ReinputCommands(BlockSelectionCardinality),
    JumpToPreviousCommand,
    BlockSelection(BlockSelectionDetails),
    BootstrappingSlow(BootstrappingInfo),
    BootstrappingSlowContents(SlowBootstrapInfo),
    /// Logged when a pending session is abandoned before it hits Bootstrapped.
    SessionAbandonedBeforeBootstrap {
        pending_shell: Option<ShellType>,
        has_pending_ssh_session: bool,
        was_ever_visible: bool,
        duration_since_start: Duration,
    },
    BootstrappingSucceeded(BootstrappingInfo),
    /// The user accepted a completion suggestion when it was the only one in the suggestions menu.
    /// This event is named with 'Tab' to maintain backwards compatibility; the completion
    /// suggestions menu may be triggered with a keybinding other than tab.
    TabSingleResultAutocompletion,
    EditorUnhandledModifierKey(String),
    CopyInviteLink,
    OpenThemeChooser,
    ThemeSelection {
        theme: String,
        entrypoint: String,
    },
    AppIconSelection {
        icon: String,
    },
    CursorDisplayType {
        cursor: String,
    },
    OpenThemeCreatorModal,
    CreateCustomTheme,
    DeleteCustomTheme,
    SplitPane,
    UnableToAutoUpdateToNewVersion,
    /// An update was successfully installed, and we're attempting to relaunch the app.
    AutoupdateRelaunchAttempt {
        new_version: String,
    },
    SkipOnboardingSurvey,
    ToggleRestoreSession(bool),
    DatabaseStartUpError(String),
    DatabaseReadError(String),
    DatabaseWriteError(String),
    AppStartup(AppStartupInfo),
    /// The native app was opened while logged out. Since Warp requires login,
    /// this usually means a new user.
    LoggedOutStartup,
    /// We attempted to bootstrap an SSH session via the SSH wrapper.  The
    /// argument is the name of the remote shell.
    SSHBootstrapAttempt(String),
    SSHControlMasterError,
    KeybindingChanged {
        action: String,
        keystroke: Keystroke,
    },
    KeybindingResetToDefault {
        action: String,
    },
    KeybindingRemoved {
        action: String,
    },
    FeaturesPageAction {
        action: String,
        value: String,
    },
    WorkflowExecuted(WorkflowTelemetryMetadata),
    WorkflowSelected(WorkflowTelemetryMetadata),
    OpenWorkflowSearch,
    OpenQuakeModeWindow,
    OpenWelcomeTips,
    CompleteWelcomeTipFeature {
        total_completed_count: usize,
        tip_name: WelcomeTipFeature,
    },
    DismissWelcomeTips,
    ShowNotificationsDiscoveryBanner,
    NotificationsDiscoveryBannerAction(NotificationsDiscoveryBannerAction),
    ShowNotificationsErrorBanner,
    NotificationsErrorBannerAction(NotificationsErrorBannerAction),
    NotificationPermissionsRequested {
        source: NotificationsTurnedOnSource,
        trigger: Option<NotificationsTrigger>,
    },
    NotificationsRequestPermissionsOutcome {
        outcome: RequestPermissionsOutcome,
    },
    // NotificationSent events are emitted at the app level. Thus, they encompass
    // notifications that are successfully sent _and_ those that fail at the platform level.
    NotificationSent {
        trigger: NotificationsTrigger,
        /// Identifies which agent variant produced the desktop notification, if any.
        agent_variant: Option<NotificationAgentVariant>,
    },
    NotificationFailedToSend {
        error: NotificationSendError,
    },
    NotificationClicked,
    ToggleFindOption {
        option: FindOption,
        enabled: bool,
    },
    SignUpButtonClicked,
    LoginButtonClicked {
        source: LoginEventSource,
    },
    LoginLaterButtonClicked {
        source: LoginEventSource,
    },
    LoginLaterConfirmationButtonClicked {
        source: LoginEventSource,
    },
    OpenNewSessionFromFilePath,
    OpenTeamFromURI,
    ShowedSuggestedAgentModeWorkflowChip {
        logging_id: SuggestedLoggingId,
    },
    ShowedSuggestedAgentModeWorkflowModal {
        logging_id: SuggestedLoggingId,
    },
    SelectNavigationPaletteItem,
    SelectCommandPaletteOption(String),
    PaletteSearchOpened {
        mode: PaletteMode,
        source: PaletteSource,
    },
    PaletteSearchResultAccepted {
        result_type: &'static str,
        filter: Option<QueryFilter>,
        buffer_length: usize,
    },
    PaletteSearchExited {
        filter: Option<QueryFilter>,
        buffer_length: usize,
    },
    AuthCommonQuestionClicked {
        question: &'static str,
    },
    AuthToggleFAQ {
        open: bool,
    },
    OpenAuthPrivacySettings {
        source: LoginEventSource,
    },
    TabRenamed(TabRenameEvent),
    MoveActiveTab {
        direction: TabMovement,
    },
    MoveTab {
        direction: TabMovement,
    },
    DragAndDropTab,
    TabOperations {
        action: TabTelemetryAction,
    },
    EditedInputBeforePrecmd,
    TriedToExecuteBeforePrecmd,
    ThinStrokesSettingChanged {
        new_value: ThinStrokes,
    },
    BookmarkBlockToggled {
        enable_bookmark: bool,
    },
    JumpToBookmark,
    JumpToBottomofBlockButtonClicked,
    ToggleJumpToBottomofBlockButton {
        enabled: bool,
    },
    ToggleShowBlockDividers {
        enabled: bool,
    },
    OpenLink {
        link: GridHighlightedLink,
        open_with: LinkOpenMethod,
    },
    OpenChangelogLink {
        url: String,
    },
    ShowInFileExplorer,
    CommandXRayTriggered {
        trigger: CommandXRayTrigger,
    },
    OpenLaunchConfigSaveModal,
    SaveLaunchConfig {
        state: SaveState,
    },
    OpenLaunchConfigFile,
    OpenLaunchConfig {
        ui_location: LaunchConfigUiLocation,
        open_in_active_window: bool,
    },
    TeamCreated,
    TeamJoined,
    TeamLeft,
    TeamLinkCopied,
    RemovedUserFromTeam,
    DeletedWorkflow,
    DeletedNotebook,
    ToggleApprovalsModal,
    ChangedInviteViewOption(TeamsInviteOption),
    SendEmailInvites,
    CommandCorrection {
        event: CommandCorrectionEvent,
    },
    SetLineHeight {
        new_value: f32,
    },
    ResourceCenterOpened,
    ResourceCenterTipsCompleted,
    ResourceCenterTipsSkipped,
    KeybindingsPageOpened,
    CommandSearchOpened {
        has_initial_query: bool,
    },
    CommandSearchExited {
        query_filter: Option<QueryFilter>,
        buffer_length: usize,
    },
    CommandSearchResultAccepted {
        result_index: usize,
        result_type: CommandSearchResultType,
        query_filter: Option<QueryFilter>,
        buffer_length: usize,
        was_immediately_executed: bool,
    },
    CommandSearchFilterChanged {
        new_filter: Option<QueryFilter>,
    },
    CommandSearchAsyncQueryCompleted {
        filters: HashSet<QueryFilter>,
        error_payload: Option<Value>,
    },
    GlobalSearchOpened,
    GlobalSearchQueryStarted,
    AICommandSearchOpened {
        entrypoint: AICommandSearchEntrypoint,
    },
    OpenNotebook(NotebookTelemetryMetadata),
    EditNotebook {
        metadata: NotebookTelemetryMetadata,
        meaningful_change: bool,
    },
    NotebookAction(NotebookActionEvent),
    OpenedAltScreenFind,
    UserInitiatedClose {
        initiated_on: CloseTarget,
    },
    QuitModalShown {
        running_processes: u32,
        shared_sessions: u32,
        modal_for: CloseTarget,
    },
    QuitModalCancel {
        nav_palette: bool,
        modal_for: CloseTarget,
    },
    QuitModalDisabled,
    UserInitiatedLogOut,
    LogOutModalShown,
    LogOutModalCancel {
        nav_palette: bool,
    },
    SetOpacity {
        // Represented in percentages from 1-100.
        opacity: u8,
    },
    SetBlurRadius {
        // The radius value from 1-18.
        blur_radius: u8,
    },
    ToggleDimInactivePanes {
        enabled: bool,
    },
    InputModeChanged {
        old_mode: InputMode,
        new_mode: InputMode,
    },
    PtySpawned {
        mode: PtySpawnMode,
    },
    InitialWorkingDirectoryConfigurationChanged {
        advanced_mode_enabled: bool,
    },
    /// Opened legacy Warp AI.
    OpenedWarpAI {
        source: OpenedWarpAISource,
    },
    /// Issued legacy Warp AI request.
    WarpAIRequestIssued {
        result: WarpAIRequestResult,
    },
    WarpAIAction {
        action_type: WarpAIActionType,
    },
    /// This is purely for static prompts! Do not send user-written prompts with this event.
    UsedWarpAIPreparedPrompt {
        prompt: &'static str,
    },
    ToggleFocusPaneOnHover {
        enabled: bool,
    },
    WarpAICharacterLimitExceeded,
    OpenInputContextMenu,
    InputCutSelectedText,
    InputCopySelectedText,
    InputSelectAll,
    InputPaste,
    InputCommandSearch,
    InputAICommandSearch,
    InputAskWarpAI,
    SaveAsWorkflowModal {
        source: SaveAsWorkflowModalSource,
    },
    ExperimentTriggered {
        experiment: &'static str,
        layer: &'static str,
        group_assignment: &'static str,
    },
    ToggleSyncAllPanesInAllTabs {
        enabled: bool,
    },
    ToggleSyncAllPanesInTab {
        enabled: bool,
    },
    ToggleSameLinePrompt {
        enabled: bool,
    },
    ToggleNewWindowsAtCustomSize {
        enabled: bool,
    },
    SetNewWindowsAtCustomSize,
    DisableInputSync,
    ToggleTabIndicators {
        enabled: bool,
    },
    TogglePreserveActiveTabColor {
        enabled: bool,
    },
    ShowSubshellBanner,
    DeclineSubshellBootstrap {
        remember: bool,
    },
    TriggerSubshellBootstrap {
        triggered_by_rc_file_snippet: bool,
    },
    AddDenylistedSubshellCommand,
    RemoveDenylistedSubshellCommand,
    AddAddedSubshellCommand,
    RemoveAddedSubshellCommand,
    ReceivedSubshellRcFileDcs,
    AddDenylistedSshTmuxWrapperHost,
    RemoveDenylistedSshTmuxWrapperHost,
    /// User Setting for enabling SSH Tmux Wrapper changed.
    ToggleSshTmuxWrapper {
        enabled: bool,
    },
    ToggleSshWarpification {
        enabled: bool,
    },
    /// User changed the SSH extension install mode.
    SetSshExtensionInstallMode {
        mode: &'static str,
    },
    /// User toggled the "Don't ask me this again" checkbox on the SSH
    /// remote-server choice block.
    SshRemoteServerChoiceDoNotAskAgainToggled {
        checked: bool,
    },
    /// An ssh interactive session was detected.
    SshInteractiveSessionDetected(SshInteractiveSessionDetected),
    SshTmuxWarpifyBannerDisplayed,
    /// A SSH Warpify Block was accepted
    SshTmuxWarpifyBlockAccepted,
    /// A SSH Warpify Block was dismissed
    SshTmuxWarpifyBlockDismissed,
    WarpifyFooterShown {
        is_ssh: bool,
    },
    AgentToolbarDismissed,
    WarpifyFooterAcceptedWarpify {
        is_ssh: bool,
    },
    /// How long until the warpify process succeeded
    SshTmuxWarpificationSuccess {
        tmux_installation: Option<TmuxInstallationState>,
        duration_ms: u64,
    },
    /// An SSH Error block was displayed to the user.
    SshTmuxWarpificationErrorBlock {
        error: WarpificationUnavailableReason,
        tmux_installation: Option<TmuxInstallationState>,
    },
    /// A SSH Install Tmux Block was displayed.
    SshInstallTmuxBlockDisplayed,
    /// A SSH Install Tmux Block was accepted.
    SshInstallTmuxBlockAccepted,
    /// A SSH Install Tmux Block was dismissed.
    SshInstallTmuxBlockDismissed,
    ShowAliasExpansionBanner,
    EnableAliasExpansionFromBanner,
    DismissAliasExpansionBanner,
    ShowVimKeybindingsBanner,
    EnableVimKeybindingsFromBanner,
    DismissVimKeybindingsBanner,
    InitiateReauth,
    InitiateAnonymousUserSignup {
        entrypoint: AnonymousUserSignupEntrypoint,
    },
    AnonymousUserExpirationLockout,
    AnonymousUserLinkedFromBrowser,
    AnonymousUserAttemptLoginGatedFeature {
        feature: LoginGatedFeature,
    },
    AnonymousUserHitObjectLimit,
    NeedsReauth,
    WarpDriveOpened {
        source: WarpDriveSource,
        is_code_mode_v2: bool,
    },
    // Toggled the legacy Warp AI side panel.
    ToggleWarpAI {
        opened: bool,
    },
    ToggleSecretRedaction {
        enabled: bool,
    },
    CustomSecretRegexAdded,
    ToggleObfuscateSecret {
        interaction: SecretInteraction,
    },
    CopySecret,
    AutoGenerateMetadataSuccess,
    AutoGenerateMetadataError {
        error_payload: Value,
    },
    UpdateSortingChoice {
        sorting_choice: DriveSortOrder,
    },
    UndoClose {
        item_type: UndoCloseItemType,
    },
    /// This event is used to measure PTY throughput.
    /// NOTE: this event is only meant to be used for WarpDev.
    PtyThroughput {
        /// The maximum PTY throughput in bytes/sec, aggregated over a 10 minute period.
        max_bytes_per_second: usize,
    },
    DuplicateObject(TelemetryObjectType),
    ExportObject(TelemetryObjectType),
    DriveSharingOnboardingBlockShown,
    CommandFileRun,
    PageUpDownInEditorPressed {
        // Key pressed when nothing is in the editor (no-op)
        is_empty_editor: bool,
        // Is PageDown. Otherwise is PageUp
        is_down: bool,
    },
    WebObjectOpenedOnDesktop {
        object_metadata: ObjectTelemetryMetadata,
    },
    UnsupportedShell {
        shell: String,
    },
    LogOut,
    SettingsImportInitiated,
    InviteTeammates {
        num_teammates: usize,
        team_uid: ServerId,
    },
    CopyObjectToClipboard(TelemetryObjectType),
    OpenAndWarpifyDockerSubshell {
        /// Some variant if we support this shell type, and None otherwise.
        shell_type: Option<ShellType>,
    },
    /// Represents an update to a block filter query that goes from empty to non-empty.
    UpdateBlockFilterQuery,
    UpdateBlockFilterQueryContextLines {
        num_context_lines: u16,
    },
    ToggleBlockFilterQuery {
        enabled: bool,
        source: ToggleBlockFilterSource,
    },
    ToggleBlockFilterCaseSensitivity {
        enabled: bool,
    },
    ToggleBlockFilterRegex {
        enabled: bool,
    },
    ToggleBlockFilterInvert {
        enabled: bool,
    },
    BlockFilterToolbeltButtonClicked,
    ToggleSnackbarInActivePane {
        show_snackbar: bool,
    },
    PaneDragInitiated,
    PaneDropped {
        drop_location: PaneDragDropLocation,
    },
    ObjectLinkCopied {
        link: String,
    },
    FileTreeToggled {
        source: FileTreeSource,
        is_code_mode_v2: bool,
        /// The CLI agent type if opened from a CLI agent footer (e.g., Claude Code).
        cli_agent: Option<CLIAgentType>,
    },
    /// User attached a file or directory as context from the file tree
    FileTreeItemAttachedAsContext {
        is_directory: bool,
    },
    /// User added selected code as context from the code editor.
    CodeSelectionAddedAsContext {
        destination: CodeContextDestination,
    },
    /// User created a new file from the file tree
    FileTreeItemCreated,
    /// Conversation list view was opened
    ConversationListViewOpened,
    /// User opened a conversation from the conversation list
    ConversationListItemOpened {
        /// Whether the conversation is an ambient agent task (vs a local conversation)
        is_ambient_agent: bool,
    },
    /// User deleted a conversation from the conversation list
    ConversationListItemDeleted,
    /// User copied a conversation link from the conversation list
    ConversationListLinkCopied {
        /// Whether the conversation is an ambient agent task (vs a local conversation)
        is_ambient_agent: bool,
    },
    /// Created a blocklist AI block.
    AgentModeCreatedAIBlock {
        /// The client-generated exchange ID for the AI exchange (input + output turn) rendered in this AI block.
        client_exchange_id: String,

        /// The server-generated output ID for the output in this block.
        ///
        /// This is only populated if the some part of the response was successfully received.
        server_output_id: Option<ServerOutputId>,

        was_autodetected_ai_query: bool,

        /// Time from sending request to receiving the first token in the output.
        time_to_first_token_ms: Option<u128>,

        /// Time from sending request to receiving the last token in the output.
        time_to_last_token_ms: Option<u128>,

        /// `true` if the output resulted in a user-facing error.
        was_user_facing_error: bool,

        /// `true` if the the AI block was cancelled before receiving any output or while streaming
        /// output.
        cancelled: bool,

        /// The ID of the conversation this block belongs to.
        conversation_id: AIConversationId,

        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
    },
    /// The user tried to send an Agent Mode query but they have already reached their AI request
    /// limit. Note that this limit is for all AI requests, not Agent Mode alone.
    AgentModeUserAttemptedQueryAtRequestLimit {
        /// The AI request limit for the user's current plan.
        limit: usize,
    },
    AgentModeClickedEntrypoint {
        entrypoint: AgentModeEntrypoint,
    },

    /// User clicked the continue conversation button from a block footer.
    AgentModeContinueConversationButtonClicked {
        conversation_id: AIConversationId,
    },

    /// User opened the rewind confirmation dialog.
    AgentModeRewindDialogOpened {
        entrypoint: AgentModeRewindEntrypoint,
    },

    /// User executed a conversation rewind.
    AgentModeRewindExecuted {
        /// The number of AI blocks that were reverted.
        num_blocks_reverted: usize,
    },

    /// Emitted when a user explicitly attaches a block as context to an Agent Mode query.
    ///
    /// This is only emitted for the initial attachment -- its intended to express user's intent to
    /// attach context.
    ///
    /// For example, this is emitted when a user selects a block to attach as context and has no
    /// prior blocks selected. If a user has already selected a block as context and is merely
    /// changing or adding to the existing selection, this is not emitted.
    ///
    /// Also note this is not emitted if the user clicks an entrypoint that automatically attaches
    /// context (like clicking the block toolbelt stars button, for instance).
    AgentModeAttachedBlockContext {
        method: AgentModeAttachContextMethod,
    },

    /// Emitted when the user toggles the "Input Auto-detection" setting in the AI settings page or
    /// in the auto-detection "speed bump" banner.
    AgentModeToggleAutoDetectionSetting {
        is_autodetection_enabled: bool,
        origin: AgentModeAutoDetectionSettingOrigin,
    },

    /// Emitted when the input type is changed from one type to new_input_type.
    AgentModeChangedInputType {
        input: Option<String>,
        buffer_length: usize,
        is_manually_changed: bool,
        new_input_type: InputType,
        active_block_id: BlockId,
        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
    },

    /// Emitted when the user manually toggles the terminal input from AI mode to shell mode when
    /// the current input text has been auto-detected as AI input -- this is likely a natural
    /// language auto-detection false-positive.
    AgentModePotentialAutoDetectionFalsePositive(AgentModeAutoDetectionFalsePositivePayload),

    /// This is a telemetry event used to help track performance of Agent Predict in Warp,
    /// by keeping track of the context given and the predictions generated.
    AgentModePrediction {
        was_suggestion_accepted: bool,
        request_duration_ms: i64,
        is_from_ai: bool,
        does_actual_command_match_prediction: bool,
        does_actual_command_match_history_prediction: bool,
        history_prediction_likelihood: f64,
        total_history_count: usize,
        // OpenWarp leaves these optional; no telemetry sender consumes them.
        actual_next_command_run: Option<String>,
        history_based_autosuggestion_state: Option<HistoryBasedAutosuggestionState>,
        generate_ai_input_suggestions_request: Option<GenerateAIInputSuggestionsRequest>,
        generate_ai_input_suggestions_response: Option<GenerateAIInputSuggestionsResponseV2>,
    },

    /// Keeps track of number of times the user is presented with a Prompt Suggestions banner.
    PromptSuggestionShown {
        id: String,
        request_duration_ms: u64,
        block_id: Option<String>,
        view: PromptSuggestionViewType,
        /// Legacy request token from the `/passive-suggestion` request that generated this
        /// suggestion. OpenWarp keeps it optional for local diagnostics only.
        server_request_token: Option<String>,
    },

    /// Keeps track of number of times the user is presented with a Suggested Code Diff banner.
    SuggestedCodeDiffBannerShown {
        prompt_suggestion_id: String,
        /// Exchange ID of the conversation that produced this diff.
        /// `None` on the MAA passive-suggestion code path, which does not
        /// create an exchange.
        code_exchange_id: Option<AIAgentExchangeId>,
        block_id: Option<String>,
        request_duration_ms: u64,
        /// Legacy request token from the `/passive-suggestion` request. OpenWarp keeps it optional
        /// for local diagnostics only.
        server_request_token: Option<String>,
    },

    /// Keeps track of number of times the user falls back to a prompt suggestion from a suggested code diff banner.
    SuggestedCodeDiffFailed {
        prompt_suggestion_id: String,
        reason: PromptSuggestionFallbackReason,
    },

    /// Keeps track of number of times the user accepts & runs a query from the Prompt Suggestions banner.
    PromptSuggestionAccepted {
        id: String,
        view: PromptSuggestionViewType,
        interaction_source: InteractionSource,
    },

    /// Keeps track of number of times the user is presented with a Static Prompt Suggestions banner.
    StaticPromptSuggestionsBannerShown {
        id: String,
        block_id: String,
        static_prompt_suggestion_name: String,
        // OpenWarp leaves these optional; no telemetry sender consumes them.
        query: Option<String>,
        block_command: Option<String>,
        request_duration_ms: u64,
        view: PromptSuggestionViewType,
    },

    /// Keeps track of number of times the user accepts a Static Prompt Suggestion.
    StaticPromptSuggestionAccepted {
        id: String,
        view: PromptSuggestionViewType,
        interaction_source: InteractionSource,
    },

    /// Keeps track of number of times the user uses a zero state prompt suggestion & the type of suggestion used.
    ZeroStatePromptSuggestionUsed {
        suggestion_type: ZeroStatePromptSuggestionType,
        triggered_from: ZeroStatePromptSuggestionTriggeredFrom,
    },

    UnitTestSuggestionShown {
        identifiers: AIIdentifiers,
    },

    UnitTestSuggestionAccepted {
        identifiers: AIIdentifiers,
        query: Option<String>,
        interaction_source: InteractionSource,
    },

    /// Keeps track of when the user cancels a suggested prompt.
    UnitTestSuggestionCancelled {
        identifiers: AIIdentifiers,
        interaction_source: InteractionSource,
    },

    /// Emitted when a user makes their first edit to any file in a code diff suggestion from Agent
    /// Mode.
    AgentModeCodeSuggestionEditedByUser {
        /// Server-generated unique ID associated with the AI API output that generated the
        /// suggestion. Used to join client-side telemetry with server-side logs.
        output_id: ServerOutputId,
    },

    /// Emitted when a user switches between files while viewing a code diff suggestion from Agent
    /// Mode.
    AgentModeCodeFilesNavigated {
        output_id: ServerOutputId,
        source: AgentModeCodeFileNavigationSource,
    },

    AgentModeCodeDiffHunksNavigated {
        output_id: ServerOutputId,
    },

    /// Emitted when the user toggles the "Intelligent autosuggestions" setting in the AI settings page.
    ToggleIntelligentAutosuggestionsSetting {
        is_intelligent_autosuggestions_enabled: bool,
    },

    /// Emitted when the user toggles global AI.
    ToggleGlobalAI {
        is_ai_enabled: bool,
    },

    /// Emitted when the user toggles active AI.
    ToggleActiveAI {
        is_active_ai_enabled: bool,
    },

    /// Emitted when the user toggles the "Prompt Suggestions" setting in the AI settings page.
    TogglePromptSuggestionsSetting {
        is_prompt_suggestions_enabled: bool,
    },

    /// Emitted when the user toggles the "Code Suggestions" setting.
    ToggleCodeSuggestionsSetting {
        source: ToggleCodeSuggestionsSettingSource,
        is_code_suggestions_enabled: bool,
    },

    /// Emitted when the user toggles the "Natural Language Autosuggestions" setting in the AI settings page.
    ToggleNaturalLanguageAutosuggestionsSetting {
        is_natural_language_autosuggestions_enabled: bool,
    },

    /// Emitted when the user toggles the "Git Operations Autogen" setting in the AI settings page.
    ToggleGitOperationsAutogenSetting {
        is_git_operations_autogen_enabled: bool,
    },

    /// Emitted when the user toggles the "Voice Input" setting in the AI settings page.
    ToggleVoiceInputSetting {
        is_voice_input_enabled: bool,
    },

    /// Emitted when the user toggles the "Show Agent Tips" setting in the AI settings page.
    ToggleShowAgentTips {
        is_enabled: bool,
    },

    TierLimitHit(TierLimitHitEvent),
    ResourceUsageStats {
        cpu: CpuUsageStats,
        mem: MemoryUsageStats,
    },
    MemoryUsageStats {
        total_application_usage_bytes: usize,
        total_blocks: usize,
        total_lines: usize,

        /// Statistics about blocks that have been seen in the past 5 minutes.
        active_block_stats: BlockMemoryUsageStats,
        /// Statistics about blocks that haven't been seen since [5m, 1h).
        inactive_5m_stats: BlockMemoryUsageStats,
        /// Statistics about blocks that haven't been seen since [1h, 24h).
        inactive_1h_stats: BlockMemoryUsageStats,
        /// Statistics about blocks that haven't been seen since [24h, ..).
        inactive_24h_stats: BlockMemoryUsageStats,
    },
    MemoryUsageHigh {
        total_application_usage_bytes: u64,
        /// Platform-specific memory breakdown (JSON object with keys that
        /// vary by OS).  See `memory_footprint::memory_breakdown()`.
        memory_breakdown: serde_json::Value,
    },
    EnvVarCollectionInvoked(EnvVarTelemetryMetadata),
    EnvVarWorkflowParameterization(EnvVarTelemetryMetadata),

    /// The user imported settings from another terminal.
    CompletedSettingsImport {
        terminal_type: TerminalType,
        imported_settings: Vec<ParsedTerminalSetting>,
    },
    /// The user focused a terminal option to import settings from.
    SettingsImportConfigFocused(TerminalType),
    /// The user clicked the "Reset to defaults" button in the settings import onboarding block.
    SettingsImportResetButtonClicked,
    /// Completed parsing a terminal for its settings to import.
    SettingsImportConfigParsed {
        timing_data: Vec<TimingDataPoint>,
        terminal_type: TerminalType,
        settings_shown_to_user: Option<Vec<SettingType>>,
    },
    /// When parsing iTerm for settings it contained multiple hotkey bindings.
    ITermMultipleHotkeys,
    UserMenuUpgradeClicked,
    ToggleWorkspaceDecorationVisibility {
        previous_value: WorkspaceDecorationVisibility,
        new_value: WorkspaceDecorationVisibility,
    },
    UpdateAltScreenPaddingMode {
        new_mode: AltScreenPaddingMode,
    },
    AddTabWithShell {
        source: AddTabWithShellSource,
        shell: String,
    },
    AgentModeSurfacedCitations {
        citations: Vec<AgentModeCitation>,
        block_id: String,
        conversation_id: AIConversationId,
        server_output_id: Option<ServerOutputId>,
    },
    AgentModeOpenedCitation {
        citation: AgentModeCitation,
        block_id: String,
        conversation_id: AIConversationId,
        server_output_id: Option<ServerOutputId>,
    },
    ToggleLigatureRendering {
        enabled: bool,
    },
    WorkflowAliasAdded {
        workflow_id: Option<WorkflowId>,
        workflow_space: Option<TelemetrySpace>,
    },
    WorkflowAliasRemoved {
        workflow_id: Option<WorkflowId>,
        workflow_space: Option<TelemetrySpace>,
    },
    WorkflowAliasEnvVarsAttached {
        workflow_id: Option<WorkflowId>,
        workflow_space: Option<TelemetrySpace>,
        env_vars_id: Option<GenericStringObjectId>,
        env_vars_space: Option<TelemetrySpace>,
    },
    WorkflowAliasArgumentEdited {
        workflow_id: Option<WorkflowId>,
        workflow_space: Option<TelemetrySpace>,
    },

    ToggledAgentModeAutoexecuteReadonlyCommandsSetting {
        src: AutonomySettingToggleSource,
        enabled: bool,
    },
    ChangedAgentModeCodingPermissions {
        src: AutonomySettingToggleSource,
        new: AgentModeCodingPermissionsType,
    },
    RepoOutlineConstructionSuccess {
        total_parse_seconds: usize,
        file_count: usize,
    },
    RepoOutlineConstructionFailed {
        error: String,
    },
    AutoexecutedAgentModeRequestedCommand {
        reason: CommandExecutionPermissionAllowedReason,
    },
    AgenticOnboardingBlockSelected {
        block_type: OnboardingChipType,
    },
    KnowledgePaneOpened {
        entrypoint: KnowledgePaneEntrypoint,
    },
    #[cfg(feature = "local_fs")]
    CodePaneOpened {
        source: CodeSource,
        layout: EditorLayout,
        preview: bool,
    },
    #[cfg(feature = "local_fs")]
    CodePanelsFileOpened {
        entrypoint: CodePanelsFileOpenEntrypoint,
        target: FileTarget,
    },
    #[cfg(feature = "local_fs")]
    PreviewPanePromoted,
    AISuggestedRuleAdded {
        rule_id: SuggestedLoggingId,
    },
    AISuggestedRuleEdited {
        rule_id: SuggestedLoggingId,
    },
    AISuggestedRuleContentChanged {
        rule_id: SuggestedLoggingId,
        is_saved: bool,
    },
    AISuggestedAgentModeWorkflowAdded {
        logging_id: SuggestedLoggingId,
    },
    AttachedImagesToAgentModeQuery {
        num_images: usize,
        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
    },
    /// An error was encountered fetching available WSL distributions from the Registry.
    /// This typically means the user hasn't installed or enabled WSL.
    #[cfg(windows)]
    WSLRegistryError,
    #[cfg(windows)]
    AutoupdateUnableToCloseApplications,
    #[cfg(windows)]
    AutoupdateFileInUse,
    #[cfg(windows)]
    AutoupdateMutexTimeout,
    #[cfg(windows)]
    AutoupdateForcekillFailed,
    ExecutedWarpDrivePrompt {
        id: Option<WorkflowId>,
        selection_source: WorkflowSelectionSource,
    },
    ImageReceived {
        image_protocol: ImageProtocol,
    },
    /// A file from the result of an AI Agent Action exceeded the context limit.
    FileExceededContextLimit {
        identifiers: AIIdentifiers,
    },
    AgentModeError {
        identifiers: AIIdentifiers,
        error: String,
        /// Some errors are retried internally without showing to the user.
        is_user_visible: bool,
        /// Whether a conversation resume will be attempted after this error.
        will_attempt_to_resume: bool,
    },
    /// Emitted when a MultiAgent request that initially failed is successfully completed after retries.
    AgentModeRequestRetrySucceeded {
        identifiers: AIIdentifiers,
        /// The number of retry attempts that were made before success
        retry_count: usize,
        /// The original error that was retried
        original_error: String,
    },
    GrepToolSucceeded,
    GrepToolFailed {
        queries: Option<Vec<String>>,
        path: Option<String>,
        shell_type: Option<ShellType>,
        working_directory: Option<String>,
        absolute_path: Option<String>,
        command: Option<String>,
        output: Option<String>,
        error: String,
        server_output_id: Option<ServerOutputId>,
    },
    FileGlobToolSucceeded,
    FileGlobToolFailed {
        server_output_id: Option<ServerOutputId>,
    },
    MCPServerCollectionPaneOpened {
        entrypoint: MCPServerCollectionPaneEntrypoint,
    },
    MCPServerAdded {
        metadata: MCPServerTelemetryMetadata,
    },
    MCPTemplateCreated {
        source: MCPTemplateCreationSource,
        variables: Vec<TemplateVariable>,
        name: String,
    },
    MCPTemplateInstalled {
        source: MCPTemplateInstallationSource,
    },
    MCPTemplateShared,
    MCPServerSpawned {
        transport_type: MCPServerTelemetryTransportType,
        error: Option<MCPServerTelemetryError>,
        server_model: MCPServerModel,
    },
    MCPToolCallAccepted {
        server_output_id: Option<ServerOutputId>,
        tool_call: String,
        error: Option<MCPServerTelemetryError>,
    },
    ShellTerminatedPrematurely {
        shell_type: Option<ShellType>,
        shell_path: Option<String>,
        reason: String,
        reason_details: Option<String>,
        antivirus_name: Option<String>,
        long_os_version: Option<String>,
        exit_reason: Option<String>,
    },
    /// User changed the input UX mode (e.g. Universal Developer Input, UDI, mode or Classic)
    InputUXModeChanged {
        is_udi_enabled: bool,
        origin: InputUXChangeOrigin,
    },
    /// User interacted with context chips (git branch, working directory, etc.)
    ContextChipInteracted {
        chip_type: String,
        /// "opened"
        action: String,
        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
    },
    /// User used voice input functionality
    VoiceInputUsed {
        action: String, // "start", "stop", "cancel"
        /// Duration of voice session in milliseconds (for stop action)
        session_duration_ms: Option<u64>,
        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
        /// Current input mode when voice was used
        current_input_mode: InputType,
    },
    /// User interacted with @-menu for context attachment
    AtMenuInteracted {
        /// Length of the query string
        query_length: Option<usize>,
        /// "opened", "item_selected", "cancelled"
        action: String,
        /// How many items were available in the menu
        item_count: Option<usize>,
        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
        /// Current input mode when @ menu was used
        current_input_mode: InputType,
    },
    TabCloseButtonPositionUpdated {
        position: TabCloseButtonPosition,
    },
    ExpandedCodeSuggestions {
        identifiers: AIIdentifiers,
    },
    AIExecutionProfileCreated,
    AIExecutionProfileDeleted,
    AIExecutionProfileSettingUpdated {
        setting_type: String,
        setting_value: String,
    },
    AIExecutionProfileAddedToAllowlist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileAddedToDenylist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileRemovedFromAllowlist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileRemovedFromDenylist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileModelSelected {
        model_type: String,
        model_value: String,
    },
    AIExecutionProfileContextWindowSelected {
        tokens: Option<u32>,
    },
    /// The AI input was not sent because there was already an in-flight request.
    AIInputNotSent {
        entrypoint: Option<EntrypointType>,
        inputs: Vec<AIAgentInput>,
        active_server_conversation_id: Option<ServerConversationToken>,
        active_client_conversation_id: Option<AIConversationId>,
    },
    OpenSlashMenu {
        source: SlashMenuSource,
        /// Whether the inline slash commands UI is enabled.
        is_inline_ui_enabled: bool,
        /// Whether the menu was opened in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    SlashCommandAccepted {
        command_details: SlashCommandAcceptedDetails,
        /// Whether the command was accepted in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    AgentModeSetupBannerAccepted,
    AgentModeSetupBannerDismissed,
    AgentModeSetupProjectScopedRulesAction {
        action: AgentModeSetupProjectScopedRulesActionType,
    },

    InputBufferSubmitted {
        input_type: input_classifier::InputType,
        is_locked: bool,
        was_lock_set_with_empty_buffer: bool,
    },
    /// User submitted a prompt from the create project view - metadata (non-UGC)
    CreateProjectPromptSubmitted {
        /// Whether this was a custom prompt or a predefined suggestion
        is_custom_prompt: bool,
        /// For suggested prompts, this is always collected. For custom prompts, this is None.
        suggested_prompt: Option<String>,
        /// Whether this was from the FTUX
        is_ftux: bool,
    },
    /// User submitted a custom prompt from the create project view - content (UGC)
    CreateProjectPromptSubmittedContent {
        /// The custom prompt content - only collected when UGC is enabled
        custom_prompt: String,
    },
    /// User submitted a repository URL from the clone repo view
    CloneRepoPromptSubmitted {
        is_ftux: bool,
    },
    /// From the first-time user "get started" page, skip straight to terminal without
    /// creating/opening a project/repository.
    GetStartedSkipToTerminal,

    /// User selected an item from the "Recent" list on the new tab zero state
    RecentMenuItemSelected {
        // The kind of recent menu item selected
        kind: &'static str,
    },

    /// User selected a folder to open as a repo from the "Open repository" button
    OpenRepoFolderSubmitted {
        is_ftux: bool,
    },

    /// User closed the "Out of credits" banner (dismissed or purchased credits)
    OutOfCreditsBannerClosed {
        action: OutOfCreditsBannerAction,
        selected_credits: Option<i32>,
        auto_reload_checkbox_enabled: bool,
        banner_toggle_flag_enabled: bool,
        post_purchase_modal_flag_enabled: bool,
    },

    /// User closed the auto-reload modal (either dismissed or enabled auto-reload)
    AutoReloadModalClosed {
        action: AutoReloadModalAction,
        selected_credits: Option<i32>,
        banner_toggle_flag_enabled: bool,
        post_purchase_modal_flag_enabled: bool,
    },

    /// User toggled auto-reload in Billing & Usage settings
    AutoReloadToggledFromBillingSettings {
        enabled: bool,
        banner_toggle_flag_enabled: bool,
        post_purchase_modal_flag_enabled: bool,
    },

    /// Emitted when the control state of the CLI subagent changes.
    CLISubagentControlStateChanged {
        conversation_id: Option<AIConversationId>,
        block_id: BlockId,
        control_state: CLISubagentControlState,
    },
    /// Emitted when user toggles the visibility of agent responses.
    CLISubagentResponsesToggled {
        conversation_id: AIConversationId,
        block_id: BlockId,
        is_hidden: bool,
    },
    /// Emitted when user dismisses the input in the CLI subagent.
    CLISubagentInputDismissed {
        conversation_id: AIConversationId,
        block_id: BlockId,
    },
    /// Emitted when user approves a blocked action from the CLI subagent.
    CLISubagentActionExecuted {
        conversation_id: AIConversationId,
        block_id: BlockId,
        is_autoexecuted: bool,
    },
    /// Emitted when user rejects a blocked action from the CLI subagent.
    CLISubagentActionRejected {
        conversation_id: AIConversationId,
        block_id: BlockId,
        user_took_over: bool,
    },
    /// Emitted when the user toggles the Agent Management View.
    AgentManagementViewToggled {
        is_open: bool,
    },
    /// Emitted when the user opens a session from the Agent Management View.
    AgentManagementViewOpenedSession,
    /// Emitted when the user copies a session link from the Agent Management View.
    AgentManagementViewCopiedSessionLink,
    /// Detected that Warp is running in an isolated sandbox.
    DetectedIsolationPlatform {
        platform: warp_isolation_platform::IsolationPlatformType,
    },

    AgentTipShown {
        tip: String,
    },
    AgentTipClicked {
        tip: String,
        click_target: String,
    },
    /// Emitted when an agent-requested command causes the shell to exit.
    AgentExitedShellProcess {
        command: String,
        server_output_id: Option<ServerOutputId>,
    },
    /// Emitted when the user uses voice input from the CLI agent footer.
    CLIAgentToolbarVoiceInputUsed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
    },
    /// Emitted when the user attaches an image from the CLI agent footer.
    CLIAgentToolbarImageAttached {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
    },
    /// Emitted when the CLI agent footer is shown.
    CLIAgentToolbarShown {
        /// The CLI agent being shown.
        cli_agent: CLIAgentType,
    },
    /// Emitted when the user opens the CLI agent rich input editor.
    CLIAgentRichInputOpened {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// How the editor was opened (Ctrl-G or footer button).
        entrypoint: CLIAgentInputEntrypoint,
    },
    /// Emitted when the CLI agent rich input editor is closed.
    CLIAgentRichInputClosed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Why the editor was closed.
        reason: CLIAgentRichInputCloseReason,
    },
    /// Emitted when the user submits a prompt via the CLI agent rich input editor.
    CLIAgentRichInputSubmitted {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Length of the submitted prompt in characters.
        prompt_length: usize,
    },
    /// Emitted when the user clicks a plugin chip (install, update, or instructions).
    CLIAgentPluginChipClicked {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// The specific action taken.
        action: PluginChipTelemetryAction,
    },
    /// Emitted when the user dismisses the plugin chip.
    CLIAgentPluginChipDismissed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Whether this was the install or update chip.
        chip_kind: PluginChipTelemetryKind,
    },
    /// Emitted when auto plugin install or update succeeds.
    CLIAgentPluginOperationSucceeded {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Whether this was an install or update operation.
        operation: PluginChipTelemetryKind,
    },
    /// Emitted when auto plugin install or update fails.
    CLIAgentPluginOperationFailed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Whether this was an install or update operation.
        operation: PluginChipTelemetryKind,
    },
    /// Emitted when a CLI agent plugin is first recognized (SessionStart event received).
    CLIAgentPluginDetected {
        /// The CLI agent whose plugin was detected.
        cli_agent: CLIAgentType,
    },
    /// Emitted when an agent notification is shown (toast or mailbox notification).
    AgentNotificationShown {
        /// Which agent variant produced the notification.
        agent_variant: NotificationAgentVariant,
    },
    /// Emitted when the user toggles the CLI agent footer setting.
    ToggleCLIAgentToolbarSetting {
        /// Whether the setting is enabled or disabled.
        is_enabled: bool,
    },
    /// Emitted when the user toggles the "Use Agent" footer setting.
    ToggleUseAgentToolbarSetting {
        /// Whether the setting is enabled or disabled.
        is_enabled: bool,
    },
    /// Emitted when the user enters the agent view.
    AgentViewEntered {
        /// The origin/entrypoint for entering the agent view.
        origin: TelemetryAgentViewEntryOrigin,
        /// Whether a request was automatically triggered upon entry (e.g., prompt was provided).
        did_auto_trigger_request: bool,
    },
    /// Emitted when the user exits the agent view.
    AgentViewExited {
        /// The origin/entrypoint that was used when entering the agent view.
        origin: TelemetryAgentViewEntryOrigin,
        /// Whether the conversation was empty (had no exchanges) when exiting.
        was_empty: bool,
    },
    /// Emitted when the inline conversation menu is opened.
    InlineConversationMenuOpened {
        /// Whether the menu was opened in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    /// Emitted when an item is selected from the inline conversation menu.
    InlineConversationMenuItemSelected {
        /// Whether the item was selected in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    /// Emitted when the agent shortcuts view visibility is toggled.
    AgentShortcutsViewToggled {
        /// Whether the shortcuts view is now visible.
        is_visible: bool,
    },
    /// Emitted when the Codex modal is opened.
    CodexModalOpened,
    /// Emitted when the user clicks "Use Codex" in the Codex modal.
    CodexModalUseCodexClicked,
    /// Emitted when a RequestComputerUse action is approved (manually or auto-executed).
    ComputerUseApproved {
        conversation_id: AIConversationId,
        is_autoexecuted: bool,
        ambient_agent_task_id: Option<AmbientAgentTaskId>,
    },
    /// Emitted when a RequestComputerUse action is cancelled/rejected.
    ComputerUseCancelled {
        conversation_id: AIConversationId,
        ambient_agent_task_id: Option<AmbientAgentTaskId>,
    },
    /// Emitted when a warp://linear deeplink is opened.
    LinearIssueLinkOpened,
    /// Emitted when the free tier limit hit interstitial is displayed.
    FreeTierLimitHitInterstitialDisplayed,
    /// Emitted when the user clicks the "Upgrade" button in the free tier limit hit interstitial.
    FreeTierLimitHitInterstitialUpgradeButtonClicked,
    /// Emitted when the user clicks close on the free tier limit hit interstitial.
    FreeTierLimitHitInterstitialClosed,
    /// Emitted when the remote server binary check completes.
    RemoteServerBinaryCheck {
        found: bool,
        error: Option<String>,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the remote server binary installation completes.
    /// `error` is `None` on success, `Some(reason)` on failure.
    RemoteServerInstallation {
        error: Option<String>,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the remote server connection + initialization completes.
    /// `error` is `None` on success, `Some(reason)` on failure.
    RemoteServerInitialization {
        phase: remote_server::manager::RemoteServerInitPhase,
        error: Option<String>,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when an established remote server connection drops.
    RemoteServerDisconnection {
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when a client request to the remote server fails.
    RemoteServerClientRequestError {
        operation: remote_server::manager::RemoteServerOperation,
        error_type: remote_server::manager::RemoteServerErrorKind,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when a server message cannot be decoded (no parseable request_id).
    RemoteServerMessageDecodingError {
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the full remote server setup flow completes successfully.
    RemoteServerSetupDuration {
        duration_ms: u64,
        installed_binary: bool,
        remote_os: Option<String>,
        remote_arch: Option<String>,
        /// Short description of the remote libc (e.g. "glibc 2.35",
        /// "musl", "unknown"). `None` when the preinstall check did
        /// not run (e.g. macOS hosts).
        remote_libc: Option<String>,
    },
    /// Emitted when the preinstall check classifies the remote host as
    /// unsupported by the prebuilt remote-server binary, so the controller
    /// silently falls back to the legacy SSH/`RemoteCommandExecutor`
    /// flow without surfacing an install prompt.
    RemoteServerHostUnsupported {
        remote_os: Option<String>,
        remote_arch: Option<String>,
        /// Detected libc on the remote host, e.g. `"glibc 2.28"`,
        /// `"musl"`, `"unknown"`.
        detected_libc: String,
        /// Required minimum glibc reported by the script. Empty when
        /// the unsupported classification was not glibc-related.
        required_glibc: String,
    },
}

impl TelemetryEvent {
    pub fn payload(&self) -> Option<Value> {
        match self {
            TelemetryEvent::ShowedSuggestedAgentModeWorkflowChip { logging_id } => Some(json!({
                "logging_id": logging_id,
            })),
            TelemetryEvent::ShowedSuggestedAgentModeWorkflowModal { logging_id } => Some(json!({
                "logging_id": logging_id,
            })),
            TelemetryEvent::AISuggestedAgentModeWorkflowAdded { logging_id } => Some(json!({
                "logging_id": logging_id,
            })),
            TelemetryEvent::AutosuggestionInserted {
                insertion_length,
                buffer_length,
            } => {
                Some(json!({"insertion_length": insertion_length, "buffer_length": buffer_length}))
            }
            TelemetryEvent::AgentModeContinueConversationButtonClicked { conversation_id } => {
                Some(json!({"conversation_id": conversation_id}))
            }
            TelemetryEvent::AgentModeRewindDialogOpened { entrypoint } => {
                Some(json!({"entrypoint": entrypoint}))
            }
            TelemetryEvent::AgentModeRewindExecuted {
                num_blocks_reverted,
            } => Some(json!({"num_blocks_reverted": num_blocks_reverted})),
            TelemetryEvent::BootstrappingSlow(info) => Some(json!(info)),
            TelemetryEvent::BootstrappingSlowContents(info) => Some(json!(info)),
            TelemetryEvent::SessionAbandonedBeforeBootstrap {
                pending_shell,
                has_pending_ssh_session,
                was_ever_visible,
                duration_since_start,
            } => Some(json!({
                "pending_shell": pending_shell.map(|shell| shell.name()),
                "has_pending_ssh_session": has_pending_ssh_session,
                "was_ever_visible": was_ever_visible,
                "duration_since_start_secs": duration_since_start.as_secs_f32(),
            })),
            TelemetryEvent::BlockCompleted {
                block_finished_to_precmd_delay_ms,
                honor_ps1_enabled,
                num_secrets_redacted,
                num_output_lines,
                num_output_lines_truncated,
                terminal_session_id,
                is_udi_enabled,
                is_in_agent_view,
            } => Some(json!({
                "block_finished_to_precmd_delay_ms": block_finished_to_precmd_delay_ms,
                "honor_ps1_enabled": honor_ps1_enabled,
                "num_secrets_redacted": num_secrets_redacted,
                "num_output_lines": num_output_lines,
                "num_output_lines_truncated": num_output_lines_truncated,
                "terminal_session_id": terminal_session_id,
                "is_udi_enabled": is_udi_enabled,
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::ToggleFocusPaneOnHover { enabled } => Some(json!({
                "enabled": enabled,
            })),
            TelemetryEvent::BlockCompletedOnDogfoodOnly {
                block_finished_to_precmd_delay_ms,
                honor_ps1_enabled,
                num_secrets_redacted,
                num_output_lines,
                num_output_lines_truncated,
                command,
                duration,
                exit_code,
                terminal_session_id,
            } => Some(json!({
                "block_finished_to_precmd_delay_ms": block_finished_to_precmd_delay_ms,
                "honor_ps1_enabled": honor_ps1_enabled,
                "num_secrets_redacted": num_secrets_redacted,
                "num_output_lines": num_output_lines,
                "num_output_lines_truncated": num_output_lines_truncated,
                "command": command,
                "duration": duration,
                "exit_code": exit_code,
                "terminal_session_id": terminal_session_id,
            })),
            TelemetryEvent::BootstrappingSucceeded(info) => Some(json!(info)),
            TelemetryEvent::SSHBootstrapAttempt(remote_shell) => {
                Some(json!({ "shell": remote_shell.as_str() }))
            }
            TelemetryEvent::OpenContextMenu { context_menu_info } => Some(
                json!({ "type": context_menu_info.type_for_telemetry(), "open_method": context_menu_info.open_method_for_telemetry() }),
            ),
            TelemetryEvent::ContextMenuCopy(entity, cardinality) => {
                Some(json!({ "entity": entity.as_str(), "cardinality": cardinality }))
            }
            TelemetryEvent::ContextMenuFindWithinBlocks(cardinality) => {
                Some(json!({ "cardinality": cardinality }))
            }
            TelemetryEvent::ContextMenuCopyPrompt { part } => Some(json!({ "part": part })),
            TelemetryEvent::ReinputCommands(cardinality) => {
                Some(json!({ "cardinality": cardinality }))
            }
            TelemetryEvent::ContextMenuToggleGitPromptDirtyIndicator { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::BlockSelection(details) => Some(json!(details)),
            TelemetryEvent::OpenSuggestionsMenu(mode) => Some(json!(mode)),
            TelemetryEvent::ConfirmSuggestion { mode, match_type } => {
                Some(json!({ "mode": mode, "match_type": match_type }))
            }
            TelemetryEvent::EditorUnhandledModifierKey(normalized_keystroke) => {
                Some(json!(normalized_keystroke.as_str()))
            }
            TelemetryEvent::ThemeSelection { theme, entrypoint } => {
                Some(json!({ "theme": theme, "entrypoint": entrypoint }))
            }
            TelemetryEvent::AppIconSelection { icon } => Some(json!({"icon": icon})),
            TelemetryEvent::CursorDisplayType {
                cursor: cursor_display_type,
            } => Some(json!({"cursor": cursor_display_type})),
            TelemetryEvent::ObjectLinkCopied { link } => Some(json!({"link": link})),
            TelemetryEvent::FileTreeToggled {
                source,
                is_code_mode_v2,
                cli_agent,
            } => Some(
                json!({"source": source, "is_code_mode_v2": is_code_mode_v2, "cli_agent": cli_agent}),
            ),
            TelemetryEvent::FileTreeItemAttachedAsContext { is_directory } => {
                Some(json!({"is_directory": is_directory}))
            }
            TelemetryEvent::ToggleRestoreSession(enabled) => Some(json!({ "enabled": enabled })),
            TelemetryEvent::DatabaseStartUpError(error) => Some(json!(error)),
            TelemetryEvent::DatabaseReadError(error) => Some(json!(error)),
            TelemetryEvent::DatabaseWriteError(error) => Some(json!(error)),
            TelemetryEvent::AppStartup(info) => Some(json!(info)),
            TelemetryEvent::BaselineCommandLatency(info) => Some(json!(info)),
            TelemetryEvent::KeybindingChanged { action, keystroke } => {
                Some(json!({ "action": action, "keystroke": keystroke.normalized() }))
            }
            TelemetryEvent::KeybindingResetToDefault { action } => {
                Some(json!({ "action": action }))
            }
            TelemetryEvent::KeybindingRemoved { action } => Some(json!({ "action": action })),
            TelemetryEvent::FeaturesPageAction { action, value } => {
                Some(json!({"action": action, "value": value}))
            }
            TelemetryEvent::WorkflowExecuted(metadata) => Some(json!(metadata)),
            TelemetryEvent::WorkflowSelected(metadata) => Some(json!(metadata)),
            TelemetryEvent::CompleteWelcomeTipFeature {
                total_completed_count,
                tip_name,
            } => Some(
                json!({ "total_completed_count": total_completed_count, "tip_name": tip_name }),
            ),
            TelemetryEvent::NotificationsDiscoveryBannerAction(action) => {
                Some(json!({ "action": action }))
            }
            TelemetryEvent::InputModeChanged { old_mode, new_mode } => {
                Some(json!({ "old_mode": old_mode, "new_mode": new_mode }))
            }
            TelemetryEvent::NotificationsErrorBannerAction(action) => {
                Some(json!({ "action": action }))
            }
            TelemetryEvent::NotificationPermissionsRequested { source, trigger } => {
                Some(json!({ "source": source, "trigger": trigger }))
            }
            TelemetryEvent::NotificationFailedToSend { error } => Some(json!({ "error": error })),
            TelemetryEvent::NotificationSent {
                trigger,
                agent_variant,
            } => Some(json!({
                "trigger": trigger,
                "agent_variant": agent_variant,
            })),
            TelemetryEvent::NotificationsRequestPermissionsOutcome { outcome } => {
                Some(json!({ "outcome": outcome }))
            }
            TelemetryEvent::ToggleFindOption { option, enabled } => {
                Some(json!({ "option": option, "enabled": enabled }))
            }
            TelemetryEvent::SelectCommandPaletteOption(option) => Some(json!({ "option": option })),
            TelemetryEvent::PaletteSearchOpened { mode, source } => {
                Some(json!({ "mode": mode, "source": source }))
            }
            TelemetryEvent::PaletteSearchResultAccepted {
                result_type,
                filter: mode,
                buffer_length,
            } => Some(
                json!({ "result_type": result_type, "mode": mode, "buffer_length": buffer_length }),
            ),
            TelemetryEvent::PaletteSearchExited {
                filter: mode,
                buffer_length,
            } => Some(json!({ "mode": mode, "buffer_length": buffer_length })),
            TelemetryEvent::AuthCommonQuestionClicked { question } => Some(json!(question)),
            TelemetryEvent::AuthToggleFAQ { open } => {
                let payload = if *open { "open" } else { "close" };
                Some(json!(payload))
            }
            TelemetryEvent::TabRenamed(rename_event) => Some(json!(rename_event)),
            TelemetryEvent::MoveActiveTab { direction } => Some(json!({ "direction": direction })),
            TelemetryEvent::MoveTab { direction } => Some(json!({ "direction": direction })),
            TelemetryEvent::TabOperations { action } => Some(json!({ "action": action })),
            TelemetryEvent::ThinStrokesSettingChanged { new_value } => {
                Some(json!({ "new_value": new_value }))
            }
            TelemetryEvent::BookmarkBlockToggled { enable_bookmark } => {
                Some(json!({ "enable_bookmark": enable_bookmark }))
            }
            TelemetryEvent::OpenLink { link, open_with } => {
                Some(json!({"link_type": link, "open_with": open_with}))
            }
            TelemetryEvent::OpenChangelogLink { url } => Some(json!({ "url": url })),
            TelemetryEvent::CommandXRayTriggered { trigger } => Some(json!({ "trigger": trigger })),
            TelemetryEvent::SaveLaunchConfig { state } => Some(json!({ "state": state })),
            TelemetryEvent::SaveAsWorkflowModal { source } => Some(json!({ "source": source })),
            TelemetryEvent::CommandCorrection { event } => Some(json!({ "event": event })),
            TelemetryEvent::SetLineHeight { new_value } => Some(json!({ "new_value": new_value })),
            TelemetryEvent::CommandSearchOpened { has_initial_query } => {
                Some(json!({ "has_initial_query": has_initial_query }))
            }
            TelemetryEvent::CommandSearchExited {
                buffer_length,
                query_filter,
            } => Some(json!({ "buffer_length": buffer_length, "query_filter": query_filter })),
            TelemetryEvent::CommandSearchResultAccepted {
                result_index,
                result_type,
                query_filter,
                buffer_length,
                was_immediately_executed,
            } => Some(json!({
                "result_index": result_index,
                "result_type": result_type,
                "query_filter": query_filter,
                "buffer_length": buffer_length,
                "was_immediately_executed": was_immediately_executed
            })),
            TelemetryEvent::CommandSearchFilterChanged { new_filter } => {
                Some(json!({ "new_filter": new_filter }))
            }
            TelemetryEvent::CommandSearchAsyncQueryCompleted {
                filters,
                error_payload,
            } => Some(json!({ "filter": filters, "error": error_payload })),
            TelemetryEvent::AICommandSearchOpened { entrypoint } => {
                Some(json!({ "entrypoint": entrypoint }))
            }
            TelemetryEvent::OpenNotebook(metadata) => Some(json!(metadata)),
            TelemetryEvent::EditNotebook {
                metadata,
                meaningful_change,
            } => Some(json!({
                "notebook_id": metadata.notebook_id,
                "team_uid": metadata.team_uid,
                "meaningful_change": meaningful_change,
            })),
            TelemetryEvent::NotebookAction(event) => Some(json!(event)),
            TelemetryEvent::UserInitiatedClose { initiated_on } => {
                Some(json!({ "initiated_on": initiated_on }))
            }
            TelemetryEvent::QuitModalShown {
                running_processes,
                shared_sessions,
                modal_for,
            } => Some(
                json!({ "running_processes": running_processes, "shared_sessions": shared_sessions, "modal_for": modal_for }),
            ),
            TelemetryEvent::QuitModalCancel {
                nav_palette,
                modal_for,
            } => Some(json!({ "nav_palette": nav_palette, "modal_for": modal_for })),
            TelemetryEvent::LogOutModalCancel { nav_palette } => {
                Some(json!({ "nav_palette": nav_palette }))
            }
            TelemetryEvent::SetBlurRadius { blur_radius } => {
                Some(json!({ "blur_radius": blur_radius }))
            }
            TelemetryEvent::SetOpacity { opacity } => Some(json!({ "opacity": opacity })),
            TelemetryEvent::ToggleDimInactivePanes { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleJumpToBottomofBlockButton { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::PtySpawned { mode } => Some(json!({ "mode": mode })),
            TelemetryEvent::InitialWorkingDirectoryConfigurationChanged {
                advanced_mode_enabled,
            } => Some(json!({ "advanced_mode_enabled": advanced_mode_enabled })),
            TelemetryEvent::OpenedWarpAI { source } => Some(json!({ "source": source })),
            TelemetryEvent::WarpAIRequestIssued { result } => Some(json!({ "result": result })),
            TelemetryEvent::WarpAIAction { action_type } => {
                Some(json!({ "action_type": action_type }))
            }
            TelemetryEvent::MCPServerCollectionPaneOpened { entrypoint } => {
                Some(json!({ "entrypoint": entrypoint }))
            }
            TelemetryEvent::MCPServerAdded { metadata } => Some(json!({
                "object_id": metadata.object_id,
                "name": metadata.name,
                "transport_type": metadata.transport_type,
                "mcp_server": metadata.mcp_server,
            })),
            TelemetryEvent::MCPTemplateCreated {
                source,
                variables,
                name,
            } => Some(json!({
                "source": source,
                "variables": variables,
                "name": name,
            })),
            TelemetryEvent::MCPTemplateInstalled { source } => Some(json!({
                "source": source,
            })),
            TelemetryEvent::MCPTemplateShared => None,
            TelemetryEvent::MCPServerSpawned {
                transport_type,
                server_model,
                error,
            } => Some(
                json!({"transport_type": transport_type, "server_model": server_model, "error": error}),
            ),
            TelemetryEvent::MCPToolCallAccepted {
                server_output_id,
                tool_call,
                error,
            } => Some(json!({
                "server_output_id": server_output_id,
                "tool_call": tool_call,
                "error": error,
            })),
            TelemetryEvent::KnowledgePaneOpened { entrypoint } => {
                Some(json!({ "entrypoint": entrypoint }))
            }
            #[cfg(feature = "local_fs")]
            TelemetryEvent::CodePaneOpened {
                source,
                layout,
                preview,
            } => Some(
                json!({ "source": source.telemetry_source_name(), "layout": layout, "preview": preview }),
            ),
            #[cfg(feature = "local_fs")]
            TelemetryEvent::CodePanelsFileOpened { entrypoint, target } => {
                let (target, layout, editor) = match target {
                    FileTarget::MarkdownViewer(layout) => {
                        ("warp_markdown_viewer", Some(*layout), None)
                    }
                    FileTarget::CodeEditor(layout) => ("warp_code_editor", Some(*layout), None),
                    FileTarget::EnvEditor => ("env_editor", None, None),
                    FileTarget::SystemDefault => ("system_default", None, None),
                    FileTarget::SystemGeneric => ("system_generic", None, None),
                    FileTarget::ExternalEditor(editor) => ("external_editor", None, Some(*editor)),
                };

                Some(json!({
                    "entrypoint": entrypoint,
                    "target": target,
                    "layout": layout,
                    "editor": editor,
                }))
            }
            #[cfg(feature = "local_fs")]
            TelemetryEvent::PreviewPanePromoted => None,
            TelemetryEvent::CodeSelectionAddedAsContext { destination } => Some(json!({
                "destination": destination,
            })),
            TelemetryEvent::AISuggestedRuleAdded { rule_id } => Some(json!({ "rule_id": rule_id })),
            TelemetryEvent::AISuggestedRuleEdited { rule_id } => {
                Some(json!({ "rule_id": rule_id }))
            }
            TelemetryEvent::AISuggestedRuleContentChanged { rule_id, is_saved } => {
                Some(json!({ "rule_id": rule_id, "is_saved": is_saved }))
            }
            TelemetryEvent::UsedWarpAIPreparedPrompt { prompt } => {
                Some(json!({ "prompt": prompt }))
            }
            TelemetryEvent::ExperimentTriggered {
                experiment,
                layer,
                group_assignment,
            } => Some(
                json!({ "experiment": experiment, "layer": layer, "group_assignment": group_assignment }),
            ),
            TelemetryEvent::ToggleSyncAllPanesInAllTabs { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleSyncAllPanesInTab { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleTabIndicators { enabled } => Some(json!({ "enabled": enabled })),
            TelemetryEvent::TogglePreserveActiveTabColor { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::DeclineSubshellBootstrap { remember } => {
                Some(json!({ "remember": remember }))
            }
            TelemetryEvent::AgentToolbarDismissed => None,
            TelemetryEvent::WarpifyFooterShown { is_ssh }
            | TelemetryEvent::WarpifyFooterAcceptedWarpify { is_ssh } => {
                Some(json!({ "is_ssh": is_ssh }))
            }
            TelemetryEvent::ToggleSameLinePrompt { enabled } => Some(json!({ "enabled": enabled })),
            TelemetryEvent::TriggerSubshellBootstrap {
                triggered_by_rc_file_snippet,
            } => Some(json!({
                "triggered_by_rc_file_snippet": triggered_by_rc_file_snippet
            })),
            TelemetryEvent::OpenLaunchConfig {
                ui_location,
                open_in_active_window,
            } => Some(
                json!({ "ui_location": ui_location, "open_in_active_window": open_in_active_window }),
            ),
            TelemetryEvent::ToggleWarpAI { opened } => Some(json!({ "opened": opened })),
            TelemetryEvent::ToggleSecretRedaction { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleObfuscateSecret { interaction } => {
                Some(json!({ "interaction": interaction }))
            }
            TelemetryEvent::AutoGenerateMetadataError { error_payload } => {
                Some(json!({ "error": error_payload }))
            }
            TelemetryEvent::UpdateSortingChoice { sorting_choice } => {
                Some(json!({ "sorting_choice": sorting_choice }))
            }
            TelemetryEvent::UndoClose { item_type } => Some(json!({ "item_type": item_type })),
            TelemetryEvent::PromptEdited { prompt, entrypoint } => Some(json!({
                "prompt": prompt,
                "entrypoint": entrypoint
            })),
            TelemetryEvent::OpenPromptEditor { entrypoint } => {
                Some(json!({ "entrypoint": entrypoint }))
            }
            TelemetryEvent::PtyThroughput {
                max_bytes_per_second,
            } => Some(json!({
                "max_bytes_per_second": max_bytes_per_second,
            })),
            TelemetryEvent::DuplicateObject(object_type) => {
                Some(json!({ "object_type": object_type }))
            }
            TelemetryEvent::ExportObject(object_type) => {
                Some(json!({ "object_type": object_type }))
            }
            TelemetryEvent::PageUpDownInEditorPressed {
                is_empty_editor,
                is_down,
            } => Some(json!({"is_empty_editor": is_empty_editor, "is_down": is_down})),
            TelemetryEvent::UnsupportedShell { shell } => Some(json!({ "shell": shell })),
            TelemetryEvent::CopyObjectToClipboard(object_type) => {
                Some(json!({ "object_type": object_type }))
            }
            TelemetryEvent::OpenAndWarpifyDockerSubshell { shell_type } => {
                Some(json!({ "shell_type": shell_type }))
            }
            TelemetryEvent::ToggleBlockFilterQuery { enabled, source } => {
                Some(json!({"enabled": enabled, "source": source}))
            }
            TelemetryEvent::ToggleBlockFilterRegex { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleShowBlockDividers { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleBlockFilterCaseSensitivity { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleBlockFilterInvert { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::UpdateBlockFilterQueryContextLines { num_context_lines } => {
                Some(json!({ "num_context_lines": num_context_lines }))
            }
            TelemetryEvent::ToggleNewWindowsAtCustomSize { enabled } => {
                Some(json!({"enabled": enabled}))
            }
            TelemetryEvent::ToggleSshTmuxWrapper { enabled } => Some(json!({"enabled": enabled})),
            TelemetryEvent::ToggleSshWarpification { enabled } => Some(json!({"enabled": enabled})),
            TelemetryEvent::SetSshExtensionInstallMode { mode } => Some(json!({"mode": mode})),
            TelemetryEvent::SshRemoteServerChoiceDoNotAskAgainToggled { checked } => {
                Some(json!({"checked": checked}))
            }
            TelemetryEvent::SshInteractiveSessionDetected(ssh_interactive_session_detected) => {
                Some(json!({"ssh_interactive_session": ssh_interactive_session_detected}))
            }
            TelemetryEvent::SshTmuxWarpificationSuccess {
                duration_ms,
                tmux_installation,
            } => Some(json!({
                "duration_ms": duration_ms,
                "tmux_installation": *tmux_installation,
            })),
            TelemetryEvent::SshTmuxWarpificationErrorBlock {
                error,
                tmux_installation,
            } => Some(json!({
                "error": error,
                "tmux_installation": *tmux_installation,
            })),
            TelemetryEvent::WebObjectOpenedOnDesktop { object_metadata } => Some(json!({
                "object": object_metadata,
            })),
            TelemetryEvent::ToggleSnackbarInActivePane { show_snackbar } => {
                Some(json!({ "show_snackbar": show_snackbar }))
            }
            TelemetryEvent::PaneDropped { drop_location } => {
                Some(json!({ "location": drop_location }))
            }
            TelemetryEvent::InviteTeammates {
                num_teammates,
                team_uid,
            } => Some(json!({"num_teammates": num_teammates, "team_uid": team_uid})),
            TelemetryEvent::AgentModeCreatedAIBlock {
                client_exchange_id,
                server_output_id,
                was_autodetected_ai_query,
                time_to_first_token_ms,
                time_to_last_token_ms,
                was_user_facing_error,
                cancelled,
                conversation_id,
                is_udi_enabled,
            } => Some(json!({
                "client_exchange_id": client_exchange_id,
                "server_output_id": server_output_id,
                "was_autodetected_ai_query": was_autodetected_ai_query,
                "time_to_first_token_ms": time_to_first_token_ms,
                "time_to_last_token_ms": time_to_last_token_ms,
                "was_user_facing_error": was_user_facing_error,
                "cancelled": cancelled,
                "conversation_id": conversation_id,
                "is_udi_enabled": is_udi_enabled,
            })),
            TelemetryEvent::TierLimitHit(event) => Some(json!(event)),
            TelemetryEvent::AgentModeUserAttemptedQueryAtRequestLimit { limit } => {
                Some(json!({"limit": limit}))
            }
            TelemetryEvent::AgentModeClickedEntrypoint { entrypoint } => {
                Some(json!({"entrypoint": entrypoint}))
            }
            TelemetryEvent::AgentModeAttachedBlockContext { method } => {
                Some(json!({"method": method}))
            }
            TelemetryEvent::AgentModeToggleAutoDetectionSetting {
                is_autodetection_enabled,
                origin,
            } => Some(
                json!({"is_autodetection_enabled": is_autodetection_enabled, "origin": origin }),
            ),
            TelemetryEvent::ToggleIntelligentAutosuggestionsSetting {
                is_intelligent_autosuggestions_enabled,
            } => Some(
                json!({"is_intelligent_autosuggestions_enabled": is_intelligent_autosuggestions_enabled}),
            ),
            // Using legacy name to avoid breaking telemetry.
            TelemetryEvent::TogglePromptSuggestionsSetting {
                is_prompt_suggestions_enabled,
            } => Some(
                json!({"is_agent_mode_query_suggestions_enabled": is_prompt_suggestions_enabled}),
            ),
            TelemetryEvent::ToggleCodeSuggestionsSetting {
                source,
                is_code_suggestions_enabled,
            } => Some(
                json!({"source": source, "is_code_suggestions_enabled": is_code_suggestions_enabled}),
            ),
            TelemetryEvent::ToggleNaturalLanguageAutosuggestionsSetting {
                is_natural_language_autosuggestions_enabled,
            } => Some(
                json!({"is_natural_language_autosuggestions_enabled": is_natural_language_autosuggestions_enabled}),
            ),
            TelemetryEvent::ToggleGitOperationsAutogenSetting {
                is_git_operations_autogen_enabled,
            } => Some(
                json!({"is_git_operations_autogen_enabled": is_git_operations_autogen_enabled}),
            ),
            TelemetryEvent::ToggleVoiceInputSetting {
                is_voice_input_enabled,
            } => Some(json!({"is_voice_input_enabled": is_voice_input_enabled})),
            TelemetryEvent::AgentModePotentialAutoDetectionFalsePositive(
                AgentModeAutoDetectionFalsePositivePayload::InternalDogfoodUsers { input_text },
            ) => Some(json!({"input_text": input_text})),
            TelemetryEvent::AgentModeChangedInputType {
                input,
                buffer_length,
                is_manually_changed,
                new_input_type,
                active_block_id,
                is_udi_enabled,
            } => Some(
                json!({"input": input, "buffer_length": buffer_length, "is_manually_changed": is_manually_changed, "new_input_type": new_input_type, "active_block_id": active_block_id, "is_udi_enabled": is_udi_enabled}),
            ),
            TelemetryEvent::AgentModePrediction {
                was_suggestion_accepted,
                request_duration_ms,
                is_from_ai,
                does_actual_command_match_prediction,
                does_actual_command_match_history_prediction,
                history_prediction_likelihood,
                total_history_count,
                actual_next_command_run,
                history_based_autosuggestion_state,
                generate_ai_input_suggestions_request,
                generate_ai_input_suggestions_response,
            } => {
                let (history_command_prediction, history_command_prediction_likelihood) =
                    if let Some(state) = history_based_autosuggestion_state {
                        (
                            Some(state.history_command_prediction.clone()),
                            Some(state.history_command_prediction_likelihood),
                        )
                    } else {
                        (None, None)
                    };

                Some(json!({
                    "was_suggestion_accepted": was_suggestion_accepted,
                    "request_duration_ms": request_duration_ms,
                    "is_from_ai": is_from_ai,
                    "does_actual_command_match_prediction": does_actual_command_match_prediction,
                    "does_actual_command_match_history_prediction": does_actual_command_match_history_prediction,
                    "history_prediction_likelihood": history_prediction_likelihood,
                    "total_history_count": total_history_count,
                    "actual_next_command_run": actual_next_command_run,
                    "generate_ai_input_suggestions_request": generate_ai_input_suggestions_request,
                    "generate_ai_input_suggestions_response": generate_ai_input_suggestions_response,
                    "history_command_prediction": history_command_prediction,
                    "history_command_prediction_likelihood": history_command_prediction_likelihood,
                }))
            }
            TelemetryEvent::PromptSuggestionShown {
                id,
                request_duration_ms,
                block_id,
                view,
                server_request_token,
            } => Some(json!({
                "id": id,
                "request_duration_ms": request_duration_ms,
                "block_id": block_id,
                "view": view,
                "server_request_token": server_request_token,
            })),
            TelemetryEvent::SuggestedCodeDiffBannerShown {
                prompt_suggestion_id,
                code_exchange_id,
                block_id,
                request_duration_ms,
                server_request_token,
            } => Some(json!({
                "prompt_suggestion_id": prompt_suggestion_id,
                "code_exchange_id": code_exchange_id,
                "block_id": block_id,
                "request_duration_ms": request_duration_ms,
                "server_request_token": server_request_token,
            })),
            TelemetryEvent::SuggestedCodeDiffFailed {
                prompt_suggestion_id,
                reason,
            } => Some(json!({
                "prompt_suggestion_id": prompt_suggestion_id,
                "reason": reason,
            })),
            TelemetryEvent::PromptSuggestionAccepted {
                id,
                view,
                interaction_source,
            } => Some(json!({
                "id": id,
                "view": view,
                "interaction_source": interaction_source,
            })),
            TelemetryEvent::StaticPromptSuggestionsBannerShown {
                id,
                query,
                block_id,
                block_command,
                static_prompt_suggestion_name,
                request_duration_ms,
                view,
            } => Some(json!({
                "id": id,
                "query": query,
                "block_id": block_id,
                "block_command": block_command,
                "static_prompt_suggestion_name": static_prompt_suggestion_name,
                "request_duration_ms": request_duration_ms,
                "view": view,
            })),
            TelemetryEvent::StaticPromptSuggestionAccepted {
                id,
                view,
                interaction_source,
            } => Some(json!({
                "id": id,
                "view": view,
                "interaction_source": interaction_source,
            })),
            TelemetryEvent::ZeroStatePromptSuggestionUsed {
                suggestion_type,
                triggered_from,
            } => Some(json!({"type": suggestion_type, "triggered_from": triggered_from})),
            TelemetryEvent::UnitTestSuggestionShown { identifiers } => Some(json!({
                "server_output_id": identifiers.server_output_id,
                "exchange_id": identifiers.client_exchange_id,
                "conversation_id": identifiers.server_conversation_id,
            })),
            TelemetryEvent::UnitTestSuggestionAccepted {
                identifiers,
                query,
                interaction_source,
            } => Some(json!({
                "server_output_id": identifiers.server_output_id,
                "exchange_id": identifiers.client_exchange_id,
                "conversation_id": identifiers.server_conversation_id,
                "query": query,
                "interaction_source": interaction_source,
            })),
            TelemetryEvent::UnitTestSuggestionCancelled {
                identifiers,
                interaction_source,
            } => Some(json!({
                "server_output_id": identifiers.server_output_id,
                "exchange_id": identifiers.client_exchange_id,
                "conversation_id": identifiers.server_conversation_id,
                "interaction_source": interaction_source,
            })),
            TelemetryEvent::AgentModeCodeSuggestionEditedByUser { output_id } => {
                Some(json!({"output_id": output_id}))
            }
            TelemetryEvent::AgentModeCodeFilesNavigated { output_id, source } => {
                Some(json!({"output_id": output_id, "source": source}))
            }
            TelemetryEvent::AgentModeCodeDiffHunksNavigated { output_id } => {
                Some(json!({"output_id": output_id}))
            }
            TelemetryEvent::ResourceUsageStats { cpu, mem } => Some(json!({
                "cpu": cpu,
                "mem": {
                    // Only report the total application usage; skip sending
                    // the additional, more detailed usage information.
                    "total_application_usage_bytes": mem.total_application_usage_bytes,
                },
            })),
            TelemetryEvent::MemoryUsageStats {
                total_application_usage_bytes,
                total_blocks,
                total_lines,
                active_block_stats,
                inactive_5m_stats,
                inactive_1h_stats,
                inactive_24h_stats,
            } => Some(json!({
                "total_application_usage_bytes": total_application_usage_bytes,
                "total_blocks": total_blocks,
                "total_lines": total_lines,
                "active_block_stats": active_block_stats,
                "inactive_5m_stats": inactive_5m_stats,
                "inactive_1h_stats": inactive_1h_stats,
                "inactive_24h_stats": inactive_24h_stats
            })),
            TelemetryEvent::MemoryUsageHigh {
                total_application_usage_bytes,
                memory_breakdown,
            } => Some(json!({
                "total_application_usage_bytes": total_application_usage_bytes,
                "memory_breakdown": memory_breakdown,
            })),
            TelemetryEvent::EnvVarCollectionInvoked(metadata) => Some(json!(metadata)),
            TelemetryEvent::EnvVarWorkflowParameterization(metadata) => Some(json!(metadata)),
            TelemetryEvent::CompletedSettingsImport {
                terminal_type,
                imported_settings,
            } => Some(
                json!({ "terminal_type": terminal_type, "imported_settings": imported_settings}),
            ),
            TelemetryEvent::SettingsImportConfigParsed {
                timing_data,
                terminal_type,
                settings_shown_to_user,
            } => Some(
                json!({"timing_data": timing_data,  "terminal_type": terminal_type, "settings_shown_to_user": settings_shown_to_user}),
            ),
            TelemetryEvent::SettingsImportConfigFocused(terminal_type_and_profile) => {
                Some(json!({"terminal_and_type_profile": terminal_type_and_profile}))
            }
            TelemetryEvent::InitiateAnonymousUserSignup { entrypoint } => {
                Some(json!({"entrypoint": entrypoint}))
            }
            TelemetryEvent::AnonymousUserAttemptLoginGatedFeature { feature } => {
                Some(json!({"feature": feature}))
            }
            TelemetryEvent::ToggleWorkspaceDecorationVisibility {
                previous_value,
                new_value,
            } => Some(json!({
                "previous_value": previous_value,
                "new_value": new_value,
            })),
            TelemetryEvent::UpdateAltScreenPaddingMode { new_mode } => Some(json!({
                "new_mode": new_mode,
            })),
            TelemetryEvent::AddTabWithShell { source, shell } => {
                Some(json!({ "source": source, "shell": shell }))
            }
            TelemetryEvent::AgentModeSurfacedCitations {
                citations,
                block_id,
                conversation_id,
                server_output_id,
            } => Some(
                json!({ "citations": citations, "block_id": block_id, "conversation_id": conversation_id, "server_output_id": server_output_id }),
            ),
            TelemetryEvent::AgentModeOpenedCitation {
                citation,
                block_id,
                conversation_id,
                server_output_id,
            } => Some(
                json!({ "citation": citation, "block_id": block_id, "conversation_id": conversation_id, "server_output_id": server_output_id }),
            ),
            TelemetryEvent::ToggleGlobalAI { is_ai_enabled } => {
                Some(json!({"is_ai_enabled": is_ai_enabled}))
            }
            TelemetryEvent::ToggleActiveAI {
                is_active_ai_enabled,
            } => Some(json!({"is_active_ai_enabled": is_active_ai_enabled})),
            TelemetryEvent::ToggleLigatureRendering { enabled } => {
                Some(json!({"enabled": enabled}))
            }
            TelemetryEvent::WorkflowAliasAdded {
                workflow_id,
                workflow_space,
            } => Some(json!({
                "workflow_id": workflow_id,
                "workflow_space": workflow_space,
            })),
            TelemetryEvent::WorkflowAliasRemoved {
                workflow_id,
                workflow_space,
            } => Some(json!({
                "workflow_id": workflow_id,
                "workflow_space": workflow_space,
            })),
            TelemetryEvent::WorkflowAliasArgumentEdited {
                workflow_id,
                workflow_space,
            } => Some(json!({
                "workflow_id": workflow_id,
                "workflow_space": workflow_space,
            })),
            TelemetryEvent::WorkflowAliasEnvVarsAttached {
                workflow_id,
                workflow_space,
                env_vars_id,
                env_vars_space,
            } => Some(json!({
                "workflow_id": workflow_id,
                "workflow_space": workflow_space,
                "env_vars_id": env_vars_id,
                "env_vars_space": env_vars_space,
            })),
            TelemetryEvent::AutoupdateRelaunchAttempt { new_version } => Some(json!({
                "new_version": new_version,
            })),
            TelemetryEvent::ToggledAgentModeAutoexecuteReadonlyCommandsSetting { src, enabled } => {
                Some(json!({
                    "source": src,
                    "enabled": enabled,
                }))
            }
            TelemetryEvent::ChangedAgentModeCodingPermissions { src, new } => Some(json!({
                "source": src,
                "new": new,
            })),
            TelemetryEvent::RepoOutlineConstructionSuccess {
                total_parse_seconds,
                file_count,
            } => Some(json!({
                "total_parse_seconds": total_parse_seconds,
                "file_count": file_count,
            })),
            TelemetryEvent::RepoOutlineConstructionFailed { error } => Some(json!({
                "error": error,
            })),
            TelemetryEvent::AutoexecutedAgentModeRequestedCommand { reason } => Some(json!({
                "reason": reason,
            })),
            TelemetryEvent::AgenticOnboardingBlockSelected { block_type } => Some(json!({
                "block_type": block_type,
            })),
            TelemetryEvent::AttachedImagesToAgentModeQuery {
                num_images,
                is_udi_enabled,
            } => Some(json!({
                "num_images": num_images,
                "is_udi_enabled": is_udi_enabled,
            })),
            TelemetryEvent::ExecutedWarpDrivePrompt {
                id,
                selection_source,
            } => Some(json!({
                "id": id,
                "selection_source": selection_source,
            })),
            TelemetryEvent::ImageReceived { image_protocol } => Some(json!({
                "image_protocol": image_protocol,
            })),
            TelemetryEvent::FileExceededContextLimit { identifiers } => Some(json!({
                "server_output_id": identifiers.server_output_id,
                "exchange_id": identifiers.client_exchange_id,
                "conversation_id": identifiers.server_conversation_id,
            })),
            TelemetryEvent::AgentModeError {
                identifiers,
                error,
                is_user_visible,
                will_attempt_to_resume,
            } => Some(json!({
                "server_output_id": identifiers.server_output_id,
                "exchange_id": identifiers.client_exchange_id,
                "conversation_id": identifiers.server_conversation_id,
                "error": error,
                "is_user_visible": is_user_visible,
                "will_attempt_to_resume": will_attempt_to_resume,
            })),
            TelemetryEvent::AgentModeRequestRetrySucceeded {
                identifiers,
                retry_count,
                original_error,
            } => Some(json!({
                "server_output_id": identifiers.server_output_id,
                "exchange_id": identifiers.client_exchange_id,
                "conversation_id": identifiers.server_conversation_id,
                "retry_count": retry_count,
                "original_error": original_error,
            })),
            TelemetryEvent::GrepToolFailed {
                queries,
                path,
                shell_type,
                working_directory,
                absolute_path,
                command,
                output,
                error,
                server_output_id,
            } => Some(json!({
                "queries": queries,
                "path": path,
                "shell_type": shell_type,
                "working_directory": working_directory,
                "absolute_path": absolute_path,
                "command": command,
                "output": output,
                "error": error,
                "server_output_id": server_output_id,
            })),
            TelemetryEvent::FileGlobToolFailed { server_output_id } => Some(json!({
                "server_output_id": server_output_id,
            })),
            TelemetryEvent::ShellTerminatedPrematurely {
                shell_type,
                shell_path,
                reason,
                reason_details,
                antivirus_name,
                long_os_version,
                exit_reason,
            } => Some(json!({
                "shell_type": shell_type,
                "shell_path": shell_path,
                "reason": reason,
                "reason_details": reason_details,
                "antivirus_name": antivirus_name,
                "long_os_version": long_os_version,
                "exit_reason": exit_reason,
            })),
            TelemetryEvent::InputUXModeChanged {
                is_udi_enabled,
                origin,
            } => Some(json!({
                "is_udi_enabled": is_udi_enabled,
                "origin": origin,
            })),
            TelemetryEvent::ContextChipInteracted {
                chip_type,
                action,
                is_udi_enabled,
            } => Some(json!({
                "chip_type": chip_type,
                "action": action,
                "is_udi_enabled": is_udi_enabled,
            })),
            TelemetryEvent::VoiceInputUsed {
                action,
                session_duration_ms,
                is_udi_enabled,
                current_input_mode,
            } => Some(json!({
                "action": action,
                "session_duration_ms": session_duration_ms,
                "is_udi_enabled": is_udi_enabled,
                "current_input_mode": current_input_mode,
            })),
            TelemetryEvent::AtMenuInteracted {
                action,
                query_length,
                item_count,
                is_udi_enabled,
                current_input_mode,
            } => Some(json!({
                "action": action,
                "query_length": query_length,
                "item_count": item_count,
                "is_udi_enabled": is_udi_enabled,
                "current_input_mode": current_input_mode,
            })),
            TelemetryEvent::TabCloseButtonPositionUpdated { position } => Some(json!({
                "position": position,
            })),
            TelemetryEvent::ExpandedCodeSuggestions { identifiers } => Some(json!({
                "server_output_id": identifiers.server_output_id,
                "exchange_id": identifiers.client_exchange_id,
                "conversation_id": identifiers.server_conversation_id,
            })),
            TelemetryEvent::BackgroundBlockStarted
            | TelemetryEvent::SessionCreation
            | TelemetryEvent::Login
            | TelemetryEvent::ContextMenuInsertSelectedText
            | TelemetryEvent::ContextMenuCopySelectedText
            | TelemetryEvent::JumpToPreviousCommand
            | TelemetryEvent::TabSingleResultAutocompletion
            | TelemetryEvent::CopyInviteLink
            | TelemetryEvent::OpenThemeChooser
            | TelemetryEvent::OpenThemeCreatorModal
            | TelemetryEvent::CreateCustomTheme
            | TelemetryEvent::DeleteCustomTheme
            | TelemetryEvent::SplitPane
            | TelemetryEvent::UnableToAutoUpdateToNewVersion
            | TelemetryEvent::SkipOnboardingSurvey
            | TelemetryEvent::LoggedOutStartup
            | TelemetryEvent::OpenWorkflowSearch
            | TelemetryEvent::OpenQuakeModeWindow
            | TelemetryEvent::OpenWelcomeTips
            | TelemetryEvent::DismissWelcomeTips
            | TelemetryEvent::ShowNotificationsDiscoveryBanner
            | TelemetryEvent::ShowNotificationsErrorBanner
            | TelemetryEvent::NotificationClicked
            | TelemetryEvent::SignUpButtonClicked
            | TelemetryEvent::OpenNewSessionFromFilePath
            | TelemetryEvent::OpenTeamFromURI
            | TelemetryEvent::SelectNavigationPaletteItem
            | TelemetryEvent::DragAndDropTab
            | TelemetryEvent::EditedInputBeforePrecmd
            | TelemetryEvent::TriedToExecuteBeforePrecmd
            | TelemetryEvent::JumpToBookmark
            | TelemetryEvent::JumpToBottomofBlockButtonClicked
            | TelemetryEvent::ShowInFileExplorer
            | TelemetryEvent::OpenLaunchConfigSaveModal
            | TelemetryEvent::OpenLaunchConfigFile
            | TelemetryEvent::TeamCreated
            | TelemetryEvent::TeamJoined
            | TelemetryEvent::TeamLeft
            | TelemetryEvent::TeamLinkCopied
            | TelemetryEvent::RemovedUserFromTeam
            | TelemetryEvent::DeletedWorkflow
            | TelemetryEvent::DeletedNotebook
            | TelemetryEvent::ToggleApprovalsModal
            | TelemetryEvent::ChangedInviteViewOption(_)
            | TelemetryEvent::SendEmailInvites
            | TelemetryEvent::ResourceCenterOpened
            | TelemetryEvent::ResourceCenterTipsCompleted
            | TelemetryEvent::ResourceCenterTipsSkipped
            | TelemetryEvent::KeybindingsPageOpened
            | TelemetryEvent::OpenedAltScreenFind
            | TelemetryEvent::QuitModalDisabled
            | TelemetryEvent::UserInitiatedLogOut
            | TelemetryEvent::LogOutModalShown
            | TelemetryEvent::WarpAICharacterLimitExceeded
            | TelemetryEvent::OpenInputContextMenu
            | TelemetryEvent::InputCutSelectedText
            | TelemetryEvent::InputCopySelectedText
            | TelemetryEvent::InputSelectAll
            | TelemetryEvent::InputPaste
            | TelemetryEvent::InputCommandSearch
            | TelemetryEvent::InputAICommandSearch
            | TelemetryEvent::InputAskWarpAI
            | TelemetryEvent::SetNewWindowsAtCustomSize
            | TelemetryEvent::DisableInputSync
            | TelemetryEvent::ShowSubshellBanner
            | TelemetryEvent::SshTmuxWarpifyBannerDisplayed
            | TelemetryEvent::AddDenylistedSubshellCommand
            | TelemetryEvent::RemoveDenylistedSubshellCommand
            | TelemetryEvent::AddAddedSubshellCommand
            | TelemetryEvent::RemoveAddedSubshellCommand
            | TelemetryEvent::ReceivedSubshellRcFileDcs
            | TelemetryEvent::AddDenylistedSshTmuxWrapperHost
            | TelemetryEvent::RemoveDenylistedSshTmuxWrapperHost
            | TelemetryEvent::SshTmuxWarpifyBlockAccepted
            | TelemetryEvent::SshTmuxWarpifyBlockDismissed
            | TelemetryEvent::SshInstallTmuxBlockDisplayed
            | TelemetryEvent::SshInstallTmuxBlockAccepted
            | TelemetryEvent::SshInstallTmuxBlockDismissed
            | TelemetryEvent::ShowAliasExpansionBanner
            | TelemetryEvent::EnableAliasExpansionFromBanner
            | TelemetryEvent::DismissAliasExpansionBanner
            | TelemetryEvent::ShowVimKeybindingsBanner
            | TelemetryEvent::EnableVimKeybindingsFromBanner
            | TelemetryEvent::DismissVimKeybindingsBanner
            | TelemetryEvent::InitiateReauth
            | TelemetryEvent::NeedsReauth
            | TelemetryEvent::AnonymousUserExpirationLockout
            | TelemetryEvent::AnonymousUserLinkedFromBrowser
            | TelemetryEvent::AnonymousUserHitObjectLimit
            | TelemetryEvent::CustomSecretRegexAdded
            | TelemetryEvent::CopySecret
            | TelemetryEvent::AutoGenerateMetadataSuccess
            | TelemetryEvent::CommandFileRun
            | TelemetryEvent::LogOut
            | TelemetryEvent::UpdateBlockFilterQuery
            | TelemetryEvent::BlockFilterToolbeltButtonClicked
            | TelemetryEvent::PaneDragInitiated
            | TelemetryEvent::AgentModePotentialAutoDetectionFalsePositive(
                AgentModeAutoDetectionFalsePositivePayload::ExternalUsers,
            )
            | TelemetryEvent::SettingsImportResetButtonClicked
            | TelemetryEvent::ITermMultipleHotkeys
            | TelemetryEvent::DriveSharingOnboardingBlockShown
            | TelemetryEvent::SSHControlMasterError
            | TelemetryEvent::SettingsImportInitiated
            | TelemetryEvent::GrepToolSucceeded
            | TelemetryEvent::FileGlobToolSucceeded
            | TelemetryEvent::UserMenuUpgradeClicked
            | TelemetryEvent::AIExecutionProfileCreated
            | TelemetryEvent::AIExecutionProfileDeleted
            | TelemetryEvent::FileTreeItemCreated
            | TelemetryEvent::ConversationListItemDeleted
            | TelemetryEvent::ConversationListViewOpened
            | TelemetryEvent::GlobalSearchOpened
            | TelemetryEvent::GlobalSearchQueryStarted
            | TelemetryEvent::GetStartedSkipToTerminal => None,
            TelemetryEvent::RemoteServerBinaryCheck {
                found,
                error,
                remote_os,
                remote_arch,
            } => Some(json!({
                "found": found,
                "error": error,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerInstallation {
                error,
                remote_os,
                remote_arch,
            } => Some(json!({
                "error": error,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerInitialization {
                phase,
                error,
                remote_os,
                remote_arch,
            } => Some(json!({
                "phase": phase,
                "error": error,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerDisconnection {
                remote_os,
                remote_arch,
            } => Some(json!({
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerClientRequestError {
                operation,
                error_type,
                remote_os,
                remote_arch,
            } => Some(json!({
                "operation": operation,
                "error_type": error_type,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerMessageDecodingError {
                remote_os,
                remote_arch,
            } => Some(json!({
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerSetupDuration {
                duration_ms,
                installed_binary,
                remote_os,
                remote_arch,
                remote_libc,
            } => Some(json!({
                "duration_ms": duration_ms,
                "installed_binary": installed_binary,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
                "remote_libc": remote_libc,
            })),
            TelemetryEvent::RemoteServerHostUnsupported {
                remote_os,
                remote_arch,
                detected_libc,
                required_glibc,
            } => Some(json!({
                "remote_os": remote_os,
                "remote_arch": remote_arch,
                "detected_libc": detected_libc,
                "required_glibc": required_glibc,
            })),
            TelemetryEvent::ConversationListItemOpened { is_ambient_agent } => Some(json!({
                "is_ambient_agent": is_ambient_agent,
            })),
            TelemetryEvent::ConversationListLinkCopied { is_ambient_agent } => Some(json!({
                "is_ambient_agent": is_ambient_agent,
            })),
            TelemetryEvent::AIExecutionProfileSettingUpdated {
                setting_type,
                setting_value,
            } => Some(json!({
                "setting_type": setting_type,
                "setting_value": setting_value,
            })),
            TelemetryEvent::AIExecutionProfileAddedToAllowlist { list_type, value } => {
                Some(json!({
                    "list_type": list_type,
                    "value": value,
                }))
            }
            TelemetryEvent::AIExecutionProfileAddedToDenylist { list_type, value } => Some(json!({
                "list_type": list_type,
                "value": value,
            })),
            TelemetryEvent::AIExecutionProfileRemovedFromAllowlist { list_type, value } => {
                Some(json!({
                    "list_type": list_type,
                    "value": value,
                }))
            }
            TelemetryEvent::AIExecutionProfileRemovedFromDenylist { list_type, value } => {
                Some(json!({
                    "list_type": list_type,
                    "value": value,
                }))
            }
            TelemetryEvent::AIExecutionProfileModelSelected {
                model_type,
                model_value,
            } => Some(json!({
                "model_type": model_type,
                "model_value": model_value,
            })),
            TelemetryEvent::AIExecutionProfileContextWindowSelected { tokens } => Some(json!({
                "tokens": tokens,
            })),
            TelemetryEvent::AIInputNotSent {
                entrypoint,
                inputs,
                active_server_conversation_id,
                active_client_conversation_id,
            } => Some(json!({
                "entrypoint": entrypoint,
                "inputs": inputs,
                "active_server_conversation_id": active_server_conversation_id,
                "active_client_conversation_id": active_client_conversation_id,
            })),
            TelemetryEvent::OpenSlashMenu {
                source,
                is_inline_ui_enabled,
                is_in_agent_view,
            } => Some(json!({
                "source": source,
                "is_inline_ui_enabled": is_inline_ui_enabled,
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::SlashCommandAccepted {
                command_details,
                is_in_agent_view,
            } => Some(json!({
                "command_details": command_details,
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::AgentModeSetupBannerAccepted => None,
            TelemetryEvent::AgentModeSetupBannerDismissed => None,
            TelemetryEvent::AgentModeSetupProjectScopedRulesAction { action } => Some(json!({
                "action": action,
            })),
            #[cfg(windows)]
            TelemetryEvent::WSLRegistryError
            | TelemetryEvent::AutoupdateUnableToCloseApplications
            | TelemetryEvent::AutoupdateFileInUse
            | TelemetryEvent::AutoupdateMutexTimeout
            | TelemetryEvent::AutoupdateForcekillFailed => None,
            TelemetryEvent::InputBufferSubmitted {
                input_type,
                is_locked,
                was_lock_set_with_empty_buffer,
            } => Some(json!({
                "input_type": input_type,
                "is_locked": is_locked,
                "was_lock_set_with_empty_buffer": was_lock_set_with_empty_buffer,
            })),
            TelemetryEvent::CreateProjectPromptSubmitted {
                is_custom_prompt,
                suggested_prompt,
                is_ftux,
            } => Some(json!({
                "is_custom_prompt": is_custom_prompt,
                "suggested_prompt": suggested_prompt,
                "is_ftux": is_ftux,
            })),
            TelemetryEvent::CreateProjectPromptSubmittedContent { custom_prompt } => Some(json!({
                "custom_prompt": custom_prompt
            })),
            TelemetryEvent::CloneRepoPromptSubmitted { is_ftux } => Some(json!({
                "is_ftux": is_ftux,
            })),
            TelemetryEvent::RecentMenuItemSelected { kind } => Some(json!({
                "kind": kind,
            })),
            TelemetryEvent::OpenRepoFolderSubmitted { is_ftux } => Some(json!({
                "is_ftux": is_ftux,
            })),
            TelemetryEvent::OutOfCreditsBannerClosed {
                action,
                selected_credits,
                auto_reload_checkbox_enabled,
                banner_toggle_flag_enabled,
                post_purchase_modal_flag_enabled,
            } => Some(json!({
                "action": action,
                "selected_credits": selected_credits,
                "auto_reload_checkbox_enabled": auto_reload_checkbox_enabled,
                "banner_toggle_flag_enabled": banner_toggle_flag_enabled,
                "post_purchase_modal_flag_enabled": post_purchase_modal_flag_enabled,
            })),
            TelemetryEvent::AutoReloadModalClosed {
                action,
                selected_credits,
                banner_toggle_flag_enabled,
                post_purchase_modal_flag_enabled,
            } => Some(json!({
                "action": action,
                "selected_credits": selected_credits,
                "banner_toggle_flag_enabled": banner_toggle_flag_enabled,
                "post_purchase_modal_flag_enabled": post_purchase_modal_flag_enabled,
            })),
            TelemetryEvent::AutoReloadToggledFromBillingSettings {
                enabled,
                banner_toggle_flag_enabled,
                post_purchase_modal_flag_enabled,
            } => Some(json!({
                "enabled": enabled,
                "banner_toggle_flag_enabled": banner_toggle_flag_enabled,
                "post_purchase_modal_flag_enabled": post_purchase_modal_flag_enabled,
            })),
            TelemetryEvent::WarpDriveOpened {
                source,
                is_code_mode_v2,
            } => Some(json!({
                "source": source,
                "is_code_mode_v2": is_code_mode_v2,
            })),
            TelemetryEvent::AgentTipShown { tip } => Some(json!({
                "tip": tip,
            })),
            TelemetryEvent::AgentTipClicked { tip, click_target } => Some(json!({
                "tip": tip,
                "click_target": click_target,
            })),
            TelemetryEvent::ToggleShowAgentTips { is_enabled } => Some(json!({
                "is_enabled": is_enabled,
            })),
            TelemetryEvent::CLISubagentControlStateChanged {
                conversation_id,
                block_id,
                control_state,
            } => Some(json!({
                "conversation_id": conversation_id,
                "block_id": block_id,
                "control_state": control_state,
            })),
            TelemetryEvent::CLISubagentResponsesToggled {
                conversation_id,
                block_id,
                is_hidden,
            } => Some(json!({
                "conversation_id": conversation_id,
                "block_id": block_id,
                "is_hidden": is_hidden,
            })),
            TelemetryEvent::CLISubagentInputDismissed {
                conversation_id,
                block_id,
            } => Some(json!({
                "conversation_id": conversation_id,
                "block_id": block_id,
            })),
            TelemetryEvent::CLISubagentActionExecuted {
                conversation_id,
                block_id,
                is_autoexecuted,
            } => Some(json!({
                "conversation_id": conversation_id,
                "block_id": block_id,
                "is_autoexecuted": is_autoexecuted,
            })),
            TelemetryEvent::CLISubagentActionRejected {
                conversation_id,
                block_id,
                user_took_over,
            } => Some(json!({
                "conversation_id": conversation_id,
                "block_id": block_id,
                "user_took_over": user_took_over,
            })),
            TelemetryEvent::AgentManagementViewToggled { is_open } => Some(json!({
                "is_open": is_open,
            })),
            TelemetryEvent::AgentManagementViewOpenedSession => None,
            TelemetryEvent::AgentManagementViewCopiedSessionLink => None,
            TelemetryEvent::DetectedIsolationPlatform { platform } => Some(json!({
                "platform": platform,
            })),
            TelemetryEvent::AgentExitedShellProcess {
                command,
                server_output_id,
            } => Some(json!({
                "command": command,
                "server_output_id": server_output_id,
            })),
            TelemetryEvent::CLIAgentToolbarVoiceInputUsed { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::CLIAgentToolbarImageAttached { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::CLIAgentToolbarShown { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::CLIAgentRichInputOpened {
                cli_agent,
                entrypoint,
            } => Some(json!({
                "agent_name": cli_agent,
                "entrypoint": entrypoint,
            })),
            TelemetryEvent::CLIAgentRichInputClosed { cli_agent, reason } => Some(json!({
                "agent_name": cli_agent,
                "reason": reason,
            })),
            TelemetryEvent::CLIAgentRichInputSubmitted {
                cli_agent,
                prompt_length,
            } => Some(json!({
                "agent_name": cli_agent,
                "prompt_length": prompt_length,
            })),
            TelemetryEvent::CLIAgentPluginChipClicked { cli_agent, action } => Some(json!({
                "agent_name": cli_agent,
                "action": action,
            })),
            TelemetryEvent::CLIAgentPluginChipDismissed {
                cli_agent,
                chip_kind,
            } => Some(json!({
                "agent_name": cli_agent,
                "chip_kind": chip_kind,
            })),
            TelemetryEvent::CLIAgentPluginOperationSucceeded {
                cli_agent,
                operation,
            } => Some(json!({
                "agent_name": cli_agent,
                "operation": operation,
            })),
            TelemetryEvent::CLIAgentPluginOperationFailed {
                cli_agent,
                operation,
            } => Some(json!({
                "agent_name": cli_agent,
                "operation": operation,
            })),
            TelemetryEvent::CLIAgentPluginDetected { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::AgentNotificationShown { agent_variant } => Some(json!({
                "agent_variant": agent_variant,
            })),
            TelemetryEvent::ToggleCLIAgentToolbarSetting { is_enabled } => Some(json!({
                "is_enabled": is_enabled,
            })),
            TelemetryEvent::ToggleUseAgentToolbarSetting { is_enabled } => Some(json!({
                "is_enabled": is_enabled,
            })),
            TelemetryEvent::AgentViewEntered {
                origin,
                did_auto_trigger_request,
            } => Some(json!({
                "origin": origin,
                "did_auto_trigger_request": did_auto_trigger_request,
            })),
            TelemetryEvent::AgentViewExited { origin, was_empty } => Some(json!({
                "origin": origin,
                "was_empty": was_empty,
            })),
            TelemetryEvent::InlineConversationMenuOpened { is_in_agent_view } => Some(json!({
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::InlineConversationMenuItemSelected { is_in_agent_view } => {
                Some(json!({
                    "is_in_agent_view": is_in_agent_view,
                }))
            }
            TelemetryEvent::AgentShortcutsViewToggled { is_visible } => Some(json!({
                "is_visible": is_visible,
            })),
            TelemetryEvent::CodexModalOpened => None,
            TelemetryEvent::CodexModalUseCodexClicked => None,
            TelemetryEvent::LinearIssueLinkOpened => None,
            TelemetryEvent::ComputerUseApproved {
                conversation_id,
                is_autoexecuted,
                ambient_agent_task_id,
            } => Some(json!({
                "conversation_id": conversation_id,
                "is_autoexecuted": is_autoexecuted,
                "ambient_agent_task_id": ambient_agent_task_id.map(|id| id.to_string()),
            })),
            TelemetryEvent::ComputerUseCancelled {
                conversation_id,
                ambient_agent_task_id,
            } => Some(json!({
                "conversation_id": conversation_id,
                "ambient_agent_task_id": ambient_agent_task_id.map(|id| id.to_string()),
            })),
            TelemetryEvent::FreeTierLimitHitInterstitialDisplayed => None,
            TelemetryEvent::FreeTierLimitHitInterstitialUpgradeButtonClicked => None,
            TelemetryEvent::FreeTierLimitHitInterstitialClosed => None,
            TelemetryEvent::LoginButtonClicked { source }
            | TelemetryEvent::LoginLaterButtonClicked { source }
            | TelemetryEvent::LoginLaterConfirmationButtonClicked { source }
            | TelemetryEvent::OpenAuthPrivacySettings { source } => Some(json!({
                "source": source,
            })),
        }
    }

    /// Returns whether the event contains user generated content.
    pub fn contains_ugc(&self) -> bool {
        match self {
            TelemetryEvent::GrepToolFailed { .. } => true,
            TelemetryEvent::BootstrappingSlowContents { .. } => true,
            TelemetryEvent::AIInputNotSent { .. } => true,
            TelemetryEvent::AgentExitedShellProcess { .. } => true,
            TelemetryEvent::CreateProjectPromptSubmitted { .. } => false,
            TelemetryEvent::CreateProjectPromptSubmittedContent { .. } => true,
            TelemetryEvent::AgentModePrediction {
                actual_next_command_run,
                history_based_autosuggestion_state,
                generate_ai_input_suggestions_request,
                generate_ai_input_suggestions_response,
                ..
            } => {
                // These fields can contain UGC, so if any are set, assume this event contains UGC.
                actual_next_command_run.is_some()
                    || history_based_autosuggestion_state.is_some()
                    || generate_ai_input_suggestions_request.is_some()
                    || generate_ai_input_suggestions_response.is_some()
            }
            TelemetryEvent::AgentModeChangedInputType { input, .. } => input.is_some(),
            TelemetryEvent::UnitTestSuggestionAccepted { query, .. } => query.is_some(),
            TelemetryEvent::AgentModePotentialAutoDetectionFalsePositive(payload) => {
                // For internal dogfood users, the payload contains UGC.
                matches!(
                    payload,
                    AgentModeAutoDetectionFalsePositivePayload::InternalDogfoodUsers { .. }
                )
            }
            TelemetryEvent::ShowedSuggestedAgentModeWorkflowModal { .. }
            | TelemetryEvent::ShowedSuggestedAgentModeWorkflowChip { .. }
            | TelemetryEvent::AISuggestedAgentModeWorkflowAdded { .. }
            | TelemetryEvent::AutosuggestionInserted { .. }
            | TelemetryEvent::BlockCompleted { .. }
            | TelemetryEvent::BlockCompletedOnDogfoodOnly { .. }
            | TelemetryEvent::BackgroundBlockStarted
            | TelemetryEvent::BaselineCommandLatency(_)
            | TelemetryEvent::SessionCreation
            | TelemetryEvent::Login
            | TelemetryEvent::AgentModeContinueConversationButtonClicked { .. }
            | TelemetryEvent::AgentModeRewindDialogOpened { .. }
            | TelemetryEvent::AgentModeRewindExecuted { .. }
            | TelemetryEvent::OpenSuggestionsMenu(_)
            | TelemetryEvent::ConfirmSuggestion { .. }
            | TelemetryEvent::OpenContextMenu { .. }
            | TelemetryEvent::ContextMenuCopy(_, _)
            | TelemetryEvent::ContextMenuFindWithinBlocks(_)
            | TelemetryEvent::ContextMenuCopyPrompt { .. }
            | TelemetryEvent::ContextMenuToggleGitPromptDirtyIndicator { .. }
            | TelemetryEvent::ContextMenuInsertSelectedText
            | TelemetryEvent::ContextMenuCopySelectedText
            | TelemetryEvent::OpenPromptEditor { .. }
            | TelemetryEvent::PromptEdited { .. }
            | TelemetryEvent::ReinputCommands(_)
            | TelemetryEvent::JumpToPreviousCommand
            | TelemetryEvent::BlockSelection(_)
            | TelemetryEvent::BootstrappingSlow(_)
            | TelemetryEvent::SessionAbandonedBeforeBootstrap { .. }
            | TelemetryEvent::BootstrappingSucceeded(_)
            | TelemetryEvent::TabSingleResultAutocompletion
            | TelemetryEvent::EditorUnhandledModifierKey(_)
            | TelemetryEvent::CopyInviteLink
            | TelemetryEvent::OpenThemeChooser
            | TelemetryEvent::ThemeSelection { .. }
            | TelemetryEvent::AppIconSelection { .. }
            | TelemetryEvent::CursorDisplayType { .. }
            | TelemetryEvent::OpenThemeCreatorModal
            | TelemetryEvent::CreateCustomTheme
            | TelemetryEvent::DeleteCustomTheme
            | TelemetryEvent::SplitPane
            | TelemetryEvent::UnableToAutoUpdateToNewVersion
            | TelemetryEvent::AutoupdateRelaunchAttempt { .. }
            | TelemetryEvent::SkipOnboardingSurvey
            | TelemetryEvent::ToggleRestoreSession(_)
            | TelemetryEvent::DatabaseStartUpError(_)
            | TelemetryEvent::DatabaseReadError(_)
            | TelemetryEvent::DatabaseWriteError(_)
            | TelemetryEvent::AppStartup(_)
            | TelemetryEvent::LoggedOutStartup
            | TelemetryEvent::SSHBootstrapAttempt(_)
            | TelemetryEvent::SSHControlMasterError
            | TelemetryEvent::KeybindingChanged { .. }
            | TelemetryEvent::KeybindingResetToDefault { .. }
            | TelemetryEvent::KeybindingRemoved { .. }
            | TelemetryEvent::FeaturesPageAction { .. }
            | TelemetryEvent::WorkflowExecuted(_)
            | TelemetryEvent::WorkflowSelected(_)
            | TelemetryEvent::OpenWorkflowSearch
            | TelemetryEvent::OpenQuakeModeWindow
            | TelemetryEvent::OpenWelcomeTips
            | TelemetryEvent::CompleteWelcomeTipFeature { .. }
            | TelemetryEvent::DismissWelcomeTips
            | TelemetryEvent::ShowNotificationsDiscoveryBanner
            | TelemetryEvent::NotificationsDiscoveryBannerAction(_)
            | TelemetryEvent::ShowNotificationsErrorBanner
            | TelemetryEvent::NotificationsErrorBannerAction(_)
            | TelemetryEvent::NotificationPermissionsRequested { .. }
            | TelemetryEvent::NotificationsRequestPermissionsOutcome { .. }
            | TelemetryEvent::NotificationSent { .. }
            | TelemetryEvent::NotificationFailedToSend { .. }
            | TelemetryEvent::NotificationClicked
            | TelemetryEvent::ToggleFindOption { .. }
            | TelemetryEvent::SignUpButtonClicked
            | TelemetryEvent::LoginButtonClicked { .. }
            | TelemetryEvent::LoginLaterButtonClicked { .. }
            | TelemetryEvent::LoginLaterConfirmationButtonClicked { .. }
            | TelemetryEvent::OpenNewSessionFromFilePath
            | TelemetryEvent::OpenTeamFromURI
            | TelemetryEvent::SelectNavigationPaletteItem
            | TelemetryEvent::SelectCommandPaletteOption(_)
            | TelemetryEvent::PaletteSearchOpened { .. }
            | TelemetryEvent::PaletteSearchResultAccepted { .. }
            | TelemetryEvent::PaletteSearchExited { .. }
            | TelemetryEvent::AuthCommonQuestionClicked { .. }
            | TelemetryEvent::AuthToggleFAQ { .. }
            | TelemetryEvent::OpenAuthPrivacySettings { .. }
            | TelemetryEvent::TabRenamed(_)
            | TelemetryEvent::MoveActiveTab { .. }
            | TelemetryEvent::MoveTab { .. }
            | TelemetryEvent::DragAndDropTab
            | TelemetryEvent::TabOperations { .. }
            | TelemetryEvent::EditedInputBeforePrecmd
            | TelemetryEvent::TriedToExecuteBeforePrecmd
            | TelemetryEvent::ThinStrokesSettingChanged { .. }
            | TelemetryEvent::BookmarkBlockToggled { .. }
            | TelemetryEvent::JumpToBookmark
            | TelemetryEvent::JumpToBottomofBlockButtonClicked
            | TelemetryEvent::ToggleJumpToBottomofBlockButton { .. }
            | TelemetryEvent::ToggleShowBlockDividers { .. }
            | TelemetryEvent::OpenLink { .. }
            | TelemetryEvent::OpenChangelogLink { .. }
            | TelemetryEvent::ShowInFileExplorer
            | TelemetryEvent::CommandXRayTriggered { .. }
            | TelemetryEvent::OpenLaunchConfigSaveModal
            | TelemetryEvent::SaveLaunchConfig { .. }
            | TelemetryEvent::OpenLaunchConfigFile
            | TelemetryEvent::OpenLaunchConfig { .. }
            | TelemetryEvent::TeamCreated
            | TelemetryEvent::TeamJoined
            | TelemetryEvent::TeamLeft
            | TelemetryEvent::TeamLinkCopied
            | TelemetryEvent::RemovedUserFromTeam
            | TelemetryEvent::DeletedWorkflow
            | TelemetryEvent::DeletedNotebook
            | TelemetryEvent::ToggleApprovalsModal
            | TelemetryEvent::ChangedInviteViewOption(_)
            | TelemetryEvent::SendEmailInvites
            | TelemetryEvent::CommandCorrection { .. }
            | TelemetryEvent::SetLineHeight { .. }
            | TelemetryEvent::ResourceCenterOpened
            | TelemetryEvent::ResourceCenterTipsCompleted
            | TelemetryEvent::ResourceCenterTipsSkipped
            | TelemetryEvent::KeybindingsPageOpened
            | TelemetryEvent::GlobalSearchOpened
            | TelemetryEvent::GlobalSearchQueryStarted
            | TelemetryEvent::CommandSearchOpened { .. }
            | TelemetryEvent::CommandSearchExited { .. }
            | TelemetryEvent::CommandSearchResultAccepted { .. }
            | TelemetryEvent::CommandSearchFilterChanged { .. }
            | TelemetryEvent::CommandSearchAsyncQueryCompleted { .. }
            | TelemetryEvent::AICommandSearchOpened { .. }
            | TelemetryEvent::OpenNotebook(_)
            | TelemetryEvent::EditNotebook { .. }
            | TelemetryEvent::NotebookAction(_)
            | TelemetryEvent::OpenedAltScreenFind
            | TelemetryEvent::UserInitiatedClose { .. }
            | TelemetryEvent::QuitModalShown { .. }
            | TelemetryEvent::QuitModalCancel { .. }
            | TelemetryEvent::QuitModalDisabled
            | TelemetryEvent::UserInitiatedLogOut
            | TelemetryEvent::LogOutModalShown
            | TelemetryEvent::LogOutModalCancel { .. }
            | TelemetryEvent::SetOpacity { .. }
            | TelemetryEvent::SetBlurRadius { .. }
            | TelemetryEvent::ToggleDimInactivePanes { .. }
            | TelemetryEvent::InputModeChanged { .. }
            | TelemetryEvent::PtySpawned { .. }
            | TelemetryEvent::InitialWorkingDirectoryConfigurationChanged { .. }
            | TelemetryEvent::OpenedWarpAI { .. }
            | TelemetryEvent::WarpAIRequestIssued { .. }
            | TelemetryEvent::WarpAIAction { .. }
            | TelemetryEvent::UsedWarpAIPreparedPrompt { .. }
            | TelemetryEvent::ToggleFocusPaneOnHover { .. }
            | TelemetryEvent::WarpAICharacterLimitExceeded
            | TelemetryEvent::OpenInputContextMenu
            | TelemetryEvent::InputCutSelectedText
            | TelemetryEvent::InputCopySelectedText
            | TelemetryEvent::InputSelectAll
            | TelemetryEvent::InputPaste
            | TelemetryEvent::InputCommandSearch
            | TelemetryEvent::InputAICommandSearch
            | TelemetryEvent::InputAskWarpAI
            | TelemetryEvent::SaveAsWorkflowModal { .. }
            | TelemetryEvent::ExperimentTriggered { .. }
            | TelemetryEvent::ToggleSyncAllPanesInAllTabs { .. }
            | TelemetryEvent::ToggleSyncAllPanesInTab { .. }
            | TelemetryEvent::ToggleSameLinePrompt { .. }
            | TelemetryEvent::ToggleNewWindowsAtCustomSize { .. }
            | TelemetryEvent::SetNewWindowsAtCustomSize
            | TelemetryEvent::DisableInputSync
            | TelemetryEvent::ToggleTabIndicators { .. }
            | TelemetryEvent::TogglePreserveActiveTabColor { .. }
            | TelemetryEvent::ShowSubshellBanner
            | TelemetryEvent::DeclineSubshellBootstrap { .. }
            | TelemetryEvent::TriggerSubshellBootstrap { .. }
            | TelemetryEvent::AddDenylistedSubshellCommand
            | TelemetryEvent::RemoveDenylistedSubshellCommand
            | TelemetryEvent::AddAddedSubshellCommand
            | TelemetryEvent::RemoveAddedSubshellCommand
            | TelemetryEvent::ReceivedSubshellRcFileDcs
            | TelemetryEvent::AddDenylistedSshTmuxWrapperHost
            | TelemetryEvent::RemoveDenylistedSshTmuxWrapperHost
            | TelemetryEvent::ToggleSshTmuxWrapper { .. }
            | TelemetryEvent::SshInteractiveSessionDetected(_)
            | TelemetryEvent::SshTmuxWarpifyBannerDisplayed
            | TelemetryEvent::SshTmuxWarpifyBlockAccepted
            | TelemetryEvent::SshTmuxWarpifyBlockDismissed
            | TelemetryEvent::WarpifyFooterShown { .. }
            | TelemetryEvent::AgentToolbarDismissed
            | TelemetryEvent::WarpifyFooterAcceptedWarpify { .. }
            | TelemetryEvent::SshTmuxWarpificationSuccess { .. }
            | TelemetryEvent::SshTmuxWarpificationErrorBlock { .. }
            | TelemetryEvent::SshInstallTmuxBlockDisplayed
            | TelemetryEvent::SshInstallTmuxBlockAccepted
            | TelemetryEvent::SshInstallTmuxBlockDismissed
            | TelemetryEvent::ShowAliasExpansionBanner
            | TelemetryEvent::EnableAliasExpansionFromBanner
            | TelemetryEvent::DismissAliasExpansionBanner
            | TelemetryEvent::ShowVimKeybindingsBanner
            | TelemetryEvent::EnableVimKeybindingsFromBanner
            | TelemetryEvent::DismissVimKeybindingsBanner
            | TelemetryEvent::InitiateReauth
            | TelemetryEvent::InitiateAnonymousUserSignup { .. }
            | TelemetryEvent::AnonymousUserExpirationLockout
            | TelemetryEvent::AnonymousUserLinkedFromBrowser
            | TelemetryEvent::AnonymousUserAttemptLoginGatedFeature { .. }
            | TelemetryEvent::AnonymousUserHitObjectLimit
            | TelemetryEvent::NeedsReauth
            | TelemetryEvent::WarpDriveOpened { .. }
            | TelemetryEvent::ToggleWarpAI { .. }
            | TelemetryEvent::ToggleSecretRedaction { .. }
            | TelemetryEvent::CustomSecretRegexAdded
            | TelemetryEvent::ToggleObfuscateSecret { .. }
            | TelemetryEvent::CopySecret
            | TelemetryEvent::AutoGenerateMetadataSuccess
            | TelemetryEvent::AutoGenerateMetadataError { .. }
            | TelemetryEvent::UpdateSortingChoice { .. }
            | TelemetryEvent::UndoClose { .. }
            | TelemetryEvent::PtyThroughput { .. }
            | TelemetryEvent::DuplicateObject(_)
            | TelemetryEvent::ExportObject(_)
            | TelemetryEvent::DriveSharingOnboardingBlockShown
            | TelemetryEvent::CommandFileRun
            | TelemetryEvent::PageUpDownInEditorPressed { .. }
            | TelemetryEvent::WebObjectOpenedOnDesktop { .. }
            | TelemetryEvent::UnsupportedShell { .. }
            | TelemetryEvent::LogOut
            | TelemetryEvent::InviteTeammates { .. }
            | TelemetryEvent::CopyObjectToClipboard(_)
            | TelemetryEvent::OpenAndWarpifyDockerSubshell { .. }
            | TelemetryEvent::UpdateBlockFilterQuery
            | TelemetryEvent::UpdateBlockFilterQueryContextLines { .. }
            | TelemetryEvent::ToggleBlockFilterQuery { .. }
            | TelemetryEvent::ToggleBlockFilterCaseSensitivity { .. }
            | TelemetryEvent::ToggleBlockFilterRegex { .. }
            | TelemetryEvent::ToggleBlockFilterInvert { .. }
            | TelemetryEvent::BlockFilterToolbeltButtonClicked
            | TelemetryEvent::ToggleSnackbarInActivePane { .. }
            | TelemetryEvent::PaneDragInitiated
            | TelemetryEvent::PaneDropped { .. }
            | TelemetryEvent::ObjectLinkCopied { .. }
            | TelemetryEvent::FileTreeToggled { .. }
            | TelemetryEvent::AgentModeUserAttemptedQueryAtRequestLimit { .. }
            | TelemetryEvent::AgentModeClickedEntrypoint { .. }
            | TelemetryEvent::AgentModeAttachedBlockContext { .. }
            | TelemetryEvent::AgentModeToggleAutoDetectionSetting { .. }
            | TelemetryEvent::PromptSuggestionShown { .. }
            | TelemetryEvent::SuggestedCodeDiffBannerShown { .. }
            | TelemetryEvent::SuggestedCodeDiffFailed { .. }
            | TelemetryEvent::PromptSuggestionAccepted { .. }
            | TelemetryEvent::ZeroStatePromptSuggestionUsed { .. }
            | TelemetryEvent::UnitTestSuggestionShown { .. }
            | TelemetryEvent::UnitTestSuggestionCancelled { .. }
            | TelemetryEvent::AgentModeCodeSuggestionEditedByUser { .. }
            | TelemetryEvent::AgentModeCodeFilesNavigated { .. }
            | TelemetryEvent::AgentModeCodeDiffHunksNavigated { .. }
            | TelemetryEvent::ToggleIntelligentAutosuggestionsSetting { .. }
            | TelemetryEvent::ToggleGlobalAI { .. }
            | TelemetryEvent::ToggleActiveAI { .. }
            | TelemetryEvent::TogglePromptSuggestionsSetting { .. }
            | TelemetryEvent::ToggleCodeSuggestionsSetting { .. }
            | TelemetryEvent::ToggleVoiceInputSetting { .. }
            | TelemetryEvent::TierLimitHit(_)
            | TelemetryEvent::ResourceUsageStats { .. }
            | TelemetryEvent::MemoryUsageStats { .. }
            | TelemetryEvent::MemoryUsageHigh { .. }
            | TelemetryEvent::EnvVarCollectionInvoked(_)
            | TelemetryEvent::EnvVarWorkflowParameterization(_)
            | TelemetryEvent::CompletedSettingsImport { .. }
            | TelemetryEvent::SettingsImportConfigFocused(_)
            | TelemetryEvent::SettingsImportResetButtonClicked
            | TelemetryEvent::SettingsImportConfigParsed { .. }
            | TelemetryEvent::ITermMultipleHotkeys
            | TelemetryEvent::ToggleWorkspaceDecorationVisibility { .. }
            | TelemetryEvent::UpdateAltScreenPaddingMode { .. }
            | TelemetryEvent::AddTabWithShell { .. }
            | TelemetryEvent::AgentModeSurfacedCitations { .. }
            | TelemetryEvent::AgentModeOpenedCitation { .. }
            | TelemetryEvent::ToggleLigatureRendering { .. }
            | TelemetryEvent::WorkflowAliasAdded { .. }
            | TelemetryEvent::WorkflowAliasRemoved { .. }
            | TelemetryEvent::WorkflowAliasEnvVarsAttached { .. }
            | TelemetryEvent::WorkflowAliasArgumentEdited { .. }
            | TelemetryEvent::ToggledAgentModeAutoexecuteReadonlyCommandsSetting { .. }
            | TelemetryEvent::ChangedAgentModeCodingPermissions { .. }
            | TelemetryEvent::RepoOutlineConstructionSuccess { .. }
            | TelemetryEvent::RepoOutlineConstructionFailed { .. }
            | TelemetryEvent::AutoexecutedAgentModeRequestedCommand { .. }
            | TelemetryEvent::AgenticOnboardingBlockSelected { .. }
            | TelemetryEvent::KnowledgePaneOpened { .. }
            | TelemetryEvent::MCPServerCollectionPaneOpened { .. }
            | TelemetryEvent::MCPServerAdded { .. }
            | TelemetryEvent::MCPTemplateCreated { .. }
            | TelemetryEvent::MCPTemplateInstalled { .. }
            | TelemetryEvent::MCPTemplateShared
            | TelemetryEvent::MCPServerSpawned { .. }
            | TelemetryEvent::MCPToolCallAccepted { .. }
            | TelemetryEvent::ExecutedWarpDrivePrompt { .. }
            | TelemetryEvent::ToggleSshWarpification { .. }
            | TelemetryEvent::SetSshExtensionInstallMode { .. }
            | TelemetryEvent::SshRemoteServerChoiceDoNotAskAgainToggled { .. }
            | TelemetryEvent::SettingsImportInitiated
            | TelemetryEvent::AgentModeCreatedAIBlock { .. }
            | TelemetryEvent::StaticPromptSuggestionsBannerShown { .. }
            | TelemetryEvent::StaticPromptSuggestionAccepted { .. }
            | TelemetryEvent::AISuggestedRuleAdded { .. }
            | TelemetryEvent::AISuggestedRuleEdited { .. }
            | TelemetryEvent::AISuggestedRuleContentChanged { .. }
            | TelemetryEvent::AttachedImagesToAgentModeQuery { .. }
            | TelemetryEvent::ImageReceived { .. }
            | TelemetryEvent::FileExceededContextLimit { .. }
            | TelemetryEvent::AgentModeError { .. }
            | TelemetryEvent::AgentModeRequestRetrySucceeded { .. }
            | TelemetryEvent::ToggleNaturalLanguageAutosuggestionsSetting { .. }
            | TelemetryEvent::ToggleGitOperationsAutogenSetting { .. }
            | TelemetryEvent::GrepToolSucceeded
            | TelemetryEvent::FileGlobToolSucceeded
            | TelemetryEvent::FileGlobToolFailed { .. }
            | TelemetryEvent::ShellTerminatedPrematurely { .. }
            | TelemetryEvent::InputUXModeChanged { .. }
            | TelemetryEvent::ContextChipInteracted { .. }
            | TelemetryEvent::VoiceInputUsed { .. }
            | TelemetryEvent::AtMenuInteracted { .. }
            | TelemetryEvent::UserMenuUpgradeClicked
            | TelemetryEvent::TabCloseButtonPositionUpdated { .. }
            | TelemetryEvent::ExpandedCodeSuggestions { .. }
            | TelemetryEvent::AIExecutionProfileCreated
            | TelemetryEvent::AIExecutionProfileDeleted
            | TelemetryEvent::AIExecutionProfileSettingUpdated { .. }
            | TelemetryEvent::AIExecutionProfileAddedToAllowlist { .. }
            | TelemetryEvent::AIExecutionProfileAddedToDenylist { .. }
            | TelemetryEvent::AIExecutionProfileRemovedFromAllowlist { .. }
            | TelemetryEvent::AIExecutionProfileRemovedFromDenylist { .. }
            | TelemetryEvent::AIExecutionProfileModelSelected { .. }
            | TelemetryEvent::AIExecutionProfileContextWindowSelected { .. }
            | TelemetryEvent::OpenSlashMenu { .. }
            | TelemetryEvent::SlashCommandAccepted { .. }
            | TelemetryEvent::AgentModeSetupBannerAccepted
            | TelemetryEvent::AgentModeSetupBannerDismissed
            | TelemetryEvent::AgentModeSetupProjectScopedRulesAction { .. }
            | TelemetryEvent::CloneRepoPromptSubmitted { .. }
            | TelemetryEvent::GetStartedSkipToTerminal
            | TelemetryEvent::FileTreeItemAttachedAsContext { .. }
            | TelemetryEvent::CodeSelectionAddedAsContext { .. }
            | TelemetryEvent::FileTreeItemCreated
            | TelemetryEvent::ConversationListViewOpened
            | TelemetryEvent::ConversationListItemOpened { .. }
            | TelemetryEvent::ConversationListItemDeleted
            | TelemetryEvent::ConversationListLinkCopied { .. }
            | TelemetryEvent::AgentViewEntered { .. }
            | TelemetryEvent::AgentViewExited { .. }
            | TelemetryEvent::InlineConversationMenuOpened { .. }
            | TelemetryEvent::InlineConversationMenuItemSelected { .. }
            | TelemetryEvent::AgentShortcutsViewToggled { .. }
            | TelemetryEvent::InputBufferSubmitted { .. }
            | TelemetryEvent::RecentMenuItemSelected { .. }
            | TelemetryEvent::OpenRepoFolderSubmitted { .. }
            | TelemetryEvent::OutOfCreditsBannerClosed { .. }
            | TelemetryEvent::AutoReloadModalClosed { .. }
            | TelemetryEvent::AutoReloadToggledFromBillingSettings { .. }
            | TelemetryEvent::CLISubagentControlStateChanged { .. }
            | TelemetryEvent::CLISubagentResponsesToggled { .. }
            | TelemetryEvent::CLISubagentInputDismissed { .. }
            | TelemetryEvent::CLISubagentActionExecuted { .. }
            | TelemetryEvent::CLISubagentActionRejected { .. }
            | TelemetryEvent::AgentManagementViewToggled { .. }
            | TelemetryEvent::AgentManagementViewOpenedSession
            | TelemetryEvent::AgentManagementViewCopiedSessionLink
            | TelemetryEvent::DetectedIsolationPlatform { .. }
            | TelemetryEvent::AgentTipShown { .. }
            | TelemetryEvent::AgentTipClicked { .. }
            | TelemetryEvent::ToggleShowAgentTips { .. }
            | TelemetryEvent::CLIAgentToolbarVoiceInputUsed { .. }
            | TelemetryEvent::CLIAgentToolbarImageAttached { .. }
            | TelemetryEvent::CLIAgentToolbarShown { .. }
            | TelemetryEvent::CLIAgentPluginChipClicked { .. }
            | TelemetryEvent::CLIAgentPluginChipDismissed { .. }
            | TelemetryEvent::CLIAgentPluginOperationSucceeded { .. }
            | TelemetryEvent::CLIAgentPluginOperationFailed { .. }
            | TelemetryEvent::CLIAgentPluginDetected { .. }
            | TelemetryEvent::AgentNotificationShown { .. }
            | TelemetryEvent::CLIAgentRichInputOpened { .. }
            | TelemetryEvent::CLIAgentRichInputClosed { .. }
            | TelemetryEvent::CLIAgentRichInputSubmitted { .. }
            | TelemetryEvent::ToggleCLIAgentToolbarSetting { .. }
            | TelemetryEvent::ToggleUseAgentToolbarSetting { .. }
            | TelemetryEvent::CodexModalOpened
            | TelemetryEvent::CodexModalUseCodexClicked
            | TelemetryEvent::LinearIssueLinkOpened
            | TelemetryEvent::ComputerUseApproved { .. }
            | TelemetryEvent::ComputerUseCancelled { .. }
            | TelemetryEvent::FreeTierLimitHitInterstitialDisplayed
            | TelemetryEvent::FreeTierLimitHitInterstitialUpgradeButtonClicked
            | TelemetryEvent::FreeTierLimitHitInterstitialClosed
            | TelemetryEvent::RemoteServerBinaryCheck { .. }
            | TelemetryEvent::RemoteServerInstallation { .. }
            | TelemetryEvent::RemoteServerInitialization { .. }
            | TelemetryEvent::RemoteServerDisconnection { .. }
            | TelemetryEvent::RemoteServerClientRequestError { .. }
            | TelemetryEvent::RemoteServerMessageDecodingError { .. }
            | TelemetryEvent::RemoteServerSetupDuration { .. }
            | TelemetryEvent::RemoteServerHostUnsupported { .. } => false,
            #[cfg(feature = "local_fs")]
            TelemetryEvent::CodePaneOpened { .. }
            | TelemetryEvent::CodePanelsFileOpened { .. }
            | TelemetryEvent::PreviewPanePromoted => false,
            #[cfg(windows)]
            TelemetryEvent::WSLRegistryError
            | TelemetryEvent::AutoupdateUnableToCloseApplications
            | TelemetryEvent::AutoupdateFileInUse
            | TelemetryEvent::AutoupdateMutexTimeout
            | TelemetryEvent::AutoupdateForcekillFailed => false,
        }
    }

}

