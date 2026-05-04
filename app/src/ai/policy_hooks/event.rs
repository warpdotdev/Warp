use std::path::PathBuf;

use serde::{Deserialize, Serialize, Serializer};

use super::{
    decision::WarpPermissionSnapshot,
    redaction::{
        capped_policy_items, mcp_argument_keys, redact_command_for_policy,
        redact_sensitive_text_for_policy, truncate_for_policy,
    },
};

pub(crate) const AGENT_POLICY_SCHEMA_VERSION: &str = "warp.agent_policy_hook.v1";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct AgentPolicyEvent {
    pub schema_version: String,
    pub event_id: uuid::Uuid,
    pub conversation_id: String,
    pub action_id: String,
    pub action_kind: AgentPolicyActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<PathBuf>,
    pub run_until_completion: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile_id: Option<String>,
    pub warp_permission: WarpPermissionSnapshot,
    pub action: AgentPolicyAction,
}

impl AgentPolicyEvent {
    pub(crate) fn new(
        conversation_id: impl Into<String>,
        action_id: impl Into<String>,
        working_directory: Option<PathBuf>,
        run_until_completion: bool,
        active_profile_id: Option<String>,
        warp_permission: WarpPermissionSnapshot,
        action: AgentPolicyAction,
    ) -> Self {
        let action = action.redacted();
        Self {
            schema_version: AGENT_POLICY_SCHEMA_VERSION.to_string(),
            event_id: uuid::Uuid::new_v4(),
            conversation_id: conversation_id.into(),
            action_id: action_id.into(),
            action_kind: action.kind(),
            working_directory,
            run_until_completion,
            active_profile_id,
            warp_permission,
            action,
        }
    }

    #[cfg(test)]
    pub(crate) fn execute_command(
        conversation_id: impl Into<String>,
        action_id: impl Into<String>,
        working_directory: Option<PathBuf>,
        run_until_completion: bool,
        active_profile_id: Option<String>,
        warp_permission: WarpPermissionSnapshot,
        action: PolicyExecuteCommandAction,
    ) -> Self {
        Self::new(
            conversation_id,
            action_id,
            working_directory,
            run_until_completion,
            active_profile_id,
            warp_permission,
            AgentPolicyAction::ExecuteCommand(action.redacted()),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentPolicyActionKind {
    ExecuteCommand,
    WriteToLongRunningShellCommand,
    ReadFiles,
    WriteFiles,
    CallMcpTool,
    ReadMcpResource,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum AgentPolicyAction {
    ExecuteCommand(PolicyExecuteCommandAction),
    WriteToLongRunningShellCommand(PolicyWriteToLongRunningShellCommandAction),
    ReadFiles(PolicyReadFilesAction),
    WriteFiles(PolicyWriteFilesAction),
    CallMcpTool(PolicyCallMcpToolAction),
    ReadMcpResource(PolicyReadMcpResourceAction),
}

impl AgentPolicyAction {
    pub(crate) fn kind(&self) -> AgentPolicyActionKind {
        match self {
            Self::ExecuteCommand(_) => AgentPolicyActionKind::ExecuteCommand,
            Self::WriteToLongRunningShellCommand(_) => {
                AgentPolicyActionKind::WriteToLongRunningShellCommand
            }
            Self::ReadFiles(_) => AgentPolicyActionKind::ReadFiles,
            Self::WriteFiles(_) => AgentPolicyActionKind::WriteFiles,
            Self::CallMcpTool(_) => AgentPolicyActionKind::CallMcpTool,
            Self::ReadMcpResource(_) => AgentPolicyActionKind::ReadMcpResource,
        }
    }

    fn redacted(self) -> Self {
        match self {
            Self::ExecuteCommand(action) => Self::ExecuteCommand(action.redacted()),
            Self::WriteToLongRunningShellCommand(action) => Self::WriteToLongRunningShellCommand(
                PolicyWriteToLongRunningShellCommandAction::new(
                    action.block_id,
                    action.input.as_bytes(),
                    action.mode,
                ),
            ),
            Self::ReadFiles(action) => Self::ReadFiles(action),
            Self::WriteFiles(action) => Self::WriteFiles(action),
            Self::CallMcpTool(action) => Self::CallMcpTool(PolicyCallMcpToolAction {
                server_id: action.server_id,
                tool_name: redact_sensitive_text_for_policy(&action.tool_name),
                argument_keys: action
                    .argument_keys
                    .into_iter()
                    .map(|key| redact_sensitive_text_for_policy(&key))
                    .collect(),
                omitted_argument_key_count: action.omitted_argument_key_count,
            }),
            Self::ReadMcpResource(action) => Self::ReadMcpResource(
                PolicyReadMcpResourceAction::new(action.server_id, action.name, action.uri),
            ),
        }
    }
}

impl Serialize for AgentPolicyAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::ExecuteCommand(action) => action.serialize(serializer),
            Self::WriteToLongRunningShellCommand(action) => action.serialize(serializer),
            Self::ReadFiles(action) => action.serialize(serializer),
            Self::WriteFiles(action) => action.serialize(serializer),
            Self::CallMcpTool(action) => action.serialize(serializer),
            Self::ReadMcpResource(action) => action.serialize(serializer),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PolicyExecuteCommandAction {
    pub command: String,
    pub normalized_command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_read_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_risky: Option<bool>,
}

impl PolicyExecuteCommandAction {
    pub(crate) fn new(
        command: impl Into<String>,
        normalized_command: impl Into<String>,
        is_read_only: Option<bool>,
        is_risky: Option<bool>,
    ) -> Self {
        Self {
            command: command.into(),
            normalized_command: normalized_command.into(),
            is_read_only,
            is_risky,
        }
    }

    pub(crate) fn redacted(self) -> Self {
        Self {
            command: redact_command_for_policy(&self.command),
            normalized_command: redact_command_for_policy(&self.normalized_command),
            is_read_only: self.is_read_only,
            is_risky: self.is_risky,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PolicyWriteToLongRunningShellCommandAction {
    pub block_id: String,
    pub input: String,
    pub mode: String,
}

impl PolicyWriteToLongRunningShellCommandAction {
    pub(crate) fn new(
        block_id: impl Into<String>,
        input: impl AsRef<[u8]>,
        mode: impl Into<String>,
    ) -> Self {
        let input = String::from_utf8_lossy(input.as_ref());
        let block_id = block_id.into();
        let mode = mode.into();
        Self {
            block_id: truncate_for_policy(&block_id),
            input: redact_command_for_policy(&input),
            mode: truncate_for_policy(&mode),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PolicyReadFilesAction {
    pub paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_path_count: Option<usize>,
}

impl PolicyReadFilesAction {
    pub(crate) fn new(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let (paths, omitted_path_count) =
            capped_policy_items(paths.into_iter().map(truncate_policy_path));
        Self {
            paths,
            omitted_path_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PolicyWriteFilesAction {
    pub paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_path_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_stats: Option<PolicyDiffStats>,
}

impl PolicyWriteFilesAction {
    pub(crate) fn new(
        paths: impl IntoIterator<Item = PathBuf>,
        diff_stats: Option<PolicyDiffStats>,
    ) -> Self {
        let (paths, omitted_path_count) =
            capped_policy_items(paths.into_iter().map(truncate_policy_path));
        Self {
            paths,
            omitted_path_count,
            diff_stats,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PolicyDiffStats {
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PolicyCallMcpToolAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_id: Option<uuid::Uuid>,
    pub tool_name: String,
    pub argument_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_argument_key_count: Option<usize>,
}

impl PolicyCallMcpToolAction {
    pub(crate) fn new(
        server_id: Option<uuid::Uuid>,
        tool_name: impl Into<String>,
        arguments: &serde_json::Value,
    ) -> Self {
        let (argument_keys, omitted_argument_key_count) = mcp_argument_keys(arguments);
        let tool_name = tool_name.into();
        Self {
            server_id,
            tool_name: redact_sensitive_text_for_policy(&tool_name),
            argument_keys,
            omitted_argument_key_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PolicyReadMcpResourceAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_id: Option<uuid::Uuid>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

impl PolicyReadMcpResourceAction {
    pub(crate) fn new(
        server_id: Option<uuid::Uuid>,
        name: impl Into<String>,
        uri: Option<String>,
    ) -> Self {
        let name = name.into();
        Self {
            server_id,
            name: redact_sensitive_text_for_policy(&name),
            uri: uri.map(|uri| redact_sensitive_text_for_policy(&uri)),
        }
    }
}

fn truncate_policy_path(path: PathBuf) -> PathBuf {
    let path_text = path.to_string_lossy();
    let redacted_path = redact_sensitive_text_for_policy(&path_text);
    if redacted_path == path_text && path_text.len() <= super::redaction::MAX_POLICY_STRING_BYTES {
        return path;
    }

    PathBuf::from(redacted_path)
}
