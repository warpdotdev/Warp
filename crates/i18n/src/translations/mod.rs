mod de;
mod en;
mod ja;
mod ko;
mod pt_br;
mod zh_cn;

use crate::Language;

/// Looks up a translation key for the given language.
/// Falls back to English if the key is missing.
/// Returns the key itself if not found in English either.
pub fn lookup(lang: Language, key: &'static str) -> &'static str {
    let translated = match lang {
        Language::English => en::TRANSLATIONS.get(key),
        Language::SimplifiedChinese => zh_cn::TRANSLATIONS.get(key),
        Language::Japanese => ja::TRANSLATIONS.get(key),
        Language::Korean => ko::TRANSLATIONS.get(key),
        Language::BrazilianPortuguese => pt_br::TRANSLATIONS.get(key),
        Language::German => de::TRANSLATIONS.get(key),
    };

    if let Some(value) = translated {
        return value;
    }

    // Fall back to English
    if lang != Language::English {
        if let Some(value) = en::TRANSLATIONS.get(key) {
            return value;
        }
    }

    // Last resort: return the key itself
    key
}

/// Returns the translation map for a given language.
#[cfg(test)]
fn translations_for(lang: Language) -> &'static std::collections::HashMap<&'static str, &'static str> {
    match lang {
        Language::English => &en::TRANSLATIONS,
        Language::SimplifiedChinese => &zh_cn::TRANSLATIONS,
        Language::Japanese => &ja::TRANSLATIONS,
        Language::Korean => &ko::TRANSLATIONS,
        Language::BrazilianPortuguese => &pt_br::TRANSLATIONS,
        Language::German => &de::TRANSLATIONS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_languages_have_english_keys() {
        let en_keys: Vec<&&str> = en::TRANSLATIONS.keys().collect();
        let mut missing_by_lang: Vec<(Language, Vec<&str>)> = Vec::new();

        for lang in Language::all() {
            if *lang == Language::English {
                continue;
            }
            let translations = translations_for(*lang);
            let missing: Vec<&str> = en_keys
                .iter()
                .filter(|key| !translations.contains_key(**key))
                .map(|key| **key)
                .collect();
            if !missing.is_empty() {
                missing_by_lang.push((*lang, missing));
            }
        }

        if !missing_by_lang.is_empty() {
            let mut msg = String::from("Missing translation keys:\n");
            for (lang, keys) in &missing_by_lang {
                msg.push_str(&format!("  {} ({}): {} keys missing\n", lang.code(), lang.native_name(), keys.len()));
                for key in keys {
                    msg.push_str(&format!("    - {}\n", key));
                }
            }
            panic!("{}", msg);
        }
    }

    #[test]
    fn no_language_has_extra_keys_beyond_english() {
        let _en_count = en::TRANSLATIONS.len();
        for lang in Language::all() {
            if *lang == Language::English {
                continue;
            }
            let translations = translations_for(*lang);
            let extra: Vec<&&str> = translations
                .keys()
                .filter(|key| !en::TRANSLATIONS.contains_key(**key))
                .collect();
            assert!(
                extra.is_empty(),
                "{} has {} extra keys not in English: {:?}",
                lang.code(),
                extra.len(),
                extra
            );
        }
    }

    #[test]
    fn english_key_count_is_reasonable() {
        // Ensure we have at least the baseline number of keys
        let count = en::TRANSLATIONS.len();
        assert!(
            count >= 80,
            "Expected at least 80 English keys, got {}",
            count
        );
    }
}
