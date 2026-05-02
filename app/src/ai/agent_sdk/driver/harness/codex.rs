//! Third-party harness for the OpenAI `codex` CLI.
//!
//! Modeled on [`super::gemini::GeminiHarness`]: the harness writes the seed prompt to a
//! tempfile and shells out to `codex` in interactive mode. Codex maintains its own
//! conversation/session state, so we don't store transcripts client-side beyond what the
//! base `HarnessRunner` already does.

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

pub(crate) struct CodexHarness;

const CODEX_CLI_FORMAT: &str = "codex_cli";
const CODEX_EXIT_COMMAND: &str = "/quit";

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ThirdPartyHarness for CodexHarness {
    fn harness(&self) -> Harness {
        // Reuse `Harness::Unknown` rather than threading a new variant through every match
        // statement in `warp_cli`. The harness is selected here via env var, not the CLI flag,
        // so the enum value is only used for telemetry/logging.
        Harness::Unknown
    }

    fn cli_agent(&self) -> CLIAgent {
        CLIAgent::Codex
    }

    fn install_docs_url(&self) -> Option<&'static str> {
        Some("https://github.com/openai/codex")
    }

    fn build_runner(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        _resumption_prompt: Option<&str>,
        _working_dir: &Path,
        _task_id: Option<AmbientAgentTaskId>,
        server_api: Arc<ServerApi>,
        terminal_driver: ModelHandle<TerminalDriver>,
        _resume: Option<ResumePayload>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError> {
        let client: Arc<dyn HarnessSupportClient> = server_api;
        Ok(Box::new(CodexHarnessRunner::new(
            self.cli_agent().command_prefix(),
            prompt,
            system_prompt,
            client,
            terminal_driver,
        )?))
    }
}

/// Build the shell command that launches the codex TUI.
///
/// `--full-auto` runs without per-action confirmation; the prompt is piped via cat so codex
/// reads it as the first user turn before continuing in interactive mode.
fn codex_command(cli_name: &str, prompt_path: &str, system_prompt_path: Option<&str>) -> String {
    let mut cmd = format!("{cli_name} --full-auto");
    if let Some(sys_path) = system_prompt_path {
        cmd.push_str(&format!(" --instructions \"$(cat '{sys_path}')\""));
    }
    cmd.push_str(&format!(" \"$(cat '{prompt_path}')\""));
    cmd
}

enum CodexRunnerState {
    Preexec,
    Running {
        conversation_id: AIConversationId,
        block_id: BlockId,
    },
}

struct CodexHarnessRunner {
    command: String,
    _temp_prompt_file: tempfile::NamedTempFile,
    _temp_system_prompt_file: Option<tempfile::NamedTempFile>,
    client: Arc<dyn HarnessSupportClient>,
    terminal_driver: ModelHandle<TerminalDriver>,
    state: Mutex<CodexRunnerState>,
}

impl CodexHarnessRunner {
    fn new(
        cli_command: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        client: Arc<dyn HarnessSupportClient>,
        terminal_driver: ModelHandle<TerminalDriver>,
    ) -> Result<Self, AgentDriverError> {
        let prompt_file = write_temp_file("oz_prompt_", prompt)?;
        let prompt_path = prompt_file.path().display().to_string();

        let system_prompt_file = system_prompt
            .map(|sp| write_temp_file("oz_system_", sp))
            .transpose()?;
        let system_prompt_path = system_prompt_file
            .as_ref()
            .map(|f| f.path().display().to_string());

        let command = codex_command(cli_command, &prompt_path, system_prompt_path.as_deref());

        Ok(Self {
            command,
            _temp_prompt_file: prompt_file,
            _temp_system_prompt_file: system_prompt_file,
            client,
            terminal_driver,
            state: Mutex::new(CodexRunnerState::Preexec),
        })
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl HarnessRunner for CodexHarnessRunner {
    async fn start(
        &self,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<CommandHandle, AgentDriverError> {
        let conversation_id = self
            .client
            .create_external_conversation(CODEX_CLI_FORMAT)
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

        *self.state.lock() = CodexRunnerState::Running {
            conversation_id,
            block_id: command_handle.block_id().clone(),
        };

        Ok(command_handle)
    }

    async fn exit(&self, foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        log::info!("Sending /quit to Codex CLI");
        let terminal_driver = self.terminal_driver.clone();
        foreground
            .spawn(move |_, ctx| {
                terminal_driver.update(ctx, |driver, ctx| {
                    driver.send_text_to_cli(CODEX_EXIT_COMMAND.to_string(), ctx);
                });
            })
            .await
            .map_err(|_| anyhow::anyhow!("Agent driver dropped while sending /quit"))
    }

    async fn save_conversation(
        &self,
        save_point: SavePoint,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<()> {
        if matches!(save_point, SavePoint::Periodic)
            && !super::has_running_cli_agent(&self.terminal_driver, foreground).await
        {
            log::debug!("Will not save conversation, Codex not in progress");
            return Ok(());
        }

        let (conversation_id, block_id) = match &*self.state.lock() {
            CodexRunnerState::Preexec => {
                log::warn!("save_conversation called before start");
                return Ok(());
            }
            CodexRunnerState::Running {
                conversation_id,
                block_id,
            } => (*conversation_id, block_id.clone()),
        };

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
