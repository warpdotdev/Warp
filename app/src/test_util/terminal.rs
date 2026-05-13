use repo_metadata::repositories::DetectedRepositories;
#[cfg(feature = "local_fs")]
use repo_metadata::RepoMetadataModel;
use std::sync::Arc;
use warp_core::ui::appearance::Appearance;

use crate::ai::agent_conversations_model::AgentConversationsModel;
use crate::ai::ambient_agents::github_auth_notifier::GitHubAuthNotifier;
use crate::ai::document::ai_document_model::AIDocumentModel;
use crate::ai::mcp::{
    gallery::MCPGalleryManager, templatable_manager::TemplatableMCPServerManager,
};
use crate::ai::skills::SkillManager;
use crate::code_review::git_status_update::GitStatusUpdateModel;
use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;
use crate::warp_managed_paths_watcher::WarpManagedPathsWatcher;
use warpui::{platform::WindowStyle, App, ViewHandle, WindowId};
use watcher::HomeDirectoryWatcher;

use super::settings::initialize_settings_for_tests;
use crate::ai::blocklist::BlocklistAIPermissions;
use crate::ai::blocklist::SerializedBlockListItem;
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::llms::LLMPreferences;
use crate::ai::restored_conversations::RestoredAgentConversations;
use crate::auth::AuthManager;
use crate::auth::AuthStateProvider;
use crate::changelog_model::ChangelogModel;
use crate::pricing::PricingInfoModel;
use crate::suggestions::ignored_suggestions_model::IgnoredSuggestionsModel;
use crate::terminal::view::inline_banner::ByoLlmAuthBannerSessionState;
use crate::undo_close::UndoCloseStack;
use crate::workspace::{OneTimeModalModel, WorkspaceRegistry};
use crate::{
    ai::{blocklist::BlocklistAIHistoryModel, AIRequestUsageModel},
    cloud_object::model::persistence::ObjectStoreModel,
    cloud_object::update_manager::UpdateManager,
    context_chips::prompt::Prompt,
    experiments,
    network::NetworkStatus,
    search::files::model::FileSearchModel,
    settings::PrivacySettings,
    settings_view::keybindings::KeybindingChangedNotifier,
    system::SystemInfo,
    system::SystemStats,
    terminal::{
        alt_screen_reporting::AltScreenReporting, keys::TerminalKeybindings,
        resizable_data::ResizableData, History, TerminalView,
    },
    workflows::local_workflows::LocalWorkflows,
    workspace::{sync_inputs::SyncedInputState, ActiveSession},
    workspaces::user_workspaces::UserWorkspaces,
};
use repo_metadata::watcher::DirectoryWatcher;

/// Initializes all of the necessary models to use a terminal view.
pub fn initialize_app_for_terminal_view(app: &mut App) {
    initialize_settings_for_tests(app);

    app.add_singleton_model(|_| ChangelogModel::new(Arc::new(http_client::Client::new())));
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(|_| Prompt::mock());
    app.add_singleton_model(ObjectStoreModel::mock);
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(MCPGalleryManager::new);
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(|_ctx| SyncedInputState::mock());
    app.add_singleton_model(|_| ResizableData::default());
    app.add_singleton_model(LocalWorkflows::new);
    app.add_singleton_model(|_| History::default());
    app.add_singleton_model(|_| BlocklistAIHistoryModel::new_for_test());
    app.add_singleton_model(|_| CLIAgentSessionsModel::new());
    app.add_singleton_model(BlocklistAIPermissions::new);
    app.add_singleton_model(UndoCloseStack::new);

    app.add_singleton_model(AIRequestUsageModel::new_for_test);
    app.add_singleton_model(|_| KeybindingChangedNotifier::new());
    app.add_singleton_model(TerminalKeybindings::new);
    app.add_singleton_model(|_| ActiveSession::default());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(LLMPreferences::new);
    app.add_singleton_model(DirectoryWatcher::new);
    app.add_singleton_model(|_| DetectedRepositories::default());
    #[cfg(feature = "local_fs")]
    app.add_singleton_model(RepoMetadataModel::new);
    app.add_singleton_model(FileSearchModel::new);
    app.add_singleton_model(|_| GitStatusUpdateModel::new());
    // OpenWarp:RepoOutlines 已删除,不再注册。
    app.add_singleton_model(HomeDirectoryWatcher::new_for_test);
    app.add_singleton_model(WarpManagedPathsWatcher::new_for_testing);
    app.add_singleton_model(SkillManager::new);

    app.add_singleton_model(|_| TemplatableMCPServerManager::default());
    app.add_singleton_model(|ctx| {
        AIExecutionProfilesModel::new(&crate::LaunchMode::new_for_unit_test(), ctx)
    });
    #[cfg(feature = "voice_input")]
    app.add_singleton_model(voice_input::VoiceInput::new);

    #[cfg(not(target_family = "wasm"))]
    app.add_singleton_model(SystemInfo::new);

    app.add_singleton_model(|_| RestoredAgentConversations::default());
    app.add_singleton_model(OneTimeModalModel::new);
    app.add_singleton_model(|_| WorkspaceRegistry::new());
    app.add_singleton_model(|_| IgnoredSuggestionsModel::new(vec![]));
    app.add_singleton_model(|_| PricingInfoModel::new());
    app.add_singleton_model(AIDocumentModel::new);
    app.add_singleton_model(ByoLlmAuthBannerSessionState::new);
    app.add_singleton_model(|_| GitHubAuthNotifier::new());
    app.add_singleton_model(AgentConversationsModel::new);

    app.update(experiments::init);
    AltScreenReporting::register(app);
}

/// Creates a window in `app` with a [`TerminalView`] as the root view.
/// Returns the handle to that terminal view.
pub fn add_window_with_terminal(
    app: &mut App,
    restored_blocks: Option<&[SerializedBlockListItem]>,
) -> ViewHandle<TerminalView> {
    add_window_with_id_and_terminal(app, restored_blocks).1
}

/// Creates a window in `app` with a [`TerminalView`] as the root view.
/// Returns the WindowID and the handle to that terminal view.
pub fn add_window_with_id_and_terminal(
    app: &mut App,
    restored_blocks: Option<&[SerializedBlockListItem]>,
) -> (WindowId, ViewHandle<TerminalView>) {
    let tips_model = app.add_model(|_| Default::default());
    app.add_window(WindowStyle::NotStealFocus, |ctx| {
        TerminalView::new_for_test(tips_model, restored_blocks, ctx)
    })
}
