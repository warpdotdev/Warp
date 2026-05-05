# Technical spec: Generic syntax-highlight definition mechanism (GH-9955)

This spec is the implementation companion to `product.md`. It picks
the discovery mechanism, the schema, the loader architecture, and
the migration strategy for the 32 existing languages.

## Current state (recap from product.md investigation)

- `crates/languages/src/lib.rs` defines `SUPPORTED_LANGUAGES: [&str;
  32]` (line 23), `language_by_name`, `language_by_filename`,
  `to_arborium_name`, `get_arborium_highlight_query`, `load_language`.
- `crates/languages/grammars/<lang>/` provides per-language
  `config.yaml`, `identifiers.scm`, `indents.scm`. Embedded via
  `RustEmbed`.
- `arborium` (internal crate) provides parsers and bundled
  highlight queries via `arborium::lang_<X>::HIGHLIGHTS_QUERY`
  consts.
- Consumers of `Language`: `crates/syntax_tree/src/queries/`
  (highlight query, indent query), `app/src/code/editor`, the AI
  context indexers, the workflow view's `syntax_highlightable.rs`.

## Architecture overview

Add a `LanguageRegistry` discovery layer that loads from THREE
sources, in priority order:

1. **Compile-time hardcoded** (existing path) — the current
   `to_arborium_name` / `get_arborium_highlight_query` matches.
   This is the "first-class" path; it stays for the existing 32
   languages until per-language migration PRs convert them.
2. **Bundled directory** — `crates/languages/grammars/<lang>/` with
   a `language.toml` driving discovery. New languages can be added
   here without touching `lib.rs`.
3. **User-local directory** — `~/.warp/grammars/<lang>/` (or
   `$XDG_CONFIG_HOME/warp/grammars/<lang>/`). WASM-only grammars,
   loaded at startup.

A bundled grammar takes precedence over a user-local one with the
same `internal_name` (B2 invariant). A compile-time hardcoded
language takes precedence over a bundled grammar with the same
name; this gives us the staged migration path of B4.

## `language.toml` schema

```toml
schema_version = 1

[language]
display_name = "Nim"
internal_name = "nim"
comment_prefix = "#"
indent_unit = { spaces = 2 }       # or { tabs = 1 }

[file_associations]
extensions = ["nim", "nims"]
filenames = ["nim.cfg"]
shebangs = ["nim"]
aliases = ["nim-lang"]

[brackets]
pairs = [
  { start = "(", end = ")" },
  { start = "{", end = "}" },
  { start = "[", end = "]" },
]

[parser]
# Bundled grammars: a Cargo dep on a tree-sitter grammar crate.
# Mutually exclusive with [parser.wasm].
rust_crate = "tree-sitter-nim"

# User-local grammars: WASM file path relative to the grammar dir.
# Mutually exclusive with [parser.rust_crate].
wasm = "grammar.wasm"

# Optional: pin the tree-sitter ABI version this grammar was
# compiled against; loader rejects mismatches with a clear error.
ts_abi = 14
```

The schema lives in a new module `crates/languages/src/schema.rs`
with `serde::Deserialize` derives and a `validate()` method that
rejects mutually-exclusive parser fields, missing files, and
unknown bracket characters.

## Loader architecture

### `crates/languages/src/loader.rs` (new)

```rust
pub enum LanguageSource {
    Hardcoded,
    Bundled { dir: PathBuf },
    UserLocal { dir: PathBuf },
}

pub struct LoadedLanguage {
    pub language: Arc<Language>,
    pub source: LanguageSource,
    pub failure: Option<LoadFailure>,
}

pub struct LoadFailure {
    pub directory: PathBuf,
    pub reason: String,
    pub schema_version: Option<u32>,
}

pub fn discover_grammars() -> Vec<LoadedLanguage> { ... }
```

`discover_grammars()` is called once at startup. It walks the three
sources in priority order, deduplicates by `internal_name`, and
returns the merged list with any failures attached.

### Loading a single grammar

For each grammar directory:
1. Parse `language.toml` (`schema.rs::parse`). On failure: record
   `LoadFailure`, return.
2. Resolve the parser:
   - `rust_crate`: look up via a compile-time-built map of crate
     name → `ParserGrammar` (this map is the only hand-edited list
     for bundled grammars; it lives in
     `crates/languages/src/bundled_parsers.rs` and is the
     mechanical PR follow-up B4 references).
   - `wasm`: `tree_sitter::WasmStore::load_language(&wasm_bytes)`.
     Reject if `WasmStore` reports an ABI mismatch with the host's
     `tree_sitter::TREE_SITTER_LANGUAGE_VERSION`.
3. Compile `highlights.scm` against the resolved grammar via
   `Query::new`. On failure: record `LoadFailure`, return.
4. Compile `indents.scm` and `identifiers.scm` (optional).
5. Construct the `Language` struct with all fields populated.
6. Return as `Ok(LoadedLanguage)`.

### Native dynamic-library rejection

The loader explicitly checks the `parser` table for any field other
than `rust_crate` or `wasm`. If a `native_lib = "grammar.so"` field
is present (or any unknown field starting with `dl` / `native` /
`so` / `dylib` / `dll`), the loader rejects the grammar with the
B5 error message and never attempts to open the file. No
`libloading::Library::new` call exists anywhere on the loader path.

### File-association registration

After all grammars load, the loader populates two maps:

```rust
struct AssociationIndex {
    by_extension: HashMap<String, Arc<Language>>,
    by_filename:  HashMap<String, Arc<Language>>,
    by_shebang:   HashMap<String, Arc<Language>>,
    by_alias:     HashMap<String, Arc<Language>>,
    by_internal_name: HashMap<String, Arc<Language>>,
}
```

The hardcoded path (the current `language_by_filename` match) is
queried first; if it returns `None`, fall through to the
`AssociationIndex`. This keeps existing behavior identical for the
32 languages until they migrate.

## Public API changes

`crates/languages/src/lib.rs`:

- `language_by_name(name: &str)` — unchanged signature; internally
  consults hardcoded match first, then `AssociationIndex.by_internal_name`
  / `by_alias`.
- `language_by_filename(path: &Path)` — unchanged signature;
  consults hardcoded path first, then `AssociationIndex` extension
  / filename / shebang lookups.
- New: `loaded_languages() -> &[LoadedLanguage]` — for the new
  Settings → Editor → Languages page.

No change for any current consumer; they continue to call the same
two functions.

## Settings page integration

`Settings → Editor → Languages` (new sub-page):

- Lists each loaded language: display name, internal name, source
  (Hardcoded / Bundled / User-local), file extensions claimed,
  parser revision (from `language.toml` `ts_abi`).
- Lists each failed grammar: directory, reason.
- A "Reveal in Finder/Files" button next to user-local grammars.
- A "Refresh after restart" pill at the top reminding users that
  changes require restart (V1 has no hot-reload).

## Migration strategy for the 32 existing languages

Each existing language migrates in one mechanical PR with the
following steps:

1. Create `crates/languages/grammars/<lang>/language.toml` with the
   file associations and parser reference matching the current
   hardcoded behavior.
2. Move the `arborium::lang_X::HIGHLIGHTS_QUERY` const into a
   `highlights.scm` file in the same directory.
3. Remove the language's arms from `to_arborium_name`,
   `get_arborium_highlight_query`, `language_by_filename`, and the
   `SUPPORTED_LANGUAGES` array.
4. Verify `crates/languages/src/lib_tests.rs` still passes.
5. Verify any language-specific indent / highlight tests in
   `crates/syntax_tree/` still pass.

The `bundled_parsers.rs` map gets one new entry per migration. The
priority-order rule (hardcoded > bundled) means a partial migration
is safe: an unmigrated language uses the hardcoded path; a migrated
one uses the bundled path. There is no flag day.

V1 of THIS PR migrates **zero** existing languages — only adds the
discovery mechanism beside them. Each follow-up PR migrates one
language and is independently revertable.

## Telemetry

- One-time `grammar_loaded` event at startup with `internal_name`,
  `source`, and `parser_kind` (rust_crate / wasm).
- One-time `grammar_load_failed` event with `internal_name` (if
  parseable), `reason_kind` (schema / parser / query / abi), and
  the path is **omitted** (PII risk).

## Test plan

### Unit tests (`crates/languages/src/schema_test.rs` — new)

- T1: Parse a minimal valid `language.toml` (display_name +
  internal_name + extensions + parser).
- T2: Parse a fully-populated `language.toml` and verify all fields
  round-trip.
- T3: Parser table with both `rust_crate` and `wasm` fields fails
  validation.
- T4: Parser table with `native_lib = "..."` is rejected with the
  B5 error message.
- T5: `schema_version` mismatch (e.g., 999) is rejected with a
  clear "unsupported schema version" error.

### Unit tests (`crates/languages/src/loader_test.rs` — new)

- T6: A bundled grammar with a stub WASM that fails to instantiate
  surfaces as a `LoadFailure` and does NOT panic.
- T7: A user-local grammar whose `internal_name` collides with a
  hardcoded language is dropped from the merged list and a warn
  fires.
- T8: A user-local grammar whose `internal_name` collides with a
  bundled grammar (after a hypothetical migration) is dropped;
  bundled wins.
- T9: ABI mismatch (host ABI 14, grammar declares ABI 13) surfaces
  as a `LoadFailure` with reason "abi mismatch".

### Integration test (`crates/languages/src/integration_test.rs` — new)

- IT1: Create a temp dir with a fixture grammar (a real
  tree-sitter-toml grammar shrunk to a minimal subset), point
  `WARP_USER_GRAMMAR_DIR` env var at it, call
  `discover_grammars()`. Assert the language is loaded and
  `language_by_filename(Path::new("test.example"))` returns it.
- IT2: Same as IT1 but with malformed TOML; assert the rest of the
  registry loads normally and the failure is reported.
- IT3: Call `loaded_languages()` after discovery and assert the 32
  hardcoded languages are present alongside the test fixture.

### Regression (existing test files unchanged)

- Existing `crates/languages/src/lib_tests.rs` (which iterates
  `SUPPORTED_LANGUAGES` and verifies each loads) must pass with no
  modifications. The 32 hardcoded languages remain in
  `SUPPORTED_LANGUAGES` until their migration PRs.
- Existing `crates/syntax_tree/src/queries/*_tests.rs` calling
  `language_by_filename` must pass with no modifications.

## Files touched

V1 (this PR — discovery mechanism only, zero migrations):

- `crates/languages/src/lib.rs` — fall-through call to the new
  `AssociationIndex` after the existing hardcoded match.
- `crates/languages/src/schema.rs` (new) — `language.toml` parser.
- `crates/languages/src/loader.rs` (new) — discovery + load.
- `crates/languages/src/bundled_parsers.rs` (new) — empty map
  initially; entries added per-migration.
- `crates/languages/src/association_index.rs` (new) — lookup maps.
- `crates/languages/Cargo.toml` — add `tree-sitter` (for
  WasmStore) and `toml` deps if not already present.
- `crates/languages/src/schema_test.rs` (new) — T1–T5.
- `crates/languages/src/loader_test.rs` (new) — T6–T9.
- `crates/languages/src/integration_test.rs` (new) — IT1–IT3.
- `app/src/settings_view/editor_languages_page.rs` (new) — the
  Languages sub-page.

V1 explicitly does NOT touch:
- The 32 hardcoded language arms.
- Any consumer of `Language`.
- `arborium` crate.

## Out-of-scope follow-ups (each independently revertable)

- Per-language migration PRs (one per language) moving from
  hardcoded to `crates/languages/grammars/<lang>/`.
- Sub-language injection mechanism (Vue, TSX, Markdown code
  blocks).
- Hot-reload of user-local grammars.
- Package-manager / cloud distribution of community grammars.
- LSP integration via a `[lsp]` section in `language.toml`.

## Open questions for maintainer review

1. WASM grammars require a tree-sitter version that supports
   `WasmStore`. Confirm the version Warp uses today (verify
   `Cargo.lock`) supports it.
2. The user-local grammar directory: `~/.warp/grammars/` vs.
   `$XDG_CONFIG_HOME/warp/grammars/`. Recommendation: XDG when set,
   fall back to `~/.warp/`.
3. Should the Settings → Editor → Languages page allow disabling
   individual loaded languages? Recommendation: yes, but a
   follow-up PR; not V1.
4. The `bundled_parsers.rs` compile-time map is the only hand-
   edited file for adding bundled grammars. Can we use an
   `inventory`-style auto-registration pattern instead? (Would
   eliminate the only remaining hand-edit but adds a build-time
   crate dependency.)
