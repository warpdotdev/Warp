use serde::{Deserialize, Serialize};

use settings::{macros::define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud};

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "UI language preference.", rename_all = "snake_case")]
pub enum UILanguage {
    #[default]
    English,
    ChineseSimplified,
}

impl UILanguage {
    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::ChineseSimplified => "中文（简体）",
        }
    }
}

define_settings_group!(LanguageSettings, settings: [
    ui_language: UILanguageSetting {
        type: UILanguage,
        default: UILanguage::English,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        storage_key: "UILanguage",
        toml_path: "appearance.language.ui_language",
        description: "The UI language preference.",
    },
]);
