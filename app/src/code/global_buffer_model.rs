#![cfg_attr(not(feature = "local_fs"), allow(dead_code))]
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bimap::BiMap;

use futures_util::stream::AbortHandle;
use lsp::types::TextDocumentContentChangeEvent;
use lsp::{LspManagerModel, LspServerLogLevel, LspServerModel};
use vec1::vec1;
use warp_core::features::FeatureFlag;
use warp_editor::content::buffer::Buffer;
use warp_editor::content::diff::{text_diff, TextDiff};
use warp_editor::content::edit::PreciseDelta;
use warp_editor::content::version::BufferVersion;
use warp_util::content_version::ContentVersion;
use warp_util::file::{FileId, FileLoadError, FileSaveError};
use warpui::{Entity, ModelContext, ModelHandle, SingletonEntity, WeakModelHandle};

cfg_if::cfg_if! {
    if #[cfg(feature = "local_fs")] {
        use lsp::LspManagerModelEvent;
        use warp_files::{FileModelEvent, FileModel};
        use warp_editor::content::text::IndentBehavior;
        use warp_editor::content::text::IndentUnit;
        use warp_editor::content::buffer::EditOrigin;
    }
}

/// State for a shared buffer including the file ID and buffer handle.
#[derive(Debug, Clone)]
pub struct BufferState {
    pub file_id: FileId,
    pub buffer: ModelHandle<Buffer>,
}

impl BufferState {
    pub fn new(file_id: FileId, buffer: ModelHandle<Buffer>) -> Self {
        Self { file_id, buffer }
    }
}

/// Tracks an active background diff parsing operation.
struct PendingDiffParse {
    abort_handle: AbortHandle,
}

struct InternalBufferState {
    buffer: WeakModelHandle<Buffer>,
    base_content_version: Option<ContentVersion>,
    /// Different from base_content_version, this is the first ever content version
    /// when the file is loaded in the app. We track this version so we don't submit
    /// a document updated event to LSP when the file is first loaded. (We do need to
    /// submit document updated if the file is auto-reloaded).
    initial_content_version: Option<ContentVersion>,
    /// Tracks the latest buffer version we've attempted to sync with LSP.
    /// Used to detect if previous versions were synced successfully.
    latest_buffer_version: Option<usize>,
    /// Tracks any active background diff parsing for auto-reload.
    pending_diff_parse: Option<PendingDiffParse>,
}

pub enum GlobalBufferModelEvent {
    BufferLoaded {
        file_id: FileId,
        content_version: ContentVersion,
    },
    FailedToLoad {
        file_id: FileId,
        error: Rc<FileLoadError>,
    },
    BufferUpdatedFromFileEvent {
        file_id: FileId,
        success: bool,
        content_version: ContentVersion,
    },
    FileSaved {
        file_id: FileId,
    },
    FailedToSave {
        file_id: FileId,
        error: Rc<FileSaveError>,
    },
}

impl GlobalBufferModelEvent {
    pub fn file_id(&self) -> FileId {
        match self {
            GlobalBufferModelEvent::BufferLoaded { file_id, .. }
            | GlobalBufferModelEvent::FailedToLoad { file_id, .. }
            | GlobalBufferModelEvent::BufferUpdatedFromFileEvent { file_id, .. }
            | GlobalBufferModelEvent::FileSaved { file_id, .. }
            | GlobalBufferModelEvent::FailedToSave { file_id, .. } => *file_id,
        }
    }
}

/// Global singleton model for managing shared buffers across editors.
///
/// This allows multiple editors to share the same buffer when editing the same file,
/// enabling consistent content synchronization and more efficient memory usage.
pub struct GlobalBufferModel {
    path_to_id: BiMap<PathBuf, FileId>,
    buffers: HashMap<FileId, InternalBufferState>,
}

impl GlobalBufferModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        #[cfg(feature = "local_fs")]
        _ctx.subscribe_to_model(&FileModel::handle(_ctx), Self::handle_file_model_events);

        #[cfg(feature = "local_fs")]
        _ctx.subscribe_to_model(
            &LspManagerModel::handle(_ctx),
            Self::handle_lsp_manager_events,
        );

        Self {
            path_to_id: BiMap::new(),
            buffers: HashMap::new(),
        }
    }

    /// Scan through all buffers and deallocate any that are no longer in use.
    pub fn remove_deallocated_buffers(&mut self, ctx: &mut ModelContext<Self>) {
        let ids_to_remove: HashSet<FileId> = self
            .buffers
            .iter()
            .filter_map(|(id, state)| {
                if state.buffer.upgrade(ctx).is_none() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        if ids_to_remove.is_empty() {
            return;
        }

        // Collect paths for didClose before removing entries.
        let paths_to_close: Vec<PathBuf> = ids_to_remove
            .iter()
            .filter_map(|id| self.path_to_id.get_by_right(id).cloned())
            .collect();

        for path in &paths_to_close {
            self.close_document_with_lsp(path, ctx);
        }

        for id in &ids_to_remove {
            self.path_to_id.remove_by_right(id);
        }

        for id in ids_to_remove {
            self.buffers.remove(&id);

            #[cfg(feature = "local_fs")]
            {
                let file_model = FileModel::handle(ctx);
                file_model.update(ctx, |file_model, ctx| {
                    file_model.cancel(id);
                    file_model.unsubscribe(id, ctx);
                });
            }
        }
    }

    pub fn buffer_loaded(&self, file_id: FileId) -> bool {
        self.buffers
            .get(&file_id)
            .map(|state| state.base_content_version.is_some())
            .unwrap_or(false)
    }

    fn cleanup_file_id(&mut self, file_id: FileId, _ctx: &mut ModelContext<Self>) {
        // Send didClose before removing the entry.
        if let Some((path, _)) = self.path_to_id.remove_by_right(&file_id) {
            self.close_document_with_lsp(&path, _ctx);
        }

        self.buffers.remove(&file_id);

        #[cfg(feature = "local_fs")]
        {
            let file_model = FileModel::handle(_ctx);
            file_model.update(_ctx, |file_model, ctx| {
                file_model.cancel(file_id);
                file_model.unsubscribe(file_id, ctx);
            });
        }
    }

    /// Returns the buffer handle if it is 1) still exists + active 2) loaded.
    fn buffer_handle_for_id(
        &mut self,
        file_id: FileId,
        ctx: &mut ModelContext<Self>,
    ) -> Option<ModelHandle<Buffer>> {
        let state = self.buffers.get(&file_id)?;

        // If the buffer hasn't been loaded yet, don't return a model handle.
        if state.base_content_version.is_none() {
            log::info!("Cannot return handle for unloaded buffers");
            return None;
        }

        match state.buffer.upgrade(ctx) {
            Some(handle) => Some(handle),
            None => {
                // Clean up deallocated buffers.
                self.cleanup_file_id(file_id, ctx);
                None
            }
        }
    }

    /// Once we finish reading the file's content from the disk, populate the buffer with the content.
    /// For initial load (is_loaded_from_file_system == true), this is synchronous.
    /// For auto-reload (is_loaded_from_file_system == false), this spawns a background task for diff computation.
    fn populate_buffer_with_read_content(
        &mut self,
        file_id: FileId,
        content: &str,
        base_version: ContentVersion,
        new_version: ContentVersion,
        is_initial_load: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(state) = self.buffers.get_mut(&file_id) else {
            return;
        };

        let Some(buffer) = state.buffer.upgrade(ctx) else {
            self.cleanup_file_id(file_id, ctx);
            log::warn!("Cannot populate buffer with content due to deallocated model handle");
            return;
        };

        if is_initial_load {
            // Initial load: use synchronous replace_all since there's nothing to preserve
            buffer.update(ctx, |buffer, ctx| {
                buffer.replace_all(content, ctx);
                buffer.set_version(new_version);
            });

            state.base_content_version = Some(new_version);

            ctx.emit(GlobalBufferModelEvent::BufferLoaded {
                file_id,
                content_version: new_version,
            });
        } else if FeatureFlag::IncrementalAutoReload.is_enabled() {
            // Auto-reload: spawn background task for diff computation
            Self::start_background_diff_parse(
                file_id,
                state,
                buffer,
                content,
                base_version,
                new_version,
                ctx,
            );
        } else {
            // Fallback: synchronous replace_all (non-incremental)
            buffer.update(ctx, |buffer, ctx| {
                buffer.replace_all(content, ctx);
                buffer.set_version(new_version);
            });

            state.base_content_version = Some(new_version);

            ctx.emit(GlobalBufferModelEvent::BufferUpdatedFromFileEvent {
                file_id,
                success: true,
                content_version: new_version,
            });
        }
    }

    /// Spawns a background task to compute the diff between current buffer content and new content.
    /// On completion, applies the diff edits to the buffer.
    fn start_background_diff_parse(
        file_id: FileId,
        state: &mut InternalBufferState,
        buffer: ModelHandle<Buffer>,
        new_content: &str,
        base_version: ContentVersion,
        new_version: ContentVersion,
        ctx: &mut ModelContext<Self>,
    ) {
        // Abort any existing diff parse for this file
        if let Some(pending) = state.pending_diff_parse.take() {
            pending.abort_handle.abort();
        }

        // Move owned strings to the background thread
        let old_text = buffer.as_ref(ctx).text().into_string();
        let new_content_owned = new_content.to_string();

        let handle = ctx.spawn(
            async move { text_diff(&old_text, &new_content_owned).await },
            move |me, diff: TextDiff, ctx| {
                me.apply_diff_result(file_id, diff, base_version, new_version, ctx);
            },
        );

        // Store the abort handle so we can cancel if a newer update arrives
        state.pending_diff_parse = Some(PendingDiffParse {
            abort_handle: handle.abort_handle(),
        });
    }

    /// Called when background diff parsing completes. Applies the diff edits to the buffer.
    fn apply_diff_result(
        &mut self,
        file_id: FileId,
        diff: TextDiff,
        base_version: ContentVersion,
        new_version: ContentVersion,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(state) = self.buffers.get_mut(&file_id) else {
            return;
        };

        // Clear the pending diff parse state
        state.pending_diff_parse = None;

        let Some(buffer) = state.buffer.upgrade(ctx) else {
            self.cleanup_file_id(file_id, ctx);
            return;
        };

        // Verify the buffer still matches the expected base version
        if !buffer.as_ref(ctx).version_match(&base_version) {
            log::info!("Buffer version changed during diff parsing, aborting apply");
            ctx.emit(GlobalBufferModelEvent::BufferUpdatedFromFileEvent {
                file_id,
                success: false,
                content_version: base_version,
            });
            return;
        }

        // Apply the diff edits
        buffer.update(ctx, |buffer, ctx| {
            if diff.is_empty() {
                // No actual changes to content, but still need to update version
                buffer.set_version(new_version);
                return;
            }
            let char_edits = diff.to_char_offset_edits(buffer);
            buffer.insert_at_char_offset_ranges(char_edits, new_version, ctx);
        });

        state.base_content_version = Some(new_version);

        ctx.emit(GlobalBufferModelEvent::BufferUpdatedFromFileEvent {
            file_id,
            success: true,
            content_version: new_version,
        });
    }

    #[cfg(feature = "local_fs")]
    fn handle_file_model_events(&mut self, event: &FileModelEvent, ctx: &mut ModelContext<Self>) {
        match event {
            FileModelEvent::FileLoaded {
                content,
                id,
                version,
            } => {
                // Only set the initial_content_version on first file load.
                if let Some(state) = self.buffers.get_mut(id) {
                    state.initial_content_version = Some(*version);
                }

                // For initial load, base_version and new_version are the same
                self.populate_buffer_with_read_content(*id, content, *version, *version, true, ctx);
            }
            FileModelEvent::FailedToLoad { id, error } => {
                ctx.emit(GlobalBufferModelEvent::FailedToLoad {
                    file_id: *id,
                    error: error.clone(),
                });
            }
            FileModelEvent::FileUpdated {
                id,
                content,
                base_version,
                new_version,
            } => {
                if let Some(buffer) = self.buffer_handle_for_id(*id, ctx) {
                    if buffer.as_ref(ctx).version_match(base_version) {
                        self.populate_buffer_with_read_content(
                            *id,
                            content,
                            *base_version,
                            *new_version,
                            false,
                            ctx,
                        );
                    } else {
                        // Buffer version doesn't match the event's base_version.
                        // Check if the buffer has no user edits (matches our internal
                        // base_content_version). If so, it's safe to start a fresh
                        // diff parse from the actual buffer version to the new content.
                        let internal_base_version = self
                            .buffers
                            .get(id)
                            .and_then(|state| state.base_content_version);
                        let has_no_user_edits = internal_base_version
                            .is_some_and(|v| buffer.as_ref(ctx).version_match(&v));

                        if has_no_user_edits {
                            // No user edits: safe to reload from the actual buffer
                            // version. This handles both:
                            log::info!(
                                "Starting fresh diff parse for file update (no user edits, \
                                 internal base {:?}, event base {:?})",
                                internal_base_version,
                                *base_version
                            );
                            let actual_version = buffer.as_ref(ctx).version();
                            self.populate_buffer_with_read_content(
                                *id,
                                content,
                                actual_version,
                                *new_version,
                                false,
                                ctx,
                            );
                        } else {
                            log::info!("Not updating global buffer due to version conflict");

                            // Abort any pending diff parse since the buffer has
                            // user edits that we must not overwrite.
                            if let Some(state) = self.buffers.get_mut(id) {
                                if let Some(pending) = state.pending_diff_parse.take() {
                                    pending.abort_handle.abort();
                                }
                            }

                            if internal_base_version != Some(*base_version) {
                                log::warn!(
                                    "Internal global buffer base version {:?} mismatches file model base version {:?}",
                                    internal_base_version,
                                    *base_version
                                );
                            }

                            ctx.emit(GlobalBufferModelEvent::BufferUpdatedFromFileEvent {
                                file_id: *id,
                                success: false,
                                content_version: *base_version,
                            });
                        }
                    }
                }
            }
            FileModelEvent::FileSaved { id, version } => {
                // Make sure base content version is updated after a save is performed.
                // This avoids us flagging the incoming update from file watcher as conflict changes.
                if let Some(state) = self.buffers.get_mut(id) {
                    state.base_content_version = Some(*version);
                }
                ctx.emit(GlobalBufferModelEvent::FileSaved { file_id: *id });
            }
            FileModelEvent::FailedToSave { id, error } => {
                ctx.emit(GlobalBufferModelEvent::FailedToSave {
                    file_id: *id,
                    error: error.clone(),
                });
            }
        }
    }

    /// Save the content of a tracked buffer to disk via FileModel.
    #[cfg(feature = "local_fs")]
    pub fn save(
        &self,
        file_id: FileId,
        content: String,
        version: ContentVersion,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), FileSaveError> {
        FileModel::handle(ctx).update(ctx, |file_model, ctx| {
            file_model.save(file_id, content, version, ctx)
        })
    }

    /// Rename a file and save its content via FileModel.
    #[cfg(feature = "local_fs")]
    pub fn rename_and_save(
        &self,
        file_id: FileId,
        new_path: PathBuf,
        content: String,
        version: ContentVersion,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), FileSaveError> {
        FileModel::handle(ctx).update(ctx, |file_model, ctx| {
            file_model.rename_and_save(file_id, new_path, content, version, ctx)
        })
    }

    /// Delete a file via FileModel.
    #[cfg(feature = "local_fs")]
    pub fn delete(
        &self,
        file_id: FileId,
        version: ContentVersion,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), FileSaveError> {
        FileModel::handle(ctx).update(ctx, |file_model, ctx| {
            file_model.delete(file_id, version, ctx)
        })
    }

    /// Remove a tracked buffer, cleaning up FileModel and LSP state.
    /// Used when a new file is deleted before ever being saved to a permanent location.
    pub fn remove(&mut self, file_id: FileId, ctx: &mut ModelContext<Self>) {
        self.cleanup_file_id(file_id, ctx);
    }

    /// Look up the file path for a tracked buffer.
    pub fn file_path(&self, file_id: FileId) -> Option<&Path> {
        self.path_to_id
            .get_by_right(&file_id)
            .map(|path| path.as_path())
    }

    /// Get the base content version (last known on-disk version) for a tracked buffer.
    pub fn base_version(&self, file_id: FileId) -> Option<ContentVersion> {
        self.buffers
            .get(&file_id)
            .and_then(|state| state.base_content_version)
    }

    /// Discard any in progress changes and reload the buffer with the canonical version from the file system.
    #[cfg(feature = "local_fs")]
    pub fn discard_unsaved_changes(&mut self, path: &PathBuf, ctx: &mut ModelContext<Self>) {
        if let Some(id) = self.path_to_id.get_by_left(path).cloned() {
            let path_clone = path.clone();
            ctx.spawn(
                async move { FileModel::read_content_for_file(&path_clone).await },
                move |me, content, ctx| match content {
                    Ok(content) => {
                        // Consider this reload as a "new" version. This prevents any race condition when there is another
                        // auto-reload while we are reading out the latest content.
                        let new_version = ContentVersion::new();
                        // For discard, we get the current base version from the buffer state
                        let base_version = me
                            .buffers
                            .get(&id)
                            .and_then(|state| {
                                state.buffer.upgrade(ctx).map(|b| b.as_ref(ctx).version())
                            })
                            .unwrap_or(new_version);
                        FileModel::handle(ctx).update(ctx, |file_model, _ctx| {
                            file_model.set_version(id, new_version);
                        });
                        me.populate_buffer_with_read_content(
                            id,
                            &content,
                            base_version,
                            new_version,
                            false,
                            ctx,
                        );
                    }
                    Err(e) => ctx.emit(GlobalBufferModelEvent::FailedToLoad {
                        file_id: id,
                        error: e.into(),
                    }),
                },
            );
        }
    }

    /// Remap an existing buffer from `old_file_id` to a new path, preserving the buffer
    /// content and unsaved edits. Sends didClose for the old path and re-registers the
    /// new path with FileModel and LSP.
    ///
    /// Used for file rename.
    #[cfg(feature = "local_fs")]
    pub fn rename(
        &mut self,
        old_file_id: FileId,
        new_path: PathBuf,
        ctx: &mut ModelContext<Self>,
    ) -> Option<BufferState> {
        let old_state = self.buffers.remove(&old_file_id)?;
        let buffer = old_state.buffer.upgrade(ctx)?;

        // Send didClose for the old path and remove the mapping.
        // Internal state cleanup is synchronous; only the LSP didClose notification
        // is dispatched asynchronously (with a no-op callback), so there is no race
        // between state removal and the close completing.
        if let Some((old_path, _)) = self.path_to_id.remove_by_right(&old_file_id) {
            self.close_document_with_lsp(&old_path, ctx);
        }

        // Cancel + unsubscribe old FileId from FileModel.
        let file_model = FileModel::handle(ctx);
        file_model.update(ctx, |file_model, ctx| {
            file_model.cancel(old_file_id);
            file_model.unsubscribe(old_file_id, ctx);
        });

        Some(self.register_buffer_for_path(
            new_path,
            buffer,
            old_state.base_content_version,
            old_state.initial_content_version,
            ctx,
        ))
    }

    /// Adopt an existing buffer under a new path without reading from disk.
    /// Used by `save_as` to register a newly-created file with GlobalBufferModel.
    #[cfg(feature = "local_fs")]
    pub fn register(
        &mut self,
        path: PathBuf,
        buffer: ModelHandle<Buffer>,
        ctx: &mut ModelContext<Self>,
    ) -> BufferState {
        let buffer_version = buffer.as_ref(ctx).version();
        self.register_buffer_for_path(
            path,
            buffer,
            Some(buffer_version),
            Some(buffer_version),
            ctx,
        )
    }

    /// Shared helper: register `buffer` under `path` with FileModel, subscribe to
    /// buffer events for LSP sync, store internal state, and open the document with LSP.
    #[cfg(feature = "local_fs")]
    fn register_buffer_for_path(
        &mut self,
        path: PathBuf,
        buffer: ModelHandle<Buffer>,
        base_content_version: Option<ContentVersion>,
        initial_content_version: Option<ContentVersion>,
        ctx: &mut ModelContext<Self>,
    ) -> BufferState {
        // If a buffer is already registered for this path, clean up the old entry
        // to avoid orphaning the previous FileId in `self.buffers`.
        if let Some(old_file_id) = self.path_to_id.get_by_left(&path).copied() {
            self.cleanup_file_id(old_file_id, ctx);
        }

        let buffer_version = buffer.as_ref(ctx).version();
        let file_id = FileModel::handle(ctx).update(ctx, |file_model, ctx| {
            let id = file_model.register_file_path(&path, true, ctx);
            file_model.set_version(id, buffer_version);
            id
        });

        self.path_to_id.insert(path.clone(), file_id);
        self.buffers.insert(
            file_id,
            InternalBufferState {
                buffer: buffer.downgrade(),
                base_content_version,
                initial_content_version,
                latest_buffer_version: None,
                pending_diff_parse: None,
            },
        );

        // Unsubscribe any existing buffer subscription (e.g. from a previous path)
        // before subscribing with the new path.
        ctx.unsubscribe_from_model(&buffer);

        // Subscribe to buffer events for LSP sync.
        let path_clone = path.clone();
        ctx.subscribe_to_model(&buffer, move |me, event, ctx| {
            use warp_editor::content::buffer::BufferEvent;

            let Some(state) = me.buffers.get(&file_id) else {
                return;
            };
            let Some(initial_version) = state.initial_content_version else {
                return;
            };
            let Some(buffer) = state.buffer.upgrade(ctx) else {
                return;
            };

            if let BufferEvent::ContentChanged {
                delta,
                buffer_version,
                origin,
                ..
            } = event
            {
                let version_matches_initial = buffer.as_ref(ctx).version_match(&initial_version);
                let fid = me.path_to_id.get_by_left(&path_clone).cloned();
                let previous_version = fid
                    .and_then(|id| me.buffers.get(&id))
                    .and_then(|state| state.latest_buffer_version);

                if let Some(id) = fid {
                    if let Some(state) = me.buffers.get_mut(&id) {
                        state.latest_buffer_version = Some(buffer_version.as_usize());
                    }
                }

                if matches!(origin, EditOrigin::SystemEdit) && version_matches_initial {
                    me.open_or_sync_document_with_lsp(buffer, &path_clone, *buffer_version, ctx);
                    return;
                }

                me.notify_lsp_of_content_change(
                    buffer,
                    &delta.precise_deltas,
                    &path_clone,
                    *buffer_version,
                    previous_version,
                    ctx,
                );
            }
        });

        // Open the document with LSP.
        let buffer_ver = buffer.as_ref(ctx).buffer_version();
        self.open_or_sync_document_with_lsp(buffer.clone(), &path, buffer_ver, ctx);

        BufferState::new(file_id, buffer)
    }

    /// Open a buffer for the given file path.
    ///
    /// If a buffer already exists for this path and is loaded, returns the existing BufferState.
    /// If no buffer exists, creates a new Buffer and BufferState using FileModel.
    /// File system updates are automatically subscribed to for all buffers.
    ///
    /// # Arguments
    /// * `path` - The file path to open
    /// * `ctx` - The model context for creating new buffers if needed
    ///
    /// # Returns
    /// Returns the BufferState for the requested file path.
    #[cfg(feature = "local_fs")]
    pub fn open(&mut self, path: PathBuf, ctx: &mut ModelContext<Self>) -> BufferState {
        if let Some(id) = self.path_to_id.get_by_left(&path).cloned() {
            debug_assert!(self.buffers.contains_key(&id));
            if let Some(state) = self.buffers.get(&id) {
                if let Some(handle) = state.buffer.upgrade(ctx) {
                    // Only emit buffer loaded if the base content version is set.
                    if state.base_content_version.is_some() {
                        ctx.emit(GlobalBufferModelEvent::BufferLoaded {
                            file_id: id,
                            content_version: handle.as_ref(ctx).version(),
                        });
                    }
                    return BufferState::new(id, handle.clone());
                }
            }
        }

        self.create_new_buffer(&path, ctx)
    }

    #[cfg(feature = "local_fs")]
    fn create_new_buffer(&mut self, path: &Path, ctx: &mut ModelContext<Self>) -> BufferState {
        // Open file through FileModel to get FileId
        // Always subscribe to updates for GlobalBufferModel created buffers
        let file_id =
            FileModel::handle(ctx).update(ctx, |file_model, ctx| file_model.open(path, true, ctx));

        // Create new buffer
        let buffer = ctx.add_model(|_| {
            // This sets the default indentation behavior. The editor will override this if it can load the grammar config
            // for the given file path.
            Buffer::new(Box::new(|_, _| {
                IndentBehavior::TabIndent(IndentUnit::Space(4))
            }))
        });

        let path_clone = path.to_path_buf();
        ctx.subscribe_to_model(&buffer, move |me, event, ctx| {
            use warp_editor::content::buffer::BufferEvent;

            let Some(state) = me.buffers.get(&file_id) else {
                me.log_lsp_sync_debug(
                    &path_clone,
                    format!(
                        "lsp-sync: ContentChanged SKIPPED file={} reason=buffer_state_missing",
                        path_clone.display()
                    ),
                    ctx,
                );
                return;
            };

            let Some(initial_version) = state.initial_content_version else {
                me.log_lsp_sync_debug(
                    &path_clone,
                    format!(
                        "lsp-sync: ContentChanged SKIPPED file={} reason=initial_version_not_set",
                        path_clone.display()
                    ),
                    ctx,
                );
                return;
            };

            let Some(buffer) = state.buffer.upgrade(ctx) else {
                me.log_lsp_sync_debug(
                    &path_clone,
                    format!(
                        "lsp-sync: ContentChanged SKIPPED file={} reason=buffer_handle_deallocated",
                        path_clone.display()
                    ),
                    ctx,
                );
                return;
            };

            if let BufferEvent::ContentChanged {
                delta,
                buffer_version,
                origin,
                ..
            } = event
            {
                let version_matches_initial = buffer.as_ref(ctx).version_match(&initial_version);

                // Read the previous latest_buffer_version before updating it.
                // This is needed to determine if we need a full sync later.
                let file_id = me.path_to_id.get_by_left(&path_clone).cloned();
                let previous_version = file_id
                    .and_then(|id| me.buffers.get(&id))
                    .and_then(|state| state.latest_buffer_version);

                // Always update the latest buffer version when we receive a ContentUpdated event,
                // even if we early return. This ensures we track versioning correctly.
                if let Some(id) = file_id {
                    if let Some(state) = me.buffers.get_mut(&id) {
                        state.latest_buffer_version = Some(buffer_version.as_usize());
                    }
                }

                // If this is a system edit AND the current buffer version matches the initial version
                // that came from file loading, this is the initial buffer population. Instead of
                // relying on the editor view to send didOpen, we handle it here to ensure the LSP
                // document lifecycle stays in sync with the buffer lifecycle.
                if matches!(origin, EditOrigin::SystemEdit) && version_matches_initial {
                    me.open_or_sync_document_with_lsp(buffer, &path_clone, *buffer_version, ctx);
                    return;
                }

                me.notify_lsp_of_content_change(
                    buffer,
                    &delta.precise_deltas,
                    &path_clone,
                    *buffer_version,
                    previous_version,
                    ctx,
                );
            }
        });

        self.path_to_id.insert(path.to_path_buf(), file_id);
        self.buffers.insert(
            file_id,
            InternalBufferState {
                buffer: buffer.downgrade(),
                base_content_version: None,
                initial_content_version: None,
                latest_buffer_version: None,
                pending_diff_parse: None,
            },
        );

        BufferState::new(file_id, buffer)
    }

    fn lsp_server_for_path(
        &self,
        path: &Path,
        ctx: &mut ModelContext<Self>,
    ) -> Option<ModelHandle<LspServerModel>> {
        LspManagerModel::as_ref(ctx).server_for_path(path, ctx)
    }

    fn log_lsp_sync_debug(&self, path: &Path, message: String, ctx: &mut ModelContext<Self>) {
        if cfg!(debug_assertions) {
            if let Some(server) = self.lsp_server_for_path(path, ctx) {
                server
                    .as_ref(ctx)
                    .log_to_server_log(LspServerLogLevel::Info, message);
            }
        }
    }

    /// Attempts to retrieve specific lines from an in-memory buffer for the given file path.
    /// Returns `Some(Vec<(usize, String)>)` if the file is loaded in a buffer, `None` otherwise.
    ///
    /// This is a fast, synchronous operation that avoids disk I/O.
    ///
    /// # Arguments
    /// * `path` - Path to the file
    /// * `line_numbers` - A list of 0-based line numbers to retrieve. Supports non-consecutive lines.
    ///
    /// # Returns
    /// A vector of (line_number, line_content) tuples for each requested line that exists.
    /// Lines that don't exist in the buffer are omitted from the result.
    pub fn get_lines_for_file(
        &mut self,
        path: &Path,
        line_numbers: Vec<usize>,
        ctx: &mut ModelContext<Self>,
    ) -> Option<Vec<(usize, String)>> {
        use warp_editor::content::text::LineCount;

        if line_numbers.is_empty() {
            return Some(Vec::new());
        }

        let file_id = self.path_to_id.get_by_left(path)?;
        let buffer = self.buffer_handle_for_id(*file_id, ctx)?;

        let buffer_ref = buffer.as_ref(ctx);
        let total_lines = (buffer_ref.max_point().row + 1) as usize;

        let mut lines = Vec::with_capacity(line_numbers.len());
        for line_idx in line_numbers {
            if line_idx >= total_lines {
                continue;
            }
            // Convert 0-based line index to 1-based LineCount
            let line_count = LineCount::from(line_idx + 1);
            let line_start = buffer_ref.line_start(line_count);
            let line_end = buffer_ref.line_end(line_count);
            let line_text = buffer_ref.text_in_range(line_start..line_end).into_string();
            lines.push((line_idx, line_text));
        }

        Some(lines)
    }

    /// Opens or resyncs a document with the LSP server.
    /// - If the LSP doesn't have the document open yet: sends `didOpen`
    /// - If the LSP already has the document open (buffer recreation): sends a full-content `didChange`
    fn open_or_sync_document_with_lsp(
        &mut self,
        buffer: ModelHandle<Buffer>,
        path: &Path,
        buffer_version: BufferVersion,
        ctx: &mut ModelContext<Self>,
    ) {
        let path_buf = path.to_path_buf();
        let current_version = buffer_version.as_usize();

        let Some(lsp_server) = &self.lsp_server_for_path(path, ctx) else {
            return;
        };

        if !lsp_server.as_ref(ctx).is_ready_for_requests() {
            return;
        };

        let content = buffer.as_ref(ctx).text_with_line_ending().into_string();

        let lsp_already_has_document = lsp_server
            .as_ref(ctx)
            .last_synced_version(&path_buf)
            .ok()
            .flatten()
            .is_some();

        if lsp_already_has_document {
            // Buffer was recreated but LSP still has the document open.
            // Send a full-content didChange to resync.
            lsp_server.as_ref(ctx).log_to_server_log(
                LspServerLogLevel::Info,
                format!(
                    "didChange -> server: RESYNC full-content file={} send_version={current_version}",
                    path.display()
                ),
            );

            let content_changed_events = vec![TextDocumentContentChangeEvent {
                range: None,
                text: content,
            }];

            let Ok(sync_future) = lsp_server.as_ref(ctx).did_change_document(
                path_buf,
                current_version.into(),
                content_changed_events,
            ) else {
                log::warn!("Failed to resync document with LSP server");
                return;
            };

            ctx.spawn(sync_future, |_, _, _| {});
        } else {
            // First time opening this document with the LSP.
            self.log_lsp_sync_debug(
                path,
                format!(
                    "lsp-sync: didOpen from GlobalBufferModel file={} version={current_version}",
                    path.display()
                ),
                ctx,
            );

            let Ok(open_future) =
                lsp_server
                    .as_ref(ctx)
                    .did_open_document(path_buf, content, current_version)
            else {
                log::warn!("Failed to open document with LSP server");
                return;
            };

            ctx.spawn(open_future, |_, _, _| {});
        }
    }

    /// Sends `didClose` to the LSP server for the given path, if the document is open.
    fn close_document_with_lsp(&mut self, path: &Path, ctx: &mut ModelContext<Self>) {
        let Some(lsp_server) = self.lsp_server_for_path(path, ctx) else {
            return;
        };

        let path_buf = path.to_path_buf();
        if !lsp_server
            .as_ref(ctx)
            .document_is_open(&path_buf)
            .is_ok_and(|is_open| is_open)
        {
            return;
        };

        self.log_lsp_sync_debug(
            path,
            format!(
                "lsp-sync: didClose from GlobalBufferModel file={}",
                path.display()
            ),
            ctx,
        );

        let Ok(close_future) = lsp_server.as_ref(ctx).did_close_document(path_buf) else {
            log::warn!("Failed to close document with LSP server");
            return;
        };

        ctx.spawn(close_future, |_, _, _| {});
    }

    /// When an LSP server starts, open all loaded buffers that match its workspace path.
    #[cfg(feature = "local_fs")]
    fn handle_lsp_manager_events(
        &mut self,
        event: &LspManagerModelEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let LspManagerModelEvent::ServerStarted(workspace_path) = event else {
            return;
        };

        // Collect (path, buffer_handle, version) for all loaded buffers under this workspace.
        let buffers_to_open: Vec<_> = self
            .path_to_id
            .iter()
            .filter(|(path, _)| path.starts_with(workspace_path))
            .filter_map(|(path, id)| {
                let state = self.buffers.get(id)?;
                // Only open buffers that have been loaded (have content).
                state.base_content_version.as_ref()?;
                let buffer = state.buffer.upgrade(ctx)?;
                let version = buffer.as_ref(ctx).buffer_version();
                Some((path.clone(), buffer, version))
            })
            .collect();

        for (path, buffer, version) in buffers_to_open {
            self.open_or_sync_document_with_lsp(buffer, &path, version, ctx);
        }
    }

    fn notify_lsp_of_content_change(
        &mut self,
        buffer: ModelHandle<Buffer>,
        deltas: &[PreciseDelta],
        path: &Path,
        buffer_version: BufferVersion,
        previous_version: Option<usize>,
        ctx: &mut ModelContext<Self>,
    ) {
        let path_buf = path.to_path_buf();
        let current_version = buffer_version.as_usize();

        let Some(lsp_server) = &self.lsp_server_for_path(path, ctx) else {
            return;
        };

        if !lsp_server.as_ref(ctx).is_ready_for_requests() {
            return;
        };

        // Check if the previous version was successfully synced.
        // If not, we need to fallback to syncing the full buffer content.
        let last_synced = lsp_server
            .as_ref(ctx)
            .last_synced_version(&path_buf)
            .ok()
            .flatten();

        // If we have a previous version that wasn't synced, we need to do a full sync.
        let needs_full_sync = previous_version.is_some_and(|prev| {
            last_synced.is_none() || last_synced.is_some_and(|synced| synced < prev)
        });

        let deltas_len = deltas.len();

        if needs_full_sync {
            lsp_server.as_ref(ctx).log_to_server_log(
                LspServerLogLevel::Info,
                format!(
                    "didChange -> server: FALLBACK full-sync file={} send_version={current_version} prev_version={previous_version:?} last_synced={last_synced:?} deltas={deltas_len}",
                    path.display()
                ),
            );
        } else {
            lsp_server.as_ref(ctx).log_to_server_log(
                LspServerLogLevel::Debug,
                format!(
                    "didChange -> server: file={} send_version={current_version} prev_version={previous_version:?} last_synced={last_synced:?} deltas={deltas_len}",
                    path.display()
                ),
            );
        }

        let line_ending_mode = buffer.as_ref(ctx).line_ending_mode();
        let content_changed_events = if needs_full_sync {
            // Send the full buffer content without a range.
            vec![TextDocumentContentChangeEvent {
                range: None,
                text: buffer.as_ref(ctx).text_with_line_ending().into_string(),
            }]
        } else {
            deltas
                .iter()
                .map(|delta| {
                    let start = lsp::types::Location {
                        line: delta.replaced_points.start.row.saturating_sub(1) as usize,
                        column: delta.replaced_points.start.column as usize,
                    };
                    let end = lsp::types::Location {
                        line: delta.replaced_points.end.row.saturating_sub(1) as usize,
                        column: delta.replaced_points.end.column as usize,
                    };

                    TextDocumentContentChangeEvent {
                        range: Some(lsp::types::Range { start, end }),
                        text: buffer
                            .as_ref(ctx)
                            .text_in_ranges(vec1![delta.resolved_range.clone()], line_ending_mode)
                            .into_string(),
                    }
                })
                .collect()
        };

        let Ok(sync_future) = lsp_server.as_ref(ctx).did_change_document(
            path_buf,
            current_version.into(),
            content_changed_events,
        ) else {
            log::warn!("Failed to sync document with LSP server");
            return;
        };

        ctx.spawn(sync_future, |_, _, _| {});
    }
}

impl Entity for GlobalBufferModel {
    type Event = GlobalBufferModelEvent;
}

impl SingletonEntity for GlobalBufferModel {}
