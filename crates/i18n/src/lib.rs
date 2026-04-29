mod translations;

use serde::{Deserialize, Serialize};
use settings_value::SettingsValue;
use std::sync::RwLock;

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ko")]
    Korean,
    #[serde(rename = "pt-BR")]
    BrazilianPortuguese,
    #[serde(rename = "de")]
    German,
}

impl Language {
    /// Returns all available languages for the dropdown.
    pub fn all() -> &'static [Language] {
        &[
            Language::English,
            Language::SimplifiedChinese,
            Language::Japanese,
            Language::Korean,
            Language::BrazilianPortuguese,
            Language::German,
        ]
    }

    /// Returns the display name in the native script.
    pub fn native_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::SimplifiedChinese => "\u{4E2D}\u{6587}\u{FF08}\u{7B80}\u{4F53}\u{FF09}",
            Language::Japanese => "\u{65E5}\u{672C}\u{8A9E}",
            Language::Korean => "\u{D55C}\u{AD6D}\u{C5B4}",
            Language::BrazilianPortuguese => "Portugu\u{00EA}s",
            Language::German => "Deutsch",
        }
    }

    /// Returns the BCP 47 language code.
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::SimplifiedChinese => "zh-CN",
            Language::Japanese => "ja",
            Language::Korean => "ko",
            Language::BrazilianPortuguese => "pt-BR",
            Language::German => "de",
        }
    }

    /// Returns the display string for the dropdown: "Native Name (code)".
    pub fn display_label(&self) -> String {
        format!("{} ({})", self.native_name(), self.code())
    }

    /// Detects language from system locale, falling back to English.
    pub fn from_system_locale() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
        let locale_lower = locale.to_lowercase();

        // Handle Chinese variants: zh-CN (Linux), zh_CN (legacy), zh-Hans (macOS BCP 47), zh (bare).
        if locale_lower.starts_with("zh-cn")
            || locale_lower.starts_with("zh_cn")
            || locale_lower.starts_with("zh-hans")
            || locale_lower.starts_with("zh")
        {
            Language::SimplifiedChinese
        } else if locale_lower.starts_with("ja") {
            Language::Japanese
        } else if locale_lower.starts_with("ko") {
            Language::Korean
        } else if locale_lower.starts_with("pt-br")
            || locale_lower.starts_with("pt_br")
            || locale_lower == "pt"
        {
            Language::BrazilianPortuguese
        } else if locale_lower.starts_with("de") {
            Language::German
        } else {
            Language::English
        }
    }
}

impl SettingsValue for Language {}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "en" => Ok(Language::English),
            "zh-CN" | "zh_CN" | "zh" => Ok(Language::SimplifiedChinese),
            "ja" => Ok(Language::Japanese),
            "ko" => Ok(Language::Korean),
            "pt-BR" | "pt_BR" | "pt" => Ok(Language::BrazilianPortuguese),
            "de" => Ok(Language::German),
            _ => Err(format!("Unknown language code: {s}")),
        }
    }
}

static CURRENT_LANGUAGE: RwLock<Language> = RwLock::new(Language::English);

/// Sets the current UI language.
pub fn set_language(lang: Language) {
    if let Ok(mut current) = CURRENT_LANGUAGE.write() {
        *current = lang;
    }
}

/// Returns the current UI language.
///
/// Panics if the internal lock is poisoned — a poisoned lock indicates
/// a prior panic in another thread and should never happen in normal operation.
pub fn current_language() -> Language {
    *CURRENT_LANGUAGE.read().unwrap_or_else(|e| {
        panic!("i18n language lock poisoned: {e}");
    })
}

/// Looks up a translation key for the current language.
/// Falls back to English if the key is missing in the current language.
pub fn t(key: &'static str) -> &'static str {
    let lang = current_language();
    translations::lookup(lang, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static GLOBAL_LANGUAGE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn language_all_returns_six_variants() {
        assert_eq!(Language::all().len(), 6);
    }

    #[test]
    fn language_codes_are_correct() {
        assert_eq!(Language::English.code(), "en");
        assert_eq!(Language::SimplifiedChinese.code(), "zh-CN");
        assert_eq!(Language::Japanese.code(), "ja");
        assert_eq!(Language::Korean.code(), "ko");
        assert_eq!(Language::BrazilianPortuguese.code(), "pt-BR");
        assert_eq!(Language::German.code(), "de");
    }

    #[test]
    fn language_native_names_are_non_empty() {
        for lang in Language::all() {
            assert!(!lang.native_name().is_empty());
            assert!(!lang.display_label().is_empty());
        }
    }

    #[test]
    fn from_str_round_trips() {
        for lang in Language::all() {
            let parsed: Language = lang.code().parse().unwrap();
            assert_eq!(parsed, *lang);
        }
    }

    #[test]
    fn t_returns_english_by_default() {
        let _guard = GLOBAL_LANGUAGE_LOCK.lock().unwrap();
        set_language(Language::English);
        let label = t("settings.appearance.language.label");
        assert_eq!(label, "Language");
    }

    #[test]
    fn t_returns_translated_string() {
        let _guard = GLOBAL_LANGUAGE_LOCK.lock().unwrap();
        set_language(Language::SimplifiedChinese);
        let label = t("settings.appearance.language.label");
        assert_eq!(label, "\u{8BED}\u{8A00}");
        // Reset to English for other tests
        set_language(Language::English);
    }

    #[test]
    fn t_returns_key_for_missing_translation() {
        let _guard = GLOBAL_LANGUAGE_LOCK.lock().unwrap();
        set_language(Language::Japanese);
        const MISSING_KEY: &str = "nonexistent.key.that.does.not.exist";
        let result = t(MISSING_KEY);
        assert_eq!(result, MISSING_KEY);
        set_language(Language::English);
    }

    #[test]
    fn set_and_get_language() {
        let _guard = GLOBAL_LANGUAGE_LOCK.lock().unwrap();
        set_language(Language::German);
        assert_eq!(current_language(), Language::German);
        set_language(Language::English);
        assert_eq!(current_language(), Language::English);
    }
}
