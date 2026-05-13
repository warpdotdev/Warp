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

#[test]
fn file_button_label_prefers_filename() {
    assert_eq!(
        file_button_label("report.txt", "outputs/other.txt"),
        "report.txt"
    );
}

#[test]
fn file_button_label_falls_back_to_filepath_basename() {
    assert_eq!(file_button_label("", "outputs/report.txt"), "report.txt");
}

#[test]
fn file_button_label_falls_back_to_generic_label() {
    assert_eq!(file_button_label("", ""), "File");
}
