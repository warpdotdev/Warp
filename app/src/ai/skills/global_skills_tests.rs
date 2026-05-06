use super::resolve_skill_repos;
use crate::ai::cloud_environments::GithubRepo;

#[test]
fn resolve_skill_repos_returns_empty_for_empty_input() {
    assert_eq!(resolve_skill_repos(&[]), Vec::<GithubRepo>::new());
}

#[test]
fn resolve_skill_repos_skips_parse_failures() {
    let repos = resolve_skill_repos(&[
        String::new(),
        "warpdotdev/warp-internal:read-google-doc".to_string(),
    ]);

    assert_eq!(
        repos,
        vec![GithubRepo::new(
            "warpdotdev".to_string(),
            "warp-internal".to_string(),
        )]
    );
}

#[test]
fn resolve_skill_repos_skips_unqualified_and_repo_only_specs() {
    let repos = resolve_skill_repos(&[
        "bare-name".to_string(),
        ".agents/skills/read-google-doc/SKILL.md".to_string(),
        "warp-internal:read-google-doc".to_string(),
    ]);

    assert_eq!(repos, Vec::<GithubRepo>::new());
}

#[test]
fn resolve_skill_repos_collects_org_qualified_repos() {
    let repos = resolve_skill_repos(&[
        "warpdotdev/warp-internal:read-google-doc".to_string(),
        "warpdotdev/warp-server:deploy".to_string(),
    ]);

    assert_eq!(
        repos,
        vec![
            GithubRepo::new("warpdotdev".to_string(), "warp-internal".to_string()),
            GithubRepo::new("warpdotdev".to_string(), "warp-server".to_string()),
        ]
    );
}

#[test]
fn resolve_skill_repos_collapses_duplicates_preserving_first_seen_order() {
    let repos = resolve_skill_repos(&[
        "org-b/foo:first".to_string(),
        "org-a/foo:second".to_string(),
        "org-b/foo:third".to_string(),
    ]);

    assert_eq!(
        repos,
        vec![
            GithubRepo::new("org-b".to_string(), "foo".to_string()),
            GithubRepo::new("org-a".to_string(), "foo".to_string()),
        ]
    );
}
