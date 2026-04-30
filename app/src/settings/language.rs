use i18n::Language;
use settings::{macros::define_settings_group, Setting, SupportedPlatforms, SyncToCloud};

define_settings_group!(LanguageSettings, settings: [
    language: LanguageState {
        type: Language,
        default: Language::English,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        storage_key: "Language",
        toml_path: "appearance.language",
        description: "The display language for Warp's interface.",
    },
]);

impl LanguageSettings {
    /// Returns the current language value.
    pub fn current_language(&self) -> Language {
        *self.language.value()
    }
}
