use std::collections::{HashMap, HashSet};
#[cfg(not(target_family = "wasm"))]
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use crate::client::ClientEvent;
use crate::client::RemoteServerClient;
use crate::setup::RemotePlatform;
use crate::setup::RemoteServerSetupState;
#[cfg(not(target_family = "wasm"))]
use crate::transport::Connection;
use crate::transport::RemoteTransport;
use crate::HostId;
use repo_metadata::RepoMetadataUpdate;
use serde::Serialize;
use warp_core::SessionId;
use warpui::{Entity, ModelContext, ModelSpawner, SingletonEntity};

/// Which phase of the remote server connection flow failed.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteServerInitPhase {
    /// `transport.connect()` failed (SSH/process spawn level).
    Connect,
    /// `client.initialize()` failed (protocol handshake level).
    Initialize,
}

/// The remote server client operation that failed.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteServerOperation {
    NavigateToDirectory,
    LoadRepoMetadataDirectory,
}

/// Classification of a remote server client error for telemetry.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteServerErrorKind {
    Timeout,
    Disconnected,
    ServerError,
    Other,
}

impl RemoteServerErrorKind {
    /// Classify a [`ClientError`] into a telemetry error kind.
    pub fn from_client_error(error: &crate::client::ClientError) -> Self {
        use crate::client::ClientError;
        match error {
            ClientError::Timeout(_) => Self::Timeout,
            ClientError::Disconnected | ClientError::ResponseChannelClosed => Self::Disconnected,
            ClientError::ServerError { .. } => Self::ServerError,
            ClientError::Protocol(_)
            | ClientError::UnexpectedResponse
            | ClientError::FileOperationFailed(_) => Self::Other,
        }
    }
}

/// Per-session connection state. Encodes which data is available at each
/// lifecycle stage so the compiler prevents invalid combinations.
///
/// For subprocess-backed transports (SSH), the `Initializing` and
/// `Connected` variants also own the transport's `Child`. Dropping or
/// replacing the state sends SIGKILL to the subprocess via
/// `kill_on_drop`, which is the authoritative teardown path -- it fires
/// on both explicit deregistration and spontaneous disconnect, and is
/// unaffected by lingering `Arc<RemoteServerClient>` clones held
/// elsewhere (e.g. the per-session command executor).
///
/// They also optionally carry a `control_path` pointing at the SSH
/// `ControlMaster` socket for this session. On explicit teardown
/// (after the user's shell exits), `deregister_session` uses this to
/// run `ssh -O exit`, forcing the master to terminate without waiting
/// for half-closed multiplexed channels to finish cleanup on the
/// remote side.
#[derive(Debug)]
pub enum RemoteSessionState {
    /// `connect_session` has been called; background task is starting the
    /// server process over SSH.
    Connecting,
    /// Server process spawned, client exists, initialize handshake in progress.
    Initializing {
        client: Arc<RemoteServerClient>,
        /// The transport's owning `Child`. Dropped when the state is
        /// replaced or removed, killing the subprocess via
        /// `kill_on_drop`.
        #[cfg(not(target_family = "wasm"))]
        _child: async_process::Child,
        /// See type-level doc.
        #[cfg(not(target_family = "wasm"))]
        control_path: Option<PathBuf>,
    },
    /// Initialize handshake succeeded. Client is ready for requests.
    Connected {
        client: Arc<RemoteServerClient>,
        host_id: HostId,
        /// The transport's owning `Child`. See `Initializing::_child`.
        #[cfg(not(target_family = "wasm"))]
        _child: async_process::Child,
        /// See type-level doc.
        #[cfg(not(target_family = "wasm"))]
        control_path: Option<PathBuf>,
    },
    /// Connection dropped (EOF/error from the reader task).
    Disconnected,
}

/// Events emitted by [`RemoteServerManager`].
#[derive(Clone, Debug)]
pub enum RemoteServerManagerEvent {
    // --- Session-scoped events ---
    /// A connection flow has started for this session.
    SessionConnecting { session_id: SessionId },
    /// This session's server is connected and ready. Includes the `HostId`
    /// received from the initialize handshake, for model deduplication.
    SessionConnected {
        session_id: SessionId,
        host_id: HostId,
    },
    /// The remote server launch or handshake failed.
    SessionConnectionFailed {
        session_id: SessionId,
        /// Which phase of the connection flow failed.
        phase: RemoteServerInitPhase,
        /// The error message from the failed phase.
        error: String,
    },
    /// This session's connection dropped. Carries `host_id` so consumers
    /// don't need to look it up from the already-transitioned state.
    /// This session's underlying connection is no longer usable: the
    /// stream closed (EOF/error), the initialize handshake failed, or the
    /// session was explicitly deregistered while `Connected`. Signals to
    /// subscribers that they should drop any `Arc<RemoteServerClient>` they
    /// hold for this session. Carries `host_id` so consumers don't need to
    /// look it up from the already-transitioned state.
    ///
    /// Note this is about *transport* state, not manager tracking: after
    /// this event fires the session may still be present in the manager
    /// in the `Disconnected` state (e.g. when the stream dropped on its
    /// own). Use `SessionDeregistered` to observe removal from the manager.
    SessionDisconnected {
        session_id: SessionId,
        host_id: HostId,
    },
    /// The manager is no longer tracking this session -- it has been
    /// removed from the `sessions` map via `deregister_session`. Fires
    /// exactly once per session, and only on explicit teardown (never as
    /// a result of a spontaneous connection drop).
    ///
    /// If the session was `Connected` at the point of deregistration, a
    /// `SessionDisconnected` event is emitted first so transport-level
    /// subscribers can release their client references.
    SessionDeregistered { session_id: SessionId },

    // --- Host-scoped events ---
    /// The first session for this host reached `Connected`. Downstream
    /// features should create per-host models (e.g. `RepoMetadataModel`).
    HostConnected { host_id: HostId },
    /// The last session for this host was disconnected or deregistered.
    /// Downstream features should tear down per-host models.
    HostDisconnected { host_id: HostId },

    // --- Repo metadata events (forwarded from ClientEvent push channel) ---
    /// Response to a `navigate_to_directory` request.
    NavigatedToDirectory {
        session_id: SessionId,
        host_id: HostId,
        indexed_path: String,
        is_git: bool,
    },
    /// A full or lazy-loaded repo metadata snapshot was pushed by the server.
    RepoMetadataSnapshot {
        host_id: HostId,
        update: RepoMetadataUpdate,
    },
    /// An incremental repo metadata update was pushed by the server.
    RepoMetadataUpdated {
        host_id: HostId,
        update: RepoMetadataUpdate,
    },
    /// A `LoadRepoMetadataDirectory` response was received from the server.
    RepoMetadataDirectoryLoaded {
        host_id: HostId,
        update: RepoMetadataUpdate,
    },

    // --- Setup events ---
    /// Intermediate state change during the binary check/install flow.
    SetupStateChanged {
        session_id: SessionId,
        state: RemoteServerSetupState,
    },
    /// Result of [`RemoteServerManager::check_binary`]. Returns a result where:
    /// - `Ok(true)` means the binary is installed and executable,
    /// - `Ok(false)` means it is definitively not installed, and
    /// - `Err(_)` means the check itself failed (e.g. SSH error or timeout).
    BinaryCheckComplete {
        session_id: SessionId,
        result: Result<bool, String>,
        /// The detected remote platform (OS + arch) from `uname -sm`.
        /// `None` if detection failed or was not attempted.
        remote_platform: Option<RemotePlatform>,
    },
    /// Result of [`RemoteServerManager::install_binary`]. Returns a result where:
    /// - `Ok(())` means the install succeeded, and
    /// - `Err(_)` means the install failed and carries the failure reason (SSH error, timeout, script error, etc.).
    BinaryInstallComplete {
        session_id: SessionId,
        result: Result<(), String>,
    },

    // --- Telemetry events ---
    /// A client request to the remote server failed.
    ClientRequestFailed {
        session_id: SessionId,
        operation: RemoteServerOperation,
        error_kind: RemoteServerErrorKind,
    },
    /// A server message could not be decoded (no parseable request_id).
    ServerMessageDecodingError { session_id: SessionId },
}

/// Shell info stashed by [`RemoteServerManager::notify_session_bootstrapped`]
/// when the session is not yet in `Connected` state. Flushed automatically
/// when [`RemoteServerManager::mark_session_connected`] fires.
#[cfg_attr(target_family = "wasm", allow(dead_code))]
struct PendingSessionBootstrappedNotification {
    shell_type: String,
    shell_path: Option<String>,
}

/// Singleton model that manages connections to `remote_server` processes on
/// remote hosts.
///
/// Each SSH session gets its own `RemoteServerClient` and SSH connection.
/// Deduplication of the underlying long-lived server process happens on the
/// remote host. The `HostId` returned by the server's `InitializeResponse`
/// is used on the client to deduplicate host-scoped models (e.g.
/// `RepoMetadataModel`), not connections.
pub struct RemoteServerManager {
    /// Per-session connection state. Each SSH session gets its own dedicated
    /// connection to the remote server.
    sessions: HashMap<SessionId, RemoteSessionState>,
    /// Reverse index: host → sessions for O(1) lookup by `HostId`.
    host_to_sessions: HashMap<HostId, HashSet<SessionId>>,
    /// Spawner for running closures back on the main thread.
    spawner: ModelSpawner<Self>,
    /// Last path requested per session for dedup. Avoids redundant
    /// `navigate_to_directory` calls when `update_active_session` fires
    /// repeatedly for the same CWD.
    last_navigated_path: HashMap<SessionId, String>,
    /// Per-session `SessionBootstrapped` notifications that arrived before the
    /// session reached `Connected`. Flushed in `mark_session_connected`.
    pending_bootstrapped_notifications: HashMap<SessionId, PendingSessionBootstrappedNotification>,
    /// Detected remote platform per session, populated during the binary check
    /// phase via `detect_platform()`. Used for telemetry.
    session_platforms: HashMap<SessionId, RemotePlatform>,
}

impl Entity for RemoteServerManager {
    type Event = RemoteServerManagerEvent;
}

impl SingletonEntity for RemoteServerManager {}

impl RemoteServerManager {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        Self {
            sessions: HashMap::new(),
            host_to_sessions: HashMap::new(),
            spawner: ctx.spawner(),
            last_navigated_path: HashMap::new(),
            pending_bootstrapped_notifications: HashMap::new(),
            session_platforms: HashMap::new(),
        }
    }

    /// Returns a connected client for the given host by picking an arbitrary
    /// session from the host's session pool.
    pub fn client_for_host(&self, host_id: &HostId) -> Option<&Arc<RemoteServerClient>> {
        let sessions = self.host_to_sessions.get(host_id)?;
        sessions
            .iter()
            .find_map(|session_id| self.client_for_session(*session_id))
    }

    /// Checks if the remote server binary is installed and executable.
    /// Emits `BinaryCheckComplete { result }`.
    ///
    /// Returns Ok(true) if the binary is installed and executable,
    /// Ok(false) if it is definitively not installed, and
    /// Err(_) if the check failed (e.g. SSH timeout/unreachable).
    #[cfg_attr(target_family = "wasm", allow(unused_variables))]
    pub fn check_binary<T>(
        &mut self,
        session_id: SessionId,
        transport: T,
        ctx: &mut ModelContext<Self>,
    ) where
        T: RemoteTransport + 'static,
    {
        #[cfg(target_family = "wasm")]
        {
            log::warn!("check_binary is a no-op on WASM");
        }

        #[cfg(not(target_family = "wasm"))]
        {
            ctx.emit(RemoteServerManagerEvent::SetupStateChanged {
                session_id,
                state: RemoteServerSetupState::Checking,
            });
            let spawner = self.spawner.clone();
            ctx.background_executor()
                .spawn(async move {
                    // Run platform detection and binary check concurrently.
                    let (platform_result, check_result) =
                        futures::join!(transport.detect_platform(), transport.check_binary(),);
                    let platform = match platform_result {
                        Ok(p) => Some(p),
                        Err(e) => {
                            log::warn!("Platform detection failed for session {session_id:?}: {e}");
                            None
                        }
                    };
                    let _ = spawner
                        .spawn(move |me, ctx| {
                            if let Some(ref p) = platform {
                                me.session_platforms.insert(session_id, p.clone());
                            }
                            ctx.emit(RemoteServerManagerEvent::BinaryCheckComplete {
                                session_id,
                                result: check_result,
                                remote_platform: platform,
                            });
                        })
                        .await;
                })
                .detach();
        }
    }

    /// Installs the remote server binary.
    /// Emits `BinaryInstallComplete { result }`.
    ///
    /// Returns Ok(()) if the install succeeded, and
    /// Err(_) if the install failed (e.g. SSH timeout/unreachable).
    #[cfg_attr(target_family = "wasm", allow(unused_variables))]
    pub fn install_binary<T>(
        &mut self,
        session_id: SessionId,
        transport: T,
        ctx: &mut ModelContext<Self>,
    ) where
        T: RemoteTransport + 'static,
    {
        #[cfg(target_family = "wasm")]
        {
            log::warn!("install_binary is a no-op on WASM");
        }

        #[cfg(not(target_family = "wasm"))]
        {
            ctx.emit(RemoteServerManagerEvent::SetupStateChanged {
                session_id,
                state: RemoteServerSetupState::Installing {
                    progress_percent: None,
                },
            });
            let spawner = self.spawner.clone();
            ctx.background_executor()
                .spawn(async move {
                    let result = transport.install_binary().await;
                    let _ = spawner
                        .spawn(move |_me, ctx| {
                            ctx.emit(RemoteServerManagerEvent::BinaryInstallComplete {
                                session_id,
                                result,
                            });
                        })
                        .await;
                })
                .detach();
        }
    }

    /// Entry point for establishing a remote server connection for a session.
    /// This assumes the binary is already installed and executable.
    /// Callers should first call `check_binary` and `install_binary` to ensure the binary is present.
    ///
    /// The full flow is:
    /// 1. **Connect** — `transport.connect()` establishes the I/O streams and
    ///    creates the `RemoteServerClient`.
    /// 2. **Handshake** — perform the initialize handshake (which returns the
    ///    `HostId`) and transition to `Connected`.
    ///
    /// No-op on WASM (remote server connections use a different transport).
    #[cfg_attr(target_family = "wasm", allow(unused_variables, unused_mut))]
    pub fn connect_session<T>(
        &mut self,
        session_id: SessionId,
        transport: T,
        ctx: &mut ModelContext<Self>,
    ) where
        T: RemoteTransport + 'static,
    {
        #[cfg(target_family = "wasm")]
        {
            log::warn!("connect_session is a no-op on WASM");
        }

        #[cfg(not(target_family = "wasm"))]
        {
            log::info!("Starting remote server connection for session {session_id:?}");

            // Advance the user-visible setup pipeline. Both callers (binary
            // already installed, and binary just installed) enter this
            // method right when the Initializing phase begins, so we emit
            // the state change from one place.
            ctx.emit(RemoteServerManagerEvent::SetupStateChanged {
                session_id,
                state: RemoteServerSetupState::Initializing,
            });

            self.sessions
                .insert(session_id, RemoteSessionState::Connecting);
            ctx.emit(RemoteServerManagerEvent::SessionConnecting { session_id });

            let spawner = self.spawner.clone();
            let executor = ctx.background_executor().clone();

            ctx.background_executor()
                .spawn(async move {
                    // ---- Phase 1: Connect (establish streams, create client) ----
                    match transport.connect(&executor).await {
                        Ok(Connection {
                            client,
                            event_rx,
                            child,
                            control_path,
                        }) => {
                            let client = Arc::new(client);

                            // Transition to Initializing and start draining
                            // the event channel for push events and disconnect.
                            // The `Child` is stashed on the session state so
                            // its lifetime is controlled by the manager -- on
                            // teardown the state is dropped, which runs the
                            // `Child`'s destructor and SIGKILLs the subprocess
                            // via `kill_on_drop`. `control_path` is stashed
                            // for explicit teardown's `ssh -O exit` call.
                            let client_for_state = Arc::clone(&client);
                            let _ = spawner
                                .spawn(move |me, ctx| {
                                    me.sessions.insert(
                                        session_id,
                                        RemoteSessionState::Initializing {
                                            client: client_for_state,
                                            _child: child,
                                            control_path,
                                        },
                                    );

                                    // Drain the event channel on the main thread.
                                    // Each push event is forwarded as a manager
                                    // event in real-time. When the stream closes
                                    // (after Disconnected or channel drop), we
                                    // transition the session to Disconnected.
                                    ctx.spawn_stream_local(
                                        event_rx,
                                        move |me, event, ctx| {
                                            me.forward_client_event(session_id, event, ctx);
                                        },
                                        move |me, ctx| {
                                            me.mark_session_disconnected(session_id, ctx);
                                        },
                                    );
                                })
                                .await;

                            // ---- Phase 2: Initialize handshake ----
                            match client.initialize().await {
                                Ok(resp) => {
                                    let host_id = HostId::new(resp.host_id);
                                    let _ = spawner
                                        .spawn(move |me, ctx| {
                                            me.mark_session_connected(session_id, host_id, ctx);
                                        })
                                        .await;
                                }
                                Err(e) => {
                            log::error!(
                                        "Initialize handshake failed for session {session_id:?}: {e}"
                                    );
                                    let error = format!("{e:#}");
                                    let _ = spawner
                                        .spawn(move |me, ctx| {
                                            ctx.emit(
                                                RemoteServerManagerEvent::SetupStateChanged {
                                                    session_id,
                                                    state: RemoteServerSetupState::Failed {
                                                        error: error.clone(),
                                                    },
                                                },
                                            );
                                            ctx.emit(
                                                RemoteServerManagerEvent::SessionConnectionFailed {
                                                    session_id,
                                                    phase: RemoteServerInitPhase::Initialize,
                                                    error,
                                                },
                                            );
                                            me.mark_session_disconnected(session_id, ctx);
                                        })
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to connect remote server for session {session_id:?}: {e:#}"
                            );
                            let error = format!("{e:#}");
                            let _ = spawner
                                .spawn(move |me, ctx| {
                                    ctx.emit(RemoteServerManagerEvent::SetupStateChanged {
                                        session_id,
                                        state: RemoteServerSetupState::Failed {
                                            error: error.clone(),
                                        },
                                    });
                                    ctx.emit(RemoteServerManagerEvent::SessionConnectionFailed {
                                        session_id,
                                        phase: RemoteServerInitPhase::Connect,
                                        error,
                                    });
                                    me.mark_session_disconnected(session_id, ctx);
                                })
                                .await;
                        }
                    }
                })
                .detach();
        }
    }

    /// Removes a session from the manager and tears down its connection.
    ///
    /// Assumes the caller has already observed that the user's shell
    /// has exited (in practice this is only invoked from the
    /// `ExitShell` teardown path). Under that assumption we also force
    /// the local SSH `ControlMaster` to exit immediately via
    /// `ssh -O exit`, which is required because the master is the
    /// user's interactive ssh process and, without the explicit
    /// `-O exit`, it hangs waiting for remote-side cleanup of
    /// multiplexed channels (see [`crate::ssh::stop_control_master`]).
    ///
    /// Mechanically:
    /// 1. Remove the session entry. Dropping the `RemoteSessionState`
    ///    drops the transport's owned `Child`, which SIGKILLs the
    ///    `ssh … remote-server-proxy` subprocess via `kill_on_drop`.
    /// 2. If the session had a ControlMaster `control_path`, spawn a
    ///    background task that runs `ssh -O exit` against it.
    ///
    /// The `Child` is owned by the manager's state, *not* by
    /// `Arc<RemoteServerClient>`. Lingering `Arc` clones held elsewhere
    /// (e.g. by the per-session command executor) do *not* keep the
    /// subprocess alive -- removing the state here always SIGKILLs the
    /// child, regardless of client refcount.
    ///
    /// Two separate events can fire here, and they mean different things:
    ///
    /// * `SessionDisconnected` -- the *transport* went away. Emitted only
    ///   when the session was `Connected` at the time of deregistration.
    ///   Subscribers can use this to drop their
    ///   `Arc<RemoteServerClient>` references and cancel in-flight
    ///   requests. The same event also fires independently from
    ///   `mark_session_disconnected` when the stream drops on its own.
    /// * `SessionDeregistered` -- the manager is no longer *tracking* this
    ///   session. Always emitted, regardless of which state the session
    ///   was in, because the entry is being removed from `sessions`
    ///   outright. Unlike `SessionDisconnected`, this one never fires for
    ///   spontaneous drops -- only for explicit teardown.
    pub fn deregister_session(&mut self, session_id: SessionId, ctx: &mut ModelContext<Self>) {
        self.last_navigated_path.remove(&session_id);
        self.pending_bootstrapped_notifications.remove(&session_id);
        self.session_platforms.remove(&session_id);

        // Remove the session entry. Dropping the `RemoteSessionState`
        // here drops the transport's owned `Child` (if any), which
        // SIGKILLs the `ssh … remote-server-proxy` subprocess via
        // `kill_on_drop`.
        let prev = self.sessions.remove(&session_id);

        // Extract the ControlMaster socket path (if any) so we can
        // force the master to exit below. Safe to do under the
        // "caller already observed ExitShell" assumption documented
        // above.
        #[cfg(not(target_family = "wasm"))]
        let control_path = match &prev {
            Some(RemoteSessionState::Connected { control_path, .. })
            | Some(RemoteSessionState::Initializing { control_path, .. }) => control_path.clone(),
            _ => None,
        };

        if let Some(RemoteSessionState::Connected { host_id, .. }) = prev {
            self.remove_from_host_index(&host_id, session_id);
            ctx.emit(RemoteServerManagerEvent::SessionDisconnected {
                session_id,
                host_id: host_id.clone(),
            });
            if !self.host_to_sessions.contains_key(&host_id) {
                ctx.emit(RemoteServerManagerEvent::HostDisconnected {
                    host_id: host_id.clone(),
                });
            }
        }
        ctx.emit(RemoteServerManagerEvent::SessionDeregistered { session_id });

        // Force the local SSH ControlMaster to exit after teardown.
        // Spawned detached because the ssh subcommand may take a moment
        // to complete and we don't want to block the main thread on it.
        #[cfg(not(target_family = "wasm"))]
        if let Some(control_path) = control_path {
            ctx.background_executor()
                .spawn(async move {
                    crate::ssh::stop_control_master(&control_path).await;
                })
                .detach();
        }
    }

    /// Returns the client for this session, if connected.
    pub fn client_for_session(&self, session_id: SessionId) -> Option<&Arc<RemoteServerClient>> {
        match self.sessions.get(&session_id) {
            Some(RemoteSessionState::Connected { client, .. }) => Some(client),
            _ => None,
        }
    }

    /// Returns the connection state for this session.
    pub fn session(&self, session_id: SessionId) -> Option<&RemoteSessionState> {
        self.sessions.get(&session_id)
    }

    /// Returns the detected remote platform for this session, if available.
    pub fn platform_for_session(&self, session_id: SessionId) -> Option<&RemotePlatform> {
        self.session_platforms.get(&session_id)
    }

    /// Returns the `HostId` for this session, if the initialize handshake
    /// has completed. Downstream features use this to deduplicate
    /// host-scoped models (e.g. `RepoMetadataModel`).
    pub fn host_id_for_session(&self, session_id: SessionId) -> Option<&HostId> {
        match self.sessions.get(&session_id) {
            Some(RemoteSessionState::Connected { host_id, .. }) => Some(host_id),
            _ => None,
        }
    }

    /// Returns all session IDs connected to a given host. O(1) via the
    /// reverse index.
    pub fn sessions_for_host(&self, host_id: &HostId) -> Option<&HashSet<SessionId>> {
        self.host_to_sessions.get(host_id)
    }

    /// Sends a `NavigatedToDirectory` request to the remote server for
    /// the given session and emits the response as a manager event.
    ///
    /// Deduplicates: if the same `(session_id, path)` was already requested,
    /// the call is a no-op.
    pub fn navigate_to_directory(
        &mut self,
        session_id: SessionId,
        path: String,
        ctx: &mut ModelContext<Self>,
    ) {
        // Dedup: skip if this session already navigated to the same path.
        if self.last_navigated_path.get(&session_id) == Some(&path) {
            return;
        }

        let Some(client) = self.client_for_session(session_id).cloned() else {
            log::warn!("navigate_to_directory: no connected client for session {session_id:?}");
            return;
        };
        let Some(host_id) = self.host_id_for_session(session_id).cloned() else {
            log::warn!("navigate_to_directory: no host_id for session {session_id:?}");
            return;
        };

        // Record only after confirming the client is connected, so that a
        // retry after SessionConnected is not incorrectly deduplicated.
        self.last_navigated_path.insert(session_id, path.clone());

        let spawner = self.spawner.clone();
        ctx.background_executor()
            .spawn(async move {
                match client.navigate_to_directory(path).await {
                    Ok(resp) => {
                        let _ = spawner
                            .spawn(move |_me, ctx| {
                                ctx.emit(RemoteServerManagerEvent::NavigatedToDirectory {
                                    session_id,
                                    host_id,
                                    indexed_path: resp.indexed_path,
                                    is_git: resp.is_git,
                                });
                            })
                            .await;
                    }
                    Err(e) => {
                        log::error!("navigate_to_directory failed for session {session_id:?}: {e}");
                        let error_kind = RemoteServerErrorKind::from_client_error(&e);
                        let _ = spawner
                            .spawn(move |_me, ctx| {
                                ctx.emit(RemoteServerManagerEvent::ClientRequestFailed {
                                    session_id,
                                    operation: RemoteServerOperation::NavigateToDirectory,
                                    error_kind,
                                });
                            })
                            .await;
                    }
                }
            })
            .detach();
    }

    /// Sends a `SessionBootstrapped` notification to the remote server.
    ///
    /// If the session is already in `Connected` state the notification is sent
    /// immediately. Otherwise it is stashed and automatically flushed when
    /// `mark_session_connected` transitions the session to `Connected`.
    pub fn notify_session_bootstrapped(
        &mut self,
        session_id: SessionId,
        shell_type: &str,
        shell_path: Option<&str>,
    ) {
        if let Some(client) = self.client_for_session(session_id) {
            client.notify_session_bootstrapped(session_id, shell_type, shell_path);
        } else {
            log::info!(
                "notify_session_bootstrapped: session {session_id:?} not yet connected, \
                 stashing notification"
            );
            self.pending_bootstrapped_notifications.insert(
                session_id,
                PendingSessionBootstrappedNotification {
                    shell_type: shell_type.to_owned(),
                    shell_path: shell_path.map(ToOwned::to_owned),
                },
            );
        }
    }

    /// Sends a `LoadRepoMetadataDirectory` request to the remote server for
    /// the given session and emits the response as a manager event.
    pub fn load_remote_repo_metadata_directory(
        &mut self,
        session_id: SessionId,
        repo_path: String,
        dir_path: String,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(client) = self.client_for_session(session_id).cloned() else {
            log::warn!(
                "load_remote_repo_metadata_directory: no connected client for session {session_id:?}"
            );
            return;
        };
        let Some(host_id) = self.host_id_for_session(session_id).cloned() else {
            log::warn!(
                "load_remote_repo_metadata_directory: no host_id for session {session_id:?}"
            );
            return;
        };

        let spawner = self.spawner.clone();
        ctx.background_executor()
            .spawn(async move {
                match client
                    .load_repo_metadata_directory(repo_path, dir_path)
                    .await
                {
                    Ok(resp) => {
                        if let Some(update) =
                            crate::repo_metadata_proto::proto_load_repo_metadata_directory_response_to_update(&resp)
                        {
                            let _ = spawner
                                .spawn(move |_me, ctx| {
                                    ctx.emit(
                                        RemoteServerManagerEvent::RepoMetadataDirectoryLoaded {
                                            host_id,
                                            update,
                                        },
                                    );
                                })
                                .await;
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "load_repo_metadata_directory failed for session {session_id:?}: {e}"
                        );
                        let error_kind = RemoteServerErrorKind::from_client_error(&e);
                        let _ = spawner
                            .spawn(move |_me, ctx| {
                                ctx.emit(RemoteServerManagerEvent::ClientRequestFailed {
                                    session_id,
                                    operation: RemoteServerOperation::LoadRepoMetadataDirectory,
                                    error_kind,
                                });
                            })
                            .await;
                    }
                }
            })
            .detach();
    }

    /// Forwards a push event from the client event channel as a manager event.
    /// No-ops if the session is not in `Connected` state (i.e. `host_id` not
    /// yet available).
    #[cfg(not(target_family = "wasm"))]
    fn forward_client_event(
        &self,
        session_id: SessionId,
        event: ClientEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(host_id) = self.host_id_for_session(session_id) else {
            log::debug!("Dropping push event for session {session_id:?}: not connected yet");
            return;
        };
        let host_id = host_id.clone();

        match event {
            ClientEvent::RepoMetadataSnapshotReceived { update } => {
                ctx.emit(RemoteServerManagerEvent::RepoMetadataSnapshot { host_id, update });
            }
            ClientEvent::RepoMetadataUpdated { update } => {
                ctx.emit(RemoteServerManagerEvent::RepoMetadataUpdated { host_id, update });
            }
            ClientEvent::MessageDecodingError => {
                ctx.emit(RemoteServerManagerEvent::ServerMessageDecodingError { session_id });
            }
            ClientEvent::Disconnected => {
                // Handled by the drain loop's completion callback.
            }
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn mark_session_connected(
        &mut self,
        session_id: SessionId,
        host_id: HostId,
        ctx: &mut ModelContext<Self>,
    ) {
        log::info!("Remote server connected for session {session_id:?}, host {host_id}");

        // Only transition if the session is still in Initializing state.
        // Remove first so we can move the client handle (and owned `Child`)
        // out.
        let Some(RemoteSessionState::Initializing {
            client,
            _child,
            control_path,
        }) = self.sessions.remove(&session_id)
        else {
            return;
        };

        let is_first_session = !self.host_to_sessions.contains_key(&host_id);
        self.sessions.insert(
            session_id,
            RemoteSessionState::Connected {
                client,
                host_id: host_id.clone(),
                _child,
                control_path,
            },
        );
        self.host_to_sessions
            .entry(host_id.clone())
            .or_default()
            .insert(session_id);
        if is_first_session {
            ctx.emit(RemoteServerManagerEvent::HostConnected {
                host_id: host_id.clone(),
            });
        }
        ctx.emit(RemoteServerManagerEvent::SetupStateChanged {
            session_id,
            state: RemoteServerSetupState::Ready,
        });
        ctx.emit(RemoteServerManagerEvent::SessionConnected {
            session_id,
            host_id,
        });

        // Flush any SessionBootstrapped notification that was stashed before
        // the session reached Connected.
        if let Some(notif) = self.pending_bootstrapped_notifications.remove(&session_id) {
            if let Some(client) = self.client_for_session(session_id) {
                log::info!(
                    "Flushing stashed SessionBootstrapped notification for session \
                     {session_id:?}"
                );
                client.notify_session_bootstrapped(
                    session_id,
                    &notif.shell_type,
                    notif.shell_path.as_deref(),
                );
            }
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn mark_session_disconnected(
        &mut self,
        session_id: SessionId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.pending_bootstrapped_notifications.remove(&session_id);
        let Some(prev) = self.sessions.remove(&session_id) else {
            return;
        };
        self.sessions
            .insert(session_id, RemoteSessionState::Disconnected);

        if let RemoteSessionState::Connected { host_id, .. } = prev {
            self.remove_from_host_index(&host_id, session_id);
            // Emit `SessionDisconnected` before `HostDisconnected` so that
            // subscribers (e.g. the command executor) drop their
            // `Arc<RemoteServerClient>` reference before any host-scoped
            // teardown runs. This matches the ordering in
            // `deregister_session` so both teardown paths look identical
            // to subscribers.
            ctx.emit(RemoteServerManagerEvent::SessionDisconnected {
                session_id,
                host_id: host_id.clone(),
            });
            if !self.host_to_sessions.contains_key(&host_id) {
                ctx.emit(RemoteServerManagerEvent::HostDisconnected { host_id });
            }
        }
    }

    /// Removes a session from the host → sessions reverse index.
    /// Cleans up the entry entirely if the set becomes empty.
    fn remove_from_host_index(&mut self, host_id: &HostId, session_id: SessionId) {
        if let Some(set) = self.host_to_sessions.get_mut(host_id) {
            set.remove(&session_id);
            if set.is_empty() {
                self.host_to_sessions.remove(host_id);
            }
        }
    }
}
