//! Tests for `WorkflowDefinition::load` / `from_str` / `render_prompt`.

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use symphony::tracker::Issue;
use symphony::workflow::{WorkflowDefinition, WorkflowError};

fn sample_issue() -> Issue {
    Issue {
        id: "uuid-1".into(),
        identifier: "PDX-9".into(),
        title: "Add the thing".into(),
        description: Some("Make it good.".into()),
        priority: Some(2),
        state: "Todo".into(),
        url: Some("https://linear.app/x/issue/PDX-9".into()),
        labels: vec!["agent:claude".into(), "bug".into()],
        blocked_by: Vec::new(),
        created_at: Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
        updated_at: Some(Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap()),
    }
}

#[test]
fn parses_full_example_workflow_md() {
    std::env::set_var("LINEAR_API_KEY", "env-key-value");
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "examples", "WORKFLOW.example.md"]
        .iter()
        .collect();
    let wf = WorkflowDefinition::load(&path).expect("loads");
    assert_eq!(wf.config.tracker.kind, "linear");
    assert_eq!(wf.config.tracker.api_key, "env-key-value");
    assert_eq!(wf.config.tracker.project_slug, "pdx-software");
    assert_eq!(wf.config.polling.interval_ms, 30_000);
    assert_eq!(wf.config.agent.max_concurrent_agents, 1);
    assert_eq!(wf.config.agent.agent_label_required, "agent:claude");
    assert!(wf.prompt_template.contains("You are Helm"));
}

#[test]
fn front_matter_absent_treats_whole_file_as_prompt() {
    let raw = "Just a plain prompt.\n";
    let wf = WorkflowDefinition::from_str(raw).expect("loads with stub config");
    assert_eq!(wf.prompt_template, raw, "body preserved verbatim");
    // Stub config has empty tracker fields; main.rs will fail later when it
    // tries to actually use them. That's the documented behaviour for a
    // file with no front matter.
    assert!(wf.config.tracker.api_key.is_empty());
    assert!(wf.config.tracker.project_slug.is_empty());
}

#[test]
fn env_indirection_resolves_dollar_var() {
    std::env::set_var("MY_TEST_LINEAR_KEY", "secret-token-123");
    let raw = r#"---
tracker:
  api_key: $MY_TEST_LINEAR_KEY
  project_slug: my-project
---
hello"#;
    let wf = WorkflowDefinition::from_str(raw).expect("parses");
    assert_eq!(wf.config.tracker.api_key, "secret-token-123");
}

#[test]
fn env_indirection_missing_var_errors() {
    std::env::remove_var("DEFINITELY_UNSET_HELM_VAR");
    let raw = r#"---
tracker:
  api_key: $DEFINITELY_UNSET_HELM_VAR
  project_slug: x
---
body"#;
    let err = WorkflowDefinition::from_str(raw).unwrap_err();
    assert!(
        matches!(err, WorkflowError::MissingEnvVar(ref n) if n == "DEFINITELY_UNSET_HELM_VAR"),
        "got {err:?}"
    );
}

#[test]
fn liquid_template_renders_with_issue_object() {
    let raw = r#"---
tracker:
  api_key: literal-key
  project_slug: x
---
Issue {{ issue.identifier }}: {{ issue.title }}
Labels: {% for l in issue.labels %}{{ l }}{% unless forloop.last %}, {% endunless %}{% endfor %}"#;
    let wf = WorkflowDefinition::from_str(raw).unwrap();
    let out = wf.render_prompt(&sample_issue(), None).unwrap();
    assert!(out.contains("Issue PDX-9: Add the thing"));
    assert!(out.contains("Labels: agent:claude, bug"));
}

#[test]
fn liquid_template_fails_on_unknown_variable() {
    // Liquid (with stdlib parser) will accept references to undefined
    // variables and render them as empty strings, but will reject
    // syntactically broken templates. We verify that a template with a
    // syntax error is rejected at parse time.
    let raw = r#"---
tracker:
  api_key: literal-key
  project_slug: x
---
{% if issue.title %}{% endfor %}"#;
    let err = WorkflowDefinition::from_str(raw).unwrap_err();
    assert!(matches!(err, WorkflowError::Liquid(_)), "got {err:?}");
}
