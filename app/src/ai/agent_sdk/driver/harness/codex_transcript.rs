//! Codex session transcript envelope + read helpers.
//!
//! Owns:
//! - [`CodexTranscriptEnvelope`] — the on-wire/on-GCS shape of a saved Codex rollout
//!   (parsed JSONL entries plus session-level metadata). Reader functions interoperate
//!   with Codex's own `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<uuid>.jsonl` layout
//!   (codex `rollout/src/recorder.rs`).
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Env var codex honors to override `~/.codex` (see codex `core/src/config/mod.rs`).
const CODEX_HOME_ENV: &str = "CODEX_HOME";
const CODEX_HOME_DIRNAME: &str = ".codex";
/// Subdirectory under `$CODEX_HOME` where rollouts live.
const CODEX_SESSIONS_SUBDIR: &str = "sessions";

/// JSON envelope sent to the server representing a complete Codex session.
///
/// The transcript is the parsed JSONL content of the rollout file; codex's resume
/// path re-reads this JSONL line by line.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct CodexTranscriptEnvelope {
    /// The directory the codex session started in (recovered from the `SessionMeta` line).
    pub(crate) cwd: PathBuf,
    /// Codex session/thread UUID. Matches the trailing `-<uuid>` in the rollout filename.
    pub(crate) session_id: Uuid,
    /// `cli_version` from `SessionMeta`, surfaced separately for the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) codex_version: Option<String>,
    /// Parsed JSONL entries.
    pub(crate) entries: Vec<Value>,
}

impl CodexTranscriptEnvelope {
    pub(crate) fn new(session_id: Uuid, meta: CodexSessionMetadata, entries: Vec<Value>) -> Self {
        Self {
            cwd: meta.cwd,
            session_id,
            codex_version: meta.codex_version,
            entries,
        }
    }
}

/// Session-level metadata pulled from the rollout's `SessionMeta` line.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CodexSessionMetadata {
    pub(crate) cwd: PathBuf,
    pub(crate) codex_version: Option<String>,
}

/// Resolve the codex sessions root, honoring `$CODEX_HOME` then falling back to `~/.codex`.
pub(crate) fn codex_sessions_root() -> anyhow::Result<PathBuf> {
    let home = if let Ok(dir) = std::env::var(CODEX_HOME_ENV) {
        PathBuf::from(dir)
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?
            .join(CODEX_HOME_DIRNAME)
    };
    Ok(home.join(CODEX_SESSIONS_SUBDIR))
}

/// Walk `<sessions_root>/YYYY/MM/DD/` looking for a `rollout-*-<session_id>.jsonl`.
///
/// Returns `None` if `sessions_root` doesn't exist yet or no matching file is found.
pub(crate) fn find_session_file(sessions_root: &Path, session_id: Uuid) -> Option<PathBuf> {
    if !sessions_root.exists() {
        return None;
    }
    let suffix = format!("-{session_id}.jsonl");
    for year_dir in read_subdirs(sessions_root) {
        for month_dir in read_subdirs(&year_dir) {
            for day_dir in read_subdirs(&month_dir) {
                let entries = match fs::read_dir(&day_dir) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    if name.starts_with("rollout-") && name.ends_with(&suffix) {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn read_subdirs(parent: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry.file_type().ok()?.is_dir().then(|| entry.path())
        })
}

/// Pull `cwd` and `cli_version` out of the first JSONL line if it's a `SessionMeta`.
pub(crate) fn parse_session_meta(first: Option<&Value>) -> Option<CodexSessionMetadata> {
    let entry = first?;
    if entry.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
        return None;
    }
    let payload = entry.get("payload")?;
    let cwd = PathBuf::from(payload.get("cwd").and_then(|v| v.as_str())?);
    let codex_version = payload
        .get("cli_version")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    Some(CodexSessionMetadata { cwd, codex_version })
}

#[cfg(test)]
#[path = "codex_transcript_tests.rs"]
mod tests;
