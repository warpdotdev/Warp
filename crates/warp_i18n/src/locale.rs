use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_value::SettingsValue;

/// Supported UI locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, JsonSchema)]
pub enum Locale {
    #[default]
    EnUs,
    ZhCn,
}

impl SettingsValue for Locale {}

impl Locale {
    /// Human-readable label shown in the language picker (always in the locale's own
    /// language so users can find it regardless of current UI language).
    pub fn display_name(self) -> &'static str {
        match self {
            Locale::EnUs => "English",
            Locale::ZhCn => "简体中文",
        }
    }

    /// BCP-47 tag used by `fluent-bundle`.
    pub fn fluent_tag(self) -> &'static str {
        match self {
            Locale::EnUs => "en-US",
            Locale::ZhCn => "zh-CN",
        }
    }

    /// Detect from the system locale.
    pub fn from_system() -> Self {
        match sys_locale::get_locale() {
            Some(tag) if tag.starts_with("zh") => Locale::ZhCn,
            _ => Locale::EnUs,
        }
    }

    /// Parse from a persisted string (e.g. TOML value).
    pub fn parse(s: &str) -> Self {
        match s {
            "zh-CN" | "zh_CN" | "zh-cn" | "zh_cn" | "ZhCn" => Locale::ZhCn,
            _ => Locale::EnUs,
        }
    }

    /// All available locales, for populating the dropdown.
    pub fn all() -> Vec<Self> {
        vec![Locale::EnUs, Locale::ZhCn]
    }
}
