//! Agent SDK entry points for invoking Agent-related functionality from the app.
//! For now this provides a simple runner that echoes the received command.

use std::fmt::Write;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent_sdk::driver::harness::{harness_kind, HarnessKind};
use crate::ai::agent_sdk::driver::{AgentDriverOptions, AgentRunPrompt, Task};
use crate::ai::agent_sdk::mcp_config::build_mcp_servers_from_specs;
#[cfg(not(target_family = "wasm"))]
use crate::ai::aws_credentials::refresh_aws_credentials;
use crate::ai::llms::LLMId;
use crate::auth::{AuthManager, AuthManagerEvent, OwnerType};
use crate::cloud_object::model::persistence::ObjectStoreModel;
use crate::workflows::workflow::Workflow;
use ai::api_keys::{ApiKeyManager, AwsCredentialsRefreshStrategy};
use anyhow::Context;
use warp_cli::{
    agent::{AgentCommand, AgentProfileCommand, OutputFormat},
    artifact::ArtifactCommand,
    harness_support::{HarnessSupportCommand, ReportArtifactCommand, TaskStatus},
    integration::IntegrationCommand,
    mcp::MCPCommand,
    model::ModelCommand,
    provider::ProviderCommand,
    CliCommand, GlobalOptions,
};
use warp_core::features::FeatureFlag;
use warp_isolation_platform::IsolationPlatformError;
#[cfg(not(target_family = "wasm"))]
use warp_logging::log_file_path;
use warp_managed_secrets::ManagedSecretManager;
use warpui::ModelSpawner;
use warpui::{platform::TerminationMode, AppContext, SingletonEntity};

use crate::{
    ai::agent_sdk::harness_support_client::{DisabledHarnessSupportClient, HarnessSupportClient},
    ai::ambient_agents::task::HarnessConfig,
    ai::ambient_agents::AgentConfigSnapshot,
    auth::AuthStateProvider,
    send_telemetry_sync_from_app_ctx,
};
use driver::AgentDriverError;

use crate::ai::skills::{
    clone_repo_for_skill, resolve_skill_spec, ResolveSkillError, ResolvedSkill,
};

pub(crate) use driver::harness::{
    task_env_vars, validate_cli_installed, ClaudeHarness, ThirdPartyHarness,
};
pub use driver::AgentDriver;
use telemetry::CliTelemetryEvent;
use warp_cli::agent::{Harness, Prompt, RunAgentArgs};

mod admin;
mod artifact;
mod common;
mod config_file;
pub(crate) mod driver;
mod harness_support;
pub(crate) mod harness_support_client;
mod mcp;
mod mcp_config;
mod model;
pub mod output;
mod profiles;
mod provider;
pub(crate) mod retry;
mod telemetry;
#[cfg(test)]
mod test_support;
mod text_layout;

/// Prints a non-blocking warning to stderr when the CLI is invoked with a team-scoped API key.
fn maybe_warn_team_api_key(ctx: &AppContext) {
    let auth_state = AuthStateProvider::handle(ctx).as_ref(ctx).get();
    let owner_type = auth_state.api_key_owner_type();
    if !matches!(owner_type, Some(OwnerType::Team)) {
        return;
    }

    eprintln!(
        "\x1b[33mWarning: Free cloud credits apply to personal runs only but this run uses \
         a team API key. If you want to use free cloud credits, consider using a personal API key instead.\x1b[0m"
    );
}

/// Run a Warp CLI command.
pub fn run(
    ctx: &mut AppContext,
    command: CliCommand,
    global_options: GlobalOptions,
) -> anyhow::Result<()> {
    let event = command_to_telemetry_event(&command);
    send_telemetry_sync_from_app_ctx!(event, ctx);

    launch_command(ctx, command, global_options)
}

/// Dispatch a CLI command to its handler.
fn dispatch_command(
    ctx: &mut AppContext,
    command: CliCommand,
    global_options: GlobalOptions,
) -> anyhow::Result<()> {
    match command {
        CliCommand::Agent(agent_cmd) => run_agent(ctx, global_options, agent_cmd),
        CliCommand::MCP(mcp_cmd) => mcp::run(ctx, global_options, mcp_cmd),
        CliCommand::Model(model_cmd) => model::run(ctx, global_options, model_cmd),
        CliCommand::Login => admin::login(ctx),
        CliCommand::Whoami => admin::whoami(ctx, global_options.output_format),
        CliCommand::Provider(provider_cmd) => {
            if !FeatureFlag::ProviderCommand.is_enabled() {
                return Err(anyhow::anyhow!("invalid value 'provider'"));
            }
            provider::run(ctx, global_options, provider_cmd)
        }
        #[cfg(not(target_family = "wasm"))]
        CliCommand::Integration(_integration_cmd) => {
            // OpenWarp:云端 Simple Integration CRUD 已下线,CLI 子命令直接报错。
            return Err(anyhow::anyhow!("Cloud integrations disabled in OpenWarp"));
        }
        #[cfg(target_family = "wasm")]
        CliCommand::Integration(_) => {
            return Err(anyhow::anyhow!("invalid value 'integration'"));
        }
        CliCommand::HarnessSupport(args) => {
            if !FeatureFlag::AgentHarness.is_enabled() {
                return Err(anyhow::anyhow!("invalid value 'harness-support'"));
            }
            harness_support::run(ctx, global_options, args)
        }
        CliCommand::Artifact(artifact_cmd) => {
            if !FeatureFlag::ArtifactCommand.is_enabled() {
                return Err(anyhow::anyhow!("invalid value 'artifact'"));
            }
            artifact::run(ctx, global_options, artifact_cmd)
        }
    }
}

fn format_skill_resolution_error(err: ResolveSkillError) -> String {
    match err {
        ResolveSkillError::NotFound { skill } => {
            format!("Skill '{skill}' not found")
        }
        ResolveSkillError::RepoNotFound { repo } => {
            format!("Repository '{repo}' not found")
        }
        ResolveSkillError::Ambiguous { skill, candidates } => {
            let mut msg = format!(
                "Skill '{skill}' is ambiguous; specify as repo:skill_name\n\nCandidates:\n"
            );
            for path in candidates {
                msg.push_str(&format!("- {}\n", path.display()));
            }
            msg
        }
        ResolveSkillError::OrgMismatch {
            repo,
            expected,
            found,
        } => {
            format!("Repository '{repo}' found but belongs to org '{found}', expected '{expected}'")
        }
        ResolveSkillError::ParseFailed { path, message } => {
            format!("Failed to parse skill file {}: {message}", path.display())
        }
        ResolveSkillError::CloneFailed { org, repo, message } => {
            format!("Failed to clone repository '{org}/{repo}': {message}")
        }
    }
}

/// Run the agent with the provided command.
fn run_agent(
    ctx: &mut AppContext,
    global_options: GlobalOptions,
    command: AgentCommand,
) -> anyhow::Result<()> {
    match command {
        AgentCommand::Run(args) => {
            if args.conversation.is_some() {
                return Err(anyhow::anyhow!(
                    "unexpected argument '--conversation' found"
                ));
            }
            if args.skill.is_some() && !FeatureFlag::OzPlatformSkills.is_enabled() {
                return Err(anyhow::anyhow!("unexpected argument '--skill' found"));
            }
            if args.harness != Harness::Oz && !FeatureFlag::AgentHarness.is_enabled() {
                return Err(anyhow::anyhow!("unexpected argument '--harness' found"));
            }
            if args.harness == Harness::OpenCode {
                return Err(anyhow::anyhow!(
                    "The opencode harness is only supported for local child agent launches."
                ));
            }

            // Start the agent driver runner, which will handle the rest of the setup steps
            // (managing both sync and async steps) as well as triggering the driver.
            let runner = ctx.add_singleton_model(|_| AgentDriverRunner);
            runner.update(ctx, move |_, ctx| {
                let spawner = ctx.spawner();
                ctx.spawn(
                    AgentDriverRunner::setup_and_run_driver(
                        spawner,
                        args,
                        global_options.output_format,
                    ),
                    |_, result, _ctx| {
                        if let Err(e) = result {
                            report_fatal_error(e.into(), _ctx);
                        }
                    },
                );
            });

            Ok(())
        }
        AgentCommand::Profile(sub) => profiles::run(ctx, global_options, sub),
        AgentCommand::List(_) => Err(anyhow::anyhow!(
            "Cloud agent skill listing is disabled in OpenWarp"
        )),
    }
}

/// Build the merged agent configuration from all sources and the Task for the driver.
/// Merge precedence: file < CLI < skill
fn build_merged_config_and_task(
    args: &RunAgentArgs,
    resolved_skill: &Option<ResolvedSkill>,
    prompt: &Option<Prompt>,
    ctx: &mut AppContext,
) -> anyhow::Result<(AgentConfigSnapshot, Task)> {
    // Server-side prompt resolution (task_id is set): the task config already lives on the
    // server and individual CLI flags (--model, --mcp, etc.) are the only local overrides.
    // No config file is involved — the worker never passes --file alongside --task-id.
    if args.task_id.is_some() {
        return build_server_side_task(args, resolved_skill, ctx);
    }

    let loaded_file = match args.config_file.file.as_deref() {
        Some(path) => Some(config_file::load_config_file(path)?),
        None => None,
    };

    let cli_mcp_servers = build_mcp_servers_from_specs(&args.all_mcp_specs())?;

    // Merge precedence: file < CLI < skill
    let file_merged = config_file::merge_with_precedence(loaded_file.as_ref(), Default::default());

    // Skill provides base_prompt and optionally name
    let (skill_name, runtime_base_prompt) = match resolved_skill {
        Some(skill) => (Some(skill.name.clone()), Some(skill.instructions.clone())),
        None => (None, None),
    };

    let harness_override = (args.harness != Harness::Oz).then_some(HarnessConfig {
        harness_type: args.harness,
    });

    let mut merged_config = AgentConfigSnapshot {
        // CLI name > skill name > file name
        name: args.name.clone().or(skill_name).or(file_merged.name),
        environment_id: None,
        model_id: args.model.model.clone().or(file_merged.model_id),
        // Skill base_prompt takes precedence over file base_prompt
        base_prompt: runtime_base_prompt.clone().or(file_merged.base_prompt),
        mcp_servers: config_file::merge_mcp_servers(file_merged.mcp_servers, cli_mcp_servers),
        profile_id: args.profile.clone(),
        worker_host: file_merged.worker_host,
        skill_spec: file_merged.skill_spec,
        computer_use_enabled: args
            .computer_use
            .computer_use_override()
            .or(file_merged.computer_use_enabled),
        harness: harness_override,
        harness_auth_secrets: None,
    };

    let runtime_mcp_specs = match merged_config.mcp_servers.as_ref() {
        Some(mcp_servers) => config_file::mcp_specs_from_mcp_servers(mcp_servers)?,
        None => Vec::new(),
    };

    let model_override: Option<LLMId> = merged_config
        .model_id
        .as_deref()
        .map(|model_id| common::validate_agent_mode_base_model_id(model_id, ctx))
        .transpose()?;

    // Keep the task config snapshot aligned with the effective model selection.
    merged_config.model_id = model_override.clone().map(|id| id.to_string());

    // Combine base_prompt with user prompt locally.
    let local_prompt = match (merged_config.base_prompt.as_deref(), prompt) {
        (Some(base_prompt), Some(Prompt::PlainText(user_prompt))) => {
            Prompt::PlainText(format!("{base_prompt}\n\n{user_prompt}"))
        }
        (Some(base_prompt), None) => {
            // Skill-only invocation: use skill instructions as the prompt
            Prompt::PlainText(base_prompt.to_string())
        }
        (_, Some(p)) => p.clone(),
        (None, None) => {
            return Err(anyhow::anyhow!(AgentDriverError::InvalidRuntimeState));
        }
    };

    let task = Task {
        prompt: AgentRunPrompt::Local(resolve_prompt(&local_prompt, ctx)?),
        model: model_override,
        profile: args.profile.clone(),
        mcp_specs: runtime_mcp_specs,
        harness: harness_kind(args.harness)?,
    };

    Ok((merged_config, task))
}

/// Build the task for server-side prompt resolution (task_id is set).
/// Only CLI args contribute — no config file merge needed.
fn build_server_side_task(
    args: &RunAgentArgs,
    resolved_skill: &Option<ResolvedSkill>,
    ctx: &mut AppContext,
) -> anyhow::Result<(AgentConfigSnapshot, Task)> {
    let cli_mcp_servers = build_mcp_servers_from_specs(&args.all_mcp_specs())?;

    let runtime_mcp_specs = match cli_mcp_servers.as_ref() {
        Some(mcp_servers) => config_file::mcp_specs_from_mcp_servers(mcp_servers)?,
        None => Vec::new(),
    };

    let model_override: Option<LLMId> = args
        .model
        .model
        .as_deref()
        .map(|model_id| common::validate_agent_mode_base_model_id(model_id, ctx))
        .transpose()?;

    let harness_override = (args.harness != Harness::Oz).then_some(HarnessConfig {
        harness_type: args.harness,
    });

    let skill_name = resolved_skill.as_ref().map(|s| s.name.clone());
    let model_id_string = model_override.as_ref().map(|id| id.to_string());
    let profile = args.profile.clone();
    let config = AgentConfigSnapshot {
        name: args.name.clone().or(skill_name),
        environment_id: None,
        model_id: model_id_string,
        base_prompt: None,
        mcp_servers: cli_mcp_servers,
        profile_id: profile.clone(),
        worker_host: None,
        skill_spec: None,
        computer_use_enabled: args.computer_use.computer_use_override(),
        harness: harness_override,
        harness_auth_secrets: None,
    };

    let skill = resolved_skill.as_ref().map(|s| s.parsed_skill.clone());

    let task = Task {
        prompt: AgentRunPrompt::ServerSide {
            skill,
            attachments_dir: None,
        },
        model: model_override,
        profile,
        mcp_specs: runtime_mcp_specs,
        harness: harness_kind(args.harness)?,
    };

    Ok((config, task))
}

/// Resolve a `Prompt` to a plain string.
fn resolve_prompt(prompt: &Prompt, ctx: &AppContext) -> Result<String, AgentDriverError> {
    match prompt {
        Prompt::PlainText(prompt_str) => Ok(prompt_str.to_string()),
        Prompt::SavedPrompt(workflow_id) => {
            let Some(workflow) = ObjectStoreModel::as_ref(ctx).get_workflow_by_uid(workflow_id)
            else {
                return Err(AgentDriverError::AIWorkflowNotFound(workflow_id.to_owned()));
            };

            let Workflow::AgentMode { query, .. } = &workflow.model().data else {
                return Err(AgentDriverError::AIWorkflowNotFound(workflow_id.to_owned()));
            };
            Ok(query.to_owned())
        }
    }
}

/// Singleton model that provides a ModelContext for spawning async operations
/// when starting the agent driver. This is needed because conversation fetching
/// requires spawning an async task, which requires a ModelContext.
struct AgentDriverRunner;

impl warpui::Entity for AgentDriverRunner {
    type Event = ();
}

impl warpui::SingletonEntity for AgentDriverRunner {}

impl AgentDriverRunner {
    async fn setup_and_run_driver(
        foreground: ModelSpawner<Self>,
        args: RunAgentArgs,
        output_format: OutputFormat,
    ) -> Result<(), AgentDriverError> {
        // Ensure we've synced team state before starting the driver.
        Self::refresh_team_metadata(&foreground).await?;

        // Wait for Warp Drive to sync before building the task config, since
        // prompt resolution (SavedPrompt -> workflow lookup) depends on it.
        if foreground
            .spawn(|_, ctx| common::refresh_warp_drive(ctx))
            .await?
            .await
            .is_err()
        {
            return Err(AgentDriverError::WarpDriveSyncFailed);
        }

        let result: Result<(), AgentDriverError> = async {
            // Pull relevant variables out of args before moving it into the closure.
            let bedrock_inference_role = args.bedrock_inference_role.clone();
            let args_harness = args.harness;

            // `--conversation` path (user-invoked local resume): validate before any task side
            // effects so mismatches fail fast. The `--task-id` path derives its conversation id
            // from the server-side task metadata inside `build_driver_options_and_task`. Both
            // can currently be passed together (the worker server-side appends `--conversation`
            // alongside `--task-id` for Slack/Linear followups); when both are set, the explicit
            // `--conversation` value wins via the merge below.
            if let Some(conversation_id) = args.conversation.as_deref() {
                common::fetch_and_validate_conversation_harness(
                    conversation_id,
                    args_harness,
                )
                .await?;
            }
            let resume_conversation_id = args.conversation.clone();

            let (mut driver_options, task, task_conversation_id) =
                Self::build_driver_options_and_task(&foreground, args).await?;

            let resume_conversation_id = resume_conversation_id.or(task_conversation_id);

            let bedrock_task_id = driver_options.task_id.map(|id| id.to_string());

            #[cfg(not(target_family = "wasm"))]
            if let Some(role_arn) = bedrock_inference_role {
                // Set the OIDC strategy on the UI thread and kick off the refresh; the
                // returned future resolves when credentials are committed to the model.
                let refresh_future = foreground
                    .spawn(move |_, ctx| {
                        ApiKeyManager::handle(ctx).update(ctx, |manager, ctx| {
                            // From here on, refresh credentials via OIDC federation only.
                            manager.set_aws_credentials_refresh_strategy(
                                AwsCredentialsRefreshStrategy::OidcManaged {
                                    task_id: bedrock_task_id,
                                    role_arn,
                                },
                            );
                            refresh_aws_credentials(manager, ctx)
                        })
                    })
                    .await?;

                refresh_future
                    .await
                    .map_err(AgentDriverError::AwsBedrockCredentialsFailed)?;
            }

            match &task.harness {
                HarnessKind::Unsupported(harness) => {
                    return Err(AgentDriverError::HarnessSetupFailed {
                        harness: harness.to_string(),
                        reason: format!(
                            "The {harness} harness is only supported for local child agent launches."
                        ),
                    });
                }
                HarnessKind::Oz | HarnessKind::ThirdParty(_) => {}
            }

            // Validate that the third-party harness is installed and authed.
            if let HarnessKind::ThirdParty(harness) = &task.harness {
                harness.validate()?;
            }

            if let Some(task_id) = driver_options.task_id {
                driver::write_run_started(&task_id.to_string(), output_format);
            }

            // Pull conversation information, if we have it
            if let Some(conversation_id) = resume_conversation_id {
                driver_options.resume =
                    Self::load_conversation_information(conversation_id, &task.harness).await?;
            }

            // Run the driver
            foreground
                .spawn(move |_, ctx| {
                    Self::create_and_run_driver(
                        ctx,
                        driver_options,
                        output_format,
                        task,
                    );
                })
                .await?;

            Ok(())
        }
        .await;

        result
    }

    async fn refresh_team_metadata(
        foreground: &ModelSpawner<Self>,
    ) -> Result<(), AgentDriverError> {
        foreground
            .spawn(
                |_, ctx| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
                    Box::pin(common::refresh_workspace_metadata(ctx))
                },
            )
            .await?
            .await
            .map_err(|_| AgentDriverError::TeamMetadataRefreshTimeout)
    }

    /// Resolve the skill spec from args, if one was provided.
    ///
    /// In sandboxed mode with a fully-qualified spec (org + repo), the repo is
    /// cloned first since it may not exist locally. Otherwise we resolve directly
    /// against the local filesystem.
    async fn resolve_skill(
        foreground: &ModelSpawner<Self>,
        args: &RunAgentArgs,
        working_dir: &Path,
    ) -> Result<Option<ResolvedSkill>, AgentDriverError> {
        if !FeatureFlag::OzPlatformSkills.is_enabled() {
            return Ok(None);
        }
        let Some(skill_spec) = args.skill.clone() else {
            return Ok(None);
        };

        // In sandboxed mode with a fully-qualified spec, clone the repo first.
        let needs_clone = args.sandboxed && skill_spec.org.is_some() && skill_spec.repo.is_some();
        if needs_clone {
            let org = skill_spec.org.as_ref().expect("org checked above");
            let repo_name = skill_spec.repo.as_ref().expect("repo checked above");
            log::info!("Cloning {org}/{repo_name} for skill resolution in sandboxed mode");
            clone_repo_for_skill(org, repo_name, working_dir)
                .await
                .map_err(|err| {
                    AgentDriverError::SkillResolutionFailed(format_skill_resolution_error(err))
                })?;
        }

        let working_dir_buf = working_dir.to_path_buf();
        let skill = foreground
            .spawn(move |_, ctx| resolve_skill_spec(&skill_spec, &working_dir_buf, ctx))
            .await?
            .map_err(|err| {
                AgentDriverError::SkillResolutionFailed(format_skill_resolution_error(err))
            })?;
        log::debug!(
            "Resolved skill '{}' from {}",
            skill.name,
            skill.skill_path.display()
        );
        Ok(Some(skill))
    }

    /// Build the AgentDriverOptions and Task, handling task creation or existing task setup.
    ///
    /// The third tuple element is the conversation id read off the server-side task metadata
    /// on the `--task-id` branch. It's `None` when no task id was passed or when the task is
    /// not linked to a conversation; callers use it to drive `--task-id`-implied resume
    /// without requiring the caller to also pass `--conversation`.
    async fn build_driver_options_and_task(
        foreground: &ModelSpawner<Self>,
        args: RunAgentArgs,
    ) -> Result<(AgentDriverOptions, Task, Option<String>), AgentDriverError> {
        // Get the working directory
        let working_dir = match args.cwd.as_ref() {
            Some(dir) => dunce::canonicalize(dir)
                .with_context(|| format!("Unable to resolve {}", dir.display())),
            None => std::env::current_dir().context("Unable to determine working directory"),
        }
        .map_err(AgentDriverError::ConfigBuildFailed)?;

        // Resolve the skill, if we have one
        let resolved_skill = Self::resolve_skill(foreground, &args, &working_dir).await?;

        // Extract variables we want to use later before moving args into the closure
        let task_id_str = args.task_id.clone();
        let prompt = args.prompt_arg.to_prompt();

        // Build the AgentConfigSnapshot, Task, and AgentDriverOptions
        let prompt_clone = prompt.clone();
        let (task, mut driver_options) = foreground
            .spawn(move |_, ctx| -> anyhow::Result<_> {
                let task =
                    build_merged_config_and_task(&args, &resolved_skill, &prompt_clone, ctx)?.1;

                let task_id = args.task_id.as_ref().and_then(|s| s.parse().ok());
                let should_share = false;

                let driver_options = driver::AgentDriverOptions {
                    working_dir: working_dir.clone(),
                    task_id,
                    parent_run_id: None,
                    should_share,
                    idle_on_complete: args.idle_on_complete.map(|d| d.into()),
                    secrets: Default::default(),
                    resume: None,
                    selected_harness: args.harness,
                };

                Ok((task, driver_options))
            })
            .await?
            .map_err(AgentDriverError::ConfigBuildFailed)?;

        // 既有 task 拉取 secrets / task metadata,新 run 走本地 task 初始化。
        // 既有 task 分支还会返回 task 的 `conversation_id`,让调用方不需要额外
        // `--conversation` 参数也能接上恢复逻辑。
        let task_conversation_id = if let Some(task_id_str) = task_id_str {
            Self::fetch_secrets_and_task_metadata(foreground, task_id_str, &mut driver_options)
                .await?
        } else {
            Self::initialize_new_task(&mut driver_options).await?;
            None
        };

        Ok((driver_options, task, task_conversation_id))
    }

    /// Creates local driver task state for a new agent run.
    ///
    /// OpenWarp(本地化,Phase 3b-2):原实现调 `server_api.create_agent_task` 在云端创建
    /// ambient agent task,获取服务端 task_id 后在后续请求中携带。本地化后:
    ///   - 不发 GraphQL `create_agent_task` mutation
    ///   - `driver_options.task_id` 保持 `None`
    ///   - 不再写入 ServerApiProvider ambient header 上下文(云端请求路径已删除)
    /// 下游所有 `if let Some(task_id) = driver_options.task_id` 分支自动跳过。
    /// BYOP 本地 harness 运行不依赖该 task_id,根据 `harness/` 代码路径仅在服务端
    /// 汇报状态时使用。
    async fn initialize_new_task(
        driver_options: &mut AgentDriverOptions,
    ) -> Result<(), AgentDriverError> {
        driver_options.task_id = None;
        Ok(())
    }

    /// 从既有 task_id 启动 agent run 时,仅拉取本地可用 secrets 并更新 driver options。
    ///
    /// OpenWarp 不再拉取云端 task metadata,因此不会从 task 自动恢复云端 conversation。
    async fn fetch_secrets_and_task_metadata(
        foreground: &ModelSpawner<Self>,
        task_id_str: String,
        driver_options: &mut AgentDriverOptions,
    ) -> Result<Option<String>, AgentDriverError> {
        let task_secrets = foreground
            .spawn({
                let task_id_str = task_id_str.clone();
                move |_, ctx| {
                    ManagedSecretManager::handle(ctx)
                        .as_ref(ctx)
                        .get_task_secrets(task_id_str)
                }
            })
            .await?;

        let parsed_task_id = match task_id_str.parse() {
            Ok(id) => Some(id),
            Err(e) => {
                log::error!("Failed to parse task ID: {e}");
                None
            }
        };

        let secrets = match task_secrets.await {
            Ok(secrets) => secrets,
            Err(err) => {
                // Ignore errors due to running in a non-isolated environment.
                // Otherwise, fail fast - we should not start the driver without secrets
                // in an environment where they should be available.
                if err
                    .downcast_ref::<IsolationPlatformError>()
                    .is_some_and(|err| {
                        matches!(err, IsolationPlatformError::NoIsolationPlatformDetected)
                    })
                {
                    Default::default()
                } else {
                    return Err(AgentDriverError::SecretsFetchFailed(err));
                }
            }
        };
        let parent_run_id = None;
        let task_conversation_id = None;

        driver_options.task_id = parsed_task_id;
        driver_options.parent_run_id = parent_run_id;
        driver_options.secrets = secrets;

        Ok(task_conversation_id)
    }

    /// If we are starting this agent run from an existing conversation, load the conversation
    /// data from the server and return the harness-specific [`ResumeOptions`] payload that the
    /// caller plugs onto [`AgentDriverOptions::resume`].
    ///
    /// `harness` is the resolved harness from the task config (already validated against the
    /// conversation's metadata up-front by [`common::fetch_and_validate_conversation_harness`]).
    ///
    /// For the Oz harness, fetches the full conversation and returns a [`driver::ResumeOptions::Oz`].
    /// For third-party harnesses, delegates to [`ThirdPartyHarness::fetch_resume_payload`] and
    /// wraps the returned payload (if any) in [`driver::ResumeOptions::ThirdParty`]; each harness
    /// owns its server call and error mapping. Returns `None` if a third-party harness has no
    /// resume payload to surface.
    async fn load_conversation_information(
        conversation_id: String,
        harness: &HarnessKind,
    ) -> Result<Option<driver::ResumeOptions>, AgentDriverError> {
        match harness {
            HarnessKind::Oz => {
                // CloudConversations was removed in OpenWarp; we can no longer
                // resume an Oz conversation from a server-stored token.
                Err(AgentDriverError::ConversationLoadFailed(format!(
                    "Conversation {conversation_id} cannot be resumed: cloud conversations are disabled in OpenWarp"
                )))
            }
            HarnessKind::ThirdParty(h) => {
                let harness_support_client: std::sync::Arc<dyn HarnessSupportClient> =
                    std::sync::Arc::new(DisabledHarnessSupportClient::new());
                let resume_conversation_id = AIConversationId::try_from(conversation_id.clone())
                    .map_err(|err| AgentDriverError::ConversationLoadFailed(format!("{err:#}")))?;
                Ok(
                    h.fetch_resume_payload(&resume_conversation_id, harness_support_client)
                        .await?
                        .map(|payload| driver::ResumeOptions::ThirdParty(Box::new(payload))),
                )
            }
            HarnessKind::Unsupported(harness) => Err(AgentDriverError::HarnessSetupFailed {
                harness: harness.to_string(),
                reason: format!(
                    "The {harness} harness is only supported for local child agent launches."
                ),
            }),
        }
    }

    /// Create the AgentDriver and start running the task.
    fn create_and_run_driver(
        ctx: &mut AppContext,
        driver_options: driver::AgentDriverOptions,
        output_format: OutputFormat,
        task: driver::Task,
    ) {
        maybe_warn_team_api_key(ctx);

        // Initializing the driver will fail if not logged in. Since we check that above, panic here - it's difficult to
        // fallibly instantiate a UI framework model.
        let driver = ctx.add_singleton_model(|ctx| {
            AgentDriver::new(driver_options, ctx).expect("Could not initialize driver")
        });

        driver.update(ctx, |driver, ctx| {
            driver.set_output_format(output_format);
            let agent_future = driver.run(task, ctx);

            ctx.spawn(agent_future, |_, result, ctx| match result {
                Ok(()) => {
                    ctx.terminate_app(TerminationMode::ForceTerminate, None);
                }
                Err(err) => {
                    report_fatal_error(err.into(), ctx);
                }
            });
        });
    }
}

/// Returns `true` if the given CLI command requires authentication.
fn command_requires_auth(command: &CliCommand) -> bool {
    match command {
        CliCommand::Agent(agent_cmd) => match agent_cmd {
            AgentCommand::Run { .. } => true,
            AgentCommand::Profile(sub) => match sub {
                AgentProfileCommand::List => true,
            },
            AgentCommand::List(_) => true,
        },
        CliCommand::MCP(mcp_cmd) => match mcp_cmd {
            MCPCommand::List => true,
        },
        CliCommand::Model(model_cmd) => match model_cmd {
            ModelCommand::List => true,
        },
        CliCommand::Login => false,
        CliCommand::Whoami => true,
        CliCommand::Provider(_) => true,
        CliCommand::Integration(_) => true,
        CliCommand::HarnessSupport(_) => true,
        CliCommand::Artifact(artifact_cmd) => match artifact_cmd {
            ArtifactCommand::Upload(_) | ArtifactCommand::Get(_) | ArtifactCommand::Download(_) => {
                false
            }
        },
    }
}

/// Launch a CLI command, checking authentication first if needed.
///
/// If auth is not required, dispatches the command immediately.
/// If auth is required and the user is logged in, triggers a user refresh
/// before launching the command.
fn launch_command(
    ctx: &mut AppContext,
    command: CliCommand,
    global_options: GlobalOptions,
) -> anyhow::Result<()> {
    let requires_auth = command_requires_auth(&command);

    if !requires_auth {
        return dispatch_command(ctx, command, global_options);
    }

    let cli_name = warp_cli::binary_name().unwrap_or_else(|| "warp".to_string());

    let auth_state = AuthStateProvider::handle(ctx).as_ref(ctx).get();
    if !auth_state.is_logged_in() {
        return Err(anyhow::anyhow!(
            "You are not logged in - please log in with `{cli_name} login` to continue."
        ));
    }

    // User is logged in — subscribe to auth events, trigger a refresh, and wait
    // for the result before running the command.
    let mut dispatched = false;
    ctx.subscribe_to_model(&AuthManager::handle(ctx), move |_, event, ctx| {
        if dispatched {
            return;
        }
        match event {
            AuthManagerEvent::AuthComplete => {
                dispatched = true;
                if let Err(err) = dispatch_command(ctx, command.clone(), global_options.clone()) {
                    report_fatal_error(err, ctx);
                }
            }
            AuthManagerEvent::NeedsReauth => {
                dispatched = true;
                let auth_state = AuthStateProvider::handle(ctx).as_ref(ctx).get();
                let message = if auth_state.is_api_key_authenticated() {
                    "Your API key is invalid. Please provide a valid key via '--api-key' or the WARP_API_KEY environment variable.".to_string()
                } else {
                    format!("Your credentials are invalid. Please log in again with `{cli_name} login`.")
                };
                report_fatal_error(anyhow::anyhow!(message), ctx);
            }
            AuthManagerEvent::AuthFailed(err) => {
                dispatched = true;
                report_fatal_error(anyhow::anyhow!("Authentication failed: {err:#}"), ctx);
            }
            _ => {}
        }
    });

    // Trigger the user refresh - the subscription above will handle the result.
    AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
        auth_manager.refresh_user(ctx);
    });

    Ok(())
}

/// Check if we're running within Warp (for example, if this is an invocation of the Warp CLI
/// within a Warp terminal session).
pub fn is_running_in_warp() -> bool {
    std::env::var("TERM_PROGRAM")
        .map(|v| v == "WarpTerminal")
        .unwrap_or(false)
}

/// Report a fatal error and terminate the app.
fn report_fatal_error(err: anyhow::Error, ctx: &mut AppContext) {
    let mut message = err.to_string();
    for cause in err.chain().skip(1) {
        let _ = write!(&mut message, "\n=> {cause}");
    }

    #[cfg(not(target_family = "wasm"))]
    {
        if let Ok(path) = log_file_path() {
            let _ = write!(
                message,
                "\n\nFor more information, check Warp logs at {}",
                path.display()
            );
        }
    }

    let error = anyhow::anyhow!(message);
    ctx.terminate_app(TerminationMode::ForceTerminate, Some(Err(error)));
}

/// Map each CLI command into a telemetry event to emit when it's executed.
fn command_to_telemetry_event(command: &CliCommand) -> CliTelemetryEvent {
    match command {
        CliCommand::Agent(AgentCommand::Run(args)) => CliTelemetryEvent::AgentRun {
            gui: args.gui,
            requested_mcp_servers: args.mcp_specs.len() + args.mcp_servers.len(),
            has_environment: false,
            task_id: args.task_id.clone(),
            harness: args.harness.to_string(),
        },
        CliCommand::Agent(AgentCommand::Profile(sub)) => match sub {
            AgentProfileCommand::List => CliTelemetryEvent::AgentProfileList,
        },
        CliCommand::Agent(AgentCommand::List(_)) => CliTelemetryEvent::AgentList,
        CliCommand::MCP(MCPCommand::List) => CliTelemetryEvent::MCPList,
        CliCommand::Model(ModelCommand::List) => CliTelemetryEvent::ModelList,
        CliCommand::Login => CliTelemetryEvent::Login,
        CliCommand::Whoami => CliTelemetryEvent::Whoami,
        CliCommand::Provider(ProviderCommand::Setup(_)) => CliTelemetryEvent::ProviderSetup,
        CliCommand::Provider(ProviderCommand::List) => CliTelemetryEvent::ProviderList,
        CliCommand::Integration(integration_cmd) => match integration_cmd {
            IntegrationCommand::Create(_) => CliTelemetryEvent::IntegrationCreate,
            IntegrationCommand::Update(_) => CliTelemetryEvent::IntegrationUpdate,
            IntegrationCommand::List => CliTelemetryEvent::IntegrationList,
        },
        CliCommand::HarnessSupport(args) => match &args.command {
            HarnessSupportCommand::Ping => CliTelemetryEvent::HarnessSupportPing,
            HarnessSupportCommand::ReportArtifact(report_args) => match &report_args.command {
                ReportArtifactCommand::PullRequest(_) => {
                    CliTelemetryEvent::HarnessSupportReportArtifact {
                        artifact_type: "pull_request",
                    }
                }
            },
            HarnessSupportCommand::NotifyUser(_) => CliTelemetryEvent::HarnessSupportNotifyUser,
            HarnessSupportCommand::FinishTask(finish_args) => {
                CliTelemetryEvent::HarnessSupportFinishTask {
                    success: finish_args.status == TaskStatus::Success,
                }
            }
        },
        CliCommand::Artifact(artifact_cmd) => match artifact_cmd {
            ArtifactCommand::Upload(_) => CliTelemetryEvent::ArtifactUpload,
            ArtifactCommand::Get(_) => CliTelemetryEvent::ArtifactGet,
            ArtifactCommand::Download(_) => CliTelemetryEvent::ArtifactDownload,
        },
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
