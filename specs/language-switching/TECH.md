# Tech Spec: UI Language Switching

Product spec: `specs/language-switching/PRODUCT.md`

## Context

Warp's UI currently has no i18n infrastructure — all strings are hardcoded English literals throughout the codebase. This spec introduces a translation framework, a persisted language setting, and a settings UI dropdown. The design follows existing Warp patterns (`define_settings_group!`, `SettingsWidget`, `render_dropdown_item`) to minimize novelty.

Relevant code:

- `app/src/settings_view/appearance_page.rs` — Appearance settings page; the language dropdown will be added here as a new category "Language" after "Themes".
- `app/src/settings_view/settings_page.rs:1791` — `SettingsWidget` trait definition.
- `app/src/settings_view/settings_page.rs:1182` — `PageType::Categorized` used by the appearance page.
- `crates/settings/src/macros.rs:703` — `define_settings_group!` macro for defining settings.
- `app/src/settings/input_mode.rs` — Reference for enum-based setting (non-boolean) using the macro.
- `app/src/settings/init.rs:57` — `register_all_settings()` where the new settings group will be registered.
- `crates/warpui_extras/src/user_preferences/mod.rs:18` — `UserPreferences` trait for persistence.

## Proposed changes

### 1. New crate: `crates/i18n/`

Create a new crate that provides the translation infrastructure.

**`crates/i18n/src/lib.rs`** — Core types and public API:

```rust
/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    pub fn all() -> &'static [Language] { ... }

    /// Returns the display name in the native script.
    pub fn native_name(&self) -> &'static str { ... }

    /// Returns the BCP 47 language code.
    pub fn code(&self) -> &'static str { ... }

    /// Detects language from system locale, falling back to English.
    pub fn from_system_locale() -> Self { ... }
}
```

**`crates/i18n/src/translations/`** — One file per language:

```
crates/i18n/src/translations/
  mod.rs          — declares modules, exposes `fn lookup(lang, key) -> &'static str`
  en.rs           — English strings (HashMap<&'static str, &'static str>)
  zh_cn.rs        — Simplified Chinese strings
  ja.rs           — Japanese strings
  ko.rs.rs        — Korean strings
  pt_br.rs        — Brazilian Portuguese strings
  de.rs           — German strings
```

Each translation file is a `phf::Map<&'static str, &'static str>` (compile-time perfect hash map) or a `lazy_static! { HashMap }` for fast lookups with zero runtime allocation.

**`crates/i18n/src/context.rs`** — Global locale state:

```rust
use std::sync::RwLock;

static CURRENT_LANGUAGE: RwLock<Language> = RwLock::new(Language::English);

pub fn set_language(lang: Language) { ... }
pub fn current_language() -> Language { ... }
pub fn t(key: &str) -> &'static str { ... }
```

`t(key)` reads `CURRENT_LANGUAGE`, looks up the key in the corresponding translation map, and falls back to the English map if the key is missing (invariant 24).

Add `crates/i18n/Cargo.toml` with dependencies: `serde`, `serde_json`, `phf` (or `lazy_static`). No dependency on `warpui` or `settings` — this crate is pure data.

### 2. Language setting definition

**New file: `app/src/settings/language.rs`**

Following the `InputModeState` pattern from `app/src/settings/input_mode.rs`:

```rust
use crate::i18n::Language;
use settings::{
    macros::define_settings_group, RespectUserSyncSetting, Setting, SupportedPlatforms, SyncToCloud,
};

define_settings_group!(LanguageSettings, settings: [
    language: LanguageState {
        type: Language,
        default: Language::English,  // overridden at startup by system locale detection
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::No,  // local-only per invariant 17
        private: false,
        storage_key: "Language",
        toml_path: "appearance.language",
        description: "The display language for Warp's interface.",
    },
]);
```

**Register in `app/src/settings/init.rs`** — add `LanguageSettings::register(ctx);` to `register_all_settings()`.

**System locale override** — in `register_all_settings()` (or a dedicated init step), after registering, check if the user has an explicit preference. If not (`!is_explicitly_set`), call `Language::from_system_locale()` and write the detected language. This satisfies invariant 27.

### 3. Translation key structure

Keys follow a hierarchical dot-notation pattern matching the settings TOML structure:

```
settings.appearance.themes
settings.appearance.language.label
settings.appearance.language.subtitle
settings.appearance.window.opacity
settings.features.copy_on_select
nav.account
nav.appearance
nav.features
nav.keybindings
nav.privacy
button.reset
button.add
button.remove
button.save
```

This structure keeps keys discoverable and aligns with the existing settings hierarchy. The `t()` function performs a direct map lookup — no interpolation needed for the initial release since translated strings contain no variables (invariant 23 is deferred; code/paths/URLs appear in English-context strings that are not translated).

### 4. Language dropdown widget

**In `app/src/settings_view/appearance_page.rs`** — add a new widget:

```rust
struct LanguageDropdownWidget {
    dropdown_state: DropdownState,
}

impl SettingsWidget for LanguageDropdownWidget {
    type View = AppearanceSettingsPageView;

    fn search_terms(&self) -> &str {
        "language locale 语言 lang display internationalization i18n"
    }

    fn render(&self, view: &Self::View, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let current = LanguageSettings::as_ref(app).language.value();
        render_dropdown_item(
            appearance,
            t("settings.appearance.language.label"),  // "Language"
            Some(t("settings.appearance.language.subtitle")),  // "Set the display language..."
            None,
            LocalOnlyIconState::Hidden,
            None,
            &view.language_dropdown,
        )
    }
}
```

**Add to `build_page()`** — insert a new `Category::new("Language", vec![Box::new(LanguageDropdownWidget::default())])` after the "Themes" category (invariant 1).

**Add action variant** — in `AppearancePageAction`, add `SetLanguage(Language)`.

**Handle action** — in the action dispatch:

```rust
AppearancePageAction::SetLanguage(lang) => {
    LanguageSettings::handle(ctx).update(ctx, |settings, ctx| {
        report_if_error!(settings.language.set_value(lang, ctx));
    });
    i18n::set_language(lang);
    ctx.notify();  // triggers re-render of all settings pages
}
```

### 5. Wiring up translation calls

This is the largest change by line count but the most mechanical. For each settings page:

1. Import `use crate::i18n::t;`
2. Replace hardcoded string literals with `t("key")` calls.
3. Keep `search_terms()` including both English and translated terms (invariant 13).

**Scope for initial PR** — to keep the PR reviewable, translate only:

- Settings navigation sidebar labels (in `app/src/settings_view/nav.rs`)
- Appearance page widget labels and subtitles
- The new language dropdown itself
- Common button labels (`"Reset"`, `"Add"`, `"Remove"`)

Other settings pages (Features, Keybindings, Privacy, AI, etc.) can be translated in follow-up PRs. The fallback-to-English behavior (invariant 24) ensures untranslated pages remain functional.

### 6. Startup initialization

**In `app/src/settings/init.rs`** — after `register_all_settings()`:

```rust
let lang = LanguageSettings::as_ref(ctx).language.value();
i18n::set_language(*lang);
```

This must happen before any UI renders (invariant 15). Since `register_all_settings()` runs during model initialization (before the first frame), the language is set before any `render()` call.

### 7. Settings search integration

The existing settings search (`update_filter` in `SettingsPageMeta`) matches against `search_terms()`. No structural change needed — each widget's `search_terms()` already includes English keywords. For translated search (invariant 13), add the translated terms to `search_terms()`:

```rust
fn search_terms(&self) -> &str {
    // Include both English and current-language terms
    "language locale 语言 lang"
}
```

This is a static string, so it won't update dynamically when the language changes. For dynamic search in the current language, a follow-up can make `search_terms()` return a `String` that includes `t("key")` calls. For the initial release, the static English keywords cover the common case.

## Testing and validation

### Unit tests

**`crates/i18n/src/lib.rs` tests:**

- `language_all_returns_six_variants` — `Language::all().len() == 6` (invariant from Summary).
- `language_native_name_round_trips` — each variant's `native_name()` is non-empty and `code()` matches expected BCP 47.
- `t_falls_back_to_english_for_missing_key` — set language to `Japanese`, call `t("nonexistent_key")`, verify it returns the English value (invariant 24).
- `t_returns_translated_string` — set language to `SimplifiedChinese`, call `t("settings.appearance.language.label")`, verify it returns `"语言"` not `"Language"`.
- `from_system_locale_falls_back_to_english` — on a system with an unsupported locale (e.g., `xx-YY`), `from_system_locale()` returns `English` (invariant 27).

**`app/src/settings/language.rs` tests:**

- `language_setting_persists_and_restores` — set language to `Japanese`, read back, verify value.
- `language_setting_defaults_to_english` — a fresh `LanguageSettings` with no stored value returns `English`.
- `invalid_toml_value_falls_back_to_english` — write `"appearance.language = 'xx-YY'"` to the settings file, reload, verify fallback to English (invariant 28).

### Integration tests

**`crates/integration/tests/language_switching.rs`:**

- `changing_language_updates_settings_ui` — open settings, change language dropdown to `SimplifiedChinese`, verify the "Appearance" nav label changes to `"外观"` (invariant 7).
- `language_persists_across_restart` — set language, simulate app restart by re-reading preferences, verify language is restored (invariant 14).
- `settings_search_finds_language_in_english` — set UI to Chinese, search "language", verify the setting appears (invariant 13).

### Manual validation

- Open Settings → Appearance, verify "Language" dropdown appears after "Themes" (invariant 1).
- Select "中文（简体）", verify all translated settings text switches to Chinese immediately (invariant 7).
- Close and reopen Warp, verify UI is still in Chinese (invariant 14).
- Search "language" in the search bar, verify the language setting is found (invariant 13).
- Delete `appearance.language` from TOML settings, restart, verify fallback to English (invariant 28).
- On a Japanese-locale system with no prior preference, verify Warp auto-selects Japanese on first launch (invariant 27).
- Rapidly toggle between English and Chinese 10 times, verify no crash or UI glitch (invariant 25).

## Risks and mitigations

### Risk: performance of `t()` calls during render
Every `render()` call now includes multiple `t()` lookups. Using `phf::Map` (compile-time perfect hash) keeps lookups O(1) with no allocation. If profiling shows concern, the current language can be cached in a thread-local to avoid the `RwLock` read on every call.

### Risk: translation completeness
Missing keys fall back to English (invariant 24), so partial translations don't break the UI. The initial PR only translates a subset of strings — the rest remain English. This is by design, not a gap.

### Risk: search_terms static string doesn't include translated terms
The initial implementation uses static `search_terms()` strings. Searching in the current language (e.g., searching "语言" when UI is Chinese) won't find the setting. Mitigation: include common translations in the static string for high-traffic settings. A follow-up can make `search_terms()` dynamic.

### Risk: crate dependency cycle
`crates/i18n/` must not depend on `settings` or `warpui` to avoid cycles. It is a leaf crate with only `serde` as a dependency. The settings layer imports `i18n`, not the other way around.

## Parallelization

The work splits cleanly into three independent tracks:

1. **i18n crate** (`crates/i18n/`) — can be built and tested in isolation.
2. **Settings integration** (`app/src/settings/language.rs`, `init.rs`) — depends only on the `i18n` crate types.
3. **UI widget + string replacement** (`appearance_page.rs`, `nav.rs`, translation files) — depends on both tracks above.

Track 1 and 2 can proceed in parallel; track 3 depends on both.

## Follow-ups

- Translate remaining settings pages (Features, Keybindings, Privacy, AI, Code, About).
- Make `search_terms()` return a `String` that includes translated terms for dynamic search.
- Add more languages (Spanish, French, Russian, Traditional Chinese) by adding translation files only.
- Consider a community translation contribution workflow (e.g., a `CONTRIBUTING-i18n.md` guide).
- Support string interpolation for invariant 23 (templates with `{variable}` placeholders).
