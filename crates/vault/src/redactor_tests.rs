use crate::redactor::Redactor;

#[test]
fn test_redacts_known_value() {
    let mut r = Redactor::new();
    r.register("supersecret".to_string());
    assert_eq!(
        r.redact("my token is supersecret ok"),
        "my token is [REDACTED] ok"
    );
}

#[test]
fn test_redacts_multiple_values() {
    let mut r = Redactor::new();
    r.register("token123".to_string());
    r.register("password456".to_string());
    let output = r.redact("token=token123 pass=password456");
    assert!(!output.contains("token123"));
    assert!(!output.contains("password456"));
    assert!(output.contains("[REDACTED]"));
}

#[test]
fn test_no_redaction_for_unknown_value() {
    let mut r = Redactor::new();
    r.register("secret".to_string());
    assert_eq!(r.redact("nothing to hide here"), "nothing to hide here");
}

#[test]
fn test_empty_secret_not_redacted() {
    let mut r = Redactor::new();
    r.register("".to_string());
    assert_eq!(r.redact("some output"), "some output");
}

#[test]
fn test_redacts_multiple_occurrences() {
    let mut r = Redactor::new();
    r.register("abc".to_string());
    assert_eq!(r.redact("abc and abc"), "[REDACTED] and [REDACTED]");
}
