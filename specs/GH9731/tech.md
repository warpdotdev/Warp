# TECH.md — File Tree Icon Themes

**GitHub Issue:** [warpdotdev/warp#9731](https://github.com/warpdotdev/warp/issues/9731)
**Product Spec:** `specs/GH9731/product.md`

## Problem

The Project Explorer currently asks `app/src/code/icon.rs` for a file icon by path. That function hard-codes extension matches and returns either bundled SVG image elements or `None`, causing the file tree to fall back to a generic `Icon::File`. Folder rows are always rendered with `Icon::Folder` in `app/src/code/file_tree/view/render.rs`. There is no setting, theme model, data format, or folder-name/open-state lookup.

The implementation needs to replace this fixed lookup with a selectable icon-theme abstraction while keeping the first release small: bundled themes only, v1 contributed source format based on Nerd Font glyph/codepoint maps, and an abstraction that can add SVG paths later without changing the mapping model.

## Relevant Code

- `app/src/code/icon.rs` — current hard-coded file extension lookup and bundled SVG rendering.
- `app/src/code/file_tree/view/render.rs` — converts `FileTreeItem` into `RenderState`, calls `icon_from_file_path` for files, and uses `Icon::Folder` for directories.
- `app/src/code/file_tree/view.rs (1808-1875)` — renders `RenderState.icon` into the fixed 16x16 icon slot and applies selected/hovered colors.
- `app/src/ui_components/item_highlight.rs:59` — `ImageOrIcon` enum currently distinguishes app icons from arbitrary image elements.
- `app/src/search/files/icon.rs` — global file search reuses `crate::code::icon_from_file_path` and falls back to `completion-file.svg`.
- `app/src/settings/theme.rs` — globally syncable appearance theme settings and `ThemeKind` storage pattern.
- `app/src/settings/code.rs` — existing Project Explorer enablement setting under `code.editor.show_project_explorer`.
- `app/src/settings_view/appearance_page.rs` — Settings → Appearance actions, widgets, dropdowns, and theme-selection UI patterns.
- `app/src/user_config/mod.rs` and `app/src/user_config/native.rs` — existing config/catalog loading and managed-path watcher patterns for terminal themes, workflows, launch configs, and tab configs.
- `app/src/user_config/util.rs` — YAML/TOML parsing helpers and recursive config directory traversal used by theme and tab-config loading.
- `crates/lsp/src/config.rs (25-75)` — current `LanguageId::from_path` mapping that can be reused for known language IDs, with a clear limitation that it covers only LSP-supported languages today.
- `app/src/code/file_tree/view/view_tests.rs` — file tree unit-test harness.
- `crates/integration/src/test/file_tree.rs` — existing Project Explorer integration tests for opening files, context menus, and keyboard navigation.

## Current State

### File icons

`icon_from_file_path(path, appearance)` extracts the path extension and matches a fixed set of extensions:

- Rust, JSON, TypeScript/TSX, JavaScript/JSX, Python, C/C++, Go, Markdown, shell, Kotlin, PHP, Perl, Cython, Flash, WASM, Zig, SQL, Angular, and Terraform.
- Most matches return `Image::new(AssetSource::Bundled { path: "bundled/svg/file_type/..." }, CacheOption::BySize)`.
- Markdown and shell use monochrome `Icon::new(...)` tinted with the active terminal theme text color.
- Unknown extensions return `None`.

### File tree rendering

`FileTreeItem::to_render_state` builds a `RenderState` containing display name, icon, expansion state, depth, mouse state, drag state, and ignored state.

- File rows use `icon_from_file_path(...).map(ImageOrIcon::Image).unwrap_or(ImageOrIcon::Icon(Icon::File))`.
- Directory rows always use `ImageOrIcon::Icon(Icon::Folder)`.
- `render_item_with_hover` renders `ImageOrIcon::Icon` through `Icon::to_warpui_icon(icon_color)` and renders `ImageOrIcon::Image` as-is. This means current image-backed file type icons are not recolored by selected/hovered row state, while generic app icons are tinted.

### Settings and config

Terminal color themes are selected through `ThemeSettings` and rendered in `AppearanceSettingsPageView`. User-provided terminal themes are loaded from `warp_core::paths::themes_dir()` into `WarpConfig` and watched by `WarpManagedPathsWatcher`.

For this feature, v1 should not add local user-defined loading. It can still reuse the same settings and validation patterns:

- a syncable setting type with a stable serialized value;
- a picker/dropdown widget in `appearance_page.rs`;
- a typed catalog of bundled definitions;
- tests that validate bundled data at build/test time.

## Proposed Changes

### 1. Introduce a file icon theme module

Add a dedicated module under `app/src/code/`, for example:

- `app/src/code/icon.rs` — keep the public render entry points or re-export from the new module.
- `app/src/code/icon_theme.rs` — theme data model, bundled catalog, resolution, and rendering helpers.
- Optional test file: `app/src/code/icon_theme_tests.rs`.

Core types:

- `FileIconThemeId` — stable setting value, with variants such as `WarpDefault` and `Seti`.
- `FileIconTheme` — display name, ID, icon definitions, and mapping tables.
- `IconDefinitionId` — string/newtype key referenced by mapping tables.
- `IconDefinition` — v1 glyph definition with optional color and reserved future fields.
- `IconSource` — internal enum such as `Glyph { text, color }` plus a compatibility `BundledSvg { path, tint }` variant if needed to preserve Warp Default without changing existing SVG rendering.
- `FileIconLookupInput` — path-derived lookup data for files.
- `FolderIconLookupInput` — name plus open/closed state for folders.
- `ResolvedFileTreeIcon` — render-ready representation that can become a `Box<dyn Element>` or existing `ImageOrIcon` variant.

The public API should make call sites explicit about files vs folders:

- `icon_for_file_path(path: &str, appearance: &Appearance, app: &AppContext) -> ResolvedFileTreeIcon`
- `icon_for_folder_path(path: &str, is_expanded: Option<bool>, appearance: &Appearance, app: &AppContext) -> ResolvedFileTreeIcon`

If passing `AppContext` through the existing render path is too invasive, the selected theme can be looked up before `to_render_state` and passed as an explicit reference. Avoid hiding settings access in code that is hard to test.

### 2. Define the v1 bundled theme schema

Represent bundled themes as Rust constants or checked-in data files. Prefer checked-in TOML or JSON if maintainers want community PRs to modify theme mappings without touching Rust. Prefer Rust constants only if build-time asset wiring makes file parsing too costly for v1.

Recommended data shape:

```json
{
  "id": "seti",
  "name": "Seti-style",
  "iconDefinitions": {
    "rust": { "glyph": "\ue7a8", "color": "#dea584" },
    "folder": { "glyph": "\ue5ff", "color": "#6d8086" },
    "folderOpen": { "glyph": "\ue5fe", "color": "#6d8086" }
  },
  "fileNames": {
    "Cargo.toml": "rust",
    ".gitignore": "git"
  },
  "fileExtensions": {
    "rs": "rust"
  },
  "languageIds": {
    "rust": "rust"
  },
  "folderNames": {
    ".git": "gitFolder",
    "node_modules": "nodeFolder",
    "src": "srcFolder"
  },
  "folderNamesExpanded": {
    "src": "srcFolderOpen"
  },
  "file": "file",
  "folder": "folder",
  "folderExpanded": "folderOpen"
}
```

Notes:

- The source format uses glyph strings/codepoints in v1.
- Colors should parse as hex colors into `ColorU`.
- All mapping targets must reference existing `iconDefinitions`.
- File extensions should be stored normalized without a leading dot.
- Exact file names should support dotfiles and compound names like `Cargo.toml`, `package.json`, `tsconfig.json`, and `Dockerfile`.
- `languageIds` should use lower-case IDs compatible with existing or future language detection. Current `lsp::LanguageId` covers only a small set, so this should be an optional layer after exact filename and extension, not the only source of language coverage.
- Reserve a compatible future field such as `svg` inside `IconDefinition`, but reject or ignore it in v1 unless the renderer supports it.

#### Seti-style provenance and attribution (pre-ship)

The Seti-style bundled theme is data-only and depends on two upstream sources, both MIT-licensed:

- **Glyph codepoints**: Nerd Fonts (https://github.com/ryanoasis/nerd-fonts). Only codepoint strings are vendored; no font binary is bundled.
- **Mapping and color palette**: Seti-UI by Jesse Weed (https://github.com/jesseweed/seti-ui). The `seti-ui/styles/components/icons/mapping.less` table and the matching color list are the canonical references.

Implementation requirements before the Seti-style theme can land:

1. The Seti-style data file carries an `attribution` field (or sibling metadata) naming both upstreams, linking their repositories, **and recording a full 40-character commit SHA for each**. Commit SHAs are required because release tags are not immutable (upstream can force-move a tag) and license review must be reproducible against the exact tree that was vendored. A release tag may appear alongside the SHA as optional display metadata (for example `seti-ui@<40-char SHA> (v1.2.3)`), but the SHA is the pin and the tag alone is not acceptable. Re-vendoring requires bumping the SHA in the same PR that touches the data.
2. `THIRD_PARTY_NOTICES` (or the Warp equivalent) gains MIT entries for Seti-UI and Nerd Fonts with copyright lines preserved, and each entry includes the same 40-character commit SHA recorded in `attribution` (release tag optional alongside).
3. The Settings → Appearance picker row for Seti-style shows attribution copy ("Based on Seti-UI by Jesse Weed" or the agreed final wording); implement via existing dropdown tooltip / description plumbing. Display does not need to include the pinned revision, but the picker copy must not contradict the recorded sources.
4. The PR landing the Seti-style theme links both upstream sources in the description and changelog entry, and quotes the pinned revisions so license review is reproducible from the PR alone.
5. No font file, no SVG asset, no other binary form of either upstream is committed in v1. If glyphs render as missing-glyph boxes for users without a Nerd Font, the user can switch back to Warp Default.
6. Theme load-time validation rejects a Seti-style data file whose `attribution` block is missing a 40-character commit SHA for either upstream. A release tag without a SHA is rejected. The validator should match `^[0-9a-f]{40}$` against the SHA portion of each entry. This prevents the bundled theme from drifting away from a reviewable provenance even if a maintainer accidentally records a tag-only pin.

If a future revision adds SVG support and Seti-style adopts SVG assets from any source, those go through a separate provenance review; this spec does not pre-approve future SVG sources.

### 3. Preserve Warp Default

Warp Default must not surprise existing users. Implement it as one of these approaches:

1. **Compatibility-backed built-in theme:** move the existing `match extension` table into a `WarpDefault` theme whose icon definitions use an internal `BundledSvg` source. This keeps current visuals and requires no new SVG asset pipeline.
2. **Glyph-backed default theme:** create a Nerd Font mapping that approximates today's icons. This is more aligned with the v1 contributed file format but may visibly change existing default icons.

Prefer option 1 unless maintainers explicitly accept default visual changes. The documented contributed format can still remain glyph-only for v1; the internal `BundledSvg` variant is an implementation detail to preserve current behavior.

### 4. Implement resolution order

Normalization rules (mirrors the product spec's name-matching contract; theme-loading and the lookup paths must agree):

- File extensions: case-insensitive over **lowercase ASCII keys only** in v1. Theme keys are stored lowercased, without a leading dot, and matching the regex `[a-z0-9]+`. The lookup uses `Path::extension`, `to_str()`, then `to_ascii_lowercase()` to form the key, then performs an exact match against the lowercased ASCII key set. An extension on disk that contains non-ASCII bytes after the conversion attempt simply misses the `fileExtensions` table and falls through to language ID and then file fallback; v1 does not implement Unicode case-folding for extensions. Empty extensions are not used as keys. Wider Unicode extension support is a documented follow-up.
- Exact file names: case-sensitive. Theme keys are stored verbatim; the lookup uses `Path::file_name` and matches byte-for-byte. Non-ASCII file names are supported because the comparison is byte-for-byte and does not depend on case folding.
- Folder names: same case-sensitive, verbatim rule as exact file names, for both `folderNames` and `folderNamesExpanded`. Non-ASCII folder names are supported on the same byte-for-byte basis.
- Language IDs: lowercase ASCII (`[a-z0-9_-]+`). Theme keys are stored lowercased; lookup lowercases input via `to_ascii_lowercase()`.
- No trimming, Unicode normalization, or whitespace collapsing in v1.
- Theme load-time validation must reject keys that violate these rules (uppercased or non-ASCII extension keys, leading-dot extension keys, non-ASCII language IDs) so bundled themes cannot drift.

File resolution:

1. Extract exact file name. Check `fileNames` (case-sensitive, verbatim). This handles dotfiles and extensionless names.
2. Extract extension. Lowercase it. Check `fileExtensions`.
3. Derive language ID from `lsp::LanguageId::from_path` where available and lowercase the resulting string. Look it up in `languageIds`. If `lsp_language_identifier()` is `pub(crate)`, either expose a safe public string method or maintain a parallel string table for theme purposes.
4. Use file fallback.

Folder resolution:

1. Extract folder name from the path. Match case-sensitive, verbatim.
2. If expanded/open, check `folderNamesExpanded` for that name.
3. Check `folderNames`.
4. If expanded/open, use folder-open fallback.
5. Otherwise use folder fallback.

The resolution code should return a fallback icon for every file/folder path. Mapping-target validation runs at theme-load/test time, not on the render hot path; runtime should log invalid data at most once per process startup rather than spamming logs during rendering.

### 5. Rendering glyph icons

Extend `ImageOrIcon` or introduce a file-tree-specific render enum to support glyph icons:

- `AppIcon(Icon)` for existing internal icons.
- `Image(Box<dyn Element>)` for existing SVG/image-backed icons.
- `Glyph { text: String, color: Option<ColorU> }` for Nerd Font themes.

Render glyph icons in the same 16x16 slot used today. The implementation can use a `Text`/formatted text element with the user's monospace font family, centered in the icon box, and sized to align with current row height. If the theme provides a color, use it for normal rows and ensure selected/hovered rows remain readable. If the row selection treatment conflicts with per-icon colors, selected rows may tint icons with the row text/icon color while normal rows use theme colors.

Avoid adding a custom renderer unless existing text elements cannot align glyphs acceptably. The implementation should not change row height, indentation, chevron rendering, or text layout.

### 6. Add settings storage

Add a syncable setting for the selected file icon theme. Reasonable options:

- Add a new setting to `ThemeSettings` in `app/src/settings/theme.rs`, because the picker lives in Appearance and the value affects presentation.
- Or create a focused `FileIconThemeSettings` group if maintainers prefer not to grow terminal theme settings.

Recommended setting behavior:

- type: `FileIconThemeId`
- default: `WarpDefault`
- supported platforms: desktop/all platforms where Project Explorer can render
- sync: global, respecting user sync settings
- TOML path: `appearance.file_tree.icon_theme` or `appearance.icon_theme.file_tree`
- description: "The icon theme used for the Project Explorer file tree."
- unknown values: fall back to default and surface schema validation as existing settings infrastructure allows

Add `schemars::JsonSchema`, `serde::{Serialize, Deserialize}`, and `settings_value::SettingsValue` derives as needed for the settings macro and schema tests.

### 7. Add Settings → Appearance UI

Update `app/src/settings_view/appearance_page.rs`:

- Add `AppearancePageAction::SetFileIconTheme(FileIconThemeId)`.
- Add a dropdown handle to `AppearanceSettingsPageView` if following the existing dropdown pattern.
- Build dropdown items from the bundled catalog's display names.
- Add a `FileTreeIconThemeWidget` with search terms from the product spec.
- Render the widget under an appropriate Appearance section near terminal theme selection or other visual presentation settings.
- On change, set the new setting and notify so visible file trees rerender.

If the theme picker should show icon previews, keep that as a follow-up unless the existing dropdown can cheaply include a small preview. The v1 requirement is a picker/dropdown, not a full theme chooser modal.

### 8. Wire file tree rendering

Update `app/src/code/file_tree/view/render.rs`:

- Files should resolve through the selected `FileIconTheme`.
- Directories should call folder resolution with `is_expanded`.
- `RenderState` should carry the new render enum instead of forcing every themed icon into `ImageOrIcon::Image` or `ImageOrIcon::Icon`.

Update `app/src/code/file_tree/view.rs`:

- Render the new icon enum in `render_item_with_hover`.
- Keep selected/hovered/ignored styling behavior intact.
- Ensure `render_item_while_dragging` uses the same theme as normal rows.

Because `FileTreeItem::to_render_state` currently receives `Appearance` but not `AppContext`, either:

- change it to receive `AppContext` or the selected theme directly; or
- resolve the theme outside and pass a `&FileIconTheme` through.

Prefer passing explicit theme data for testability.

### 9. Apply the selected theme to global file search

The product spec resolves this: in v1, global file search file results render the selected File Tree icon theme. `app/src/search/files/icon.rs` currently reuses `crate::code::icon_from_file_path`, so the cleanest implementation routes both Project Explorer and search through the new `icon_for_file_path` API.

- Do not introduce a Project-Explorer-only API in v1; that would require duplicating the resolution code or fork-then-merge later.
- Search currently renders only file results; folder resolution does not need a search-side path.
- Add a unit/integration test that confirms the search call site resolves through the same `FileIconTheme` instance the file tree uses, and that switching the theme is observable in search-result render data.

### 10. Validation and error handling

Add validation for bundled themes:

- every mapping target exists in `iconDefinitions`;
- required fallbacks exist;
- color strings parse;
- theme IDs are unique;
- setting values map to a known bundled theme;
- exact filename and extension matching precedence is deterministic.

Runtime behavior:

- invalid or missing selected theme falls back to Warp Default;
- missing mapping falls back to file/folder fallback;
- glyph render issues do not prevent row text from rendering.

## End-to-End Flow

### Selecting a theme

1. User opens Settings → Appearance.
2. User selects File Tree icon theme = Seti-style.
3. `AppearancePageAction::SetFileIconTheme(Seti)` updates the setting.
4. Settings change notification causes affected views to rerender.
5. Project Explorer rows resolve icons through the Seti-style catalog.
6. The setting is saved and restored on next launch.

### Rendering a file row

1. File tree flattens repository metadata into `FileTreeItem::File`.
2. Render path extracts file name, extension, and optionally language ID from the file path.
3. Selected theme resolves exact filename, then extension, then language ID, then file fallback.
4. Resolved icon is converted into a row element in the existing icon slot.
5. Existing file name, ignored styling, selection, hover, click, keyboard, context menu, and drag/drop behavior continue unchanged.

### Rendering a folder row

1. File tree flattens repository metadata into `FileTreeItem::DirectoryHeader`.
2. Render path passes folder name and expansion state to the selected theme.
3. Selected theme checks open-specific folder name mapping, folder name mapping, open fallback, then closed fallback.
4. Resolved folder glyph/image is rendered next to the existing expand/collapse chevron.

## Risks and Mitigations

1. **Default visual regression.** Moving hard-coded icons into a theme could accidentally change current icons. Mitigate with Warp Default snapshot/unit tests and by using an internal bundled-SVG compatibility source if necessary.
2. **Nerd Font glyph availability.** Users without a compatible font may see missing glyph boxes in the Seti-style theme. Mitigate by keeping Warp Default as the default and noting Nerd Font expectations in the setting/docs.
3. **Settings placement ambiguity.** The picker belongs visually in Appearance, while Project Explorer enablement is under Code. Mitigate by placing the control in Appearance and adding search terms that include Project Explorer/File Tree.
4. **Language ID coverage.** Existing `lsp::LanguageId` is intentionally small. Mitigate by relying primarily on exact filename and extension mappings and treating language IDs as an additional layer.
5. **Render performance.** Resolving icons during row rendering could happen often. Mitigate with pre-normalized `HashMap`s in the bundled theme catalog and cheap path parsing; avoid per-row JSON parsing.
6. **Theme data drift.** Community-added mappings can reference missing definitions. Mitigate with tests that validate all bundled themes.
7. **Search behavior surprise.** Shared icon lookup could change global file search icons unintentionally. Mitigate by making the search scope decision explicit in the implementation and tests.

## Testing and Validation

1. Unit tests for file resolution precedence:
   - exact filename beats extension;
   - extension beats language ID;
   - language ID beats fallback;
   - dotfiles and extensionless files work;
   - extension matching is case-insensitive.
2. Unit tests for folder resolution:
   - open-specific folder mapping wins for expanded folders;
   - folder name mapping works for closed folders;
   - `.git`, `node_modules`, `src`, and `dist` are data-driven;
   - folder fallback and folder-open fallback always resolve.
3. Unit tests for bundled catalog validation:
   - all mapping references exist;
   - required fallbacks exist;
   - all colors parse;
   - IDs are unique.
4. Settings tests:
   - default selected theme is Warp Default;
   - setting schema validates;
   - unknown values fall back safely where existing settings infrastructure supports that behavior.
5. File tree view tests using `app/src/code/file_tree/view/view_tests.rs` harness:
   - file and folder rows produce themed render states for a sample repo;
   - changing the selected theme changes the resolved icon source without changing row identity/position.
6. Integration/manual validation using `crates/integration/src/test/file_tree.rs` patterns:
   - open Project Explorer, switch theme, click file, context-menu open in new pane/tab, keyboard navigate and press Enter.
7. Manual visual validation:
   - capture Project Explorer with Warp Default and Seti-style for the same sample workspace;
   - verify no row height/alignment regressions and selected/hovered rows stay readable.
8. Search behavior:
   - the search-side icon call site routes through the new `icon_for_file_path` API and resolves through the same `FileIconTheme` instance as the file tree;
   - switching the theme is observable in search-result render data without restart.
9. Theme-load validation:
   - rejects extension keys with leading dots or uppercase letters;
   - rejects non-ASCII / non-lowercase language ID keys;
   - rejects mapping references to missing icon definitions;
   - Seti-style fixture carries the required `attribution` metadata and parses successfully.

## Follow-ups

- Load user-defined icon themes from a discoverable local config folder.
- Add SVG fields to `iconDefinitions` and prefer SVG when the renderer supports it.
- Add a richer theme picker with previews similar to the terminal theme chooser.
- Publish docs for adding bundled icon themes and mapping common file/folder names.
- Expand language ID detection beyond current LSP-supported languages.
