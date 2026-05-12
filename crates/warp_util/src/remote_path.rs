use serde::{Deserialize, Serialize};

use crate::host_id::HostId;
use crate::standardized_path::StandardizedPath;

/// Identifies a file on a remote host.
///
/// Pairs a [`HostId`] (to deduplicate across multiple SSH sessions to the
/// same host) with the server-side [`StandardizedPath`]. This type is the
/// canonical representation for remote file locations and is shared across
/// buffer tracking, repository identification, and other host-scoped features.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemotePath {
    pub host_id: HostId,
    pub path: StandardizedPath,
}

impl RemotePath {
    pub fn new(host_id: HostId, path: StandardizedPath) -> Self {
        Self { host_id, path }
    }
}
