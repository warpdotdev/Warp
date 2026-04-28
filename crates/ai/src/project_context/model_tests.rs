use super::*;
use std::path::PathBuf;

#[test]
fn test_find_applicable_rules_empty_rules() {
    let rules = ProjectRules { rules: vec![] };
    let path = PathBuf::from("/a/b/c/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert!(result.is_empty());
}

#[test]
fn test_find_applicable_rules_no_matching_rules() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/x/y/WARP.md"), "content1".to_string());
    rules.upsert_rule(Path::new("/z/AGENTS.md"), "content2".to_string());

    let path = PathBuf::from("/a/b/c/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert!(result.is_empty());
}

#[test]
fn test_find_applicable_rules_single_matching_rule() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/a/WARP.md"), "content1".to_string());
    rules.upsert_rule(Path::new("/x/AGENTS.md"), "content2".to_string());

    let path = PathBuf::from("/a/b/c/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, PathBuf::from("/a/WARP.md"));
}

#[test]
fn test_find_applicable_rules_includes_all_ancestor_rules() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/a/WARP.md"), "root_warp".to_string());
    rules.upsert_rule(Path::new("/a/b/WARP.md"), "nested_warp".to_string());
    rules.upsert_rule(Path::new("/a/b/c/WARP.md"), "deep_warp".to_string());

    let path = PathBuf::from("/a/b/c/d/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 3);

    // All should be WARP.md files (same priority), order is not specified by depth
    // Just verify all expected rules are present
    let paths: Vec<PathBuf> = result.iter().map(|r| r.path.clone()).collect();
    assert!(paths.contains(&PathBuf::from("/a/WARP.md")));
    assert!(paths.contains(&PathBuf::from("/a/b/WARP.md")));
    assert!(paths.contains(&PathBuf::from("/a/b/c/WARP.md")));
}

#[test]
fn test_find_applicable_rules_multiple_patterns() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/a/b/AGENTS.md"), "agents_content".to_string());
    rules.upsert_rule(Path::new("/a/WARP.md"), "warp_content".to_string());

    let path = PathBuf::from("/a/b/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 2);

    assert_eq!(result[0].path, PathBuf::from("/a/b/AGENTS.md"));
    assert_eq!(result[0].content, "agents_content");
    assert_eq!(result[1].path, PathBuf::from("/a/WARP.md"));
    assert_eq!(result[1].content, "warp_content");
}

#[test]
fn test_find_applicable_rules_exact_path_match() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/a/b/WARP.md"), "exact_match".to_string());

    let path = PathBuf::from("/a/b/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, PathBuf::from("/a/b/WARP.md"));
    assert_eq!(result[0].content, "exact_match");
}

#[test]
fn test_find_applicable_rules_ignores_deeper_paths() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/a/WARP.md"), "applicable".to_string());
    rules.upsert_rule(Path::new("/a/b/c/d/e/WARP.md"), "too_deep".to_string()); // Path doesn't contain /a/b

    let path = PathBuf::from("/a/b/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, PathBuf::from("/a/WARP.md"));
    assert_eq!(result[0].content, "applicable");
}

#[test]
fn test_find_applicable_rules_handles_root_path() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/WARP.md"), "root_rule".to_string());

    let path = PathBuf::from("/a/b/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, PathBuf::from("/WARP.md"));
    assert_eq!(result[0].content, "root_rule");
}

#[test]
fn test_find_applicable_rules_complex_scenario() {
    // This test covers the example from the original request:
    // For path /a/b/c/file.rs with rules:
    // - /a/WARP.md
    // - /a/AGENTS.md
    // - /a/b/WARP.md
    // - /a/b/AGENTS.md
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/a/WARP.md"), "a_warp".to_string());
    rules.upsert_rule(Path::new("/a/AGENTS.md"), "a_agents".to_string());
    rules.upsert_rule(Path::new("/a/b/WARP.md"), "ab_warp".to_string());
    rules.upsert_rule(Path::new("/a/b/AGENTS.md"), "ab_agents".to_string());
    rules.upsert_rule(Path::new("/x/WARP.md"), "irrelevant".to_string()); // Should be ignored

    let path = PathBuf::from("/a/b/c/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 2);

    // Expect only WARP.md files to be included as they have higher priority.
    assert_eq!(result[0].path, PathBuf::from("/a/WARP.md"));
    assert_eq!(result[0].content, "a_warp");
    assert_eq!(result[1].path, PathBuf::from("/a/b/WARP.md"));
    assert_eq!(result[1].content, "ab_warp");
}

#[test]
fn test_find_applicable_rules_handles_unknown_file_patterns() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("/a/WARP.md"), "known_pattern".to_string());
    rules.upsert_rule(Path::new("/a/UNKNOWN.md"), "unknown_pattern".to_string());
    let path = PathBuf::from("/a/file.rs");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 1);

    assert_eq!(result[0].path, PathBuf::from("/a/WARP.md"));
    assert_eq!(result[0].content, "known_pattern");
}

#[test]
fn test_find_applicable_rules_with_relative_paths() {
    let mut rules = ProjectRules::default();

    rules.upsert_rule(Path::new("src/WARP.md"), "src_warp".to_string());
    rules.upsert_rule(
        Path::new("src/components/WARP.md"),
        "components_warp".to_string(),
    );

    let path = PathBuf::from("src/components/Button.tsx");

    let result = rules.find_active_or_applicable_rules(&path).active_rules;
    assert_eq!(result.len(), 2);

    // Both are WARP.md files (same priority), order within same priority is not guaranteed
    // Just verify both rules are present
    let paths: Vec<PathBuf> = result.iter().map(|r| r.path.clone()).collect();
    assert!(paths.contains(&PathBuf::from("src/WARP.md")));
    assert!(paths.contains(&PathBuf::from("src/components/WARP.md")));
}
