use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tempfile::NamedTempFile;
use uuid::Uuid;
use warp_cli::agent::Harness;
use warpui::{ModelHandle, ModelSpawner};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent_events::AgentEventStreamClient;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::terminal::model::block::BlockId;
use crate::terminal::model::session::ExecuteCommandOptions;
use crate::terminal::CLIAgent;

use super::super::terminal::{CommandHandle, TerminalDriver};
use super::super::{AgentDriver, AgentDriverError};
use super::json_utils::{read_json_file_or_default, write_json_file};
use super::{write_temp_file, HarnessRunner, ManagedSecretValue, SavePoint, ThirdPartyHarness};
mod parent_bridge;

#[cfg(test)]
use super::super::OZ_MESSAGE_LISTENER_STATE_ROOT_ENV;
use parent_bridge::MessageBridge;
#[cfg(test)]
use parent_bridge::{
    acknowledge_parent_bridge_hook_output, ensure_parent_bridge_state_dir,
    parent_bridge_char_count, parent_bridge_hook_output_ack_file, parent_bridge_hook_output_file,
    parent_bridge_root, parent_bridge_staged_message_path, parent_bridge_surfaced_message_path,
    prepare_parent_bridge_hook_output, render_parent_bridge_message_block,
    stage_parent_bridge_message, MessageBridgeHookOutput, MessageBridgeMessageRecord,
    MESSAGE_BRIDGE_CONTEXT_PREAMBLE,
};

pub(crate) struct ClaudeHarness;

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

    fn build_runner(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        resumption_prompt: Option<&str>,
        working_dir: &Path,
        task_id: Option<AmbientAgentTaskId>,
        agent_event_stream_client: Arc<dyn AgentEventStreamClient>,
        terminal_driver: ModelHandle<TerminalDriver>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError> {
        // Claude treats the user-turn message as immediate intent, so the resumption preamble
        // is most reliable when prepended directly to the prompt that gets piped into the CLI.
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
            agent_event_stream_client,
            terminal_driver,
        )?))
    }
}

/// Command used to exit claude.
const CLAUDE_EXIT_COMMAND: &str = "/exit";

/// Build the shell command that launches the Claude CLI for a given session and
/// prompt file.
///
/// The CLI receives `--session-id <uuid>` to pin a fresh local session to that id.
/// If `system_prompt_path` is provided, the CLI appends its contents to the base
/// system prompt.
fn claude_command(
    cli_name: &str,
    session_id: &Uuid,
    prompt_path: &str,
    system_prompt_path: Option<&str>,
) -> String {
    let mut cmd = format!("{cli_name} --session-id {session_id} --dangerously-skip-permissions");
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
    agent_event_stream_client: Arc<dyn AgentEventStreamClient>,
    terminal_driver: ModelHandle<TerminalDriver>,
    state: Mutex<ClaudeRunnerState>,
    session_id: Uuid,
    working_dir: PathBuf,
    parent_bridge: Option<MessageBridge>,
    /// Lazily cached output of `claude --version`.
    claude_version: Mutex<Option<String>>,
}

impl ClaudeHarnessRunner {
    #[allow(clippy::too_many_arguments)]
    fn new(
        cli_command: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        working_dir: &Path,
        task_id: Option<AmbientAgentTaskId>,
        agent_event_stream_client: Arc<dyn AgentEventStreamClient>,
        terminal_driver: ModelHandle<TerminalDriver>,
    ) -> Result<Self, AgentDriverError> {
        // Write the prompt to a temp file so we can feed it via stdin redirect,
        // avoiding shell-quoting issues with complex content (e.g. skill instructions).
        let temp_file = write_temp_file("oz_prompt_", prompt)?;
        let prompt_path = temp_file.path().display().to_string();

        let session_id = Uuid::new_v4();

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

        Ok(Self {
            command: claude_command(
                cli_command,
                &session_id,
                &prompt_path,
                system_prompt_path.as_deref(),
            ),
            cli_name: cli_command.to_string(),
            _temp_prompt_file: temp_file,
            _temp_system_prompt_file: temp_system_prompt_file,
            agent_event_stream_client,
            terminal_driver,
            state: Mutex::new(ClaudeRunnerState::Preexec),
            session_id,
            working_dir: working_dir.to_path_buf(),
            parent_bridge,
            claude_version: Mutex::new(None),
        })
    }
}

impl ClaudeHarnessRunner {
    async fn handle_parent_bridge_session_update(&self) -> Result<()> {
        let Some(parent_bridge) = self.parent_bridge.as_ref() else {
            return Ok(());
        };
        parent_bridge.handle_session_update().await
    }

    async fn flush_parent_bridge_acks(&self) -> Result<()> {
        let Some(parent_bridge) = self.parent_bridge.as_ref() else {
            return Ok(());
        };
        parent_bridge.flush_acks().await
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
            .start(foreground, self.agent_event_stream_client.clone())
            .await
    }

    fn cleanup_parent_bridge(&self) -> Result<()> {
        if let Some(parent_bridge) = self.parent_bridge.as_ref() {
            parent_bridge.cleanup()?;
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
        let conversation_id = AIConversationId::new();
        log::info!("Created local Claude conversation {conversation_id}");
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
                self.cleanup_parent_bridge()
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

        let _ = (foreground, conversation_id, block_id, claude_version);
        log::debug!("Skipping Claude transcript and block snapshot export in OpenWarp");

        Ok(())
    }
    async fn cleanup(&self, _foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        self.flush_parent_bridge_acks().await?;
        self.cleanup_parent_bridge()
    }
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

fn claude_config_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))
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

#[derive(Default, Deserialize, Serialize, Debug)]
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

#[derive(Default, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CustomApiKeyResponses {
    #[serde(default)]
    approved: Vec<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Default, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ClaudeProjectConfig {
    #[serde(default)]
    has_trust_dialog_accepted: bool,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Default, Deserialize, Serialize, Debug)]
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
