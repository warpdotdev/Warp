use std::fs;
use std::path::Path;

use orchestrator::Role;
use skills::{LoaderConfig, SkillRegistry, SkillsError};
use tempfile::TempDir;

fn write_skill(dir: &Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create skill parent dir");
    }
    fs::write(&path, contents).expect("write skill file");
}

#[tokio::test]
async fn loads_skill_with_full_front_matter() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "deploy.md",
        "---\n\
name: cloudflare-deploy\n\
description: Deploy a Worker via wrangler with safe defaults\n\
roles: [Worker, BulkRefactor]\n\
tags: [cloudflare, deploy]\n\
---\n\
# Body heading\n\
Some details about deploying.\n",
    );

    let registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(tmp.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    assert_eq!(registry.len(), 1);
    let skill = registry.get("cloudflare-deploy").expect("found by name");
    assert_eq!(skill.name, "cloudflare-deploy");
    assert_eq!(
        skill.description,
        "Deploy a Worker via wrangler with safe defaults"
    );
    assert_eq!(skill.roles, vec![Role::Worker, Role::BulkRefactor]);
    assert_eq!(
        skill.tags,
        vec!["cloudflare".to_string(), "deploy".to_string()]
    );
    assert!(skill.body.contains("Body heading"));
    assert!(skill.body.contains("Some details about deploying."));
    assert!(!skill.body.starts_with("---"));
}

#[tokio::test]
async fn loads_skill_without_front_matter_using_filename() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "shell-tips.md",
        "\n\n# Shell tips\n\nUse rg instead of grep.\n",
    );

    let registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(tmp.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    let skill = registry.get("shell-tips").expect("filename-derived name");
    assert_eq!(skill.name, "shell-tips");
    // Description falls back to first non-blank line of body, with leading
    // markdown heading marks stripped.
    assert_eq!(skill.description, "Shell tips");
    assert!(skill.roles.is_empty(), "no roles -> applies to all");
    assert!(skill.tags.is_empty());
}

#[tokio::test]
async fn repo_skills_override_user_skills_on_name_conflict() {
    let user = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();

    write_skill(
        user.path(),
        "shared.md",
        "---\nname: shared\ndescription: user version\n---\nuser body\n",
    );
    write_skill(
        repo.path(),
        "shared.md",
        "---\nname: shared\ndescription: repo version\n---\nrepo body\n",
    );

    let registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(user.path().to_path_buf()),
        repo_root: Some(repo.path().to_path_buf()),
    })
    .await
    .expect("load");

    let skill = registry.get("shared").expect("repo override");
    assert_eq!(skill.description, "repo version");
    assert!(skill.body.contains("repo body"));
    assert_eq!(registry.len(), 1);
}

#[tokio::test]
async fn select_for_filters_by_role() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "worker-only.md",
        "---\nname: worker-only\nroles: [Worker]\ntags: [a]\n---\nbody\n",
    );
    write_skill(
        tmp.path(),
        "planner-only.md",
        "---\nname: planner-only\nroles: [Planner]\ntags: [a]\n---\nbody\n",
    );
    write_skill(
        tmp.path(),
        "any-role.md",
        "---\nname: any-role\ntags: [a]\n---\nbody\n",
    );

    let registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(tmp.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    let selected = registry.select_for(Role::Worker, &["a".to_string()]);
    let names: Vec<&str> = selected.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"worker-only"));
    assert!(names.contains(&"any-role"));
    assert!(!names.contains(&"planner-only"));
    assert_eq!(selected.len(), 2);
}

#[tokio::test]
async fn select_for_filters_by_tag_intersection() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "deploy.md",
        "---\nname: deploy\ntags: [cloudflare, deploy]\n---\nbody\n",
    );
    write_skill(
        tmp.path(),
        "lint.md",
        "---\nname: lint\ntags: [rust, lint]\n---\nbody\n",
    );

    let registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(tmp.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    let selected = registry.select_for(Role::Worker, &["cloudflare".to_string()]);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].name, "deploy");

    let selected = registry.select_for(Role::Worker, &["rust".to_string()]);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].name, "lint");

    let none = registry.select_for(Role::Worker, &["nope".to_string()]);
    assert!(none.is_empty());
}

#[tokio::test]
async fn select_for_includes_role_agnostic_skills_when_no_role_specified_in_skill() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "general.md",
        "---\nname: general\n---\nbody\n", // no roles, no tags
    );
    write_skill(
        tmp.path(),
        "scoped.md",
        "---\nname: scoped\nroles: [Reviewer]\n---\nbody\n",
    );

    let registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(tmp.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    // Calling with an arbitrary role and no wanted tags should pick up the
    // role-agnostic skill but not the Reviewer-only one.
    let selected = registry.select_for(Role::Worker, &[]);
    let names: Vec<&str> = selected.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"general"));
    assert!(!names.contains(&"scoped"));
}

#[tokio::test]
async fn walkdir_finds_skills_in_subdirectories() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "a/b/c/nested.md",
        "---\nname: nested\n---\nbody\n",
    );
    write_skill(tmp.path(), "top.md", "---\nname: top\n---\nbody\n");

    let registry = SkillRegistry::load(LoaderConfig {
        user_root: Some(tmp.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect("load");

    assert!(registry.get("nested").is_some());
    assert!(registry.get("top").is_some());
    assert_eq!(registry.len(), 2);
}

#[tokio::test]
async fn front_matter_yaml_error_returns_error() {
    let tmp = TempDir::new().unwrap();
    // `roles` is supposed to be a sequence of strings — passing a mapping
    // makes serde_yaml fail.
    write_skill(
        tmp.path(),
        "broken.md",
        "---\nname: broken\nroles:\n  not: a-sequence\n---\nbody\n",
    );

    let err = SkillRegistry::load(LoaderConfig {
        user_root: Some(tmp.path().to_path_buf()),
        repo_root: None,
    })
    .await
    .expect_err("expected yaml error");

    match err {
        SkillsError::Yaml { path, .. } => {
            assert!(path.contains("broken.md"));
        }
        other => panic!("expected SkillsError::Yaml, got {other:?}"),
    }
}
