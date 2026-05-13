//! [`TerminalView`]-specific implementation for ambient agent functionality.

use warp_cli::agent::Harness;

use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::AIRequestUsageModel;
use warpui::prelude::Empty;

use crate::ai::blocklist::{agent_view::AgentViewEntryOrigin, BlocklistAIHistoryModel};
use crate::terminal::view::ambient_agent::AmbientAgentInitialUserQuery;
use crate::terminal::view::rich_content::RichContentInsertionPosition;
use crate::terminal::view::TerminalView;
use crate::terminal::CLIAgent;
use crate::workspaces::user_workspaces::UserWorkspaces;
use warp_core::ui::appearance::Appearance;
use warpui::elements::Align;
use warpui::{AppContext, Element, EntityId, SingletonEntity, ViewContext};

use super::loading_screen::{
    render_ambient_agent_cancelled_screen, render_ambient_agent_error_screen,
    render_ambient_agent_github_auth_required_screen, render_ambient_agent_loading_screen,
};
use super::AmbientAgentViewModelEvent;
const CHILD_AGENT_GITHUB_AUTH_REQUIRED_BLOCKED_ACTION: &str =
    "GitHub authentication required before starting the child agent.";

impl TerminalView {
    fn active_ambient_agent_conversation_id(&self, ctx: &AppContext) -> Option<AIConversationId> {
        self.agent_view_controller
            .as_ref(ctx)
            .agent_view_state()
            .active_conversation_id()
    }

    fn active_ambient_agent_conversation_is_child(&self, ctx: &AppContext) -> bool {
        let Some(conversation_id) = self.active_ambient_agent_conversation_id(ctx) else {
            return false;
        };

        BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&conversation_id)
            .is_some_and(|conversation| conversation.is_child_agent_conversation())
    }

    fn update_active_ambient_agent_conversation_status(
        &self,
        status: ConversationStatus,
        error_message: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(conversation_id) = self.active_ambient_agent_conversation_id(ctx) else {
            return;
        };

        BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
            history_model.update_conversation_status_with_error_message(
                self.id(),
                conversation_id,
                status,
                error_message,
                ctx,
            );
        });
    }

    pub(in crate::terminal::view) fn show_out_of_credits_modal(&self, ctx: &mut ViewContext<Self>) {
        let is_on_paid_plan = UserWorkspaces::as_ref(ctx)
            .current_workspace()
            .is_some_and(|workspace| workspace.billing_metadata.is_user_on_paid_plan());

        if is_on_paid_plan {
            // 去云端分支:不再展示 agent capacity 模态
        } else {
            AIRequestUsageModel::handle(ctx).update(ctx, |model, ctx| {
                model.refresh_request_usage_async(ctx);
            });
        }
    }

    /// Handles ambient agent view model events.
    pub(in crate::terminal::view) fn handle_ambient_agent_event(
        &mut self,
        event: &AmbientAgentViewModelEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        // Tear down the non-oz ambient-agent queued-prompt block on terminal / transition
        // events that replace it. `Failed`, `NeedsGithubAuth`, and `Cancelled` hand off
        // to the existing error / auth / cancelled UI; `HarnessCommandStarted` hands
        // off to the live harness CLI block. Idempotent and cheap when no block exists.
        if matches!(
            event,
            AmbientAgentViewModelEvent::Failed { .. }
                | AmbientAgentViewModelEvent::NeedsGithubAuth
                | AmbientAgentViewModelEvent::Cancelled
                | AmbientAgentViewModelEvent::HarnessCommandStarted
        ) {
            self.remove_pending_user_query_block(ctx);
        }

        match event {
            AmbientAgentViewModelEvent::EnteredSetupState => {
                // Re-render to show the setup view.
                self.update_pane_configuration(ctx);
                ctx.notify();
            }
            AmbientAgentViewModelEvent::EnteredComposingState => {
                // Update pane configuration to show cloud indicator.
                self.update_pane_configuration(ctx);
            }
            AmbientAgentViewModelEvent::DispatchedAgent => {
                // Pane chrome (e.g. cloud indicator, task id) must update on viewer surfaces
                // too, so this runs above the viewer short-circuit below.
                self.update_pane_configuration(ctx);
                // Only the spawner's view handles `DispatchedAgent`. Viewer surfaces (shared
                // ambient agent session or transcript viewer) have no submitted prompt to render
                // and should not insert ambient-agent rich content here.
                let is_viewer = self.is_shared_ambient_agent_session()
                    || self.model.lock().is_conversation_transcript_viewer();
                if is_viewer {
                    ctx.notify();
                    return;
                }
                if false {
                    if self
                        .ambient_agent_view_model
                        .as_ref(ctx)
                        .is_third_party_harness()
                    {
                        // Non-oz runs: render the submitted prompt via the queued-prompt UI.
                        // The block is removed later by `HarnessCommandStarted` / failure /
                        // cancel / auth handlers.
                        let prompt = self
                            .ambient_agent_view_model
                            .as_ref(ctx)
                            .request()
                            .map(|request| request.prompt.clone())
                            .unwrap_or_default();
                        if !prompt.is_empty() {
                            self.insert_ambient_agent_queued_user_query_block(prompt, ctx);
                        }
                    } else {
                        let initial_user_query = ctx.add_view(|ctx| {
                            AmbientAgentInitialUserQuery::new(
                                self.ambient_agent_view_model.clone(),
                                ctx,
                            )
                        });
                        self.insert_rich_content(
                            None,
                            initial_user_query,
                            None,
                            RichContentInsertionPosition::Append {
                                insert_below_long_running_block: true,
                            },
                            ctx,
                        );
                        self.ambient_agent_view_model.update(ctx, |model, _| {
                            model.set_has_inserted_ambient_agent_user_query_block(true);
                        });
                    }
                } else {
                    // Reset tip cooldown so the first tip shows for 60 seconds
                    let tip_model = self
                        .ambient_agent_view_model
                        .as_ref(ctx)
                        .ui_state
                        .tip_model
                        .clone();
                    tip_model.update(ctx, |model, model_ctx| {
                        model.reset_cooldown(model_ctx);
                    });
                }
                // Re-render to show loading state.
                ctx.notify();
            }
            AmbientAgentViewModelEvent::SessionReady => {
                // Auto-open details panel for local ambient-agent once the session is ready.
                self.maybe_auto_open_ambient_agent_details_panel(ctx);
                // Re-render to hide the loading screen now that the session is ready.
                ctx.notify();
            }
            AmbientAgentViewModelEvent::ProgressUpdated => {
                // Refresh the tip (respects 60s cooldown internally)
                let tip_model = self
                    .ambient_agent_view_model
                    .as_ref(ctx)
                    .ui_state
                    .tip_model
                    .clone();
                tip_model.update(ctx, |model, model_ctx| {
                    model.maybe_refresh_tip(model_ctx);
                });
                // Update pane header to reflect any changes (e.g., task_id being set)
                self.update_pane_configuration(ctx);
                ctx.notify();
            }
            AmbientAgentViewModelEvent::Failed { error_message } => {
                self.update_active_ambient_agent_conversation_status(
                    ConversationStatus::Error,
                    Some(error_message.clone()),
                    ctx,
                );
                // Re-render to show the error state in the footer.
                ctx.notify();
            }
            AmbientAgentViewModelEvent::ShowAICreditModal => {
                if false
                    && self.ambient_agent_view_model.as_ref(ctx).is_ambient_agent()
                    && !self.model.lock().is_shared_ambient_agent_session()
                {
                    self.show_out_of_credits_modal(ctx);
                }

                ctx.notify();
            }
            AmbientAgentViewModelEvent::NeedsGithubAuth => {
                if self.active_ambient_agent_conversation_is_child(ctx) {
                    self.update_active_ambient_agent_conversation_status(
                        ConversationStatus::Blocked {
                            blocked_action: CHILD_AGENT_GITHUB_AUTH_REQUIRED_BLOCKED_ACTION
                                .to_string(),
                        },
                        None,
                        ctx,
                    );
                }
                // Re-render to show the GitHub auth required state in the footer.
                ctx.notify();
            }
            AmbientAgentViewModelEvent::Cancelled => {
                self.update_active_ambient_agent_conversation_status(
                    ConversationStatus::Cancelled,
                    None,
                    ctx,
                );
                // Re-render to show the cancelled state in the footer.
                ctx.notify();
            }
            AmbientAgentViewModelEvent::HarnessSelected => {
                self.maybe_enter_agent_view_for_shared_third_party_viewer(ctx);
                ctx.notify();
            }
            AmbientAgentViewModelEvent::HarnessCommandStarted => {
                // Stop classifying new blocks as environment setup commands, mirroring the
                // Oz path in the `AppendedExchange` handler. Flipping this flag to `false`
                // also un-hides and un-marks the active block so it renders like a normal
                // CLI-agent session.
                {
                    let mut model = self.model.lock();
                    if model
                        .block_list()
                        .is_executing_oz_environment_startup_commands()
                    {
                        model
                            .block_list_mut()
                            .set_is_executing_oz_environment_startup_commands(false);
                    }
                }
                // Force a fresh viewer size report to the sharer so the harness CLI (e.g.
                // the claude TUI) starts at our terminal's actual dimensions instead of
                // whatever the sandbox PTY was sized to during setup.
                self.force_report_viewer_terminal_size(ctx);
                ctx.notify();
            }
        }
    }

    /// Enters agent view for a live shared-session viewer of a non-oz cloud run, so every
    /// viewer lands in the same agent-view chrome regardless of which entry point opened the
    /// conversation. Called from the `HarnessSelected` handler once the viewer has resolved
    /// the run's harness asynchronously.
    ///
    /// Transcript viewer entry is handled directly in `load_data_into_transcript_viewer` so
    /// the snapshot block exists before we retag — we intentionally do not trigger that path
    /// here.
    ///
    /// The viewer-context guard is load-bearing: `HarnessSelected` also fires when the local
    /// spawner picks a harness from the dropdown, and in that case the cloud-mode setup flow
    /// handles agent view entry instead.
    fn maybe_enter_agent_view_for_shared_third_party_viewer(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) {
        if self
            .agent_view_controller
            .as_ref(ctx)
            .agent_view_state()
            .is_active()
        {
            return;
        }
        if !self
            .ambient_agent_view_model
            .as_ref(ctx)
            .is_third_party_harness()
        {
            return;
        }
        if !self.is_shared_ambient_agent_session() {
            return;
        }

        self.enter_agent_view_for_new_conversation(
            None,
            AgentViewEntryOrigin::ThirdPartyCloudAgent,
            ctx,
        );

        let Some(vehicle_conversation_id) = self
            .agent_view_controller
            .as_ref(ctx)
            .agent_view_state()
            .active_conversation_id()
        else {
            return;
        };

        // Retag existing non-setup blocks so the harness content passes the agent view filter.
        self.model
            .lock()
            .block_list_mut()
            .attach_non_startup_blocks_to_conversation(vehicle_conversation_id);

        // Retag rich content inserted in terminal mode (setup-commands summary, tombstone, …)
        // so it stays visible under the vehicle conversation. Rich content with
        // `agent_view_conversation_id == None` is hidden in full-screen agent view by
        // `RichContentItem::should_hide_for_agent_view_state`.
        let ids_to_retag: Vec<EntityId> = self
            .rich_content_views
            .iter()
            .filter(|rc| rc.agent_view_conversation_id().is_none())
            .map(|rc| rc.view_id())
            .collect();
        for view_id in ids_to_retag {
            self.set_rich_content_agent_view_conversation_id(view_id, vehicle_conversation_id);
        }
    }

    /// Returns `true` when the active block's command is the CLI for the run's configured
    /// non-oz harness (e.g. `claude …` for [`Harness::Claude`]).
    /// Used to detect the harness-start transition at `AfterBlockStarted` time. Unlike
    /// `detect_cli_agent_from_model`, this does NOT gate on `is_active_and_long_running` —
    /// we want to classify the block as the harness session as soon as it starts, before the
    /// long-running timer would otherwise elapse.
    fn active_block_matches_run_harness(&self, ctx: &AppContext) -> bool {
        let command = self
            .model
            .lock()
            .block_list()
            .active_block()
            .command_with_secrets_obfuscated(false);
        let Some(cli_agent) = CLIAgent::detect(&command, None, None, ctx) else {
            return false;
        };
        match self.ambient_agent_view_model.as_ref(ctx).selected_harness() {
            Harness::Oz => false,
            Harness::Claude => matches!(cli_agent, CLIAgent::Claude),
            Harness::OpenCode => matches!(cli_agent, CLIAgent::OpenCode),
            Harness::Gemini => matches!(cli_agent, CLIAgent::Gemini),
            Harness::Unknown => false,
        }
    }

    /// Renders the ambient agent progress view based on agent progress.
    pub(in crate::terminal::view) fn render_ambient_agent_progress(
        &self,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let ambient_agent_model = self.ambient_agent_view_model.as_ref(app);
        let Some(progress) = ambient_agent_model.agent_progress() else {
            return Empty::new().finish();
        };

        // Show appropriate screen based on agent status
        let ui_state = &ambient_agent_model.ui_state;
        let screen = if ambient_agent_model.is_cancelled() {
            // Show cancelled screen
            render_ambient_agent_cancelled_screen(appearance)
        } else if let Some(auth_url) = ambient_agent_model.github_auth_url() {
            // Show GitHub auth required screen
            render_ambient_agent_github_auth_required_screen(
                auth_url,
                appearance,
                &ui_state.auth_button_mouse_state,
                app,
            )
        } else if let Some(error_message) = ambient_agent_model.error_message() {
            // Show error screen
            render_ambient_agent_error_screen(
                error_message,
                appearance,
                &ui_state.error_selection_handle,
                &ui_state.error_selected_text,
                app,
            )
        } else {
            // Show loading screen - determine the message based on progress state
            let message = if progress.harness_started_at.is_some() {
                "Starting Environment (Step 3/3)"
            } else if progress.claimed_at.is_some() {
                "Creating Environment (Step 2/3)"
            } else {
                "Connecting to Host (Step 1/3)"
            };

            render_ambient_agent_loading_screen(
                message,
                appearance,
                &ui_state.loading_shimmer_handle,
                &ui_state.tip_model,
                app,
            )
        };

        // Center the screen within the terminal view
        Align::new(screen).finish()
    }

    /// Auto-opens the ambient-agent details panel once. No-op (cloud removed).
    pub(in crate::terminal::view) fn maybe_auto_open_ambient_agent_details_panel(
        &mut self,
        _ctx: &mut ViewContext<Self>,
    ) {
    }
}
