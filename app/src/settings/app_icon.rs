use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use settings_value::SettingsValue;
use warp_core::settings::{macros::define_settings_group, SupportedPlatforms, SyncToCloud};

/// The app icon to use (mac-only).
///
/// IMPORTANT NOTE: If you add a new icon, you will need to update the logic in WarpDockTilePlugin.m
/// to read the new icon and also add the icon to app/DockTilePlugin/Resources.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Sequence, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(
    description = "The app icon displayed in the dock.",
    rename_all = "snake_case"
)]
pub enum AppIcon {
    /// Warper default icon.
    #[default]
    #[serde(
        alias = "aurora",
        alias = "classic1",
        alias = "classic_1",
        alias = "classic2",
        alias = "classic_2",
        alias = "classic3",
        alias = "classic_3",
        alias = "comets",
        alias = "cow",
        alias = "glasssky",
        alias = "glass_sky",
        alias = "glitch",
        alias = "glow",
        alias = "holographic",
        alias = "mono",
        alias = "neon",
        alias = "original",
        alias = "starburst",
        alias = "sticker",
        alias = "warpone",
        alias = "warp_one"
    )]
    #[schemars(description = "Default")]
    Default,
    #[schemars(description = "Beaver")]
    Beaver,
    #[schemars(description = "Classic")]
    Classic,
    #[schemars(description = "Dark")]
    Dark,
    #[schemars(description = "Grunge")]
    Grunge,
    #[schemars(description = "Light")]
    Light,
    #[schemars(description = "Space")]
    Space,
    #[schemars(description = "Swiss")]
    Swiss,
    #[schemars(description = "Vostok")]
    Vostok,
}

impl std::fmt::Display for AppIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match &self {
            AppIcon::Default => "Default",
            AppIcon::Beaver => "Beaver",
            AppIcon::Classic => "Classic",
            AppIcon::Dark => "Dark",
            AppIcon::Grunge => "Grunge",
            AppIcon::Light => "Light",
            AppIcon::Space => "Space",
            AppIcon::Swiss => "Swiss",
            AppIcon::Vostok => "Vostok",
        };
        write!(f, "{value}")
    }
}

impl AppIconSettings {
    pub fn get_base_icon_file_name(icon: AppIcon) -> &'static str {
        match icon {
            AppIcon::Default | AppIcon::Classic => "classic",
            AppIcon::Beaver => "beaver",
            AppIcon::Dark => "dark",
            AppIcon::Grunge => "grunge",
            AppIcon::Light => "light",
            AppIcon::Space => "space",
            AppIcon::Swiss => "swiss",
            AppIcon::Vostok => "vostok",
        }
    }
}

impl SettingsValue for AppIcon {
    fn to_file_value(&self) -> serde_json::Value {
        let value = match self {
            AppIcon::Default => "default",
            AppIcon::Beaver => "beaver",
            AppIcon::Classic => "classic",
            AppIcon::Dark => "dark",
            AppIcon::Grunge => "grunge",
            AppIcon::Light => "light",
            AppIcon::Space => "space",
            AppIcon::Swiss => "swiss",
            AppIcon::Vostok => "vostok",
        };
        serde_json::Value::String(value.to_string())
    }

    fn from_file_value(value: &serde_json::Value) -> Option<Self> {
        match value.as_str()? {
            "default" => Some(AppIcon::Default),
            "beaver" => Some(AppIcon::Beaver),
            "classic" => Some(AppIcon::Classic),
            "dark" => Some(AppIcon::Dark),
            "grunge" => Some(AppIcon::Grunge),
            "light" => Some(AppIcon::Light),
            "space" => Some(AppIcon::Space),
            "swiss" => Some(AppIcon::Swiss),
            "vostok" => Some(AppIcon::Vostok),
            "aurora" | "classic1" | "classic_1" | "classic2" | "classic_2" | "classic3"
            | "classic_3" | "comets" | "cow" | "glasssky" | "glass_sky" | "glitch" | "glow"
            | "holographic" | "mono" | "neon" | "original" | "starburst" | "sticker"
            | "warpone" | "warp_one" => Some(AppIcon::Default),
            _ => None,
        }
    }
}

define_settings_group!(AppIconSettings, settings: [
    app_icon: AppIconState {
        type: AppIcon,
        default: AppIcon::Default,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        storage_key: "AppIcon",
        toml_path: "appearance.icon.app_icon",
        description: "The app icon displayed in the dock.",
    },
]);
