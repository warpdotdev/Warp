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
pub fn t_required(key: &'static str, fallback: &'static str) -> Cow<'static, str>
pub fn interpolate(template: &str, args: &[(&str, String)]) -> Cow<'static, str>
```

**Locale resolution** (`init_locale`):
1. Select candidates from the first non-empty source in priority order: `WARP_LANG` > `LANGUAGE` > `LC_ALL` > `LC_MESSAGES` > `LANG` > system locale API > `"en"` (default). `WARP_LANG` is Warp-specific and has highest priority. `LANGUAGE` is treated as the message-translation preference list on Linux-like environments.
2. Normalize each candidate before classification:
   - Trim whitespace.
   - For `LANGUAGE`, split colon-separated preference lists and evaluate entries in order.
   - Strip encoding and modifier suffixes after `.` or `@` (`zh_CN.UTF-8`, `zh_CN.utf8`, and `zh_CN.UTF-8@pinyin` all normalize to `zh-CN`).
   - Convert `_` to `-`, compare language/script/region subtags case-insensitively, and normalize script/region casing for tests (`zh-hans-cn` → `zh-Hans-CN`).
3. Classify the first supported normalized candidate: `zh`, `zh-CN`, `zh_CN`, `zh-Hans*`, and `zh_Hans*` map to `"zh-CN"`; all other candidates, including `zh-TW`, `zh-HK`, and `zh-Hant*`, map to `"en"` until a Traditional Chinese locale ships.
4. `WARP_LANG` is an explicit override. If it is set but does not classify to `"zh-CN"`, resolution returns `"en"` and does not fall through to lower-priority system locale sources.
5. No raw candidate is ever used as a locale key; runtime locale is always one of the two supported values.

**Translation lookup** (`t`):
1. Look up key in `TRANSLATIONS[current_locale]`.
2. If missing, fall back to `TRANSLATIONS["en"]`.
3. If still missing, ordinary non-sensitive UI returns the key string itself as a borrowed `Cow` (never panic, never render empty text). Security-sensitive UI must follow the no-locale fallback rule below instead of rendering raw dot-path keys.

**Required translation lookup** (`t_required`):
1. Look up key in `TRANSLATIONS[current_locale]`.
2. If missing, fall back to `TRANSLATIONS["en"]`.
3. If still missing, return the embedded English fallback passed by the call site.
4. `t_required` never returns the raw key. Auth, billing, privacy, sharing/permission, and agent-consent surfaces must use this API, either directly or through the `t_required!()` macro.

**Interpolation** (`interpolate`):
- Simple `str::replace` of `{name}` placeholders with provided values.
- Not a template engine — no escaping, no positional arguments, no format specifiers.
- Values are pre-formatted to `String` by the `t!()` macro before reaching `interpolate`.
- Interpolated values are treated as plain text. Call sites that render into rich, Markdown, link-capable, or otherwise parsed UI must pass interpolated values through the appropriate escaping layer or build the rich UI from structured fragments rather than by concatenating formatted strings. File paths, branch names, agent-provided labels, and server-provided metadata must never be allowed to inject markup, links, or formatting through translation interpolation.

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

Release builds (`#[cfg(not(debug_assertions))]`) only search the platform bundled resources path. Dev builds add the following higher-priority paths for rapid iteration:

| Priority | Path | Gate |
|----------|------|------|
| 1 | `$WARP_LOCALES_DIR` env var | `#[cfg(debug_assertions)]` |
| 2 | Platform bundled resources: `<exe_dir>/resources/bundled/locales` | Always enabled |
| 3 | `$CARGO_MANIFEST_DIR/../resources/bundled/locales` (and up to 4 ancestor levels) | `#[cfg(debug_assertions)]` |
| 4 | `$PWD/resources/bundled/locales` | `#[cfg(debug_assertions)]` |

**Security:** Paths 1, 3, and 4 are compiled out of release binaries. This prevents shipped builds from loading arbitrary YAML from environment variables or the current working directory into the startup parsing and UI rendering pipeline. In debug builds, locale files loaded from dev-only paths are subject to a size cap (8 MB per file) to prevent intentionally large or malformed YAML from causing excessive startup parsing in poisoned local environments. If a file loaded from a dev-only path exceeds the cap or is malformed, it is silently skipped and the next discovery path is tried — the application does not crash.

**No-locale fallback:** If no locale file can be loaded from any discovery path (e.g., corrupt installation, missing resource directory), `init_locale()` still completes successfully. For ordinary non-sensitive UI, the translation map remains empty and `t!()` returns the raw key string as the rendered text. Security-sensitive UI is different: auth, billing, privacy, sharing/permission, and agent-consent surfaces must not render raw dot-path keys. Those call sites must use `t_required!()` with an embedded English fallback, or fail closed by disabling the affected action and showing a readable English error that also uses `t_required!()`.

### 2. `t!()` and `t_required!()` macros (defined in `app/src/lib.rs`)

`t!()` is a `macro_rules!` macro with three match arms. The actual expansion uses `match` (not combinator chains) to preserve `Cow<'static, str>` type flow:

```rust
// Arm 1: Simple lookup
t!("menu.file")
// Expands to:
match i18n::t("menu.file") {
    value if value == "menu.file" => Cow::Owned("menu.file".to_string()),
    value => value,
}

// Arm 2: Explicit interpolation
t!("terminal.hand_off", environment = name)
// Expands to:
match i18n::t("terminal.hand_off") {
    value if value == "terminal.hand_off" => Cow::Owned("terminal.hand_off".to_string()),
    value => i18n::interpolate(value.as_ref(), &[("environment", format!("{}", name))]),
}

// Arm 3: Implicit interpolation (variable name = key name)
t!("some.key", count)
// Expands to:
match i18n::t("some.key") {
    value if value == "some.key" => Cow::Owned("some.key".to_string()),
    value => i18n::interpolate(value.as_ref(), &[("count", format!("{}", count))]),
}
```

The match guard `value if value == key` detects when `t()` returned the key itself (translation missing). In that case, the key string is returned as `Cow::Owned` — interpolation is skipped because there is no template to interpolate into.

`t_required!()` is the security-sensitive variant. It mirrors the same simple and interpolated call shapes, but requires an embedded English fallback literal:

```rust
t_required!("auth.require_login_share", "In order to share, please create an account.")
t_required!(
    "billing.shared_objects_limit_reached",
    "Shared {object_type}s limit reached",
    object_type = object_type_name,
)
```

The required macro expands through `i18n::t_required(key, fallback)`, then applies interpolation to the translated value or embedded fallback. It must never branch back to the raw key. The fallback argument must be a string literal so static analysis can verify that every security-sensitive call site has readable English text even when locale files are missing.

Duplicate `t!()` and `t_required!()` macros exist in `crates/onboarding/src/lib.rs` so the onboarding binary can use translations without depending on the full `app` crate. The macro copies are structurally identical — same call shapes, same fallback logic, same interpolation semantics. The only difference is the function reference: `$crate::i18n::*` in the app crate vs `warp_i18n::*` in onboarding. When changes to either macro are needed, both copies must be updated in the same commit. A code comment at each macro location references the other copy's file path to prevent drift.

### 3. Locale files

Two YAML files at `resources/bundled/locales/`:

| File | Locale | Keys | Lines |
|------|--------|------|-------|
| `en.yml` | English (fallback) | ~2,732 | 2,944 |
| `zh-CN.yml` | Simplified Chinese | ~2,732 | 2,944 |

Both files share identical structure — 94 top-level YAML sections corresponding to UI areas: `menu`, `tab`, `workspace`, `auth`, `billing`, `settings`, `ai_settings_page`, `terminal`, `shared_session`, `common`, etc.

The dot-separated YAML path (e.g., `menu.file`) is the lookup key used in `t!()` calls. The English string value at that path is the runtime fallback rendered when the active locale has no entry. The zh-CN string value is the Chinese rendering shown when zh-CN is active. Both values are independently authored — the key is the path, not the English text.

### 4. Usage patterns in the codebase

All user-facing string literals are replaced with `t!()` or `t_required!()` calls. ~4,700+ callsites across `app/src/`. Key patterns:

| Pattern | Example | When |
|---------|---------|------|
| Simple string | `t!("common.save")` | Static labels |
| With interpolation | `t!("ai_output.in_path", path = display_path)` | Dynamic content |
| Security-sensitive fallback | `t_required!("auth.require_login_share", "In order to share, please create an account.")` | Auth, billing, privacy, sharing, permission, and agent-consent copy |
| `.to_string()` | `t!("common.save").to_string()` | APIs requiring `String` |
| In UI components | `ActionButton::new(t!("key"), theme)` | Buttons, menus |
| In formatted text | `FormattedTextFragment::plain_text(t!("key"))` | Rich text blocks |

### 5. Migration scope

The migration scope must match the product requirement for the whole Warp UI surface. The implementation PR is not complete until all user-visible string literals in these areas are either localized or explicitly documented as out of scope:

| Area | Required coverage |
|------|-------------------|
| macOS app/menu chrome | app menu, tab/window menus, context menus, command palette labels |
| Core app UI in `app/src/` | workspace, panes, tabs, toolbars, settings, modals, notifications, toasts, tooltips, empty states |
| Account and monetization | auth, onboarding login, teams, billing, plan/credit warnings, payment restriction copy |
| Sharing and permissions | Warp Drive sharing, shared sessions, access labels, permission prompts |
| AI and agent surfaces | agent input/output chrome, Oz/cloud handoff, autonomy permissions, MCP/tool permissions, code review UI, agent management |
| Onboarding binary | first-run onboarding screens and prompts in `crates/onboarding` |
| Bundled resources | `resources/bundled/locales/en.yml` and `resources/bundled/locales/zh-CN.yml` included in platform release artifacts |

Explicit non-goals remain untranslated: PTY output, shell command output, file contents opened by the user, keyboard shortcut glyphs, protocol/log/debug identifiers, telemetry event names, test fixtures, and developer-only tracing.

Coverage is verified by a static extractor that scans Rust call sites for remaining user-visible string literals in the required areas and by locale parity checks against `en.yml`. Any intentional exception must be listed in a checked-in allowlist with a short reason.

### 6. Application integration

`init_locale()` is called at the top of `app::run()` (`app/src/lib.rs:610`), before feature flags are initialized and before any UI is created. This ensures translations are available for the entire application lifecycle.

The `app/src/i18n.rs` file re-exports `warp_i18n::*` so the rest of the application uses `crate::i18n::t()` without needing to depend on `warp_i18n` directly.

### 7. Windows compiler support

`windows-rs` crate macros generate code referencing `i18n::t()` on the `app` crate. Windows resource compilation works correctly because no i18n call appears in a const-evaluation context.

## Testing and validation

### Security-sensitive translation review (manual)
- Strings in security-relevant UI surfaces must be manually reviewed by a maintainer before merge. These surfaces include: auth dialogs (sign-in, sign-up, permission prompts), billing pages (pricing, plan descriptions, credit consumption warnings), sharing/permission controls (access grant text, visibility labels), and agent-mode consent prompts (data-handling disclosures, handoff confirmations).
- The reviewer verifies that Chinese translations preserve the legal and behavioral intent of the original English text — warnings are not weakened, consent semantics are not altered, and permission descriptions remain accurate.
- A checklist of security-sensitive keys is maintained alongside the locale files for targeted review. It must include exact YAML sections and prefixes for auth, billing, sharing, agent consent, and data-handling surfaces: `auth.*`, `billing.*`, `billing_ext.*`, `teams.*`, `privacy.*`, `shared_session.*`, `agent_management.*`, `hoa_onboarding.*`, `ai_ext.grant_access_files`, `ai_ext.grant_access_repository`, `ai_ext.missing_github_auth`, `ai_ext.authenticate_github`, `ai_ext.hand_off_to_cloud`, `ai_ext.handoff_to_cloud`, `ai_ext.hand_off_to_environment`, `ai_output.manage_ai_autonomy_permissions`, `ai_output.read_mcp_resource_permission`, `ai_output.upload_artifact_permission`, `ai_output.manage_agent_permissions`, and `ai_output.use_computer_permission`. When new permission, handoff, data-retention, or account/billing warning keys are added, the checklist must be updated in the same PR.
- Every key in that checklist must be reached through `t_required!()` or an explicit fail-closed path. A CI lint rejects security-sensitive keys rendered through plain `t!()` and rejects `t_required!()` calls whose fallback is not a string literal.

### Locale file integrity (automated)
- Every key present in `en.yml` must have a corresponding key in `zh-CN.yml`. A script or build-time check verifies this invariant — missing keys in `zh-CN.yml` cause a CI failure.
- **Interpolation placeholder parity:** for every key whose English value contains `{name}` placeholders, the zh-CN value must contain the exact same set of placeholder names (same count, same names). Mismatched placeholder names (e.g., en has `{count}` but zh-CN has `{number}`) produce runtime rendering bugs and must be rejected at CI time.
- Both YAML files must parse successfully as valid YAML and produce the expected top-level locale key (`en:` / `zh-CN:`).
- No orphaned keys: every key referenced by a `t!()` or `t_required!()` call in the codebase must exist in `en.yml`. A static analysis script (e.g., `rg 't(_required)?!\("([^"]+)"' --only-matching | sort -u` diffed against keys extracted from `en.yml`) must run locally and in CI to catch callsite-locale drift.
- The number of keys in `en.yml` and `zh-CN.yml` must be equal (after accounting for any intentionally untranslatable keys).
- No stale locale keys: every key in `en.yml` is either referenced by `t!()` / `t_required!()` or listed in a checked-in allowlist for platform/resource-only strings.
- No unlocalized required-scope strings: the static extractor scans the migration-scope directories for UI literals passed to common label/button/menu/tooltip/modal APIs without `t!()` or `t_required!()`. Remaining literals must be intentionally allowed with a reason.
- Release bundle check: every packaged app artifact must contain both `resources/bundled/locales/en.yml` and `resources/bundled/locales/zh-CN.yml` at the path searched by release builds. CI fails if either file is missing from macOS, Windows, or Linux packaging outputs.
- Dev-only locale loading check: release binaries must not reference `$WARP_LOCALES_DIR`, `$PWD/resources/bundled/locales`, or source-tree fallback paths. A binary/string or build-time assertion verifies those discovery paths are compiled out outside `debug_assertions`.

### Unit tests

- `warp_i18n` must include tests for:
  - `t()` with a key present in both locales returns the current locale's value
  - `t()` with a key present only in English falls back to English
  - `t()` with a missing key returns the key string itself
  - `t_required()` with a missing key returns the embedded English fallback, never the raw key
  - `interpolate()` correctly substitutes one and multiple placeholders
  - `set_locale("zh-CN")` correctly switches the active locale
  - `set_locale("fr")` falls back to `en`
  - locale normalization handles `zh_CN.UTF-8`, `zh_CN.utf8`, `zh_CN.UTF-8@pinyin`, mixed-case `zh-hans-cn`, and colon-separated `LANGUAGE` lists
  - `WARP_LANG=fr` forces `en` even when lower-priority locale sources are Simplified Chinese
  - `zh-TW`, `zh-HK`, and `zh-Hant*` resolve to `en`
  - `load_dir()` correctly parses YAML and produces flattened keys
  - malformed or over-size dev locale files are skipped without panicking

### Integration / manual verification

Manual verification must cover platform resource loading and locale detection, not just a local dev run:

| Platform | Scenario | Expected result |
|----------|----------|-----------------|
| macOS release artifact | `WARP_LANG=zh-CN` | menu bar, settings, agent UI, tooltips, notifications, onboarding entry points render zh-CN |
| macOS release artifact | `WARP_LANG=zh_CN.UTF-8` | same zh-CN result |
| macOS release artifact | `WARP_LANG=zh-TW` | English UI |
| Windows MSVC release artifact | Simplified Chinese system locale with no `WARP_LANG` | zh-CN UI after bundled resource load |
| Windows MSVC release artifact | `WARP_LANG=fr` on Simplified Chinese system locale | English UI, proving explicit override behavior |
| Linux release artifact | `LANG=zh_CN.UTF-8` | zh-CN UI |
| Linux release artifact | `LANGUAGE=fr:zh_CN:en` with `WARP_LANG` unset | zh-CN UI |
| All platforms | bundled locale directory missing or unreadable | app launches; ordinary UI may show raw keys; security-sensitive surfaces show embedded English fallback or disabled fail-closed action |
| All platforms | terminal PTY output under zh-CN UI | shell output is unchanged |
| All platforms | deliberate ordinary missing key in a test-only build | raw key renders without panic |

### Regression prevention

- The `cargo check` / `cargo build` pipeline for the `warp-oss` binary must pass on macOS, Windows MSVC, and Linux
- All existing tests must pass after the migration — no test assertions may be broken by i18n
- Behavior invariants from `PRODUCT.md` map to verification steps above
- CI must run the locale integrity script, security-sensitive key lint, release bundle check, and locale normalization tests before the implementation PR can merge

## Parallelization

Not applicable. The i18n work is a single cohesive change across the codebase — string replacements, locale file authoring, and framework implementation are all tightly coupled and should be done in a single branch by a single author.
