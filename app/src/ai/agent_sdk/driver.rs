use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::OsString,
    future::Future,
    io::{self, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::ai::llms::{LLMId, LLMPreferences};
use crate::ai::mcp::MCPServerState;

use crate::ai::agent_sdk::driver::harness::{
    task_env_vars, HarnessKind, HarnessRunner, SavePoint, ThirdPartyHarness,
};
use crate::terminal::cli_agent_sessions::plugin_manager::{
    plugin_manager_for, CliAgentPluginManager,
};
use crate::terminal::cli_agent_sessions::{
    CLIAgentSessionStatus, CLIAgentSessionsModel, CLIAgentSessionsModelEvent,
};
use crate::{
    ai::{
        agent::{
            AIAgentExchange, AIAgentInput, AIAgentOutput, CancellationReason, RenderableAIError,
        },
        agent_events::DisabledAgentEventStreamClient,
        ambient_agents::{
            conversation_output_status_from_conversation, AmbientAgentTaskId,
            AmbientConversationStatus,
        },
        blocklist::{
            agent_view::AgentViewEntryOrigin, BlocklistAIHistoryEvent, BlocklistAIHistoryModel,
            BlocklistAIPermissions,
        },
        execution_profiles::profiles::AIExecutionProfilesModel,
        mcp::{
            parsing::{normalize_mcp_json, ParsedTemplatableMCPServerResult},
            templatable_manager::TemplatableMCPServerManagerEvent,
            TemplatableMCPServerInstallation, TemplatableMCPServerManager,
        },
    },
    auth::AuthStateProvider,
    server::ids::{ServerId, SyncId},
};
use anyhow::Context as _;
use futures::{
    channel::oneshot,
    future::{self, Either},
    FutureExt as _,
};
use oneshot::{Canceled, Receiver, Sender};
use uuid::Uuid;
use warp_cli::agent::{Harness, OutputFormat};
use warp_cli::mcp::MCPSpec;
use warp_core::{features::FeatureFlag, report_if_error, safe_debug, safe_info};
use warp_managed_secrets::ManagedSecretValue;
use warpui::{
    r#async::{FutureExt, TimeoutError},
    Entity, ModelContext, ModelHandle, ModelSpawner, SingletonEntity,
};

pub(crate) mod harness;
pub(super) mod output;
pub(crate) mod terminal;

use terminal::TerminalDriverEvent;

const MCP_SERVER_STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const HARNESS_SAVE_INTERVAL: Duration = Duration::from_secs(30);
pub(crate) const WARP_DRIVE_SYNC_TIMEOUT: Duration = Duration::from_secs(60);
const SETUP_FAILED_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// Maximum time to wait for an automatic error resume before propagating the error.
/// If no follow-up status arrives within this window, the driver terminates with the
/// original error so the CLI does not hang indefinitely.
const AUTO_RESUME_TIMEOUT: Duration = Duration::from_secs(120);
/// Signals to Claude child-harness hooks that Warp already owns the background
/// message-listener lifecycle, so the plugin should reuse the shared state
/// files instead of spawning and cleaning up its own listener.
///
/// When this variable is absent, the Claude plugin falls back to its legacy
/// self-managed listener path so older Warp builds and standalone plugin
/// invocations keep working.
pub(crate) const OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV: &str =
    "OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY";
/// Optional root directory for the per-session Claude message-listener state
/// that Warp and the Claude hook scripts share.
pub(crate) const OZ_MESSAGE_LISTENER_STATE_ROOT_ENV: &str = "OZ_MESSAGE_LISTENER_STATE_ROOT";
// Keep exporting the legacy `OZ_PARENT_*` names to child hooks until the
// external Claude plugin has fully migrated to the canonical
// `OZ_MESSAGE_LISTENER_*` names.
const LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV: &str =
    "OZ_PARENT_LISTENER_MANAGED_EXTERNALLY";
const LEGACY_OZ_PARENT_STATE_ROOT_ENV: &str = "OZ_PARENT_STATE_ROOT";

/// IdleTimeoutSender is wrapper around a sender that signals when a run is done after
/// an idle timeout. Used for both Oz runs and third-party harnesses.
///
/// We use a generation-based approach to cancel timers instead of storing timer handles:
///
/// - `tx_cell` holds the completion sender; taking it ensures we only complete once.
/// - `timer_generation` starts at 0 and is incremented each time we want to cancel
///   existing timers and potentially start a new one. When a timer fires, it checks
///   if its generation still matches the current generation. If not, the timer was
///   "cancelled" by a newer timer and should not complete the conversation.
///
/// This approach avoids the complexity of storing and cancelling timer handles,
/// while allowing multiple events to safely race without double-completion.
struct IdleTimeoutSender<T: Send + 'static> {
    tx_cell: Arc<Mutex<Option<oneshot::Sender<T>>>>,
    generation: Arc<AtomicUsize>,
}

impl<T: Send + 'static> IdleTimeoutSender<T> {
    fn new(tx: oneshot::Sender<T>) -> Self {
        Self {
            tx_cell: Arc::new(Mutex::new(Some(tx))),
            generation: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// End the run by sending `value` immediately.
    fn end_run_now(&self, value: T) {
        if let Ok(mut guard) = self.tx_cell.lock() {
            if let Some(sender) = guard.take() {
                let _ = sender.send(value);
            }
        }
    }

    /// End the run after `timeout` by sending `value`, unless cancelled before then.
    fn end_run_after(&self, timeout: Duration, value: T) {
        // Increment the generation counter to invalidate any existing timers,
        // then capture the new generation for our timer to check against.
        let current_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let tx_cell = Arc::clone(&self.tx_cell);
        let generation = Arc::clone(&self.generation);

        // Spawn a background thread that will complete the oneshot after the idle timeout,
        // unless a follow-up query resets the timer (by bumping the generation counter).
        thread::spawn(move || {
            thread::sleep(timeout);

            // Check if our timer generation is still current. If not, a follow-up
            // query or other activity has "cancelled" this timer by bumping the generation.
            if generation.load(Ordering::SeqCst) != current_gen {
                return;
            }
            if let Ok(mut guard) = tx_cell.lock() {
                if let Some(sender) = guard.take() {
                    // Send the value after the idle timeout expires.
                    let _ = sender.send(value);
                }
            }
        });
    }

    /// Cancel any pending idle timers.
    fn cancel_idle_timeout(&self) {
        if self.generation.load(Ordering::SeqCst) > 0 {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// Options for initializing the agent driver.
pub struct AgentDriverOptions {
    /// Initial working directory for the agent's terminal session.
    pub working_dir: PathBuf,
    /// Secrets to inject into the agent's terminal session.
    pub secrets: HashMap<String, ManagedSecretValue>,
    /// ID of the task being executed.
    pub task_id: Option<AmbientAgentTaskId>,
    /// Parent run ID for child-agent flows, if this task was spawned by another run.
    pub parent_run_id: Option<String>,
    /// Whether the agent run should share its session.
    pub should_share: bool,
    /// How long to keep the session alive after the agent run completes, if at all.
    pub idle_on_complete: Option<Duration>,
    /// Selected execution harness for this run.
    pub selected_harness: Harness,
}

/// `AgentDriver` is a model for driving an ambient Warp agent to completion.
///
/// Its primary responsibility is to configure a headless terminal pane and execute an AI query within it.
pub struct AgentDriver {
    terminal_driver: ModelHandle<terminal::TerminalDriver>,
    working_dir: PathBuf,

    /// Secrets available to the running agent.
    /// - Secrets are injected as environment variables when the terminal session is created.
    /// - Secrets are passed to MCP servers during spawning.
    secrets: Arc<HashMap<String, ManagedSecretValue>>,

    output_format: OutputFormat,

    // The associated task ID for this agent run, if any.
    task_id: Option<AmbientAgentTaskId>,

    /// Harness adapter for the running agent. This is only set if:
    /// - The harness has started successfully.
    /// - We're using a third-party harness.
    /// In the future, we _may_ use the harness abstraction for the Oz agent as well.
    harness: Option<Arc<dyn HarnessRunner>>,

    // Optional idle timeout after completion. If set, the process will stay alive for follow-ups
    // and exit after this period of inactivity.
    idle_on_complete: Option<Duration>,
}

pub(crate) enum SDKConversationOutputStatus {
    Success,
    Error { error: RenderableAIError },
    Cancelled { reason: CancellationReason },
    Blocked { blocked_action: String },
}

impl SDKConversationOutputStatus {
    pub fn into_result(self) -> Result<(), AgentDriverError> {
        match self {
            SDKConversationOutputStatus::Success => Ok(()),
            SDKConversationOutputStatus::Error { error } => {
                Err(AgentDriverError::ConversationError { error })
            }
            // NOTE: this doesn't happen in the SDK (yet) because CTRL+C kills the whole program.
            SDKConversationOutputStatus::Cancelled { reason } => {
                Err(AgentDriverError::ConversationCancelled { reason })
            }
            SDKConversationOutputStatus::Blocked { blocked_action } => {
                Err(AgentDriverError::ConversationBlocked { blocked_action })
            }
        }
    }
}

/// Task configuration for running an agent.
#[derive(Debug)]
pub struct Task {
    /// The prompt for the agent.
    pub prompt: AgentRunPrompt,
    pub model: Option<LLMId>,
    /// ID of the profile to run as (SyncId string). If None, use the default profile.
    pub profile: Option<String>,
    /// MCP server specifications to start prior to execution.
    pub mcp_specs: Vec<MCPSpec>,
    /// Which harness to use for executing the agent run.
    pub harness: HarnessKind,
}

/// Prompt that we initialize an agent driver with.
#[derive(Debug, Clone)]
pub enum AgentRunPrompt {
    /// Prompt is provided locally (already resolved to a plain string).
    Local(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AgentDriverError {
    #[error("Terminal session is not available.")]
    TerminalUnavailable,
    #[error("Invalid runtime state - please file a bug report.")]
    InvalidRuntimeState,
    #[error("Requested MCP server not found: {0}")]
    MCPServerNotFound(uuid::Uuid),
    #[error("Failed to start MCP servers")]
    MCPStartupFailed,
    #[error("Failed to parse MCP server JSON: {0}")]
    MCPJsonParseError(String),
    #[error("MCP server configuration is missing required variables")]
    MCPMissingVariables,
    #[error("Agent profile \"{0}\" not found")]
    ProfileError(String),
    #[error("Local user state is unavailable. Restart OpenWarp and try again.")]
    NotLoggedIn,
    #[error("Saved prompt not found for id {0}")]
    AIWorkflowNotFound(String),
    #[error("Terminal bootstrap failed")]
    BootstrapFailed,
    #[error("Error syncing Warp Drive")]
    WarpDriveSyncFailed,
    #[error("Requested environment not found: {0}")]
    EnvironmentNotFound(String),
    #[error("Environment setup failed: {0}")]
    EnvironmentSetupFailed(String),

    #[error("Could not resolve working directory {}", path.display())]
    InvalidWorkingDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{error}")]
    ConversationError { error: RenderableAIError },
    #[error("Conversation was canceled: {reason}")]
    ConversationCancelled { reason: CancellationReason },
    #[error("The agent got stuck waiting for user confirmation on the action: {blocked_action}")]
    ConversationBlocked { blocked_action: String },
    #[error("Timed out refreshing team metadata")]
    TeamMetadataRefreshTimeout,
    #[error("{0}")]
    SkillResolutionFailed(String),
    #[error("Failed to build agent configuration")]
    ConfigBuildFailed(#[source] anyhow::Error),
    #[error("Failed to initialize AWS Bedrock credentials: {0}")]
    AwsBedrockCredentialsFailed(String),
    #[error("Harness command exited with code {exit_code}")]
    HarnessCommandFailed { exit_code: i32 },
    #[error("Harness '{harness}' setup failed: {reason}")]
    HarnessSetupFailed { harness: String, reason: String },
    #[error("Harness '{harness}' config setup failed")]
    HarnessConfigSetupFailed {
        harness: String,
        #[source]
        error: anyhow::Error,
    },
}

impl From<warpui::ModelDropped> for AgentDriverError {
    fn from(_: warpui::ModelDropped) -> Self {
        AgentDriverError::InvalidRuntimeState
    }
}

impl AgentDriver {
    pub fn new(
        options: AgentDriverOptions,
        ctx: &mut ModelContext<Self>,
    ) -> Result<Self, AgentDriverError> {
        let AgentDriverOptions {
            working_dir,
            task_id,
            parent_run_id,
            should_share,
            idle_on_complete,
            secrets,
            selected_harness,
        } = options;

        safe_info!(
            safe: ("Initializing agent driver: share={should_share}, idle_on_complete={idle_on_complete:?}"),
            full: (
                "Initializing agent driver: share={should_share}, idle_on_complete={idle_on_complete:?}, working_dir={}",
                working_dir.display()
            )
        );

        // OpenWarp 启动时会初始化本地用户;走到这里说明本地 auth singleton 未正确初始化。
        if !AuthStateProvider::as_ref(ctx).get().is_logged_in() {
            return Err(AgentDriverError::NotLoggedIn);
        }

        // Build environment variables from secrets for the terminal session.
        // Do not override env vars that are already set to a non-empty value in the current
        // process. This ensures that worker-injected credentials (e.g. harness auth secrets)
        // and user-provided env vars (e.g. on self-hosted workers) take precedence over
        // generic managed secrets.
        let mut env_vars = HashMap::with_capacity(secrets.len() + 1);
        for (name, secret) in &secrets {
            let (env_name, env_value) = match secret {
                ManagedSecretValue::RawValue { value } => (name.as_str(), value.as_str()),
                ManagedSecretValue::AnthropicApiKey { api_key } => {
                    ("ANTHROPIC_API_KEY", api_key.as_str())
                }
                ManagedSecretValue::AnthropicBedrockAccessKey {
                    aws_access_key_id,
                    aws_secret_access_key,
                    aws_session_token,
                    aws_region,
                } => {
                    // Inject env vars needed for Claude Code Bedrock access key authentication.
                    // AWS_SESSION_TOKEN is only injected when the user provided one (i.e. for
                    // temporary/STS credentials).
                    let mut vars = vec![
                        ("AWS_ACCESS_KEY_ID", aws_access_key_id.as_str()),
                        ("AWS_SECRET_ACCESS_KEY", aws_secret_access_key.as_str()),
                        ("CLAUDE_CODE_USE_BEDROCK", "1"),
                        ("AWS_REGION", aws_region.as_str()),
                    ];
                    if let Some(token) = aws_session_token.as_deref() {
                        vars.push(("AWS_SESSION_TOKEN", token));
                    }
                    for (env_name, env_value) in vars {
                        if std::env::var(env_name).is_ok_and(|v| !v.is_empty()) {
                            log::warn!(
                                "Skipping managed secret {env_name}: already set in environment"
                            );
                            continue;
                        }
                        env_vars.insert(OsString::from(env_name), OsString::from(env_value));
                    }
                    continue; // Skip the single-var insert below since we handled all vars inline.
                }
                ManagedSecretValue::AnthropicBedrockApiKey {
                    aws_bearer_token_bedrock,
                    aws_region,
                } => {
                    // Inject all three env vars needed for Claude Code Bedrock authentication.
                    let vars = [
                        (
                            "AWS_BEARER_TOKEN_BEDROCK",
                            aws_bearer_token_bedrock.as_str(),
                        ),
                        ("CLAUDE_CODE_USE_BEDROCK", "1"),
                        ("AWS_REGION", aws_region.as_str()),
                    ];
                    for (env_name, env_value) in vars {
                        if std::env::var(env_name).is_ok_and(|v| !v.is_empty()) {
                            log::warn!(
                                "Skipping managed secret {env_name}: already set in environment"
                            );
                            continue;
                        }
                        env_vars.insert(OsString::from(env_name), OsString::from(env_value));
                    }
                    continue; // Skip the single-var insert below since we handled all vars inline.
                }
            };
            if std::env::var(env_name).is_ok_and(|v| !v.is_empty()) {
                log::warn!("Skipping managed secret {env_name}: already set in environment");
                continue;
            }
            env_vars.insert(OsString::from(env_name), OsString::from(env_value));
        }

        env_vars.extend(task_env_vars(
            task_id.as_ref(),
            parent_run_id.as_deref(),
            selected_harness,
        ));

        // Signal to third-party harnesses (e.g. Claude Code) that we're in a sandbox
        // so they allow root execution with permissive flags.
        if warp_isolation_platform::detect().is_some() {
            env_vars.insert(OsString::from("IS_SANDBOX"), OsString::from("1"));
        }

        let terminal_driver = terminal::TerminalDriver::create(
            terminal::TerminalDriverOptions {
                working_dir: working_dir.clone(),
                env_vars,
                should_share,
                task_id,
            },
            ctx,
        )?;

        // Subscribe to TerminalDriver events for task-specific handling.
        ctx.subscribe_to_model(&terminal_driver, |me, event, _| {
            me.handle_terminal_driver_event(event);
        });

        Ok(Self {
            terminal_driver,
            working_dir,
            secrets: Arc::new(secrets),
            output_format: OutputFormat::default(),
            task_id,
            harness: None,
            idle_on_complete,
        })
    }

    pub fn set_output_format(&mut self, output_format: OutputFormat) {
        self.output_format = output_format;
    }

    pub fn run(
        &mut self,
        task: Task,
        ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = Result<(), AgentDriverError>> {
        let (tx, rx) = oneshot::channel();
        let foreground = ctx.spawner();
        let idle_on_complete = self.idle_on_complete;

        ctx.spawn(
            async move {
                let result = Self::run_internal(task, foreground.clone()).await;

                if tx.send(result).is_err() {
                    log::error!("Caller did not wait for agent driver to finish");
                }
            },
            |_, _, _| {},
        );

        let task_id = self.task_id;

        async move {
            if let Some(ref task_id) = task_id {
                log::info!("Executing task {task_id}");
            }

            let result = match rx.await {
                Ok(result) => result,
                Err(Canceled) => {
                    log::error!("Agent driver exited abruptly");
                    Err(AgentDriverError::InvalidRuntimeState)
                }
            };

            if let (Some(_task_id), Err(err)) = (task_id, &result) {
                // Keep the session alive after environment setup failures so
                // the viewer can connect, receive scrollback, and see the error.
                if let (Some(idle_timeout), true) = (
                    idle_on_complete,
                    matches!(err, AgentDriverError::EnvironmentSetupFailed(_)),
                ) {
                    let timeout = idle_timeout.min(SETUP_FAILED_IDLE_TIMEOUT);
                    log::info!("Environment setup failed; keeping session alive for {timeout:?}");
                    warpui::r#async::Timer::after(timeout).await;
                }
            }

            result
        }
    }

    /// Check that the working directory exists. Since it's user-specified, we don't automatically
    /// create the directory (in case they made a typo).
    fn check_working_dir(&self) -> impl Future<Output = Result<(), AgentDriverError>> {
        let working_dir = self.working_dir.clone();
        async move {
            match async_fs::metadata(&working_dir).await {
                Ok(metadata) => {
                    if metadata.is_dir() {
                        Ok(())
                    } else {
                        Err(AgentDriverError::InvalidWorkingDirectory {
                            path: working_dir.to_owned(),
                            source: io::ErrorKind::NotADirectory.into(),
                        })
                    }
                }
                Err(err) => Err(AgentDriverError::InvalidWorkingDirectory {
                    path: working_dir.to_owned(),
                    source: err,
                }),
            }
        }
    }

    /// Resolve MCP specs into UUIDs for existing servers and ephemeral installations for inline specs.
    ///
    /// Returns (existing_server_uuids, ephemeral_installations)
    fn resolve_mcp_specs(
        specs: &[MCPSpec],
    ) -> Result<(Vec<Uuid>, Vec<TemplatableMCPServerInstallation>), AgentDriverError> {
        let mut existing_uuids = Vec::new();
        let mut ephemeral_installations = Vec::new();

        for spec in specs {
            match spec {
                MCPSpec::Uuid(uuid) => {
                    existing_uuids.push(*uuid);
                }
                MCPSpec::Json(json_str) => {
                    // Normalize the JSON - if it's a single server definition (has command or url
                    // at top level), wrap it with a generated name.
                    let normalized_json = normalize_mcp_json(json_str)
                        .map_err(|e| AgentDriverError::MCPJsonParseError(e.to_string()))?;

                    // Parse as inline MCP server configuration
                    let parsed_results =
                        ParsedTemplatableMCPServerResult::from_user_json(&normalized_json)
                            .map_err(|e| AgentDriverError::MCPJsonParseError(e.to_string()))?;

                    for result in parsed_results {
                        let installation = result
                            .templatable_mcp_server_installation
                            .ok_or(AgentDriverError::MCPMissingVariables)?;
                        ephemeral_installations.push(installation);
                    }
                }
            }
        }

        Ok((existing_uuids, ephemeral_installations))
    }

    /// Start MCP servers from profile allowlist for the terminal.
    fn start_profile_mcp_servers(
        &self,
        ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = Result<(), AgentDriverError>> {
        let terminal_id = self.terminal_driver.as_ref(ctx).terminal_view().id();
        let permissions = BlocklistAIPermissions::as_ref(ctx);
        let profile_allowlist = permissions.get_mcp_allowlist(ctx, Some(terminal_id));

        if !profile_allowlist.is_empty() {
            log::info!(
                "Starting {} MCP servers allowlisted in profile",
                profile_allowlist.len()
            );
        }
        self.start_mcp_servers(&profile_allowlist, ctx)
    }

    fn get_mcp_servers_to_start(
        &self,
        uuids: &[uuid::Uuid],
        ctx: &mut ModelContext<Self>,
    ) -> Result<HashSet<Uuid>, AgentDriverError> {
        let templatable_mcp_manager = TemplatableMCPServerManager::handle(ctx);

        let mut servers_to_start: HashSet<Uuid> = HashSet::new();

        for uuid in uuids.iter() {
            if templatable_mcp_manager
                .as_ref(ctx)
                .is_server_active_or_pending(*uuid)
            {
                log::debug!("MCP server {uuid} is already active or pending; skipping");
                continue;
            } else if templatable_mcp_manager
                .as_ref(ctx)
                .get_installed_server(uuid)
                .is_some()
            {
                servers_to_start.insert(*uuid);
            } else {
                return Err(AgentDriverError::MCPServerNotFound(*uuid));
            }
        }

        Ok(servers_to_start)
    }

    fn subscribe_to_mcp_managers(
        &self,
        tx: Sender<Result<(), AgentDriverError>>,
        servers_to_start: HashSet<Uuid>,
        ctx: &mut ModelContext<Self>,
    ) {
        use std::rc::Rc;

        let templatable_mcp_manager = TemplatableMCPServerManager::handle(ctx);
        let mcp_to_start = Rc::new(RefCell::new(servers_to_start));
        let manager_clone = templatable_mcp_manager.clone();
        let mut tx = Some(tx);
        ctx.subscribe_to_model(
            &templatable_mcp_manager,
            move |_me, event, ctx| match event {
                TemplatableMCPServerManagerEvent::StateChanged { uuid, state } => {
                    let mut pending_ids = mcp_to_start.borrow_mut();
                    if !pending_ids.contains(uuid) {
                        return;
                    }
                    match state {
                        MCPServerState::Running => {
                            pending_ids.remove(uuid);
                            if pending_ids.is_empty() {
                                log::info!("All MCP servers started");
                                if let Some(sender) = tx.take() {
                                    let _ = sender.send(Ok(()));
                                }
                                ctx.unsubscribe_from_model(&manager_clone);
                            }
                        }
                        MCPServerState::FailedToStart => {
                            log::warn!("Failed to start MCP server {uuid}");
                            if let Some(sender) = tx.take() {
                                let _ = sender.send(Err(AgentDriverError::MCPStartupFailed));
                            }
                            ctx.unsubscribe_from_model(&manager_clone);
                        }
                        _ => {}
                    }
                }
                TemplatableMCPServerManagerEvent::ServerInstallationAdded(_)
                | TemplatableMCPServerManagerEvent::ServerInstallationDeleted(_)
                | TemplatableMCPServerManagerEvent::TemplatableMCPServersUpdated
                | TemplatableMCPServerManagerEvent::LegacyServerConverted => {}
            },
        );
    }

    fn spawn_inactive_servers(
        &self,
        servers_to_start: HashSet<Uuid>,
        ctx: &mut ModelContext<Self>,
    ) {
        let templatable_mcp_manager = TemplatableMCPServerManager::handle(ctx);
        templatable_mcp_manager.update(ctx, |manager, ctx| {
            for uuid in servers_to_start {
                manager.spawn_server(uuid, ctx);
            }
        });
    }

    fn start_mcp_servers(
        &self,
        uuids: &[uuid::Uuid],
        ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = Result<(), AgentDriverError>> {
        let (tx, rx) = oneshot::channel();
        let servers_to_start = match self.get_mcp_servers_to_start(uuids, ctx) {
            Ok(val) => val,
            Err(e) => {
                return Either::Right(future::ready(Err(e)));
            }
        };

        // If we don't need to start any servers, complete immediately.
        if servers_to_start.is_empty() {
            return Either::Right(future::ready(Ok(())));
        }

        log::info!("Starting {} MCP servers...", servers_to_start.len());

        self.subscribe_to_mcp_managers(tx, servers_to_start.clone(), ctx);

        self.spawn_inactive_servers(servers_to_start, ctx);

        Either::Left(async move {
            match rx.with_timeout(MCP_SERVER_STARTUP_TIMEOUT).await {
                Ok(Ok(result)) => result,
                Ok(Err(Canceled)) => {
                    log::error!("Subscription dropped before MCP servers started");
                    Err(AgentDriverError::InvalidRuntimeState)
                }
                Err(TimeoutError) => {
                    log::error!("Timed out waiting for MCP servers to start");
                    Err(AgentDriverError::MCPStartupFailed)
                }
            }
        })
    }

    /// Start ephemeral MCP servers from inline JSON specifications.
    /// These servers are not persisted and exist only for the duration of the agent run.
    fn start_ephemeral_mcp_servers(
        &self,
        mut installations: Vec<TemplatableMCPServerInstallation>,
        ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = Result<(), AgentDriverError>> {
        if installations.is_empty() {
            return Either::Right(future::ready(Ok(())));
        }

        // Inject secrets into the ephemeral MCP server installations.
        for installation in installations.iter_mut() {
            installation.apply_secrets(&self.secrets);
        }

        let (tx, rx) = oneshot::channel();
        let mut tx = Some(tx);
        let mut uuids_to_start: HashSet<Uuid> = installations.iter().map(|i| i.uuid()).collect();

        log::info!("Starting {} ephemeral MCP servers...", installations.len());

        // Subscribe to state changes for these ephemeral servers.
        let templatable_mcp_manager = TemplatableMCPServerManager::handle(ctx);
        let manager_clone = templatable_mcp_manager.clone();

        ctx.subscribe_to_model(&templatable_mcp_manager, move |_me, event, ctx| {
            if let TemplatableMCPServerManagerEvent::StateChanged { uuid, state } = event {
                if !uuids_to_start.contains(uuid) {
                    return;
                }
                match state {
                    MCPServerState::Running => {
                        uuids_to_start.remove(uuid);
                        if uuids_to_start.is_empty() {
                            log::info!("All ephemeral MCP servers started");
                            if let Some(sender) = tx.take() {
                                let _ = sender.send(Ok(()));
                            }
                            ctx.unsubscribe_from_model(&manager_clone);
                        }
                    }
                    MCPServerState::FailedToStart => {
                        log::warn!("Failed to start ephemeral MCP server {uuid}");
                        if let Some(sender) = tx.take() {
                            let _ = sender.send(Err(AgentDriverError::MCPStartupFailed));
                        }
                        ctx.unsubscribe_from_model(&manager_clone);
                    }
                    _ => {}
                }
            }
        });

        // Spawn the ephemeral servers.
        templatable_mcp_manager.update(ctx, move |manager, ctx| {
            for installation in installations {
                manager.spawn_cli_ephemeral_server(installation, ctx);
            }
        });

        Either::Left(async move {
            match rx.with_timeout(MCP_SERVER_STARTUP_TIMEOUT).await {
                Ok(Ok(result)) => result,
                Ok(Err(Canceled)) => {
                    log::error!("Subscription dropped before ephemeral MCP servers started");
                    Err(AgentDriverError::InvalidRuntimeState)
                }
                Err(TimeoutError) => {
                    log::error!("Timed out waiting for ephemeral MCP servers to start");
                    Err(AgentDriverError::MCPStartupFailed)
                }
            }
        })
    }

    /// Wait for all file-based MCP servers with the given UUIDs to reach a terminal state
    /// (`Running` or `FailedToStart`). Non-fatal: always completes without returning an error.
    ///
    /// **Sequencing note:** `AgentDriver` supports only one active subscription to
    /// [`TemplatableMCPServerManager`] at a time. This function, [`Self::start_mcp_servers`],
    /// and [`Self::start_ephemeral_mcp_servers`] must therefore run sequentially, never
    /// concurrently.
    fn wait_for_file_based_mcps_running(
        &self,
        uuids: Vec<Uuid>,
        ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = ()> {
        // Filter out UUIDs that have already reached a terminal state.
        let mut pending_uuids: HashSet<Uuid> = {
            let templatable_manager = TemplatableMCPServerManager::as_ref(ctx);
            uuids
                .into_iter()
                .filter(|uuid| {
                    !matches!(
                        templatable_manager.get_server_state(*uuid),
                        Some(MCPServerState::Running) | Some(MCPServerState::FailedToStart)
                    )
                })
                .collect()
        };

        if pending_uuids.is_empty() {
            log::info!("All file-based MCP servers are already running; proceeding");
            return Either::Right(future::ready(()));
        }

        let (tx, rx) = oneshot::channel::<()>();
        let mut tx = Some(tx);

        let templatable_manager_handle = TemplatableMCPServerManager::handle(ctx);
        let manager_clone = templatable_manager_handle.clone();

        ctx.subscribe_to_model(&templatable_manager_handle, move |_me, event, ctx| {
            if let TemplatableMCPServerManagerEvent::StateChanged { uuid, state } = event {
                if !pending_uuids.contains(uuid) {
                    return;
                }
                match state {
                    MCPServerState::Running | MCPServerState::FailedToStart => {
                        pending_uuids.remove(uuid);
                    }
                    _ => {
                        return;
                    }
                }
                if pending_uuids.is_empty() {
                    log::info!("All file-based MCP servers reached a terminal state; proceeding");
                    if let Some(sender) = tx.take() {
                        let _ = sender.send(());
                    }
                    ctx.unsubscribe_from_model(&manager_clone);
                }
            }
        });

        Either::Left(async move {
            match rx.with_timeout(MCP_SERVER_STARTUP_TIMEOUT).await {
                Ok(Ok(())) => {}
                Ok(Err(Canceled)) => {
                    log::warn!(
                        "File-based MCP server readiness subscription dropped early; proceeding"
                    );
                }
                Err(TimeoutError) => {
                    log::warn!(
                        "Timed out waiting for file-based MCP servers to reach a terminal state; proceeding without"
                    );
                }
            }
        })
    }

    /// Runs the agent to completion.
    /// Driving the agent mostly requires main-thread UI framework updates, but using `async` and
    /// a `ModelSpawner` lets us express the high-level process linearly rather than in a
    /// series of callbacks and state machine updates.
    async fn run_internal(
        task: Task,
        foreground: ModelSpawner<Self>,
    ) -> Result<(), AgentDriverError> {
        safe_debug!(
            safe: ("Running agent driver"),
            full: ("Running agent driver for query `{:?}`", task.prompt)
        );

        foreground
            .spawn(|me, _| me.check_working_dir())
            .await?
            .await?;

        // IMPORTANT: Wait for the terminal session to bootstrap before starting MCP servers.
        // Some of the initializations are necessary for the MCP servers to start correctly.
        //
        // Why: MCP server startup can happen before we actually execute the agent prompt. For
        // `TransportType::CLIServer` MCPs we currently depend on `AISettings.mcp_execution_path`,
        // which is populated as part of terminal bootstrap. Waiting for the session bootstrap
        // here avoids a subtle race where MCP spawn runs with an unset PATH and then the driver
        // only fails via a timeout.
        foreground
            .spawn(|me, ctx| {
                me.terminal_driver
                    .as_ref(ctx)
                    .wait_for_session_bootstrapped()
            })
            .await?
            .await?;

        // For the Oz harness only: set up MCP servers, model overrides, and profile information.
        if matches!(task.harness, HarnessKind::Oz) {
            // Resolve MCP specs into existing server UUIDs and ephemeral installations.
            let mcp_specs = task.mcp_specs.clone();
            let (existing_uuids, ephemeral_installations) = foreground
                .spawn(move |_, _| Self::resolve_mcp_specs(&mcp_specs))
                .await??;

            // Start any requested existing MCP servers first.
            log::info!(
                "Starting {} existing and {} ephemeral MCP servers",
                existing_uuids.len(),
                ephemeral_installations.len()
            );

            // TODO(BenS): combine these
            if !existing_uuids.is_empty() {
                foreground
                    .spawn(move |me, ctx| me.start_mcp_servers(&existing_uuids, ctx))
                    .await?
                    .await?;
            }
            // Start ephemeral MCP servers from inline JSON specs.
            if !ephemeral_installations.is_empty() {
                foreground
                    .spawn(move |me, ctx| {
                        me.start_ephemeral_mcp_servers(ephemeral_installations, ctx)
                    })
                    .await?
                    .await?;
            }
            let profile = task.profile.clone();
            foreground
                .spawn(move |me, ctx| me.configure_terminal(profile, ctx))
                .await??;

            if let Some(model_id) = task.model.clone() {
                foreground
                    .spawn(move |me, ctx| me.set_base_model_override(model_id, ctx))
                    .await??;
            }

            foreground
                .spawn(|me, ctx| me.start_profile_mcp_servers(ctx))
                .await?
                .await?;
        }

        // Run the harness with a prompt
        match task.harness {
            HarnessKind::Oz => {
                let conversation_status = foreground
                    .spawn(move |me, ctx| me.execute_run(task.prompt, ctx))
                    .await?
                    .await
                    .map_err(|_| {
                        log::error!("Subscription dropped before agent finished");
                        AgentDriverError::InvalidRuntimeState
                    })?;

                // Pause before returning to make sure that all conversation events are transmitted before the session is closed.
                // TODO: This is a bit of a bandaid fix, and it would be better if we explicitly waited for the session to end before terminating.
                // The way we could do that is through having the driver wait for all in-flight streams to be finished before terminating
                // and then call stop_sharing_session when they're done. To know when streams are finished, we would need to modify start_ordered_terminal_events_listener
                // to send a message when the streams are finished, flushed, and the websocket is disconnected. For now, we'll just sleep for a second, as this seems
                // to be enough time for the streams to be finished and the events to be flushed.
                warpui::r#async::Timer::after(Duration::from_secs(1)).await;

                conversation_status.into_result()
            }
            HarnessKind::ThirdParty(harness) => {
                let harness_exit_rx = Self::setup_harness(harness.as_ref(), &foreground).await?;
                let runner =
                    Self::prepare_harness(&task.prompt, harness.as_ref(), &foreground).await?;
                Self::run_harness(runner, &foreground, harness_exit_rx).await
            }
            HarnessKind::Unsupported(harness) => Err(AgentDriverError::HarnessSetupFailed {
                harness: harness.to_string(),
                reason: format!(
                    "The {harness} harness is only supported for local child agent launches."
                ),
            }),
        }
    }

    /// Sets up the third-party harness by subscribing to CLI session events and
    /// installing the Warp plugin and platform plugin, if applicable.
    ///
    /// Returns a oneshot receiver that fires when the harness should exit
    /// (either immediately on completion or after the idle-on-complete timeout).
    async fn setup_harness(
        harness: &dyn ThirdPartyHarness,
        foreground: &ModelSpawner<Self>,
    ) -> Result<oneshot::Receiver<()>, AgentDriverError> {
        let (exit_tx, exit_rx) = oneshot::channel();
        let harness_exit = IdleTimeoutSender::new(exit_tx);

        // Subscribe to CLI agent session events so we can update the task
        // state as the harness emits stop/blocked notifications.
        foreground
            .spawn(move |me, ctx| me.subscribe_to_cli_agent_session_events(harness_exit, ctx))
            .await?;

        // Install plugins before running the harness command.
        let plugin_manager: Option<Box<dyn CliAgentPluginManager>> =
            plugin_manager_for(harness.cli_agent());
        if let Some(manager) = plugin_manager {
            if let Err(e) = manager.install().await {
                log::warn!("Plugin installation failed (continuing): {e}");
            }
        }

        Ok(exit_rx)
    }

    /// Configure a third-party harness for execution. This will set `self.harness` and
    /// return a handle to the harness runner.
    async fn prepare_harness(
        prompt: &AgentRunPrompt,
        harness: &dyn ThirdPartyHarness,
        foreground: &ModelSpawner<Self>,
    ) -> Result<Arc<dyn harness::HarnessRunner>, AgentDriverError> {
        let (working_dir, task_id, agent_event_stream_client, terminal_driver) = foreground
            .spawn(|me, _| {
                if me.harness.is_some() {
                    log::error!(
                        "Attempted to prepare a third-party harness, but one was already configured"
                    );
                    return Err(AgentDriverError::InvalidRuntimeState);
                }

                Ok((
                    me.working_dir.clone(),
                    me.task_id,
                    Arc::new(DisabledAgentEventStreamClient),
                    me.terminal_driver.clone(),
                ))
            })
            .await
            .map_err(|_| AgentDriverError::InvalidRuntimeState)
            .flatten()?;

        let AgentRunPrompt::Local(prompt_text) = prompt;
        let system_prompt: Option<String> = None;
        let resumption_prompt: Option<String> = None;

        // Prepare harness config files (onboarding, trust dialog, API-key approval, etc.).
        let secrets = foreground
            .spawn(|me, _| Arc::clone(&me.secrets))
            .await
            .map_err(|_| AgentDriverError::InvalidRuntimeState)?;
        harness.prepare_environment_config(&working_dir, system_prompt.as_deref(), &secrets)?;

        let runner: Arc<dyn HarnessRunner> = harness
            .build_runner(
                prompt_text,
                system_prompt.as_deref(),
                resumption_prompt.as_deref(),
                &working_dir,
                task_id,
                agent_event_stream_client,
                terminal_driver,
            )?
            .into();

        let stored_runner = runner.clone();
        foreground
            .spawn(move |me, _| me.harness = Some(stored_runner))
            .await?;

        Ok(runner)
    }

    /// Execute a configured external harness in the terminal.
    ///
    /// The `harness_exit_rx` oneshot fires when the subscription determines it's
    /// time to exit (either immediately on completion or after the idle timeout).
    async fn run_harness(
        runner: Arc<dyn harness::HarnessRunner>,
        foreground: &ModelSpawner<Self>,
        harness_exit_rx: oneshot::Receiver<()>,
    ) -> Result<(), AgentDriverError> {
        // Start the third-party harness.
        let mut command_handle = runner.start(foreground).await?.fuse();
        let mut harness_exit_rx = harness_exit_rx.fuse();

        // Periodically save the conversation while the command is running and handle
        // exiting gracefully once the idle timeout elapses.
        let command_result = loop {
            futures::select! {
                exit_code = command_handle => break exit_code,
                _ = warpui::r#async::Timer::after(HARNESS_SAVE_INTERVAL).fuse() => {
                    log::debug!("Triggering periodic save of harness conversation data");
                    report_if_error!(runner
                        .save_conversation(SavePoint::Periodic, foreground)
                        .await
                        .context("Failed to save harness conversation (periodic)"));
                }
                _ = harness_exit_rx => {
                    log::debug!("Requesting harness exit");
                    report_if_error!(runner
                        .exit(foreground)
                        .await
                        .context("Failed to exit harness"));
                }
            }
        };

        // Final save after the command finishes.
        log::debug!("Triggering final save of harness conversation data");
        report_if_error!(runner
            .save_conversation(SavePoint::Final, foreground)
            .await
            .context("Failed to save harness conversation (final)"));
        report_if_error!(runner
            .cleanup(foreground)
            .await
            .context("Failed to clean up harness runtime state"));

        let exit_code = command_result?;
        log::debug!("Agent harness exited with status {exit_code}");

        if exit_code.was_successful() {
            Ok(())
        } else {
            Err(AgentDriverError::HarnessCommandFailed {
                exit_code: exit_code.value(),
            })
        }
    }

    /// Configure the active terminal session with the specified profile.
    fn configure_terminal(
        &self,
        profile: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), AgentDriverError> {
        let terminal_id = self.terminal_driver.as_ref(ctx).terminal_view().id();

        if let Some(profile) = profile {
            let server_id = ServerId::try_from(profile.as_str())
                .map_err(|_| AgentDriverError::ProfileError(profile.clone()))?;
            let sync_id = SyncId::ServerId(server_id);
            AIExecutionProfilesModel::handle(ctx).update(ctx, |model, ctx| {
                if let Some(profile_id) = model.get_profile_id_by_sync_id(&sync_id) {
                    model.set_active_profile(terminal_id, profile_id, ctx);
                } else {
                    return Err(AgentDriverError::ProfileError(profile.clone()));
                }
                Ok(())
            })?;
        }

        Ok(())
    }

    fn set_base_model_override(
        &self,
        model_id: LLMId,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), AgentDriverError> {
        let terminal_view_id = self.terminal_driver.as_ref(ctx).terminal_view().id();
        log::info!("Selecting base agent model {model_id} (from agent driver)");

        LLMPreferences::handle(ctx).update(ctx, |preferences, ctx| {
            preferences.update_preferred_agent_mode_llm(&model_id, terminal_view_id, ctx);
        });
        Ok(())
    }

    /// Execute an AI run in the terminal session and wait for it to complete.
    ///
    /// Conversation output is streamed as it's available.
    fn execute_run(
        &self,
        task_prompt: AgentRunPrompt,
        ctx: &mut ModelContext<Self>,
    ) -> Receiver<SDKConversationOutputStatus> {
        // Create a oneshot channel to signal task completion.
        let (tx, rx) = oneshot::channel();
        let run_exit = IdleTimeoutSender::new(tx);

        // Subscribe before the conversation starts.
        let history_model_handle = BlocklistAIHistoryModel::handle(ctx);
        let terminal_id = self.terminal_driver.as_ref(ctx).terminal_view().id();
        let mut written_conversation_id = false;

        // Create shared storage for the conversation ID
        let conversation_id_cell = Arc::new(Mutex::new(Option::<String>::None));
        let conversation_id_cell_for_handler = Arc::clone(&conversation_id_cell);

        ctx.subscribe_to_model(&history_model_handle, move |me, event, ctx| {
            if event.terminal_view_id().is_some_and(|id| id != terminal_id) {
                return;
            }

            match event {
                BlocklistAIHistoryEvent::UpdatedTodoList { .. } => {
                    // TODO: Log TODO list updates.
                }
                BlocklistAIHistoryEvent::AppendedExchange {
                    exchange_id,
                    conversation_id,
                    ..
                } => {
                    let Some(conversation) = BlocklistAIHistoryModel::as_ref(ctx)
                        .conversation(conversation_id)
                    else {
                        log::warn!("Invalid conversation ID: {conversation_id:?}");
                        return;
                    };

                    let Some(exchange) = conversation.exchange_with_id(*exchange_id) else {
                        log::warn!("Invalid exchange ID: {exchange_id:?}");
                        return;
                    };

                    // When a new exchange is appended, we should already have its inputs available.
                    report_if_error!(me
                        .write_exchange_inputs(exchange)
                        .context("Failed to write exchange inputs"));

                    // Reset the idle timer only if we've already scheduled one.
                    // This handles the case where a follow-up query creates new exchanges after
                    // the conversation has finished and an idle timer was set.
                    run_exit.cancel_idle_timeout();
                }
                BlocklistAIHistoryEvent::UpdatedStreamingExchange {
                    exchange_id,
                    conversation_id,
                    ..
                } => {
                    // Get conversation data first to avoid borrowing conflicts
                    let history_model = BlocklistAIHistoryModel::handle(ctx);
                    let conversation_data = history_model.as_ref(ctx).conversation(conversation_id)
                        .and_then(|conv| {
                            let token = conv.server_conversation_token().map(|t| t.as_str().to_string());
                            let exchange = conv.exchange_with_id(*exchange_id)?;
                            Some((token, exchange))
                        });
                    let Some((token_opt, exchange)) = conversation_data else {
                        log::warn!("Invalid conversation or exchange ID: {conversation_id:?}, {exchange_id:?}");
                        return;
                    };

                    if !written_conversation_id {
                        if let Some(token) = token_opt {
                            report_if_error!(output::with_stdout_buffered(|buf| match me.output_format {
                                OutputFormat::Json | OutputFormat::Ndjson => output::json::conversation_started(&token, buf),
                                OutputFormat::Text | OutputFormat::Pretty => output::text::conversation_started(&token, buf),
                            }).context("Failed to write conversation ID"));
                            written_conversation_id = true;

                            if let Ok(mut guard) = conversation_id_cell_for_handler.lock() {
                                *guard = Some(token);
                            }
                        }
                    }

                    // Once the outputs are fully streamed from the server, write them to stdout.
                    if exchange.output_status.is_finished() {
                        report_if_error!(me
                            .write_exchange_output(exchange)
                            .context("Failed to write exchange output"));
                    }

                }

                BlocklistAIHistoryEvent::UpdatedConversationStatus { terminal_view_id: conversation_terminal_id, conversation_id, .. } => {
                    if *conversation_terminal_id != terminal_id {
                        return;
                    }
                    let history_model = BlocklistAIHistoryModel::as_ref(ctx);
                    let Some(conversation) = history_model.conversation(conversation_id) else {
                        log::warn!("No active conversation for terminal view {conversation_terminal_id} with id {conversation_id}");
                        return;
                    };

                    if conversation.status().is_in_progress() {
                        // Conversation resumed or a new one started; cancel any pending idle timeout.
                        run_exit.cancel_idle_timeout();
                        return;
                    }

                    // Conversation is no longer in progress. Handle completion based on the result.
                    if let Some(conversation_status) =
                         conversation_output_status_from_conversation(conversation)
                    {
                        let output_status = match conversation_status {
                            AmbientConversationStatus::Success => {
                                SDKConversationOutputStatus::Success
                            }
                            AmbientConversationStatus::Cancelled { reason } => {
                                SDKConversationOutputStatus::Cancelled { reason }
                            }
                            AmbientConversationStatus::Error { error } => {
                                SDKConversationOutputStatus::Error { error }
                            }
                            AmbientConversationStatus::Blocked { blocked_action } => {
                                SDKConversationOutputStatus::Blocked { blocked_action }
                            }
                        };

                        match output_status {
                            SDKConversationOutputStatus::Success
                            | SDKConversationOutputStatus::Blocked { .. }
                            | SDKConversationOutputStatus::Cancelled { .. } => {
                                // Whether to keep the process alive after completion is controlled by
                                // the `warp agent run --idle-on-complete[=<DURATION>]` flag.
                                if let Some(idle_timeout) = me.idle_on_complete {
                                    run_exit.end_run_after(idle_timeout, output_status);
                                } else {
                                    run_exit.end_run_now(output_status);
                                }
                            }
                            // For errors, check if we expect an automatic retry.
                            SDKConversationOutputStatus::Error { ref error } => {
                                // If the error indicates that an automatic resume will be attempted,
                                // don't terminate yet - give the retry a chance to succeed.
                                // However, bound the wait so the CLI doesn't hang indefinitely
                                // if the follow-up never arrives.
                                if error.will_attempt_resume() {
                                    log::info!("Error occurred but automatic resume will be attempted; waiting up to {AUTO_RESUME_TIMEOUT:?} for retry");
                                    run_exit.end_run_after(AUTO_RESUME_TIMEOUT, output_status);
                                    return;
                                }

                                run_exit.end_run_now(output_status);
                            }
                        }
                    }
                }

                BlocklistAIHistoryEvent::SetActiveConversation { .. } => {
                    // Continuing an existing conversation should reset the idle timer.
                    run_exit.cancel_idle_timeout();
                }
                BlocklistAIHistoryEvent::StartedNewConversation { .. }
                | BlocklistAIHistoryEvent::ReassignedExchange { .. }
                | BlocklistAIHistoryEvent::ClearedConversationsInTerminalView { .. }
                | BlocklistAIHistoryEvent::UpdatedAutoexecuteOverride { .. }
                | BlocklistAIHistoryEvent::SplitConversation { .. }
                | BlocklistAIHistoryEvent::RemoveConversation { .. }
                | BlocklistAIHistoryEvent::DeletedConversation { .. }
                | BlocklistAIHistoryEvent::RestoredConversations { .. }
                | BlocklistAIHistoryEvent::CreatedSubtask { .. }
                | BlocklistAIHistoryEvent::UpgradedTask { .. }
                | BlocklistAIHistoryEvent::UpdatedConversationMetadata { .. }
                | BlocklistAIHistoryEvent::ClearedActiveConversation { .. }
                | BlocklistAIHistoryEvent::UpdatedConversationArtifacts { .. }
                | BlocklistAIHistoryEvent::ConversationAgentIdAssigned { .. } => (),
            }
        });

        // openWarp 不同步 plan 到 Warp Drive,原 "plan_artifact_created" CLI 输出依赖云 notebook_link,
        // 这里不再订阅 AIDocumentModel 的 SaveStatusUpdated 事件。

        // Submit the AI query.
        self.terminal_driver.update(ctx, |td, ctx| {
            td.with_terminal_view(ctx, |terminal, ctx| {
                let AgentRunPrompt::Local(prompt_str) = task_prompt;
                if FeatureFlag::AgentView.is_enabled() {
                    terminal.enter_agent_view(
                        Some(prompt_str),
                        None,
                        AgentViewEntryOrigin::Cli,
                        ctx,
                    );
                } else {
                    terminal.set_ai_input_mode_with_query(Some(&prompt_str), ctx);
                    terminal
                        .input()
                        .update(ctx, |input, ctx| input.input_enter(ctx));
                }
            })
        });

        rx
    }

    /// Write the inputs to an exchange to stdout.
    fn write_exchange_inputs(&self, exchange: &AIAgentExchange) -> io::Result<()> {
        output::with_stdout_buffered(|buf| {
            for input in &exchange.input {
                self.write_input(buf, input)?;
            }
            Ok(())
        })
    }

    /// Write the outputs of an exchange to stdout.
    fn write_exchange_output(&self, exchange: &AIAgentExchange) -> io::Result<()> {
        let Some(shared) = exchange.output_status.output() else {
            return Ok(());
        };
        let output = shared.get();

        output::with_stdout_buffered(|buf| self.write_output(buf, &output))
    }

    /// Format an agent input for display.
    fn write_input<W: Write>(&self, w: &mut W, input: &AIAgentInput) -> io::Result<()> {
        match self.output_format {
            OutputFormat::Json | OutputFormat::Ndjson => output::json::format_input(input, w),
            OutputFormat::Text | OutputFormat::Pretty => output::text::format_input(input, w),
        }
    }

    /// Format an agent output for display.
    fn write_output<W: Write>(&self, w: &mut W, output: &AIAgentOutput) -> io::Result<()> {
        match self.output_format {
            OutputFormat::Json | OutputFormat::Ndjson => output::json::format_output(output, w),
            OutputFormat::Text | OutputFormat::Pretty => output::text::format_output(output, w),
        }
    }

    /// Subscribe to the singleton `CLIAgentSessionsModel` so that idle-on-complete
    /// timers are driven by CLI agent session status changes.
    ///
    /// Task state reporting is handled centrally by `TaskStatusSyncModel`;
    /// the driver only registers the `terminal_view_id → task_id` mapping
    /// so that the sync model can look up the task for each session.
    fn subscribe_to_cli_agent_session_events(
        &self,
        harness_exit: IdleTimeoutSender<()>,
        ctx: &mut ModelContext<Self>,
    ) {
        let terminal_view_id = self.terminal_driver.as_ref(ctx).terminal_view().id();

        ctx.subscribe_to_model(
            &CLIAgentSessionsModel::handle(ctx),
            move |me, event, ctx| match event {
                CLIAgentSessionsModelEvent::StatusChanged {
                    terminal_view_id: event_tid,
                    status,
                    ..
                } => {
                    if *event_tid != terminal_view_id {
                        return;
                    }

                    // Drive idle-on-complete timer for the harness exit signal.
                    match status {
                        CLIAgentSessionStatus::Success | CLIAgentSessionStatus::Blocked { .. } => {
                            if let Some(idle_timeout) = me.idle_on_complete {
                                harness_exit.end_run_after(idle_timeout, ());
                            } else {
                                harness_exit.end_run_now(());
                            }
                        }
                        CLIAgentSessionStatus::InProgress => {
                            harness_exit.cancel_idle_timeout();
                        }
                    }
                }
                CLIAgentSessionsModelEvent::SessionUpdated {
                    terminal_view_id: event_tid,
                    ..
                } => {
                    if *event_tid != terminal_view_id {
                        return;
                    }

                    let Some(runner) = me.harness.clone() else {
                        return;
                    };
                    let spawner = ctx.spawner();
                    ctx.spawn(
                        async move {
                            log::debug!(
                                "Triggering post-turn harness session update from CLI agent event"
                            );
                            report_if_error!(runner
                                .handle_session_update(&spawner)
                                .await
                                .context("Failed to update harness state from CLI session event"));
                            log::debug!("Triggering post-turn save of harness conversation data");
                            report_if_error!(runner
                                .save_conversation(SavePoint::PostTurn, &spawner)
                                .await
                                .context("Failed to save harness conversation (post-turn)"));
                        },
                        |_, _, _| {},
                    );
                }
                CLIAgentSessionsModelEvent::Started { .. }
                | CLIAgentSessionsModelEvent::InputSessionChanged { .. }
                | CLIAgentSessionsModelEvent::Ended { .. } => {}
            },
        );
    }

    /// Handle events re-emitted by the `TerminalDriver`.
    fn handle_terminal_driver_event(&mut self, event: &TerminalDriverEvent) {
        match event {
            TerminalDriverEvent::SlowBootstrap => {
                eprintln!(
                    "Warning: Terminal session is slow to bootstrap. See https://docs.warp.dev/support-and-community/troubleshooting-and-support/known-issues#shells to troubleshoot."
                );
            }
        }
    }
}

impl Entity for AgentDriver {
    type Event = ();
}

/// The only reason that `AgentDriver` is a singleton entity is to ensure the UI framework
/// doesn't drop it. Generally, we should not assume there's only one running agent.
impl SingletonEntity for AgentDriver {}

/// Write the run ID to stdout using the appropriate output format.
pub(super) fn write_run_started(run_id: &str, output_format: OutputFormat) {
    report_if_error!(output::with_stdout_buffered(|buf| match output_format {
        OutputFormat::Json | OutputFormat::Ndjson => output::json::run_started(run_id, buf),
        OutputFormat::Text | OutputFormat::Pretty => output::text::run_started(run_id, buf),
    })
    .context("Failed to write run ID"));
}
