//! Onboarding-specific AI types and conversions.

use ai::LLMId;
use onboarding::slides::OnboardingModelInfo;
use warp_core::ui::icons::Icon;

use super::llms::{LLMInfo, LLMPreferences};

impl From<&LLMInfo> for OnboardingModelInfo {
    fn from(llm: &LLMInfo) -> Self {
        Self {
            id: llm.id.clone(),
            title: llm.display_name.clone(),
            icon: llm.provider.icon().unwrap_or(Icon::Oz),
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
