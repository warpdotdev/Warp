use std::path::Path;

use command::r#async::Command;
use command::Stdio;
use tempfile::TempDir;

use super::{detect_current_branch, detect_current_branch_display};

/// Helper: run a git command inside the given repo directory.
async fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("failed to run git");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Creates a temp git repo with one commit and returns `(dir_handle, repo_path)`.
async fn init_repo() -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().to_path_buf();

    git(&path, &["init", "-b", "main"]).await;
    git(&path, &["config", "user.email", "test@test.com"]).await;
    git(&path, &["config", "user.name", "Test"]).await;
    git(&path, &["commit", "--allow-empty", "-m", "initial"]).await;

    (dir, path)
}

#[tokio::test]
async fn on_normal_branch_returns_branch_name() {
    let (_dir, repo) = init_repo().await;
    git(&repo, &["checkout", "-b", "feature-xyz"]).await;

    assert_eq!(detect_current_branch(&repo).await.unwrap(), "feature-xyz");
    assert_eq!(
        detect_current_branch_display(&repo).await.unwrap(),
        "feature-xyz"
    );
}

#[tokio::test]
async fn detached_head_raw_returns_head() {
    let (_dir, repo) = init_repo().await;
    git(&repo, &["checkout", "--detach", "HEAD"]).await;

    assert_eq!(detect_current_branch(&repo).await.unwrap(), "HEAD");
}

#[tokio::test]
async fn detached_head_display_returns_short_sha() {
    let (_dir, repo) = init_repo().await;
    let full_sha = git(&repo, &["rev-parse", "HEAD"]).await;
    git(&repo, &["checkout", "--detach", "HEAD"]).await;

    let result = detect_current_branch_display(&repo).await.unwrap();

    assert_ne!(
        result, "HEAD",
        "display variant should not return literal HEAD"
    );
    assert!(
        full_sha.starts_with(&result),
        "expected {full_sha} to start with {result}"
    );
}

#[tokio::test]
async fn detached_tag_display_returns_short_sha() {
    let (_dir, repo) = init_repo().await;
    git(&repo, &["tag", "v1.0"]).await;
    git(&repo, &["checkout", "v1.0"]).await;

    let full_sha = git(&repo, &["rev-parse", "HEAD"]).await;
    let result = detect_current_branch_display(&repo).await.unwrap();

    assert_ne!(result, "HEAD");
    assert!(
        full_sha.starts_with(&result),
        "expected {full_sha} to start with {result}"
    );
}

// ── PrInfo parser tests ──────────────────────────────────────────────────────
//
// These exercise the pure `parse_pr_info` helper against representative
// `gh pr view --json …` payloads so we don't have to spawn `gh` to verify
// schema handling.

#[cfg(feature = "local_fs")]
mod pr_info_parser {
    use super::super::{
        parse_pr_info, PrMergeState, PrReviewDecision, PrReviewState, PrState,
    };
    use serde_json::json;

    fn full_payload() -> serde_json::Value {
        json!({
            "number": 10432,
            "url": "https://github.com/warp/warp/pull/10432",
            "title": "Refactor foo crate",
            "state": "OPEN",
            "isDraft": false,
            "reviewDecision": "APPROVED",
            "reviewRequests": [{"login": "carol"}],
            "latestReviews": [
                {"author": {"login": "alice"}, "state": "APPROVED"},
                {"author": {"login": "bob"}, "state": "CHANGES_REQUESTED"}
            ],
            "mergeStateStatus": "CLEAN",
            "statusCheckRollup": [
                {"conclusion": "SUCCESS"},
                {"conclusion": "FAILURE"},
                {"conclusion": "PENDING"}
            ]
        })
    }

    #[test]
    fn parses_full_payload() {
        let info = parse_pr_info(&full_payload()).expect("parse");
        assert_eq!(info.number, 10432);
        assert_eq!(info.url, "https://github.com/warp/warp/pull/10432");
        assert_eq!(info.title.as_deref(), Some("Refactor foo crate"));
        assert_eq!(info.state, Some(PrState::Open));
        assert_eq!(info.review_decision, Some(PrReviewDecision::Approved));
        assert_eq!(info.merge_state, Some(PrMergeState::Clean));

        let checks = info.checks.expect("checks");
        assert_eq!(checks.passing, 1);
        assert_eq!(checks.failing, 1);
        assert_eq!(checks.pending, 1);

        let logins: Vec<_> = info.reviewers.iter().map(|r| r.login.as_str()).collect();
        assert!(logins.contains(&"alice"));
        assert!(logins.contains(&"bob"));
        assert!(logins.contains(&"carol"));
        let alice = info.reviewers.iter().find(|r| r.login == "alice").unwrap();
        assert_eq!(alice.state, PrReviewState::Approved);
        let bob = info.reviewers.iter().find(|r| r.login == "bob").unwrap();
        assert_eq!(bob.state, PrReviewState::ChangesRequested);
        let carol = info.reviewers.iter().find(|r| r.login == "carol").unwrap();
        assert_eq!(carol.state, PrReviewState::Requested);
    }

    #[test]
    fn folds_is_draft_into_pr_state() {
        let mut payload = full_payload();
        payload["isDraft"] = json!(true);
        let info = parse_pr_info(&payload).expect("parse");
        assert_eq!(info.state, Some(PrState::Draft));
    }

    #[test]
    fn parses_merged_state() {
        let mut payload = full_payload();
        payload["state"] = json!("MERGED");
        payload["isDraft"] = json!(false);
        let info = parse_pr_info(&payload).expect("parse");
        assert_eq!(info.state, Some(PrState::Merged));
    }

    #[test]
    fn parses_closed_state() {
        let mut payload = full_payload();
        payload["state"] = json!("CLOSED");
        let info = parse_pr_info(&payload).expect("parse");
        assert_eq!(info.state, Some(PrState::Closed));
    }

    #[test]
    fn missing_optional_fields_degrade_to_none() {
        let payload = json!({
            "number": 1,
            "url": "https://example.com/pull/1"
        });
        let info = parse_pr_info(&payload).expect("parse");
        assert_eq!(info.number, 1);
        assert!(info.title.is_none());
        assert!(info.state.is_none());
        assert!(info.review_decision.is_none());
        assert!(info.reviewers.is_empty());
        assert!(info.merge_state.is_none());
        assert!(info.checks.is_none());
    }

    #[test]
    fn blank_title_degrades_to_none() {
        let mut payload = full_payload();
        payload["title"] = json!("   ");
        let info = parse_pr_info(&payload).expect("parse");
        assert!(info.title.is_none());
    }

    #[test]
    fn missing_number_is_an_error() {
        let payload = json!({"url": "https://example.com/pull/1"});
        assert!(parse_pr_info(&payload).is_err());
    }

    #[test]
    fn missing_url_is_an_error() {
        let payload = json!({"number": 1});
        assert!(parse_pr_info(&payload).is_err());
    }

    #[test]
    fn unknown_merge_state_degrades_to_none() {
        let mut payload = full_payload();
        payload["mergeStateStatus"] = json!("SOMETHING_NEW");
        let info = parse_pr_info(&payload).expect("parse");
        assert!(info.merge_state.is_none());
    }

    #[test]
    fn unknown_review_decision_degrades_to_none() {
        let mut payload = full_payload();
        payload["reviewDecision"] = json!("MAYBE");
        let info = parse_pr_info(&payload).expect("parse");
        assert!(info.review_decision.is_none());
    }

    #[test]
    fn empty_check_rollup_degrades_to_none() {
        let mut payload = full_payload();
        payload["statusCheckRollup"] = json!([]);
        let info = parse_pr_info(&payload).expect("parse");
        assert!(info.checks.is_none());
    }

    #[test]
    fn reviewer_dedupe_prefers_actual_review_over_pending_request() {
        let payload = json!({
            "number": 1,
            "url": "https://example.com/pull/1",
            "reviewRequests": [{"login": "alice"}],
            "latestReviews": [
                {"author": {"login": "alice"}, "state": "APPROVED"}
            ]
        });
        let info = parse_pr_info(&payload).expect("parse");
        assert_eq!(info.reviewers.len(), 1);
        assert_eq!(info.reviewers[0].login, "alice");
        assert_eq!(info.reviewers[0].state, PrReviewState::Approved);
    }
}
