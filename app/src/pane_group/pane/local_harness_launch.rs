use std::{collections::HashMap, ffi::OsString, path::PathBuf, sync::Arc};

use crate::ai::{
    agent::conversation::LocalClaudeHarnessMetadata,
    agent_sdk::{
        driver::{
            harness::{
                claude_code::{prepare_claude_environment_config, CLAUDE_CODE_FORMAT},
                harness_kind, harness_model_env_vars, HarnessKind,
            },
            AgentDriverError,
        },
        task_env_vars, validate_cli_installed,
    },
    ambient_agents::{task::HarnessConfig, AgentConfigSnapshot, AmbientAgentTaskId},
};
use crate::server::server_api::ai::AIClient;
use crate::server::server_api::harness_support::HarnessSupportClient;
use crate::terminal::cli_agent_sessions::plugin_manager::plugin_manager_for;
use crate::terminal::shell::ShellType;
use shell_words::quote as shell_quote;
use uuid::Uuid;
use warp_cli::agent::Harness;

#[derive(Clone)]
pub(super) struct PreparedLocalHarnessLaunch {
    pub command: String,
    pub env_vars: HashMap<OsString, OsString>,
    pub run_id: String,
    pub task_id: AmbientAgentTaskId,
    pub local_claude_harness_metadata: Option<LocalClaudeHarnessMetadata>,
}
pub(super) struct LocalHarnessChildLaunchRequest {
    pub prompt: String,
    pub harness_type: String,
    pub model_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub shell_type: Option<ShellType>,
    pub startup_directory: Option<PathBuf>,
    pub ai_client: Arc<dyn AIClient>,
    pub harness_support_client: Arc<dyn HarnessSupportClient>,
}

pub(super) fn normalize_local_child_harness(harness_type: &str) -> Option<Harness> {
    Harness::parse_local_child_harness(harness_type)
}

pub(super) fn validate_local_harness_shell(shell_type: Option<ShellType>) -> Result<(), String> {
    match shell_type {
        Some(ShellType::Bash) | Some(ShellType::Zsh) | Some(ShellType::Fish) => Ok(()),
        Some(ShellType::PowerShell) => Err(
            "Local child harnesses currently require bash, zsh, or fish; PowerShell is not supported."
                .to_string(),
        ),
        None => Err(
            "Local child harnesses currently require a detected bash, zsh, or fish session."
                .to_string(),
        ),
    }
}

pub(super) fn build_local_claude_child_command(prompt: &str, session_id: Uuid) -> String {
    let quoted_prompt = shell_quote(prompt);
    // Local child harness panes are launched off-screen. We intentionally skip
    // Claude's own permission prompts here so the child can start unattended
    // instead of hanging on an approval UI the user cannot see in that hidden
    // pane.
    format!("claude --session-id {session_id} --dangerously-skip-permissions {quoted_prompt}")
}

pub(super) fn build_local_opencode_child_command(prompt: &str) -> String {
    let quoted_prompt = shell_quote(prompt);
    format!("opencode --prompt {quoted_prompt}")
}
pub(super) fn build_local_codex_child_command(prompt: &str) -> String {
    let quoted_prompt = shell_quote(prompt);
    format!("codex --dangerously-bypass-approvals-and-sandbox {quoted_prompt}")
}

fn local_child_task_config(harness: Harness) -> Option<AgentConfigSnapshot> {
    match harness {
        Harness::Oz | Harness::Unknown => None,
        Harness::Claude | Harness::OpenCode | Harness::Gemini | Harness::Codex => {
            Some(AgentConfigSnapshot {
                harness: Some(HarnessConfig::from_harness_type(harness)),
                ..Default::default()
            })
        }
    }
}

pub(super) async fn prepare_local_harness_child_launch(
    request: LocalHarnessChildLaunchRequest,
) -> Result<PreparedLocalHarnessLaunch, String> {
    let LocalHarnessChildLaunchRequest {
        prompt,
        harness_type,
        model_id,
        parent_run_id,
        shell_type,
        startup_directory,
        ai_client,
        harness_support_client,
    } = request;
    let Some(harness) = normalize_local_child_harness(&harness_type) else {
        let harness_name = harness_type.trim();
        return Err(if harness_name.is_empty() {
            "Local child harness type is missing.".to_string()
        } else {
            format!("Unsupported local child harness '{harness_name}'.")
        });
    };
    validate_local_harness_shell(shell_type)?;
    let mut local_claude_harness_metadata = None;
    let command = match harness {
        Harness::Oz => unreachable!("normalize_local_child_harness filters out Oz"),
        Harness::Unknown => unreachable!("normalize_local_child_harness filters out Unknown"),
        Harness::Claude => {
            let working_dir = startup_directory
                .or_else(|| std::env::current_dir().ok())
                .ok_or_else(|| {
                    format!(
                        "Could not resolve a working directory for the local {} child.",
                        harness.display_name()
                    )
                })?;
            let HarnessKind::ThirdParty(third_party_harness) =
                harness_kind(harness).map_err(|error: AgentDriverError| error.to_string())?
            else {
                unreachable!("Claude resolves to a third-party harness")
            };
            third_party_harness
                .validate()
                .map_err(|error: AgentDriverError| error.to_string())?;
            // Local child harness panes inherit the user's existing local
            // auth/session state. We still prepare harness config files here,
            // but there are no Warp-managed secrets to materialize into the
            // hidden child pane.
            prepare_claude_environment_config(&working_dir, &HashMap::new())
                .map_err(|error| error.to_string())?;
            if let Some(manager) = plugin_manager_for(third_party_harness.cli_agent()) {
                if let Err(error) = manager.install().await {
                    log::warn!("Claude plugin installation failed for child harness: {error}");
                }
                if let Err(error) = manager.install_platform_plugin().await {
                    log::warn!(
                        "Claude platform plugin installation failed for child harness: {error}"
                    );
                }
            }

            let conversation_id = harness_support_client
                .create_external_conversation(CLAUDE_CODE_FORMAT)
                .await
                .map_err(|error| {
                    format!("Failed to create local Claude child conversation: {error}")
                })?;
            let session_id = Uuid::new_v4();
            local_claude_harness_metadata = Some(LocalClaudeHarnessMetadata {
                conversation_id,
                session_id,
                working_dir,
            });
            build_local_claude_child_command(&prompt, session_id)
        }
        Harness::Codex => {
            let HarnessKind::ThirdParty(third_party_harness) =
                harness_kind(harness).map_err(|error: AgentDriverError| error.to_string())?
            else {
                unreachable!("Codex resolves to a third-party harness")
            };
            third_party_harness
                .validate()
                .map_err(|error: AgentDriverError| error.to_string())?;

            // Local Codex child panes must rely on the user's existing local
            // auth/session state. Do not run the shared Codex environment prep
            // here: it can seed OPENAI_API_KEY into ~/.codex/auth.json and
            // rewrite ~/.codex/config.toml for the whole machine.
            build_local_codex_child_command(&prompt)
        }
        Harness::OpenCode => {
            validate_cli_installed("opencode", Some("https://opencode.ai/docs"))
                .map_err(|error: AgentDriverError| error.to_string())?;
            build_local_opencode_child_command(&prompt)
        }
        Harness::Gemini => unreachable!("normalize_local_child_harness filters out Gemini"),
    };

    let task_id = ai_client
        .create_agent_task(
            prompt.clone(),
            None,
            parent_run_id.clone(),
            local_child_task_config(harness),
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to create local {} child task: {error}",
                harness.display_name()
            )
        })?;
    if let Some(metadata) = local_claude_harness_metadata.as_ref() {
        ai_client
            .update_agent_task(
                task_id,
                None,
                None,
                Some(metadata.conversation_id.to_string()),
                None,
            )
            .await
            .map_err(|error| {
                format!("Failed to link local Claude child task to conversation: {error}")
            })?;
    }

    let mut env_vars = task_env_vars(Some(&task_id), parent_run_id.as_deref(), harness);
    // Propagate the selected model to Claude Code via ANTHROPIC_MODEL.
    // Codex local children never receive a model override — the UI
    // ensures model_id is empty for local Codex.
    env_vars.extend(harness_model_env_vars(harness, model_id.as_deref()));

    Ok(PreparedLocalHarnessLaunch {
        command,
        env_vars,
        run_id: task_id.to_string(),
        task_id,
        local_claude_harness_metadata,
    })
}

#[cfg(test)]
#[path = "local_harness_launch_tests.rs"]
mod tests;
