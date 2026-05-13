use super::*;

#[test]
fn test_parse_github_pr_url() {
    assert_eq!(
        parse_github_pr_url("https://github.com/owner/repo/pull/123"),
        Some(("repo".to_string(), 123))
    );
    assert_eq!(
        parse_github_pr_url("https://github.com/my-org/my-repo/pull/456"),
        Some(("my-repo".to_string(), 456))
    );
    assert_eq!(
        parse_github_pr_url("https://github.com/my-org/my-repo"),
        None
    );
    assert_eq!(parse_github_pr_url("not a url"), None);
}
