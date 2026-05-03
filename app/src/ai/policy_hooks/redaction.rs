use once_cell::sync::Lazy;
use regex::Regex;

pub(crate) const MAX_POLICY_STRING_BYTES: usize = 8 * 1024;
pub(crate) const MAX_POLICY_COLLECTION_ITEMS: usize = 256;

static SECRET_ASSIGNMENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)\b([A-Z0-9_.-]*(?:TOKEN|SECRET|PASSWORD|PASSWD|API[_-]?KEY|ACCESS[_-]?KEY)[A-Z0-9_.-]*)=("(?:[^"\\]|\\.)*"|"(?:[^;&|]*)|'(?:[^'\\]|\\.)*'|'(?:[^;&|]*)|[^\s;&|]+)"#,
    )
    .expect("secret assignment regex should compile")
});

static AUTHORIZATION_BEARER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(authorization:\s*bearer\s+)([^\s"']+)"#)
        .expect("authorization header regex should compile")
});

static AUTHORIZATION_BASIC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(authorization:\s*basic\s+)([A-Za-z0-9+/=._-]+)"#)
        .expect("authorization basic regex should compile")
});

static URL_USERINFO_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b([a-z][a-z0-9+.-]*://)([^/\s"'<>@]+(?::[^/\s"'<>@]*)?@)"#)
        .expect("URL userinfo regex should compile")
});

static CURL_BASIC_AUTH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)(\bcurl\b[^;&|\n]*?\s(?:-u\s*|--user(?:=|\s+)|--proxy-user(?:=|\s+)))("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|[^\s;&|]+)"#,
    )
    .expect("curl basic auth regex should compile")
});

static SPLIT_SECRET_ARG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)(^|[\s;&|])(-{1,2}[a-z0-9_-]*(?:token|secret|password|passwd|api[-_]?key|access[-_]?key|authorization|auth)\b\s+)("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|bearer\s+[^\s;&|]+|basic\s+[^\s;&|]+|[^\s;&|]+)"#,
    )
    .expect("split secret arg regex should compile")
});

static INLINE_SECRET_ARG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)(^|[\s;&|])(-{1,2}[a-z0-9_-]*(?:token|secret|password|passwd|api[-_]?key|access[-_]?key|authorization|auth)\b=)("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|bearer\s+[^\s;&|]+|basic\s+[^\s;&|]+|[^\s;&|]+)"#,
    )
    .expect("inline secret arg regex should compile")
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
    let value = AUTHORIZATION_BASIC_RE.replace_all(&value, "$1<redacted>");
    let value = CURL_BASIC_AUTH_RE.replace_all(&value, "$1<redacted>");
    let value = URL_USERINFO_RE.replace_all(&value, "$1<redacted>@");
    let value = INLINE_SECRET_ARG_RE.replace_all(&value, "$1$2<redacted>");
    let value = SPLIT_SECRET_ARG_RE.replace_all(&value, "$1$2<redacted>");
    let value = COMMON_TOKEN_RE.replace_all(&value, "<redacted>");
    truncate_for_policy(&value)
}

pub(crate) fn mcp_argument_keys(arguments: &serde_json::Value) -> (Vec<String>, Option<usize>) {
    let serde_json::Value::Object(map) = arguments else {
        return (Vec::new(), None);
    };

    let mut keys = map
        .keys()
        .take(MAX_POLICY_COLLECTION_ITEMS)
        .map(|key| truncate_for_policy(key))
        .collect::<Vec<_>>();
    keys.sort();
    let omitted_count = map.len().saturating_sub(keys.len());
    (keys, (omitted_count > 0).then_some(omitted_count))
}

pub(crate) fn capped_policy_items<T>(
    items: impl IntoIterator<Item = T>,
) -> (Vec<T>, Option<usize>) {
    let mut capped = Vec::new();
    let mut total_count = 0usize;
    for item in items {
        if capped.len() < MAX_POLICY_COLLECTION_ITEMS {
            capped.push(item);
        }
        total_count = total_count.saturating_add(1);
    }

    let omitted_count = total_count.saturating_sub(capped.len());
    (capped, (omitted_count > 0).then_some(omitted_count))
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
