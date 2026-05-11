//! Settings for cloud agent functionality.
//!
//! This module contains user-specific settings for cloud agent features,
//! such as remembering the last selected environment.

use std::collections::HashMap;

use settings::{macros::define_settings_group, Setting as _, SupportedPlatforms, SyncToCloud};
use warp_cli::agent::Harness;

use crate::server::ids::SyncId;

define_settings_group!(CloudAgentSettings, settings: [
    last_selected_environment_id: LastSelectedEnvironmentId {
        type: Option<SyncId>,
        default: None,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },
    harness_auth_ftux_completed: HarnessAuthFtuxCompleted {
        type: HashMap<String, bool>,
        default: HashMap::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },
    last_selected_harness: LastSelectedHarness {
        type: Option<String>,
        default: None,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },
    last_selected_host: LastSelectedHost {
        type: Option<String>,
        default: None,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },
    last_selected_harness_model: LastSelectedHarnessModel {
        type: HashMap<String, String>,
        default: HashMap::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },
    last_selected_auth_secret: LastSelectedAuthSecret {
        type: HashMap<String, String>,
        default: HashMap::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    }
]);

impl CloudAgentSettings {
    pub fn is_harness_auth_ftux_completed(&self, harness: Harness) -> bool {
        self.harness_auth_ftux_completed
            .value()
            .get(harness.config_name())
            .copied()
            .unwrap_or(false)
    }

    pub fn mark_harness_auth_ftux_completed(
        &mut self,
        harness: Harness,
        ctx: &mut warpui::ModelContext<Self>,
    ) {
        let mut map = self.harness_auth_ftux_completed.value().clone();
        map.insert(harness.config_name().to_string(), true);
        let _ = self.harness_auth_ftux_completed.set_value(map, ctx);
    }
}
