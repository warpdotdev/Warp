use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tempfile::NamedTempFile;
use warp_cli::agent::Harness;
use warp_managed_secrets::ManagedSecretValue;
use warpui::{ModelHandle, ModelSpawner, SingletonEntity};

use crate::ai::agent_events::AgentEventStreamClient;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::terminal::cli_agent_sessions::{CLIAgentSessionStatus, CLIAgentSessionsModel};
use crate::terminal::CLIAgent;
use crate::util::path::resolve_executable;
use warp_cli::{OZ_CLI_ENV, OZ_HARNESS_ENV, OZ_PARENT_RUN_ID_ENV, OZ_RUN_ID_ENV};
use warp_core::channel::ChannelState;

use super::terminal::{CommandHandle, TerminalDriver};
use super::{
    AgentDriver, AgentDriverError, LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV,
    LEGACY_OZ_PARENT_STATE_ROOT_ENV, OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV,
    OZ_MESSAGE_LISTENER_STATE_ROOT_ENV,
};

mod claude_code;
mod gemini;
mod json_utils;

pub(crate) use claude_code::ClaudeHarness;
use gemini::GeminiHarness;

/// Trait for third-party agent harnesses that execute prompts via their own CLIs.
///
/// Each new external harness (e.g. Claude, Codex) implements this to be used with local agent runs.
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub(crate) trait ThirdPartyHarness: Send + Sync {
    /// Returns the [`Harness`] variant this implementation corresponds to.
    fn harness(&self) -> Harness;

    /// Returns the CLIAgent type associated with this harness.
    fn cli_agent(&self) -> CLIAgent;

    /// URL to install instructions for this harness's CLI, surfaced in the
    /// default [`validate`] impl when the CLI is not on `PATH`.
    fn install_docs_url(&self) -> Option<&'static str> {
        None
    }

    /// Validate that the harness is ready to run. Default impl checks that the
    /// CLI is installed on `PATH`; override for additional checks.
    fn validate(&self) -> Result<(), AgentDriverError> {
        validate_cli_installed(self.cli_agent().command_prefix(), self.install_docs_url())
    }

    /// Prepare CLI-specific config files before launching the harness command.
    fn prepare_environment_config(
        &self,
        _working_dir: &Path,
        _system_prompt: Option<&str>,
        _secrets: &HashMap<String, ManagedSecretValue>,
    ) -> Result<(), AgentDriverError> {
        Ok(())
    }

    /// Build a runner for executing this harness with the given prompt.
    #[allow(clippy::too_many_arguments)]
    fn build_runner(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        resumption_prompt: Option<&str>,
        working_dir: &Path,
        task_id: Option<AmbientAgentTaskId>,
        agent_event_stream_client: Arc<dyn AgentEventStreamClient>,
        terminal_driver: ModelHandle<TerminalDriver>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError>;
}

/// Harness type for driver dispatch.
pub(crate) enum HarnessKind {
    Oz,
    /// Third-party CLI-backed harness (e.g. Claude, Gemini).
    ThirdParty(Box<dyn ThirdPartyHarness>),
    /// Harnesses that exist in the shared CLI enum but are not supported by the
    /// standalone agent driver.
    Unsupported(Harness),
}

impl HarnessKind {
    /// Corresponding [`Harness`] enum value.
    pub(crate) fn harness(&self) -> Harness {
        match self {
            HarnessKind::Oz => Harness::Oz,
            HarnessKind::ThirdParty(h) => h.harness(),
            HarnessKind::Unsupported(harness) => *harness,
        }
    }
}

impl fmt::Debug for HarnessKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use the `Display` method on the [`Harness`] enum.
        write!(f, "{}", self.harness())
    }
}

/// Build a [`HarnessKind`] for the given [`Harness`].
///
/// We shouldn't ever get a `--harness unknown` here because clap should handle
/// it.
pub(crate) fn harness_kind(harness: Harness) -> Result<HarnessKind, AgentDriverError> {
    match harness {
        Harness::Oz => Ok(HarnessKind::Oz),
        Harness::Claude => Ok(HarnessKind::ThirdParty(Box::new(ClaudeHarness))),
        Harness::OpenCode => Ok(HarnessKind::Unsupported(Harness::OpenCode)),
        Harness::Gemini => Ok(HarnessKind::ThirdParty(Box::new(GeminiHarness))),
        Harness::Unknown => Err(AgentDriverError::InvalidRuntimeState),
    }
}

/// Check that `cli` is installed and on PATH, returning a `HarnessSetupFailed`
/// error with an optional install-docs link when it isn't.
pub(crate) fn validate_cli_installed(
    cli: &str,
    install_docs_url: Option<&str>,
) -> Result<(), AgentDriverError> {
    if resolve_executable(cli).is_none() {
        let mut reason = format!("'{cli}' CLI not found on your machine.");
        if let Some(url) = install_docs_url {
            reason.push_str(&format!(" Install it first: {url}"));
        }
        return Err(AgentDriverError::HarnessSetupFailed {
            harness: cli.into(),
            reason,
        });
    }
    Ok(())
}

fn insert_non_empty_task_env_var(
    env_vars: &mut HashMap<OsString, OsString>,
    key: &'static str,
    value: String,
) {
    if value.is_empty() {
        return;
    }

    env_vars.insert(OsString::from(key), OsString::from(value));
}

fn insert_task_env_var_aliases(
    env_vars: &mut HashMap<OsString, OsString>,
    keys: &[&'static str],
    value: &str,
) {
    for key in keys {
        env_vars.insert(OsString::from(key), OsString::from(value));
    }
}

fn message_listener_state_root() -> Option<String> {
    [
        OZ_MESSAGE_LISTENER_STATE_ROOT_ENV,
        LEGACY_OZ_PARENT_STATE_ROOT_ENV,
    ]
    .into_iter()
    .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
}

fn task_env_vars_for_harness_name(
    task_id: Option<&AmbientAgentTaskId>,
    parent_run_id: Option<&str>,
    selected_harness: Harness,
) -> HashMap<OsString, OsString> {
    let mut env_vars = HashMap::with_capacity(7);

    if let Some(id) = task_id {
        env_vars.insert(
            OsString::from(OZ_RUN_ID_ENV),
            OsString::from(id.to_string()),
        );
    }

    if let Some(parent_run_id) = parent_run_id.filter(|id| !id.is_empty()) {
        env_vars.insert(
            OsString::from(OZ_PARENT_RUN_ID_ENV),
            OsString::from(parent_run_id),
        );
    }

    env_vars.insert(
        OsString::from(OZ_CLI_ENV),
        OsString::from(
            std::env::current_exe()
                .unwrap_or_else(|_| ChannelState::channel().cli_command_name().into()),
        ),
    );
    // `OZ_HARNESS` is only consumed by child orchestration telemetry when the child
    // CLI emits `run message *` events.
    env_vars.insert(
        OsString::from(OZ_HARNESS_ENV),
        OsString::from(selected_harness.to_string()),
    );
    if selected_harness == Harness::Claude && task_id.is_some() {
        insert_task_env_var_aliases(
            &mut env_vars,
            &[
                OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV,
                LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV,
            ],
            "1",
        );
        if let Some(state_root) = message_listener_state_root() {
            insert_task_env_var_aliases(
                &mut env_vars,
                &[
                    OZ_MESSAGE_LISTENER_STATE_ROOT_ENV,
                    LEGACY_OZ_PARENT_STATE_ROOT_ENV,
                ],
                &state_root,
            );
        }
    }
    // Server URL overrides are disabled on release channels, so there's no
    // override to propagate to child processes there.
    env_vars
}

pub(crate) fn task_env_vars(
    task_id: Option<&AmbientAgentTaskId>,
    parent_run_id: Option<&str>,
    selected_harness: Harness,
) -> HashMap<OsString, OsString> {
    task_env_vars_for_harness_name(task_id, parent_run_id, selected_harness)
}

/// Indicates when the harness conversation is being saved.
/// Implementations may use this to customize the saved data, such as
/// recording additional metadata on completion.
pub(crate) enum SavePoint {
    /// A periodic auto-save to minimize data loss.
    Periodic,
    /// The final save of conversation state, after the harness has completed.
    Final,
    /// A save after the harness reports it finished an agent turn.
    PostTurn,
}

/// Stateful per-run representation of an external harness produced
/// by [`ThirdPartyHarness::build_runner`].
///
/// All `HarnessRunner` methods take `&self` as a parameter, but may mutate internal
/// state. There are no `&mut self` methods, as this would require that the `AgentDriver`
/// store the runner in a mutex and lock it across `await` points.
///
/// The driver uses this to manage the lifecycle of a particular third-party harness.
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub(crate) trait HarnessRunner: Send + Sync {
    /// Create local conversation state and start the harness command in the terminal.
    ///
    /// Returns a [`CommandHandle`] that resolves to the exit code. The runner
    /// stores the conversation ID and block ID internally for use in
    /// [`save_conversation`].
    async fn start(
        &self,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<CommandHandle, AgentDriverError>;

    /// Save the current conversation state.
    async fn save_conversation(
        &self,
        save_point: SavePoint,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<()>;

    /// Gracefully ask the harness to exit.
    async fn exit(&self, foreground: &ModelSpawner<AgentDriver>) -> Result<()>;
    /// Handle a CLI session update such as a prompt submit or completed tool use.
    async fn handle_session_update(&self, _foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        Ok(())
    }

    /// Clean up any harness-owned background state after the harness exits.
    async fn cleanup(&self, _foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        Ok(())
    }
}

/// Returns `true` if the terminal tracked by `terminal_driver` has a CLI agent session
/// that is currently in progress.
pub(crate) async fn has_running_cli_agent(
    terminal_driver: &ModelHandle<TerminalDriver>,
    foreground: &ModelSpawner<AgentDriver>,
) -> bool {
    let driver = terminal_driver.clone();
    let Ok(running) = foreground
        .spawn(move |_, ctx| {
            let terminal_view_id = driver.as_ref(ctx).terminal_view().id();
            CLIAgentSessionsModel::handle(ctx)
                .as_ref(ctx)
                .session(terminal_view_id)
                .is_some_and(|s| s.status == CLIAgentSessionStatus::InProgress)
        })
        .await
    else {
        return false;
    };
    running
}

/// Create a [`NamedTempFile`] with the given prefix and write `content` into it.
///
/// Used by third-party harnesses to stage prompts / system prompts on disk
/// before launching the CLI, avoiding shell-quoting issues with complex input.
pub(super) fn write_temp_file(
    prefix: &str,
    content: &str,
) -> Result<NamedTempFile, AgentDriverError> {
    let mut file = tempfile::Builder::new()
        .prefix(prefix)
        .suffix(".txt")
        .tempfile()
        .map_err(|e| {
            AgentDriverError::ConfigBuildFailed(anyhow::anyhow!(
                "Failed to create temp file '{prefix}': {e}"
            ))
        })?;
    file.write_all(content.as_bytes()).map_err(|e| {
        AgentDriverError::ConfigBuildFailed(anyhow::anyhow!(
            "Failed to write temp file '{prefix}': {e}"
        ))
    })?;
    Ok(file)
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
