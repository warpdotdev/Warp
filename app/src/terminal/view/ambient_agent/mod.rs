mod block;
// OpenWarp Wave 7-2:`first_time_setup` 随 cloud ambient agent UI 物理删。
mod footer;
mod harness_selector;
mod host_selector;
mod loading_screen;
mod model;
mod model_selector;
mod progress;
mod progress_ui_state;
mod tips;
mod view_impl;

pub use block::*;
pub use footer::{render_error_footer, render_loading_footer};
pub use harness_selector::{HarnessSelector, HarnessSelectorAction, HarnessSelectorEvent};
pub use host_selector::{
    Host, HostSelector, HostSelectorAction, HostSelectorEvent, NakedHeaderButtonTheme,
};
pub use loading_screen::{render_cloud_mode_error_screen, render_cloud_mode_loading_screen};
pub use model::{AgentProgress, AmbientAgentViewModel, AmbientAgentViewModelEvent, Status};
pub use model_selector::{ModelSelector, ModelSelectorAction, ModelSelectorEvent};
pub use progress::{render_progress, ProgressProps, ProgressStep, ProgressStepState};
pub use progress_ui_state::AmbientAgentProgressUIState;
pub use tips::{get_cloud_mode_tips, CloudModeTip};
use warp_core::features::FeatureFlag;

use crate::ai::blocklist::agent_view::{AgentViewController, AgentViewState};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use warpui::{AppContext, ModelHandle, SingletonEntity};

/// Returns `true` when a cloud agent shared session is ready but no agent exchange has been
/// received yet. In this state, we hide the interactive input and render a loading footer
/// instead.
pub fn is_cloud_agent_pre_first_exchange(
    ambient_agent_view_model: &ModelHandle<AmbientAgentViewModel>,
    agent_view_controller: &ModelHandle<AgentViewController>,
    app: &AppContext,
) -> bool {
    if !(false && FeatureFlag::AgentView.is_enabled()) {
        return false;
    }

    if !matches!(
        ambient_agent_view_model.as_ref(app).status(),
        Status::AgentRunning
    ) {
        return false;
    }

    let agent_view_state = agent_view_controller.as_ref(app).agent_view_state().clone();
    let AgentViewState::Active {
        conversation_id,
        origin,
        ..
    } = agent_view_state
    else {
        return false;
    };

    if !origin.is_cloud_agent() {
        return false;
    }

    // For non-oz harness runs, there is no Oz `AppendedExchange` to key off of, so we also
    // exit the pre-first-exchange phase when the harness CLI (e.g. `claude`, `gemini`) has
    // been detected. See `mark_harness_command_started`.
    if ambient_agent_view_model
        .as_ref(app)
        .harness_command_started()
    {
        return false;
    }

    BlocklistAIHistoryModel::as_ref(app)
        .conversation(&conversation_id)
        .is_some_and(|conversation| conversation.exchange_count() == 0)
}
