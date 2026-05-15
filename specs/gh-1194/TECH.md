# TECH: i18n / Multiple Language Support

## Context

Warp has no existing i18n infrastructure. Every user-facing string in the codebase is a hardcoded English literal — `"Save"`, `"File"`, `"Close tab"`, etc. This spec proposes a custom, lightweight i18n framework built directly into the Warp Rust codebase, with zh-CN as the first non-English locale.

**Relevant files:**
- `crates/i18n/src/lib.rs` — core i18n engine (new crate)
- `app/src/lib.rs:4-30` — `t!()` macro definition
- `app/src/lib.rs:610` — `init_locale()` call in application startup
- `resources/bundled/locales/en.yml` — English locale file (2,944 lines)
- `resources/bundled/locales/zh-CN.yml` — Chinese locale file (2,944 lines)
- `crates/onboarding/src/lib.rs:3-29` — duplicate `t!()` macro for onboarding binary
- `crates/onboarding/src/bin/main.rs:43` — `init_locale()` for onboarding

**Why not a third-party crate?** Existing Rust i18n crates (Fluent, gettext, ICU4X) add significant complexity and dependencies for a feature that currently only needs two locales with simple key-value lookups. A custom solution keeps the surface area small and the build fast.

## Proposed changes

### 1. New crate: `warp_i18n`

A new workspace crate at `crates/i18n/` with dependencies `serde_yaml` (for parsing YAML) and `sys-locale` (for OS-level locale detection).

**Core types:**
- `Translations`: `HashMap<Locale, HashMap<Key, String>>` — double-map structure where the outer key is locale (`"en"`, `"zh-CN"`) and the inner key is a dot-separated path (e.g., `"menu.file"`).
- Global state: `TRANSLATIONS` (lazy-initialized via `OnceLock`) and `CURRENT_LOCALE` (backed by `RwLock<&'static str>`).

**Public API:**
```rust
pub fn init_locale()                                    // Resolve & set current locale
pub fn set_locale(locale: &str)                         // Force a specific locale
pub fn t(key: &'static str) -> Cow<'static, str>        // Primary translation lookup
pub fn interpolate(template: &str, args: &[(&str, String)]) -> Cow<'static, str>
```

**Locale resolution pipeline** (`init_locale`):
1. Check `WARP_LANG` env var → if set and starts with `"zh"`, use `zh-CN`.
2. Check system locale (`LANG`, `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` on POSIX; native API on Windows/macOS) → if starts with `"zh"`, use `zh-CN`.
3. Default to `en`.

**Translation lookup** (`t`):
1. Look up key in `TRANSLATIONS[current_locale]`.
2. If missing, fall back to `TRANSLATIONS["en"]`.
3. If still missing, return the key string itself as a borrowed `Cow` (never panic, never render empty text).

**Interpolation** (`interpolate`):
- Simple `str::replace` of `{name}` placeholders with provided values.
- Not a template engine — no escaping, no positional arguments, no format specifiers.
- Values are pre-formatted to `String` by the `t!()` macro before reaching `interpolate`.

**YAML loading:**
```rust
fn load_translations() -> Translations {
    for dir in locale_dirs() {          // Try multiple filesystem paths
        if let Some(t) = load_dir(dir) { // First successful directory wins
            return t;
        }
    }
    // On WASM, use include_str!() instead of filesystem access
}
```

`load_dir` reads all `.yml`/`.yaml` files in a directory, parses them with `serde_yaml`, identifies the locale from the top-level key, and recursively flattens nested YAML into dot-separated keys via `flatten_value`.

**File discovery order** (non-WASM):
1. `$WARP_LOCALES_DIR` env var (if set)
2. Platform bundled resources: `<exe_dir>/resources/bundled/locales`
3. `$CARGO_MANIFEST_DIR/../resources/bundled/locales` (and up to 4 ancestor levels)
4. `$PWD/resources/bundled/locales`

### 2. `t!()` macro (defined in `app/src/lib.rs`)

A `macro_rules!` macro with three match arms:

```rust
// Arm 1: Simple lookup
t!("menu.file")
// Expands to: i18n::t("menu.file")

// Arm 2: Explicit interpolation
t!("terminal.hand_off", environment = name)
// Expands to: i18n::t("terminal.hand_off")
//               .map(|v| i18n::interpolate(&v, &[("environment", format!("{}", name))]))

// Arm 3: Implicit interpolation (variable name = key name)
t!("some.key", count)
// Expands to: i18n::t("some.key")
//               .map(|v| i18n::interpolate(&v, &[("count", format!("{}", count))]))
```

All arms include the English fallback check: if `t()` returns the key itself (meaning no translation was found), no interpolation is performed — the raw key is returned.

A duplicate `t!()` macro exists in `crates/onboarding/src/lib.rs` so the onboarding binary can use translations without depending on the full `app` crate.

### 3. Locale files

Two YAML files at `resources/bundled/locales/`:

| File | Locale | Keys | Lines |
|------|--------|------|-------|
| `en.yml` | English (fallback) | ~2,732 | 2,944 |
| `zh-CN.yml` | Simplified Chinese | ~2,732 | 2,944 |

Both files share identical structure — 94 top-level YAML sections corresponding to UI areas: `menu`, `tab`, `workspace`, `auth`, `billing`, `settings`, `ai_settings_page`, `terminal`, `shared_session`, `common`, etc.

Each string value in `en.yml` serves as the canonical key identifier. The `zh-CN.yml` file has the same keys with Chinese translations.

### 4. Usage patterns in the codebase

All user-facing string literals are replaced with `t!()` calls. ~4,700+ callsites across `app/src/`. Key patterns:

| Pattern | Example | When |
|---------|---------|------|
| Simple string | `t!("common.save")` | Static labels |
| With interpolation | `t!("ai_output.in_path", path = display_path)` | Dynamic content |
| `.to_string()` | `t!("common.save").to_string()` | APIs requiring `String` |
| In UI components | `ActionButton::new(t!("key"), theme)` | Buttons, menus |
| In formatted text | `FormattedTextFragment::plain_text(t!("key"))` | Rich text blocks |

### 5. Application integration

`init_locale()` is called at the top of `app::run()` (`app/src/lib.rs:610`), before feature flags are initialized and before any UI is created. This ensures translations are available for the entire application lifecycle.

The `app/src/i18n.rs` file re-exports `warp_i18n::*` so the rest of the application uses `crate::i18n::t()` without needing to depend on `warp_i18n` directly.

### 6. Windows compiler support

`windows-rs` crate macros generate code referencing `i18n::t()` on the `app` crate. Windows resource compilation works correctly because no i18n call appears in a const-evaluation context.

## Testing and validation

### Unit tests

- `warp_i18n` should have tests for:
  - `t()` with a key present in both locales returns the current locale's value
  - `t()` with a key present only in English falls back to English
  - `t()` with a missing key returns the key string itself
  - `interpolate()` correctly substitutes one and multiple placeholders
  - `set_locale("zh-CN")` correctly switches the active locale
  - `set_locale("fr")` falls back to `en`
  - `load_dir()` correctly parses YAML and produces flattened keys

### Integration / manual verification

- Launch Warp with `WARP_LANG=zh-CN` on macOS and verify:
  - Menu bar shows Chinese labels
  - Settings panels render in Chinese
  - Agent mode UI texts are in Chinese
  - Tooltips and notifications are in Chinese
- Launch without `WARP_LANG` (or with `WARP_LANG=en`) and verify all UI renders in English
- Verify that terminal PTY output is not affected by locale
- Verify that a deliberate missing key (present in code but absent from both YAML files) renders as the key string rather than crashing

### Regression prevention

- The `cargo check` / `cargo build` pipeline for the `warp-oss` binary must pass on both macOS and Windows MSVC
- All existing tests must pass after the migration — no test assertions should be broken by i18n
- Behavior invariants from `PRODUCT.md` map to verification steps above

## Parallelization

Not applicable. The i18n work is a single cohesive change across the codebase — string replacements, locale file authoring, and framework implementation are all tightly coupled and should be done in a single branch by a single author.
