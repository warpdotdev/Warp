use std::fs;
use std::path::Path;

use anyhow::Result;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::claude_transcript::read_jsonl;
use super::*;

/// Walk `sessions_root` for `session_id`'s rollout and assemble an envelope.
fn read_envelope(
    session_id: Uuid,
    sessions_root: &Path,
) -> Result<Option<CodexTranscriptEnvelope>> {
    let Some(path) = find_session_file(sessions_root, session_id) else {
        return Ok(None);
    };
    let entries = read_jsonl(&path)?;
    let meta = parse_session_meta(entries.first()).unwrap_or_default();
    Ok(Some(CodexTranscriptEnvelope::new(
        session_id, meta, entries,
    )))
}

/// Minimal SessionMeta line in the same shape codex writes (codex
/// `protocol/src/protocol.rs::RolloutItem`): `{type, payload}`.
fn session_meta_line(uuid: Uuid, cwd: &str, timestamp: &str, cli_version: &str) -> String {
    serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": uuid.to_string(),
            "timestamp": timestamp,
            "cwd": cwd,
            "originator": "test",
            "cli_version": cli_version,
        },
    })
    .to_string()
}

#[test]
#[serial_test::serial]
fn codex_sessions_root_honors_codex_home_env() {
    let tmp = TempDir::new().unwrap();
    let prev = std::env::var(CODEX_HOME_ENV).ok();
    std::env::set_var(CODEX_HOME_ENV, tmp.path());

    let root = codex_sessions_root().unwrap();

    match prev {
        Some(v) => std::env::set_var(CODEX_HOME_ENV, v),
        None => std::env::remove_var(CODEX_HOME_ENV),
    }
    assert_eq!(root, tmp.path().join(CODEX_SESSIONS_SUBDIR));
}

#[test]
fn find_session_file_walks_yyyy_mm_dd_tree() {
    let tmp = TempDir::new().unwrap();
    let uuid = Uuid::new_v4();
    let day = tmp.path().join("2026").join("04").join("30");
    fs::create_dir_all(&day).unwrap();
    let file = day.join(format!("rollout-ignored-ts-{uuid}.jsonl"));
    fs::write(&file, "").unwrap();

    let found = find_session_file(tmp.path(), uuid);
    assert_eq!(found, Some(file));
}

#[test]
fn find_session_file_returns_none_when_no_match() {
    let tmp = TempDir::new().unwrap();
    let day = tmp.path().join("2026").join("04").join("30");
    fs::create_dir_all(&day).unwrap();
    fs::write(
        day.join(format!("rollout-ignored-ts-{}.jsonl", Uuid::new_v4())),
        "",
    )
    .unwrap();

    assert!(find_session_file(tmp.path(), Uuid::new_v4()).is_none());
}

#[test]
fn find_session_file_returns_none_when_root_missing() {
    let tmp = TempDir::new().unwrap();
    assert!(find_session_file(&tmp.path().join("missing"), Uuid::new_v4()).is_none());
}

#[test]
fn read_envelope_recovers_cwd_and_version_from_session_meta() {
    let tmp = TempDir::new().unwrap();
    let uuid = Uuid::new_v4();
    let day = tmp.path().join("2026").join("04").join("30");
    fs::create_dir_all(&day).unwrap();
    let meta = session_meta_line(uuid, "/work/proj", "2026-04-30T01:54:20.000Z", "0.55.0");
    let body = format!("{meta}\n{{\"type\":\"event_msg\",\"payload\":{{\"x\":1}}}}\n");
    fs::write(day.join(format!("rollout-ignored-ts-{uuid}.jsonl")), body).unwrap();

    let envelope = read_envelope(uuid, tmp.path()).unwrap().unwrap();
    assert_eq!(envelope.session_id, uuid);
    assert_eq!(envelope.cwd, std::path::PathBuf::from("/work/proj"));
    assert_eq!(envelope.codex_version.as_deref(), Some("0.55.0"));
    assert_eq!(envelope.entries.len(), 2);
}

#[test]
fn read_envelope_returns_none_when_missing() {
    let tmp = TempDir::new().unwrap();
    assert!(read_envelope(Uuid::new_v4(), tmp.path()).unwrap().is_none());
}
