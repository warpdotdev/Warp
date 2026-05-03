use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::decision::AgentPolicyUnavailableDecision;

pub(crate) const DEFAULT_AGENT_POLICY_HOOK_TIMEOUT_MS: u64 = 5_000;
pub(crate) const MAX_AGENT_POLICY_HOOK_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        self.enabled && !self.before_action.is_empty()
    }

    pub(crate) fn validate(&self) -> Result<(), AgentPolicyHookConfigError> {
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
        self.allow_hook_autoapproval
            || self
                .before_action
                .iter()
                .all(|hook| hook.allow_autoapproval)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub(crate) enum AgentPolicyHookTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default, skip_serializing)]
        env: BTreeMap<String, AgentPolicyHookSecretValue>,
        #[serde(default)]
        working_directory: Option<PathBuf>,
    },
    Http {
        url: String,
        #[serde(default, skip_serializing)]
        headers: BTreeMap<String, AgentPolicyHookSecretValue>,
    },
}

impl AgentPolicyHookTransport {
    pub(crate) fn validate(&self) -> Result<(), AgentPolicyHookConfigError> {
        match self {
            Self::Stdio {
                command,
                working_directory,
                ..
            } => {
                if command.trim().is_empty() {
                    return Err(AgentPolicyHookConfigError::MissingStdioCommand);
                }

                if working_directory
                    .as_deref()
                    .is_some_and(|path| path.as_os_str().is_empty())
                {
                    return Err(AgentPolicyHookConfigError::InvalidWorkingDirectory(
                        Path::new("").to_path_buf(),
                    ));
                }
            }
            Self::Http { url, .. } => {
                let parsed = url::Url::parse(url)
                    .map_err(|_| AgentPolicyHookConfigError::InvalidHttpUrl(url.clone()))?;

                let host = parsed.host_str().unwrap_or_default();
                let is_localhost = matches!(host, "localhost" | "127.0.0.1" | "::1");
                let is_allowed_local_http = parsed.scheme() == "http" && is_localhost;
                if parsed.scheme() != "https" && !is_allowed_local_http {
                    return Err(AgentPolicyHookConfigError::InsecureHttpUrl(url.clone()));
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct AgentPolicyHookSecretValue(String);

impl AgentPolicyHookSecretValue {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AgentPolicyHookSecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"<redacted>\"")
    }
}

impl From<String> for AgentPolicyHookSecretValue {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for AgentPolicyHookSecretValue {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
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
        "agent policy hook timeout must be between 1 and {MAX_AGENT_POLICY_HOOK_TIMEOUT_MS} ms"
    )]
    InvalidTimeoutMs,
    #[error("agent policy hook working directory is invalid: {0:?}")]
    InvalidWorkingDirectory(PathBuf),
    #[error("agent policy hook HTTP URL is invalid: {0}")]
    InvalidHttpUrl(String),
    #[error("agent policy hook HTTP URL must use HTTPS unless it targets localhost: {0}")]
    InsecureHttpUrl(String),
}

fn validate_timeout_ms(timeout_ms: u64) -> Result<(), AgentPolicyHookConfigError> {
    if !(1..=MAX_AGENT_POLICY_HOOK_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(AgentPolicyHookConfigError::InvalidTimeoutMs);
    }

    Ok(())
}
