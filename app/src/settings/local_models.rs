use ::ai::local_models::LocalModelProvider;
use settings::{macros::define_settings_group, Setting, SupportedPlatforms, SyncToCloud};

define_settings_group!(LocalModelSettings, settings: [
    enabled: LocalModelsEnabled {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.local_models.enabled",
        description: "Whether local model providers are enabled.",
    },
    provider: LocalModelProviderSetting {
        type: String,
        default: "none".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.local_models.provider",
        description: "The selected local model provider (none, ollama, lmstudio).",
    },
    ollama_base_url: LocalModelsOllamaBaseUrl {
        type: String,
        default: "http://localhost:11434".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.local_models.ollama_url",
        description: "Base URL for Ollama.",
    },
    lmstudio_base_url: LocalModelsLmStudioBaseUrl {
        type: String,
        default: "http://localhost:1234/v1".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.local_models.lmstudio_url",
        description: "Base URL for LM Studio.",
    },
    selected_model: LocalModelsSelectedModel {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "agents.local_models.selected_model",
        description: "Selected local model name.",
    },
]);

impl LocalModelSettings {
    pub fn selected_provider(&self) -> LocalModelProvider {
        LocalModelProvider::from_storage_value(self.provider.value())
    }

    pub fn is_enabled_and_configured(&self) -> bool {
        *self.enabled
            && self.selected_provider() != LocalModelProvider::None
            && self.selected_model_name().is_some()
    }

    pub fn selected_base_url(&self) -> Option<String> {
        match self.selected_provider() {
            LocalModelProvider::None => None,
            LocalModelProvider::Ollama => Some(self.ollama_base_url.value().clone()),
            LocalModelProvider::LMStudio => Some(self.lmstudio_base_url.value().clone()),
        }
    }

    pub fn selected_model_name(&self) -> Option<String> {
        let model = self.selected_model.value().trim();
        if model.is_empty() {
            None
        } else {
            Some(model.to_string())
        }
    }

    pub fn provider_storage_value(provider: LocalModelProvider) -> String {
        provider.as_storage_value().to_string()
    }
}
