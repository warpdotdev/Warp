use std::path::{Path, PathBuf};

use lsp_types::Uri;

use crate::config::{lsp_uri_to_path, path_to_lsp_uri};
use crate::supported_servers::LSPServerType;
use crate::LanguageId;

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

#[test]
fn test_elixir_language_id_from_extension() {
    for ext in ["ex", "exs"] {
        let path = PathBuf::from(format!("foo.{ext}"));
        assert_eq!(
            LanguageId::from_path(&path),
            Some(LanguageId::Elixir),
            "extension .{ext} should map to Elixir"
        );
    }
}

#[test]
fn test_eex_language_id_from_extension() {
    for ext in ["eex", "leex"] {
        let path = PathBuf::from(format!("foo.{ext}"));
        assert_eq!(
            LanguageId::from_path(&path),
            Some(LanguageId::Eex),
            "extension .{ext} should map to Eex"
        );
    }
}

#[test]
fn test_phoenix_heex_language_id_from_extension() {
    let path = PathBuf::from("foo.heex");
    assert_eq!(LanguageId::from_path(&path), Some(LanguageId::PhoenixHeex));
}

#[test]
fn test_elixir_language_id_from_filename() {
    for name in ["mix.exs", "mix.lock", ".formatter.exs"] {
        let path = Path::new(name);
        assert_eq!(
            LanguageId::from_path(path),
            Some(LanguageId::Elixir),
            "filename {name} should map to Elixir"
        );
    }
}

#[test]
fn test_elixir_family_server_type() {
    assert_eq!(LanguageId::Elixir.server_type(), LSPServerType::Expert);
    assert_eq!(LanguageId::Eex.server_type(), LSPServerType::Expert);
    assert_eq!(
        LanguageId::PhoenixHeex.server_type(),
        LSPServerType::Expert
    );
}

#[test]
fn test_elixir_family_lsp_identifiers() {
    assert_eq!(LanguageId::Elixir.lsp_language_identifier(), "elixir");
    assert_eq!(LanguageId::Eex.lsp_language_identifier(), "eex");
    assert_eq!(
        LanguageId::PhoenixHeex.lsp_language_identifier(),
        "phoenix-heex"
    );
}

#[test]
fn test_expert_binary_name() {
    assert_eq!(LSPServerType::Expert.binary_name(), "expert");
}

#[test]
fn test_expert_language_name() {
    assert_eq!(LSPServerType::Expert.language_name(), "Elixir");
}
