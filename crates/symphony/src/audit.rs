//! Append-only JSONL audit log.
//!
//! Writes are best-effort: I/O failures are logged through `tracing` and
//! never panic, since the audit log is observability, not load-bearing
//! state. The default location is `~/.warp/symphony/audit.log`.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Categorical audit event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    /// Issue picked up by the orchestrator and claimed.
    Claimed,
    /// Agent dispatched against a workspace.
    Dispatched,
    /// Streaming agent emitted a chunk of output.
    Chunk,
    /// Agent invoked a tool.
    ToolCall,
    /// Agent received a tool result.
    ToolResult,
    /// Agent reported task completion.
    Completed,
    /// Agent reported a failure.
    Failed,
    /// Diff guard rejected the run for being too large.
    DiffGuardExceeded,
    /// Workflow tick boundary.
    Tick,
}

/// Single audit-log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// When the event was recorded.
    pub timestamp: DateTime<Utc>,
    /// Linear issue id, if applicable.
    pub issue_id: Option<String>,
    /// Linear identifier (`PDX-12`), if applicable.
    pub issue_identifier: Option<String>,
    /// Event kind.
    pub kind: AuditEventKind,
    /// Optional agent provider tag (e.g. `"claude_code"`, `"codex"`).
    pub agent_provider: Option<String>,
    /// Optional token counter.
    pub tokens_used: Option<u64>,
    /// Optional human-readable error.
    pub error: Option<String>,
    /// Optional free-form message.
    pub message: Option<String>,
}

impl AuditEvent {
    /// Construct a new event with `now()` as timestamp.
    pub fn new(kind: AuditEventKind) -> Self {
        Self {
            timestamp: Utc::now(),
            issue_id: None,
            issue_identifier: None,
            kind,
            agent_provider: None,
            tokens_used: None,
            error: None,
            message: None,
        }
    }

    /// Builder-style setter for `issue_id`.
    pub fn with_issue(mut self, id: impl Into<String>, identifier: impl Into<String>) -> Self {
        self.issue_id = Some(id.into());
        self.issue_identifier = Some(identifier.into());
        self
    }

    /// Builder-style setter for `agent_provider`.
    pub fn with_provider(mut self, p: impl Into<String>) -> Self {
        self.agent_provider = Some(p.into());
        self
    }

    /// Builder-style setter for `error`.
    pub fn with_error(mut self, e: impl Into<String>) -> Self {
        self.error = Some(e.into());
        self
    }

    /// Builder-style setter for `message`.
    pub fn with_message(mut self, m: impl Into<String>) -> Self {
        self.message = Some(m.into());
        self
    }
}

/// Append-only JSONL writer.
pub struct AuditLog {
    path: PathBuf,
    file: Mutex<Option<std::fs::File>>,
}

impl AuditLog {
    /// Open (or create) the log file at `path`. Best effort: if the parent
    /// directory cannot be created, the writer falls back to a no-op state
    /// and emits a `tracing::warn!` on the first attempted write.
    pub fn open(path: PathBuf) -> Self {
        let file = match Self::open_inner(&path) {
            Ok(f) => Some(f),
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "failed to open audit log");
                None
            }
        };
        Self {
            path,
            file: Mutex::new(file),
        }
    }

    fn open_inner(path: &PathBuf) -> std::io::Result<std::fs::File> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    }

    /// Append `event` as a JSON line. Failures are logged, never returned.
    pub fn record(&self, event: AuditEvent) {
        let line = match serde_json::to_string(&event) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize audit event");
                return;
            }
        };
        let mut guard = match self.file.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(error = %e, "audit log mutex poisoned");
                return;
            }
        };
        if guard.is_none() {
            // Try to reopen — the underlying directory may have appeared.
            if let Ok(f) = Self::open_inner(&self.path) {
                *guard = Some(f);
            }
        }
        if let Some(f) = guard.as_mut() {
            if let Err(e) = writeln!(f, "{}", line) {
                tracing::warn!(error = %e, "failed to write audit event");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_event() {
        let ev = AuditEvent::new(AuditEventKind::Dispatched)
            .with_issue("abc", "PDX-1")
            .with_provider("claude_code")
            .with_message("ok");
        let s = serde_json::to_string(&ev).unwrap();
        let back: AuditEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back.kind, AuditEventKind::Dispatched);
        assert_eq!(back.issue_id.as_deref(), Some("abc"));
        assert_eq!(back.issue_identifier.as_deref(), Some("PDX-1"));
        assert_eq!(back.agent_provider.as_deref(), Some("claude_code"));
        assert_eq!(back.message.as_deref(), Some("ok"));
    }
}
