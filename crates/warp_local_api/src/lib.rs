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

/// Hard cap on inbound IPC frame size, enforced by the framing layer
/// before any payload is read off the wire or deserialized. Sized to fit
/// `MAX_SEND_TEXT_BYTES` plus bincode envelope/cookie overhead with margin
/// for future fields. An unauthenticated peer that announces a frame
/// larger than this is rejected before allocation, so the cookie check
/// can't be bypassed by exhausting memory first.
pub const MAX_REQUEST_BYTES: usize = MAX_SEND_TEXT_BYTES + 4 * 1024;

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

/// Default app-id domain used when no override is set and exactly one
/// channel cannot be inferred.
pub const DEFAULT_DATA_DOMAIN: &str = "dev.warp.Warp";

/// Filename Warp uses for the published address file inside its
/// per-channel data directory.
pub const ADDRESS_FILE_NAME: &str = "local-api.address";

/// Outcome of resolving which Warp instance a CLI invocation should target.
pub enum AddressResolution {
    /// A single address-file path was selected.
    Single(PathBuf),
    /// More than one channel currently publishes an address file. Caller must
    /// disambiguate via `WARP_LOCAL_API_DOMAIN` / `WARP_LOCAL_API_ADDRESS`.
    Ambiguous(Vec<PathBuf>),
}

/// Resolve the address-file path that the `wp` CLI should read.
///
/// Resolution order:
/// 1. `WARP_LOCAL_API_ADDRESS` — full-path override (e.g. set by an
///    integration script that already knows the target instance).
/// 2. `WARP_LOCAL_API_DOMAIN` — domain-only override; resolved through
///    [`address_publish_path_for`].
/// 3. Auto-discovery: scan `<data-local-dir>/*/{ADDRESS_FILE_NAME}` and
///    return the single existing match, or `Ambiguous` if multiple
///    channels are running.
/// 4. Fall back to the default domain (`DEFAULT_DATA_DOMAIN`) so callers
///    still get a deterministic path to surface in error messages.
pub fn resolve_address_path() -> AddressResolution {
    if let Some(p) = std::env::var_os("WARP_LOCAL_API_ADDRESS") {
        return AddressResolution::Single(PathBuf::from(p));
    }
    if let Ok(domain) = std::env::var("WARP_LOCAL_API_DOMAIN") {
        return AddressResolution::Single(address_publish_path_for(&domain));
    }
    let candidates = discover_address_files();
    match candidates.len() {
        0 => AddressResolution::Single(address_publish_path_for(DEFAULT_DATA_DOMAIN)),
        1 => AddressResolution::Single(candidates.into_iter().next().unwrap()),
        _ => AddressResolution::Ambiguous(candidates),
    }
}

/// Backwards-compatible accessor for the default address path. Prefer
/// [`resolve_address_path`] in callers that want auto-discovery.
pub fn address_publish_path() -> PathBuf {
    match resolve_address_path() {
        AddressResolution::Single(p) => p,
        AddressResolution::Ambiguous(mut v) => {
            v.sort();
            v.into_iter()
                .next()
                .unwrap_or_else(|| address_publish_path_for(DEFAULT_DATA_DOMAIN))
        }
    }
}

/// Build the default address-file path for a specific data domain. Used by
/// the running Warp app to publish under its own channel/installation
/// namespace.
pub fn address_publish_path_for(domain: &str) -> PathBuf {
    address_publish_root().join(domain).join(ADDRESS_FILE_NAME)
}

fn address_publish_root() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir)
}

/// Walk the address-publish root one level deep and return every existing
/// `<domain>/local-api.address` file. Read-only; no side effects.
fn discover_address_files() -> Vec<PathBuf> {
    let root = address_publish_root();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let candidate = entry.path().join(ADDRESS_FILE_NAME);
        if candidate.is_file() {
            out.push(candidate);
        }
    }
    out.sort();
    out
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
