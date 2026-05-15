# PRODUCT: i18n / Multiple Language Support

## Summary

Add internationalization (i18n) support to Warp, enabling the application UI to render in languages other than English. Simplified Chinese (zh-CN) is shipped as the first non-English locale. English remains the default and fallback language. Locale selection is automatic based on system settings, with an environment variable override for power users.

## Problem

Warp's UI is entirely hardcoded in English. Non-English-speaking developers must navigate menus, settings, agent UI, and onboarding in a language they may not be fluent in, creating friction and limiting adoption. Internationalization is consistently one of the most-requested features from the Warp community.

## Goals / Non-goals

**Goals:**
- Provide a complete zh-CN translation of the Warp UI
- Establish an i18n framework that supports adding more locales in the future
- Automatic locale detection that requires zero configuration
- Graceful fallback to English for any untranslated strings

**Non-goals:**
- A language picker in Settings UI (this is env-var-only for the initial release)
- Pluralization or locale-aware number/date formatting
- Right-to-left (RTL) language support
- Runtime locale switching without application restart

## Behavior

### 1. Locale detection

1.1. On application startup, Warp determines the active locale from the following sources, in order:
    1. The `WARP_LANG` environment variable (highest priority).
    2. The system locale, read from the platform's native locale API (e.g., `LANG` / `LC_ALL` / `LC_MESSAGES` on Linux, `GetUserDefaultLocaleName` on Windows, `NSLocale` on macOS).
    3. If no locale can be determined, the application defaults to English (`en`).

1.2. The resolved locale is matched as follows:
    - Any locale starting with `"zh"` (e.g., `zh-CN`, `zh-TW`, `zh-HK`, `zh`) resolves to `zh-CN` (Simplified Chinese). This is an intentional simplification: zh-CN is the only Chinese locale shipped, so all Chinese-language systems receive Simplified Chinese.
    - All other locales — including explicit non-Chinese values of `WARP_LANG` — resolve to `en` (English).
    - The explicit `WARP_LANG` override is NOT passed through as a raw locale string. Setting `WARP_LANG=fr` results in `en`, not `fr`.
    - The `WARP_LANG` variable takes precedence over system locale only when its value starts with `"zh"`. On a Chinese-system machine (where the OS locale is `zh-CN`), setting `WARP_LANG=fr` still yields `en`; unsetting `WARP_LANG` restores `zh-CN`.

1.3. The application ships with exactly two locale files: `en.yml` and `zh-CN.yml`.

### 2. Translation rendering

2.1. Every user-facing text string in the Warp UI is translated via the `t!("dot.path")` macro. The argument to `t!()` is a dot-separated path into a YAML locale file — this path is the lookup key. The English value at that path in `en.yml` serves as the fallback text when no translation exists for the active locale. The Chinese value at the same path in `zh-CN.yml` is shown when zh-CN is active.

2.2. When a translation is requested for a given key:
    1. The active locale file is checked first.
    2. If the key is missing from the active locale, the English (`en`) locale file is checked.
    3. If the key is missing from both, the raw key string is displayed (e.g., `"menu.file"`).

2.3. The entire UI surface is covered: macOS menu bar, tab context menus, workspace toolbar, settings panels (AI, appearance, keybindings, billing, features), agent UI, onboarding, auth dialogs, resource center, code review, terminal prompts, tooltips, notifications, and all modal dialogs. In total, approximately 2,700 distinct UI strings are translated.

### 3. String interpolation

3.1. Some translated strings contain dynamic values (e.g., `"Hand off to {environment}"`). These placeholders use `{name}` syntax within the YAML locale strings.

3.2. At render time, the translation macro accepts named arguments: `t!("key", name = value)`. The value is converted to a string and substituted for `{name}` in the translated template.

3.3. If a key with interpolation arguments is missing from both locale files, the raw key is displayed (interpolation is not applied to the fallback key).

### 4. Locale file format

4.1. Locale files use YAML and are stored at `resources/bundled/locales/<locale>.yml`.

4.2. The top-level key in each file is the locale name (e.g., `en:`, `zh-CN:`). Nested YAML structure maps to dot-separated translation keys. For example:
    ```yaml
    en:
      menu:
        file: "File"
        edit: "Edit"
    ```
    produces keys `menu.file` and `menu.edit`.

4.3. English (`en.yml`) is the canonical source of truth for key names. Every key present in `en.yml` must have a corresponding entry in `zh-CN.yml`.

### 5. Locale switching

5.1. There is no in-app language switcher in the initial release.

5.2. To change locale, the user sets `WARP_LANG=zh-CN` (or `WARP_LANG=zh`) before launching Warp, and the change takes effect on next launch.

5.3. Setting `WARP_LANG` to a non-Chinese value (or unsetting it) restores the system-locale-based default (English for non-Chinese systems).

### 6. Fallback and missing translations

6.1. If a translation key is present in `en.yml` but missing from `zh-CN.yml`, the English string is shown. No warning or error is surfaced to the user.

6.2. If a translation key is missing from both `en.yml` and `zh-CN.yml` (e.g., a newly-added UI string that has not been localized yet), the raw key string is shown. This ensures the application never panics or renders empty text due to a missing translation.

6.3. The English locale file (`en.yml`) is always loaded regardless of the active locale, to serve as the fallback.

### 7. Platform consistency

7.1. macOS menu bar items are fully localized. Chinese users see localized menu labels (e.g., "文件" for "File", "编辑" for "Edit").

7.2. Onboarding slides shown to new users on first launch are localized.

7.3. Keyboard shortcut labels and modifier key names are NOT translated (e.g., `⌘S` remains `⌘S` regardless of locale).

7.4. Terminal output (PTY content) is NOT affected by i18n. Only the Warp UI chrome is translated.

## Figma

Figma: none provided

## Open questions

- **Language picker in Settings:** Should a future iteration add a UI language selector in the Appearance or General settings? This would require runtime locale switching without restart.
- **Contributor workflow:** How should translators contribute locale files for additional languages? A CONTRIBUTING guide for new locale YAML files may be needed.
- **String extraction:** Should locale keys be auto-generated from source code, or manually authored in YAML? The current approach is manual YAML authorship.
