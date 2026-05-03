use once_cell::sync::Lazy;
use regex::Regex;

pub(crate) const MAX_POLICY_STRING_BYTES: usize = 8 * 1024;

static SECRET_ASSIGNMENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b([A-Z0-9_.-]*(?:TOKEN|SECRET|PASSWORD|PASSWD|API[_-]?KEY|ACCESS[_-]?KEY)[A-Z0-9_.-]*)=([^\s;&|]+)",
    )
    .expect("secret assignment regex should compile")
});

static AUTHORIZATION_BEARER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(authorization:\s*bearer\s+)([^\s"']+)"#)
        .expect("authorization header regex should compile")
});

static COMMON_TOKEN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(sk-[A-Za-z0-9_-]{12,}|gh[pousr]_[A-Za-z0-9_]{12,})\b")
        .expect("common token regex should compile")
});

pub(crate) fn redact_command_for_policy(command: &str) -> String {
    redact_sensitive_text_for_policy(command)
}

pub(crate) fn redact_sensitive_text_for_policy(value: &str) -> String {
    let value = SECRET_ASSIGNMENT_RE.replace_all(value, "$1=<redacted>");
    let value = AUTHORIZATION_BEARER_RE.replace_all(&value, "$1<redacted>");
    let value = COMMON_TOKEN_RE.replace_all(&value, "<redacted>");
    truncate_for_policy(&value)
}

pub(crate) fn mcp_argument_keys(arguments: &serde_json::Value) -> Vec<String> {
    let serde_json::Value::Object(map) = arguments else {
        return Vec::new();
    };

    let mut keys = map.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys
}

#[allow(dead_code)]
pub(crate) fn redact_sensitive_json_shape(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Null => serde_json::Value::Null,
        serde_json::Value::Bool(_) => serde_json::json!({ "type": "bool" }),
        serde_json::Value::Number(_) => serde_json::json!({ "type": "number" }),
        serde_json::Value::String(_) => serde_json::json!({ "type": "string" }),
        serde_json::Value::Array(values) => serde_json::json!({
            "type": "array",
            "length": values.len(),
        }),
        serde_json::Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            serde_json::json!({
                "type": "object",
                "keys": keys,
            })
        }
    }
}

pub(crate) fn truncate_for_policy(value: &str) -> String {
    if value.len() <= MAX_POLICY_STRING_BYTES {
        return value.to_string();
    }

    let mut end = MAX_POLICY_STRING_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }

    format!("{}...[truncated]", &value[..end])
}
