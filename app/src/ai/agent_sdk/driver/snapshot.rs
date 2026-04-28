//! End-of-run snapshot upload pipeline invoked from `AgentDriver::run_snapshot_upload`.
//!
//! Reads a JSONL declarations file listing repos and files, gathers git-diff patches or file
//! contents for each, and uploads them (plus a `snapshot_state.json` manifest) to presigned GCS
//! URLs. Transient upload failures retry through the shared [`with_bounded_retry`] helper.
//!
//! All failures are logged and absorbed so the driver continues regardless.
//!
//! # Declarations file format (v1)
//!
//! The declarations file is an append-only UTF-8 JSONL file. The Rust pipeline only ever
//! *reads* it; the sibling bash generator `snapshot-declarations.sh` (shipped in
//! `warp-agent-docker`) is the primary writer, and operators may hand-edit entries.
//!
//! Each non-empty line is a JSON object with:
//! - `version`: `1`,
//! - `kind`: `repo` or `file`,
//! - `path`: an absolute path.
//!
//! Lines are trimmed before parsing, so a stray trailing `\r` or leading/trailing whitespace
//! around the JSON object is tolerated. Malformed lines (invalid JSON, missing fields, unknown
//! `version`, unknown `kind`, non-absolute path) are logged at WARN and skipped; they never abort
//! parsing. Non-UTF-8 content short-circuits the read with a WARN because
//! [`std::fs::read_to_string`] fails.
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use command::r#async::Command;
use command::Stdio;
use futures::future::join_all;
use warp_core::report_error;
use warpui::r#async::FutureExt as _;

use crate::ai::agent_sdk::retry::with_bounded_retry;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::server::server_api::harness_support::{
    upload_to_target, HarnessSupportClient, SnapshotFileInfo, SnapshotUploadRequest, UploadTarget,
};

/// Default path of the declarations file when neither the env var override nor a task ID
/// is available. Per-run files use `{DEFAULT_DECLARATIONS_DIR}/<id>/{DEFAULT_DECLARATIONS_FILENAME}`.
const DEFAULT_DECLARATIONS_DIR: &str = "/tmp/oz";
const DEFAULT_DECLARATIONS_FILENAME: &str = "snapshot-declarations.jsonl";
const DECLARATION_VERSION: u32 = 1;

/// Env var override for the declarations file path (useful for tests and operators).
const DECLARATIONS_PATH_ENV_VAR: &str = "OZ_SNAPSHOT_DECLARATIONS_FILE";

/// Env var pointing directly at the declarations-generator script.
/// Set by `entrypoint.sh` in containerized runs and by `oz-local --docker-dir` in local dev.
const DECLARATIONS_SCRIPT_PATH_ENV_VAR: &str = "OZ_SNAPSHOT_DECLARATIONS_SCRIPT";

/// Upper bound on declarations-script runtime. If the script takes longer we log an error and
/// move on; the upload step then reads whatever the file already contains (possibly nothing).
pub(super) const DEFAULT_DECLARATIONS_SCRIPT_TIMEOUT: Duration = Duration::from_secs(60);

/// Upper bound on the end-of-run upload pipeline's total runtime, enforced at the call site in
/// `AgentDriver::run_snapshot_upload`. Cleanup continues regardless of the outcome.
pub(super) const DEFAULT_SNAPSHOT_UPLOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// Upper bound for each git subprocess spawned during the gather phase.
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Max files per `POST /harness-support/upload-snapshot` call.
/// Must match the server-side `binding:"required,min=1,max=25"` on
/// `UploadSnapshotRequest.Files` in `router/handlers/public_api/harness_support.go`.
const UPLOAD_BATCH_SIZE: usize = 25;

/// Total cap on files (blobs + manifest) uploaded per run.
/// The server is stateless across `upload-snapshot` calls and assigns a fresh GCS UUID per
/// filename, so we chunk into requests of [`UPLOAD_BATCH_SIZE`] and enforce the per-run total
/// here. Blobs beyond the cap are dropped from upload and marked `skipped` in the manifest so
/// consumers can distinguish capped entries from real upload failures.
const MAX_SNAPSHOT_FILES_PER_RUN: usize = 100;

// --- Declarations file parsing ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EntryKind {
    Repo,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclarationEntry {
    kind: EntryKind,
    path: String,
}

#[derive(serde::Deserialize)]
struct DeclarationLine {
    version: Option<u32>,
    kind: String,
    path: String,
}

/// Invoke `snapshot-declarations.sh` to (re)generate the declarations file consumed by the
/// rest of the upload pipeline.
///
/// The script path resolves from `$OZ_SNAPSHOT_DECLARATIONS_SCRIPT`, the scan root defaults to
/// `working_dir` (the agent's workspace), and writes to the per-run declarations file resolved
/// from `task_id`. The script appends to the file if it already exists, so
/// repeated invocations within a single run accumulate repos instead of clobbering.
///
/// A missing env var, a missing script, a non-zero exit status, a spawn failure, or a runtime
/// exceeding `script_timeout` are each logged at `log::error!` and returned without aborting the
/// caller — if a previous invocation already produced a declarations file on disk it remains
/// usable; otherwise the upload pipeline becomes a no-op.
///
/// Exposed as a standalone helper so future call sites can trigger declarations generation at
/// other points in the run lifecycle (e.g. periodic mid-run snapshots).
pub(super) async fn run_declarations_script(
    working_dir: &Path,
    task_id: &AmbientAgentTaskId,
    script_timeout: Duration,
) {
    let Some(script_path) = std::env::var_os(DECLARATIONS_SCRIPT_PATH_ENV_VAR) else {
        log::error!(
            "{DECLARATIONS_SCRIPT_PATH_ENV_VAR} is not set; skipping snapshot declarations script (task {task_id})"
        );
        return;
    };
    let script_path = PathBuf::from(script_path);
    if !script_path.exists() {
        log::error!(
            "Snapshot declarations script not found at '{}'; skipping (task {task_id})",
            script_path.display()
        );
        return;
    }

    // Anchor the scan to the agent's workspace and the per-run declarations file.
    //
    // Setting `current_dir` ensures `$PWD` in the bash script is the workspace even when the
    // driver process's own CWD has drifted (e.g. the macOS startup path does `cd $HOME`).
    // Setting `OZ_SNAPSHOT_DECLARATIONS_FILE` keeps the script and the upload pipeline in sync
    // on which file to read/write.
    let declarations_path = resolve_declarations_path(Some(task_id));
    log::info!(
        "Running snapshot declarations script {} with cwd={} output={} (task {task_id})",
        script_path.display(),
        working_dir.display(),
        declarations_path.display(),
    );
    let mut command = Command::new(&script_path);
    command
        .current_dir(working_dir)
        .env(DECLARATIONS_PATH_ENV_VAR, &declarations_path);

    let output = match command.output().with_timeout(script_timeout).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            log::error!(
                "Failed to spawn snapshot declarations script '{}': {e:#} (task {task_id})",
                script_path.display()
            );
            return;
        }
        Err(_) => {
            log::error!(
                "Snapshot declarations script '{}' timed out after {:?} (task {task_id})",
                script_path.display(),
                script_timeout
            );
            return;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!(
            "Snapshot declarations script '{}' exited with {}: {stderr} (task {task_id})",
            script_path.display(),
            output.status
        );
    }
}

/// Resolve the declarations file path from the process env and optional task ID.
///
/// Reads `$OZ_SNAPSHOT_DECLARATIONS_FILE` for the operator/test override, then delegates to
/// [`resolve_declarations_path_with_override`] so tests can exercise the pure logic without
/// racing on the shared env var.
fn resolve_declarations_path(task_id: Option<&AmbientAgentTaskId>) -> PathBuf {
    resolve_declarations_path_with_override(task_id, std::env::var_os(DECLARATIONS_PATH_ENV_VAR))
}

/// Pure resolver: returns the declarations file path given an explicit override.
///
/// Precedence:
/// 1. `override_path` (from `$OZ_SNAPSHOT_DECLARATIONS_FILE` in production).
/// 2. `{DEFAULT_DECLARATIONS_DIR}/<task-id>/{DEFAULT_DECLARATIONS_FILENAME}` when a task ID
///    is provided, so concurrent runs don't clobber each other's declarations.
/// 3. `{DEFAULT_DECLARATIONS_DIR}/{DEFAULT_DECLARATIONS_FILENAME}` as a final fallback.
fn resolve_declarations_path_with_override(
    task_id: Option<&AmbientAgentTaskId>,
    override_path: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(override_path) = override_path {
        return PathBuf::from(override_path);
    }
    match task_id {
        Some(id) => PathBuf::from(DEFAULT_DECLARATIONS_DIR)
            .join(id.to_string())
            .join(DEFAULT_DECLARATIONS_FILENAME),
        None => PathBuf::from(DEFAULT_DECLARATIONS_DIR).join(DEFAULT_DECLARATIONS_FILENAME),
    }
}

/// Read and parse the declarations file.
///
/// Returns `None` when the file is missing, unreadable, or yields no valid entries; logs a
/// WARN describing why in each case. A returned `Some(entries)` is guaranteed non-empty.
fn read_and_parse_declarations(
    path: &Path,
    task_id: &AmbientAgentTaskId,
) -> Option<Vec<DeclarationEntry>> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!(
                "Snapshot declarations file not found at '{}'; skipping upload (task {task_id})",
                path.display()
            );
            return None;
        }
        Err(e) => {
            log::warn!(
                "Failed to read snapshot declarations file '{}': {e:#}; skipping upload (task {task_id})",
                path.display()
            );
            return None;
        }
    };
    let entries = parse_declarations(&contents, task_id);
    if entries.is_empty() {
        log::warn!(
            "Snapshot declarations file '{}' has no valid entries; skipping upload (task {task_id})",
            path.display()
        );
        return None;
    }
    Some(entries)
}

/// Parse JSONL declarations text, one entry per non-empty line.
///
/// Valid lines are JSON objects containing `version` (currently `1`), `kind` (`repo` or `file`),
/// and `path` (absolute path). Blank lines are ignored. Malformed lines (invalid JSON, missing
/// fields, unsupported versions, unknown kind, non-absolute path) are logged at WARN and skipped;
/// they never abort parsing.
fn parse_declarations(contents: &str, task_id: &AmbientAgentTaskId) -> Vec<DeclarationEntry> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for (index, raw) in contents.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let declaration: DeclarationLine = match serde_json::from_str(line) {
            Ok(declaration) => declaration,
            Err(e) => {
                log::warn!(
                    "Malformed snapshot declarations JSONL line {line_number}: {e:#}: {raw:?} (task {task_id})"
                );
                continue;
            }
        };
        if declaration.version != Some(DECLARATION_VERSION) {
            log::warn!(
                "Malformed snapshot declarations line {line_number} (missing or unsupported version): {raw:?} (task {task_id})"
            );
            continue;
        }
        if declaration.path.is_empty() {
            log::warn!(
                "Malformed snapshot declarations line {line_number} (missing path): {raw:?} (task {task_id})"
            );
            continue;
        }
        if !Path::new(&declaration.path).is_absolute() {
            log::warn!(
                "Malformed snapshot declarations line {line_number} (non-absolute path): {raw:?} (task {task_id})"
            );
            continue;
        }
        let kind = match declaration.kind.as_str() {
            "repo" => EntryKind::Repo,
            "file" => EntryKind::File,
            other => {
                log::warn!(
                    "Malformed snapshot declarations line {line_number} (unknown kind '{other}'): {raw:?} (task {task_id})"
                );
                continue;
            }
        };
        if !seen.insert((kind, declaration.path.clone())) {
            continue;
        }
        entries.push(DeclarationEntry {
            kind,
            path: declaration.path,
        });
    }
    entries
}

// --- Gather phase: upload blobs and per-entry results ---

struct SnapshotUploadFile {
    filename: String,
    content: Vec<u8>,
    mime_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryStatus {
    Uploaded,
    Failed,
    Skipped,
    GatherFailed,
    ReadFailed,
}

impl EntryStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Uploaded => "uploaded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::GatherFailed => "gather_failed",
            Self::ReadFailed => "read_failed",
        }
    }
}

#[derive(Debug)]
struct EntryResult {
    /// Label for log output — prefers the snapshot filename and falls back to the source path.
    label: String,
    status: EntryStatus,
    error: Option<String>,
}

struct SnapshotSummary {
    uploaded: usize,
    failed: usize,
    skipped: usize,
    gather_failed: usize,
    read_failed: usize,
    total: usize,
    manifest_uploaded: bool,
}

impl SnapshotSummary {
    fn from_entries(entries: &[EntryResult], manifest_uploaded: bool) -> Self {
        let mut s = Self {
            uploaded: 0,
            failed: 0,
            skipped: 0,
            gather_failed: 0,
            read_failed: 0,
            total: entries.len(),
            manifest_uploaded,
        };
        for e in entries {
            match e.status {
                EntryStatus::Uploaded => s.uploaded += 1,
                EntryStatus::Failed => s.failed += 1,
                EntryStatus::Skipped => s.skipped += 1,
                EntryStatus::GatherFailed => s.gather_failed += 1,
                EntryStatus::ReadFailed => s.read_failed += 1,
            }
        }
        s
    }

    fn all_uploaded(&self) -> bool {
        self.manifest_uploaded && self.uploaded == self.total
    }
}

#[derive(Debug)]
struct SnapshotOutcome {
    entries: Vec<EntryResult>,
    manifest_uploaded: bool,
}

// --- Manifest schema ---

#[derive(serde::Serialize)]
struct RepoManifestEntry {
    path: String,
    repo_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_sha: Option<String>,
    patch_file: Option<String>,
    status: &'static str,
    uploaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(serde::Serialize)]
struct FileManifestEntry {
    path: String,
    snapshot_file: Option<String>,
    status: &'static str,
    uploaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(serde::Serialize)]
struct SnapshotManifest {
    version: u32,
    repos: Vec<RepoManifestEntry>,
    files: Vec<FileManifestEntry>,
}

// --- Upload helpers ---

/// Upload `body` to `target` through the shared retry helper, re-cloning `body` per attempt.
async fn upload_with_retry(
    http: &http_client::Client,
    target: &UploadTarget,
    body: Vec<u8>,
    operation: &str,
) -> Result<()> {
    with_bounded_retry(operation, || upload_to_target(http, target, body.clone())).await
}

// --- Entry point ---

/// Run the end-of-run snapshot upload pipeline. All outcomes are logged; this function never
/// returns a value because production callers only care about completion.
pub(super) async fn upload_snapshot_from_declarations(
    client: Arc<dyn HarnessSupportClient>,
    task_id: &AmbientAgentTaskId,
) {
    let declarations_path = resolve_declarations_path(Some(task_id));
    let _ = upload_snapshot_from_declarations_file(&declarations_path, client, task_id).await;
}

/// Internal entry that reads from an explicit path and returns the structured outcome so tests
/// can inspect per-entry statuses. Production callers go through
/// [`upload_snapshot_from_declarations`] which discards the outcome.
async fn upload_snapshot_from_declarations_file(
    path: &Path,
    client: Arc<dyn HarnessSupportClient>,
    task_id: &AmbientAgentTaskId,
) -> Option<SnapshotOutcome> {
    log::info!(
        "Snapshot upload starting from {} (task {task_id})",
        path.display()
    );
    let declarations = read_and_parse_declarations(path, task_id)?;
    let (repo_count, file_count) = declarations
        .iter()
        .fold((0usize, 0usize), |(r, f), e| match e.kind {
            EntryKind::Repo => (r + 1, f),
            EntryKind::File => (r, f + 1),
        });
    log::info!(
        "Snapshot declarations: {} entries ({repo_count} repo, {file_count} file) (task {task_id})",
        declarations.len(),
    );
    let outcome = run_pipeline(declarations, client, task_id).await?;
    log_snapshot_outcome(&outcome, task_id);
    Some(outcome)
}

/// Core upload pipeline.
///
/// Gather/read/upload failures are captured in [`SnapshotOutcome::entries`] and never abort
/// the pipeline. A failure to allocate presigned upload targets is the only case where the
/// pipeline gives up and returns `None`.
async fn run_pipeline(
    declarations: Vec<DeclarationEntry>,
    client: Arc<dyn HarnessSupportClient>,
    task_id: &AmbientAgentTaskId,
) -> Option<SnapshotOutcome> {
    let GatheredSnapshot {
        manifest_filename,
        upload_files,
        repos,
        files,
        pre_upload_entries,
    } = gather_snapshot_entries(declarations, task_id).await;

    upload_gathered_snapshot(
        client,
        task_id,
        manifest_filename,
        upload_files,
        repos,
        files,
        pre_upload_entries,
    )
    .await
}

struct GatheredSnapshot {
    manifest_filename: String,
    upload_files: Vec<SnapshotUploadFile>,
    repos: Vec<RepoManifestEntry>,
    files: Vec<FileManifestEntry>,
    pre_upload_entries: Vec<EntryResult>,
}

async fn gather_snapshot_entries(
    declarations: Vec<DeclarationEntry>,
    task_id: &AmbientAgentTaskId,
) -> GatheredSnapshot {
    let mut used_filenames = HashSet::new();
    let manifest_filename = unique_filename("snapshot_state.json", &mut used_filenames);

    // Gather phase: produce upload blobs and per-entry manifest stubs.
    // Gather/read failures are captured as EntryResult entries and surfaced in the log output.
    let mut upload_files: Vec<SnapshotUploadFile> = Vec::new();
    let mut repos: Vec<RepoManifestEntry> = Vec::new();
    let mut files: Vec<FileManifestEntry> = Vec::new();
    let mut pre_upload_entries: Vec<EntryResult> = Vec::new();

    let mut repo_index: usize = 0;
    for entry in &declarations {
        match entry.kind {
            EntryKind::Repo => {
                repo_index += 1;
                gather_repo(
                    &entry.path,
                    repo_index,
                    &mut used_filenames,
                    &mut upload_files,
                    &mut repos,
                    &mut pre_upload_entries,
                    task_id,
                )
                .await;
            }
            EntryKind::File => {
                gather_file(
                    &entry.path,
                    &mut used_filenames,
                    &mut upload_files,
                    &mut files,
                    &mut pre_upload_entries,
                    task_id,
                )
                .await;
            }
        }
    }

    GatheredSnapshot {
        manifest_filename,
        upload_files,
        repos,
        files,
        pre_upload_entries,
    }
}

async fn upload_gathered_snapshot(
    client: Arc<dyn HarnessSupportClient>,
    task_id: &AmbientAgentTaskId,
    manifest_filename: String,
    mut upload_files: Vec<SnapshotUploadFile>,
    mut repos: Vec<RepoManifestEntry>,
    mut files: Vec<FileManifestEntry>,
    mut pre_upload_entries: Vec<EntryResult>,
) -> Option<SnapshotOutcome> {
    // Enforce the per-run total cap before allocating presigned URLs.
    // The manifest always takes one slot; blobs share the remaining budget. Anything beyond
    // is dropped from the upload plan and marked `skipped` in the manifest so consumers can
    // distinguish capped entries from real upload failures.
    apply_per_run_cap(
        &mut upload_files,
        &mut repos,
        &mut files,
        &mut pre_upload_entries,
        task_id,
    );

    // Ask the server for presigned URLs for every filename we intend to upload —
    // blobs plus the manifest.
    // Chunked into requests of at most [`UPLOAD_BATCH_SIZE`] to stay under the server's
    // per-request binding cap. The server is stateless across calls and assigns a fresh GCS
    // UUID per filename, so chunks compose into one effective allocation.
    let mut file_infos: Vec<SnapshotFileInfo> = upload_files
        .iter()
        .map(|f| SnapshotFileInfo {
            filename: f.filename.clone(),
            mime_type: f.mime_type.clone(),
        })
        .collect();
    file_infos.push(SnapshotFileInfo {
        filename: manifest_filename.clone(),
        mime_type: "application/json".to_string(),
    });

    let mut target_map: HashMap<String, UploadTarget> = HashMap::new();
    for chunk in file_infos.chunks(UPLOAD_BATCH_SIZE) {
        let targets = match client
            .get_snapshot_upload_targets(&SnapshotUploadRequest {
                files: chunk.to_vec(),
            })
            .await
        {
            Ok(t) => t,
            Err(e) => {
                // Pipeline-abort: route through report_error! so Sentry captures the structured
                // error chain and on-call alerting can fire.
                report_error!(e.context(format!(
                    "Failed to get snapshot upload targets; skipping upload (task {task_id})"
                )));
                return None;
            }
        };
        if targets.len() != chunk.len() {
            log::warn!(
                "Snapshot upload-target response length {} does not match request length {}; \
                 extras will be marked skipped (task {task_id})",
                targets.len(),
                chunk.len(),
            );
        }
        for (file, target) in chunk.iter().zip(targets.into_iter()) {
            target_map.insert(file.filename.clone(), target);
        }
    }

    // Upload non-manifest blobs concurrently, each with bounded retries on transient errors.
    let http = client.http_client();
    let upload_futures = upload_files
        .iter()
        .map(|file| upload_entry(http, file, &target_map, task_id));
    let upload_entries: Vec<EntryResult> = join_all(upload_futures).await;
    fold_upload_results(&mut repos, &mut files, &upload_entries, task_id);

    // Build and upload the manifest last, with the real outcomes baked in.
    let manifest = SnapshotManifest {
        version: 1,
        repos,
        files,
    };
    let manifest_bytes = match serde_json::to_vec_pretty(&manifest) {
        Ok(b) => b,
        Err(e) => {
            // Pipeline-abort: route through report_error! so Sentry captures it.
            report_error!(anyhow::Error::from(e).context(format!(
                "Failed to serialize snapshot manifest; skipping upload (task {task_id})"
            )));
            return None;
        }
    };
    let (manifest_uploaded, manifest_error) = match target_map.get(&manifest_filename) {
        Some(target) => {
            let upload_target = merge_content_type(target, "application/json");
            let operation = format!("snapshot upload '{manifest_filename}'");
            match upload_with_retry(http, &upload_target, manifest_bytes, &operation).await {
                Ok(()) => (true, None),
                Err(e) => {
                    // Capture the full chain for the manifest's `error` field, then surface it
                    // to Sentry via report_error!.
                    let e = e.context(format!(
                        "Failed to upload manifest '{manifest_filename}' (task {task_id})"
                    ));
                    let msg = format!("{e:#}");
                    report_error!(e);
                    (false, Some(msg))
                }
            }
        }
        None => (
            false,
            Some(String::from("no upload target returned by server")),
        ),
    };

    // Assemble the final entries list in a stable order: pre-upload failures, upload results,
    // then the manifest itself.
    let mut entries = pre_upload_entries;
    entries.extend(upload_entries);
    entries.push(EntryResult {
        label: manifest_filename,
        status: if manifest_uploaded {
            EntryStatus::Uploaded
        } else {
            EntryStatus::Failed
        },
        error: manifest_error,
    });

    Some(SnapshotOutcome {
        entries,
        manifest_uploaded,
    })
}

/// Gather a repo entry: run `build_repo_patch` and append an upload blob + manifest stub.
async fn gather_repo(
    repo_path: &str,
    repo_index: usize,
    used_filenames: &mut HashSet<String>,
    upload_files: &mut Vec<SnapshotUploadFile>,
    repos: &mut Vec<RepoManifestEntry>,
    pre_upload_entries: &mut Vec<EntryResult>,
    task_id: &AmbientAgentTaskId,
) {
    let repo = Path::new(repo_path);
    let metadata = repo_metadata(repo).await;
    match build_repo_patch(repo).await {
        Ok(patch) if patch.is_empty() => {
            repos.push(RepoManifestEntry {
                path: repo_path.to_string(),
                repo_name: metadata.repo_name,
                branch: metadata.branch,
                head_sha: metadata.head_sha,
                patch_file: None,
                status: "clean",
                uploaded: None,
                error: None,
            });
        }
        Ok(patch) => {
            let preferred = format!(
                "{}_{}.patch",
                repo_index,
                sanitize_filename_component(&metadata.repo_name)
            );
            let filename = unique_filename(&preferred, used_filenames);
            upload_files.push(SnapshotUploadFile {
                filename: filename.clone(),
                content: patch,
                mime_type: "text/x-diff".to_string(),
            });
            repos.push(RepoManifestEntry {
                path: repo_path.to_string(),
                repo_name: metadata.repo_name,
                branch: metadata.branch,
                head_sha: metadata.head_sha,
                patch_file: Some(filename),
                status: "dirty",
                uploaded: None,
                error: None,
            });
        }
        Err(e) => {
            let err_str = format!("{e:#}");
            log::warn!("Failed to snapshot repo '{repo_path}': {err_str} (task {task_id})");
            repos.push(RepoManifestEntry {
                path: repo_path.to_string(),
                repo_name: metadata.repo_name,
                branch: metadata.branch,
                head_sha: metadata.head_sha,
                patch_file: None,
                status: "gather_failed",
                uploaded: None,
                error: Some(err_str.clone()),
            });
            pre_upload_entries.push(EntryResult {
                label: format!("[repo] {repo_path}"),
                status: EntryStatus::GatherFailed,
                error: Some(err_str),
            });
        }
    }
}

/// Gather a file entry: read the file and append an upload blob + manifest stub.
async fn gather_file(
    file_path: &str,
    used_filenames: &mut HashSet<String>,
    upload_files: &mut Vec<SnapshotUploadFile>,
    files: &mut Vec<FileManifestEntry>,
    pre_upload_entries: &mut Vec<EntryResult>,
    task_id: &AmbientAgentTaskId,
) {
    let path = Path::new(file_path);
    match tokio::fs::read(path).await {
        Ok(content) => {
            let preferred = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file_path.to_string());
            let filename = unique_filename(&preferred, used_filenames);
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            upload_files.push(SnapshotUploadFile {
                filename: filename.clone(),
                content,
                mime_type: mime,
            });
            files.push(FileManifestEntry {
                path: file_path.to_string(),
                snapshot_file: Some(filename),
                // Placeholder; rewritten by fold_upload_results once uploads settle.
                status: "uploaded",
                uploaded: None,
                error: None,
            });
        }
        Err(e) => {
            let err_str = format!("Failed to read file '{file_path}': {e:#}");
            log::warn!("{err_str} (task {task_id})");
            files.push(FileManifestEntry {
                path: file_path.to_string(),
                snapshot_file: None,
                status: "read_failed",
                uploaded: None,
                error: Some(err_str.clone()),
            });
            pre_upload_entries.push(EntryResult {
                label: format!("[file] {file_path}"),
                status: EntryStatus::ReadFailed,
                error: Some(err_str),
            });
        }
    }
}

/// Upload a single prepared file through the retry helper.
/// Produces an [`EntryResult`] labelled with the file's filename, or marked `skipped` if the
/// server did not return a target for it.
async fn upload_entry(
    http: &http_client::Client,
    file: &SnapshotUploadFile,
    target_map: &HashMap<String, UploadTarget>,
    task_id: &AmbientAgentTaskId,
) -> EntryResult {
    let Some(target) = target_map.get(&file.filename) else {
        log::warn!(
            "No upload target for file '{}', skipping (task {task_id})",
            file.filename
        );
        return EntryResult {
            label: file.filename.clone(),
            status: EntryStatus::Skipped,
            error: Some("no upload target returned by server".to_string()),
        };
    };

    let upload_target = merge_content_type(target, &file.mime_type);
    let operation = format!("snapshot upload '{}'", file.filename);
    match upload_with_retry(http, &upload_target, file.content.clone(), &operation).await {
        Ok(()) => EntryResult {
            label: file.filename.clone(),
            status: EntryStatus::Uploaded,
            error: None,
        },
        Err(e) => {
            let msg = format!("{e:#}");
            log::warn!(
                "Failed to upload '{}': {msg} (task {task_id})",
                file.filename
            );
            EntryResult {
                label: file.filename.clone(),
                status: EntryStatus::Failed,
                error: Some(msg),
            }
        }
    }
}

/// Fold upload outcomes into the per-entry manifest stubs so the uploaded manifest reflects
/// what actually landed in GCS.
fn fold_upload_results(
    repos: &mut [RepoManifestEntry],
    files: &mut [FileManifestEntry],
    upload_entries: &[EntryResult],
    task_id: &AmbientAgentTaskId,
) {
    for entry in upload_entries {
        if let Some(repo_entry) = repos
            .iter_mut()
            .find(|r| r.patch_file.as_deref() == Some(entry.label.as_str()))
        {
            match entry.status {
                EntryStatus::Uploaded => {
                    repo_entry.uploaded = Some(true);
                    repo_entry.status = "uploaded";
                }
                EntryStatus::Failed => {
                    repo_entry.uploaded = Some(false);
                    repo_entry.status = "failed";
                    repo_entry.error = entry.error.clone();
                }
                EntryStatus::Skipped => {
                    repo_entry.uploaded = Some(false);
                    repo_entry.status = "skipped";
                    repo_entry.error = entry.error.clone();
                }
                EntryStatus::GatherFailed | EntryStatus::ReadFailed => {
                    log::error!(
                        "fold_upload_results: unexpected pre-upload status {:?} for repo patch '{}' (task {task_id})",
                        entry.status,
                        entry.label
                    );
                }
            }
        } else if let Some(file_entry) = files
            .iter_mut()
            .find(|f| f.snapshot_file.as_deref() == Some(entry.label.as_str()))
        {
            match entry.status {
                EntryStatus::Uploaded => {
                    file_entry.uploaded = Some(true);
                    file_entry.status = "uploaded";
                }
                EntryStatus::Failed => {
                    file_entry.uploaded = Some(false);
                    file_entry.status = "failed";
                    file_entry.error = entry.error.clone();
                }
                EntryStatus::Skipped => {
                    file_entry.uploaded = Some(false);
                    file_entry.status = "skipped";
                    file_entry.error = entry.error.clone();
                }
                EntryStatus::GatherFailed | EntryStatus::ReadFailed => {
                    log::error!(
                        "fold_upload_results: unexpected pre-upload status {:?} for file '{}' (task {task_id})",
                        entry.status,
                        entry.label
                    );
                }
            }
        }
    }
}

/// Enforce [`MAX_SNAPSHOT_FILES_PER_RUN`] by truncating the upload blob list.
/// Reserves one slot for the `snapshot_state.json` manifest, so blobs share the remaining
/// budget. For each dropped blob, rewrites the matching manifest entry to `skipped` with a
/// cap-reason error and records a pre-upload [`EntryResult`] so the summary count is honest.
fn apply_per_run_cap(
    upload_files: &mut Vec<SnapshotUploadFile>,
    repos: &mut [RepoManifestEntry],
    files: &mut [FileManifestEntry],
    pre_upload_entries: &mut Vec<EntryResult>,
    task_id: &AmbientAgentTaskId,
) {
    let blob_limit = MAX_SNAPSHOT_FILES_PER_RUN.saturating_sub(1);
    if upload_files.len() <= blob_limit {
        return;
    }
    let total_including_manifest = upload_files.len() + 1;
    let dropped = upload_files.split_off(blob_limit);
    log::warn!(
        "Snapshot exceeds per-run cap of {MAX_SNAPSHOT_FILES_PER_RUN} files ({total_including_manifest} declared); dropping {} blob(s) from upload (task {task_id})",
        dropped.len(),
    );
    let err_msg = format!("exceeded per-run snapshot cap of {MAX_SNAPSHOT_FILES_PER_RUN} files");
    for dropped_file in dropped {
        mark_capped_manifest_entry(repos, files, &dropped_file.filename, &err_msg);
        pre_upload_entries.push(EntryResult {
            label: dropped_file.filename,
            status: EntryStatus::Skipped,
            error: Some(err_msg.clone()),
        });
    }
}

/// Rewrite the manifest entry matching `filename` (by `patch_file` or `snapshot_file`) to
/// `skipped` with the given error message. Used when blobs are dropped to honor the per-run cap.
fn mark_capped_manifest_entry(
    repos: &mut [RepoManifestEntry],
    files: &mut [FileManifestEntry],
    filename: &str,
    err_msg: &str,
) {
    if let Some(repo_entry) = repos
        .iter_mut()
        .find(|r| r.patch_file.as_deref() == Some(filename))
    {
        repo_entry.status = "skipped";
        repo_entry.uploaded = Some(false);
        repo_entry.error = Some(err_msg.to_string());
    } else if let Some(file_entry) = files
        .iter_mut()
        .find(|f| f.snapshot_file.as_deref() == Some(filename))
    {
        file_entry.status = "skipped";
        file_entry.uploaded = Some(false);
        file_entry.error = Some(err_msg.to_string());
    }
}

/// Clone an [`UploadTarget`] and ensure its `Content-Type` header matches `mime_type`
/// (preserving any casing the server used if the header is already present).
fn merge_content_type(target: &UploadTarget, mime_type: &str) -> UploadTarget {
    let mut headers = target.headers.clone();
    if !headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("content-type"))
    {
        headers.insert("Content-Type".to_string(), mime_type.to_string());
    }
    UploadTarget {
        url: target.url.clone(),
        method: target.method.clone(),
        headers,
    }
}

/// Log the final outcome at INFO when everything uploaded, WARN otherwise. The log line
/// includes per-entry statuses so operators can diagnose partial state without parsing any
/// downstream logs.
fn log_snapshot_outcome(outcome: &SnapshotOutcome, task_id: &AmbientAgentTaskId) {
    let summary = SnapshotSummary::from_entries(&outcome.entries, outcome.manifest_uploaded);
    let manifest_bit = if summary.manifest_uploaded {
        "manifest: uploaded"
    } else {
        "manifest: failed"
    };
    let header = format!(
        "Snapshot upload: {}/{} uploaded (failed: {}, skipped: {}, gather_failed: {}, read_failed: {}; {manifest_bit}) (task {task_id})",
        summary.uploaded,
        summary.total,
        summary.failed,
        summary.skipped,
        summary.gather_failed,
        summary.read_failed,
    );
    if summary.all_uploaded() {
        log::info!("{header}");
        for e in &outcome.entries {
            log::info!("  {}: {}", e.label, e.status.as_str());
        }
    } else {
        log::warn!("{header}");
        for e in &outcome.entries {
            match &e.error {
                Some(err) => {
                    log::warn!("  {}: {} ({err})", e.label, e.status.as_str());
                }
                None => {
                    log::warn!("  {}: {}", e.label, e.status.as_str());
                }
            }
        }
    }
}

// --- Git-diff and filename helpers ---
struct RepoMetadata {
    repo_name: String,
    branch: Option<String>,
    head_sha: Option<String>,
}

async fn repo_metadata(repo_dir: &Path) -> RepoMetadata {
    RepoMetadata {
        repo_name: repo_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo")
            .to_string(),
        branch: git_output_string(repo_dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await,
        head_sha: git_output_string(repo_dir, &["rev-parse", "HEAD"]).await,
    }
}

async fn build_repo_patch(repo_dir: &Path) -> Result<Vec<u8>> {
    let mut patch = git_output_bytes(repo_dir, ["diff", "--binary", "HEAD"], &[0]).await?;
    let untracked_listing = git_output_bytes(
        repo_dir,
        ["ls-files", "--others", "--exclude-standard", "-z"],
        &[0],
    )
    .await?;

    for raw_path in untracked_listing.split(|byte| *byte == 0) {
        if raw_path.is_empty() {
            continue;
        }
        let path = untracked_path_arg(raw_path);
        let args = [
            OsString::from("diff"),
            OsString::from("--binary"),
            OsString::from("--no-index"),
            OsString::from("--"),
            OsString::from("/dev/null"),
            path,
        ];
        let untracked_patch = git_output_bytes(repo_dir, args, &[0, 1]).await?;
        if untracked_patch.is_empty() {
            continue;
        }
        if !patch.is_empty() && !patch.ends_with(b"\n") {
            patch.push(b'\n');
        }
        patch.extend_from_slice(&untracked_patch);
    }

    Ok(patch)
}

fn untracked_path_arg(raw_path: &[u8]) -> OsString {
    #[cfg(unix)]
    {
        OsStr::from_bytes(raw_path).to_os_string()
    }
    #[cfg(not(unix))]
    {
        String::from_utf8_lossy(raw_path).into_owned().into()
    }
}

async fn git_output_string(repo_dir: &Path, args: &[&str]) -> Option<String> {
    let output = git_output_bytes(repo_dir, args, &[0]).await.ok()?;
    let value = String::from_utf8(output).ok()?;
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn sanitize_filename_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "repo".to_string()
    } else {
        trimmed.to_string()
    }
}

fn unique_filename(preferred: &str, used: &mut HashSet<String>) -> String {
    let preferred = Path::new(preferred)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "snapshot_artifact".to_string());
    let preferred = if preferred.is_empty() {
        "snapshot_artifact".to_string()
    } else {
        preferred
    };

    if used.insert(preferred.clone()) {
        return preferred;
    }

    let path = Path::new(&preferred);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "snapshot_artifact".to_string());
    let extension = path.extension().map(|e| e.to_string_lossy().to_string());

    for suffix in 2.. {
        let candidate = match &extension {
            Some(extension) if !extension.is_empty() => format!("{stem}_{suffix}.{extension}"),
            _ => format!("{stem}_{suffix}"),
        };
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    unreachable!("unbounded suffix loop should always return");
}

/// Run `git <args>` in `repo_dir` and return stdout bytes. Fails when the process exits with an
/// exit code outside `allowed_exit_codes` or when it runs longer than [`GIT_COMMAND_TIMEOUT`].
///
/// The whole call is async: the `async_process` child is awaited via `Command::output()` with a
/// timeout composed on top, so no additional OS threads are spawned per git invocation and no
/// polling loop is needed. `kill_on_drop` ensures the child is reaped if the timeout elapses and
/// the future is dropped.
async fn git_output_bytes<I, S>(
    repo_dir: &Path,
    args: I,
    allowed_exit_codes: &[i32],
) -> Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect::<Vec<_>>();
    let mut command = Command::new("git");
    command
        .args(&args)
        .current_dir(repo_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = match command.output().with_timeout(GIT_COMMAND_TIMEOUT).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(anyhow::Error::new(e).context(format!(
                "Failed to run git {:?} in {}",
                args,
                repo_dir.display()
            )));
        }
        Err(_) => anyhow::bail!(
            "git {:?} timed out after {:?} in {}",
            args,
            GIT_COMMAND_TIMEOUT,
            repo_dir.display()
        ),
    };

    let status_code = output.status.code().unwrap_or(-1);
    if !allowed_exit_codes.contains(&status_code) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {:?} failed in {}: {stderr}", args, repo_dir.display());
    }
    Ok(output.stdout)
}

// Snapshot upload is cloud-agent-only and only ever runs inside a Linux Docker container, so
// skip the tests on Windows rather than teach every fixture to emit POSIX paths.
#[cfg(all(test, not(windows)))]
#[path = "snapshot_tests.rs"]
mod tests;
