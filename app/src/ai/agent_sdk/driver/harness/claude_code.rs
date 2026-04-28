use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tempfile::NamedTempFile;
use uuid::Uuid;
use warp_cli::agent::Harness;
use warp_core::safe_warn;
use warpui::{ModelHandle, ModelSpawner};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent_events::MessageHydrator;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::server::server_api::harness_support::{
    upload_to_target, HarnessSupportClient, ResolvePromptRequest,
};
use crate::server::server_api::ServerApi;
use crate::terminal::model::block::BlockId;
use crate::terminal::model::session::ExecuteCommandOptions;
use crate::terminal::CLIAgent;

use super::super::terminal::{CommandHandle, TerminalDriver};
use super::super::{AgentDriver, AgentDriverError};
use super::json_utils::{read_json_file_or_default, write_json_file};
use super::{
    cli_agent_session_status, write_temp_file, HarnessCleanupDisposition, HarnessRunner,
    ManagedSecretValue, ResumePayload, SavePoint, ThirdPartyHarness,
};
mod parent_bridge;

#[cfg(test)]
use super::super::OZ_MESSAGE_LISTENER_STATE_ROOT_ENV;
use parent_bridge::{
    acknowledge_parent_bridge_hook_output, ensure_parent_bridge_state_dir,
    parent_bridge_max_context_chars, parent_bridge_root, prepare_parent_bridge_hook_output,
    stage_parent_bridge_message, MessageBridge, MessageBridgeCleanupDisposition,
    MessageBridgeMessageRecord,
};
#[cfg(test)]
use parent_bridge::{
    parent_bridge_char_count, parent_bridge_event_cursor_file, parent_bridge_hook_output_ack_file,
    parent_bridge_hook_output_file, parent_bridge_staged_message_path,
    parent_bridge_surfaced_message_path, read_parent_bridge_event_cursor,
    render_parent_bridge_message_block, write_parent_bridge_event_cursor, MessageBridgeHookOutput,
    MESSAGE_BRIDGE_CONTEXT_PREAMBLE,
};

#[derive(Debug)]
pub(crate) struct ClaudeResumeInfo {
    pub(crate) conversation_id: AIConversationId,
    pub(crate) session_id: Uuid,
    pub(crate) envelope: ClaudeTranscriptEnvelope,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaudeWakeMessage {
    pub(crate) sequence: i64,
    pub(crate) message_id: String,
    pub(crate) sender_run_id: String,
    pub(crate) subject: String,
    pub(crate) body: String,
    pub(crate) occurred_at: String,
}

impl From<ClaudeWakeMessage> for MessageBridgeMessageRecord {
    fn from(value: ClaudeWakeMessage) -> Self {
        Self {
            sequence: value.sequence,
            message_id: value.message_id,
            sender_run_id: value.sender_run_id,
            subject: value.subject,
            body: value.body,
            occurred_at: value.occurred_at,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ClaudeWakeRemoteContext {
    session_id: Uuid,
    envelope: ClaudeTranscriptEnvelope,
    wake_prompt: String,
}

pub(crate) struct ClaudeHarness;

impl ClaudeHarness {
    pub(crate) async fn fetch_local_wake_remote_context(
        task_id: AmbientAgentTaskId,
        server_api: Arc<ServerApi>,
    ) -> Result<ClaudeWakeRemoteContext> {
        let resolved = server_api
            .resolve_prompt_for_task(
                &task_id,
                ResolvePromptRequest {
                    skill: None,
                    attachments_dir: None,
                },
            )
            .await
            .with_context(|| format!("Failed to resolve Claude wake prompt for task {task_id}"))?;
        let bytes = server_api
            .fetch_transcript_for_task(&task_id)
            .await
            .with_context(|| format!("Failed to fetch Claude transcript for task {task_id}"))?;
        let envelope: ClaudeTranscriptEnvelope =
            serde_json::from_slice(&bytes).with_context(|| {
                format!("Failed to deserialize Claude transcript for wake task {task_id}")
            })?;
        let wake_prompt = match resolved.resumption_prompt {
            Some(resumption_prompt) if !resumption_prompt.is_empty() => {
                format!(
                    "{resumption_prompt}

{CLAUDE_WAKE_PROMPT}"
                )
            }
            _ => CLAUDE_WAKE_PROMPT.to_string(),
        };
        Ok(ClaudeWakeRemoteContext {
            session_id: envelope.uuid,
            envelope,
            wake_prompt,
        })
    }

    pub(crate) async fn prepare_local_wake_command(
        server_api: Arc<ServerApi>,
        working_dir: Option<PathBuf>,
        mut remote: ClaudeWakeRemoteContext,
        pending_messages: Vec<ClaudeWakeMessage>,
    ) -> Result<String> {
        let working_dir = working_dir.unwrap_or_else(|| remote.envelope.cwd.clone());
        prepare_claude_environment_config(&working_dir, &HashMap::new())
            .context("Failed to prepare Claude environment for wake")?;

        remote.envelope.cwd = working_dir.clone();
        let config_root = claude_config_dir().context("Failed to resolve Claude config dir")?;
        write_envelope(&remote.envelope, &config_root)
            .context("Failed to rehydrate Claude transcript for wake")?;
        if let Err(error) = write_session_index_entry(remote.session_id, &working_dir, &config_root)
        {
            log::warn!("Failed to update Claude sessions-index.json for wake: {error:#}");
        }

        let state_dir = parent_bridge_root()?.join(remote.session_id.to_string());
        ensure_parent_bridge_state_dir(&state_dir)?;
        let hydrator = MessageHydrator::new(server_api);
        acknowledge_parent_bridge_hook_output(&hydrator, &state_dir).await?;
        for record in pending_messages
            .into_iter()
            .map(MessageBridgeMessageRecord::from)
        {
            stage_parent_bridge_message(&state_dir, &record)?;
        }
        prepare_parent_bridge_hook_output(&hydrator, &state_dir, parent_bridge_max_context_chars())
            .await?;

        let prompt_path = state_dir.join(CLAUDE_WAKE_PROMPT_FILE_NAME);
        std::fs::write(&prompt_path, remote.wake_prompt.as_bytes())
            .with_context(|| format!("Failed to write {}", prompt_path.display()))?;

        Ok(claude_command(
            CLIAgent::Claude.command_prefix(),
            &remote.session_id,
            &prompt_path.display().to_string(),
            None,
            true,
        ))
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ThirdPartyHarness for ClaudeHarness {
    fn harness(&self) -> Harness {
        Harness::Claude
    }

    fn cli_agent(&self) -> CLIAgent {
        CLIAgent::Claude
    }

    fn install_docs_url(&self) -> Option<&'static str> {
        Some("https://code.claude.com/docs/en/quickstart")
    }

    fn prepare_environment_config(
        &self,
        working_dir: &Path,
        _system_prompt: Option<&str>,
        secrets: &HashMap<String, ManagedSecretValue>,
    ) -> Result<(), AgentDriverError> {
        prepare_claude_environment_config(working_dir, secrets).map_err(|error| {
            AgentDriverError::HarnessConfigSetupFailed {
                harness: self.cli_agent().command_prefix().to_owned(),
                error,
            }
        })
    }

    async fn fetch_resume_payload(
        &self,
        conversation_id: &AIConversationId,
        harness_support_client: Arc<dyn HarnessSupportClient>,
    ) -> Result<Option<ResumePayload>, AgentDriverError> {
        let conversation_id_str = conversation_id.to_string();
        let bytes = harness_support_client
            .fetch_transcript()
            .await
            .map_err(|err| {
                let message = format!("{err:#}").to_lowercase();
                if message.contains("status 404") {
                    AgentDriverError::ConversationResumeStateMissing {
                        harness: "claude".to_string(),
                        conversation_id: conversation_id_str.clone(),
                    }
                } else {
                    AgentDriverError::ConversationLoadFailed(format!("{err:#}"))
                }
            })?;
        let envelope: ClaudeTranscriptEnvelope = serde_json::from_slice(&bytes).map_err(|err| {
            AgentDriverError::ConversationLoadFailed(format!(
                "Failed to deserialize Claude transcript for {conversation_id_str}: {err:#}"
            ))
        })?;
        Ok(Some(ResumePayload::Claude(ClaudeResumeInfo {
            conversation_id: *conversation_id,
            session_id: envelope.uuid,
            envelope,
        })))
    }

    fn build_runner(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        resumption_prompt: Option<&str>,
        working_dir: &Path,
        task_id: Option<AmbientAgentTaskId>,
        server_api: Arc<ServerApi>,
        terminal_driver: ModelHandle<TerminalDriver>,
        resume: Option<ResumePayload>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError> {
        let claude_resume = resume.map(|payload| match payload {
            ResumePayload::Claude(info) => info,
        });
        let owned_prompt = match resumption_prompt {
            Some(preamble) if !preamble.is_empty() => format!("{preamble}\n\n{prompt}"),
            _ => prompt.to_string(),
        };
        Ok(Box::new(ClaudeHarnessRunner::new(
            self.cli_agent().command_prefix(),
            &owned_prompt,
            system_prompt,
            working_dir,
            task_id,
            server_api,
            terminal_driver,
            claude_resume,
        )?))
    }
}

/// Format slug sent to the server when creating a Claude Code conversation.
const CLAUDE_CODE_FORMAT: &str = "claude_code_cli";
/// Command used to exit claude.
const CLAUDE_EXIT_COMMAND: &str = "/exit";
const CLAUDE_WAKE_PROMPT: &str =
    "New lead-agent messages are available. Read the latest lead-agent updates and continue the task accordingly.";
const CLAUDE_WAKE_PROMPT_FILE_NAME: &str = "wake-turn-prompt.txt";

/// Build the shell command that launches the Claude CLI for a given session and
/// prompt file.
fn claude_command(
    cli_name: &str,
    session_id: &Uuid,
    prompt_path: &str,
    system_prompt_path: Option<&str>,
    resuming: bool,
) -> String {
    let flag = if resuming { "--resume" } else { "--session-id" };
    let mut cmd = format!("{cli_name} {flag} {session_id} --dangerously-skip-permissions");
    if let Some(sp_path) = system_prompt_path {
        let _ = write!(cmd, " --append-system-prompt-file '{sp_path}'");
    }
    format!("{cmd} < '{prompt_path}'")
}

/// Runtime state of a [`ClaudeHarnessRunner`].
enum ClaudeRunnerState {
    /// Runner is built but [`HarnessRunner::start`] has not been called yet.
    Preexec,
    /// The harness command is running (or has finished).
    Running {
        conversation_id: AIConversationId,
        block_id: BlockId,
    },
}

struct ClaudeHarnessRunner {
    command: String,
    /// The CLI name used to invoke Claude Code.
    cli_name: String,
    /// Held so the temp file is cleaned up when the runner is dropped.
    _temp_prompt_file: NamedTempFile,
    /// Held so the system prompt temp file is cleaned up when the runner is dropped.
    _temp_system_prompt_file: Option<NamedTempFile>,
    client: Arc<dyn HarnessSupportClient>,
    server_api: Arc<ServerApi>,
    terminal_driver: ModelHandle<TerminalDriver>,
    state: Mutex<ClaudeRunnerState>,
    session_id: Uuid,
    working_dir: PathBuf,
    parent_bridge: Option<MessageBridge>,
    /// Lazily cached output of `claude --version`.
    claude_version: Mutex<Option<String>>,
    preexisting_conversation_id: Option<AIConversationId>,
}

impl ClaudeHarnessRunner {
    fn new(
        cli_command: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        working_dir: &Path,
        task_id: Option<AmbientAgentTaskId>,
        server_api: Arc<ServerApi>,
        terminal_driver: ModelHandle<TerminalDriver>,
        resume: Option<ClaudeResumeInfo>,
    ) -> Result<Self, AgentDriverError> {
        // Write the prompt to a temp file so we can feed it via stdin redirect,
        // avoiding shell-quoting issues with complex content (e.g. skill instructions).
        let temp_file = write_temp_file("oz_prompt_", prompt)?;
        let prompt_path = temp_file.path().display().to_string();
        let (session_id, preexisting_conversation_id, resuming) = match resume {
            Some(ClaudeResumeInfo {
                conversation_id,
                session_id,
                mut envelope,
            }) => {
                envelope.cwd = working_dir.to_path_buf();
                let config_root = claude_config_dir().map_err(|error| {
                    AgentDriverError::ConfigBuildFailed(
                        error.context("Failed to resolve Claude config dir"),
                    )
                })?;
                write_envelope(&envelope, &config_root).map_err(|error| {
                    AgentDriverError::ConfigBuildFailed(
                        error.context("Failed to rehydrate Claude transcript"),
                    )
                })?;
                if let Err(error) = write_session_index_entry(session_id, working_dir, &config_root)
                {
                    log::warn!("Failed to update Claude sessions-index.json: {error:#}");
                }
                (session_id, Some(conversation_id), true)
            }
            None => (Uuid::new_v4(), None, false),
        };

        let temp_system_prompt_file = system_prompt
            .map(|sp| write_temp_file("oz_system_prompt_", sp))
            .transpose()?;
        let system_prompt_path = temp_system_prompt_file
            .as_ref()
            .map(|f| f.path().display().to_string());
        let parent_bridge = task_id
            .map(|task_id| MessageBridge::new(task_id.to_string(), session_id))
            .transpose()
            .map_err(AgentDriverError::ConfigBuildFailed)?;
        let client: Arc<dyn HarnessSupportClient> = server_api.clone();

        Ok(Self {
            command: claude_command(
                cli_command,
                &session_id,
                &prompt_path,
                system_prompt_path.as_deref(),
                resuming,
            ),
            cli_name: cli_command.to_string(),
            _temp_prompt_file: temp_file,
            _temp_system_prompt_file: temp_system_prompt_file,
            client,
            server_api,
            terminal_driver,
            state: Mutex::new(ClaudeRunnerState::Preexec),
            session_id,
            working_dir: working_dir.to_path_buf(),
            parent_bridge,
            claude_version: Mutex::new(None),
            preexisting_conversation_id,
        })
    }
}

impl ClaudeHarnessRunner {
    async fn handle_parent_bridge_session_update(&self) -> Result<()> {
        let Some(parent_bridge) = self.parent_bridge.as_ref() else {
            return Ok(());
        };
        parent_bridge
            .handle_session_update(self.server_api.clone())
            .await
    }

    async fn flush_parent_bridge_acks(&self) -> Result<()> {
        let Some(parent_bridge) = self.parent_bridge.as_ref() else {
            return Ok(());
        };
        parent_bridge.flush_acks(self.server_api.clone()).await
    }
    /// Return the cached Claude Code version, or resolve it by running
    /// `<cli_name> --version`.
    async fn resolve_claude_version(
        &self,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Option<String> {
        if let Some(cached) = self.claude_version.lock().clone() {
            return Some(cached);
        }

        let terminal_driver = self.terminal_driver.clone();
        let session = foreground
            .spawn(move |_, ctx| {
                let tv = terminal_driver.as_ref(ctx).terminal_view().as_ref(ctx);
                tv.active_session().as_ref(ctx).session(ctx)
            })
            .await
            .ok()?;
        let session = session?;

        let cli_name = &self.cli_name;
        let output = session
            .execute_command(
                &format!("{cli_name} --version"),
                None,
                None,
                ExecuteCommandOptions::default(),
            )
            .await
            .ok()?;

        let version = output.to_string().ok()?.trim().to_string();
        if version.is_empty() {
            return None;
        }

        *self.claude_version.lock() = Some(version.clone());
        Some(version)
    }

    async fn start_parent_bridge(&self, foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        let Some(parent_bridge) = self.parent_bridge.as_ref() else {
            return Ok(());
        };
        parent_bridge
            .start(foreground, self.server_api.clone())
            .await
    }

    async fn should_preserve_parent_bridge(
        &self,
        cleanup_disposition: HarnessCleanupDisposition,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> bool {
        if !matches!(
            cleanup_disposition,
            HarnessCleanupDisposition::PreserveResumptionStateIfSupported
        ) {
            return false;
        }

        !matches!(
            cli_agent_session_status(&self.terminal_driver, foreground).await,
            Some(crate::terminal::cli_agent_sessions::CLIAgentSessionStatus::Blocked { .. })
                | Some(crate::terminal::cli_agent_sessions::CLIAgentSessionStatus::InProgress)
        )
    }

    fn cleanup_parent_bridge(&self, preserve_state: bool) -> Result<()> {
        if let Some(parent_bridge) = self.parent_bridge.as_ref() {
            let cleanup_disposition = if preserve_state {
                MessageBridgeCleanupDisposition::PreserveState
            } else {
                MessageBridgeCleanupDisposition::RemoveState
            };
            parent_bridge.cleanup(cleanup_disposition)?;
        }
        Ok(())
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl HarnessRunner for ClaudeHarnessRunner {
    async fn start(
        &self,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<CommandHandle, AgentDriverError> {
        let conversation_id = match self.preexisting_conversation_id {
            Some(conversation_id) => {
                log::info!("Resuming external conversation {conversation_id}");
                conversation_id
            }
            None => self
                .client
                .create_external_conversation(CLAUDE_CODE_FORMAT)
                .await
                .map_err(|e| {
                    log::error!("Failed to create external conversation: {e}");
                    AgentDriverError::ConfigBuildFailed(e)
                })?,
        };
        if self.preexisting_conversation_id.is_none() {
            log::info!("Created external conversation {conversation_id}");
        }
        self.start_parent_bridge(foreground)
            .await
            .map_err(AgentDriverError::ConfigBuildFailed)?;

        let command = self.command.clone();
        let terminal_driver = self.terminal_driver.clone();
        let command_handle = match foreground
            .spawn(move |_, ctx| {
                terminal_driver.update(ctx, |driver, ctx| driver.execute_command(&command, ctx))
            })
            .await??
            .await
        {
            Ok(command_handle) => command_handle,
            Err(err) => {
                self.cleanup_parent_bridge(false)
                    .map_err(AgentDriverError::ConfigBuildFailed)?;
                return Err(err);
            }
        };

        // Only store conversation info once the CLI command has started.
        *self.state.lock() = ClaudeRunnerState::Running {
            conversation_id,
            block_id: command_handle.block_id().clone(),
        };

        Ok(command_handle)
    }

    async fn exit(&self, foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        log::info!("Sending /exit to Claude Code CLI");
        let terminal_driver = self.terminal_driver.clone();
        foreground
            .spawn(move |_, ctx| {
                terminal_driver.update(ctx, |driver, ctx| {
                    driver.send_text_to_cli(CLAUDE_EXIT_COMMAND.to_string(), ctx);
                });
            })
            .await
            .map_err(|_| anyhow::anyhow!("Agent driver dropped while sending /exit"))
    }

    async fn handle_session_update(&self, _foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        self.handle_parent_bridge_session_update().await
    }

    async fn save_conversation(
        &self,
        save_point: SavePoint,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<()> {
        if matches!(save_point, SavePoint::Periodic)
            && !super::has_running_cli_agent(&self.terminal_driver, foreground).await
        {
            log::debug!("Will not save conversation, Claude Code not in progress");
            return Ok(());
        }

        let (conversation_id, block_id) = match &*self.state.lock() {
            ClaudeRunnerState::Preexec => {
                log::warn!("save_conversation called before start");
                return Ok(());
            }
            ClaudeRunnerState::Running {
                conversation_id,
                block_id,
            } => (*conversation_id, block_id.clone()),
        };

        let claude_version = self.resolve_claude_version(foreground).await;

        let client = self.client.as_ref();
        let session_id = self.session_id;
        let working_dir = &self.working_dir;

        futures::try_join!(
            super::upload_current_block_snapshot(
                foreground,
                &self.terminal_driver,
                client,
                conversation_id,
                block_id,
            ),
            upload_transcript(
                client,
                conversation_id,
                session_id,
                working_dir,
                claude_version
            ),
        )?;

        Ok(())
    }
    async fn cleanup(
        &self,
        cleanup_disposition: HarnessCleanupDisposition,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<()> {
        self.flush_parent_bridge_acks().await?;
        let preserve_state = self
            .should_preserve_parent_bridge(cleanup_disposition, foreground)
            .await;
        self.cleanup_parent_bridge(preserve_state)
    }
}

/// Upload the Claude Code session transcript to the server.
async fn upload_transcript(
    client: &dyn HarnessSupportClient,
    conversation_id: AIConversationId,
    session_id: Uuid,
    working_dir: &Path,
    claude_version: Option<String>,
) -> Result<()> {
    log::info!("Uploading Claude Code transcript to conversation {conversation_id}");

    let config_dir = claude_config_dir().context("Failed to resolve Claude config dir")?;
    let working_dir = working_dir.to_path_buf();
    let body = tokio::task::spawn_blocking(move || {
        let mut envelope = read_envelope(session_id, &working_dir, &config_dir)
            .with_context(|| format!("Failed to read transcript for session {session_id}"))?;
        envelope.claude_version = claude_version;
        serde_json::to_vec(&envelope).context("Failed to serialize transcript envelope")
    })
    .await
    .context("read_envelope task panicked")??;
    let target = client
        .get_transcript_upload_target(&conversation_id)
        .await
        .with_context(|| format!("Failed to get transcript upload target for {conversation_id}"))?;
    upload_to_target(client.http_client(), &target, body).await
}

// ─── Transcript envelope ──────────────────────────────────────────────────────

/// JSON envelope sent to the server representing a complete Claude Code session.
///
/// Bundles the main session transcript, any subagent transcripts, and
/// per-agent TODO lists assembled from the Claude state directory.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ClaudeTranscriptEnvelope {
    /// The directory that the Claude Code session started in.
    cwd: PathBuf,
    /// Unique session identifier.
    uuid: Uuid,
    /// Claude Code version, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claude_version: Option<String>,
    /// List of messages in the main agent conversation.
    entries: Vec<Value>,
    /// Messages in each subagent conversation, keyed by the agent filename (e.g. `"agent-aac0b7f3db6bccfaf"`).
    subagents: HashMap<String, Vec<Value>>,
    /// TODO lists for each agent, keyed on the session and agent (e.g. `"<session_uuid>-agent-<agent_id>"`).
    todos: HashMap<String, Value>,
}

/// Encode a filesystem path as a Claude config directory name, matching the
/// Claude CLI convention of replacing every `/` with `-`.
///
/// Example: `/Users/ben/src/foo` → `-Users-ben-src-foo`
fn encode_cwd(cwd: &Path) -> String {
    cwd.to_string_lossy().replace(['/', '.'], "-")
}

/// Resolve the Claude config directory.
///
/// Reads `$CLAUDE_CONFIG_DIR` if set, otherwise falls back to `~/.claude`.
//
/// TODO(REMOTE-1209): Use the transcript path reported by our hook.
fn claude_config_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))
}

/// Assemble a [`ClaudeTranscriptEnvelope`] from the Claude config directory.
///
/// Reads:
/// - `<config_root>/projects/<encoded_cwd>/<session_uuid>.jsonl` - main transcript
/// - `<config_root>/projects/<encoded_cwd>/<session_uuid>/subagents/*.jsonl` - subagents
/// - `<config_root>/todos/<session_uuid>-agent-*.json` - per-agent todo lists
///
/// If the main JSONL does not exist yet (e.g. during an early periodic save)
/// the envelope is returned with an empty `entries` list rather than an error.
fn read_envelope(
    session_uuid: Uuid,
    cwd: &Path,
    config_root: &Path,
) -> Result<ClaudeTranscriptEnvelope> {
    let encoded = encode_cwd(cwd);
    let projects_dir = config_root.join("projects").join(&encoded);

    // Main session transcript.
    let session_file = projects_dir.join(format!("{session_uuid}.jsonl"));
    let entries = read_jsonl(&session_file)?;

    // Subagents are stored in a directory named after the session UUID.
    let mut subagents: HashMap<String, Vec<Value>> = HashMap::new();
    let subagents_dir = projects_dir
        .join(session_uuid.to_string())
        .join("subagents");
    if subagents_dir.is_dir() {
        for entry in std::fs::read_dir(&subagents_dir)
            .with_context(|| format!("Failed to read subagents dir {}", subagents_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            subagents.insert(stem.to_owned(), read_jsonl(&path)?);
        }
    }

    // Per-agent todo lists.
    let mut todos: HashMap<String, Value> = HashMap::new();
    let todos_dir = config_root.join("todos");
    let todos_prefix = format!("{session_uuid}-agent-");
    if todos_dir.is_dir() {
        for entry in std::fs::read_dir(&todos_dir)
            .with_context(|| format!("Failed to read todos dir {}", todos_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !stem.starts_with(&todos_prefix) {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(value) => {
                        todos.insert(stem.to_owned(), value);
                    }
                    Err(e) => log::warn!("Failed to parse todos file {}: {e}", path.display()),
                },
                Err(e) => log::warn!("Failed to read todos file {}: {e}", path.display()),
            }
        }
    }

    Ok(ClaudeTranscriptEnvelope {
        cwd: cwd.to_path_buf(),
        uuid: session_uuid,
        claude_version: None,
        entries,
        subagents,
        todos,
    })
}

/// Write a [`ClaudeTranscriptEnvelope`] back to disk using the same layout
/// that Claude Code uses.
///
/// Creates:
/// - `<config_root>/projects/<encoded_cwd>/<uuid>.jsonl` - main transcript
/// - `<config_root>/projects/<encoded_cwd>/<uuid>/subagents/<stem>.jsonl` - subagents
/// - `<config_root>/todos/<stem>.json` - per-agent todo lists
fn write_envelope(envelope: &ClaudeTranscriptEnvelope, config_root: &Path) -> Result<()> {
    let encoded = encode_cwd(&envelope.cwd);
    let projects_dir = config_root.join("projects").join(&encoded);
    std::fs::create_dir_all(&projects_dir)
        .with_context(|| format!("Failed to create {}", projects_dir.display()))?;

    // Main session JSONL.
    let session_file = projects_dir.join(format!("{}.jsonl", envelope.uuid));
    std::fs::write(&session_file, entries_to_jsonl(&envelope.entries)?)
        .with_context(|| format!("Failed to write {}", session_file.display()))?;

    // Subagent JSONLs.
    if !envelope.subagents.is_empty() {
        let subagents_dir = projects_dir
            .join(envelope.uuid.to_string())
            .join("subagents");
        std::fs::create_dir_all(&subagents_dir)
            .with_context(|| format!("Failed to create {}", subagents_dir.display()))?;
        for (stem, entries) in &envelope.subagents {
            let path = subagents_dir.join(format!("{stem}.jsonl"));
            std::fs::write(&path, entries_to_jsonl(entries)?)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
    }

    // Per-agent todo lists.
    if !envelope.todos.is_empty() {
        let todos_dir = config_root.join("todos");
        std::fs::create_dir_all(&todos_dir)
            .with_context(|| format!("Failed to create {}", todos_dir.display()))?;
        for (stem, value) in &envelope.todos {
            let path = todos_dir.join(format!("{stem}.json"));
            std::fs::write(&path, serde_json::to_vec(value)?)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
    }

    Ok(())
}

/// Filename of Claude's global session index.
const SESSIONS_INDEX_FILENAME: &str = "sessions-index.json";

/// Upsert an entry for `session_uuid` into `<config_root>/sessions-index.json` so Claude's
/// `claude --resume <uuid>` lookup can find the rehydrated jsonl.
///
/// Best-effort: callers should log a warning on failure rather than aborting the run.
fn write_session_index_entry(session_uuid: Uuid, cwd: &Path, config_root: &Path) -> Result<()> {
    let index_path = config_root.join(SESSIONS_INDEX_FILENAME);

    let mut index: serde_json::Map<String, Value> = match std::fs::read_to_string(&index_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(Value::Object(map)) => map,
            Ok(_) => {
                safe_warn!(
                    safe: ("sessions-index.json is not a JSON object; overwriting"),
                    full: ("sessions-index.json at {} is not a JSON object; overwriting", index_path.display())
                );
                serde_json::Map::new()
            }
            Err(error) => {
                safe_warn!(
                    safe: ("Failed to parse sessions-index.json; overwriting"),
                    full: ("Failed to parse sessions-index.json at {}: {error}; overwriting", index_path.display())
                );
                serde_json::Map::new()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::Map::new(),
        Err(error) => {
            return Err(anyhow::Error::from(error)
                .context(format!("Failed to read {}", index_path.display())));
        }
    };

    let encoded = encode_cwd(cwd);
    let transcript_path = format!("projects/{encoded}/{session_uuid}.jsonl");
    let entry = serde_json::json!({
        "sessionId": session_uuid.to_string(),
        "cwd": cwd.to_string_lossy(),
        "projectPath": encoded,
        "transcriptPath": transcript_path,
    });
    index.insert(session_uuid.to_string(), entry);

    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(
        &index_path,
        serde_json::to_vec_pretty(&Value::Object(index))
            .context("Failed to serialize sessions-index.json")?,
    )
    .with_context(|| format!("Failed to write {}", index_path.display()))?;
    Ok(())
}
/// Serialize a slice of JSON values as a JSONL byte string (one value per line).
fn entries_to_jsonl(entries: &[Value]) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    for entry in entries {
        serde_json::to_writer(&mut buf, entry)?;
        buf.push(b'\n');
    }
    Ok(buf)
}

/// Read a JSONL file, returning one parsed [`Value`] per non-blank line.
///
/// Lines that fail to parse as JSON are skipped with a warning rather than
/// causing the entire read to fail. A missing file returns an empty [`Vec`].
fn read_jsonl(path: &Path) -> Result<Vec<Value>> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(
                anyhow::Error::from(e).context(format!("Failed to open {}", path.display()))
            );
        }
    };
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line.with_context(|| format!("Failed to read line from {}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str(trimmed) {
            Ok(value) => entries.push(value),
            Err(e) => {
                safe_warn!(
                    safe: ("Skipping malformed JSONL entry"),
                    full: ("Skipping malformed JSONL entry in {}: {e}", path.display())
                );
            }
        }
    }
    Ok(entries)
}

fn prepare_claude_environment_config(
    working_dir: &Path,
    secrets: &HashMap<String, ManagedSecretValue>,
) -> Result<()> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
    let claude_json_path = home_dir.join(CLAUDE_JSON_FILE_NAME);
    let claude_settings_path = claude_config_dir()?.join(CLAUDE_SETTINGS_FILE_NAME);
    let api_key_suffix = resolve_anthropic_api_key_suffix(secrets);
    prepare_claude_config(&claude_json_path, working_dir, api_key_suffix.as_deref())?;
    prepare_claude_settings(&claude_settings_path)?;
    Ok(())
}

fn prepare_claude_config(
    claude_json_path: &Path,
    working_dir: &Path,
    api_key_suffix: Option<&str>,
) -> Result<()> {
    let mut claude_config: ClaudeConfig = read_json_file_or_default(claude_json_path)?;
    claude_config.has_completed_onboarding = true;
    claude_config.lsp_recommendation_disabled = true;
    claude_config
        .projects
        .entry(working_dir.to_string_lossy().into_owned())
        .or_default()
        .has_trust_dialog_accepted = true;
    if let Some(suffix) = api_key_suffix {
        let responses = claude_config
            .custom_api_key_responses
            .get_or_insert_with(CustomApiKeyResponses::default);
        if !responses.approved.iter().any(|s| s == suffix) {
            responses.approved.push(suffix.to_owned());
        }
    }
    write_json_file(
        claude_json_path,
        &claude_config,
        "Failed to serialize Claude config",
    )?;
    Ok(())
}

fn prepare_claude_settings(claude_settings_path: &Path) -> Result<()> {
    let mut settings: ClaudeSettings = read_json_file_or_default(claude_settings_path)?;
    settings.skip_dangerous_mode_permission_prompt = true;
    write_json_file(
        claude_settings_path,
        &settings,
        "Failed to serialize Claude settings",
    )?;
    Ok(())
}

const ANTHROPIC_API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
const CLAUDE_JSON_FILE_NAME: &str = ".claude.json";
const CLAUDE_SETTINGS_FILE_NAME: &str = "settings.json";
const ANTHROPIC_API_KEY_SUFFIX_LEN: usize = 20;

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeConfig {
    #[serde(default)]
    has_completed_onboarding: bool,
    #[serde(default)]
    lsp_recommendation_disabled: bool,
    #[serde(default)]
    projects: HashMap<String, ClaudeProjectConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    custom_api_key_responses: Option<CustomApiKeyResponses>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomApiKeyResponses {
    #[serde(default)]
    approved: Vec<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeProjectConfig {
    #[serde(default)]
    has_trust_dialog_accepted: bool,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeSettings {
    #[serde(default)]
    skip_dangerous_mode_permission_prompt: bool,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

/// Try to get the last 20 chars of the ANTHROPIC_API_KEY from the secrets map,
/// where 20 chars is the suffix length that Claude Code truncates keys to.
/// Falls back to the environment variable.
fn resolve_anthropic_api_key_suffix(
    secrets: &HashMap<String, ManagedSecretValue>,
) -> Option<String> {
    // First, check for an AnthropicApiKey variant anywhere in the secrets map,
    // since the secret name doesn't necessarily match the env var.
    for secret in secrets.values() {
        if let ManagedSecretValue::AnthropicApiKey { api_key } = secret {
            return suffix_of(api_key).map(str::to_owned);
        }
    }
    // Then check for a RawValue stored under the env var name.
    if let Some(ManagedSecretValue::RawValue { value }) = secrets.get(ANTHROPIC_API_KEY_ENV) {
        return suffix_of(value).map(str::to_owned);
    }
    // Fall back to the environment variable, which a user may have set separately in the env.
    std::env::var(ANTHROPIC_API_KEY_ENV)
        .ok()
        .and_then(|k| suffix_of(&k).map(str::to_owned))
}

fn suffix_of(key: &str) -> Option<&str> {
    if key.len() >= ANTHROPIC_API_KEY_SUFFIX_LEN {
        key.get(key.len() - ANTHROPIC_API_KEY_SUFFIX_LEN..)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "claude_code_tests.rs"]
mod tests;
