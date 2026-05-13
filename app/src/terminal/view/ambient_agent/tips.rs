//! Local fallback tips for the ambient-agent loading screen.

use crate::ai::agent_tips::AITip;
use warpui::keymap::Keystroke;
use warpui::AppContext;

/// A local fallback tip with text and optional link.
#[derive(Clone, Debug)]
pub struct CloudModeTip {
    text: String,
    link: Option<String>,
}

impl CloudModeTip {
    pub fn new(text: impl Into<String>, link: Option<impl Into<String>>) -> Self {
        Self {
            text: text.into(),
            link: link.map(|l| l.into()),
        }
    }
}

impl AITip for CloudModeTip {
    fn keystroke(&self, _app: &AppContext) -> Option<Keystroke> {
        None
    }

    fn link(&self) -> Option<String> {
        self.link.clone()
    }

    fn description(&self) -> &str {
        &self.text
    }

    // Uses the default implementation which adds "Tip: " prefix and parses backticks as inline code
}

/// Returns a collection of tips for the ambient-agent loading screen.
pub fn get_cloud_mode_tips() -> Vec<CloudModeTip> {
    Vec::new()
}
