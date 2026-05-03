use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{
    decision::AgentPolicyEffectiveDecision,
    event::{AgentPolicyAction, AgentPolicyActionKind, AgentPolicyEvent},
};

#[cfg(not(test))]
const AUDIT_DIR_NAME: &str = "agent_policy_hooks";
#[cfg(not(test))]
const AUDIT_FILE_NAME: &str = "audit.jsonl";

#[derive(Debug, Serialize)]
struct AgentPolicyAuditRecord<'a> {
    schema_version: &'a str,
    timestamp: DateTime<Utc>,
    event_id: uuid::Uuid,
    conversation_id: &'a str,
    action_id: &'a str,
    action_kind: AgentPolicyActionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    working_directory: Option<&'a PathBuf>,
    run_until_completion: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_profile_id: Option<&'a str>,
    action: &'a AgentPolicyAction,
    effective_decision: &'a AgentPolicyEffectiveDecision,
    redaction: AgentPolicyAuditRedaction,
}

#[derive(Debug, Serialize)]
struct AgentPolicyAuditRedaction {
    command_secrets_redacted: bool,
    mcp_argument_values_omitted: bool,
}

pub(crate) fn write_audit_record(
    event: &AgentPolicyEvent,
    decision: &AgentPolicyEffectiveDecision,
) -> Result<()> {
    let Some(path) = audit_log_path() else {
        return Ok(());
    };
    let parent = path
        .parent()
        .context("agent policy audit path has no parent directory")?;

    create_private_directory_all(parent)
        .with_context(|| format!("create agent policy audit directory {}", parent.display()))?;

    let line = audit_record_json_line(event, decision)?;

    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }

    let mut file = options
        .open(&path)
        .with_context(|| format!("open agent policy audit log {}", path.display()))?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("write agent policy audit log {}", path.display()))?;
    file.write_all(b"\n")
        .with_context(|| format!("terminate agent policy audit log {}", path.display()))?;
    set_private_file_permissions(&path);

    Ok(())
}

fn create_private_directory_all(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;

        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700).create(path)?;
        set_private_directory_permissions(path);
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::create_dir_all(path)
    }
}

pub(crate) fn audit_record_json_line(
    event: &AgentPolicyEvent,
    decision: &AgentPolicyEffectiveDecision,
) -> Result<String> {
    let record = AgentPolicyAuditRecord {
        schema_version: event.schema_version.as_str(),
        timestamp: Utc::now(),
        event_id: event.event_id,
        conversation_id: event.conversation_id.as_str(),
        action_id: event.action_id.as_str(),
        action_kind: event.action_kind,
        working_directory: event.working_directory.as_ref(),
        run_until_completion: event.run_until_completion,
        active_profile_id: event.active_profile_id.as_deref(),
        action: &event.action,
        effective_decision: decision,
        redaction: AgentPolicyAuditRedaction {
            command_secrets_redacted: true,
            mcp_argument_values_omitted: true,
        },
    };

    serde_json::to_string(&record).context("serialize agent policy audit record")
}

fn audit_log_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        None
    }

    #[cfg(not(test))]
    {
        Some(
            warp_core::paths::secure_state_dir()
                .unwrap_or_else(warp_core::paths::state_dir)
                .join(AUDIT_DIR_NAME)
                .join(AUDIT_FILE_NAME),
        )
    }
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;

    if let Err(err) = fs::set_permissions(path, fs::Permissions::from_mode(0o700)) {
        log::warn!(
            "Failed to set private permissions on agent policy audit directory {}: {err}",
            path.display()
        );
    }
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) {}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;

    if let Err(err) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
        log::warn!(
            "Failed to set private permissions on agent policy audit log {}: {err}",
            path.display()
        );
    }
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) {}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::create_private_directory_all;

    #[test]
    fn create_private_directory_all_uses_private_permissions() {
        let root = tempfile::tempdir().unwrap();
        let audit_dir = root.path().join("agent_policy_hooks");

        create_private_directory_all(&audit_dir).unwrap();

        let mode = audit_dir.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }
}
