use std::path::PathBuf;

use lsp_types::Uri;

use crate::config::{lsp_uri_to_path, path_to_lsp_uri};

// Unix-specific tests use Unix paths
#[cfg(not(windows))]
mod unix_tests {
    use super::*;

    #[test]
    fn test_lsp_uri_to_path_basic() {
        let uri: Uri = "file:///Users/test/project/src/main.rs".parse().unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/Users/test/project/src/main.rs"));
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_at_symbol() {
        // %40 is the URL encoding for @
        let uri: Uri = "file:///Users/test/node_modules/%40firebase/auth/dist/index.d.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/test/node_modules/@firebase/auth/dist/index.d.ts")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_spaces() {
        // %20 is the URL encoding for space
        let uri: Uri = "file:///Users/test/My%20Project/src/main.rs"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/Users/test/My Project/src/main.rs"));
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_multiple_special_chars() {
        // Test multiple encoded characters: @ (%40), space (%20), # (%23)
        let uri: Uri = "file:///Users/test/%40scope/my%20package%23v1/index.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/test/@scope/my package#v1/index.ts")
        );
    }

    #[test]
    fn test_path_to_lsp_uri_basic() {
        let path = PathBuf::from("/Users/test/project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///Users/test/project/src/main.rs");
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_spaces() {
        let path = PathBuf::from("/Users/test/My Project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///Users/test/My%20Project/src/main.rs");
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_non_ascii() {
        let path = PathBuf::from("/Users/관리자/project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert!(uri.as_str().starts_with("file:///Users/%"));
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_accented_chars() {
        let path = PathBuf::from("/Users/José/project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert!(uri.as_str().starts_with("file:///Users/Jos%"));
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_hash() {
        let path = PathBuf::from("/Users/test/my#project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///Users/test/my%23project/src/main.rs");
    }

    #[test]
    fn test_roundtrip_path_to_uri_to_path() {
        let original_path = PathBuf::from("/Users/test/project/src/main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }

    #[test]
    fn test_roundtrip_non_ascii_path() {
        let original_path = PathBuf::from("/Users/관리자/project/src/main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }

    #[test]
    fn test_roundtrip_path_with_spaces() {
        let original_path = PathBuf::from("/Users/test/My Project/src/main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_brackets() {
        let path = PathBuf::from("/Users/test/routes/blog/[slug].tsx");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(
            uri.as_str(),
            "file:///Users/test/routes/blog/%5Bslug%5D.tsx"
        );
    }

    #[test]
    fn test_roundtrip_path_with_brackets() {
        let original_path = PathBuf::from("/Users/test/routes/[id]/[slug].tsx");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }
}

// Windows-specific tests use Windows paths
#[cfg(windows)]
mod windows_tests {
    use super::*;

    #[test]
    fn test_lsp_uri_to_path_basic() {
        let uri: Uri = "file:///C:/Users/test/project/src/main.rs".parse().unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\project\\src\\main.rs")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_at_symbol() {
        // %40 is the URL encoding for @
        let uri: Uri = "file:///C:/Users/test/node_modules/%40firebase/auth/dist/index.d.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\node_modules\\@firebase\\auth\\dist\\index.d.ts")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_spaces() {
        // %20 is the URL encoding for space
        let uri: Uri = "file:///C:/Users/test/My%20Project/src/main.rs"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\My Project\\src\\main.rs")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_multiple_special_chars() {
        // Test multiple encoded characters: @ (%40), space (%20), # (%23)
        let uri: Uri = "file:///C:/Users/test/%40scope/my%20package%23v1/index.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\@scope\\my package#v1\\index.ts")
        );
    }

    #[test]
    fn test_path_to_lsp_uri_basic() {
        let path = PathBuf::from("C:\\Users\\test\\project\\src\\main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///C:/Users/test/project/src/main.rs");
    }

    #[test]
    fn test_roundtrip_path_to_uri_to_path() {
        let original_path = PathBuf::from("C:\\Users\\test\\project\\src\\main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }
}

// Platform-independent tests
#[test]
fn test_lsp_uri_to_path_rejects_non_file_uri() {
    let uri: Uri = "https://example.com/path".parse().unwrap();
    let result = lsp_uri_to_path(&uri);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid file URI"));
}

#[test]
fn test_path_to_lsp_uri_rejects_relative_path() {
    let path = PathBuf::from("relative/path/file.rs");
    let result = path_to_lsp_uri(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be absolute"));
}

mod json_language_detection {
    //! Regression tests for #9556 — JSON language support via the VS Code
    //! JSON language server.

    use super::*;
    use crate::config::LanguageId;
    use crate::supported_servers::LSPServerType;

    #[test]
    fn classifies_plain_json_files() {
        assert_eq!(
            LanguageId::from_path(&PathBuf::from("package.json")),
            Some(LanguageId::Json)
        );
        assert_eq!(
            LanguageId::from_path(&PathBuf::from("nested/dir/data.json")),
            Some(LanguageId::Json)
        );
    }

    #[test]
    fn classifies_jsonc_files_distinctly() {
        // .jsonc is JSON-with-comments; the VS Code JSON server distinguishes
        // `json` from `jsonc` and only allows comments under `jsonc`. We must
        // route .jsonc through its own LanguageId so the right languageId is
        // sent in `didOpen`.
        assert_eq!(
            LanguageId::from_path(&PathBuf::from("docs/example.jsonc")),
            Some(LanguageId::Jsonc)
        );
    }

    #[test]
    fn known_dotjson_jsonc_filenames_route_to_jsonc() {
        // Several well-known config files use `.json` by convention but
        // contain JSON-with-comments. Sending `json` would surface valid
        // `// …` lines as syntax errors.
        for name in [
            "tsconfig.json",
            "jsconfig.json",
            "tsconfig.build.json",
            "jsconfig.app.json",
            ".vscode/settings.json",
            ".vscode/launch.json",
            ".vscode/keybindings.json",
            ".vscode/tasks.json",
            ".vscode/extensions.json",
            ".devcontainer/devcontainer.json",
        ] {
            assert_eq!(
                LanguageId::from_path(&PathBuf::from(name)),
                Some(LanguageId::Jsonc),
                "{name} should be classified as JSONC",
            );
        }
    }

    #[test]
    fn unrelated_dotjson_stays_strict_json() {
        // package.json, manifest.json, etc. are strict JSON.
        for name in [
            "package.json",
            "package-lock.json",
            "manifest.json",
            "data.json",
        ] {
            assert_eq!(
                LanguageId::from_path(&PathBuf::from(name)),
                Some(LanguageId::Json),
                "{name} should remain strict JSON",
            );
        }
    }

    #[test]
    fn vscode_filenames_outside_dotvscode_stay_strict_json() {
        // A `settings.json` (or `launch.json`, etc.) that lives outside a
        // `.vscode/` directory could just as easily be strict JSON in an
        // unrelated project. We must not relax validation for it just
        // because the filename matches a VS Code convention.
        for name in [
            "settings.json",
            "launch.json",
            "tasks.json",
            "keybindings.json",
            "extensions.json",
            "src/settings.json",
            "config/launch.json",
            "deeply/nested/extensions.json",
        ] {
            assert_eq!(
                LanguageId::from_path(&PathBuf::from(name)),
                Some(LanguageId::Json),
                "{name} (no .vscode/ parent) should remain strict JSON",
            );
        }
    }

    #[test]
    fn vscode_filenames_under_dotvscode_become_jsonc() {
        // The same names *do* route to JSONC when the immediate parent is
        // `.vscode/` — that's the canonical VS Code config layout.
        for name in [
            ".vscode/settings.json",
            ".vscode/launch.json",
            ".vscode/tasks.json",
            ".vscode/keybindings.json",
            ".vscode/extensions.json",
            // Nested-project case: `frontend/.vscode/settings.json`.
            "frontend/.vscode/settings.json",
        ] {
            assert_eq!(
                LanguageId::from_path(&PathBuf::from(name)),
                Some(LanguageId::Jsonc),
                "{name} (under .vscode/) should be classified as JSONC",
            );
        }
    }

    #[test]
    fn both_json_and_jsonc_route_to_vscode_json_language_server() {
        assert_eq!(
            LanguageId::Json.server_type(),
            LSPServerType::VsCodeJsonLanguageServer
        );
        assert_eq!(
            LanguageId::Jsonc.server_type(),
            LSPServerType::VsCodeJsonLanguageServer
        );
    }

    #[test]
    fn vscode_json_language_server_advertises_both_languages() {
        assert_eq!(
            LSPServerType::VsCodeJsonLanguageServer.languages(),
            vec![LanguageId::Json, LanguageId::Jsonc]
        );
    }

    #[test]
    fn vscode_json_language_server_uses_npm_binary_name() {
        assert_eq!(
            LSPServerType::VsCodeJsonLanguageServer.binary_name(),
            "vscode-json-languageserver"
        );
    }

    #[test]
    fn vscode_json_language_server_appears_in_all() {
        let all: Vec<LSPServerType> = LSPServerType::all().collect();
        assert!(
            all.contains(&LSPServerType::VsCodeJsonLanguageServer),
            "LSPServerType::all() must yield VsCodeJsonLanguageServer; got {all:?}",
        );
    }

    #[test]
    fn json_and_jsonc_emit_distinct_lsp_language_identifiers() {
        // The LSP spec defines `json` and `jsonc` as separate document
        // languageIds; only `jsonc` accepts comments. Make sure we don't
        // collapse them into the same identifier when sending `didOpen`.
        assert_eq!(LanguageId::Json.lsp_language_identifier(), "json");
        assert_eq!(LanguageId::Jsonc.lsp_language_identifier(), "jsonc");
    }
}
