use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

const DEFAULT_LOCALE: &str = "en";
const ZH_CN_LOCALE: &str = "zh-CN";
const LOCALES_DIR: &str = "bundled/locales";

type Locale = String;
type Key = String;
type Translations = HashMap<Locale, HashMap<Key, String>>;

static CURRENT_LOCALE: RwLock<&'static str> = RwLock::new(DEFAULT_LOCALE);
static TRANSLATIONS: OnceLock<Translations> = OnceLock::new();

pub fn init_locale() {
    let locale = std::env::var("WARP_LANG")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();

    if locale.starts_with("zh") {
        set_locale(ZH_CN_LOCALE);
    } else {
        set_locale(DEFAULT_LOCALE);
    }
}

pub fn set_locale(locale: &str) {
    let locale = if locale.starts_with("zh") {
        ZH_CN_LOCALE
    } else {
        DEFAULT_LOCALE
    };

    if let Ok(mut current_locale) = CURRENT_LOCALE.write() {
        *current_locale = locale;
    }
}

pub fn t(key: &'static str) -> Cow<'static, str> {
    translate(current_locale(), key)
        .or_else(|| translate(DEFAULT_LOCALE, key))
        .unwrap_or(Cow::Borrowed(key))
}

pub fn interpolate(template: &str, args: &[(&str, String)]) -> Cow<'static, str> {
    let mut value = template.to_owned();
    for (key, replacement) in args {
        value = value.replace(&format!("{{{key}}}"), replacement);
    }
    Cow::Owned(value)
}

fn current_locale() -> &'static str {
    CURRENT_LOCALE
        .read()
        .map(|locale| *locale)
        .unwrap_or(DEFAULT_LOCALE)
}

fn translate(locale: &str, key: &'static str) -> Option<Cow<'static, str>> {
    translations()
        .get(locale)
        .and_then(|translations| translations.get(key))
        .map(|value| Cow::Borrowed(value.as_str()))
}

fn translations() -> &'static Translations {
    TRANSLATIONS.get_or_init(load_translations)
}

#[cfg(not(target_family = "wasm"))]
fn load_translations() -> Translations {
    locale_dirs()
        .into_iter()
        .find_map(load_dir)
        .unwrap_or_default()
}

#[cfg(not(target_family = "wasm"))]
fn locale_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(path) = std::env::var("WARP_LOCALES_DIR") {
        dirs.push(path.into());
    }

    if let Some(resources_dir) = warp_core::paths::bundled_resources_dir() {
        dirs.push(resources_dir.join(LOCALES_DIR));
    }

    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        let app_dir = std::path::PathBuf::from(manifest_dir);
        if let Some(repo_root) = app_dir.parent() {
            dirs.push(repo_root.join("resources").join(LOCALES_DIR));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("resources").join(LOCALES_DIR));
    }

    dirs
}

#[cfg(not(target_family = "wasm"))]
fn load_dir(path: std::path::PathBuf) -> Option<Translations> {
    let entries = std::fs::read_dir(path).ok()?;
    let mut translations = Translations::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };

        if !matches!(extension, "yml" | "yaml") {
            continue;
        }

        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        merge_locale_file(&contents, &mut translations);
    }

    (!translations.is_empty()).then_some(translations)
}

#[cfg(target_family = "wasm")]
fn load_translations() -> Translations {
    let mut translations = Translations::new();
    merge_locale_file(
        include_str!("../../resources/bundled/locales/en.yml"),
        &mut translations,
    );
    merge_locale_file(
        include_str!("../../resources/bundled/locales/zh-CN.yml"),
        &mut translations,
    );
    translations
}

fn merge_locale_file(contents: &str, translations: &mut Translations) {
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(contents) else {
        return;
    };

    let serde_yaml::Value::Mapping(locales) = value else {
        return;
    };

    for (locale, values) in locales {
        let Some(locale) = locale.as_str() else {
            continue;
        };

        flatten_value(
            "",
            &values,
            translations.entry(locale.to_owned()).or_default(),
        );
    }
}

fn flatten_value(prefix: &str, value: &serde_yaml::Value, translations: &mut HashMap<Key, String>) {
    match value {
        serde_yaml::Value::Mapping(values) => {
            for (key, value) in values {
                let Some(key) = key.as_str() else {
                    continue;
                };

                let key = if prefix.is_empty() {
                    key.to_owned()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_value(&key, value, translations);
            }
        }
        serde_yaml::Value::String(value) => {
            translations.insert(prefix.to_owned(), value.to_owned());
        }
        _ => {}
    }
}
