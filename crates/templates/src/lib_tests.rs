use std::collections::HashMap;

use super::{is_valid_placeholder, substitute, TemplateManifest};

fn ctx(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// ── substitute ──────────────────────────────────────────────────

#[test]
fn substitute_simple() {
    assert_eq!(substitute("Hello {{name}}!", &ctx(&[("name", "Helm")])), "Hello Helm!");
}

#[test]
fn substitute_unknown_placeholder_preserved() {
    let out = substitute("{{a}} {{b}}", &ctx(&[("a", "A")]));
    assert_eq!(out, "A {{b}}");
}

#[test]
fn substitute_repeated_var() {
    assert_eq!(substitute("{{x}}-{{x}}", &ctx(&[("x", "hi")])), "hi-hi");
}

#[test]
fn substitute_empty_context_leaves_input_unchanged() {
    let s = "{{a}} and {{b}}";
    assert_eq!(substitute(s, &HashMap::new()), s);
}

#[test]
fn substitute_triple_brace_emitted_verbatim() {
    // {{{x}}} is an escape block; {{x}} after it should still be substituted.
    let out = substitute("{{{x}}} {{x}}", &ctx(&[("x", "X")]));
    assert_eq!(out, "{{{x}}} X");
}

#[test]
fn substitute_spaced_placeholder_not_replaced() {
    // Spaces inside braces make it an invalid placeholder.
    let out = substitute("{{ name }}", &ctx(&[("name", "Helm")]));
    assert_eq!(out, "{{ name }}");
}

#[test]
fn substitute_trailing_space_in_placeholder_not_replaced() {
    let out = substitute("{{name }}", &ctx(&[("name", "Helm")]));
    assert_eq!(out, "{{name }}");
}

#[test]
fn substitute_no_closing_braces_preserved() {
    let out = substitute("{{name", &ctx(&[("name", "Helm")]));
    assert_eq!(out, "{{name");
}

#[test]
fn substitute_multiple_vars() {
    let out = substitute(
        "{{a}}/{{b}}/{{c}}",
        &ctx(&[("a", "x"), ("b", "y"), ("c", "z")]),
    );
    assert_eq!(out, "x/y/z");
}

#[test]
fn substitute_unicode_values() {
    let out = substitute("Hello {{name}}!", &ctx(&[("name", "世界")]));
    assert_eq!(out, "Hello 世界!");
}

// ── is_valid_placeholder ────────────────────────────────────────

#[test]
fn valid_placeholder_identifiers() {
    assert!(is_valid_placeholder("project_name"));
    assert!(is_valid_placeholder("author"));
    assert!(is_valid_placeholder("my-var"));
    assert!(is_valid_placeholder("_underscore"));
    assert!(is_valid_placeholder("-dash-start"));
}

#[test]
fn invalid_placeholder_identifiers() {
    assert!(!is_valid_placeholder(""));
    assert!(!is_valid_placeholder("has space"));
    assert!(!is_valid_placeholder("1starts_digit"));
    assert!(!is_valid_placeholder(" leading_space"));
}

// ── manifest deserialisation ────────────────────────────────────

#[test]
fn manifest_deserializes_minimal() {
    let toml = r#"name = "my-template""#;
    let m: TemplateManifest = toml_edit::de::from_str(toml).unwrap();
    assert_eq!(m.name, "my-template");
    assert!(m.variables.is_empty());
    assert!(m.hooks.post_init.is_empty());
}

#[test]
fn manifest_deserializes_full() {
    let toml = r#"
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
"#;
    let m: TemplateManifest = toml_edit::de::from_str(toml).unwrap();
    assert_eq!(m.name, "cloudflare-fullstack");
    assert_eq!(m.description, "Fullstack Cloudflare Workers project");
    assert_eq!(m.version, "0.1.0");
    assert_eq!(m.variables.len(), 2);
    assert!(m.variables[0].required);
    assert_eq!(m.variables[1].default, "anon");
    assert_eq!(m.hooks.post_init, vec!["npm install", "git init"]);
}
