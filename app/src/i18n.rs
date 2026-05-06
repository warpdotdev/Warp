/// Centralized i18n system for the Settings UI.
///
/// Translation strings are stored in JSON files under `src/locales/`.
/// Each file covers one Settings menu section.
///
/// JSON format:
/// ```json
/// {
///   "settings.account.sign_up": { "en": "Sign up", "zh": "注册" }
/// }
/// ```
///
/// The `tr!(key, ctx)` macro looks up by key + current UILanguage.
/// Falls back to "en" if the requested language is missing.
///
/// The legacy `t!(ctx, "en_text", "zh_text")` macro still works for
/// any strings not yet migrated to JSON keys.
use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::settings::UILanguage;

// ── JSON schema ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TranslationEntry {
    en: String,
    #[serde(default)]
    zh: Option<String>,
}

// ── Static registry ────────────────────────────────────────────────────────

/// Flat map: key -> lang -> text
static REGISTRY: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();

/// All locale JSON files embedded at compile time.
const LOCALE_FILES: &[&str] = &[
    include_str!("locales/menus.json"),
    include_str!("locales/settings.json"),
    include_str!("locales/settings_account.json"),
    include_str!("locales/settings_agents.json"),
    include_str!("locales/settings_appearance.json"),
    include_str!("locales/settings_features.json"),
    include_str!("locales/settings_keybindings.json"),
    include_str!("locales/settings_privacy.json"),
    include_str!("locales/settings_code.json"),
    include_str!("locales/settings_billing.json"),
    include_str!("locales/settings_teams.json"),
    include_str!("locales/settings_warp_drive.json"),
    include_str!("locales/settings_warpify.json"),
    include_str!("locales/settings_about.json"),
    include_str!("locales/settings_environments.json"),
    include_str!("locales/settings_mcp_servers.json"),
];

fn registry() -> &'static HashMap<String, HashMap<String, String>> {
    REGISTRY.get_or_init(|| {
        let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
        for json in LOCALE_FILES {
            let entries: HashMap<String, TranslationEntry> =
                serde_json::from_str(json).expect("locale JSON must be valid");
            for (key, entry) in entries {
                let mut langs = HashMap::new();
                langs.insert("en".to_string(), entry.en);
                if let Some(zh) = entry.zh {
                    langs.insert("zh".to_string(), zh);
                }
                map.insert(key, langs);
            }
        }
        map
    })
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Look up a translation by key and language code.
/// Falls back to "en" if the requested language is absent.
/// Returns `None` if the key is not found (caller should use the key as fallback).
pub fn translate(key: &'static str, lang: &str) -> &'static str {
    let reg = registry();
    if let Some(langs) = reg.get(key) {
        let text = langs
            .get(lang)
            .or_else(|| langs.get("en"))
            .map(|s| s.as_str())
            .unwrap_or(key);
        // SAFETY: `text` points into the 'static OnceLock registry; the registry is
        // never mutated after initialization, so the pointer is stable for 'static.
        unsafe { std::mem::transmute::<&str, &'static str>(text) }
    } else {
        key
    }
}

/// Convert a UILanguage to its ISO language tag.
pub fn lang_code(lang: UILanguage) -> &'static str {
    match lang {
        UILanguage::English => "en",
        UILanguage::ChineseSimplified => "zh",
    }
}

// ── Macros ─────────────────────────────────────────────────────────────────

/// Look up a translation by JSON key.
///
/// Usage: `tr!("settings.account.sign_up", ctx)`
///
/// Falls back to "en" if the current language has no entry for this key.
/// Falls back to the key string itself if no entry exists at all.
#[macro_export]
macro_rules! tr {
    ($key:literal, $ctx:expr) => {{
        use ::warpui::SingletonEntity as _;
        use ::settings::Setting as _;
        let lang = $crate::i18n::lang_code(
            *<$crate::settings::LanguageSettings as ::warpui::SingletonEntity>::as_ref($ctx)
                .ui_language
                .value(),
        );
        $crate::i18n::translate($key, lang)
    }};
}

/// Legacy inline-string translation macro.
///
/// Usage: `t!(ctx, "English text", "中文文本")`
///
/// Kept for backward compatibility. Prefer `tr!` for new strings.
#[macro_export]
macro_rules! t {
    ($ctx:expr, $en:literal, $zh:literal) => {{
        use ::settings::Setting as _;
        use ::warpui::SingletonEntity as _;
        match *<$crate::settings::LanguageSettings as ::warpui::SingletonEntity>::as_ref($ctx)
            .ui_language
            .value()
        {
            $crate::settings::UILanguage::ChineseSimplified => $zh,
            _ => $en,
        }
    }};
}
