use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use tempfile::NamedTempFile;
use warp_cli::agent::Harness;
use warp_managed_secrets::ManagedSecretValue;
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

/// Format slug sent to the server when creating a Codex conversation.
const CODEX_CLI_FORMAT: &str = "codex_cli";
/// Slash command Codex's TUI recognises as a graceful shutdown.
const CODEX_EXIT_COMMAND: &str = "/exit";

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ThirdPartyHarness for CodexHarness {
    fn harness(&self) -> Harness {
        Harness::Codex
    }

    fn cli_agent(&self) -> CLIAgent {
        CLIAgent::Codex
    }

    fn install_docs_url(&self) -> Option<&'static str> {
        Some("https://developers.openai.com/codex/cli")
    }

    fn prepare_environment_config(
        &self,
        _working_dir: &Path,
        system_prompt: Option<&str>,
        _secrets: &HashMap<String, ManagedSecretValue>,
    ) -> Result<(), AgentDriverError> {
        prepare_codex_environment_config(system_prompt).map_err(|error| {
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
        _resumption_prompt: Option<&str>,
        working_dir: &Path,
        _task_id: Option<AmbientAgentTaskId>,
        server_api: Arc<ServerApi>,
        terminal_driver: ModelHandle<TerminalDriver>,
        _resume: Option<ResumePayload>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError> {
        // TODO(REMOTE-1503): support resume for Codex.
        let client: Arc<dyn HarnessSupportClient> = server_api;
        Ok(Box::new(CodexHarnessRunner::new(
            self.cli_agent().command_prefix(),
            prompt,
            system_prompt,
            working_dir,
            client,
            terminal_driver,
        )?))
    }
}

/// Build the shell command that launches the Codex TUI.
///
/// `--dangerously-bypass-approvals-and-sandbox` disables both the sandbox and approval
/// prompts so the agent can run autonomously.
fn codex_command(cli_name: &str, prompt_path: &str) -> String {
    format!("{cli_name} --dangerously-bypass-approvals-and-sandbox \"$(cat '{prompt_path}')\"")
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
    /// Held so the temp file is cleaned up when the runner is dropped.
    _temp_prompt_file: NamedTempFile,
    client: Arc<dyn HarnessSupportClient>,
    terminal_driver: ModelHandle<TerminalDriver>,
    state: Mutex<CodexRunnerState>,
}

impl CodexHarnessRunner {
    fn new(
        cli_command: &str,
        prompt: &str,
        _system_prompt: Option<&str>,
        _working_dir: &Path,
        client: Arc<dyn HarnessSupportClient>,
        terminal_driver: ModelHandle<TerminalDriver>,
    ) -> Result<Self, AgentDriverError> {
        let temp_file = write_temp_file("oz_prompt_", prompt)?;
        let prompt_path = temp_file.path().display().to_string();

        Ok(Self {
            command: codex_command(cli_command, &prompt_path),
            _temp_prompt_file: temp_file,
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
        log::info!("Sending /exit to Codex CLI");
        let terminal_driver = self.terminal_driver.clone();
        foreground
            .spawn(move |_, ctx| {
                terminal_driver.update(ctx, |driver, ctx| {
                    driver.send_text_to_cli(CODEX_EXIT_COMMAND.to_string(), ctx);
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

        // TODO(REMOTE-1504) Also save the conversation transcript.
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

const CODEX_CONFIG_DIR: &str = ".codex";
const CODEX_AGENTS_OVERRIDE_FILE_NAME: &str = "AGENTS.override.md";

fn prepare_codex_environment_config(system_prompt: Option<&str>) -> Result<()> {
    let Some(prompt) = system_prompt else {
        return Ok(());
    };
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
    write_codex_agents_override(&home_dir.join(CODEX_CONFIG_DIR), prompt)
}

fn write_codex_agents_override(codex_dir: &Path, system_prompt: &str) -> Result<()> {
    fs::create_dir_all(codex_dir).with_context(|| {
        format!(
            "Failed to create Codex config dir at {}",
            codex_dir.display()
        )
    })?;

    // Note: this currently works because we are only doing this for cloud agents; if we enable
    // this for local runs we'll want to make sure we don't clobber any existing file overrides.
    let prompt_path = codex_dir.join(CODEX_AGENTS_OVERRIDE_FILE_NAME);
    fs::write(&prompt_path, system_prompt).with_context(|| {
        format!(
            "Failed to write Codex system prompt to {}",
            prompt_path.display()
        )
    })
}
