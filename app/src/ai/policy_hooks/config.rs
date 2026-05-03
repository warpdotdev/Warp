use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use http::header::HeaderName;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use thiserror::Error;

use super::decision::AgentPolicyUnavailableDecision;

pub(crate) const DEFAULT_AGENT_POLICY_HOOK_TIMEOUT_MS: u64 = 5_000;
pub(crate) const MAX_AGENT_POLICY_HOOK_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(default)]
pub(crate) struct AgentPolicyHookConfig {
    pub enabled: bool,
    pub before_action: Vec<AgentPolicyHook>,
    pub timeout_ms: u64,
    pub on_unavailable: AgentPolicyUnavailableDecision,
    pub allow_hook_autoapproval: bool,
}

impl Default for AgentPolicyHookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            before_action: Vec::new(),
            timeout_ms: DEFAULT_AGENT_POLICY_HOOK_TIMEOUT_MS,
            on_unavailable: AgentPolicyUnavailableDecision::Ask,
            allow_hook_autoapproval: false,
        }
    }
}

impl AgentPolicyHookConfig {
    pub(crate) fn is_active(&self) -> bool {
        self.enabled
    }

    fn validate_safe_to_persist(&self) -> Result<(), AgentPolicyHookConfigError> {
        for hook in &self.before_action {
            hook.validate_safe_to_persist()?;
        }

        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), AgentPolicyHookConfigError> {
        self.validate_safe_to_persist()?;

        if !self.enabled {
            return Ok(());
        }

        validate_timeout_ms(self.timeout_ms)?;

        if self.before_action.is_empty() {
            return Err(AgentPolicyHookConfigError::NoBeforeActionHooks);
        }

        for hook in &self.before_action {
            hook.validate()?;
        }

        Ok(())
    }

    pub(crate) fn hook_timeout_ms(&self, hook: &AgentPolicyHook) -> u64 {
        hook.timeout_ms.unwrap_or(self.timeout_ms)
    }

    pub(crate) fn hook_unavailable_decision(
        &self,
        hook: &AgentPolicyHook,
    ) -> AgentPolicyUnavailableDecision {
        hook.on_unavailable.unwrap_or(self.on_unavailable)
    }

    pub(crate) fn allow_autoapproval_for_all_hooks(&self) -> bool {
        !self.before_action.is_empty()
            && (self.allow_hook_autoapproval
                || self
                    .before_action
                    .iter()
                    .all(|hook| hook.allow_autoapproval))
    }
}

impl Serialize for AgentPolicyHookConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let sanitized_config;
        let config = if self.validate_safe_to_persist().is_ok() {
            self
        } else {
            sanitized_config = Self::default();
            &sanitized_config
        };

        let mut state = serializer.serialize_struct("AgentPolicyHookConfig", 5)?;
        state.serialize_field("enabled", &config.enabled)?;
        state.serialize_field("before_action", &config.before_action)?;
        state.serialize_field("timeout_ms", &config.timeout_ms)?;
        state.serialize_field("on_unavailable", &config.on_unavailable)?;
        state.serialize_field("allow_hook_autoapproval", &config.allow_hook_autoapproval)?;
        state.end()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct AgentPolicyHook {
    pub name: String,
    pub timeout_ms: Option<u64>,
    pub on_unavailable: Option<AgentPolicyUnavailableDecision>,
    pub allow_autoapproval: bool,
    #[serde(flatten)]
    pub transport: AgentPolicyHookTransport,
}

impl AgentPolicyHook {
    fn validate_safe_to_persist(&self) -> Result<(), AgentPolicyHookConfigError> {
        self.transport.validate_safe_to_persist()
    }

    pub(crate) fn validate(&self) -> Result<(), AgentPolicyHookConfigError> {
        if self.name.trim().is_empty() {
            return Err(AgentPolicyHookConfigError::MissingHookName);
        }

        if let Some(timeout_ms) = self.timeout_ms {
            validate_timeout_ms(timeout_ms)?;
        }

        self.transport.validate()
    }
}

impl Default for AgentPolicyHook {
    fn default() -> Self {
        Self {
            name: String::new(),
            timeout_ms: None,
            on_unavailable: None,
            allow_autoapproval: false,
            transport: AgentPolicyHookTransport::Stdio {
                command: String::new(),
                args: Vec::new(),
                env: BTreeMap::new(),
                working_directory: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub(crate) enum AgentPolicyHookTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        env: BTreeMap<String, AgentPolicyHookSecretValue>,
        #[serde(default)]
        working_directory: Option<PathBuf>,
    },
    Http {
        url: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, AgentPolicyHookSecretValue>,
    },
}

impl AgentPolicyHookTransport {
    fn validate_safe_to_persist(&self) -> Result<(), AgentPolicyHookConfigError> {
        match self {
            Self::Stdio {
                command, args, env, ..
            } => {
                validate_stdio_command(command)?;
                validate_stdio_args(args)?;
                validate_stdio_secret_value_map(env)?;
            }
            Self::Http { url, headers } => {
                if http_url_contains_credentials(url) {
                    return Err(AgentPolicyHookConfigError::HttpUrlContainsCredentials);
                }
                validate_http_secret_value_map(headers)?;
            }
        }

        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), AgentPolicyHookConfigError> {
        match self {
            Self::Stdio {
                command,
                args,
                env,
                working_directory,
            } => {
                if command.trim().is_empty() {
                    return Err(AgentPolicyHookConfigError::MissingStdioCommand);
                }
                validate_stdio_command(command)?;
                validate_stdio_args(args)?;
                validate_stdio_secret_value_map(env)?;

                if working_directory
                    .as_deref()
                    .is_some_and(|path| path.as_os_str().is_empty())
                {
                    return Err(AgentPolicyHookConfigError::InvalidWorkingDirectory(
                        Path::new("").to_path_buf(),
                    ));
                }
            }
            Self::Http { url, headers } => {
                if http_url_contains_credentials(url) {
                    return Err(AgentPolicyHookConfigError::HttpUrlContainsCredentials);
                }

                let parsed = url::Url::parse(url)
                    .map_err(|_| AgentPolicyHookConfigError::InvalidHttpUrl(url.clone()))?;

                let host = parsed.host_str().unwrap_or_default();
                let is_localhost = matches!(host, "localhost" | "127.0.0.1" | "::1");
                let is_allowed_local_http = parsed.scheme() == "http" && is_localhost;
                if parsed.scheme() != "https" && !is_allowed_local_http {
                    return Err(AgentPolicyHookConfigError::InsecureHttpUrl(url.clone()));
                }

                validate_http_secret_value_map(headers)?;
            }
        }

        Ok(())
    }
}

/// Reference to a local environment variable that supplies a hook credential at runtime.
/// The profile persists only the environment variable name, never the credential value.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentPolicyHookSecretValue {
    env: String,
}

impl AgentPolicyHookSecretValue {
    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn resolved_value(&self) -> Result<String, String> {
        std::env::var(&self.env).map_err(|_| self.env.clone())
    }

    #[cfg(target_family = "wasm")]
    pub(crate) fn resolved_value(&self) -> Result<String, String> {
        Err(self.env.clone())
    }

    fn validate(&self) -> Result<(), AgentPolicyHookConfigError> {
        let env = self.env.trim();
        if env.is_empty() {
            return Err(AgentPolicyHookConfigError::MissingSecretEnvironmentVariableName);
        }
        if env != self.env || !is_env_reference_name(env) || text_contains_common_token(env) {
            return Err(AgentPolicyHookConfigError::InvalidSecretEnvironmentVariableName);
        }
        Ok(())
    }
}

impl fmt::Debug for AgentPolicyHookSecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Env").field("env", &self.env).finish()
    }
}

fn validate_stdio_secret_value_map(
    values: &BTreeMap<String, AgentPolicyHookSecretValue>,
) -> Result<(), AgentPolicyHookConfigError> {
    for (name, value) in values {
        if !is_env_reference_name(name) || text_contains_common_token(name) {
            return Err(AgentPolicyHookConfigError::InvalidSecretEnvironmentVariableName);
        }
        value.validate()?;
    }
    Ok(())
}

fn validate_http_secret_value_map(
    values: &BTreeMap<String, AgentPolicyHookSecretValue>,
) -> Result<(), AgentPolicyHookConfigError> {
    for (name, value) in values {
        if HeaderName::from_bytes(name.as_bytes()).is_err() || text_contains_common_token(name) {
            return Err(AgentPolicyHookConfigError::InvalidHttpHeaderName(
                name.clone(),
            ));
        }
        value.validate()?;
    }
    Ok(())
}

fn validate_stdio_args(args: &[String]) -> Result<(), AgentPolicyHookConfigError> {
    if args.iter().any(|arg| stdio_arg_contains_credentials(arg)) {
        return Err(AgentPolicyHookConfigError::StdioArgContainsCredentials);
    }
    if args.windows(2).any(|args| {
        stdio_arg_expects_secret_value(&args[0]) && stdio_arg_value_is_literal_secret(&args[1])
    }) {
        return Err(AgentPolicyHookConfigError::StdioArgContainsCredentials);
    }
    Ok(())
}

fn validate_stdio_command(command: &str) -> Result<(), AgentPolicyHookConfigError> {
    if stdio_arg_contains_credentials(command) {
        return Err(AgentPolicyHookConfigError::StdioCommandContainsCredentials);
    }

    let words = command.split_ascii_whitespace().collect::<Vec<_>>();
    if words.windows(2).any(|words| {
        stdio_arg_expects_secret_value(words[0]) && stdio_arg_value_is_literal_secret(words[1])
    }) {
        return Err(AgentPolicyHookConfigError::StdioCommandContainsCredentials);
    }

    Ok(())
}

fn http_url_contains_credentials(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        return !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query_pairs().any(|(key, value)| {
                text_contains_credentials(&key) || text_contains_credentials(&value)
            })
            || parsed.fragment().is_some_and(text_contains_credentials);
    }

    let url = url.trim_start();
    let Some(scheme_end) = url.find(':') else {
        return false;
    };
    let scheme = &url[..scheme_end];
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }

    let mut authority_start = scheme_end + 1;
    if url[authority_start..].starts_with("//") {
        authority_start += 2;
    } else if url[authority_start..].starts_with('/') {
        authority_start += 1;
    }

    let authority_end = url[authority_start..]
        .find(|ch| matches!(ch, '/' | '?' | '#'))
        .map(|offset| authority_start + offset)
        .unwrap_or(url.len());

    if url[authority_start..authority_end].contains('@') {
        return true;
    }

    let suffix = &url[authority_end..];
    suffix
        .find(|ch| matches!(ch, '?' | '#'))
        .is_some_and(|offset| text_contains_credentials(&suffix[offset + 1..]))
}

fn text_contains_credentials(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("bearer ") || lower.contains("bearer%20") {
        return true;
    }
    if lower.contains("basic ") || lower.contains("basic%20") {
        return true;
    }

    let normalized = lower.replace(['_', '-'], "");
    if normalized.contains("apikey")
        || normalized.contains("accesskey")
        || normalized.ends_with("token")
        || normalized.ends_with("secret")
        || normalized.ends_with("password")
        || normalized.ends_with("passwd")
        || normalized.ends_with("authorization")
    {
        return true;
    }

    lower
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|part| {
            matches!(
                part,
                "token" | "secret" | "password" | "passwd" | "authorization"
            )
        })
        || text_contains_common_token(value)
}

fn stdio_arg_contains_credentials(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if let Some(offset) = lower.find("authorization:") {
        let value = value[offset + "authorization:".len()..].trim();
        if !value.is_empty() && !stdio_arg_value_uses_env_secret_reference(value) {
            return true;
        }
    }

    if (lower.contains("bearer ") || lower.contains("basic "))
        && !stdio_arg_value_uses_env_secret_reference(value)
    {
        return true;
    }

    if let Some((name, secret)) = value.split_once('=') {
        let secret = secret.trim();
        if text_contains_credentials(name)
            && !secret.is_empty()
            && !stdio_arg_value_uses_env_secret_reference(secret)
        {
            return true;
        }
    }

    text_contains_common_token(value)
}

fn text_contains_common_token(value: &str) -> bool {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_')
        .any(|part| {
            part.strip_prefix("sk-")
                .is_some_and(|token| token.len() >= 12)
                || ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"]
                    .iter()
                    .any(|prefix| {
                        part.strip_prefix(prefix).is_some_and(|token| {
                            token.len() >= 12
                                && token
                                    .chars()
                                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                        })
                    })
        })
}

fn stdio_arg_expects_secret_value(value: &str) -> bool {
    let value = value
        .trim()
        .trim_matches(|ch| ch == '"' || ch == '\'')
        .trim();
    if value.contains('=') {
        return false;
    }
    let is_flag = value.starts_with('-');
    let is_header_name = value.ends_with(':');
    if !is_flag && !is_header_name {
        return false;
    }
    let value = value.trim_start_matches('-').trim_end_matches(':');
    let normalized = value.to_ascii_lowercase().replace(['_', '-'], "");

    normalized.contains("apikey")
        || normalized.contains("accesskey")
        || normalized.ends_with("token")
        || normalized.ends_with("secret")
        || normalized.ends_with("password")
        || normalized.ends_with("passwd")
        || normalized.ends_with("authorization")
        || normalized == "auth"
}

fn stdio_arg_value_is_literal_secret(value: &str) -> bool {
    let value = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    !value.is_empty() && !stdio_arg_value_uses_env_secret_reference(value)
}

fn stdio_arg_value_uses_env_secret_reference(value: &str) -> bool {
    let value = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    let value = strip_ascii_case_prefix(value, "authorization:")
        .unwrap_or(value)
        .trim();
    let value = strip_ascii_case_prefix(value, "bearer ")
        .or_else(|| strip_ascii_case_prefix(value, "basic "))
        .unwrap_or(value)
        .trim();

    if let Some(name) = value
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
    {
        return is_env_reference_name(name);
    }

    value.strip_prefix('$').is_some_and(is_env_reference_name)
}

fn is_env_reference_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn strip_ascii_case_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then_some(&value[prefix.len()..])
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum AgentPolicyHookConfigError {
    #[error("agent policy hooks are enabled but no before-action hooks are configured")]
    NoBeforeActionHooks,
    #[error("agent policy hook name must not be empty")]
    MissingHookName,
    #[error("agent policy hook stdio command must not be empty")]
    MissingStdioCommand,
    #[error(
        "agent policy hook stdio command must not include credentials; use args with env secret references"
    )]
    StdioCommandContainsCredentials,
    #[error(
        "agent policy hook stdio args must not include credentials; use env secret references"
    )]
    StdioArgContainsCredentials,
    #[error(
        "agent policy hook timeout must be between 1 and {MAX_AGENT_POLICY_HOOK_TIMEOUT_MS} ms"
    )]
    InvalidTimeoutMs,
    #[error("agent policy hook working directory is invalid: {0:?}")]
    InvalidWorkingDirectory(PathBuf),
    #[error("agent policy hook HTTP URL is invalid: {0}")]
    InvalidHttpUrl(String),
    #[error("agent policy hook HTTP URL must use HTTPS unless it targets localhost: {0}")]
    InsecureHttpUrl(String),
    #[error("agent policy hook HTTP URL must not include embedded credentials")]
    HttpUrlContainsCredentials,
    #[error("agent policy hook HTTP header name is invalid: {0}")]
    InvalidHttpHeaderName(String),
    #[error("agent policy hook secret environment variable name must not be empty")]
    MissingSecretEnvironmentVariableName,
    #[error("agent policy hook secret environment variable reference must be an environment variable name")]
    InvalidSecretEnvironmentVariableName,
}

fn validate_timeout_ms(timeout_ms: u64) -> Result<(), AgentPolicyHookConfigError> {
    if !(1..=MAX_AGENT_POLICY_HOOK_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(AgentPolicyHookConfigError::InvalidTimeoutMs);
    }

    Ok(())
}
