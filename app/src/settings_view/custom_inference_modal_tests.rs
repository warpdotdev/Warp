use super::*;

#[test]
fn validate_url_accepts_https_with_host() {
    assert!(validate_url("https://api.example.com/v1").is_ok());
    assert!(validate_url("https://example.com").is_ok());
    assert!(validate_url("https://localhost:8080").is_ok());
}

#[test]
fn validate_url_rejects_http() {
    assert_eq!(
        validate_url("http://api.example.com/v1"),
        Err("URL must use HTTPS")
    );
    assert_eq!(
        validate_url("http://example.com"),
        Err("URL must use HTTPS")
    );
}

#[test]
fn validate_url_rejects_ftp_and_other_schemes() {
    assert_eq!(
        validate_url("ftp://files.example.com"),
        Err("URL must use HTTPS")
    );
    assert_eq!(
        validate_url("file:///etc/passwd"),
        Err("URL must use HTTPS")
    );
    assert_eq!(
        validate_url("ws://socket.example.com"),
        Err("URL must use HTTPS")
    );
}

#[test]
fn validate_url_rejects_malformed_strings() {
    assert_eq!(validate_url("not a url"), Err("Invalid URL"));
    assert_eq!(validate_url("https://"), Err("Invalid URL"));
}

#[test]
fn validate_url_rejects_empty_host() {
    assert_eq!(validate_url("https://?query=1"), Err("Invalid URL"));
}

#[test]
fn validate_url_allows_empty_string() {
    assert!(validate_url("").is_ok());
}

#[test]
fn validate_url_allows_whitespace_only() {
    assert!(validate_url("   ").is_ok());
}
