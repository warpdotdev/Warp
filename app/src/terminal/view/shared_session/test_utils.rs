use std::{collections::HashMap, sync::Arc};

use crate::terminal::shared_session::protocol::SessionSourceType;
use crate::terminal::shared_session::protocol::{ParticipantId, ParticipantList, SessionId};
use warpui::platform::WindowStyle;
use warpui::{App, ViewHandle};

use crate::auth::UserUid;
use crate::editor::ReplicaId;
use crate::pane_group::{NewTerminalOptions, PaneGroup, PanesLayout};
use crate::terminal::TerminalView;
use crate::test_util::terminal::initialize_app_for_terminal_view;
use crate::GlobalResourceHandles;

/// Creates a terminal view that is created via the terminal manager
/// for shared session viewers. That is, it has all of the relevant models
/// set up for the viewer.
pub fn terminal_view_for_viewer(app: &mut App) -> ViewHandle<TerminalView> {
    initialize_app_for_terminal_view(app);

    let global_resource_handles = GlobalResourceHandles::mock(app);
    let GlobalResourceHandles {
        model_event_sender,
        tips_completed,
        user_default_shell_unsupported_banner_model_handle,
        ..
    } = global_resource_handles.clone();

    let (_, pane_group) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
        PaneGroup::new_with_panes_layout(
            tips_completed,
            user_default_shell_unsupported_banner_model_handle,
            PanesLayout::SingleTerminal(Box::<NewTerminalOptions>::default()),
            Arc::new(HashMap::new()),
            model_event_sender,
            ctx,
        )
    });

    let terminal = pane_group.read(app, |pane_group, ctx| {
        pane_group
            .terminal_view_at_pane_index(0, ctx)
            .expect("TerminalView exists at pane index 0")
            .to_owned()
    });

    let user_uid = UserUid::new("mock_user_uid");
    terminal.update(app, |view, ctx| {
        view.on_session_share_joined(
            ParticipantId::new(),
            user_uid,
            ReplicaId::random(),
            Box::new(ParticipantList::default()),
            SessionId::new(),
            SessionSourceType::default(),
            ctx,
        );
    });

    terminal
}
