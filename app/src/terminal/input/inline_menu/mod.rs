//! Generic inline menu view for rendering search results with selection and navigation.
mod message_bar;
mod message_provider;
mod model;
pub(crate) mod positioning;
pub mod styles;
mod view;

use super::{InputSuggestionsMode, UserQueryMenuAction};
use serde::{Deserialize, Serialize};

pub use message_bar::{InlineMenuMessageArgs, InlineMenuMessageBarArgs};
pub use message_provider::{default_navigation_message_items, InlineMenuMessageProvider};
pub use model::{InlineMenuModel, InlineMenuModelEvent, InlineMenuTabConfig};
pub use positioning::InlineMenuPositioner;
pub use view::{
    DetailsRenderConfig, InlineMenuAction, InlineMenuClickBehavior, InlineMenuEvent,
    InlineMenuHeaderConfig, InlineMenuRowAction, InlineMenuView,
};

/// Identifies a specific inline menu type.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Identifies a specific inline menu.",
    rename_all = "snake_case"
)]
pub enum InlineMenuType {
    SlashCommands,
    ModelSelector,
    ConversationMenu,
    ProfileSelector,
    PromptsMenu,
    SkillMenu,
    UserQueryMenu,
    RewindMenu,
    InlineHistoryMenu,
    IndexedReposMenu,
    PlanMenu,
}

impl InlineMenuType {
    fn display_label(&self) -> &'static str {
        match self {
            Self::SlashCommands => "/Commands",
            Self::ModelSelector => "/Model",
            Self::ConversationMenu => "/Conversations",
            Self::ProfileSelector => "/Profiles",
            Self::PromptsMenu => "/Prompts",
            Self::SkillMenu => "/Skills",
            Self::UserQueryMenu => "/Fork",
            Self::RewindMenu => "/Rewind",
            Self::InlineHistoryMenu => "History",
            Self::IndexedReposMenu => "/Repos",
            Self::PlanMenu => "/Plans",
        }
    }

    pub(crate) fn from_suggestions_mode(mode: &InputSuggestionsMode) -> Option<Self> {
        match mode {
            InputSuggestionsMode::SlashCommands => Some(Self::SlashCommands),
            InputSuggestionsMode::ModelSelector => Some(Self::ModelSelector),
            InputSuggestionsMode::ConversationMenu => Some(Self::ConversationMenu),
            InputSuggestionsMode::ProfileSelector => Some(Self::ProfileSelector),
            InputSuggestionsMode::PromptsMenu => Some(Self::PromptsMenu),
            InputSuggestionsMode::SkillMenu => Some(Self::SkillMenu),
            InputSuggestionsMode::UserQueryMenu {
                action: UserQueryMenuAction::ForkFrom,
                ..
            } => Some(Self::UserQueryMenu),
            InputSuggestionsMode::UserQueryMenu {
                action: UserQueryMenuAction::Rewind,
                ..
            } => Some(Self::RewindMenu),
            InputSuggestionsMode::InlineHistoryMenu { .. } => Some(Self::InlineHistoryMenu),
            InputSuggestionsMode::IndexedReposMenu => Some(Self::IndexedReposMenu),
            InputSuggestionsMode::PlanMenu { .. } => Some(Self::PlanMenu),
            InputSuggestionsMode::Closed
            | InputSuggestionsMode::HistoryUp { .. }
            | InputSuggestionsMode::CompletionSuggestions { .. }
            | InputSuggestionsMode::StaticWorkflowEnumSuggestions { .. }
            | InputSuggestionsMode::DynamicWorkflowEnumSuggestions { .. }
            | InputSuggestionsMode::AIContextMenu { .. } => None,
        }
    }
}
