use crate::server::telemetry::CLIAgentType;
use crate::view_components::find::FindDirection;
use crate::code_review::diff_state::DiffMode;
use serde::Serialize;
use serde_with::SerializeDisplay;
use std::fmt::Display;

/// Entry points for opening the code review pane.
#[derive(Clone, Copy, Debug, SerializeDisplay, Default)]
pub enum CodeReviewPaneEntrypoint {
    /// Opened via the git diff chip (git changes button in AI control panel).
    GitDiffChip,
    /// Opened via the "View changes" button when Agent mode is done running.
    AgentModeCompleted,
    /// Opened via the "Review changes" button when Agent mode is running.
    AgentModeRunning,
    /// Opened via the "/code-review" slash command.
    SlashCommand,
    /// Opened by the agent tool call.
    InvokedByAgent,
    // Force opened when user accepted first diff of a conversation
    ForceOpened,
    // Opened via the agent mode diff header
    CodeDiffHeader,
    // Opened via the pane header
    PaneHeader,
    // Opened via the code mode v2 right panel button
    RightPanel,
    /// Opened via the CLI agent view footer (e.g., Claude Code).
    CLIAgentView,
    /// Opened via other means (unknown entry point).
    #[default]
    Other,
}

impl Display for CodeReviewPaneEntrypoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitDiffChip => write!(f, "git_diff_chip"),
            Self::AgentModeCompleted => write!(f, "agent_mode_completed"),
            Self::AgentModeRunning => write!(f, "agent_mode_running"),
            Self::SlashCommand => write!(f, "slash_command"),
            Self::InvokedByAgent => write!(f, "invoked_by_agent"),
            Self::ForceOpened => write!(f, "force_opened"),
            Self::CodeDiffHeader => write!(f, "agent_mode_diff_header"),
            Self::PaneHeader => write!(f, "pane_header"),
            Self::RightPanel => write!(f, "right_panel"),
            Self::CLIAgentView => write!(f, "cli_agent_view"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Origin of an "Add to context" action.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum AddToContextOrigin {
    /// User selected text and added it to context.
    #[serde(rename = "selected_text")]
    SelectedText,
    /// User clicked the gutter to add a line/hunk to context.
    #[serde(rename = "gutter")]
    Gutter,
    /// User clicked the "Add diff set as context" button in code review header.
    #[serde(rename = "code_review_header")]
    #[allow(unused)]
    CodeReviewHeader,
}

/// Where code review content was sent after the user action.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum CodeReviewContextDestination {
    /// Written directly to the terminal PTY for an active CLI agent.
    #[serde(rename = "pty")]
    Pty,
    /// Inserted into the Warp AI input buffer as plain text.
    #[serde(rename = "agent_input")]
    AgentInput,
    /// Registered as an AI attachment and referenced from the input.
    #[serde(rename = "agent_attachment")]
    AgentAttachment,
    /// Inserted into the active command buffer while a command is running.
    #[serde(rename = "active_command_buffer")]
    ActiveCommandBuffer,
    /// Submitted as an inline code review request through the Warp AI path.
    #[serde(rename = "agent_review")]
    AgentReview,
    /// Inserted into CLI agent rich input.
    #[serde(rename = "rich_input")]
    RichInput,
}

/// Scope of a diff set attachment initiated from code review.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub enum DiffSetContextScope {
    /// Attach the full diff set for the current review.
    #[serde(rename = "all")]
    All,
    /// Attach the diff set for a single file.
    #[serde(rename = "file")]
    File,
}

/// Pane state change for minimize/maximize events.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum PaneStateChange {
    /// Pane was minimized.
    #[serde(rename = "minimized")]
    Minimized,
    /// Pane was maximized.
    #[serde(rename = "maximized")]
    Maximized,
}

/// Telemetry events associated with the code review pane.
#[derive(Serialize, Debug)]
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub enum CodeReviewTelemetryEvent {
    /// Emitted when the code review pane is opened.
    PaneOpened {
        entrypoint: CodeReviewPaneEntrypoint,
        is_code_mode_v2: bool,
        /// The CLI agent type if opened from a CLI agent footer (e.g., Claude Code).
        cli_agent: Option<CLIAgentType>,
    },
    /// Emitted when a user adds content to AI context from code review.
    AddToContext {
        origin: AddToContextOrigin,
        destination: CodeReviewContextDestination,
        diff_set_scope: Option<DiffSetContextScope>,
    },
    /// Emitted when a user clicks the revert hunk button.
    RevertHunkClicked,
    /// Emitted when a file is saved in the code review pane.
    FileSaved,
    /// Emitted when the code review pane is minimized or maximized.
    PaneStateChanged { state_change: PaneStateChange },
    /// Emitted when the diff base is changed (e.g., from uncommitted to main branch).
    BaseChanged {
        /// The new diff mode.
        mode: DiffMode,
    },
    /// Failure when we are calculating the diff metadata.
    CalculateDiffMetadataFailed { error: String },
    /// Failure when we are loading the actual diff content.
    LoadDiffFailed { error: String },
    /// Emitted when the code review find bar is opened or closed.
    FindBarToggled {
        /// Whether the find bar is now open.
        is_open: bool,
    },
    /// Emitted when search mode settings are changed.
    FindBarModeChanged {
        /// Whether case-sensitive search is enabled.
        case_sensitive: bool,
        /// Whether regex search is enabled.
        regex: bool,
    },
    /// Emitted when the user navigates to the next or previous match.
    FindNavigated {
        /// Direction of navigation.
        direction: FindDirection,
    },
    /// Emitted when the inline comment editor is opened in the code review pane.
    CommentEditorOpened,
    /// Emitted when a new comment is added to the inline review.
    CommentAdded,
    /// Emitted when an existing comment is edited.
    CommentEdited,
    /// Emitted when a comment is deleted from the inline review.
    CommentDeleted { is_imported: bool },
    /// Emitted when the bottom comment list panel is expanded.
    CommentListExpanded {
        /// Number of comments currently in the list.
        comment_count: usize,
    },
    /// Emitted when the user submits an inline review to the agent.
    ReviewSubmitted {
        /// Number of comments in the submitted review.
        comment_count: usize,
        /// Number of unique files with comments.
        file_count: usize,
        /// Where the review was submitted.
        destination: CodeReviewContextDestination,
    },
    /// Emitted when a comment in the list view is clicked to jump to its location.
    CommentListItemClicked,
    /// Emitted when one or more comments fail to be precisely relocated after code changes.
    CommentRelocationFailed {
        /// Number of comments that could not be matched to an exact line and had to fall back.
        fallback_count: usize,
    },
    /// Emitted when one or more comments are resolved.
    CommentResolved {
        /// Number of comments resolved by this operation.
        resolved_count: usize,
    },
    /// Emitted when the agent's insert_code_review_comments tool call is received and processed.
    CommentsReceived {
        /// Number of raw InsertReviewComment items from the tool call.
        raw_count: usize,
        /// Number of successfully converted PendingImportedReviewComments.
        converted_count: usize,
        /// Number of AttachedReviewComments after thread flattening.
        thread_count: usize,
    },
    /// Emitted after newly-imported comments are relocated against editor lines.
    CommentsAttached {
        /// Number of non-outdated imported comments after relocation.
        active_count: usize,
        /// Number of outdated imported comments after relocation.
        outdated_count: usize,
    },
}
