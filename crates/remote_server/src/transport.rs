//! Transport abstraction for [`RemoteServerManager`].
//!
//! Separates SSH-specific concerns (ControlMaster sockets, binary install,
//! process spawning) from the transport-agnostic session lifecycle managed
//! by [`RemoteServerManager`]. Alternative transports (Docker exec,
//! in-process for tests) implement the same trait without touching the
//! manager.
//!
//! Methods are async. Callers use the trait via generics
//! (`T: RemoteTransport`) rather than `dyn` dispatch.
//!
//! [`RemoteServerManager`]: crate::manager::RemoteServerManager
use std::future::Future;
#[cfg(not(target_family = "wasm"))]
use std::path::PathBuf;

use async_channel::Receiver;
use warpui::r#async::executor;

use crate::client::{ClientEvent, RemoteServerClient};
use crate::setup::RemotePlatform;

/// A successful return from [`RemoteTransport::connect`].
///
/// Bundles the live [`RemoteServerClient`] and its [`ClientEvent`]
/// receiver together with any transport-specific resources whose
/// lifetime must match the session (notably an owning `Child` for
/// subprocess-backed transports). The caller -- typically
/// [`RemoteServerManager`] -- stashes the whole `Connection` on its
/// per-session state so that dropping the state cleans everything up at
/// once.
///
/// [`RemoteServerManager`]: crate::manager::RemoteServerManager
#[cfg_attr(target_family = "wasm", allow(dead_code))]
pub struct Connection {
    pub client: RemoteServerClient,
    pub event_rx: Receiver<ClientEvent>,
    /// The subprocess whose stdio backs the client (e.g.
    /// `ssh … remote-server-proxy`). Spawned with `kill_on_drop(true)`
    /// by the transport, so dropping this `Child` sends SIGKILL to the
    /// subprocess. The [`RemoteServerManager`] holds it for the
    /// lifetime of the session and drops it on teardown.
    ///
    /// [`RemoteServerManager`]: crate::manager::RemoteServerManager
    #[cfg(not(target_family = "wasm"))]
    pub child: async_process::Child,
    /// For transports that multiplex through a local SSH
    /// `ControlMaster` socket: the path to that socket, used on
    /// explicit teardown (after the user's shell exits) to run
    /// `ssh -O exit` and force the master to terminate without
    /// waiting for half-closed channels. `None` for transports with
    /// no separate master process (in-process tests, etc.).
    ///
    /// See [`crate::ssh::stop_control_master`] for the exact command.
    #[cfg(not(target_family = "wasm"))]
    pub control_path: Option<PathBuf>,
}

pub trait RemoteTransport: Send + Sync {
    /// Detects the remote host's OS and architecture by running `uname -sm`.
    ///
    /// Returns the parsed [`RemotePlatform`] on success, or an error string
    /// if the command fails or the output cannot be parsed.
    fn detect_platform(&self) -> impl Future<Output = Result<RemotePlatform, String>> + Send;

    /// Checks whether the remote server binary is present on the remote host.
    ///
    /// Pure I/O — does not emit any events. The caller
    /// ([`RemoteServerManager::check_binary`]) is responsible for emitting
    /// [`SetupStateChanged`] and [`BinaryCheckComplete`].
    ///
    /// Returns `Ok(true)` if the binary is installed and executable,
    /// `Ok(false)` if it is definitively not installed, and
    /// `Err(_)` if the check failed (e.g. SSH timeout/unreachable).
    fn check_binary(&self) -> impl Future<Output = Result<bool, String>> + Send;

    /// Installs the remote server binary on the remote host.
    ///
    /// Pure I/O — does not emit any events. The caller
    /// ([`RemoteServerManager::install_binary`]) is responsible for emitting
    /// [`SetupStateChanged`] and [`BinaryInstallComplete`].
    ///
    /// Returns `Ok(())` if the install succeeded, and
    /// `Err(_)` if the install failed (e.g. SSH timeout, script error).
    fn install_binary(&self) -> impl Future<Output = Result<(), String>> + Send;

    /// Establish a new connection to the remote server.
    ///
    /// Called on both the initial connect and every subsequent reconnect
    /// attempt. Returns a [`Connection`] carrying the live client, its
    /// event channel, and any transport-specific resources (e.g. an
    /// owning `Child`) whose lifetime must match the session.
    ///
    /// The implementation is responsible for any transport-specific setup
    /// required before messages can flow (e.g. spawning a process, connecting
    /// a socket). Stderr forwarding to local logging should also happen here.
    fn connect(
        &self,
        executor: &executor::Background,
    ) -> impl Future<Output = anyhow::Result<Connection>> + Send;
}
