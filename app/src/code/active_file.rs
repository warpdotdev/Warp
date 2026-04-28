//! Module containing the definition of [`ActiveFileModel`],
//! which tracks the currently focused file across an entire PaneGroup.

use std::path::PathBuf;

use warpui::{Entity, ModelContext};

/// Events emitted by the ActiveFileModel.
#[derive(Debug, Clone)]
pub enum ActiveFileEvent {
    /// A new file became focused.
    ActiveFileChanged { file_info: PathBuf },
}

/// Model that tracks the currently focused file.
#[derive(Default)]
pub struct ActiveFileModel {
    /// The currently focused file, if any.
    active_file: Option<PathBuf>,
}

impl Entity for ActiveFileModel {
    type Event = ActiveFileEvent;
}

impl ActiveFileModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently active file, if any.
    pub fn active_file(&self) -> Option<&PathBuf> {
        self.active_file.as_ref()
    }

    /// Set the currently active file.
    pub fn active_file_changed(&mut self, path: PathBuf, ctx: &mut ModelContext<Self>) {
        // Only emit event if the active file changed.
        if self.active_file.as_ref() != Some(&path) {
            self.active_file = Some(path.clone());
            ctx.emit(ActiveFileEvent::ActiveFileChanged { file_info: path });
            ctx.notify();
        }
    }
}
