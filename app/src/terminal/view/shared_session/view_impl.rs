//! [`TerminalView`]-specific implementation for shared sessions.

use crate::context_chips::ContextChipKind;
use crate::editor::{InteractionState, ReplicaId};
use crate::settings::InputModeSettings;
use crate::terminal::block_list_viewport::ScrollPositionUpdate;
use crate::terminal::model::blocks::BlockListPoint;
use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::WithinBlock;
use crate::terminal::shared_session::protocol::RoleUpdatedReason;
use crate::terminal::shared_session::protocol::SessionSourceType;
use crate::terminal::shared_session::protocol::{ParticipantId, Role, SessionId, WindowSize};
use crate::terminal::shared_session::protocol::{RoleUpdateReason, SessionEndedReason};
use crate::terminal::shared_session::settings::SharedSessionSettings;
use crate::terminal::shared_session::{
    selections::point_to_session_sharing, SharedSessionActionSource, SharedSessionScrollbackType,
    SharedSessionStatus,
};
use crate::terminal::view::{
    ContextMenuAction, Event, InlineBannerItem, InlineBannerType, RichContentInsertionPosition,
    SharedSessionBanners, SizeUpdateBuilder, TerminalAction, TerminalView,
};
use crate::terminal::TerminalModel;
use crate::view_components::ToastFlavor;
use crate::{
    menu::{MenuItem, MenuItemFields},
    terminal::shared_session::presence_manager::{Event as PresenceManagerEvent, PresenceManager},
};
use chrono::{DateTime, Local};
use itertools::Itertools;
use warpui::r#async::Timer;

use settings::Setting as _;
use warp_core::semantic_selection::SemanticSelection;
use warpui::units::IntoLines;
use warpui::SingletonEntity;
use warpui::{ModelHandle, ViewContext};

use crate::terminal::shared_session::participant_avatar_view::ParticipantAvatarEvent;
use crate::terminal::shared_session::participant_avatar_view::ParticipantAvatarView;

use crate::terminal::shared_session::protocol::ParticipantList;
use crate::terminal::shared_session::protocol::ParticipantPresenceUpdate;

use warpui::AppContext;

use super::adapter::{Adapter, Kind, Participant};
use super::sharer::inactivity_modal::InactivityModalEvent;
use super::sharer::Sharer;
use super::viewer::Viewer;
use super::ConversationEndedTombstoneView;

impl TerminalView {
    pub fn sharer_session_kind(&self) -> Option<&Kind> {
        self.shared_session.as_ref().map(|s| s.kind())
    }

    pub fn sharer_session_kind_mut(&mut self) -> Option<&mut Kind> {
        self.shared_session.as_mut().map(|s| s.kind_mut())
    }

    pub fn shared_session_sharer(&self) -> Option<&Sharer> {
        self.sharer_session_kind().and_then(|k| k.as_sharer())
    }

    pub fn shared_session_sharer_mut(&mut self) -> Option<&mut Sharer> {
        self.sharer_session_kind_mut()
            .and_then(|k| k.as_sharer_mut())
    }

    pub fn shared_session_viewer(&self) -> Option<&Viewer> {
        self.sharer_session_kind().and_then(|k| k.as_viewer())
    }

    pub fn shared_session_viewer_mut(&mut self) -> Option<&mut Viewer> {
        self.sharer_session_kind_mut()
            .and_then(|k| k.as_viewer_mut())
    }

    // TODO (suraj): do we actually need to expose this? It's a bit of a smell.
    pub fn shared_session_presence_manager(&self) -> Option<ModelHandle<PresenceManager>> {
        Some(self.shared_session.as_ref()?.presence_manager().clone())
    }

    pub fn shared_session_id(&self) -> Option<&SessionId> {
        Some(self.shared_session.as_ref()?.session_id())
    }

    fn shared_session_source_type(&self) -> Option<&SessionSourceType> {
        Some(self.shared_session.as_ref()?.source_type())
    }

    pub(crate) fn is_shared_session_for_ambient_agent(&self) -> bool {
        matches!(
            self.shared_session_source_type(),
            Some(SessionSourceType::AmbientAgent { .. })
        )
    }

    fn handle_participant_avatar_event(
        &mut self,
        event: &ParticipantAvatarEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ParticipantAvatarEvent::ScrollToSharedSessionParticipant { participant_id } => {
                self.scroll_to_shared_session_participant_selection(participant_id, ctx);
            }
            ParticipantAvatarEvent::MenuOpened { participant_id } => {
                // Ensure only one context menu is open at a time
                if let Some(shared_session) = &self.shared_session {
                    for (avatar_participant_id, participant) in shared_session.viewers() {
                        if participant_id != avatar_participant_id {
                            participant.avatar.update(ctx, |avatar, ctx| {
                                avatar.close_context_menu(ctx);
                            });
                        }
                    }
                }
            }
            // ParticipantAvatarEvent::MenuClosed is not handled in the match statement
            // since it only needs to trigger a pane header re-render which is called for every event.
            _ => {}
        }

        self.update_shared_session_pane_header(ctx);
    }

    pub fn update_role(
        &mut self,
        participant_id: ParticipantId,
        role: Role,
        ctx: &mut ViewContext<Self>,
    ) {
        self.on_participant_role_changed(&participant_id, role, ctx);
        ctx.emit(Event::UpdateRole {
            participant_id,
            role,
        });
    }

    fn refresh_input_data_for_participants(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(shared_session) = &self.shared_session else {
            return;
        };
        let presence_manager = shared_session.presence_manager().clone();
        for participant in presence_manager
            .as_ref(ctx)
            .all_present_participants()
            .cloned()
            .collect_vec()
        {
            let (input_replica_id, cursor_data) = presence_manager
                .as_ref(ctx)
                .input_data_for_participant(&participant);
            let replica_id = ReplicaId::from(input_replica_id);
            self.input().update(ctx, |input, ctx| {
                input.editor().update(ctx, |editor, ctx| {
                    editor.set_remote_peer_selection_data(&replica_id, cursor_data, ctx);
                });
            });
        }
        ctx.notify();
    }

    fn update_shared_session_pane_header(&mut self, _ctx: &mut ViewContext<Self>) {
        // OpenWarp Phase 2a: pane-header sharing UI is gone, so the pane no
        // longer tracks `ShareableObject::Session`. The shared-session itself
        // still runs; it just doesn't surface a "share" button in the header.
    }

    // OpenWarp:Share Session 路径已切断,下面两个方法保留签名但 no-op,
    // 不再 emit `Event::OpenShareSessionModal{,DeniedModal}`,也不再触达云端协同会话服务。
    pub fn open_share_session_modal(
        &mut self,
        _open_source: SharedSessionActionSource,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    pub fn open_share_session_denied_modal(&mut self, _ctx: &mut ViewContext<Self>) {}

    /// Focuses the view by telling the parent view to focus this session.
    /// For example, in the common case, the parent pane group would consume
    /// this event and focus the pane that this session lives in.
    pub fn focus_shared_session(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.windows().show_window_and_focus_app(ctx.window_id());
        ctx.emit(Event::FocusSession);
    }

    /// The entrypoint to start a shared session: all attempts to start a shared session must
    /// go through this API! This is important to guarantee that the right session is being shared.
    /// The TerminalView is responsible for decorating the terminal to reflect its shared status and for
    /// emitting the appropriate events for its terminal manager to setup the appropriate facilities for
    /// sharing to work.
    ///
    /// Specifically, this is the data flow to start a shared session:
    /// 1. User attempts to start a shared session (i.e. this API)
    /// 2. We emit an event that the `shared_session::sharer::Network` model (configured by TerminalManager) picks up.
    /// 3. The `Network` model attempts to establish a shared session connection
    ///    with the server. Once established, it emits an event back.
    /// 4. The TerminalManager handles this event by
    ///    a. Updating the shared session status in the TerminalModel
    ///    b. Registering the shared session with the [`shared_session::manager::Manager`]
    ///    c. Calling into [`TerminalView::on_session_share_started`]
    /// 5. Once the session is registered with [`shared_session::manager::Manager`], it
    ///    will emit an event for relevant subscribers (e.g. the Workspace will need to
    ///    re-render when a share starts for tab indicator, share button, etc.)
    // OpenWarp:Shared Session 网络入口已切断,attempt_to_share_session 整体 no-op,
    // 不再 set SharePending 状态、不再 emit StartSharingCurrentSession、不再触发遥测。
    pub fn attempt_to_share_session(
        &mut self,
        _scrollback_type: SharedSessionScrollbackType,
        _source: Option<SharedSessionActionSource>,
        _source_type: SessionSourceType,
        _bypass_conversation_guard: bool,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    /// Sets the PresenceManager and decorates the view accordingly when a shared session has been started.
    pub fn on_session_share_started(
        &mut self,
        sharer_id: ParticipantId,
        firebase_uid: UserUid,
        scrollback_type: SharedSessionScrollbackType,
        session_id: SessionId,
        source_type: SessionSourceType,
        ctx: &mut ViewContext<Self>,
    ) {
        let started_at = Local::now();
        // TODO(openwarp-cloud-removal Phase 5): `self_handle` 原本喂给 ShareableObject::Session
        // 用于 sharing UI 反查 pane;sharing UI 已删但 shared_session 整条链路仍在,
        // 完整退役 shared_session 时再删这个 ctx.handle() 调用。
        let _self_handle = ctx.handle();
        let adapter = Adapter::new_for_sharer(
            sharer_id,
            firebase_uid,
            session_id,
            started_at,
            source_type,
            ctx,
        );
        let presence_manager = adapter.presence_manager().clone();

        self.shared_session = Some(adapter);
        self.reset_sharer_inactivity_timer(ctx);
        self.input.update(ctx, |input, _| {
            input.set_shared_session_presence_manager(presence_manager);
        });
        let share_source = self.pending_share_source.take();
        let is_remote_control = matches!(share_source, Some(SharedSessionActionSource::FooterChip));
        self.insert_shared_session_started_banner(
            scrollback_type,
            is_remote_control,
            started_at,
            ctx,
        );

        self.pane_configuration.update(ctx, |pane_config, ctx| {
            pane_config.refresh_pane_header_overflow_menu_items(ctx);
            // OpenWarp Phase 2a: sharing dialog + pane-header `ShareableObject`
            // bookkeeping removed; the shared session continues without a UI
            // entry point.
            pane_config.notify_header_content_changed(ctx);
        });
    }

    /// The entrypoint to stop a shared session: all attempts to stop a shared session must
    /// go through this API! This is important to guarantee that we correctly stop the share.
    pub fn stop_sharing_session(&mut self, ctx: &mut ViewContext<Self>) {
        self.stop_sharing_session_for_reason(SessionEndedReason::EndedBySharer, ctx);
    }

    fn stop_sharing_session_for_reason(
        &mut self,
        reason: SessionEndedReason,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.emit(Event::StopSharingCurrentSession { reason });
    }

    // TODO: why do we need to pass through input replica ID as a separate argument?
    // It should be in `participant_list`.
    #[allow(clippy::too_many_arguments)]
    pub fn on_session_share_joined(
        &mut self,
        viewer_id: ParticipantId,
        firebase_uid: UserUid,
        input_replica_id: ReplicaId,
        participant_list: Box<ParticipantList>,
        session_id: SessionId,
        source_type: SessionSourceType,
        ctx: &mut ViewContext<Self>,
    ) {
        let started_at = Local::now();
        // TODO(openwarp-cloud-removal Phase 5): `self_handle` 原本喂给 ShareableObject::Session
        // 用于 sharing UI 反查 pane;sharing UI 已删但 shared_session 整条链路仍在,
        // 完整退役 shared_session 时再删这个 ctx.handle() 调用。
        let _self_handle = ctx.handle();
        let adapter = Adapter::new_for_viewer(
            viewer_id.clone(),
            firebase_uid,
            participant_list,
            session_id,
            started_at,
            source_type.clone(),
            ctx,
        );
        let presence_manager = adapter.presence_manager().clone();
        let role = presence_manager.as_ref(ctx).role();
        self.shared_session = Some(adapter);

        self.insert_shared_session_started_banner(
            SharedSessionScrollbackType::All,
            false,
            started_at,
            ctx,
        );

        self.input.update(ctx, |input, ctx| {
            input.on_session_share_joined(input_replica_id, presence_manager, ctx);
        });

        // Mark this terminal as a viewer for chips and AI context menu once on join
        let is_ambient = self.ambient_agent_view_model.as_ref(ctx).is_ambient_agent();
        self.input().update(ctx, |input, ctx| {
            input
                .prompt_render_helper
                .prompt_view()
                .update(ctx, |prompt_display, ctx| {
                    prompt_display.update_shared_session_viewer_status(true, ctx);
                });

            input.editor().update(ctx, |editor, ctx| {
                if let Some(ai_context_menu) = editor.ai_context_menu() {
                    ai_context_menu.update(ctx, |menu, ctx| {
                        menu.set_is_shared_session_viewer(true, ctx);
                        menu.set_is_in_ambient_agent(is_ambient, ctx);
                    });
                }
            });
        });

        // If viewer joined as an executor, make sure the view state is updated.
        if let Some(role) = role {
            self.on_self_role_updated(role, ctx);
        }

        self.pane_configuration.update(ctx, |pane_config, ctx| {
            pane_config.refresh_pane_header_overflow_menu_items(ctx);
            // OpenWarp Phase 2a: removed `set_shareable_object` (cloud sharing UI gone).
            pane_config.notify_header_content_changed(ctx);
        });

        // When we join a shared session, we get a snapshot of the sharer's chip states,
        // including the working directory chip. We can use this chip value to set the terminal title
        // with the correct pwd on-join (even if there is no active block yet to populate the TerminalView's pwd).
        if let Some(pwd) = self
            .current_prompt
            .as_ref(ctx)
            .latest_chip_value(&ContextChipKind::WorkingDirectory, ctx)
        {
            self.terminal_title = pwd.to_string();
        }

        // Update the pane title, which will show either the conversation title/status
        // if there's an active conversation, or fall back to the terminal_title (pwd).
        self.update_pane_configuration(ctx);

        self.update_shared_session_pane_header(ctx);
    }

    /// Clear the presence manager and handle any UI necessary on shared session end.
    /// Applies to both sharer and viewer when the session sharing ends.
    pub fn on_session_share_ended(&mut self, ctx: &mut ViewContext<Self>) {
        let should_insert_tombstone = {
            let model = self.model.lock();
            false
                && model.is_shared_ambient_agent_session()
                && !self.has_inserted_conversation_ended_tombstone
                && !model.is_receiving_agent_conversation_replay()
        };
        if should_insert_tombstone {
            self.insert_conversation_ended_tombstone(ctx);
        }
        // Ensure inactivity timer is aborted for sharer
        if let Some(sharer) = self.shared_session_sharer_mut() {
            if let Some(old_abort_handle) = sharer.inactivity_timer_abort_handle.take() {
                old_abort_handle.abort();
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.active_viewer_driven_size.is_some() {
            self.restore_pty_to_sharer_size(ctx);
        }

        self.shared_session = None;
        self.insert_shared_session_ended_banner(ctx);
        self.on_shared_session_reconnection_status_changed(false, ctx);

        self.input().update(ctx, |input, ctx| {
            input.editor().update(ctx, |editor, ctx| {
                editor.unregister_all_remote_peers(ctx);
            });
        });

        // When the session is ended, the input should be uneditable iff this is a viewer.
        if self.model.lock().shared_session_status().is_viewer() {
            self.input().update(ctx, |input, ctx| {
                input.editor().update(ctx, |editor, ctx| {
                    editor.set_interaction_state(InteractionState::Selectable, ctx);
                });
            });
        }

        self.pane_configuration.update(ctx, |pane_config, ctx| {
            pane_config.refresh_pane_header_overflow_menu_items(ctx);
            pane_config.notify_header_content_changed(ctx);
            ctx.notify();
        });
    }

    pub fn handle_inactivity_modal_event(
        &mut self,
        event: &InactivityModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(sharer) = self.shared_session_sharer_mut() else {
            return;
        };
        sharer.close_inactivity_warning_modal();
        ctx.notify();

        match event {
            InactivityModalEvent::TimedOut => self.end_session_on_inactivity_period_expired(ctx),
            InactivityModalEvent::StopSharing => self.stop_sharing_session(ctx),
            InactivityModalEvent::ContinueSharing => self.reset_sharer_inactivity_timer(ctx),
        }
    }

    fn end_session_on_inactivity_period_expired(&mut self, ctx: &mut ViewContext<Self>) {
        self.stop_sharing_session_for_reason(SessionEndedReason::InactivityLimitReached, ctx);
        self.show_persistent_toast(
            "Sharing ended due to inactivity".to_owned(),
            ToastFlavor::Error,
            ctx,
        );
    }

    fn show_warning_on_inactivity_period_expired(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(sharer) = self.shared_session_sharer_mut() else {
            return;
        };
        // Ensure warning modal isn't already open
        if !sharer.is_inactivity_warning_modal_open {
            sharer.open_inactivity_warning_modal(ctx);
            ctx.notify();
        }
    }

    fn set_inactivity_timer_to_show_warning(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(sharer) = self.shared_session_sharer_mut() else {
            return;
        };

        // After the second interval of inactivity, we display a warning modal
        let inactivity_period = SharedSessionSettings::as_ref(ctx)
            .inactivity_period_between_revoking_roles_and_warning();
        let timer_handler = ctx.spawn_abortable(
            Timer::after(inactivity_period),
            move |me, _, ctx| me.show_warning_on_inactivity_period_expired(ctx),
            |_, _| {},
        );
        sharer.inactivity_timer_abort_handle = Some(timer_handler);
    }

    fn revoke_roles_on_inactivity_period_expired(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(shared_session) = self.shared_session.as_mut() else {
            return;
        };

        // Ensure executors exist
        let num_executors = shared_session.presence_manager().read(ctx, |manager, _| {
            manager
                .get_present_viewers()
                .filter(|viewer| viewer.role.is_some_and(|r| r.can_execute()))
                .count()
        });
        if num_executors > 0 {
            self.make_all_shared_session_participants_readers(
                RoleUpdateReason::InactivityLimitReached,
                ctx,
            );
            self.show_persistent_toast(
                "Shared editing permissions were revoked due to inactivity".to_owned(),
                ToastFlavor::Error,
                ctx,
            );
        }

        // Set timer for second interval
        self.set_inactivity_timer_to_show_warning(ctx);
    }

    /// Resets sharer's inactivity timer
    /// (1) After the first interval, we revoke all executor permissions
    /// (2) After the second interval, we show a warning modal
    /// (3) After the third interval, we end the session
    pub fn reset_sharer_inactivity_timer(&mut self, ctx: &mut ViewContext<Self>) {
        // For ambient agent shared sessions, we do not auto-revoke roles or end the
        // session due to inactivity. Clear any existing timer and return early so
        // the session stays open until explicitly closed.
        if self.model.lock().is_shared_ambient_agent_session() {
            if let Some(sharer) = self.shared_session_sharer_mut() {
                if let Some(old_abort_handle) = sharer.inactivity_timer_abort_handle.take() {
                    old_abort_handle.abort();
                }
            }
            return;
        }

        let Some(sharer) = self.shared_session_sharer_mut() else {
            return;
        };

        // Ignore timer resets from throttled activity when warning modal is open.
        // User must explicitly close modal to continue the session.
        if sharer.is_inactivity_warning_modal_open {
            return;
        }

        if let Some(old_abort_handle) = sharer.inactivity_timer_abort_handle.take() {
            old_abort_handle.abort();
        }

        // After the first interval of inactivity, we revoke all executor permissions
        let inactivity_period = SharedSessionSettings::as_ref(ctx)
            .inactivity_period_before_revoking_roles
            .value();
        let timer_handler = ctx.spawn_abortable(
            Timer::after(*inactivity_period),
            move |me, _, ctx| me.revoke_roles_on_inactivity_period_expired(ctx),
            |_, _| {},
        );
        sharer.inactivity_timer_abort_handle = Some(timer_handler);
    }

    pub fn get_shared_session_presence_selection(
        &self,
        ctx: &AppContext,
    ) -> crate::terminal::shared_session::protocol::Selection {
        let model_lock = self.model.lock();
        let input_mode = *InputModeSettings::as_ref(ctx).input_mode.value();
        let semantic_selection = SemanticSelection::as_ref(ctx);

        // First check if we have any selected blocks.
        let selected_block_ids = self
            .selected_blocks
            .to_block_ids(model_lock.block_list())
            .map(|id| id.to_string().into())
            .collect_vec();
        if !selected_block_ids.is_empty() {
            return crate::terminal::shared_session::protocol::Selection::Blocks {
                block_ids: selected_block_ids,
            };
        }

        // Then check if we have selected text in the alt screen or block list.
        if model_lock.is_alt_screen_active() {
            if let Some(selection_range) =
                model_lock.alt_screen().selection_range(semantic_selection)
            {
                return crate::terminal::shared_session::protocol::Selection::AltScreenText {
                    start: point_to_session_sharing(*selection_range.start()),
                    end: point_to_session_sharing(*selection_range.end()),
                    is_reversed: selection_range.is_reversed(),
                };
            }
        } else if let Some((start, end, is_reversed)) = model_lock
            .block_list()
            .text_selection_range(semantic_selection, input_mode.is_inverted_blocklist())
        {
            let Some(start) = start.to_session_sharing_block_point(model_lock.block_list()) else {
                log::error!("Failed convert start of selection range to BlockPoint");
                return crate::terminal::shared_session::protocol::Selection::None;
            };
            let Some(end) = end.to_session_sharing_block_point(model_lock.block_list()) else {
                log::error!("Failed convert end of selection range to BlockPoint");
                return crate::terminal::shared_session::protocol::Selection::None;
            };
            return crate::terminal::shared_session::protocol::Selection::BlockText {
                start,
                end,
                is_reversed,
            };
        }
        crate::terminal::shared_session::protocol::Selection::None
    }

    pub fn handle_presence_manager_event(
        &mut self,
        event: &PresenceManagerEvent,
        presence_manager: ModelHandle<PresenceManager>,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(shared_session) = self.shared_session.as_mut() else {
            log::warn!("Received presence manager event for a session that isn't shared");
            return;
        };

        match event {
            // TODO(suraj): improve the diff approach.
            PresenceManagerEvent::ParticipantListUpdated => {
                // Make sure all the absent viewers have been removed.
                for viewer in presence_manager
                    .as_ref(ctx)
                    .absent_viewers()
                    .cloned()
                    .collect_vec()
                {
                    if !shared_session.viewers().contains_key(viewer.id()) {
                        continue;
                    }

                    shared_session.remove_viewer(viewer.id());
                    self.input.update(ctx, |input, ctx| {
                        input.editor().update(ctx, |editor, ctx| {
                            let replica_id = (viewer.input_replica_id().clone()).into();
                            editor.unregister_remote_peer(&replica_id, ctx);
                        });
                    });
                }

                // Make sure all the active viewers are added.
                let active_viewers = presence_manager
                    .as_ref(ctx)
                    .get_present_viewers()
                    .cloned()
                    .collect_vec();
                let is_self_sharer = shared_session.kind().is_sharer();
                let is_reconnecting = presence_manager.as_ref(ctx).is_reconnecting();
                for viewer in active_viewers {
                    if let Some(existing_viewer) = shared_session.viewers().get(viewer.id()) {
                        // A change to the viewer's ACL may have originated from
                        // warp-server, so we need to update the avatar's role.
                        existing_viewer.avatar.update(ctx, |avatar, ctx| {
                            if avatar.role() != viewer.role {
                                avatar.set_role(viewer.role);
                                ctx.notify();
                            }
                        });
                        continue;
                    }

                    let pane_header_avatar = ctx.add_typed_action_view(|ctx| {
                        ParticipantAvatarView::new(
                            is_self_sharer,
                            viewer.info.clone(),
                            viewer.color,
                            is_reconnecting,
                            viewer.role,
                            ctx,
                        )
                    });
                    ctx.subscribe_to_view(&pane_header_avatar, |me, _, event, ctx| {
                        me.handle_participant_avatar_event(event, ctx);
                    });
                    shared_session.add_viewer(viewer.id().to_owned(), pane_header_avatar);

                    let (input_replica_id, cursor_data) = presence_manager
                        .as_ref(ctx)
                        .input_data_for_participant(&viewer);
                    self.input.update(ctx, |input, ctx| {
                        input.editor().update(ctx, |editor, ctx| {
                            editor.register_remote_peer(input_replica_id.into(), cursor_data, ctx);
                        });
                    });
                }

                if let Some(sharer) = presence_manager.as_ref(ctx).get_sharer().cloned() {
                    if let Kind::Viewer(v) = shared_session.kind_mut() {
                        let pane_header_avatar = ctx.add_typed_action_view(|ctx| {
                            ParticipantAvatarView::new(
                                is_self_sharer,
                                sharer.info.clone(),
                                sharer.color,
                                is_reconnecting,
                                None,
                                ctx,
                            )
                        });
                        ctx.subscribe_to_view(&pane_header_avatar, |me, _, event, ctx| {
                            me.handle_participant_avatar_event(event, ctx);
                        });
                        v.sharer = Some(Participant::new(pane_header_avatar));
                    }

                    let (input_replica_id, cursor_data) = presence_manager
                        .as_ref(ctx)
                        .input_data_for_participant(&sharer);
                    self.input.update(ctx, |input, ctx| {
                        input.editor().update(ctx, |editor, ctx| {
                            editor.register_remote_peer(input_replica_id.into(), cursor_data, ctx);
                        });
                    });
                }
            }
        }

        self.update_shared_session_pane_header(ctx);

        // Notify the pane header that its content has changed and needs to re-render.
        self.pane_configuration.update(ctx, |config, ctx| {
            config.notify_header_content_changed(ctx);
        });
    }

    fn scroll_to_shared_session_participant_selection(
        &mut self,
        participant_id: &ParticipantId,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(participant) = self
            .shared_session_presence_manager()
            .as_ref()
            .and_then(|pm| pm.as_ref(ctx).get_participant(participant_id))
        else {
            return;
        };

        // If we the participant has block(s) selected, scroll to the block where the avatar is.
        // Otherwise, if the participant has block text selected, scroll so the cursor is in view.
        if let Some(block_index) = {
            let index =
                participant.get_selected_block_index_for_avatar(self.model.lock().block_list());
            index
        } {
            self.update_scroll_position_locking(
                ScrollPositionUpdate::ScrollToTopOfBlockWithBuffer {
                    block_index,
                    buffer_lines: 2.into_lines(),
                },
                ctx,
            );
        } else if let crate::terminal::shared_session::protocol::Selection::BlockText {
            start,
            end,
            is_reversed,
        } = &participant.info.selection
        {
            let cursor_point = if *is_reversed { start } else { end };
            let Some(within_block_point) = WithinBlock::<Point>::from_session_sharing_block_point(
                cursor_point.clone(),
                self.model.lock().block_list(),
            ) else {
                return;
            };
            let block_list_point = BlockListPoint::from_within_block_point(
                &within_block_point,
                self.model.lock().block_list(),
            );
            self.update_scroll_position_locking(
                ScrollPositionUpdate::ScrollToBlocklistRowIfNotVisible {
                    row: block_list_point.row.into_lines(),
                },
                ctx,
            );
        } else {
            return;
        }
    }

    // If open, ensure that participant avatar context menu is not triggered
    pub fn pane_header_overflow_menu_toggled(
        &mut self,
        is_open: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(shared_session) = self.shared_session.as_mut() {
            for viewer in shared_session.viewers().values() {
                viewer.avatar.update(ctx, |avatar, _| {
                    avatar.set_is_pane_header_overflow_menu_open(is_open);
                });
            }
        }
    }

    pub fn make_all_shared_session_participants_readers(
        &mut self,
        reason: RoleUpdateReason,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(shared_session) = self.shared_session.as_mut() {
            if !shared_session.kind().is_sharer() {
                return;
            }

            shared_session
                .presence_manager()
                .update(ctx, |manager, ctx| {
                    manager.make_all_participants_readers(ctx);
                });

            for viewer in shared_session.viewers().values() {
                viewer.avatar.update(ctx, |avatar, ctx| {
                    avatar.set_role(Some(Role::Reader));
                    ctx.notify();
                });
            }
        }

        self.update_shared_session_pane_header(ctx);
        log::warn!("Ignoring removed shared session revoke-all network update: {reason:?}");
    }

    // Called when viewer receives acknowledgment from server
    // on role request status (in flight, or failed)
    /// Updates view state when our own role was changed.
    fn on_self_role_updated(&mut self, role: Role, ctx: &mut ViewContext<Self>) {
        // Update shared session status only if we are an active viewer.
        // This avoids a race condition if a viewer receives a role change
        // before catching up, by ensuring the view is still pending.
        if self.model.lock().shared_session_status().is_active_viewer() {
            // If not an active viewer now, role and status will be updated
            // in the call `process_ordered_terminal_event`.
            self.model
                .lock()
                .set_shared_session_status(SharedSessionStatus::ActiveViewer { role });
        }

        // Enable/disable the editor based on the new role
        self.input().update(ctx, |input, ctx| {
            input.editor().update(ctx, |editor, ctx| {
                let role = &role;
                editor.set_interaction_state(role.into(), ctx);
            });
        });
    }

    fn insert_shared_session_started_banner(
        &mut self,
        scrollback_type: SharedSessionScrollbackType,
        is_remote_control: bool,
        started_at: DateTime<Local>,
        ctx: &mut ViewContext<Self>,
    ) {
        let banner_id = self.inline_banners_state.next_banner_id();

        let mut model = self.model.lock();

        // TODO: technically the first block index could change between the time we insert
        // the banner and the time we actually compute the scrollback.
        let block_index = scrollback_type.first_block_index(&model);

        // Remove any existing banners if any.
        if let SharedSessionBanners::LastShared {
            started_banner_id,
            ended_banner_id,
            ..
        } = self.inline_banners_state.shared_session_banner_state
        {
            model
                .block_list_mut()
                .remove_inline_banner(started_banner_id);
            model.block_list_mut().remove_inline_banner(ended_banner_id);
        }

        self.inline_banners_state.shared_session_banner_state = SharedSessionBanners::ActiveShare {
            started_banner_id: banner_id,
            started_at,
            is_remote_control,
        };

        model.block_list_mut().insert_inline_banner_before_block(
            block_index,
            InlineBannerItem::new(banner_id, InlineBannerType::SharedSessionStart),
            None,
        );

        ctx.notify();
    }

    pub fn on_participant_presence_updated(
        &mut self,
        update: &ParticipantPresenceUpdate,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(presence_manager) = &self.shared_session_presence_manager() else {
            return;
        };
        let input_data = presence_manager.update(ctx, |manager, ctx| {
            manager.update_participant_presence(update.to_owned(), ctx);
            manager
                .get_participant(&update.participant_id)
                .map(|participant| manager.input_data_for_participant(participant))
        });

        if let Some((input_replica_id, cursor_data)) = input_data {
            let replica_id = ReplicaId::from(input_replica_id);
            self.input.update(ctx, |input, ctx| {
                input.editor().update(ctx, |editor, ctx| {
                    editor.set_remote_peer_selection_data(&replica_id, cursor_data, ctx);
                });
            });
        }
        ctx.notify();
    }

    /// Only show toast if role is new and reason is valid.
    pub fn maybe_show_role_changed_toast(
        &mut self,
        participant_id: &ParticipantId,
        reason: RoleUpdatedReason,
        new_role: Role,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(presence_manager) = self.shared_session_presence_manager() else {
            return;
        };
        let is_self_role_updated = participant_id == &presence_manager.as_ref(ctx).id();
        let is_new_role_reader = match presence_manager.as_ref(ctx).role() {
            Some(old_role) => old_role.can_execute() && matches!(new_role, Role::Reader),
            None => false,
        };

        if is_self_role_updated
            && is_new_role_reader
            && matches!(reason, RoleUpdatedReason::InactivityLimitReached)
        {
            self.show_persistent_toast(
                "Editing permissions were revoked because the sharer is idle".to_owned(),
                ToastFlavor::Error,
                ctx,
            );
        }
    }

    // Called by both sharer and viewer when a participant's role has changed.
    pub fn on_participant_role_changed(
        &mut self,
        participant_id: &ParticipantId,
        new_role: Role,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(shared_session) = self.shared_session.as_mut() {
            shared_session.update_participant_role(participant_id, new_role, ctx);

            let is_self = *participant_id == shared_session.presence_manager().as_ref(ctx).id();
            if is_self {
                self.on_self_role_updated(new_role, ctx);
            }
        }
        self.update_shared_session_pane_header(ctx);
    }

    pub fn on_self_role_maybe_changed(
        &mut self,
        participant_list: &ParticipantList,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(shared_session) = self.shared_session.as_ref() else {
            return;
        };
        let presence_manager = shared_session.presence_manager().as_ref(ctx);
        let self_id = presence_manager.id();
        let Some(existing_role) = presence_manager.role() else {
            return;
        };

        let Some(new_role) = participant_list
            .present_viewers
            .iter()
            .find(|v| v.info.id == self_id)
            .map(|v| v.max_acl)
        else {
            log::warn!("Could not find new role for viewer {self_id:?} in participant list");
            return;
        };

        if existing_role != new_role {
            self.on_self_role_updated(new_role, ctx);
        }
    }

    pub fn insert_shared_session_ended_banner(&mut self, ctx: &mut ViewContext<Self>) {
        let banner_id = self.inline_banners_state.next_banner_id();
        let banner = InlineBannerItem::new(banner_id, InlineBannerType::SharedSessionEnd);

        if let SharedSessionBanners::ActiveShare {
            started_banner_id,
            started_at,
            is_remote_control,
        } = self.inline_banners_state.shared_session_banner_state
        {
            self.inline_banners_state.shared_session_banner_state =
                SharedSessionBanners::LastShared {
                    started_banner_id,
                    started_at,
                    is_remote_control,
                    ended_at: Local::now(),
                    ended_banner_id: banner_id,
                };
        }

        let mut model = self.model.lock();
        if model.shared_session_status().is_active_viewer() {
            // For viewers, the banner goes after the long running block so no content appears after the banner.
            model
                .block_list_mut()
                .append_inline_banner_after_long_running(banner);
        } else {
            // For sharers, it goes before the long running block so the banner doesn't end up pinned at the bottom while the block above changes.
            model.block_list_mut().append_inline_banner(banner);
        }

        ctx.notify();
    }

    pub fn insert_conversation_ended_tombstone(&mut self, ctx: &mut ViewContext<Self>) {
        let task_id = self.model.lock().ambient_agent_task_id();
        let terminal_view_id = self.id();

        let tombstone_view_handle = ctx.add_typed_action_view(|ctx| {
            ConversationEndedTombstoneView::new(ctx, terminal_view_id, task_id)
        });
        self.insert_rich_content(
            None,
            tombstone_view_handle,
            None,
            RichContentInsertionPosition::Append {
                insert_below_long_running_block: true,
            },
            ctx,
        );
        self.has_inserted_conversation_ended_tombstone = true;
    }

    /// Updates shared session reconnection banner, participant avatars and
    /// input interaction state depending on the reconnection state.
    pub fn on_shared_session_reconnection_status_changed(
        &mut self,
        is_reconnecting: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if is_reconnecting
            && !self
                .model
                .lock()
                .shared_session_status()
                .is_sharer_or_viewer()
        {
            log::warn!(
                "Tried to open shared session reconnecting banner for a session that isn't shared"
            );
            return;
        }

        if let Some(shared_session) = self.shared_session.as_mut() {
            shared_session.on_reconnection_status_changed(is_reconnecting, ctx);
        }

        // Input is disabled for an offline executor and re-enabled when back online.
        if self.model.lock().shared_session_status().is_executor() {
            let interaction_state = if is_reconnecting {
                InteractionState::Selectable
            } else {
                InteractionState::Editable
            };
            self.input().update(ctx, |input, ctx| {
                input.editor().update(ctx, |editor, ctx| {
                    editor.set_interaction_state(interaction_state, ctx);
                });
            });
        }

        self.refresh_input_data_for_participants(ctx);
        self.update_shared_session_pane_header(ctx);
        ctx.notify();
    }

    pub fn session_sharing_context_menu_items(
        &self,
        model: &TerminalModel,
        is_share_session_disabled: bool,
    ) -> Vec<MenuItem<TerminalAction>> {
        let mut items = Vec::new();

        let _ = is_share_session_disabled;
        if model.shared_session_status().is_active_sharer() {
            items.push(
                MenuItemFields::new(crate::t!("terminal-stop-sharing"))
                    .with_on_select_action(TerminalAction::ContextMenu(
                        ContextMenuAction::StopSharing,
                    ))
                    .into_item(),
            );
        }

        items
    }

    /// Resizes the terminal from when the sharer updates size.
    pub fn resize_from_sharer_update(
        &mut self,
        new_sharer_size: WindowSize,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(viewer) = self.shared_session_viewer_mut() {
            viewer.sharer_size = Some(new_sharer_size);

            let size_update = SizeUpdateBuilder::for_shared_session_update(
                *self.size_info,
                new_sharer_size.num_rows,
                new_sharer_size.num_cols,
            )
            .build(self, ctx);
            self.resize_internal(size_update, ctx);
        }
    }

    /// Returns true if viewer-driven sizing should be active.
    /// For ambient-agent sessions, the same-user identity check is skipped.
    /// Otherwise, conditions: exactly 1 viewer, and that viewer is the same user as the sharer.
    pub(crate) fn is_viewer_driven_sizing_eligible(
        &self,
        is_sharer: bool,
        ctx: &ViewContext<Self>,
    ) -> bool {
        let skip_uid_check = self.is_shared_session_for_ambient_agent();
        self.shared_session_presence_manager()
            .map(|manager| {
                let manager = manager.as_ref(ctx);
                if is_sharer {
                    let one_viewer = manager.present_viewer_count() == 1;
                    one_viewer
                        && (skip_uid_check
                            || manager.get_present_viewers().all(|v| {
                                v.info.profile_data.firebase_uid
                                    == manager.firebase_uid().as_string()
                            }))
                } else {
                    // We are a viewer — we must be the only viewer and the sharer must be us.
                    // (present_viewer_count() excludes ourselves)
                    let only_viewer = manager.present_viewer_count() == 0;
                    only_viewer
                        && (skip_uid_check
                            || manager.get_sharer().is_some_and(|s| {
                                s.info.profile_data.firebase_uid
                                    == manager.firebase_uid().as_string()
                            }))
                }
            })
            .unwrap_or(false)
    }

    /// Restores the PTY to the sharer's own terminal size by refreshing
    /// through the normal resize pipeline.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn restore_pty_to_sharer_size(&mut self, ctx: &mut ViewContext<Self>) {
        self.active_viewer_driven_size = None;
        self.refresh_size(ctx);
    }

    /// Forces a fresh viewer-size report to the sharer by clearing the dedup cache and
    /// refreshing size. No-op when not an active viewer or when viewer-driven sizing is
    /// not eligible. Used when a new process (e.g. the harness CLI starting for a non-oz
    /// Cloud Mode run) needs the sharer to resize its PTY so the new process picks up
    /// correct terminal dimensions at startup.
    pub(in crate::terminal::view) fn force_report_viewer_terminal_size(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(viewer) = self.shared_session_viewer_mut() {
            viewer.last_reported_natural_size = None;
        }
        self.refresh_size(ctx);
    }

    /// Resizes the sharer's terminal to match the viewer's reported size,
    /// going through the normal view/model/PTY resize pipeline.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn resize_from_viewer_report(
        &mut self,
        viewer_size: WindowSize,
        ctx: &mut ViewContext<Self>,
    ) {
        self.active_viewer_driven_size = Some((viewer_size.num_rows, viewer_size.num_cols));
        let size_update = SizeUpdateBuilder::for_viewer_size_report(
            *self.size_info,
            viewer_size.num_rows,
            viewer_size.num_cols,
        )
        .build(self, ctx);
        self.resize_internal(size_update, ctx);
    }
}

#[cfg(test)]
#[path = "view_impl_test.rs"]
mod tests;
use crate::auth::UserUid;
