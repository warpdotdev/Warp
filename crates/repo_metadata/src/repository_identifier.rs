use std::path::{Path, PathBuf};

use warp_core::HostId;
use warp_util::standardized_path::StandardizedPath;

/// Identifies a repository across local and remote environments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RepositoryIdentifier {
    /// A repository on the local filesystem, identified by its standardized path.
    Local(StandardizedPath),
    /// A repository on a remote server, identified by session + path.
    Remote(RemoteRepositoryIdentifier),
}

impl RepositoryIdentifier {
    /// Convenience constructor for a local repository identifier.
    pub fn local(path: StandardizedPath) -> Self {
        Self::Local(path)
    }

    /// Convenience constructor that creates a `Local` identifier from a
    /// `std::path::Path`. Returns `None` if the path is not absolute or
    /// contains non-UTF-8 characters.
    pub fn try_local(path: &Path) -> Option<Self> {
        StandardizedPath::try_from_local(path).ok().map(Self::Local)
    }

    /// Returns the local path if this is a `Local` variant.
    pub fn local_path(&self) -> Option<&StandardizedPath> {
        match self {
            Self::Local(path) => Some(path),
            Self::Remote(_) => None,
        }
    }

    /// Returns the local path as a `PathBuf` if this is a `Local` variant
    /// and the encoding matches the current OS.
    pub fn local_path_buf(&self) -> Option<PathBuf> {
        match self {
            Self::Local(path) => path.to_local_path(),
            Self::Remote(_) => None,
        }
    }
}

/// Identifies a repository on a remote server.
///
/// Pairs a [`HostId`] (to deduplicate across multiple SSH sessions to the
/// same host) with the server-side [`StandardizedPath`]. The path lives on
/// the remote machine and is constructed without I/O using encoding
/// information from the remote OS.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteRepositoryIdentifier {
    pub host_id: HostId,
    pub path: StandardizedPath,
}

impl RemoteRepositoryIdentifier {
    pub fn new(host_id: HostId, path: StandardizedPath) -> Self {
        Self { host_id, path }
    }
}

impl From<RemoteRepositoryIdentifier> for RepositoryIdentifier {
    fn from(id: RemoteRepositoryIdentifier) -> Self {
        Self::Remote(id)
    }
}
