//! Onboarding-specific AI types and conversions.

use ai::LLMId;
use onboarding::slides::OnboardingModelInfo;
use onboarding::OnboardingAuthState;
use warp_core::ui::icons::Icon;
use warpui::{AppContext, SingletonEntity};

use crate::auth::AuthStateProvider;
use crate::experiments::FreeTierDefaultModel;
use crate::workspaces::user_workspaces::UserWorkspaces;

use super::llms::{DisableReason, LLMInfo, LLMPreferences};

/// mirrors server-side model ids
const AUTO_OPEN_LLM_ID: &str = "auto-open";
const AUTO_COST_EFFICIENT_LLM_ID: &str = "auto-efficient";

impl From<&LLMInfo> for OnboardingModelInfo {
    fn from(llm: &LLMInfo) -> Self {
        Self {
            id: llm.id.clone(),
            title: llm.display_name.clone(),
            icon: llm.provider.icon().unwrap_or(Icon::Oz),
            requires_upgrade: matches!(llm.disable_reason, Some(DisableReason::RequiresUpgrade)),
            is_default: false,
        }
    }
}

pub fn build_onboarding_models(prefs: &LLMPreferences) -> (Vec<OnboardingModelInfo>, LLMId) {
    let default_id = prefs.get_default_base_model().id.clone();
    let models: Vec<OnboardingModelInfo> = prefs
        .get_base_llm_choices_for_agent_mode()
        .map(|llm| {
            let mut info = OnboardingModelInfo::from(llm);
            info.is_default = info.id == default_id;
            info
        })
        .collect();
    (models, default_id)
}

pub fn apply_free_tier_default_model_override(
    models: &mut [OnboardingModelInfo],
    server_default_id: LLMId,
    ctx: &mut AppContext,
) -> LLMId {
    // server only gives back cost-efficient as a default if you're on a free or no plan
    // if you ARE on some sort of plan... we should respect what the server says
    if server_default_id != LLMId::from(AUTO_COST_EFFICIENT_LLM_ID) {
        return server_default_id;
    }
    let auto_open_id = LLMId::from(AUTO_OPEN_LLM_ID);
    let auto_open_available = models.iter().any(|m| m.id == auto_open_id);
    if !auto_open_available || !FreeTierDefaultModel::should_default_to_auto_open(ctx) {
        return server_default_id;
    }
    for m in models.iter_mut() {
        m.is_default = m.id == auto_open_id;
    }
    auto_open_id
}

pub fn current_onboarding_auth_state(ctx: &AppContext) -> OnboardingAuthState {
    let auth_state = AuthStateProvider::as_ref(ctx).get();
    if auth_state.is_anonymous_or_logged_out() {
        return OnboardingAuthState::LoggedOut;
    }
    let is_on_paid_plan = UserWorkspaces::as_ref(ctx)
        .current_workspace()
        .map(|w| w.billing_metadata.is_user_on_paid_plan())
        .unwrap_or(false);
    if is_on_paid_plan {
        OnboardingAuthState::PayingUser
    } else {
        OnboardingAuthState::FreeUser
    }
}
