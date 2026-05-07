//! Wire types and `ipc::Service` definition for Warp's local control API.
//!
//! The running Warp process hosts a UDS server that speaks this protocol; the
//! `wp` CLI is the canonical client. Both ends share these types via this
//! crate so the bincode wire format stays in lockstep.
//!
//! # Authentication
//!
//! On startup, Warp generates a random per-session cookie and writes the
//! socket path + cookie to a 0600 address file under the user's data dir.
//! Possessing the file (i.e., being the same UID) is the authentication
//! factor; every request carries the cookie inside [`LocalApiEnvelope`] and
//! the server rejects mismatches before dispatching.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Maximum bytes accepted for `SendText.text`. Defends the in-app handler
/// against unbounded payloads from a same-UID local client.
pub const MAX_SEND_TEXT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SplitDir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalApiRequest {
    Ping,
    /// Split the active pane in `dir`; reply contains the new pane's id.
    Split {
        dir: SplitDir,
    },
    /// Write `text` to the PTY of the named terminal pane (or active if None).
    SendText {
        pane: Option<String>,
        text: String,
    },
    /// Returns the ids of all terminal panes in the active workspace.
    ListPanes,
    /// Returns the id of the focused terminal pane in the active workspace.
    ActivePane,
    /// Closes the named terminal pane.
    ClosePane {
        pane: String,
    },
}

/// Wire envelope: every request carries the per-session cookie, validated
/// server-side before dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalApiEnvelope {
    pub cookie: String,
    pub request: LocalApiRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalApiResponse {
    Pong,
    Ok,
    PaneId(String),
    Panes(Vec<String>),
    Err(String),
}

pub struct LocalApiService;

#[async_trait]
impl ipc::Service for LocalApiService {
    type Request = LocalApiEnvelope;
    type Response = LocalApiResponse;
}

/// Filesystem path where the running Warp publishes its current connection
/// address + cookie. The `wp` CLI reads this file to find the live socket.
/// Created with mode 0600.
///
/// Resolution order:
/// 1. `WARP_LOCAL_API_ADDRESS` — full path override. The running Warp sets
///    this in the env of every spawned shell so child `wp` invocations land
///    on the same instance even when several Warp variants run side-by-side.
/// 2. `<data-local-dir>/<WARP_LOCAL_API_DOMAIN or default>/local-api.address`
///    — default for ad-hoc invocations from outside a Warp shell.
///
/// The default domain is the production app id `dev.warp.Warp`; dev / preview
/// / oss instances each publish under their own domain so they don't clobber
/// each other's address files.
pub fn address_publish_path() -> PathBuf {
    if let Some(p) = std::env::var_os("WARP_LOCAL_API_ADDRESS") {
        return PathBuf::from(p);
    }
    let domain =
        std::env::var("WARP_LOCAL_API_DOMAIN").unwrap_or_else(|_| "dev.warp.Warp".to_owned());
    address_publish_path_for(&domain)
}

/// Build the default address-file path for a specific data domain. Used by
/// the running Warp app to publish under its own channel/installation
/// namespace.
pub fn address_publish_path_for(domain: &str) -> PathBuf {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join(domain).join("local-api.address")
}

/// Format the address-file body. Two lines:
///   line 1: socket path
///   line 2: cookie hex
pub fn format_address_file(socket: &str, cookie: &str) -> String {
    format!("{socket}\n{cookie}\n")
}

/// Inverse of [`format_address_file`]. Returns `(socket, cookie)`.
pub fn parse_address_file(s: &str) -> Option<(String, String)> {
    let mut lines = s.lines();
    let socket = lines.next()?.trim().to_owned();
    let cookie = lines.next()?.trim().to_owned();
    if socket.is_empty() || cookie.is_empty() {
        return None;
    }
    Some((socket, cookie))
}
