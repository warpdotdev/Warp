use settings::macros::implement_setting_for_enum;
use settings::{define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud};
use warp_i18n::Locale;

implement_setting_for_enum!(
    LocaleSetting,
    LocaleSettings,
    SupportedPlatforms::DESKTOP,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "general.locale",
    description: "The display language for the Warp UI.",
);

define_settings_group!(LocaleSettings, settings: [ locale: LocaleSetting ]);
