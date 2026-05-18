use std::path::Path;

use crate::{language_by_filename, language_by_name, load_language, SUPPORTED_LANGUAGES};

/// Validate that every supported language can be loaded successfully.
/// This catches invalid node types, syntax errors, and other issues in .scm query files
/// (highlights, indents, identifiers) that would otherwise only surface at runtime.
#[test]
fn all_supported_languages_load_successfully() {
    let failures: Vec<_> = SUPPORTED_LANGUAGES
        .iter()
        .filter(|lang| load_language(lang).is_none())
        .collect();

    assert!(
        failures.is_empty(),
        "The following languages failed to load:\n{}",
        failures
            .iter()
            .map(|lang| format!("  - {lang}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Both `.html` and the legacy three-character `.htm` extension should resolve to
/// the same HTML language entry. `.htm` is widely produced by static-site generators
/// and historical web tooling (DOS 8.3 filename limits) and is already treated as
/// an HTML/text file elsewhere in the codebase
/// (see `is_development_text_extension` in `crates/warp_util/src/file_type.rs`).
#[test]
fn html_extensions_resolve_to_html() {
    for filename in ["index.html", "index.htm"] {
        let language = language_by_filename(Path::new(filename))
            .unwrap_or_else(|| panic!("expected {filename} to resolve to a language"));
        assert_eq!(
            language.display_name(),
            "HTML",
            "{filename} should resolve to HTML",
        );
    }
}

/// `.command` is the macOS convention for double-clickable shell scripts.
/// Make sure `language_by_filename` recognizes it as shell so the editor
/// renders syntax highlighting instead of the
/// "Language support is unavailable for this file type" footer.
#[test]
fn command_extension_resolves_to_shell() {
    let language = language_by_filename(Path::new("script.command"))
        .expect("`.command` files should resolve to a language");
    assert_eq!(language.display_name(), "Shell");
}

/// Newly added languages should resolve from their canonical file extensions so
/// that opening, e.g., a `.dart` file renders syntax highlighting.
#[test]
fn new_language_extensions_resolve() {
    for (filename, expected) in [
        ("main.dart", "Dart"),
        ("build.zig", "Zig"),
        ("styles.scss", "SCSS"),
        ("analysis.R", "R"),
        ("script.jl", "Julia"),
        ("lib.ml", "OCaml"),
        ("server.erl", "Erlang"),
        ("flake.nix", "Nix"),
        ("build.gradle", "Groovy"),
        ("Token.sol", "Solidity"),
        ("schema.graphql", "GraphQL"),
        ("api.proto", "Protocol Buffers"),
        ("core.clj", "Clojure"),
        ("Main.elm", "Elm"),
        ("config.cmake", "CMake"),
        ("CMakeLists.txt", "CMake"),
    ] {
        let language = language_by_filename(Path::new(filename))
            .unwrap_or_else(|| panic!("expected {filename} to resolve to a language"));
        assert_eq!(
            language.display_name(),
            expected,
            "{filename} should resolve to {expected}",
        );
    }
}

/// Common markdown code-fence aliases should normalize to the canonical language
/// so fenced blocks like ```` ```protobuf ```` are highlighted.
#[test]
fn new_language_aliases_normalize() {
    for (alias, expected) in [
        ("protobuf", "Protocol Buffers"),
        ("gradle", "Groovy"),
        ("erl", "Erlang"),
        ("jl", "Julia"),
        ("sol", "Solidity"),
        ("gql", "GraphQL"),
    ] {
        let language = language_by_name(alias)
            .unwrap_or_else(|| panic!("expected alias `{alias}` to resolve to a language"));
        assert_eq!(language.display_name(), expected, "`{alias}` should resolve");
    }
}
