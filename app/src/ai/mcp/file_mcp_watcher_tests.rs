use super::{
    insert_project_config_target, project_config_parent_dirs, providers_in_scope,
    substitute_env_vars,
};
use crate::ai::mcp::MCPProvider;
use std::{collections::HashMap, env, path::PathBuf};

fn cleanup_env_vars(vars: &[&str]) {
    for var in vars {
        env::remove_var(var);
    }
}

#[test]
fn test_substitute_env_vars_success() {
    let test_vars = ["FOO", "BAZ", "REPEATED"];

    // Setup environment variables
    env::set_var("FOO", "bar");
    env::set_var("BAZ", "qux");
    env::set_var("REPEATED", "value");

    // Test 1: Single variable substitution
    let input = r#"{"key": "${FOO}"}"#;
    let result = substitute_env_vars(input).expect("Single variable substitution should succeed");
    assert_eq!(
        result, r#"{"key": "bar"}"#,
        "Single variable FOO should be replaced with 'bar'"
    );

    // Test 2: Multiple different variables
    let input = r#"{"key": "${FOO}", "other": "${BAZ}"}"#;
    let result = substitute_env_vars(input).expect("Multiple variable substitution should succeed");
    assert_eq!(
        result, r#"{"key": "bar", "other": "qux"}"#,
        "Multiple variables FOO and BAZ should be replaced"
    );

    // Test 3: Multiple occurrences of same variable
    let input = r#"{"a": "${REPEATED}", "b": "${REPEATED}", "c": "prefix_${REPEATED}_suffix"}"#;
    let result = substitute_env_vars(input).expect("Repeated variable substitution should succeed");
    assert_eq!(
        result, r#"{"a": "value", "b": "value", "c": "prefix_value_suffix"}"#,
        "All occurrences of REPEATED should be replaced with 'value', including within context"
    );

    // Cleanup
    cleanup_env_vars(&test_vars);
}

#[test]
fn test_substitute_env_vars_missing_or_empty() {
    // Test 1: Missing variable
    // Ensure MISSING_VAR is not set
    env::remove_var("MISSING_VAR");

    let input = r#"{"key": "${MISSING_VAR}"}"#;
    let result = substitute_env_vars(input);
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Missing or empty environment variable: MISSING_VAR"),
        "Error message should mention MISSING_VAR, got: {err_msg}"
    );

    // Test 2: Empty variable
    env::set_var("EMPTY_VAR", "");

    let input = r#"{"key": "${EMPTY_VAR}"}"#;
    let result = substitute_env_vars(input);
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Missing or empty environment variable: EMPTY_VAR"),
        "Error message should mention EMPTY_VAR, got: {err_msg}"
    );

    // Cleanup
    cleanup_env_vars(&["EMPTY_VAR"]);
}

#[test]
fn test_project_config_parent_dirs_only_tracks_known_config_locations() {
    let root = PathBuf::from("/tmp/repo");
    let mut parent_dirs = project_config_parent_dirs(root)
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    parent_dirs.sort();

    assert_eq!(
        parent_dirs,
        vec![
            "/tmp/repo",
            "/tmp/repo/.agents",
            "/tmp/repo/.codex",
            "/tmp/repo/.warp",
        ]
    );
}

#[test]
fn test_providers_in_scope_preserves_project_and_provider_config_paths() {
    let root = PathBuf::from("/tmp/repo");
    let mut config_paths = providers_in_scope(root.clone(), root)
        .map(|(provider, path)| (provider, path.to_string_lossy().to_string()))
        .collect::<Vec<_>>();
    config_paths.sort_by(|(_, left), (_, right)| left.cmp(right));

    assert!(config_paths.contains(&(MCPProvider::Claude, "/tmp/repo/.claude.json".into())));
    assert!(config_paths.contains(&(MCPProvider::Claude, "/tmp/repo/.mcp.json".into())));
    assert!(config_paths.contains(&(MCPProvider::Codex, "/tmp/repo/.codex/config.toml".into())));
    assert!(config_paths.contains(&(MCPProvider::Warp, "/tmp/repo/.warp/.mcp.json".into())));
    assert!(config_paths.contains(&(MCPProvider::Agents, "/tmp/repo/.agents/.mcp.json".into())));
}

#[test]
fn test_insert_project_config_target_deduplicates_root_provider_pairs() {
    let mut targets = HashMap::new();
    let config_path = PathBuf::from("/tmp/repo/.mcp.json");
    let root = PathBuf::from("/tmp/repo");

    insert_project_config_target(
        &mut targets,
        config_path.clone(),
        root.clone(),
        MCPProvider::Claude,
    );
    insert_project_config_target(
        &mut targets,
        config_path.clone(),
        root.clone(),
        MCPProvider::Claude,
    );
    insert_project_config_target(&mut targets, config_path.clone(), root, MCPProvider::Agents);

    assert_eq!(targets.get(&config_path).unwrap().len(), 2);
}
