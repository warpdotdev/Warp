use std::{
    collections::HashMap,
    ffi::OsString,
    future::Future,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::channel::oneshot;
use warp_completer::completer::CommandOutput;
use warp_core::command::ExitCode;
use warp_util::path::ShellFamily;
use warpui::{r#async::FutureExt, AppContext, Entity, ModelContext, ModelHandle, ViewHandle};

use crate::terminal::model::session::ExecuteCommandOptions;

use crate::{
    ai::ambient_agents::AmbientAgentTaskId,
    pane_group::NewTerminalOptions,
    root_view::{open_new_with_workspace_source, NewWorkspaceSource},
    terminal::{
        model::block::{BlockId, SerializedBlock},
        shared_session::IsSharedSessionCreator,
        shell::ShellType,
        TerminalView,
    },
    util::sync::Condition,
};

use crate::ai::attachment_utils::attachments_download_dir;

use super::AgentDriverError;

const TERMINAL_SESSION_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(60);

/// Options for creating the terminal view before constructing a [`TerminalDriver`].
pub(crate) struct TerminalDriverOptions {
    pub working_dir: PathBuf,
    pub env_vars: HashMap<OsString, OsString>,
    pub should_share: bool,
    pub task_id: Option<AmbientAgentTaskId>,
}

/// Events emitted by [`TerminalDriver`] for [`super::AgentDriver`] to react to.
pub(crate) enum TerminalDriverEvent {
    /// Terminal bootstrap is taking unusually long.
    SlowBootstrap,
}

/// Manages the terminal session lifecycle for the agent driver.
///
/// Responsibilities:
/// - Monitoring for terminal bootstrapping to be done
/// - Executing commands in the session
/// - Detecting block completion
pub(crate) struct TerminalDriver {
    terminal_view: ViewHandle<TerminalView>,
    session_bootstrapped: Condition,
    waiting_command: Option<oneshot::Sender<ExitCode>>,

    /// State for the pending command we're expecting to start executing.
    /// The `String` is the expected command text, and the sender is used
    /// to send the block ID to the waiting caller.
    pending_command_start: Option<(String, oneshot::Sender<BlockId>)>,
}

impl Entity for TerminalDriver {
    type Event = TerminalDriverEvent;
}

/// Create the terminal window and extract the [`ViewHandle<TerminalView>`].
///
/// This is separate from [`TerminalDriver::new`] because [`AppContext::add_model`]
/// requires an infallible constructor; the fallible window/view creation must happen first.
fn create_terminal_view(
    options: TerminalDriverOptions,
    ctx: &mut AppContext,
) -> Result<ViewHandle<TerminalView>, AgentDriverError> {
    let _ = options.should_share;
    let is_shared_session_creator = IsSharedSessionCreator::No;

    let (_, root_view) = open_new_with_workspace_source(
        NewWorkspaceSource::Session {
            options: Box::new(NewTerminalOptions {
                is_shared_session_creator,
                initial_directory: Some(options.working_dir),
                env_vars: options.env_vars,
                ..Default::default()
            }),
        },
        ctx,
    );

    root_view
        .as_ref(ctx)
        .workspace_view()
        .ok_or(AgentDriverError::TerminalUnavailable)?
        .as_ref(ctx)
        .active_tab_pane_group()
        .as_ref(ctx)
        .active_session_view(ctx)
        .ok_or(AgentDriverError::TerminalUnavailable)
}

impl TerminalDriver {
    /// Create a terminal view from the given options and wrap it in a new `TerminalDriver` model.
    pub(crate) fn create(
        options: TerminalDriverOptions,
        ctx: &mut AppContext,
    ) -> Result<ModelHandle<Self>, AgentDriverError> {
        let task_id = options.task_id;
        let working_dir = options.working_dir.clone();
        let terminal_view = create_terminal_view(options, ctx)?;
        Ok(ctx.add_model(|ctx| Self::new(terminal_view, task_id, working_dir, ctx)))
    }

    /// Wrap an already-created terminal view in a new `TerminalDriver` model.
    ///
    /// Unlike [`Self::create`], this does not open a new window — it reuses an
    /// existing view (e.g. a docker sandbox pane). Session sharing is disabled
    /// and no task ID is associated.
    pub(crate) fn create_from_existing_view(
        terminal_view: ViewHandle<TerminalView>,
        ctx: &mut AppContext,
    ) -> ModelHandle<Self> {
        ctx.add_model(|ctx| Self::new(terminal_view, None, PathBuf::default(), ctx))
    }

    /// Set up event subscriptions for an already-created terminal view.
    fn new(
        terminal_view: ViewHandle<TerminalView>,
        task_id: Option<AmbientAgentTaskId>,
        working_dir: PathBuf,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let session_bootstrapped = Condition::new();

        // 立即把 task_id 和附件下载目录写入 AI controller,供本地任务元数据和附件使用。
        if let Some(tid) = task_id {
            let attachments_dir = attachments_download_dir(&working_dir);
            terminal_view.update(ctx, |terminal, ctx| {
                terminal.ai_controller().update(ctx, |controller, ctx| {
                    controller.set_ambient_agent_task_id(Some(tid), ctx);
                    controller.set_attachments_download_dir(attachments_dir);
                });
            });
        }

        ctx.subscribe_to_view(&terminal_view, move |me, event, ctx| {
            me.handle_terminal_view_event(event, ctx);
        });

        // If the session already bootstrapped before we subscribed, set the
        // condition immediately so callers of `wait_for_session_bootstrapped`
        // don't block forever.
        let already_bootstrapped = terminal_view.read(ctx, |terminal, _| {
            terminal
                .model
                .lock()
                .block_list()
                .is_bootstrapping_precmd_done()
        });
        if already_bootstrapped {
            session_bootstrapped.set();
        }

        Self {
            terminal_view,
            session_bootstrapped,
            waiting_command: None,
            pending_command_start: None,
        }
    }

    /// Get a handle to the backing terminal view.
    pub fn terminal_view(&self) -> &ViewHandle<TerminalView> {
        &self.terminal_view
    }

    /// Provide mutable access to the terminal view through a closure.
    pub fn with_terminal_view(
        &self,
        ctx: &mut ModelContext<Self>,
        f: impl FnOnce(&mut TerminalView, &mut warpui::ViewContext<TerminalView>),
    ) {
        self.terminal_view.update(ctx, f);
    }

    /// Submit `text` to the active CLI agent on the terminal PTY using the
    /// agent-specific submission strategy.
    ///
    /// Used to send exit commands to third-party harnesses.
    pub(super) fn send_text_to_cli(&self, text: String, ctx: &mut ModelContext<Self>) {
        self.terminal_view.update(ctx, |terminal, ctx| {
            terminal.submit_text_to_cli_agent_pty(text, ctx);
        });
    }

    /// Return a snapshot of the block with the given ID.
    pub fn block_snapshot(&self, block_id: &BlockId, ctx: &AppContext) -> Option<SerializedBlock> {
        let terminal = self.terminal_view.as_ref(ctx);
        let model = terminal.model.lock();
        model
            .block_list()
            .block_with_id(block_id)
            .map(SerializedBlock::from)
    }

    /// Execute a command in the terminal and return a future that resolves to a
    /// [`CommandHandle`] once the command starts executing.
    pub fn execute_command(
        &mut self,
        command: &str,
        ctx: &mut ModelContext<Self>,
    ) -> Result<impl Future<Output = Result<CommandHandle, AgentDriverError>>, AgentDriverError>
    {
        let (exit_tx, exit_rx) = oneshot::channel::<ExitCode>();
        let (start_tx, start_rx) = oneshot::channel::<BlockId>();

        // We should not be able to execute a command while we are still waiting on another one.
        // This is enforced by the caller by waiting on rx before continuing.
        if self.waiting_command.is_some() || self.pending_command_start.is_some() {
            return Err(AgentDriverError::InvalidRuntimeState);
        }

        let command_string = command.to_string();
        self.terminal_view.update(ctx, |terminal, ctx| {
            self.waiting_command = Some(exit_tx);
            self.pending_command_start = Some((command_string, start_tx));
            terminal.execute_command_or_set_pending(command, ctx);
        });

        Ok(async move {
            let block_id = start_rx
                .await
                .map_err(|_| AgentDriverError::InvalidRuntimeState)?;
            Ok(CommandHandle {
                exit_status_rx: exit_rx,
                block_id,
            })
        })
    }

    /// Execute a command through the active session's in-band command
    /// executor, without adding a block to the user-visible blocklist.
    ///
    /// Intended for silent probes (e.g. `test -d`) that the agent needs to
    /// drive through the terminal session (so they run against the correct
    /// filesystem, including inside a Docker sandbox) but should not clutter
    /// the user's command history.
    pub fn execute_silent_command(
        &self,
        command: String,
        ctx: &ModelContext<Self>,
    ) -> impl Future<Output = Result<CommandOutput, AgentDriverError>> {
        let session = self.terminal_view.read(ctx, |terminal, app| {
            terminal
                .active_block_session_id()
                .and_then(|id| terminal.sessions_model().as_ref(app).get(id))
        });
        async move {
            let session = session.ok_or(AgentDriverError::InvalidRuntimeState)?;
            session
                .execute_command(&command, None, None, ExecuteCommandOptions::default())
                .await
                .map_err(|e| {
                    log::warn!("silent command failed: {e:#}");
                    AgentDriverError::InvalidRuntimeState
                })
        }
    }

    /// Returns the shell type of the active terminal session, if known.
    pub fn active_session_shell_type(&self, ctx: &AppContext) -> Option<ShellType> {
        self.terminal_view
            .read(ctx, |terminal, app| terminal.active_session_shell_type(app))
    }

    /// Build the shell-aware `cd <escaped>` command for the active session.
    ///
    /// Shared between [`Self::cd`] and [`Self::cd_silent`] so both paths use
    /// the same [`ShellFamily::shell_escape`] logic (posix single-quoting,
    /// fish backslash, pwsh double-quote doubling) and don't drift.
    fn build_cd_command(&self, target: &str, ctx: &AppContext) -> String {
        let shell_family = self.terminal_view.read(ctx, |terminal, app| {
            terminal
                .active_session_shell_type(app)
                .map(ShellFamily::from)
                .unwrap_or(ShellFamily::Posix)
        });
        let escaped_target = shell_family.shell_escape(target);
        format!("cd {escaped_target}")
    }

    /// Change directory within the active terminal session.
    pub fn cd(
        &mut self,
        target: &str,
        ctx: &mut ModelContext<Self>,
    ) -> Result<impl Future<Output = Result<CommandHandle, AgentDriverError>>, AgentDriverError>
    {
        let cd_command = self.build_cd_command(target, ctx);
        self.execute_command(&cd_command, ctx)
    }

    /// Change directory within the active terminal session, silently — no
    /// visible block is added to the user-facing blocklist.
    ///
    /// Uses the same shell-aware escaping as [`Self::cd`] but dispatches
    /// through [`Self::execute_silent_command`]. Intended for callers that
    /// need to position the session's CWD as an implementation detail of a
    /// larger setup step (e.g. positioning the session before running
    /// silent probes).
    pub fn cd_silent(
        &self,
        target: &str,
        ctx: &ModelContext<Self>,
    ) -> impl Future<Output = Result<CommandOutput, AgentDriverError>> {
        let cd_command = self.build_cd_command(target, ctx);
        self.execute_silent_command(cd_command, ctx)
    }

    /// The current working directory of the active terminal session, if known.
    #[allow(dead_code)]
    pub fn current_directory(&self, ctx: &AppContext) -> Option<PathBuf> {
        // TODO(ben): This should handle non-local paths.
        self.terminal_view
            .as_ref(ctx)
            .active_session_path_if_local(ctx)
    }

    /// Returns a future that resolves when the session has bootstrapped.
    ///
    /// This only waits for the `SessionBootstrapped` terminal view event.
    pub fn wait_for_session_bootstrapped(
        &self,
    ) -> impl Future<Output = Result<(), AgentDriverError>> {
        let session_bootstrapped = self.session_bootstrapped.clone();

        async move {
            session_bootstrapped
                .wait()
                .with_timeout(TERMINAL_SESSION_BOOTSTRAP_TIMEOUT)
                .await
                .map_err(|_| {
                    log::error!("Timed out waiting for session bootstrap");
                    AgentDriverError::BootstrapFailed
                })
        }
    }
}

/// A handle to a running terminal command.
///
/// Resolves to the command's [`ExitCode`] when the block completes.
/// Also carries the [`BlockId`] so callers can retrieve the block snapshot
/// after completion.
pub(crate) struct CommandHandle {
    exit_status_rx: oneshot::Receiver<ExitCode>,
    block_id: BlockId,
}

impl CommandHandle {
    /// The block ID of the command that was executed.
    pub fn block_id(&self) -> &BlockId {
        &self.block_id
    }
}

impl Future for CommandHandle {
    type Output = Result<ExitCode, AgentDriverError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.exit_status_rx)
            .poll(cx)
            .map(|result| result.map_err(|_| AgentDriverError::InvalidRuntimeState))
    }
}

impl TerminalDriver {
    /// Handle terminal view events.
    fn handle_terminal_view_event(
        &mut self,
        event: &crate::terminal::view::Event,
        ctx: &mut ModelContext<Self>,
    ) {
        match event {
            crate::terminal::view::Event::SessionBootstrapped => {
                self.session_bootstrapped.set();
            }
            crate::terminal::view::Event::SlowBootstrap => {
                ctx.emit(TerminalDriverEvent::SlowBootstrap);
            }
            crate::terminal::view::Event::ExecuteCommand(event) => {
                if let Some((_expected_command, sender)) = self
                    .pending_command_start
                    .take_if(|(cmd, _)| *cmd == event.command)
                {
                    let block_id = self.terminal_view.read(ctx, |terminal, _| {
                        terminal.model.lock().block_list().active_block_id().clone()
                    });
                    let _ = sender.send(block_id);
                }
            }
            crate::terminal::view::Event::BlockCompleted { block, .. } => {
                if let Some(sender) = self.waiting_command.take_if(|_| {
                    let bootstrapping_done = self.terminal_view.read(ctx, |terminal, _| {
                        terminal
                            .model
                            .lock()
                            .block_list()
                            .is_bootstrapping_precmd_done()
                    });
                    // This was originally checking `bootstrapping_done && block.did_execute`.
                    // Oddly, we've seen cases where we missed the preexec hook, so
                    // `block.did_execute` is false even though the command actually did run.
                    // To hedge against this while we're still figuring out the root cause,
                    // we instead simply make sure it was not a background block.
                    bootstrapping_done && !block.is_background
                }) {
                    let _ = sender.send(block.exit_code);
                }
            }
            _ => (),
        }
    }
}
