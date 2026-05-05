# Product spec: Generic syntax-highlight definition mechanism (GH-9955)

## Problem

Adding a new language to Warp's syntax-highlighting today requires
changes in 5+ places, all in `crates/languages/src/lib.rs`:

1. The `SUPPORTED_LANGUAGES: [&str; 32]` array.
2. The `language_by_filename` extension-to-language match (one
   match arm).
3. The `to_arborium_name` aliasing match (only if the name differs).
4. The `get_arborium_highlight_query` match (one arm with a hard
   reference to `arborium::lang_X::HIGHLIGHTS_QUERY`).
5. A `crates/languages/grammars/<lang>/` folder with `config.yaml`,
   `identifiers.scm`, and `indents.scm`.

Steps 1–4 require modifying compiled Rust code, which means a new
language requires a Warp release. Step 4 also requires the language
to be supported by the upstream `arborium` tree-sitter aggregator
crate (which is itself an internal dependency of Warp), which means
adding a language Warp does not yet support requires either:
- Waiting for `arborium` to add it, or
- Vendoring a tree-sitter grammar into the Warp source tree.

The closed registry blocks the most-requested kind of community
contribution: "I use $LANG and would happily contribute the
highlighting definition." Today that contribution requires touching
internal crate dependencies and shipping a release.

The reporter explicitly cited this as the bottleneck for
distributing syntax-highlight work to individual contributors and
referenced Sublime Text, TextMate, Midnight Commander, and modern
tree-sitter-based editors as prior art for pluggable grammar
mechanisms.

## Goal

A contributor can add a new language to Warp's syntax highlighting
**without modifying compiled Rust code and without releasing Warp**,
by dropping a directory of files into either:
- the Warp source tree (a "bundled" community contribution that
  ships with the next release), or
- a user-local config directory (a "user-local" grammar that loads
  at startup on the contributor's machine).

The mechanism preserves Warp's tree-sitter substrate (the right
substrate; not switching to TextMate-style regex grammars), and the
existing 32 first-class languages keep working with no behavior
change.

## Non-goals (V1)

- **Switching away from tree-sitter.** Tree-sitter is the right
  substrate for accurate parsing. TextMate / Sublime grammars are
  regex-based and inferior for accuracy. They are referenced in the
  issue as community-distribution exemplars, not as recommended
  technology.
- **Runtime-compiled tree-sitter grammars (loadable .so/.dylib).**
  Loading native `.so` files is a security and portability hazard.
  V1 supports only **WASM-compiled** tree-sitter grammars and
  **vendored Rust grammars**; native dynamic loading is explicitly
  rejected.
- **Per-user theme / color-scheme definition mechanism.** This spec
  is about adding new languages to highlighting, not about styling
  the captures.
- **LSP integration mechanism.** The `Language` struct comment hints
  at LSP being the next addition; that is a separate spec.
- **Sub-language injection** (e.g. SQL inside a Python string,
  CSS inside a Vue template). Currently handled via the existing
  `vue` / `tsx` special cases. Out of V1.
- **Hot-reload of grammars.** Grammars load once at startup; user
  edits require a Warp restart. Hot-reload is a follow-up.
- **First-class user-contributed grammars in the cloud / via a
  package manager.** V1 is local files only. A package-manager
  layer can be built on top later.

## Behavior contract (V1)

### B1 — Drop-in directory definition

A new language is defined by a directory containing:
- `language.toml` — display name, file extensions, filename matches,
  alias names, comment prefix, brackets, indent unit.
- `highlights.scm` — tree-sitter highlight query.
- `indents.scm` — (optional) tree-sitter indent query.
- `identifiers.scm` — (optional) tree-sitter symbol query.
- `grammar.wasm` OR a `cargo` reference to a vendored Rust
  grammar — the parser itself.

The `language.toml` schema is the single contract a contributor
must learn. All other files are tree-sitter standard files.

### B2 — Two load paths: bundled and user-local

**Bundled:** A `crates/languages/grammars/<lang>/` directory is
discovered at compile time via the existing `RustEmbed` mechanism.
A new language directory is the only required Rust change; no
hand-written match arms.

**User-local:** A `~/.warp/grammars/<lang>/` (or
`$XDG_CONFIG_HOME/warp/grammars/<lang>/`) directory is discovered at
startup. User-local grammars are loaded after bundled grammars and
do not override them by default (preventing a malicious
user-grammar from masquerading as Rust). A user-local grammar
whose `language.toml` declares a name that collides with a bundled
language is logged at `warn` level and ignored.

### B3 — Schema-driven file association

`language.toml` declares its own filename / extension / shebang
patterns:

```toml
[language]
display_name = "Nim"
internal_name = "nim"

[file_associations]
extensions = ["nim", "nims"]
filenames = ["nim.cfg"]
shebangs = ["nim"]            # for `#!/usr/bin/env nim` scripts
aliases = ["nim-lang"]        # markdown ```nim-lang code blocks
```

The hand-coded `language_by_filename` and `normalize_language_name`
match statements are replaced with a registry-driven lookup. The
existing 32 languages get their associations migrated from the
match statements to per-language `language.toml` files in a
single mechanical PR (this spec calls out that PR as a follow-up,
not part of V1).

### B4 — Backwards compatibility for the existing 32 languages

The 32 existing languages continue to work bit-for-bit identically.
Their grammars stay in `arborium` (V1 does not vendor or rewrite
them). The discovery mechanism is added beside the existing
hardcoded match statements, not in place of them. A bundled
language defined via the new mechanism takes precedence over a
hardcoded one only after manual migration of that language
(staged migration; not a flag day).

### B5 — Security: WASM only for runtime-loaded grammars

User-local grammars must ship as `grammar.wasm`. Native dynamic
libraries (`.so`, `.dylib`, `.dll`) are explicitly rejected and
never loaded. The WASM is loaded via tree-sitter's existing WASM
runtime. WASM provides the sandboxing that makes user-local
grammars safe.

Bundled grammars (the Warp source tree) can be either WASM or a
Rust crate reference. The Rust crate reference is for the existing
`arborium` languages and for any future first-class language that
warrants a Cargo dependency.

### B6 — Validation and clear failure modes

A grammar directory that fails to load (malformed `language.toml`,
WASM that fails to instantiate, `highlights.scm` that fails to
parse against the grammar) does NOT break Warp startup. Instead:
- A `log::error!` fires with the directory path and the failure
  reason.
- A persistent in-app notification surfaces the failure (one
  notification per failed grammar, dismissible).
- The language is omitted from the registry but other languages
  load normally.

A grammar with valid `language.toml` and parser but a missing
`highlights.scm` loads as a syntax-tree-aware language with no
coloring (you still get bracket pairing, indent, etc.). This makes
a "minimum viable" grammar contribution low-effort.

### B7 — Discoverability of installed grammars

A new command `warp_grammars list` (or a settings-page surface,
Settings → Editor → Languages) shows:
- Each loaded language, its source (bundled / user-local), its
  parser revision, and the file extensions it claims.
- Each failed-to-load grammar with its failure reason.

This is the diagnostic surface a contributor uses to confirm their
new grammar loaded.

### B8 — Existing settings keys preserve forward compatibility

The existing `editor.indent_unit` per-language settings, the
`renderer.theme` highlight color mappings, and any other downstream
consumer of `Language` continues to work. The `Language` struct
stays the same shape; only its construction path changes.

## Acceptance criteria

A1. A contributor adds `crates/languages/grammars/nim/` containing
    `language.toml`, `highlights.scm`, `indents.scm`, and a Cargo
    dependency on a Nim tree-sitter grammar. After `cargo build`,
    `.nim` files render with syntax highlighting in Warp.
    No edits to `lib.rs` were required.

A2. A user drops `~/.warp/grammars/zig/` containing
    `language.toml`, `highlights.scm`, and `grammar.wasm`. After
    restarting Warp, `.zig` files render with syntax highlighting.

A3. The 32 existing languages render bit-for-bit identically to
    today. The existing test suite (`crates/languages/src/lib_tests.rs`,
    `crates/syntax_tree/src/queries/indent_query_tests.rs`) passes
    unchanged.

A4. A user-local `nim` grammar with a `language.toml` that
    collides with a future bundled `nim` (different file extensions
    too) is ignored; bundled wins. A `log::warn!` fires.

A5. A user-local grammar with malformed `language.toml` does not
    break Warp startup; the failure surfaces via the in-app
    notification and `Settings → Editor → Languages` view.

A6. An attempt to load a `grammar.so` (native dylib) from a
    user-local directory fails with a clear "native dynamic
    libraries are not supported; use grammar.wasm" error message.
    No `dlopen` is attempted.

A7. The new `Settings → Editor → Languages` page lists all loaded
    grammars and any failures.

## Risks and decisions for tech.md

1. **WASM tree-sitter runtime cost.** WASM grammars are slower than
   native (compiled Rust) grammars. The TECH spec must define:
   - The benchmark we run before / after to establish the
     regression budget.
   - Whether bundled grammars stay native by default and only
     user-local grammars use WASM (recommended).

2. **`language.toml` schema versioning.** Future Warp releases will
   want to add fields (e.g. LSP server binary path). The schema
   needs a `schema_version` field at the root and a migration story
   for older grammars.

3. **The migration of the 32 existing languages.** This spec
   explicitly does NOT migrate them in V1 (B4). The TECH spec
   should sketch the per-language migration PR template so a
   follow-up can be done incrementally without coordinated
   flag-day risk.

4. **Sub-language injection** (Vue, TSX, Markdown code blocks).
   The current Vue/TSX special casing is hand-written. New
   user-local grammars cannot define injections in V1. This is
   acknowledged in non-goals.

5. **Theme integration.** Highlight queries reference capture names
   (`@keyword`, `@string`, `@function.method`, etc.) that the
   theme then colors. A user-local grammar that uses a non-standard
   capture name gets no color. The TECH spec must define:
   - The list of capture names the theme guarantees support for
     (the "standard set"), AND
   - The fallback color for unknown capture names (recommended:
     foreground, no styling).

6. **Per-user grammar cache and parser-revision pinning.** Tree-
   sitter ABI changes have caused breakage in other editors when
   user-local grammars are compiled against a different ABI than
   the host editor uses. The TECH spec must define how the loader
   detects ABI mismatch and reports it.

## Reporter-supplied context (preserved)

The reporter explicitly cited Midnight Commander's syntax definition
folder as inspiration, and modern reference points: Sublime Text,
TextMate, and the Rust syntax-highlighting library ecosystem.
The reporter's stated motivation is to "distribute work on all the
syntax highlight feature requests to individual contributors" — i.e.,
the unblocking outcome is contributor velocity, not parser
expressiveness.
