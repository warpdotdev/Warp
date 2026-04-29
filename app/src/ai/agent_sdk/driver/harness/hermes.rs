use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use warp_cli::agent::Harness;
use warpui::{ModelHandle, ModelSpawner};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::server::server_api::harness_support::HarnessSupportClient;
use crate::server::server_api::ServerApi;
use crate::terminal::model::block::BlockId;
use crate::terminal::CLIAgent;

use super::super::terminal::{CommandHandle, TerminalDriver};
use super::super::{AgentDriver, AgentDriverError};
use super::{write_temp_file, HarnessRunner, ResumePayload, SavePoint, ThirdPartyHarness};

pub(crate) struct HermesHarness;

/// Format slug sent to the server when creating a Hermes conversation.
const HERMES_AGENT_FORMAT: &str = "hermes_agent_cli";
/// Command used to exit Hermes.
const HERMES_EXIT_COMMAND: &str = "/exit";

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ThirdPartyHarness for HermesHarness {
    fn harness(&self) -> Harness {
        Harness::Hermes
    }

    fn cli_agent(&self) -> CLIAgent {
        CLIAgent::Hermes
    }

    fn install_docs_url(&self) -> Option<&'static str> {
        Some("https://hermes-agent.nousresearch.com/docs/")
    }

    fn build_runner(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        _resumption_prompt: Option<&str>,
        working_dir: &Path,
        _task_id: Option<AmbientAgentTaskId>,
        server_api: Arc<ServerApi>,
        terminal_driver: ModelHandle<TerminalDriver>,
        _resume: Option<ResumePayload>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError> {
        // Hermes does not support conversation resume yet. When it does, it will add its
        // own `ResumePayload::Hermes(..)` variant and override `fetch_resume_payload`.
        let client: Arc<dyn HarnessSupportClient> = server_api;
        Ok(Box::new(HermesHarnessRunner::new(
            self.cli_agent().command_prefix(),
            prompt,
            system_prompt,
            working_dir,
            client,
            terminal_driver,
        )?))
    }
}

/// Build the shell command that launches the Hermes Agent CLI.
///
/// `hermes chat -q` runs in non-interactive one-shot mode: reads the prompt,
/// executes, and exits. For interactive TUI mode, we use `hermes` directly
/// and pipe the prompt via stdin.
///
/// We use `-q` (query mode) with the prompt passed via a temp file to avoid
/// shell-quoting issues, and `--yolo` to auto-approve tool calls (matching
/// how Claude Code uses `--dangerously-skip-permissions`).
fn hermes_command(cli_name: &str, prompt_path: &str, system_prompt_path: Option<&str>) -> String {
    let mut cmd = format!("{cli_name} chat -q \"$(cat '{prompt_path}')\" --yolo");
    if let Some(sp_path) = system_prompt_path {
        cmd.push_str(&format!(" -s \"$(cat '{sp_path}')\""));
    }
    cmd
}

/// Runtime state of a [`HermesHarnessRunner`].
enum HermesRunnerState {
    /// Runner is built but [`HarnessRunner::start`] has not been called yet.
    Preexec,
    /// The harness command is running (or has finished).
    Running {
        conversation_id: AIConversationId,
        block_id: BlockId,
    },
}

struct HermesHarnessRunner {
    command: String,
    /// Held so the temp file is cleaned up when the runner is dropped.
    _temp_prompt_file: tempfile::NamedTempFile,
    /// Held so the system prompt temp file is cleaned up when the runner is dropped.
    _temp_system_prompt_file: Option<tempfile::NamedTempFile>,
    client: Arc<dyn HarnessSupportClient>,
    terminal_driver: ModelHandle<TerminalDriver>,
    state: Mutex<HermesRunnerState>,
}

impl HermesHarnessRunner {
    fn new(
        cli_command: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        _working_dir: &Path,
        client: Arc<dyn HarnessSupportClient>,
        terminal_driver: ModelHandle<TerminalDriver>,
    ) -> Result<Self, AgentDriverError> {
        // Write the prompt to a temp file so we can feed it via stdin redirect,
        // avoiding shell-quoting issues with complex content (e.g. skill instructions).
        let temp_file = write_temp_file("oz_prompt_", prompt)?;
        let prompt_path = temp_file.path().display().to_string();

        let temp_system_prompt_file = system_prompt
            .map(|sp| write_temp_file("oz_system_prompt_", sp))
            .transpose()?;
        let system_prompt_path = temp_system_prompt_file
            .as_ref()
            .map(|f| f.path().display().to_string());

        Ok(Self {
            command: hermes_command(cli_command, &prompt_path, system_prompt_path.as_deref()),
            _temp_prompt_file: temp_file,
            _temp_system_prompt_file: temp_system_prompt_file,
            client,
            terminal_driver,
            state: Mutex::new(HermesRunnerState::Preexec),
        })
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl HarnessRunner for HermesHarnessRunner {
    async fn start(
        &self,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<CommandHandle, AgentDriverError> {
        // Create the external conversation record on the server.
        let conversation_id = self
            .client
            .create_external_conversation(HERMES_AGENT_FORMAT)
            .await
            .map_err(|e| {
                log::error!("Failed to create external conversation: {e}");
                AgentDriverError::ConfigBuildFailed(e)
            })?;
        log::info!("Created external conversation {conversation_id}");

        let command = self.command.clone();
        let terminal_driver = self.terminal_driver.clone();
        let command_handle = foreground
            .spawn(move |_, ctx| {
                terminal_driver.update(ctx, |driver, ctx| driver.execute_command(&command, ctx))
            })
            .await??
            .await?;

        // Only store conversation info once the CLI command has started.
        *self.state.lock() = HermesRunnerState::Running {
            conversation_id,
            block_id: command_handle.block_id().clone(),
        };

        Ok(command_handle)
    }

    async fn exit(&self, foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        log::info!("Sending /exit to Hermes Agent CLI");
        let terminal_driver = self.terminal_driver.clone();
        foreground
            .spawn(move |_, ctx| {
                terminal_driver.update(ctx, |driver, ctx| {
                    driver.send_text_to_cli(HERMES_EXIT_COMMAND.to_string(), ctx);
                });
            })
            .await
            .map_err(|_| anyhow::anyhow!("Agent driver dropped while sending /exit"))
    }

    async fn save_conversation(
        &self,
        save_point: SavePoint,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<()> {
        if matches!(save_point, SavePoint::Periodic)
            && !super::has_running_cli_agent(&self.terminal_driver, foreground).await
        {
            log::debug!("Will not save conversation, Hermes not in progress");
            return Ok(());
        }

        let (conversation_id, block_id) = match &*self.state.lock() {
            HermesRunnerState::Preexec => {
                log::warn!("save_conversation called before start");
                return Ok(());
            }
            HermesRunnerState::Running {
                conversation_id,
                block_id,
            } => (*conversation_id, block_id.clone()),
        };

        // TODO: Also save the Hermes conversation transcript when Hermes exposes
        // a transcript export API similar to Claude Code's JSONL format.
        super::upload_current_block_snapshot(
            foreground,
            &self.terminal_driver,
            self.client.as_ref(),
            conversation_id,
            block_id,
        )
        .await
    }
}

#[cfg(test)]
#[path = "hermes_tests.rs"]
mod tests;
