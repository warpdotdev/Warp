use std::path::PathBuf;

use warp_util::remote_path::RemotePath;

/// Uniquely identifies where a buffer's content lives.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum BufferLocation {
    /// File on the local filesystem.
    Local(PathBuf),
    /// File on a remote host, identified by host + path.
    Remote(RemotePath),
}

/// Tracks sync state between client and server for a single remote buffer.
///
/// Uses a version vector with two components:
/// - `server_version`: bumped by the server when the file changes on disk.
/// - `client_version`: bumped by the client when the user edits the buffer.
///
/// Conflict detection:
/// - Server pushes `{S_new, C_expected}`. Client checks `C_expected == local client_version`.
///   Match → accept. Mismatch → conflict.
/// - Client sends `{S_expected, C_new}`. Server checks `S_expected == local server_version`.
///   Match → accept. Mismatch → reject (server pushes its current state).
#[derive(Clone, Debug)]
pub struct SyncClock {
    /// Last version acknowledged from the server (file-watcher side).
    ///
    /// This is a raw `u64` rather than `ContentVersion` because it represents
    /// a version counter from the remote server protocol, not a local
    /// buffer mutation. `ContentVersion` is auto-incremented locally and
    /// cannot be constructed from an arbitrary wire value.
    pub server_version: u64,
    /// Last version acknowledged from the client (user-edit side).
    pub client_version: u64,
}

impl SyncClock {
    pub fn new(server_version: u64) -> Self {
        Self {
            server_version,
            client_version: 0,
        }
    }

    /// Bump the client version after a local edit. Returns the new client version.
    pub fn bump_client(&mut self) -> u64 {
        self.client_version += 1;
        self.client_version
    }

    /// Check whether a server push's expected client version matches our local state.
    pub fn server_push_matches(&self, expected_client_version: u64) -> bool {
        self.client_version == expected_client_version
    }
}
