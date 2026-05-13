use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::ambient_agents::{AgentSource, AmbientAgentTask, AmbientAgentTaskState};
use crate::ai::artifacts::Artifact;
use crate::ai::blocklist::{format_credits, BlocklistAIHistoryEvent, BlocklistAIHistoryModel};
use crate::ai::conversation_navigation::ConversationNavigationData;
use crate::auth::{AuthStateProvider, UserUid};
use crate::ui_components::icons::Icon;
use crate::workspace::{RestoreConversationLayout, WorkspaceAction};
use crate::workspaces::user_profiles::UserProfiles;
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;
use warp_core::ui::theme::{color::internal_colors, WarpTheme};
use warpui::color::ColorU;
use warpui::{AppContext, Entity, EntityId, ModelContext, SingletonEntity, WindowId};

const SESSION_EXPIRATION_TIME: chrono::Duration = chrono::Duration::weeks(1);

#[derive(PartialEq)]
pub enum SessionStatus {
    Available,
    Expired,
    Unavailable,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum StatusFilter {
    #[default]
    All,
    Working,
    Done,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SourceFilter {
    #[default]
    All,
    Specific(AgentSource),
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CreatorFilter {
    #[default]
    All,
    Specific {
        name: String,
        uid: String,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ArtifactFilter {
    #[default]
    All,
    PullRequest,
    Plan,
    Screenshot,
    File,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CreatedOnFilter {
    #[default]
    All,
    Last24Hours,
    Past3Days,
    LastWeek,
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum EnvironmentFilter {
    #[default]
    All,
    NoEnvironment,
    Specific(String),
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnerFilter {
    All,
    #[default]
    PersonalOnly,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum HarnessFilter {
    #[default]
    All,
    Specific(Harness),
}

impl Serialize for HarnessFilter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            HarnessFilter::All => serializer.serialize_str("all"),
            HarnessFilter::Specific(harness) => serializer.collect_str(harness),
        }
    }
}

impl<'de> Deserialize<'de> for HarnessFilter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(Harness::from_str(&raw, false)
            .ok()
            .map(HarnessFilter::Specific)
            .unwrap_or(HarnessFilter::All))
    }
}

#[derive(Default, PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct AgentManagementFilters {
    pub owners: OwnerFilter,
    pub status: StatusFilter,
    pub source: SourceFilter,
    pub created_on: CreatedOnFilter,
    pub creator: CreatorFilter,
    pub artifact: ArtifactFilter,
    #[serde(default)]
    pub environment: EnvironmentFilter,
    #[serde(default)]
    pub harness: HarnessFilter,
}

impl AgentManagementFilters {
    pub fn reset_all_but_owner(&mut self) {
        self.status = StatusFilter::default();
        self.source = SourceFilter::default();
        self.created_on = CreatedOnFilter::default();
        self.creator = CreatorFilter::default();
        self.artifact = ArtifactFilter::default();
        self.environment = EnvironmentFilter::default();
        self.harness = HarnessFilter::default();
    }

    pub fn is_filtering(&self) -> bool {
        self.status != StatusFilter::default()
            || self.source != SourceFilter::default()
            || self.created_on != CreatedOnFilter::default()
            || self.creator != CreatorFilter::default() && self.owners != OwnerFilter::PersonalOnly
            || self.artifact != ArtifactFilter::default()
            || self.environment != EnvironmentFilter::default()
            || self.harness != HarnessFilter::default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentRunDisplayStatus {
    TaskQueued,
    TaskPending,
    TaskClaimed,
    TaskInProgress,
    TaskSucceeded,
    TaskFailed,
    TaskError,
    TaskBlocked { blocked_action: String },
    TaskCancelled,
    TaskUnknown,
    ConversationInProgress,
    ConversationSucceeded,
    ConversationError,
    ConversationBlocked { blocked_action: String },
    ConversationCancelled,
}

impl AgentRunDisplayStatus {
    pub fn from_task(task: &AmbientAgentTask, app: &AppContext) -> Self {
        match &task.state {
            AmbientAgentTaskState::Queued
            | AmbientAgentTaskState::Pending
            | AmbientAgentTaskState::Claimed => Self::from_task_state(task),
            AmbientAgentTaskState::InProgress => {
                let history_model = BlocklistAIHistoryModel::as_ref(app);
                AgentConversationsModel::conversation_id_shadowed_by_task(task, history_model)
                    .and_then(|conversation_id| history_model.conversation(&conversation_id))
                    .map(|conversation| Self::from_conversation_status(conversation.status()))
                    .unwrap_or_else(|| Self::from_task_state(task))
            }
            AmbientAgentTaskState::Succeeded
            | AmbientAgentTaskState::Failed
            | AmbientAgentTaskState::Error
            | AmbientAgentTaskState::Blocked
            | AmbientAgentTaskState::Cancelled
            | AmbientAgentTaskState::Unknown => Self::from_task_state(task),
        }
    }

    pub fn from_conversation_status(status: &ConversationStatus) -> Self {
        match status {
            ConversationStatus::InProgress => Self::ConversationInProgress,
            ConversationStatus::Success => Self::ConversationSucceeded,
            ConversationStatus::Error => Self::ConversationError,
            ConversationStatus::Cancelled => Self::ConversationCancelled,
            ConversationStatus::Blocked { blocked_action } => Self::ConversationBlocked {
                blocked_action: blocked_action.clone(),
            },
        }
    }

    fn from_task_state(task: &AmbientAgentTask) -> Self {
        match &task.state {
            AmbientAgentTaskState::Queued => Self::TaskQueued,
            AmbientAgentTaskState::Pending => Self::TaskPending,
            AmbientAgentTaskState::Claimed => Self::TaskClaimed,
            AmbientAgentTaskState::InProgress => Self::TaskInProgress,
            AmbientAgentTaskState::Succeeded => Self::TaskSucceeded,
            AmbientAgentTaskState::Failed => Self::TaskFailed,
            AmbientAgentTaskState::Error => Self::TaskError,
            AmbientAgentTaskState::Blocked => Self::TaskBlocked {
                blocked_action: task
                    .status_message
                    .as_ref()
                    .map(|m| m.message.clone())
                    .unwrap_or_else(|| "Task blocked".to_string()),
            },
            AmbientAgentTaskState::Cancelled => Self::TaskCancelled,
            AmbientAgentTaskState::Unknown => Self::TaskUnknown,
        }
    }

    pub fn status_filter(&self) -> StatusFilter {
        match self {
            AgentRunDisplayStatus::TaskQueued
            | AgentRunDisplayStatus::TaskPending
            | AgentRunDisplayStatus::TaskClaimed
            | AgentRunDisplayStatus::TaskInProgress
            | AgentRunDisplayStatus::ConversationInProgress => StatusFilter::Working,
            AgentRunDisplayStatus::TaskSucceeded | AgentRunDisplayStatus::ConversationSucceeded => {
                StatusFilter::Done
            }
            AgentRunDisplayStatus::TaskFailed
            | AgentRunDisplayStatus::TaskError
            | AgentRunDisplayStatus::TaskBlocked { .. }
            | AgentRunDisplayStatus::TaskCancelled
            | AgentRunDisplayStatus::TaskUnknown
            | AgentRunDisplayStatus::ConversationError
            | AgentRunDisplayStatus::ConversationBlocked { .. }
            | AgentRunDisplayStatus::ConversationCancelled => StatusFilter::Failed,
        }
    }

    pub fn is_cancellable(&self) -> bool {
        self.is_working()
    }

    pub fn is_working(&self) -> bool {
        matches!(
            self,
            AgentRunDisplayStatus::TaskQueued
                | AgentRunDisplayStatus::TaskPending
                | AgentRunDisplayStatus::TaskClaimed
                | AgentRunDisplayStatus::TaskInProgress
                | AgentRunDisplayStatus::ConversationInProgress
        )
    }

    pub fn status_icon_and_color(&self, theme: &WarpTheme) -> (Icon, ColorU) {
        match self {
            AgentRunDisplayStatus::TaskQueued
            | AgentRunDisplayStatus::TaskPending
            | AgentRunDisplayStatus::TaskClaimed
            | AgentRunDisplayStatus::TaskInProgress
            | AgentRunDisplayStatus::ConversationInProgress => {
                (Icon::ClockLoader, theme.ansi_fg_magenta())
            }
            AgentRunDisplayStatus::TaskSucceeded | AgentRunDisplayStatus::ConversationSucceeded => {
                (Icon::Check, theme.ansi_fg_green())
            }
            AgentRunDisplayStatus::TaskFailed
            | AgentRunDisplayStatus::TaskError
            | AgentRunDisplayStatus::TaskUnknown
            | AgentRunDisplayStatus::ConversationError => (Icon::Triangle, theme.ansi_fg_red()),
            AgentRunDisplayStatus::TaskBlocked { .. }
            | AgentRunDisplayStatus::ConversationBlocked { .. } => {
                (Icon::StopFilled, theme.ansi_fg_yellow())
            }
            AgentRunDisplayStatus::TaskCancelled => (
                Icon::Cancelled,
                theme.disabled_text_color(theme.background()).into_solid(),
            ),
            AgentRunDisplayStatus::ConversationCancelled => {
                (Icon::StopFilled, internal_colors::neutral_5(theme))
            }
        }
    }
}

impl std::fmt::Display for AgentRunDisplayStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRunDisplayStatus::TaskQueued => write!(f, "Queued"),
            AgentRunDisplayStatus::TaskPending => write!(f, "Pending"),
            AgentRunDisplayStatus::TaskClaimed => write!(f, "Claimed"),
            AgentRunDisplayStatus::TaskInProgress
            | AgentRunDisplayStatus::ConversationInProgress => write!(f, "In progress"),
            AgentRunDisplayStatus::TaskSucceeded | AgentRunDisplayStatus::ConversationSucceeded => {
                write!(f, "Done")
            }
            AgentRunDisplayStatus::TaskFailed => write!(f, "Failed"),
            AgentRunDisplayStatus::TaskError | AgentRunDisplayStatus::ConversationError => {
                write!(f, "Error")
            }
            AgentRunDisplayStatus::TaskBlocked { .. }
            | AgentRunDisplayStatus::ConversationBlocked { .. } => write!(f, "Blocked"),
            AgentRunDisplayStatus::TaskCancelled | AgentRunDisplayStatus::ConversationCancelled => {
                write!(f, "Cancelled")
            }
            AgentRunDisplayStatus::TaskUnknown => write!(f, "Failed"),
        }
    }
}

/// Stores conversation metadata needed for display in conversation/task views.
pub struct ConversationMetadata {
    pub nav_data: ConversationNavigationData,
}

/// ConversationOrTask is a wrapper around either conversation
/// or task data stored in the `AgentConversationsModel`.
///
/// It provides a unified interface for reading data related to tasks and conversations.
pub enum ConversationOrTask<'a> {
    Task(&'a AmbientAgentTask),
    Conversation(&'a ConversationMetadata),
}

impl ConversationOrTask<'_> {
    pub fn title(&self, app: &AppContext) -> String {
        match self {
            ConversationOrTask::Task(task) => task.title.clone(),
            ConversationOrTask::Conversation(metadata) => {
                // We try to read the title from the history model first (that's the most up-to-date),
                // but fall back to the one stored in the navigation data.
                let history_model = BlocklistAIHistoryModel::as_ref(app);
                history_model
                    .conversation(&metadata.nav_data.id)
                    .and_then(|conv| conv.title().clone())
                    .unwrap_or(metadata.nav_data.title.clone())
            }
        }
    }

    /// Map to conversation status for the UI status display
    pub fn status(&self, app: &AppContext) -> ConversationStatus {
        match self {
            ConversationOrTask::Task(task) => match &task.state {
                AmbientAgentTaskState::Queued
                | AmbientAgentTaskState::Pending
                | AmbientAgentTaskState::Claimed
                | AmbientAgentTaskState::InProgress => ConversationStatus::InProgress,
                AmbientAgentTaskState::Succeeded => ConversationStatus::Success,
                AmbientAgentTaskState::Cancelled => ConversationStatus::Cancelled,
                AmbientAgentTaskState::Blocked => ConversationStatus::Blocked {
                    blocked_action: task
                        .status_message
                        .as_ref()
                        .map(|m| m.message.clone())
                        .unwrap_or_else(|| "Task blocked".to_string()),
                },
                AmbientAgentTaskState::Failed
                | AmbientAgentTaskState::Error
                | AmbientAgentTaskState::Unknown => ConversationStatus::Error,
            },
            ConversationOrTask::Conversation(metadata) => {
                let history_model = BlocklistAIHistoryModel::as_ref(app);
                history_model
                    .conversation(&metadata.nav_data.id)
                    .map(|conv| conv.status().clone())
                    .unwrap_or(ConversationStatus::Success)
            }
        }
    }

    pub fn display_status(&self, app: &AppContext) -> AgentRunDisplayStatus {
        match self {
            ConversationOrTask::Task(task) => AgentRunDisplayStatus::from_task(task, app),
            ConversationOrTask::Conversation(metadata) => {
                let history_model = BlocklistAIHistoryModel::as_ref(app);
                history_model
                    .conversation(&metadata.nav_data.id)
                    .map(|conv| AgentRunDisplayStatus::from_conversation_status(conv.status()))
                    .unwrap_or(AgentRunDisplayStatus::ConversationSucceeded)
            }
        }
    }

    /// Grab the creator name from the task, or from the auth state if it is a conversation
    pub fn creator_name(&self, app: &AppContext) -> Option<String> {
        match self {
            ConversationOrTask::Task(task) => task.creator_display_name().or_else(|| {
                // Fallback to the cached users in the UserProfiles singleton
                let uid = task.creator.as_ref().map(|c| &c.uid)?;
                let user_profiles = UserProfiles::as_ref(app);
                user_profiles.displayable_identifier_for_uid(UserUid::new(uid))
            }),
            ConversationOrTask::Conversation(_) => {
                AuthStateProvider::as_ref(app).get().username_for_display()
            }
        }
    }

    /// Grab the creator UID from the task, or from the auth state if it is a conversation
    pub fn creator_uid(&self, app: &AppContext) -> Option<String> {
        match self {
            ConversationOrTask::Task(task) => task.creator.as_ref().map(|c| c.uid.clone()),
            ConversationOrTask::Conversation(_) => AuthStateProvider::as_ref(app)
                .get()
                .user_id()
                .map(|uid| uid.to_string()),
        }
    }

    /// Returns the request usage for the task or conversation
    pub(super) fn request_usage(&self, app: &AppContext) -> Option<f32> {
        match self {
            ConversationOrTask::Task(task) => task.credits_used(),
            ConversationOrTask::Conversation(metadata) => {
                let history_model = BlocklistAIHistoryModel::as_ref(app);
                history_model
                    .conversation(&metadata.nav_data.id)
                    .map(|conv| conv.credits_spent())
                    .or_else(|| {
                        history_model
                            .get_conversation_metadata(&metadata.nav_data.id)
                            .and_then(|m| m.credits_spent)
                    })
            }
        }
    }

    /// Formats the request usage for display.
    pub fn display_request_usage(&self, app: &AppContext) -> Option<String> {
        self.request_usage(app).map(format_credits)
    }

    pub fn last_updated(&self) -> DateTime<Utc> {
        match self {
            ConversationOrTask::Task(task) => task.updated_at,
            ConversationOrTask::Conversation(metadata) => metadata.nav_data.last_updated.into(),
        }
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            ConversationOrTask::Task(task) => task.created_at,
            ConversationOrTask::Conversation(metadata) => metadata.nav_data.last_updated.into(),
        }
    }

    pub fn is_ambient_agent_conversation(&self) -> bool {
        matches!(self, ConversationOrTask::Task(_))
    }

    /// Returns the navigation data for local conversations, used for emitting the Navigate event.
    pub fn navigation_data(&self) -> Option<&ConversationNavigationData> {
        match self {
            ConversationOrTask::Task(_) => None,
            ConversationOrTask::Conversation(metadata) => Some(&metadata.nav_data),
        }
    }

    pub fn run_time(&self) -> Option<String> {
        match self {
            // TODO this should really be done server-side
            ConversationOrTask::Task(task) => {
                let Some(duration) = task.run_time() else {
                    return Some("Not started".to_string());
                };
                if duration.num_minutes() < 1 {
                    Some(format!("{} seconds", duration.num_seconds()))
                } else {
                    Some(format!("{} minutes", duration.num_minutes()))
                }
            }
            // Local conversations don't currently track run time
            ConversationOrTask::Conversation(_) => None,
        }
    }

    pub fn source(&self) -> Option<&AgentSource> {
        match self {
            ConversationOrTask::Task(task) => task.source.as_ref(),
            ConversationOrTask::Conversation(_) => Some(&AgentSource::Interactive),
        }
    }

    pub fn environment_id(&self) -> Option<&str> {
        match self {
            ConversationOrTask::Task(task) => task
                .agent_config_snapshot
                .as_ref()
                .and_then(|s| s.environment_id.as_deref()),
            ConversationOrTask::Conversation(_) => None,
        }
    }

    /// Resolve the effective execution harness for this run.
    pub fn harness(&self) -> Option<Harness> {
        match self {
            ConversationOrTask::Task(task) => {
                task.agent_config_snapshot.as_ref().and_then(|config| {
                    config
                        .harness
                        .as_ref()
                        .map(|h| h.harness_type)
                        .or(Some(Harness::Oz))
                })
            }
            ConversationOrTask::Conversation(_) => Some(Harness::Oz),
        }
    }

    /// Returns artifacts for the task or conversation.
    pub fn artifacts(&self, app: &AppContext) -> Vec<Artifact> {
        match self {
            ConversationOrTask::Task(task) => task.artifacts.clone(),
            ConversationOrTask::Conversation(metadata) => {
                let history_model = BlocklistAIHistoryModel::as_ref(app);
                history_model
                    .conversation(&metadata.nav_data.id)
                    .map(|conv| conv.artifacts().to_vec())
                    .or_else(|| {
                        history_model
                            .get_conversation_metadata(&metadata.nav_data.id)
                            .map(|m| m.artifacts.clone())
                    })
                    .unwrap_or_default()
            }
        }
    }

    pub fn get_session_status(&self) -> Option<SessionStatus> {
        match self {
            ConversationOrTask::Task(task) => {
                if task.session_id.is_some() {
                    Some(SessionStatus::Available)
                } else if (Utc::now() - task.created_at) > SESSION_EXPIRATION_TIME {
                    Some(SessionStatus::Expired)
                } else {
                    Some(SessionStatus::Unavailable)
                }
            }
            ConversationOrTask::Conversation(_) => None,
        }
    }

    /// Check if this item matches the current status filter.
    fn matches_status(&self, status_filter: &StatusFilter, app: &AppContext) -> bool {
        match status_filter {
            StatusFilter::All => true,
            StatusFilter::Working | StatusFilter::Done | StatusFilter::Failed => {
                self.display_status(app).status_filter() == *status_filter
            }
        }
    }

    /// Check if this item matches the artifact filter.
    fn matches_artifact(&self, artifact_filter: &ArtifactFilter, app: &AppContext) -> bool {
        artifacts_match_filter(&self.artifacts(app), artifact_filter)
    }

    /// Check if this item matches the harness filter.
    fn matches_harness(&self, harness_filter: &HarnessFilter) -> bool {
        match harness_filter {
            HarnessFilter::All => true,
            HarnessFilter::Specific(h) => self.harness() == Some(*h),
        }
    }

    /// Check if this item matches the owner and creator filters.
    fn matches_owner_and_creator(
        &self,
        owner_filter: &OwnerFilter,
        creator_filter: &CreatorFilter,
        app: &AppContext,
    ) -> bool {
        let current_user_id = AuthStateProvider::as_ref(app)
            .get()
            .user_id()
            .map(|uid| uid.as_string());

        // First check owner filter
        let passes_owner = match owner_filter {
            OwnerFilter::All => true,
            OwnerFilter::PersonalOnly => match self {
                ConversationOrTask::Task(_) => self.creator_uid(app) == current_user_id,
                // Local conversations are always owned by the current user
                ConversationOrTask::Conversation(_) => true,
            },
        };

        if !passes_owner {
            return false;
        }

        // We don't want to apply the creator filter if we are in the personal only view.
        if matches!(owner_filter, OwnerFilter::PersonalOnly) {
            return true;
        }

        // Then check creator filter (only relevant when owner is "All")
        match creator_filter {
            CreatorFilter::All => true,
            CreatorFilter::Specific { name, .. } => self.creator_name(app).as_ref() == Some(name),
        }
    }

    /// Returns the appropriate `WorkspaceAction` to dispatch when opening this item.
    /// This encapsulates the decision logic for opening ambient agent sessions vs
    /// navigating to local conversations.
    pub fn get_open_action(
        &self,
        restore_layout: Option<RestoreConversationLayout>,
    ) -> Option<WorkspaceAction> {
        match self {
            ConversationOrTask::Task(_) => None,
            ConversationOrTask::Conversation(metadata) => {
                let nav_data = &metadata.nav_data;
                Some(WorkspaceAction::RestoreOrNavigateToConversation {
                    conversation_id: nav_data.id,
                    window_id: nav_data.window_id,
                    pane_view_locator: nav_data.pane_view_locator,
                    terminal_view_id: nav_data.terminal_view_id,
                    restore_layout,
                })
            }
        }
    }
}

pub(crate) fn artifacts_match_filter(
    artifacts: &[Artifact],
    artifact_filter: &ArtifactFilter,
) -> bool {
    match artifact_filter {
        ArtifactFilter::All => true,
        ArtifactFilter::PullRequest => artifacts
            .iter()
            .any(|artifact| matches!(artifact, Artifact::PullRequest { .. })),
        ArtifactFilter::Plan => artifacts
            .iter()
            .any(|artifact| matches!(artifact, Artifact::Plan { .. })),
        ArtifactFilter::Screenshot => artifacts
            .iter()
            .any(|artifact| matches!(artifact, Artifact::Screenshot { .. })),
        ArtifactFilter::File => artifacts
            .iter()
            .any(|artifact| matches!(artifact, Artifact::File { .. })),
    }
}

/// This model serves as a unified interface for reading both local and ambient agent conversations
/// (i.e. conversations & tasks). The model is responsible for polling for new tasks and updating
/// its local state accordingly.
///
/// This model backs both the agent management view and the conversation list view.
pub struct AgentConversationsModel {
    /// A map of task IDs to agent tasks.
    tasks: HashMap<AmbientAgentTaskId, AmbientAgentTask>,
    /// A map of conversation IDs to local conversations.
    conversations: HashMap<AIConversationId, ConversationMetadata>,
    /// Set of view IDs actively consuming this model's data per window.
    /// OpenWarp:本地化后无轮询,仅作为 register_view_open/closed 的占位记录使用。
    active_data_consumers_per_window: HashMap<WindowId, HashSet<EntityId>>,
    /// Whether we have finished the initial task load
    has_finished_initial_load: bool,
    /// Task IDs that have been manually opened from the management page.
    /// These will appear in the conversation list even if their source is not user-initiated
    /// (and even after they have been closed).
    manually_opened_task_ids: HashSet<AmbientAgentTaskId>,
}

pub enum AgentConversationsModelEvent {
    /// Initial load of tasks completed.
    ConversationsLoaded,
    /// Existing task data may have been updated (e.g., state changes).
    TasksUpdated,
    /// Conversation status data was updated
    ConversationUpdated,
    /// Conversation artifacts were updated (plans, PRs, etc.)
    ConversationArtifactsUpdated { conversation_id: AIConversationId },
    /// A task was manually opened from the management page.
    TaskManuallyOpened,
}

impl Entity for AgentConversationsModel {
    type Event = AgentConversationsModelEvent;
}

impl SingletonEntity for AgentConversationsModel {}

impl AgentConversationsModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        // OpenWarp(本地化,Phase 3b-1 / Wave 6-6):AgentConversationsModel 原本负责轮询/探听
        // 远端 ambient agent tasks 与 conversation metadata。本地化场景下:
        //   - 不订阅任何事件
        //   - 无轮询子系统(Wave 6-6 物理删)
        //   - has_finished_initial_load 直接为 true,使 UI 查询以空集合返回
        // BYOP agent 本地运行不依赖该模型,零影响。
        Self {
            tasks: HashMap::new(),
            conversations: HashMap::new(),
            active_data_consumers_per_window: HashMap::new(),
            has_finished_initial_load: true,
            manually_opened_task_ids: HashSet::new(),
        }
    }

    pub fn is_loading(&self) -> bool {
        !self.has_finished_initial_load
    }

    /// Sync all conversations to the AgentConversationsModel.
    ///
    /// This function will loop through all active panes, recently closed panes, and historical
    /// conversations to construct a complete snapshot of conversations.
    pub fn sync_conversations(&mut self, ctx: &mut ModelContext<Self>) {
        if !FeatureFlag::InteractiveConversationManagementView.is_enabled() {
            return;
        }

        let nav_data_list = ConversationNavigationData::all_conversations(ctx);

        self.conversations.clear();
        for nav_data in nav_data_list {
            let conversation_id = nav_data.id;
            let metadata = ConversationMetadata { nav_data };
            self.conversations.insert(conversation_id, metadata);
        }

        ctx.emit(AgentConversationsModelEvent::ConversationsLoaded);
    }

    /// Called when a view that consumes this model's data becomes visible.
    /// Uses view_id to make registration idempotent.
    pub fn register_view_open(
        &mut self,
        window_id: WindowId,
        view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.active_data_consumers_per_window
            .entry(window_id)
            .or_default()
            .insert(view_id);
        self.sync_conversations(ctx);
    }

    /// Called when a view that consumes this model's data becomes hidden.
    /// Uses view_id to make unregistration idempotent.
    pub fn register_view_closed(
        &mut self,
        window_id: WindowId,
        view_id: EntityId,
        _ctx: &mut ModelContext<Self>,
    ) {
        if let Some(views) = self.active_data_consumers_per_window.get_mut(&window_id) {
            views.remove(&view_id);
            if views.is_empty() {
                self.active_data_consumers_per_window.remove(&window_id);
            }
        }
    }

    /// Returns true if we have tasks or local conversations in this view
    pub fn has_items(&self) -> bool {
        !self.tasks.is_empty() || !self.conversations.is_empty()
    }

    /// Returns an iterator over all ambient agent tasks.
    pub fn tasks_iter(&self) -> impl Iterator<Item = &AmbientAgentTask> {
        self.tasks.values()
    }

    /// Returns the local conversation ID represented by the given task, if this task and a
    /// conversation entry both point at the same underlying local run.
    ///
    /// We first match using the orchestration agent ID (task ID / run ID under v2), and fall back
    /// to the legacy conversation token for cases where the task only carries conversation identity
    /// through `conversation_id`.
    fn conversation_id_shadowed_by_task(
        task: &AmbientAgentTask,
        history_model: &BlocklistAIHistoryModel,
    ) -> Option<AIConversationId> {
        history_model
            .conversation_id_for_agent_id(&task.task_id.to_string())
            .or_else(|| {
                task.conversation_id.as_ref().and_then(|conversation_id| {
                    history_model.find_conversation_id_by_server_token(
                        &ServerConversationToken::new(conversation_id.clone()),
                    )
                })
            })
    }

    fn conversation_ids_shadowed_by_tasks(&self, app: &AppContext) -> HashSet<AIConversationId> {
        let history_model = BlocklistAIHistoryModel::as_ref(app);
        self.tasks
            .values()
            .filter_map(|task| Self::conversation_id_shadowed_by_task(task, history_model))
            .collect()
    }

    fn handle_history_event(
        &mut self,
        event: &BlocklistAIHistoryEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        if !FeatureFlag::InteractiveConversationManagementView.is_enabled() {
            return;
        }
        match event {
            // Events that affect conversation navigation data - need full sync
            BlocklistAIHistoryEvent::StartedNewConversation { .. }
            | BlocklistAIHistoryEvent::SetActiveConversation { .. }
            | BlocklistAIHistoryEvent::AppendedExchange { .. }
            | BlocklistAIHistoryEvent::SplitConversation { .. }
            | BlocklistAIHistoryEvent::RestoredConversations { .. }
            | BlocklistAIHistoryEvent::RemoveConversation { .. }
            | BlocklistAIHistoryEvent::DeletedConversation { .. }
            | BlocklistAIHistoryEvent::ClearedConversationsInTerminalView { .. }
            | BlocklistAIHistoryEvent::ClearedActiveConversation { .. } => {
                self.sync_conversations(ctx);
            }

            // Status changes - just trigger re-render since status is looked up at render time
            BlocklistAIHistoryEvent::UpdatedConversationStatus { .. } => {
                ctx.emit(AgentConversationsModelEvent::ConversationUpdated);
            }

            // Artifact changes - sync live artifacts into the cached task and notify.
            BlocklistAIHistoryEvent::UpdatedConversationArtifacts {
                conversation_id, ..
            } => {
                let conversation = BlocklistAIHistoryModel::as_ref(ctx).conversation(conversation_id);
                let Some(conversation) = conversation else {
                    return;
                };

                let task_id = conversation.task_id();
                if let Some(task_id) = task_id {
                    // If the conversation is associated with a task, update the saved task
                    // with live artifacts.
                    if let Some(task) = self.tasks.get_mut(&task_id) {
                        task.artifacts = conversation.artifacts().to_vec();
                        ctx.emit(AgentConversationsModelEvent::TasksUpdated);
                    }
                }
                ctx.emit(AgentConversationsModelEvent::ConversationArtifactsUpdated {
                    conversation_id: *conversation_id,
                });
            }

            // Task/exchange-level changes that don't affect conversation navigation.
            BlocklistAIHistoryEvent::CreatedSubtask { .. }
            | BlocklistAIHistoryEvent::UpgradedTask { .. }
            | BlocklistAIHistoryEvent::ReassignedExchange { .. }
            | BlocklistAIHistoryEvent::UpdatedTodoList { .. }
            | BlocklistAIHistoryEvent::UpdatedAutoexecuteOverride { .. }
            | BlocklistAIHistoryEvent::UpdatedConversationMetadata { .. }
            // UpdatedStreamingExchange covers streaming and other exchange-level updates but
            // doesn't change any ConversationNavigationData fields (title comes from
            // UpdateTaskDescription, last_updated uses exchange.start_time which is set at append time).
            | BlocklistAIHistoryEvent::UpdatedStreamingExchange { .. }
            | BlocklistAIHistoryEvent::ConversationAgentIdAssigned { .. }
            => {}
        }
    }

    /// Returns an iterator with all tasks and conversations with filters applied, sorted with the
    /// most recently updated items first.
    pub fn get_tasks_and_conversations(
        &self,
        filters: &AgentManagementFilters,
        app: &AppContext,
    ) -> impl Iterator<Item = ConversationOrTask<'_>> {
        let conversation_ids_shadowed_by_tasks = self.conversation_ids_shadowed_by_tasks(app);
        let owner_creator_filter = move |t: &ConversationOrTask| {
            t.matches_owner_and_creator(&filters.owners, &filters.creator, app)
        };

        let status_filter = move |t: &ConversationOrTask| t.matches_status(&filters.status, app);

        let source_filter = move |t: &ConversationOrTask| match &filters.source {
            SourceFilter::All => true,
            SourceFilter::Specific(s) => t.source() == Some(s),
        };

        let now = Utc::now();
        let created_cutoff = match filters.created_on {
            CreatedOnFilter::All => None,
            CreatedOnFilter::Last24Hours => Some(now - chrono::Duration::hours(24)),
            CreatedOnFilter::Past3Days => Some(now - chrono::Duration::days(3)),
            CreatedOnFilter::LastWeek => Some(now - chrono::Duration::days(7)),
        };

        let created_on_filter = move |t: &ConversationOrTask| match created_cutoff {
            Some(cutoff) => t.created_at() >= cutoff,
            None => true,
        };

        let artifact_filter_value = filters.artifact;
        let artifact_filter =
            move |t: &ConversationOrTask| t.matches_artifact(&artifact_filter_value, app);

        let environment_filter = move |t: &ConversationOrTask| match &filters.environment {
            EnvironmentFilter::All => true,
            EnvironmentFilter::NoEnvironment => t.environment_id().is_none(),
            EnvironmentFilter::Specific(id) => t.environment_id() == Some(id.as_str()),
        };

        let harness_filter_value = filters.harness;
        let harness_filter = move |t: &ConversationOrTask| t.matches_harness(&harness_filter_value);

        let tasks_iter = self.tasks.values().map(ConversationOrTask::Task);
        let conversations_iter = self
            .conversations
            .values()
            .filter(move |conversation| {
                // Prefer rendering the task row when both representations exist for the same local
                // run. Task entries preserve task-specific affordances like source, runtime,
                // session status, and ambient-session open behavior that the conversation row
                // cannot express.
                !conversation_ids_shadowed_by_tasks.contains(&conversation.nav_data.id)
            })
            .map(ConversationOrTask::Conversation);

        tasks_iter
            .chain(conversations_iter)
            .filter(owner_creator_filter)
            .filter(status_filter)
            .filter(source_filter)
            .filter(created_on_filter)
            .filter(artifact_filter)
            .filter(environment_filter)
            .filter(harness_filter)
            .sorted_by(|a, b| b.last_updated().cmp(&a.last_updated()))
    }

    /// Get a task by its task ID
    pub fn get_task(&self, task_id: &AmbientAgentTaskId) -> Option<ConversationOrTask<'_>> {
        self.tasks.get(task_id).map(ConversationOrTask::Task)
    }

    /// Get raw task data by task ID
    pub fn get_task_data(&self, task_id: &AmbientAgentTaskId) -> Option<AmbientAgentTask> {
        self.tasks.get(task_id).cloned()
    }

    /// 按 task ID 读取本地已缓存的 task 数据。
    ///
    /// OpenWarp 不再向云端补取 ambient agent task。调用方如果恢复了旧布局但本地模型没有
    /// 对应 task,这里返回 `None`,由现有面板降级路径处理。
    pub fn get_or_async_fetch_task_data(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> Option<AmbientAgentTask> {
        self.tasks.get(task_id).cloned()
    }

    /// Get a conversation by its AIConversationId
    pub fn get_conversation(
        &self,
        conversation_id: &AIConversationId,
    ) -> Option<ConversationOrTask<'_>> {
        self.conversations
            .get(conversation_id)
            .map(ConversationOrTask::Conversation)
    }

    /// Returns all (name, uid) pairs for creators of tasks in the model.
    ///
    /// We use this function to populate the available creator filter list
    /// based on the tasks we have.
    pub fn get_all_creators(&self, app: &AppContext) -> Vec<(String, String)> {
        let mut creators: Vec<(String, String)> = self
            .tasks
            .values()
            .filter_map(|t| {
                let wrapper = ConversationOrTask::Task(t);
                let name = wrapper.creator_name(app)?;
                let uid = wrapper.creator_uid(app)?;
                Some((name, uid))
            })
            .collect();

        // Include the current user since they may have local conversations
        let auth_state = AuthStateProvider::as_ref(app).get();
        if let (Some(name), Some(uid)) = (auth_state.display_name(), auth_state.user_id()) {
            creators.push((name, uid.to_string()));
        }

        creators.sort_by(|a, b| a.0.cmp(&b.0));
        creators.dedup_by(|a, b| a.0 == b.0);

        creators
    }

    pub fn mark_task_as_manually_opened(
        &mut self,
        task_id: AmbientAgentTaskId,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.manually_opened_task_ids.insert(task_id) {
            ctx.emit(AgentConversationsModelEvent::TaskManuallyOpened);
        }
    }

    pub fn is_task_manually_opened(&self, task_id: &AmbientAgentTaskId) -> bool {
        self.manually_opened_task_ids.contains(task_id)
    }

    /// Clears all stored conversation and task data in memory.
    /// This is used when logging out to ensure no conversation history persists across users.
    pub(crate) fn reset(&mut self) {
        self.tasks.clear();
        self.conversations.clear();
        self.active_data_consumers_per_window.clear();
        self.manually_opened_task_ids.clear();
        // Reset the initial load flag so that we can retry the initial sync with the new logged in user
        self.has_finished_initial_load = false;
    }
}

#[cfg(test)]
#[path = "agent_conversations_model_tests.rs"]
mod tests;
