use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use warp_util::path::LineAndColumnArg;

use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent::AIAgentExchangeId;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::document::ai_document_model::{AIDocumentId, AIDocumentVersion};
use crate::auth::LoginGatedFeature;
use crate::drive::items::WarpDriveItemId;
use crate::drive::ObjectTypeAndId;
use crate::palette::PaletteMode;
use crate::prompt::editor_modal::OpenSource as PromptEditorOpenSource;
use crate::search;
use crate::server::ids::SyncId;
use crate::server::telemetry::{AddTabWithShellSource, AgentModeEntrypoint, PaletteSource};
use crate::settings_view::{SettingsAction as SettingsTabAction, SettingsSection};
use crate::tab::{NewSessionMenuItem, SelectedTabColor};
use crate::tab_configs::TabConfig;
use crate::terminal::available_shells::AvailableShell;
use crate::terminal::view::inline_banner::ZeroStatePromptSuggestionType;
use crate::themes::theme::AnsiColorIdentifier;
use crate::themes::theme_chooser::ThemeChooserMode;
use crate::workflows::{WorkflowSelectionSource, WorkflowSource, WorkflowType};
use crate::workspace::PaneViewLocator;

use ui_components::lightbox;
use warpui::accessibility::AccessibilityVerbosity;
use warpui::geometry::rect::RectF;
use warpui::geometry::vector::Vector2F;
use warpui::platform::Cursor;
use warpui::{EntityId, WindowId};

use super::global_actions::{ForkFromExchange, ForkedConversationDestination};
use super::tab_settings::{
    VerticalTabsCompactSubtitle, VerticalTabsDisplayGranularity, VerticalTabsPrimaryInfo,
    VerticalTabsTabItemMode, VerticalTabsViewMode,
};
use super::view::{OnboardingTutorial, WorkspaceBanner};

/// This enum determines how the search query is initialized when opening command search.
#[derive(Clone, Default, Debug)]
pub enum InitContent {
    /// Read the content of the active terminal input, and make that the initial search query.
    #[default]
    FromInputBuffer,
    /// Specify an exact string to initialize the query to.
    Custom(String),
}

/// To initialize command search, we may want to specify a search filter, or the content of the
/// query itself.
#[derive(Clone, Default, Debug)]
pub struct CommandSearchOptions {
    pub filter: Option<search::QueryFilter>,
    pub init_content: InitContent,
}

/// Specifies how to restore a conversation when it's not already open in a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum RestoreConversationLayout {
    /// Restore the conversation into the currently active pane.
    ActivePane,
    /// Restore the conversation in a new split pane.
    SplitPane,
    /// Restore the conversation in a new tab.
    #[default]
    NewTab,
}

#[derive(Debug, Clone, Copy)]
pub enum TabContextMenuAnchor {
    Pointer(Vector2F),
    VerticalTabsKebab,
}

#[derive(Debug, Clone, Copy)]
pub enum VerticalTabsPaneContextMenuTarget {
    ClickedPane(PaneViewLocator),
    ActivePane(PaneViewLocator),
}

impl VerticalTabsPaneContextMenuTarget {
    pub fn locator(self) -> PaneViewLocator {
        match self {
            Self::ClickedPane(locator) | Self::ActivePane(locator) => locator,
        }
    }
}

#[derive(Debug, Clone)]
pub enum WorkspaceAction {
    ActivateTab(usize),
    ActivatePrevTab,
    ActivateNextTab,
    ActivateLastTab,
    CyclePrevSession,
    CycleNextSession,
    MoveActiveTabLeft,
    MoveActiveTabRight,
    MoveTabLeft(usize),
    MoveTabRight(usize),
    RenameTab(usize),
    ResetTabName(usize),
    RenamePane(PaneViewLocator),
    ResetPaneName(PaneViewLocator),
    RenameActiveTab,
    SetActiveTabName(String),
    /// Sets the manual color override for the active tab.
    ///
    /// - `Color(_)` — apply that color.
    /// - `Cleared` — explicitly clear (suppresses any directory default).
    /// - `Unset` — remove the manual override (lets the directory default apply, if any).
    SetActiveTabColor(SelectedTabColor),
    ToggleTabRightClickMenu {
        tab_index: usize,
        anchor: TabContextMenuAnchor,
    },
    ToggleVerticalTabsPaneContextMenu {
        tab_index: usize,
        target: VerticalTabsPaneContextMenuTarget,
        position: Vector2F,
    },
    TabHoverWidthStart {
        width: f32,
    },
    TabHoverWidthEnd,
    ToggleTabBarOverflowMenu,
    ToggleWelcomeTips,
    CloseTab(usize),
    CloseActiveTab,
    CloseOtherTabs(usize),
    CloseNonActiveTabs,
    CloseTabsRight(usize),
    CloseTabsRightActiveTab,
    AddDefaultTab,
    AddTerminalTab {
        hide_homepage: bool,
    },
    /// 在当前 tab 中央开新 terminal pane,执行 `ssh user@host`(openWarp 独有)。
    /// 由 SshServerView 的 Connect 按钮 / SshManagerPanel 右键"连接" 触发。
    OpenSshTerminal {
        node_id: String,
        server: warp_ssh_manager::SshServerInfo,
    },
    /// 打开/关闭左侧 panel 的 SSH 管理器视图(openWarp 独有)。
    ToggleSshManager,
    /// 打开/关闭左侧 panel 的 Skill 管理器视图(openWarp 独有)。
    ToggleSkillManager,
    AddTabWithShell {
        shell: AvailableShell,
        source: AddTabWithShellSource,
    },
    AddGetStartedTab,
    /// Add a new tab that immediately enters agent view with a new conversation.
    AddAgentTab,
    /// Add a new tab running a local Docker sandbox via `sbx`.
    AddDockerSandboxTab,
    OpenNewSessionMenu {
        position: Vector2F,
    },
    ToggleTabConfigsMenu,
    ToggleNewSessionMenu {
        position: Vector2F,
        is_vertical_tabs: bool,
    },
    SelectNewSessionMenuItem(NewSessionMenuItem),
    AutoupdateFailureLink,
    ApplyUpdate,
    // 去中心化分支:`LogOut` 已删除。
    CopyVersion(&'static str),
    DownloadNewVersion,
    ConfigureKeybindingSettings {
        keybinding_name: Option<String>,
    },
    ShowSettings,
    ShowSettingsPage(SettingsSection),
    ShowSettingsPageWithSearch {
        search_query: String,
        section: Option<SettingsSection>,
    },
    ShowThemeChooser(ThemeChooserMode),
    ShowThemeChooserForActiveTheme,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    IncreaseZoom,
    DecreaseZoom,
    ResetZoom,
    ActivateTabByNumber(usize),
    OpenPalette {
        mode: PaletteMode,
        source: PaletteSource,
        query: Option<String>,
    },
    TogglePalette {
        mode: PaletteMode,
        source: PaletteSource,
    },
    // 去中心化分支:`ShowUpgrade` / `ShowReferralSettingsPage` 已删除。
    JoinSlack,
    ViewUserDocs,
    ViewLatestChangelog,
    ViewPrivacyPolicy,
    SendFeedback,
    /// Open the log directory in the system file explorer with the current log file selected.
    #[cfg(not(target_family = "wasm"))]
    ViewLogs,
    ChangeCursor(Cursor),
    ToggleBlockSnackbar,
    ToggleErrorUnderlining,
    ToggleSyntaxHighlighting,
    CheckForUpdate,
    ExportAllWarpDriveObjects,
    SetA11yVerbosityLevel(AccessibilityVerbosity),
    ToggleNotifications,
    ToggleTabColor {
        color: AnsiColorIdentifier,
        tab_index: usize,
    },
    OpenLaunchConfigSaveModal,
    SelectTabConfig(TabConfig),
    DispatchToSettingsTab(SettingsTabAction),
    ToggleResourceCenter,
    ToggleUserMenu,
    ToggleAIAssistant,
    ClickedAIAssistantIcon,
    ToggleKeybindingsPage,
    ShowCommandSearch(CommandSearchOptions),
    CreatePersonalNotebook,
    ImportToPersonalDrive,
    CreatePersonalWorkflow,
    CreatePersonalFolder,
    CreatePersonalEnvVarCollection,
    CreatePersonalAIPrompt,
    ToggleMouseReporting,
    ToggleScrollReporting,
    ToggleFocusReporting,
    StartTabDrag,
    DragTab {
        tab_index: usize,
        tab_position: RectF,
    },
    DropTab,
    /// Toggles the left panel. In Code Mode V1 this toggles Warp Drive.
    /// In Code Mode V2 this toggles the left panel which contains both the project explorer and
    /// Warp Drive. This happens as explicit action from the user.
    ToggleLeftPanel,
    /// Toggles directly to the Warp Drive tab of the left panel in Code Mode V2
    ToggleWarpDrive,
    /// Unconditionally opens Warp Drive. This is used in the case of user lifecycle
    /// events like new user onboarding or when the user joins a team.
    OpenWarpDrive,
    /// Toggles the right panel. This happens as an explicit action from the user.
    ToggleRightPanel,
    /// Opens the code review panel (right panel) without toggling. If already open,
    /// switches to the target pane's repo. Used by vertical tabs diff stats chip.
    OpenCodeReviewPanel(PaneViewLocator),
    /// Toggles the vertical tabs panel. This happens as an explicit action from the user.
    ToggleVerticalTabsPanel,
    ToggleVerticalTabsSettingsPopup,
    SetVerticalTabsDisplayGranularity(VerticalTabsDisplayGranularity),
    SetVerticalTabsTabItemMode(VerticalTabsTabItemMode),
    SetVerticalTabsViewMode(VerticalTabsViewMode),
    SetVerticalTabsPrimaryInfo(VerticalTabsPrimaryInfo),
    SetVerticalTabsCompactSubtitle(VerticalTabsCompactSubtitle),
    ToggleVerticalTabsShowPrLink,
    ToggleVerticalTabsShowDiffStats,
    ToggleVerticalTabsShowDetailsOnHover,
    /// Closes the focused panel. This happens as an explicit action from the user.
    ClosePanel,
    CopyTextToClipboard(String),
    /// An action only registered in dev and local builds, which writes the user's current access
    /// token to the system clipboard to aid debugging and development.
    CopyAccessTokenToClipboard,
    DismissWorkspaceBanner(WorkspaceBanner),
    /// An action only registered in dev and local builds, which crashes the
    /// app (via a Sentry helper method) immediately when called.
    Crash,
    /// An action only registered in dev and local builds, which triggers a
    /// panic immediately when called.
    Panic,
    /// Stops the heap profiler (if one is running) and writes the profiling
    /// data to disk.
    DumpHeapProfile,
    ShowAIAssistantWarmWelcome,
    ClickedAIAssistantWarmWelcome,
    /// An action to open a new window with a view hierarchy debugger.
    OpenViewTreeDebugWindow,
    DismissAIAssistantWarmWelcome,
    /// An action to either upgrade syncing status from none or just in one tab
    /// to syncing all tabs, or downgrade from syncing all tabs to no syncing
    ToggleSyncAllTerminalInputsInAllTabs,
    /// An action to either cancel syncing
    /// or switch from no syncing/syncing all tabs to syncing within one tab
    ToggleSyncTerminalInputsInTab,
    /// An action to force terminal input syncing off
    DisableTerminalInputSync,
    HandleConflictingWorkflow(SyncId),
    HandleConflictingEnvVarCollection(SyncId),
    OpenPromptEditor {
        open_source: PromptEditorOpenSource,
    },
    OpenAgentToolbarEditor,
    OpenCLIAgentToolbarEditor,
    OpenHeaderToolbarEditor,
    ShowHeaderToolbarContextMenu {
        position: Vector2F,
    },
    // 去中心化分支:`Reauth` / `SignupAnonymousUser` / `SignInAnonymousWebUser` 已删除。
    OpenLink(String),
    /// On WASM, opens a given URL in the desktop Warp app (if installed) or redirects to download page.
    #[cfg(target_family = "wasm")]
    OpenLinkOnDesktop(url::Url),
    ReopenClosedSession,
    AddWindow,
    AddWindowWithShell {
        shell: AvailableShell,
    },
    /// Moves focus to the panel on the left
    FocusLeftPanel,
    /// Moves focus to the panel on the right
    FocusRightPanel,
    /// An action to view a newly created/edited workflow in WD from the toast
    ViewObjectInWarpDrive(WarpDriveItemId),
    UndoTrash(ObjectTypeAndId),
    /// Open a local path in the file explorer.
    OpenInExplorer {
        path: PathBuf,
    },
    /// Open a local file with the system's default application.
    OpenFilePath {
        path: PathBuf,
    },
    TerminateApp,
    CloseWindow,
    /// Help the user call the Warp executable with the [`crate::args::DEBUG_DUMP_FLAG`].
    DumpDebugInfo,
    /// Log review comment send eligibility for panes in the active tab.
    LogReviewCommentSendStatusForActiveTab,
    ToggleRecordingMode,
    ToggleInBandGenerators,
    ToggleDebugNetworkStatus,
    ToggleShowMemoryStats,
    RunAISuggestedCommand(String),
    RunCommand(String),
    InsertInInput {
        content: String,
        replace_buffer: bool,
        /// Whether to ensure agent mode is enabled when inserting content
        ensure_agent_mode: bool,
    },
    /// Open a new tab with its input in AI mode.
    NewTabInAgentMode {
        /// The entrypoint that triggered this action.
        entrypoint: AgentModeEntrypoint,
        /// The type of zero state prompt suggestion to start with (optional).
        zero_state_prompt_suggestion_type: Option<ZeroStatePromptSuggestionType>,
    },
    /// Open a new pane with its input in AI mode.
    NewPaneInAgentMode {
        /// The entrypoint that triggered this action.
        entrypoint: AgentModeEntrypoint,
        /// The type of zero state prompt suggestion to start with (optional).
        zero_state_prompt_suggestion_type: Option<ZeroStatePromptSuggestionType>,
    },
    // 去中心化分支:`AttemptLoginGatedAIUpgrade` 已删除。
    /// Dismisses the Wayland crash recovery banner and opens a link to our docs page with more
    /// information.
    #[cfg(target_os = "linux")]
    DismissWaylandCrashRecoveryBannerAndOpenLink,
    /// Open a new pane with its input in AI mode
    /// with query "Fix this" with error name and details from AI summary.
    FixInAgentMode {
        query: String,
    },
    OpenAIFactCollection,
    OpenMCPServerCollection,
    // OpenWarp Wave 7-3:`OpenEnvironmentManagementPane` WorkspaceAction 随 ambient-agent UI
    // 子系统物理删。
    ToggleAIDocumentPane {
        document_id: AIDocumentId,
        document_version: AIDocumentVersion,
    },
    /// Closes all visible AI document panes in the active pane group.
    HideAIDocumentPanes,
    /// Closes any other ai document panes in the active pane group, and opens the specified document_id.
    OpenAIDocumentPane {
        document_id: AIDocumentId,
        document_version: AIDocumentVersion,
    },
    FocusTerminalViewInWorkspace {
        terminal_view_id: EntityId,
    },
    /// Focus a specific pane by its locator (pane_group_id and pane_id).
    FocusPane(PaneViewLocator),
    /// Start a new AI conversation in a terminal view. This sets the pending query state
    /// to default and focuses the terminal view.
    StartNewConversation {
        terminal_view_id: EntityId,
    },
    /// Jump to the terminal pane of the most recent agent toast
    JumpToLatestToast,
    /// Open a file in a new tab with a code pane
    OpenFileInNewTab {
        full_path: PathBuf,
        line_and_column: Option<LineAndColumnArg>,
    },
    OpenNotebook {
        id: SyncId,
    },
    RunWorkflow {
        workflow: Arc<WorkflowType>,
        workflow_source: WorkflowSource,
        workflow_selection_source: WorkflowSelectionSource,
        argument_override: Option<HashMap<String, String>>,
    },
    ScrollToSettingsWidget {
        page: SettingsSection,
        widget_id: &'static str,
    },
    /// Navigate to an existing AI conversation, focusing on its terminal view.
    ///
    /// If the conversation is not in an open pane, restore it based on the layout setting or override.
    RestoreOrNavigateToConversation {
        pane_view_locator: Option<PaneViewLocator>,
        window_id: Option<WindowId>,
        conversation_id: AIConversationId,
        terminal_view_id: Option<EntityId>,
        /// If provided, use this layout to restore the conversation.
        /// Otherwise, fall back to the user's setting.
        restore_layout: Option<RestoreConversationLayout>,
    },
    /// Fork an existing AI conversation.
    /// Optionally summarizes the conversation after forking and/or sends an initial prompt.
    ForkAIConversation {
        conversation_id: AIConversationId,
        /// When Some, fork from the given response (or exchange if `fork_from_exact_exchange`
        /// is true). When None, fork from the last exchange.
        fork_from_exchange: Option<ForkFromExchange>,
        /// Whether to summarize the conversation after forking.
        summarize_after_fork: bool,
        /// Prompt to use for summarization when `summarize_after_fork` is true.
        summarization_prompt: Option<String>,
        /// Initial prompt to send in the forked conversation (sent after summarization if enabled).
        initial_prompt: Option<String>,
        /// Where to open the forked conversation.
        destination: ForkedConversationDestination,
    },
    /// Fork an existing AI conversation into a new pane and prefill the input with a local
    /// continuation command (selecting all text).
    #[cfg(not(target_family = "wasm"))]
    ContinueConversationLocally {
        conversation_id: AIConversationId,
    },
    /// Insert the /fork slash command into the active terminal's input.
    InsertForkSlashCommand,
    /// Summarize the active AI conversation in the focused pane.
    SummarizeAIConversation {
        prompt: Option<String>,
        /// Optional prompt to send after summarization completes successfully.
        initial_prompt: Option<String>,
    },
    /// Queue a prompt to be sent after the current conversation finishes.
    QueuePromptForConversation {
        prompt: String,
    },
    /// Install the Warp CLI command to /usr/local/bin
    #[cfg(target_os = "macos")]
    InstallCLI,
    /// Uninstall the Warp CLI command from /usr/local/bin
    #[cfg(target_os = "macos")]
    UninstallCLI,
    UndoRevertInCodeReviewPane {
        window_id: WindowId,
        view_id: EntityId,
    },
    /// Handle a file being renamed in the file tree
    #[cfg(feature = "local_fs")]
    FileRenamed {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    /// Handle a file being deleted in the file tree
    #[cfg(feature = "local_fs")]
    FileDeleted {
        path: PathBuf,
    },
    /// Open a repository directory via file picker. The `path` is an `Option` because some
    /// dispatchers don't know the path to open yet (so the Workspace must open the file picker)
    /// and some do, e.g. the GetStartedView. The GetStartedView needs to handle the file picker
    /// because it needs to determine whether or not to close itself based on whether the user
    /// actually selects a file in the file picker or cancels it.
    OpenRepository {
        path: Option<String>,
    },
    /// Open the native folder picker for a repo param in the tab-config modal after the
    /// current interaction cycle finishes.
    OpenTabConfigRepoPicker {
        param_index: usize,
    },
    /// Open a new blank code file in the current tab
    NewCodeFile,
    NavigatePrevPaneOrPanel,
    NavigateNextPaneOrPanel,
    ToggleProjectExplorer,
    ToggleGlobalSearch,
    OpenGlobalSearch,
    ToggleConversationListView,
    /// Reset the AWS Bedrock login banner dismissed state (for debugging).
    #[cfg(debug_assertions)]
    DebugResetAwsBedrockLoginBannerDismissed,
    /// Open the OpenWarp Launch Modal (for debugging)
    #[cfg(debug_assertions)]
    OpenOpenWarpLaunchModal,
    /// Reset the OpenWarp launch modal dismissed state (for debugging)
    #[cfg(debug_assertions)]
    ResetOpenWarpLaunchModalState,
    /// Install the opencode-warp plugin from GitHub into the global opencode config.
    #[cfg(debug_assertions)]
    InstallOpenCodeWarpPlugin,
    /// Use a local checkout of the opencode-warp plugin (for testing/development).
    #[cfg(debug_assertions)]
    UseLocalOpenCodeWarpPlugin,
    /// Take a process sample of the app (equivalent to Activity Monitor > Sample Process).
    #[cfg(target_os = "macos")]
    SampleProcess,
    ToggleNotificationMailbox {
        select_first: bool,
    },
    ToggleAgentManagementView,
    ViewAgentRunsForEnvironment {
        environment_id: String,
    },
    /// Show the rewind confirmation dialog before rewinding an AI conversation
    ShowRewindConfirmationDialog {
        ai_block_view_id: EntityId,
        exchange_id: AIAgentExchangeId,
        conversation_id: AIConversationId,
    },
    /// Execute the actual rewind after confirmation
    ExecuteRewindAIConversation {
        ai_block_view_id: EntityId,
        exchange_id: AIAgentExchangeId,
        conversation_id: AIConversationId,
    },
    /// Execute the actual deletion of a conversation after confirmation
    ExecuteDeleteConversation {
        conversation_id: AIConversationId,
        terminal_view_id: Option<EntityId>,
    },
    /// Open an ambient agent session by joining its shared session.
    /// Used when the sandbox is running or when we need to view a live session.
    OpenAmbientAgentSession {
        task_id: AmbientAgentTaskId,
    },
    /// Load conversation data into a transcript viewer.
    /// Used for persisted view-only conversations.
    OpenConversationTranscriptViewer {
        conversation_id: ServerConversationToken,
        ambient_agent_task_id: Option<AmbientAgentTaskId>,
    },
    /// Toggle the conversation transcript details panel (WASM-only).
    #[cfg(target_family = "wasm")]
    ToggleConversationTranscriptDetailsPanel,
    /// Open a full-window lightbox displaying the given images.
    OpenLightbox {
        images: Vec<lightbox::LightboxImage>,
        /// The index of the image to display initially.
        initial_index: usize,
    },
    /// Update a single image in the currently open lightbox.
    UpdateLightboxImage {
        index: usize,
        image: lightbox::LightboxImage,
    },
    StartAgentOnboardingTutorial(OnboardingTutorial),
    ShowSessionConfigModal,
    DismissSessionConfigTabConfigChip,
    /// Start the HOA onboarding flow (for debugging)
    #[cfg(debug_assertions)]
    ShowHoaOnboardingFlow,
    /// Open the "New worktree" modal for creating a reusable worktree tab config.
    OpenNewWorktreeModal,
    /// Open the native folder picker for the repo field in the new-worktree modal.
    OpenNewWorktreeRepoPicker,
    /// Create a new worktree in the given repo using the default worktree tab config.
    /// The branch name is auto-generated.
    OpenWorktreeInRepo {
        repo_path: String,
    },
    SaveCurrentTabAsNewConfig(usize),
    SyncTrafficLights,
    /// Opens a tab config file in the editor and dismisses the associated error toast.
    OpenTabConfigErrorFile {
        path: PathBuf,
        toast_object_id: String,
    },
    /// Sidecar action: set the hovered item as the Cmd+T default.
    TabConfigSidecarMakeDefault {
        mode: crate::settings::ai::DefaultSessionMode,
        tab_config_path: Option<PathBuf>,
        shell: Option<AvailableShell>,
    },
    /// Sidecar action: open the tab config TOML in the user's editor.
    TabConfigSidecarEditConfig {
        path: PathBuf,
    },
    /// Sidecar action: show the remove confirmation dialog for a tab config.
    TabConfigSidecarRemoveConfig {
        name: String,
        path: PathBuf,
    },
    /// Opens the settings.toml file in a code editor pane.
    OpenSettingsFile,
    /// Opens a new agent session to fix settings.toml errors using the modify-settings skill.
    FixSettingsWithOz {
        error_description: String,
    },
}

impl From<&WorkspaceAction> for LoginGatedFeature {
    fn from(val: &WorkspaceAction) -> LoginGatedFeature {
        let _ = val;
        "Unknown reason"
    }
}

impl WorkspaceAction {
    pub fn blocked_for_anonymous_user(&self) -> bool {
        false
    }

    /// Matches what actions require the app state to be saved, and which don't. We match all
    /// actions directly, rather than using _, so we're forced to make a conscious decision for each
    /// of them, rather than following some default.
    pub fn should_save_app_state_on_action(&self) -> bool {
        use WorkspaceAction::*;
        match self {
            #[cfg(not(target_family = "wasm"))]
            ContinueConversationLocally { .. } => true,
            ActivateTab(_)
            | ActivateTabByNumber(_)
            | ActivatePrevTab
            | ActivateNextTab
            | ActivateLastTab
            | CyclePrevSession
            | CycleNextSession
            | MoveActiveTabLeft
            | MoveActiveTabRight
            | MoveTabLeft(_)
            | MoveTabRight(_)
            | DropTab
            | RenameTab(_)
            | ResetTabName(_)
            | RenamePane(_)
            | ResetPaneName(_)
            | RenameActiveTab
            | SetActiveTabName(_)
            | SetActiveTabColor(_)
            | CloseTab(_)
            | CloseActiveTab
            | CloseOtherTabs(_)
            | CloseNonActiveTabs
            | CloseTabsRight(_)
            | CloseTabsRightActiveTab
            | ToggleTabColor { .. }
            | AddDefaultTab
            | AddTerminalTab { .. }
            | OpenSshTerminal { .. }
            | ToggleSshManager
            | ToggleSkillManager
            | AddTabWithShell { .. }
            | AddGetStartedTab
            | AddAgentTab
            | AddDockerSandboxTab
            | AddWindow
            | AddWindowWithShell { .. }
            | CloseWindow
            | ScrollToSettingsWidget { .. }
            | NewTabInAgentMode { .. }
            | NewPaneInAgentMode { .. }
            | FixInAgentMode { .. }
            | OpenNotebook { .. }
            | RunWorkflow { .. }
            | OpenFileInNewTab { .. }
            | RestoreOrNavigateToConversation { .. }
            | NewCodeFile
            | ForkAIConversation { .. }
            | SummarizeAIConversation { .. }
            | OpenRepository { .. }
            | SelectTabConfig(_)
            | ToggleVerticalTabsPanel => true, // actions that actually change a state of the state of user's
            // workspace would most likely require a save, so that if the app gets
            // restarted, the user can continue working
            AutoupdateFailureLink
            | ApplyUpdate
            | CopyVersion(_)
            | DownloadNewVersion
            | ConfigureKeybindingSettings { .. }
            | ExportAllWarpDriveObjects
            | ShowSettings
            | ShowSettingsPage(_)
            | ShowSettingsPageWithSearch { .. }
            | ShowThemeChooser(_)
            | ShowThemeChooserForActiveTheme
            | IncreaseFontSize
            | DecreaseFontSize
            | ResetFontSize
            | IncreaseZoom
            | DecreaseZoom
            | ResetZoom
            | OpenPalette { .. }
            | TogglePalette { mode: _, source: _ }
            | JoinSlack
            | ViewUserDocs
            | ViewLatestChangelog
            | ViewPrivacyPolicy
            | SendFeedback
            | ChangeCursor(_)
            | ToggleBlockSnackbar
            | ToggleErrorUnderlining
            | ToggleSyntaxHighlighting
            | OpenLaunchConfigSaveModal
            | ToggleTabRightClickMenu { .. }
            | ToggleVerticalTabsPaneContextMenu { .. }
            | OpenNewSessionMenu { .. }
            | ToggleTabConfigsMenu
            | ToggleNewSessionMenu { .. }
            | SelectNewSessionMenuItem(_)
            | ToggleTabBarOverflowMenu
            | CheckForUpdate
            | SetA11yVerbosityLevel(_)
            | ToggleNotifications
            | DispatchToSettingsTab { .. }
            | ToggleResourceCenter
            | ToggleUserMenu
            | ClickedAIAssistantIcon
            | ToggleAIAssistant
            | ToggleKeybindingsPage
            | ShowCommandSearch(_)
            | ToggleMouseReporting
            | ToggleScrollReporting
            | ToggleFocusReporting
            | ImportToPersonalDrive
            | CreatePersonalNotebook
            | CreatePersonalWorkflow
            | CreatePersonalFolder
            | CreatePersonalEnvVarCollection
            | CreatePersonalAIPrompt
            | OpenInExplorer { .. }
            | DragTab { .. }
            | StartTabDrag
            | ToggleLeftPanel
            | ToggleWarpDrive
            | OpenWarpDrive
            | ClosePanel
            | ToggleRightPanel
            | OpenCodeReviewPanel(..)
            | ToggleVerticalTabsSettingsPopup
            | SetVerticalTabsDisplayGranularity(_)
            | SetVerticalTabsTabItemMode(_)
            | SetVerticalTabsViewMode(_)
            | SetVerticalTabsPrimaryInfo(_)
            | SetVerticalTabsCompactSubtitle(_)
            | ToggleVerticalTabsShowPrLink
            | ToggleVerticalTabsShowDiffStats
            | ToggleVerticalTabsShowDetailsOnHover
            | ToggleWelcomeTips
            | CopyTextToClipboard(_)
            | CopyAccessTokenToClipboard
            | OpenTabConfigRepoPicker { .. }
            | OpenNewWorktreeModal
            | OpenNewWorktreeRepoPicker
            | OpenWorktreeInRepo { .. }
            | Crash
            | Panic
            | DumpHeapProfile
            | OpenViewTreeDebugWindow
            | ShowAIAssistantWarmWelcome
            | ClickedAIAssistantWarmWelcome
            | DismissAIAssistantWarmWelcome
            | DismissWorkspaceBanner(..)
            | ToggleSyncAllTerminalInputsInAllTabs
            | ToggleSyncTerminalInputsInTab
            | DisableTerminalInputSync
            | HandleConflictingWorkflow(_)
            | HandleConflictingEnvVarCollection(_)
            | OpenPromptEditor { .. }
            | OpenAgentToolbarEditor
            | OpenCLIAgentToolbarEditor
            | OpenHeaderToolbarEditor
            | ShowHeaderToolbarContextMenu { .. }
            | OpenLink(_)
            | ReopenClosedSession
            | FocusLeftPanel
            | FocusRightPanel
            | DumpDebugInfo
            | LogReviewCommentSendStatusForActiveTab
            | ToggleRecordingMode
            | ToggleInBandGenerators
            | ToggleDebugNetworkStatus
            | ToggleShowMemoryStats
            | RunAISuggestedCommand { .. }
            | RunCommand { .. }
            | InsertInInput { .. }
            | InsertForkSlashCommand
            | QueuePromptForConversation { .. }
            | UndoTrash(_)
            | OpenFilePath { .. }
            | ViewObjectInWarpDrive(_)
            | TerminateApp
            | TabHoverWidthStart { .. }
            | TabHoverWidthEnd
            | OpenAIFactCollection
            | OpenMCPServerCollection
            | FocusTerminalViewInWorkspace { .. }
            | FocusPane(..)
            | StartNewConversation { .. }
            | UndoRevertInCodeReviewPane { .. }
            | JumpToLatestToast
            | NavigatePrevPaneOrPanel
            | NavigateNextPaneOrPanel
            | ToggleProjectExplorer
            | ToggleGlobalSearch
            | OpenGlobalSearch
            | ToggleConversationListView
            | ToggleNotificationMailbox { .. }
            | ToggleAgentManagementView
            | ViewAgentRunsForEnvironment { .. }
            | ToggleAIDocumentPane { .. }
            | HideAIDocumentPanes
            | OpenAIDocumentPane { .. }
            | ShowRewindConfirmationDialog { .. }
            | ExecuteRewindAIConversation { .. }
            | ExecuteDeleteConversation { .. }
            | OpenAmbientAgentSession { .. }
            | OpenConversationTranscriptViewer { .. }
            | OpenLightbox { .. }
            | UpdateLightboxImage { .. }
            | StartAgentOnboardingTutorial(_)
            | ShowSessionConfigModal
            | DismissSessionConfigTabConfigChip
            | SaveCurrentTabAsNewConfig(_)
            | SyncTrafficLights
            | OpenTabConfigErrorFile { .. }
            | TabConfigSidecarMakeDefault { .. }
            | TabConfigSidecarEditConfig { .. }
            | TabConfigSidecarRemoveConfig { .. }
            | OpenSettingsFile
            | FixSettingsWithOz { .. } => false,
            #[cfg(debug_assertions)]
            ShowHoaOnboardingFlow => false,
            #[cfg(target_family = "wasm")]
            ToggleConversationTranscriptDetailsPanel => false,
            #[cfg(debug_assertions)]
            DebugResetAwsBedrockLoginBannerDismissed
            | OpenOpenWarpLaunchModal
            | ResetOpenWarpLaunchModalState
            | InstallOpenCodeWarpPlugin
            | UseLocalOpenCodeWarpPlugin => false,
            #[cfg(not(target_family = "wasm"))]
            ViewLogs => false,
            #[cfg(target_os = "macos")]
            SampleProcess => false,
            #[cfg(target_os = "macos")]
            InstallCLI | UninstallCLI => false,
            #[cfg(feature = "local_fs")]
            FileRenamed { .. } => false, // File rename doesn't change workspace state
            #[cfg(feature = "local_fs")]
            FileDeleted { .. } => false, // File deletion doesn't change workspace state
            // OpenWarp Wave 7-3:`OpenEnvironmentManagementPane` WorkspaceAction 随 ambient-agent UI
            // 子系统物理删。
            #[cfg(target_os = "linux")]
            DismissWaylandCrashRecoveryBannerAndOpenLink => false,
            #[cfg(target_family = "wasm")]
            OpenLinkOnDesktop(_) => false,
            // actions that are related to updating user settings or
            // managing some ui elements (like closing/opening modals)
            // that don't reflect on actual workspace and don't need to
            // be preserved between restarts.
        }
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
