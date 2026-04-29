# Product Spec: UI Language Switching

**Issue:** (to be filed)
**Figma:** none provided

## Summary

Add a language selector to Warp's Settings > Appearance page, allowing users to switch the UI display language. The initial release supports six languages: English (en), Simplified Chinese (zh-CN), Japanese (ja), Korean (ko), Brazilian Portuguese (pt-BR), and German (de), with an extensible architecture for adding more languages in the future.

## Problem

Warp's UI is currently English-only. Non-English-speaking users face a language barrier when navigating settings, understanding AI agent prompts, and interpreting system messages. A language switching feature lowers the barrier to entry for international users and improves accessibility.

## Goals

- Users can switch the UI language from Settings > Appearance.
- Language selection persists across app restarts.
- Changing the language takes effect immediately without restarting Warp.
- The translation infrastructure is extensible — adding a new language requires only a new translation file, no code changes.

## Non-goals

- Translating user-generated content (terminal output, AI responses, code).
- Translating documentation or external links.
- Right-to-left (RTL) layout support.
- Per-workspace or per-profile language overrides (global setting only).
- Translating the onboarding flow in this iteration.

## Behavior

### Language selector location and interaction

1. A "Language" dropdown appears in Settings > Appearance, in the "General" category, below the existing "Theme" dropdown.
2. The dropdown label reads "Language" with a subtitle: "Set the display language for Warp's interface."
3. The dropdown displays the currently selected language name in its native script (e.g., "English", "中文（简体）").
4. Clicking the dropdown opens a list of available languages, each shown in its native script with the language code in parentheses (e.g., "English (en)", "中文（简体）(zh-CN)", "日本語 (ja)", "한국어 (ko)", "Português (pt-BR)", "Deutsch (de)").
5. The currently selected language has a checkmark indicator in the dropdown list.
6. The dropdown is searchable — typing filters the language list by native name or language code.

### Language change behavior

7. Selecting a different language from the dropdown immediately re-renders all settings UI text in the new language.
8. The language change takes effect without restarting Warp — all open settings pages, dialogs, and navigation items update in place.
9. After changing the language, the terminal area (command output, shell prompts) is unaffected — only Warp's chrome/UI changes.
10. AI agent blocks, tool call descriptions, and Warp's built-in AI UI elements are translated.
11. Error toasts, confirmation dialogs, and system notifications are translated.
12. Keyboard shortcut labels in the keybindings page remain in English (they reference physical keys).
13. Settings search works in both the current language and English — searching "theme" finds the theme setting regardless of whether the UI is displayed in Chinese, Japanese, Korean, Portuguese, or German.

### Persistence

14. The selected language is persisted to the user's local preferences (TOML settings file at `appearance.language`).
15. On app launch, Warp reads the persisted language and applies it before the first UI frame renders — no flash of English text followed by a switch.
16. If the persisted language code is not found in the available translations (e.g., the translation file was removed), Warp falls back to English and logs a warning.
17. The language setting is NOT synced to the cloud — it is a local-only preference, since the same user may prefer different languages on different machines.

### Translation coverage

18. All settings page titles, category headers, widget labels, subtitles, and tooltips are translated.
19. All navigation sidebar items (including umbrella group names) are translated.
20. All button labels (e.g., "Reset", "Add", "Remove", "Save") are translated.
21. All placeholder text in input fields is translated.
22. The command palette command names remain in English (they are developer-facing identifiers), but their descriptions are translated.
23. Strings that contain code, file paths, URLs, or variable names keep those tokens untranslated and interpolate them into the translated template.

### Edge cases

24. If a translation key is missing for the selected language, the English string is displayed as a fallback (no empty or broken UI).
25. Rapidly switching between languages (e.g., toggling back and forth) does not cause UI glitches, panics, or data loss.
26. The language setting is accessible via the settings search — searching "language" or "语言" (in any supported language) finds the setting.
27. On first launch (no persisted preference), Warp detects the system locale. If the system locale matches an available language, it is auto-selected; otherwise, English is the default.
28. If the user manually edits `appearance.language` in the TOML settings file to an invalid value, Warp falls back to English on next launch.
29. The dropdown shows all languages that have translation files present in the bundle — it does not show languages that are defined but lack a translation file.

## Success criteria

1. User can change the UI language among English, Simplified Chinese, Japanese, Korean, Brazilian Portuguese, and German via Settings > Appearance.
2. All settings page text updates immediately on language change without restart.
3. Language selection persists across app restarts.
4. Searching settings works in both English and the selected language.
5. Missing translation keys gracefully fall back to English.
6. System locale auto-detection selects the correct language on first launch.
