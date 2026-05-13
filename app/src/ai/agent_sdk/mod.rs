//! Agent SDK entry points for invoking Agent-related functionality from the app.
//! For now this provides a simple runner that echoes the received command.

use std::fmt::Write;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

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
    mcp::MCPCommand,
    model::ModelCommand,
    CliCommand, GlobalOptions,
};
use warp_core::features::FeatureFlag;
#[cfg(not(target_family = "wasm"))]
use warp_logging::log_file_path;
use warpui::ModelSpawner;
use warpui::{platform::TerminationMode, AppContext, SingletonEntity};

use crate::{
    ai::ambient_agents::task::HarnessConfig,
    ai::ambient_agents::AgentConfigSnapshot,
    auth::AuthStateProvider,
};
use driver::AgentDriverError;

use crate::ai::skills::{
    clone_repo_for_skill, resolve_skill_spec, ResolveSkillError, ResolvedSkill,
};

pub(crate) use driver::harness::{
    task_env_vars, validate_cli_installed, ClaudeHarness, ThirdPartyHarness,
};
pub use driver::AgentDriver;
use warp_cli::agent::{Harness, Prompt, RunAgentArgs};

mod admin;
mod common;
mod config_file;
pub(crate) mod driver;
mod mcp;
mod mcp_config;
mod model;
pub mod output;
mod profiles;
mod provider;
pub(crate) mod retry;
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
        "\x1b[33mWarning: Personal credits apply to personal runs only but this run uses \
         a team API key. If you want to use personal credits, consider using a personal API key instead.\x1b[0m"
    );
}

/// Run a Warp CLI command.
pub fn run(
    ctx: &mut AppContext,
    command: CliCommand,
    global_options: GlobalOptions,
) -> anyhow::Result<()> {
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
            "Agent skill listing is disabled in OpenWarp"
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

            let (driver_options, task) =
                Self::build_driver_options_and_task(&foreground, args).await?;

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

    /// Build the AgentDriverOptions and Task, handling local task setup.
    async fn build_driver_options_and_task(
        foreground: &ModelSpawner<Self>,
        args: RunAgentArgs,
    ) -> Result<(AgentDriverOptions, Task), AgentDriverError> {
        // Get the working directory
        let working_dir = match args.cwd.as_ref() {
            Some(dir) => dunce::canonicalize(dir)
                .with_context(|| format!("Unable to resolve {}", dir.display())),
            None => std::env::current_dir().context("Unable to determine working directory"),
        }
        .map_err(AgentDriverError::ConfigBuildFailed)?;

        // Resolve the skill, if we have one
        let resolved_skill = Self::resolve_skill(foreground, &args, &working_dir).await?;

        let prompt = args.prompt_arg.to_prompt();

        // Build the AgentConfigSnapshot, Task, and AgentDriverOptions
        let prompt_clone = prompt.clone();
        let (task, mut driver_options) = foreground
            .spawn(move |_, ctx| -> anyhow::Result<_> {
                let task =
                    build_merged_config_and_task(&args, &resolved_skill, &prompt_clone, ctx)?.1;

                let should_share = false;

                let driver_options = driver::AgentDriverOptions {
                    working_dir: working_dir.clone(),
                    task_id: None,
                    parent_run_id: None,
                    should_share,
                    idle_on_complete: args.idle_on_complete.map(|d| d.into()),
                    secrets: Default::default(),
                    selected_harness: args.harness,
                };

                Ok((task, driver_options))
            })
            .await?
            .map_err(AgentDriverError::ConfigBuildFailed)?;

        Self::initialize_new_task(&mut driver_options).await?;

        Ok((driver_options, task))
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

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
