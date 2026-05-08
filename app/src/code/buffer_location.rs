use std::path::{Path, PathBuf};

use warp_util::content_version::ContentVersion;
use warp_util::remote_path::RemotePath;

/// Uniquely identifies where a file lives — either on the local filesystem
/// or on a remote host. Used across both the buffer model and the
/// editor/view layers as the canonical file-identity type.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum FileLocation {
    /// File on the local filesystem.
    Local(PathBuf),
    /// File on a remote host, identified by host + path.
    Remote(RemotePath),
}

impl FileLocation {
    /// Returns the file name component for display (e.g. tab titles).
    pub fn display_name(&self) -> &str {
        match self {
            FileLocation::Local(path) => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default(),
            FileLocation::Remote(remote) => remote.path.file_name().unwrap_or_default(),
        }
    }

    /// Returns the local path if this is a `Local` location, `None` for `Remote`.
    /// Callers that only work with local files (LSP, save-to-disk, reveal-in-finder)
    /// should use this to gate their behavior.
    pub fn to_local_path(&self) -> Option<&Path> {
        match self {
            FileLocation::Local(path) => Some(path.as_path()),
            FileLocation::Remote(_) => None,
        }
    }
}

impl From<PathBuf> for FileLocation {
    fn from(path: PathBuf) -> Self {
        FileLocation::Local(path)
    }
}

impl From<RemotePath> for FileLocation {
    fn from(remote: RemotePath) -> Self {
        FileLocation::Remote(remote)
    }
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
///
/// Both fields use `ContentVersion` internally. At the wire boundary (proto
/// encode/decode), convert via `ContentVersion::as_u64()` and
/// `ContentVersion::from_raw()`.
#[derive(Clone, Debug)]
pub struct SyncClock {
    /// Last version acknowledged from the server (file-watcher side).
    pub server_version: ContentVersion,
    /// Last version acknowledged from the client (user-edit side).
    pub client_version: ContentVersion,
}

impl SyncClock {
    #[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
    pub fn new() -> Self {
        Self {
            server_version: ContentVersion::from_raw(0),
            client_version: ContentVersion::from_raw(0),
        }
    }

    /// Reconstruct a `SyncClock` from wire values (proto deserialization).
    pub fn from_wire(server_version: u64, client_version: u64) -> Self {
        Self {
            server_version: ContentVersion::from_raw(server_version as usize),
            client_version: ContentVersion::from_raw(client_version as usize),
        }
    }

    /// Bump the server version after a file-watcher change.
    #[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
    pub fn bump_server(&mut self) -> ContentVersion {
        self.server_version = ContentVersion::new();
        self.server_version
    }

    /// Check whether a server push's expected client version matches our local state.
    pub fn server_push_matches(&self, expected_client_version: ContentVersion) -> bool {
        self.client_version == expected_client_version
    }

    /// Check whether a client edit's expected server version matches our local state.
    #[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
    pub fn client_edit_matches(&self, expected_server_version: ContentVersion) -> bool {
        self.server_version == expected_server_version
    }
}

#[cfg(test)]
#[path = "buffer_location_tests.rs"]
mod tests;
