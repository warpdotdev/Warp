use std::collections::HashMap;
use std::fs;
use std::path::Path;

use templates::{Template, TemplateError};
use tempfile::TempDir;

fn write_file(dir: &Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn minimal_manifest(name: &str) -> String {
    format!(r#"name = "{name}""#)
}

// ── Template::load ───────────────────────────────────────────────

#[tokio::test]
async fn load_reads_manifest_and_catalogs_files() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "template.toml", &minimal_manifest("hello"));
    write_file(tmp.path(), "src/main.rs", "fn main() {}");
    write_file(tmp.path(), "README.md", "# {{project_name}}");

    let t = Template::load(tmp.path()).await.unwrap();
    assert_eq!(t.manifest.name, "hello");
    assert_eq!(t.files.len(), 2);
}

#[tokio::test]
async fn load_excludes_manifest_from_files() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "template.toml", &minimal_manifest("no-leak"));
    write_file(tmp.path(), "main.rs", "");

    let t = Template::load(tmp.path()).await.unwrap();
    let names: Vec<_> = t.files.iter().map(|p| p.to_string_lossy().to_string()).collect();
    assert!(!names.iter().any(|n| n.contains("template.toml")));
    assert_eq!(t.files.len(), 1);
}

#[tokio::test]
async fn load_missing_manifest_returns_io_error() {
    let tmp = TempDir::new().unwrap();
    let err = Template::load(tmp.path()).await.expect_err("no manifest");
    assert!(matches!(err, TemplateError::Io { .. }));
}

#[tokio::test]
async fn load_invalid_manifest_returns_parse_error() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "template.toml", "not valid [ toml ??");
    let err = Template::load(tmp.path()).await.expect_err("bad toml");
    assert!(matches!(err, TemplateError::ManifestParse { .. }));
}

#[tokio::test]
async fn load_full_manifest_parsed_correctly() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "template.toml",
        r#"
name = "cloudflare-fullstack"
description = "Fullstack Cloudflare Workers project"
version = "0.1.0"
author = "Warp Team"

[[variables]]
name = "project_name"
description = "Project directory name (kebab-case)"
required = true

[[variables]]
name = "author"
default = "anon"

[hooks]
post_init = ["npm install", "git init"]
"#,
    );
    write_file(tmp.path(), "package.json", r#"{"name":"{{project_name}}"}"#);

    let t = Template::load(tmp.path()).await.unwrap();
    assert_eq!(t.manifest.name, "cloudflare-fullstack");
    assert_eq!(t.manifest.variables.len(), 2);
    assert!(t.manifest.variables[0].required);
    assert_eq!(t.manifest.variables[1].default, "anon");
    assert_eq!(t.manifest.hooks.post_init, vec!["npm install", "git init"]);
}

#[tokio::test]
async fn load_excludes_git_directory() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "template.toml", &minimal_manifest("no-git"));
    write_file(tmp.path(), ".git/HEAD", "ref: refs/heads/main");
    write_file(tmp.path(), ".git/config", "[core]\n    bare = false");
    write_file(tmp.path(), "README.md", "# hello");

    let t = Template::load(tmp.path()).await.unwrap();
    let names: Vec<_> = t.files.iter().map(|p| p.to_string_lossy().to_string()).collect();
    assert!(!names.iter().any(|n| n.starts_with(".git")), "names: {names:?}");
    assert_eq!(t.files.len(), 1);
}

// ── Template::instantiate — happy paths ─────────────────────────

#[tokio::test]
async fn instantiate_substitutes_file_contents() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(src.path(), "template.toml", &minimal_manifest("subst"));
    write_file(src.path(), "README.md", "# {{project_name}}\n\nBy {{author}}.\n");

    let t = Template::load(src.path()).await.unwrap();
    let ctx = HashMap::from([
        ("project_name".to_string(), "MyApp".to_string()),
        ("author".to_string(), "Alice".to_string()),
    ]);
    let written = t.instantiate(dst.path(), &ctx).await.unwrap();
    assert_eq!(written.len(), 1);

    let contents = fs::read_to_string(dst.path().join("README.md")).unwrap();
    assert_eq!(contents, "# MyApp\n\nBy Alice.\n");
}

#[tokio::test]
async fn instantiate_substitutes_path_segments() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(src.path(), "template.toml", &minimal_manifest("path-subst"));
    // Directory name and filename both contain placeholders.
    write_file(src.path(), "{{project_name}}/main.rs", "// {{project_name}}");

    let t = Template::load(src.path()).await.unwrap();
    let ctx = HashMap::from([("project_name".to_string(), "myapp".to_string())]);
    t.instantiate(dst.path(), &ctx).await.unwrap();

    let rendered = dst.path().join("myapp/main.rs");
    assert!(rendered.exists(), "rendered path should exist: {rendered:?}");
    assert_eq!(fs::read_to_string(&rendered).unwrap(), "// myapp");
}

#[tokio::test]
async fn instantiate_preserves_unknown_placeholders() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(src.path(), "template.toml", &minimal_manifest("preserve"));
    write_file(src.path(), "file.txt", "known={{known}}, unknown={{unknown}}");

    let t = Template::load(src.path()).await.unwrap();
    let ctx = HashMap::from([("known".to_string(), "yes".to_string())]);
    t.instantiate(dst.path(), &ctx).await.unwrap();

    let contents = fs::read_to_string(dst.path().join("file.txt")).unwrap();
    assert_eq!(contents, "known=yes, unknown={{unknown}}");
}

#[tokio::test]
async fn instantiate_creates_nested_directories() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(src.path(), "template.toml", &minimal_manifest("nested"));
    write_file(src.path(), "a/b/c/deep.txt", "deep");

    let t = Template::load(src.path()).await.unwrap();
    t.instantiate(dst.path(), &HashMap::new()).await.unwrap();

    assert!(dst.path().join("a/b/c/deep.txt").exists());
}

#[tokio::test]
async fn instantiate_copies_binary_file_unchanged() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(src.path(), "template.toml", &minimal_manifest("binary"));
    // Write non-UTF-8 bytes.
    let bin_path = src.path().join("icon.png");
    fs::write(&bin_path, b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0d{{invalid}}").unwrap();

    let t = Template::load(src.path()).await.unwrap();
    let ctx = HashMap::from([("invalid".to_string(), "replaced".to_string())]);
    t.instantiate(dst.path(), &ctx).await.unwrap();

    let out = fs::read(dst.path().join("icon.png")).unwrap();
    assert_eq!(out, b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0d{{invalid}}");
}

// ── Template::instantiate — error paths ─────────────────────────

#[tokio::test]
async fn instantiate_errors_on_missing_required_variable() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(
        src.path(),
        "template.toml",
        r#"
name = "req"
[[variables]]
name = "project_name"
required = true
"#,
    );
    write_file(src.path(), "file.txt", "{{project_name}}");

    let t = Template::load(src.path()).await.unwrap();
    let err = t
        .instantiate(dst.path(), &HashMap::new())
        .await
        .expect_err("missing required variable should fail");

    match err {
        TemplateError::MissingVariable { name } => assert_eq!(name, "project_name"),
        other => panic!("unexpected error: {other:?}"),
    }

    // Target must be untouched.
    assert!(is_empty_dir(dst.path()), "no files should have been written");
}

#[tokio::test]
async fn instantiate_errors_on_non_empty_target() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(src.path(), "template.toml", &minimal_manifest("ne"));
    write_file(src.path(), "f.txt", "hi");
    // Pre-populate target.
    write_file(dst.path(), "existing.txt", "already here");

    let t = Template::load(src.path()).await.unwrap();
    let err = t
        .instantiate(dst.path(), &HashMap::new())
        .await
        .expect_err("non-empty target should fail");

    assert!(matches!(err, TemplateError::TargetNotEmpty { .. }));
}

#[tokio::test]
async fn instantiate_accepts_empty_existing_target() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap(); // exists but is empty
    write_file(src.path(), "template.toml", &minimal_manifest("empty-dst"));
    write_file(src.path(), "hello.txt", "world");

    let t = Template::load(src.path()).await.unwrap();
    t.instantiate(dst.path(), &HashMap::new())
        .await
        .expect("empty target should be accepted");

    assert!(dst.path().join("hello.txt").exists());
}

// ── helpers ──────────────────────────────────────────────────────

fn is_empty_dir(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|mut e| e.next().is_none())
        .unwrap_or(false)
}
