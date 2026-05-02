use chrono::{TimeZone, Utc};
use skills::metadata::{MetadataStore, SkillMetadata};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// SkillMetadata serialisation round-trips
// ---------------------------------------------------------------------------

#[test]
fn default_metadata_serialises_to_minimal_json() {
    let meta = SkillMetadata::default();
    let json = serde_json::to_string(&meta).expect("serialise");
    // Optional/empty fields must not appear in the output.
    assert!(!json.contains("last_used"), "unexpected last_used in {json}");
    assert!(!json.contains("success_rate"), "unexpected success_rate in {json}");
    assert!(!json.contains("user_tags"), "unexpected user_tags in {json}");
    assert!(!json.contains("user_description"), "unexpected user_description in {json}");
    // Zero counts are always present.
    assert!(json.contains("total_tokens"));
    assert!(json.contains("tool_call_count"));
}

#[test]
fn full_metadata_round_trips_through_json() {
    let original = SkillMetadata {
        last_used: Some(Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap()),
        success_rate: Some(0.85),
        total_tokens: 4096,
        tool_call_count: 7,
        user_tags: vec!["ci".to_string(), "deploy".to_string()],
        user_description: Some("My override description".to_string()),
    };
    let json = serde_json::to_string(&original).expect("serialise");
    let recovered: SkillMetadata = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(original, recovered);
}

#[test]
fn partial_metadata_deserialises_with_defaults() {
    // Only last_used is set; all other fields should fall back to defaults.
    let json = r#"{"last_used":"2024-01-15T09:30:00Z"}"#;
    let meta: SkillMetadata = serde_json::from_str(json).expect("deserialise");
    assert!(meta.last_used.is_some());
    assert!(meta.success_rate.is_none());
    assert_eq!(meta.total_tokens, 0);
    assert_eq!(meta.tool_call_count, 0);
    assert!(meta.user_tags.is_empty());
    assert!(meta.user_description.is_none());
}

// ---------------------------------------------------------------------------
// SkillMetadata helper methods
// ---------------------------------------------------------------------------

#[test]
fn effective_description_prefers_user_description() {
    let mut meta = SkillMetadata::default();
    assert_eq!(meta.effective_description("fallback"), "fallback");

    meta.user_description = Some("user override".to_string());
    assert_eq!(meta.effective_description("fallback"), "user override");
}

#[test]
fn effective_tags_merges_and_deduplicates() {
    let meta = SkillMetadata {
        user_tags: vec!["rust".to_string(), "new-tag".to_string()],
        ..Default::default()
    };
    let base = vec!["rust".to_string(), "ci".to_string()];
    let effective = meta.effective_tags(&base);

    // Front-matter tags come first, user tags appended without duplicates.
    assert_eq!(
        effective,
        vec!["rust".to_string(), "ci".to_string(), "new-tag".to_string()]
    );
}

#[test]
fn effective_tags_with_no_user_tags_returns_base() {
    let meta = SkillMetadata::default();
    let base = vec!["cloudflare".to_string(), "deploy".to_string()];
    assert_eq!(meta.effective_tags(&base), base);
}

#[test]
fn effective_tags_with_empty_base_returns_user_tags() {
    let meta = SkillMetadata {
        user_tags: vec!["a".to_string(), "b".to_string()],
        ..Default::default()
    };
    assert_eq!(meta.effective_tags(&[]), vec!["a".to_string(), "b".to_string()]);
}

// ---------------------------------------------------------------------------
// MetadataStore: empty / missing file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn load_returns_empty_store_for_missing_file() {
    let store = MetadataStore::load("/tmp/nonexistent-skill-metadata-xyz.json")
        .await
        .expect("load missing file");
    assert!(store.is_empty());
}

// ---------------------------------------------------------------------------
// MetadataStore: save and load round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn save_and_load_round_trips() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("metadata.json");

    let mut store = MetadataStore::default();
    store.set(
        "my-skill",
        SkillMetadata {
            last_used: Some(Utc.with_ymd_and_hms(2025, 3, 10, 8, 0, 0).unwrap()),
            success_rate: Some(0.9),
            total_tokens: 1024,
            tool_call_count: 5,
            user_tags: vec!["rust".to_string()],
            user_description: Some("custom desc".to_string()),
        },
    );
    store.set(
        "other-skill",
        SkillMetadata {
            total_tokens: 256,
            ..Default::default()
        },
    );

    store.save(&path).await.expect("save");

    let loaded = MetadataStore::load(&path).await.expect("load");
    assert_eq!(loaded.len(), 2);

    let meta = loaded.get("my-skill").expect("my-skill present");
    assert_eq!(meta.total_tokens, 1024);
    assert_eq!(meta.tool_call_count, 5);
    assert_eq!(meta.success_rate, Some(0.9));
    assert_eq!(meta.user_description.as_deref(), Some("custom desc"));
    assert_eq!(meta.user_tags, vec!["rust".to_string()]);

    let other = loaded.get("other-skill").expect("other-skill present");
    assert_eq!(other.total_tokens, 256);
    assert!(other.last_used.is_none());
}

#[tokio::test]
async fn save_creates_parent_directories() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deep").join("nested").join("metadata.json");

    let mut store = MetadataStore::default();
    store.set("skill", SkillMetadata::default());

    store.save(&path).await.expect("save with deep path");
    assert!(path.exists());
}

// ---------------------------------------------------------------------------
// MetadataStore: get/set/remove
// ---------------------------------------------------------------------------

#[test]
fn get_returns_none_for_absent_key() {
    let store = MetadataStore::default();
    assert!(store.get("no-such-skill").is_none());
}

#[test]
fn set_overwrites_existing_entry() {
    let mut store = MetadataStore::default();
    store.set("s", SkillMetadata { total_tokens: 10, ..Default::default() });
    store.set("s", SkillMetadata { total_tokens: 99, ..Default::default() });
    assert_eq!(store.get("s").unwrap().total_tokens, 99);
}

#[test]
fn remove_deletes_entry_and_returns_it() {
    let mut store = MetadataStore::default();
    store.set("s", SkillMetadata { total_tokens: 42, ..Default::default() });
    let removed = store.remove("s").expect("removed");
    assert_eq!(removed.total_tokens, 42);
    assert!(store.get("s").is_none());
    assert_eq!(store.len(), 0);
}

#[test]
fn iter_visits_all_entries() {
    let mut store = MetadataStore::default();
    store.set("a", SkillMetadata { total_tokens: 1, ..Default::default() });
    store.set("b", SkillMetadata { total_tokens: 2, ..Default::default() });
    store.set("c", SkillMetadata { total_tokens: 3, ..Default::default() });

    let mut names: Vec<&str> = store.iter().map(|(name, _)| name).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["a", "b", "c"]);
    assert_eq!(store.len(), 3);
}

// ---------------------------------------------------------------------------
// MetadataStore: invalid JSON returns an error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn load_returns_error_for_invalid_json() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.json");
    tokio::fs::write(&path, b"not valid json {{{").await.unwrap();

    let err = MetadataStore::load(&path).await.expect_err("expected json error");
    let msg = err.to_string();
    assert!(msg.contains("json error"), "unexpected error message: {msg}");
}

// ---------------------------------------------------------------------------
// SkillRegistry::apply_metadata integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_metadata_merges_into_registry() {
    use skills::{LoaderConfig, SkillRegistry};
    use std::fs;

    let dir = tempfile::TempDir::new().unwrap();
    fs::write(
        dir.path().join("greet.md"),
        "---\nname: greet\n---\nSay hello.\n",
    )
    .unwrap();

    let mut registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(dir.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    // Metadata is default before apply_metadata.
    assert_eq!(registry.get("greet").unwrap().metadata.total_tokens, 0);

    let mut store = MetadataStore::default();
    store.set(
        "greet",
        SkillMetadata {
            total_tokens: 512,
            tool_call_count: 2,
            success_rate: Some(1.0),
            user_description: Some("A greeting skill".to_string()),
            ..Default::default()
        },
    );

    registry.apply_metadata(&store);

    let meta = &registry.get("greet").unwrap().metadata;
    assert_eq!(meta.total_tokens, 512);
    assert_eq!(meta.tool_call_count, 2);
    assert_eq!(meta.success_rate, Some(1.0));
    assert_eq!(meta.user_description.as_deref(), Some("A greeting skill"));
}

#[tokio::test]
async fn apply_metadata_leaves_unmatched_skills_at_default() {
    use skills::{LoaderConfig, SkillRegistry};
    use std::fs;

    let dir = tempfile::TempDir::new().unwrap();
    fs::write(dir.path().join("foo.md"), "---\nname: foo\n---\nbody\n").unwrap();

    let mut registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(dir.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    // Store has an entry for a different skill.
    let mut store = MetadataStore::default();
    store.set("bar", SkillMetadata { total_tokens: 999, ..Default::default() });

    registry.apply_metadata(&store);

    // "foo" should still be at default.
    assert_eq!(registry.get("foo").unwrap().metadata.total_tokens, 0);
}
