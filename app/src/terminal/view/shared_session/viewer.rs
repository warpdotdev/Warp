use crate::terminal::view::TerminalView;

use crate::terminal::shared_session::protocol::WindowSize;
use warpui::{elements::MouseStateHandle, ViewContext};

use super::adapter::Participant;

pub struct Viewer {
    pub sharer: Option<Participant>,
    pub is_reconnecting: bool,

    pub role_change_menu_button: MouseStateHandle,

    pub sharer_size: Option<WindowSize>,

    /// The last natural size (rows, cols) that was reported to the sharer.
    /// Used to deduplicate ReportViewerTerminalSize events.
    pub last_reported_natural_size: Option<(usize, usize)>,
}

impl Viewer {
    pub fn new(_ctx: &mut ViewContext<TerminalView>) -> Self {
        Self {
            is_reconnecting: false,
            sharer: None,
            role_change_menu_button: Default::default(),
            sharer_size: None,
            last_reported_natural_size: None,
        }
    }

    pub fn set_is_reconnecting(&mut self, is_reconnectiong: bool) {
        self.is_reconnecting = is_reconnectiong;
    }
}
