use std::collections::{HashMap, HashSet};

use warp_util::file::FileId;
use warpui::{ModelContext, SingletonEntity};

use super::server_model::{ConnectionId, ServerModel};
use crate::code::global_buffer_model::GlobalBufferModel;
use crate::remote_server::protocol::RequestId;

/// Bridges the ServerModel's per-connection state with the GlobalBufferModel's
/// tracked buffers. Manages:
/// - Wire path → FileId mappings for open server-local buffers
/// - Per-buffer connection sets (which connections have each buffer open)
/// - Pending async requests (OpenBuffer, SaveBuffer) awaiting events
pub struct ServerBufferTracker {
    /// Maps wire path strings to `FileId` for open server-local buffers.
    open_buffers: HashMap<String, FileId>,
    /// Tracks which connections have each buffer open.
    /// File-watcher pushes go to all connections in the set.
    buffer_connections: HashMap<FileId, HashSet<ConnectionId>>,
    /// Tracks in-flight OpenBuffer / SaveBuffer requests so
    /// `GlobalBufferModelEvent`s can be correlated back to the originating
    /// request and connection.
    pending_requests: HashMap<FileId, (RequestId, ConnectionId)>,
}

impl ServerBufferTracker {
    pub fn new() -> Self {
        Self {
            open_buffers: HashMap::new(),
            buffer_connections: HashMap::new(),
            pending_requests: HashMap::new(),
        }
    }

    // ── Path ↔ FileId mapping ─────────────────────────────────────

    /// Register a wire path → FileId mapping.
    pub fn track_open_buffer(&mut self, path: String, file_id: FileId) {
        self.open_buffers.insert(path, file_id);
    }

    /// Look up a FileId by its wire path.
    pub fn file_id_for_path(&self, path: &str) -> Option<FileId> {
        self.open_buffers.get(path).copied()
    }

    /// Look up the wire path for a given FileId.
    pub fn path_for_file_id(&self, file_id: FileId) -> Option<String> {
        self.open_buffers.iter().find_map(|(p, id)| {
            if *id == file_id {
                Some(p.clone())
            } else {
                None
            }
        })
    }

    // ── Connection tracking ───────────────────────────────────────

    /// Add a connection to a buffer's subscriber set.
    pub fn add_connection(&mut self, file_id: FileId, conn_id: ConnectionId) {
        self.buffer_connections
            .entry(file_id)
            .or_default()
            .insert(conn_id);
    }

    /// Returns the set of connections subscribed to a buffer.
    pub fn connections_for_buffer(&self, file_id: &FileId) -> Option<&HashSet<ConnectionId>> {
        self.buffer_connections.get(file_id)
    }

    /// Remove a connection from all buffer subscription sets.
    /// Returns the list of FileIds that have no remaining connections
    /// (orphaned buffers that should be deallocated).
    pub fn remove_connection(
        &mut self,
        conn_id: ConnectionId,
        ctx: &mut ModelContext<ServerModel>,
    ) -> Vec<FileId> {
        let orphaned: Vec<FileId> = self
            .buffer_connections
            .iter_mut()
            .filter_map(|(file_id, conns)| {
                conns.remove(&conn_id);
                if conns.is_empty() {
                    Some(*file_id)
                } else {
                    None
                }
            })
            .collect();

        for &file_id in &orphaned {
            self.buffer_connections.remove(&file_id);
            self.open_buffers.retain(|_, id| *id != file_id);
            GlobalBufferModel::handle(ctx).update(ctx, |gbm, ctx| gbm.remove(file_id, ctx));
        }

        orphaned
    }

    /// Close a buffer by path: clear all connections and deallocate.
    pub fn close_buffer(&mut self, path: &str, ctx: &mut ModelContext<ServerModel>) {
        let Some(&file_id) = self.open_buffers.get(path) else {
            return;
        };

        if let Some(conns) = self.buffer_connections.get_mut(&file_id) {
            conns.clear();
        }

        self.buffer_connections.remove(&file_id);
        self.open_buffers.remove(path);
        GlobalBufferModel::handle(ctx).update(ctx, |gbm, ctx| gbm.remove(file_id, ctx));
    }

    // ── Pending request tracking ──────────────────────────────────

    /// Stash a pending async request for later correlation with an event.
    pub fn insert_pending(
        &mut self,
        file_id: FileId,
        request_id: RequestId,
        conn_id: ConnectionId,
    ) {
        self.pending_requests.insert(file_id, (request_id, conn_id));
    }

    /// Retrieve and remove a pending request for the given FileId.
    pub fn take_pending(&mut self, file_id: &FileId) -> Option<(RequestId, ConnectionId)> {
        self.pending_requests.remove(file_id)
    }
}
