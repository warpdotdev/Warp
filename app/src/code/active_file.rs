//! Module containing the definition of [`ActiveFileModel`],
//! which tracks the currently focused file across an entire PaneGroup.

use super::buffer_location::FileLocation;
use warpui::{Entity, ModelContext};

/// Events emitted by the ActiveFileModel.
#[derive(Debug, Clone)]
pub enum ActiveFileEvent {
    /// A new file became focused.
    ActiveFileChanged { location: FileLocation },
}

/// Model that tracks the currently focused file.
#[derive(Default)]
pub struct ActiveFileModel {
    /// The currently focused file, if any.
    active_file: Option<FileLocation>,
}

impl Entity for ActiveFileModel {
    type Event = ActiveFileEvent;
}

impl ActiveFileModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently active file, if any.
    pub fn active_file(&self) -> Option<&FileLocation> {
        self.active_file.as_ref()
    }

    /// Set the currently active file.
    pub fn active_file_changed(&mut self, location: FileLocation, ctx: &mut ModelContext<Self>) {
        // Only emit event if the active file changed.
        if self.active_file.as_ref() != Some(&location) {
            self.active_file = Some(location.clone());
            ctx.emit(ActiveFileEvent::ActiveFileChanged { location });
            ctx.notify();
        }
    }
}
