//! Logic to convert from / to [`ServerExperiment`].

use std::fmt::{Display, Formatter};

use anyhow::{Ok, Result};

use super::ServerExperiment;

impl Display for ServerExperiment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::DisableAgentModeExperiment => "DISABLE_AGENT_MODE_EXPERIMENT",
            Self::EnvVarsEarlyAccessExperiment => "ENV_VARS_EARLY_ACCESS_EXPERIMENT",
            Self::AgentModeAnalyticsExperiment => "AGENT_MODE_ANALYTICS_EXPERIMENT",
            Self::WindowsLaunchExperiment => "WINDOWS_LAUNCH_EXPERIMENT",
            Self::TmuxSshWarpificationControl => "TMUX_SSH_WARPIFICATION_CONTROL",
            Self::TmuxSshWarpificationExperiment => "TMUX_SSH_WARPIFICATION_EXPERIMENT",
            Self::SuggestedCodeDiffsControl => "SUGGESTED_CODE_DIFFS_CONTROL",
            Self::SuggestedCodeDiffsExperiment => "SUGGESTED_CODE_DIFFS_EXPERIMENT",
            Self::BuildPlanAutoReloadControl => "BUILD_PLAN_AUTO_RELOAD_CONTROL",
            Self::BuildPlanAutoReloadBannerToggle => "BUILD_PLAN_AUTO_RELOAD_BANNER_TOGGLE",
            Self::BuildPlanAutoReloadPostPurchaseModal => {
                "BUILD_PLAN_AUTO_RELOAD_POST_PURCHASE_MODAL"
            }
            Self::PromptSuggestionsViaMaaControl => "PROMPT_SUGGESTIONS_VIA_MAA_CONTROL",
            Self::PromptSuggestionsViaMaaExperiment => "PROMPT_SUGGESTIONS_VIA_MAA_EXPERIMENT",
            Self::PromptSuggestionsViaMaaOutOfBandExperiment => {
                "PROMPT_SUGGESTIONS_VIA_MAA_OOB_EXPERIMENT"
            }
            Self::OzMultiHarnessControl => "OZ_MULTI_HARNESS_CONTROL",
            Self::OzMultiHarnessExperiment => "OZ_MULTI_HARNESS_EXPERIMENT",
            #[cfg(test)]
            Self::TestExperiment => "TEST_EXPERIMENT",
        };
        write!(f, "{str}")
    }
}

impl ServerExperiment {
    pub fn from_string(s: String) -> Result<Self> {
        match s.as_str() {
            "DISABLE_AGENT_MODE_EXPERIMENT" => Ok(Self::DisableAgentModeExperiment),
            "ENV_VARS_EARLY_ACCESS_EXPERIMENT" => Ok(Self::EnvVarsEarlyAccessExperiment),
            "AGENT_MODE_ANALYTICS_EXPERIMENT" => Ok(Self::AgentModeAnalyticsExperiment),
            "WINDOWS_LAUNCH_EXPERIMENT" => Ok(Self::WindowsLaunchExperiment),
            "TMUX_SSH_WARPIFICATION_CONTROL" => Ok(Self::TmuxSshWarpificationControl),
            "TMUX_SSH_WARPIFICATION_EXPERIMENT" => Ok(Self::TmuxSshWarpificationExperiment),
            "SUGGESTED_CODE_DIFFS_CONTROL" => Ok(Self::SuggestedCodeDiffsControl),
            "SUGGESTED_CODE_DIFFS_EXPERIMENT" => Ok(Self::SuggestedCodeDiffsExperiment),
            "BUILD_PLAN_AUTO_RELOAD_CONTROL" => Ok(Self::BuildPlanAutoReloadControl),
            "BUILD_PLAN_AUTO_RELOAD_BANNER_TOGGLE" => Ok(Self::BuildPlanAutoReloadBannerToggle),
            "BUILD_PLAN_AUTO_RELOAD_POST_PURCHASE_MODAL" => {
                Ok(Self::BuildPlanAutoReloadPostPurchaseModal)
            }
            "PROMPT_SUGGESTIONS_VIA_MAA_CONTROL" => Ok(Self::PromptSuggestionsViaMaaControl),
            "PROMPT_SUGGESTIONS_VIA_MAA_EXPERIMENT" => Ok(Self::PromptSuggestionsViaMaaExperiment),
            "OZ_MULTI_HARNESS_CONTROL" => Ok(Self::OzMultiHarnessControl),
            "OZ_MULTI_HARNESS_EXPERIMENT" => Ok(Self::OzMultiHarnessExperiment),
            s => Err(anyhow::anyhow!(
                "String doesn't match any server experiment variant {s}"
            )),
        }
    }
}
