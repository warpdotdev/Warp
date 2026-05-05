use crate::provider::{is_valid_env_var, Secret};

#[test]
fn test_secret_new_valid() {
    let s = Secret::new("API_KEY".to_string(), "mysecret".to_string());
    assert!(s.is_ok());
}

#[test]
fn test_secret_new_rejects_shell_injection() {
    let s = Secret::new("FOO$(cmd)".to_string(), "value".to_string());
    assert!(s.is_err());
}

#[test]
fn test_secret_new_rejects_space() {
    let s = Secret::new("FOO BAR".to_string(), "value".to_string());
    assert!(s.is_err());
}

#[test]
fn test_secret_new_rejects_leading_digit() {
    let s = Secret::new("1FOO".to_string(), "value".to_string());
    assert!(s.is_err());
}

#[test]
fn test_secret_new_allows_underscore_prefix() {
    let s = Secret::new("_FOO".to_string(), "value".to_string());
    assert!(s.is_ok());
}

#[test]
fn test_env_var_getter_matches_input() {
    let s = Secret::new("MY_KEY".to_string(), "val".to_string()).unwrap();
    assert_eq!(s.env_var(), "MY_KEY");
}

#[test]
fn test_is_valid_env_var() {
    assert!(is_valid_env_var("VALID_KEY"));
    assert!(is_valid_env_var("_PRIVATE"));
    assert!(!is_valid_env_var("1INVALID"));
    assert!(!is_valid_env_var("FOO BAR"));
    assert!(!is_valid_env_var("FOO$(cmd)"));
    assert!(!is_valid_env_var(""));
}
